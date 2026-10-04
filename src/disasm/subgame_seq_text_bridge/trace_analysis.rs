//! Entry trace의 jr table 확장과 jalr source 분류.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::coords::{normalize_kseg, ExeMap};
use crate::isa::{FlowKind, Instruction, InstructionKind as K, Reg};

use super::super::{trace, xref, Trace};
use super::indirect_sources::find_stack_arg5_jalr_candidates;
use super::{
    error, SubgameSeqTextBridgeError, SubgameSeqTextBridgeSpec, SubgameSeqTextExpandedTrace,
    SubgameSeqTextJalrForwardedSource, SubgameSeqTextJalrInitializedSource,
    SubgameSeqTextJalrInitializedTableSource, SubgameSeqTextJalrRuntimeFieldSource,
    SubgameSeqTextJalrRuntimeSource, SubgameSeqTextJrTableSource,
};

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, SubgameSeqTextBridgeError> {
    instructions
        .binary_search_by_key(&address, |instruction| instruction.addr)
        .ok()
        .map(|index| &instructions[index])
        .ok_or_else(|| error(format!("text bridge 명령 주소 없음: 0x{address:08X}")))
}

pub(super) fn require(
    instructions: &[Instruction],
    spec: SubgameSeqTextBridgeSpec,
    address: u32,
    expected: K,
) -> Result<(), SubgameSeqTextBridgeError> {
    let actual = instruction_at(instructions, address)?;
    if actual.kind != expected {
        return Err(error(format!(
            "{} text bridge 명령 불일치: address=0x{address:08X}, expected={expected:?}, actual={:?}",
            spec.exe, actual.kind
        )));
    }
    Ok(())
}

pub(super) fn high_low(address: u32) -> (u16, i16) {
    let high = address.wrapping_add(0x8000) >> 16;
    (high as u16, address as u16 as i16)
}

fn previous_definition(
    instructions: &[Instruction],
    before: usize,
    register: Reg,
) -> Option<(usize, Instruction)> {
    instructions[..before]
        .iter()
        .enumerate()
        .rev()
        .find(|(_, instruction)| instruction.dest_reg() == Some(register))
        .map(|(index, instruction)| (index, *instruction))
}

fn constant_before(instructions: &[Instruction], before: usize, register: Reg) -> Option<u32> {
    let (index, definition) = previous_definition(instructions, before, register)?;
    if before.saturating_sub(index) > 8 {
        return None;
    }
    match definition.kind {
        K::Lui { rt, imm } if rt == register => Some(u32::from(imm) << 16),
        K::Addiu { rt, rs, imm } if rt == register && rs == register => {
            let (high_index, high) = previous_definition(instructions, index, register)?;
            let K::Lui {
                rt: high_rt,
                imm: high_imm,
            } = high.kind
            else {
                return None;
            };
            (high_rt == register && index.saturating_sub(high_index) <= 8)
                .then_some((u32::from(high_imm) << 16).wrapping_add(imm as i32 as u32))
        }
        K::Ori { rt, rs, imm } if rt == register && rs == register => {
            let (high_index, high) = previous_definition(instructions, index, register)?;
            let K::Lui {
                rt: high_rt,
                imm: high_imm,
            } = high.kind
            else {
                return None;
            };
            (high_rt == register && index.saturating_sub(high_index) <= 8)
                .then_some((u32::from(high_imm) << 16) | u32::from(imm))
        }
        _ => None,
    }
}

/// `jr` target가 bounded index×4 absolute-word table에서 온 경우의 source.
/// guard의 의미 전체를 복원하는 분석은 아니며 local compiler shape만 받는다.
pub(super) fn jr_table_source(
    instructions: &[Instruction],
    jr_index: usize,
    target_reg: Reg,
) -> Option<(u32, usize)> {
    let (load_index, load) = previous_definition(instructions, jr_index, target_reg)?;
    let K::Lw {
        rt,
        base: address_reg,
        offset,
    } = load.kind
    else {
        return None;
    };
    if rt != target_reg || jr_index.saturating_sub(load_index) > 3 {
        return None;
    }

    let (address_index, address) = previous_definition(instructions, load_index, address_reg)?;
    let K::Addu { rd, rs, rt } = address.kind else {
        return None;
    };
    if rd != address_reg || load_index.saturating_sub(address_index) > 3 {
        return None;
    }

    let mut source = None;
    for (scaled_reg, constant_reg) in [(rs, rt), (rt, rs)] {
        let Some((scaled_index, scaled)) =
            previous_definition(instructions, address_index, scaled_reg)
        else {
            continue;
        };
        let K::Sll {
            rd: scaled_rd,
            rt: index_reg,
            sa: 2,
        } = scaled.kind
        else {
            continue;
        };
        if scaled_rd != scaled_reg || address_index.saturating_sub(scaled_index) > 8 {
            continue;
        }
        let Some(base) = constant_before(instructions, address_index, constant_reg) else {
            continue;
        };
        source = Some((base.wrapping_add(offset as i32 as u32), index_reg));
        break;
    }
    let (table_runtime, index_reg) = source?;

    let search_start = jr_index.saturating_sub(16);
    let (guard_reg, entries) = instructions[search_start..jr_index].iter().rev().find_map(
        |instruction| match instruction.kind {
            K::Sltiu { rt, rs, imm } | K::Slti { rt, rs, imm } if rs == index_reg && imm > 0 => {
                usize::try_from(imm).ok().map(|entries| (rt, entries))
            }
            _ => None,
        },
    )?;
    let guarded = instructions[search_start..load_index]
        .iter()
        .any(|instruction| {
            matches!(
                instruction.kind,
                K::Beq {
                    rs,
                    rt: Reg::ZERO,
                    ..
                } if rs == guard_reg
            ) || matches!(
                instruction.kind,
                K::Beq {
                    rs: Reg::ZERO,
                    rt,
                    ..
                } if rt == guard_reg
            )
        });
    guarded.then_some((table_runtime, entries))
}

pub(super) fn is_bios_vector_jr(
    instructions: &[Instruction],
    jr_index: usize,
    target_reg: Reg,
) -> bool {
    previous_definition(instructions, jr_index, target_reg).is_some_and(|(index, definition)| {
        jr_index.saturating_sub(index) <= 2
            && matches!(
                definition.kind,
                K::Addiu {
                    rt,
                    rs: Reg::ZERO,
                    imm: 0xA0 | 0xB0 | 0xC0,
                } if rt == target_reg
            )
    })
}

#[derive(Debug, Default)]
struct TraceJrSources {
    bios_sites: Vec<u32>,
    table_sources: Vec<SubgameSeqTextJrTableSource>,
    unclassified_sites: Vec<u32>,
}

fn classify_trace_jr_sources(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    entry_trace: &Trace,
    spec: SubgameSeqTextBridgeSpec,
) -> Result<TraceJrSources, SubgameSeqTextBridgeError> {
    let mut sources = TraceJrSources::default();
    let constructor = normalize_kseg(spec.constructor_runtime);
    for &runtime in &entry_trace.indirect_sites {
        let Some(instruction) = entry_trace.instructions.get(&runtime) else {
            return Err(error(format!(
                "{} entry trace 간접 site 명령 누락: 0x{runtime:08X}",
                spec.exe
            )));
        };
        let K::Jr { rs: target_reg } = instruction.kind else {
            continue;
        };
        if target_reg == Reg::RA {
            return Err(error(format!(
                "{} entry trace에 return jr가 간접 site로 들어옴: 0x{runtime:08X}",
                spec.exe
            )));
        }
        let jr_index = instructions
            .binary_search_by_key(&runtime, |candidate| candidate.addr)
            .map_err(|_| error(format!("{} jr linear 명령 누락", spec.exe)))?;
        if is_bios_vector_jr(instructions, jr_index, target_reg) {
            sources.bios_sites.push(runtime);
            continue;
        }
        let Some((table_runtime, table_entries)) =
            jr_table_source(instructions, jr_index, target_reg)
        else {
            sources.unclassified_sites.push(runtime);
            continue;
        };
        let table_offset = map
            .text_offset(table_runtime)
            .ok_or_else(|| error(format!("{} jr table이 EXE 밖임", spec.exe)))?;
        let table_len = table_entries
            .checked_mul(4)
            .ok_or_else(|| error(format!("{} jr table 길이 overflow", spec.exe)))?;
        let table_end = table_offset
            .checked_add(table_len)
            .ok_or_else(|| error(format!("{} jr table 끝 overflow", spec.exe)))?;
        let table = text
            .get(table_offset..table_end)
            .ok_or_else(|| error(format!("{} jr table 범위 이탈", spec.exe)))?;
        let constructor_targets = table
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes(word.try_into().expect("4바이트 word")))
            .filter(|&target| normalize_kseg(target) == constructor)
            .count();
        sources.table_sources.push(SubgameSeqTextJrTableSource {
            call_runtime: runtime,
            table_runtime,
            table_entries,
            constructor_targets,
        });
    }

    Ok(sources)
}

pub(super) fn classify_entry_jr_sources(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    entry_trace: &Trace,
    spec: SubgameSeqTextBridgeSpec,
) -> Result<(Vec<u32>, Vec<SubgameSeqTextJrTableSource>), SubgameSeqTextBridgeError> {
    let TraceJrSources {
        bios_sites,
        table_sources,
        unclassified_sites,
    } = classify_trace_jr_sources(map, text, instructions, entry_trace, spec)?;
    let jr_sites = bios_sites.len() + table_sources.len();
    if jr_sites != spec.expected_entry_jr_sites
        || bios_sites.len() != spec.expected_entry_jr_bios_sites
        || table_sources.len() != spec.expected_entry_jr_table_sites
        || !unclassified_sites.is_empty()
        || table_sources
            .iter()
            .any(|source| source.constructor_targets != 0)
    {
        return Err(error(format!(
            "{} entry jr 기준선 불일치: sites={}/{}, bios={}/{}, tables={}/{}, unclassified={unclassified_sites:?}, constructor={}",
            spec.exe,
            jr_sites,
            spec.expected_entry_jr_sites,
            bios_sites.len(),
            spec.expected_entry_jr_bios_sites,
            table_sources.len(),
            spec.expected_entry_jr_table_sites,
            table_sources
                .iter()
                .map(|source| source.constructor_targets)
                .sum::<usize>(),
        )));
    }
    Ok((bios_sites, table_sources))
}

pub(super) fn collect_jr_table_targets(
    map: &ExeMap,
    text: &[u8],
    sources: &[SubgameSeqTextJrTableSource],
) -> Result<(BTreeSet<u32>, BTreeSet<u32>), SubgameSeqTextBridgeError> {
    let mut inside = BTreeSet::new();
    let mut outside = BTreeSet::new();
    for source in sources {
        let table_offset = map
            .text_offset(source.table_runtime)
            .ok_or_else(|| error(format!("{} jr table이 EXE 밖임", map.name)))?;
        let table_len = source
            .table_entries
            .checked_mul(4)
            .ok_or_else(|| error(format!("{} jr table 길이 overflow", map.name)))?;
        let table_end = table_offset
            .checked_add(table_len)
            .ok_or_else(|| error(format!("{} jr table 끝 overflow", map.name)))?;
        let table = text
            .get(table_offset..table_end)
            .ok_or_else(|| error(format!("{} jr table 범위 이탈", map.name)))?;
        for target in table
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes(word.try_into().expect("4바이트 word")))
            .map(normalize_kseg)
        {
            if map.contains(target) && target.is_multiple_of(4) {
                inside.insert(target);
            } else {
                outside.insert(target);
            }
        }
    }
    Ok((inside, outside))
}

#[derive(Debug, Default)]
pub(super) struct ExpandedJalrSources {
    pub(super) bios_vector_sites: Vec<u32>,
    pub(super) initialized_direct_sources: BTreeMap<(u32, u32), Vec<u32>>,
    pub(super) initialized_rooted_sources: BTreeMap<(u32, u32), Vec<u32>>,
    pub(super) outside_text_sources: BTreeMap<u32, Vec<u32>>,
    pub(super) computed_memory_sources: Vec<SubgameSeqTextJalrRuntimeFieldSource>,
    pub(super) forwarded_register_sites: Vec<u32>,
    pub(super) unclassified_sites: Vec<u32>,
    pub(super) fresh_stack_arg5_sites: Vec<u32>,
}

pub(super) fn classify_expanded_jalr_sources(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    expanded: &Trace,
) -> Result<ExpandedJalrSources, SubgameSeqTextBridgeError> {
    let fresh_arg5 = find_stack_arg5_jalr_candidates(instructions)
        .into_iter()
        .map(|candidate| candidate.call_runtime)
        .collect::<BTreeSet<_>>();
    let mut sources = ExpandedJalrSources::default();
    for &runtime in &expanded.indirect_sites {
        let Some(instruction) = expanded.instructions.get(&runtime) else {
            return Err(error(format!(
                "{} expanded jalr site 명령 누락: 0x{runtime:08X}",
                map.name
            )));
        };
        let K::Jalr {
            rd: Reg::RA,
            rs: target_reg,
        } = instruction.kind
        else {
            continue;
        };
        if fresh_arg5.contains(&runtime) {
            sources.fresh_stack_arg5_sites.push(runtime);
        }
        let index = instructions
            .binary_search_by_key(&runtime, |candidate| candidate.addr)
            .map_err(|_| error(format!("{} expanded jalr linear 명령 누락", map.name)))?;
        let local_definition = previous_definition(instructions, index, target_reg)
            .filter(|(definition_index, _)| index.saturating_sub(*definition_index) <= 8);
        let Some((definition_index, definition)) = local_definition else {
            sources.forwarded_register_sites.push(runtime);
            continue;
        };
        match definition.kind {
            K::Addiu {
                rs: Reg::ZERO,
                imm: 0xA0 | 0xB0 | 0xC0,
                ..
            } => sources.bios_vector_sites.push(runtime),
            K::Addu { rs, rt, .. } if rs == Reg::ZERO || rt == Reg::ZERO => {
                sources.forwarded_register_sites.push(runtime);
            }
            K::Lw { base, offset, .. } => {
                let runtime_field_source = SubgameSeqTextJalrRuntimeFieldSource {
                    call_runtime: runtime,
                    load_runtime: definition.addr,
                    target_reg,
                    base_reg: base,
                    field_offset: offset,
                };
                if let Some(base_value) = constant_before(instructions, definition_index, base) {
                    let source_runtime = base_value.wrapping_add(offset as i32 as u32);
                    if map.contains(source_runtime) {
                        let initialized_target = read_word(map, text, source_runtime)?;
                        sources
                            .initialized_direct_sources
                            .entry((source_runtime, initialized_target))
                            .or_default()
                            .push(runtime);
                    } else {
                        sources
                            .outside_text_sources
                            .entry(source_runtime)
                            .or_default()
                            .push(runtime);
                    }
                    continue;
                }

                let rooted_source = previous_definition(instructions, definition_index, base)
                    .filter(|(base_index, _)| definition_index.saturating_sub(*base_index) <= 8)
                    .and_then(|(base_index, base_definition)| {
                        let K::Lw {
                            base: root_base,
                            offset: root_offset,
                            ..
                        } = base_definition.kind
                        else {
                            return None;
                        };
                        let root_base_value = constant_before(instructions, base_index, root_base)?;
                        Some(root_base_value.wrapping_add(root_offset as i32 as u32))
                    });
                let Some(root_source_runtime) = rooted_source else {
                    sources.computed_memory_sources.push(runtime_field_source);
                    continue;
                };
                if !map.contains(root_source_runtime) {
                    sources.computed_memory_sources.push(runtime_field_source);
                    continue;
                }
                let root = read_word(map, text, root_source_runtime)?;
                let source_runtime = root.wrapping_add(offset as i32 as u32);
                if !map.contains(source_runtime) {
                    sources.computed_memory_sources.push(runtime_field_source);
                    continue;
                }
                let initialized_target = read_word(map, text, source_runtime)?;
                sources
                    .initialized_rooted_sources
                    .entry((source_runtime, initialized_target))
                    .or_default()
                    .push(runtime);
            }
            _ => sources.unclassified_sites.push(runtime),
        }
    }
    Ok(sources)
}

fn initialized_sources(
    sources: BTreeMap<(u32, u32), Vec<u32>>,
) -> Vec<SubgameSeqTextJalrInitializedSource> {
    sources
        .into_iter()
        .map(|((source_runtime, initialized_target), call_sites)| {
            SubgameSeqTextJalrInitializedSource {
                source_runtime,
                initialized_target,
                call_sites,
            }
        })
        .collect()
}

pub(super) fn analyze_forwarded_jalr_sources(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    sites: &[u32],
    constructor_runtime: u32,
) -> Result<Vec<SubgameSeqTextJalrForwardedSource>, SubgameSeqTextBridgeError> {
    let address_loads = xref::find_address_loads(instructions, xref::DEFAULT_WINDOW);
    let constructor = normalize_kseg(constructor_runtime);
    let mut sources = Vec::with_capacity(sites.len());
    for &call_runtime in sites {
        let call_index = instructions
            .binary_search_by_key(&call_runtime, |instruction| instruction.addr)
            .map_err(|_| error(format!("{} forwarded jalr 명령 누락", map.name)))?;
        let K::Jalr {
            rd: Reg::RA,
            rs: target_reg,
        } = instructions[call_index].kind
        else {
            return Err(error(format!(
                "{} forwarded site가 jalr $ra가 아님: 0x{call_runtime:08X}",
                map.name
            )));
        };
        let (capture_index, capture) = previous_definition(instructions, call_index, target_reg)
            .ok_or_else(|| {
                error(format!(
                    "{} forwarded target capture가 없음: 0x{call_runtime:08X}",
                    map.name
                ))
            })?;
        let source_argument_reg = match capture.kind {
            K::Addu { rd, rs, rt } if rd == target_reg && rt == Reg::ZERO => rs,
            K::Addu { rd, rs, rt } if rd == target_reg && rs == Reg::ZERO => rt,
            _ => {
                return Err(error(format!(
                    "{} forwarded target capture shape 불일치: 0x{:08X}",
                    map.name, capture.addr
                )))
            }
        };
        let entry_index = instructions[..=capture_index]
            .iter()
            .rposition(|instruction| {
                matches!(
                    instruction.kind,
                    K::Addiu {
                        rt: Reg::SP,
                        rs: Reg::SP,
                        imm,
                    } if imm < 0
                )
            })
            .ok_or_else(|| error(format!("{} forwarded 함수 frame이 없음", map.name)))?;
        let function_entry_runtime = instructions[entry_index].addr;
        let mut direct_call_sites = Vec::new();
        let mut direct_call_targets = Vec::new();
        for (caller_index, caller) in instructions.iter().enumerate() {
            if !matches!(
                caller.flow_kind(),
                FlowKind::Call { target }
                    if normalize_kseg(target) == normalize_kseg(function_entry_runtime)
            ) {
                continue;
            }
            direct_call_sites.push(caller.addr);
            let target = constant_before(instructions, caller_index, source_argument_reg)
                .ok_or_else(|| {
                    error(format!(
                        "{} forwarded direct caller의 source argument가 상수가 아님: 0x{:08X}",
                        map.name, caller.addr
                    ))
                })?;
            direct_call_targets.push(target);
        }
        let entry_address_load_refs = address_loads
            .iter()
            .filter(|load| normalize_kseg(load.value) == normalize_kseg(function_entry_runtime))
            .count();
        let entry_raw_pointer_refs = count_raw_pointer(text, function_entry_runtime)
            + count_raw_pointer(text, function_entry_runtime | 0x2000_0000);
        let constructor_targets = direct_call_targets
            .iter()
            .filter(|&&target| normalize_kseg(target) == constructor)
            .count();
        sources.push(SubgameSeqTextJalrForwardedSource {
            call_runtime,
            target_reg,
            capture_runtime: capture.addr,
            function_entry_runtime,
            source_argument_reg,
            direct_call_sites,
            direct_call_targets,
            entry_address_load_refs,
            entry_raw_pointer_refs,
            constructor_targets,
        });
    }
    Ok(sources)
}

pub(super) fn analyze_outside_text_sources(
    instructions: &[Instruction],
    sources: BTreeMap<u32, Vec<u32>>,
) -> Vec<SubgameSeqTextJalrRuntimeSource> {
    let address_loads = xref::find_address_loads(instructions, xref::DEFAULT_WINDOW);
    sources
        .into_iter()
        .map(|(source_runtime, call_sites)| {
            let mut direct_read_sites = Vec::new();
            let mut direct_write_sites = Vec::new();
            let mut address_value_sites = Vec::new();
            for load in address_loads
                .iter()
                .filter(|load| load.value == source_runtime)
            {
                let Some(instruction) = instructions
                    .binary_search_by_key(&load.combine_addr, |instruction| instruction.addr)
                    .ok()
                    .map(|index| instructions[index])
                else {
                    continue;
                };
                match instruction.kind {
                    K::Lb { .. }
                    | K::Lbu { .. }
                    | K::Lh { .. }
                    | K::Lhu { .. }
                    | K::Lw { .. }
                    | K::Lwl { .. }
                    | K::Lwr { .. }
                    | K::Lwc2 { .. } => direct_read_sites.push(instruction.addr),
                    K::Sb { .. }
                    | K::Sh { .. }
                    | K::Sw { .. }
                    | K::Swl { .. }
                    | K::Swr { .. }
                    | K::Swc2 { .. } => direct_write_sites.push(instruction.addr),
                    _ => address_value_sites.push(instruction.addr),
                }
            }
            SubgameSeqTextJalrRuntimeSource {
                source_runtime,
                call_sites,
                direct_read_sites,
                direct_write_sites,
                address_value_sites,
            }
        })
        .collect()
}

pub(super) fn inspect_initialized_jalr_table(
    map: &ExeMap,
    text: &[u8],
    call_runtime: u32,
    table_runtime: u32,
    table_entries: usize,
    constructor_runtime: u32,
) -> Result<SubgameSeqTextJalrInitializedTableSource, SubgameSeqTextBridgeError> {
    let table_offset = map
        .text_offset(table_runtime)
        .ok_or_else(|| error(format!("{} jalr table이 EXE 밖임", map.name)))?;
    let table_len = table_entries
        .checked_mul(4)
        .ok_or_else(|| error(format!("{} jalr table 길이 overflow", map.name)))?;
    let table_end = table_offset
        .checked_add(table_len)
        .ok_or_else(|| error(format!("{} jalr table 끝 overflow", map.name)))?;
    let table = text
        .get(table_offset..table_end)
        .ok_or_else(|| error(format!("{} jalr table 범위 이탈", map.name)))?;
    let targets = table
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().expect("4바이트 word")))
        .collect::<Vec<_>>();
    let unique_targets = targets.iter().copied().collect::<BTreeSet<_>>().len();
    let zero_targets = targets.iter().filter(|&&target| target == 0).count();
    let outside_exe_targets = targets
        .iter()
        .copied()
        .filter(|&target| target != 0 && !map.contains(normalize_kseg(target)))
        .count();
    let constructor = normalize_kseg(constructor_runtime);
    let constructor_targets = targets
        .iter()
        .filter(|&&target| normalize_kseg(target) == constructor)
        .count();
    Ok(SubgameSeqTextJalrInitializedTableSource {
        call_runtime,
        table_runtime,
        table_entries,
        unique_targets,
        zero_targets,
        outside_exe_targets,
        constructor_targets,
        sha256: Sha256::digest(table).into(),
    })
}

pub(super) fn expand_entry_trace_through_jr_tables(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    initial_tables: &[SubgameSeqTextJrTableSource],
    spec: SubgameSeqTextBridgeSpec,
) -> Result<SubgameSeqTextExpandedTrace, SubgameSeqTextBridgeError> {
    let (mut seeds, _) = collect_jr_table_targets(map, text, initial_tables)?;
    let mut iterations = 0usize;
    loop {
        iterations = iterations
            .checked_add(1)
            .ok_or_else(|| error(format!("{} expanded trace 반복 overflow", spec.exe)))?;
        if iterations > instructions.len() {
            return Err(error(format!(
                "{} expanded trace가 고정점에 도달하지 않음",
                spec.exe
            )));
        }
        let seed_list = seeds.iter().copied().collect::<Vec<_>>();
        let expanded = trace(map, text, &seed_list);
        let sources = classify_trace_jr_sources(map, text, instructions, &expanded, spec)?;
        let (new_seeds, outside_targets) =
            collect_jr_table_targets(map, text, &sources.table_sources)?;
        let previous_seed_count = seeds.len();
        seeds.extend(new_seeds);
        if seeds.len() != previous_seed_count {
            continue;
        }
        let jalr_sites = expanded
            .indirect_sites
            .iter()
            .filter(|&&runtime| {
                expanded
                    .instructions
                    .get(&runtime)
                    .is_some_and(|instruction| matches!(instruction.kind, K::Jalr { .. }))
            })
            .count();
        let mut jalr_sources = classify_expanded_jalr_sources(map, text, instructions, &expanded)?;
        let indexed_table_specs = [
            (
                spec.dispatcher_runtime + 0x40,
                spec.dispatcher_table_runtime,
                256,
            ),
            (
                spec.object_control_call_runtime,
                spec.object_control_table_runtime,
                48,
            ),
        ];
        let mut initialized_indexed_sources = Vec::with_capacity(indexed_table_specs.len());
        for (call_runtime, table_runtime, table_entries) in indexed_table_specs {
            let Some(index) = jalr_sources
                .computed_memory_sources
                .iter()
                .position(|source| source.call_runtime == call_runtime)
            else {
                return Err(error(format!(
                    "{} initialized indexed jalr가 computed source에 없음: 0x{call_runtime:08X}",
                    spec.exe
                )));
            };
            jalr_sources.computed_memory_sources.remove(index);
            initialized_indexed_sources.push(inspect_initialized_jalr_table(
                map,
                text,
                call_runtime,
                table_runtime,
                table_entries,
                spec.constructor_runtime,
            )?);
        }
        let forwarded_sources = analyze_forwarded_jalr_sources(
            map,
            text,
            instructions,
            &jalr_sources.forwarded_register_sites,
            spec.constructor_runtime,
        )?;
        return Ok(SubgameSeqTextExpandedTrace {
            iterations,
            table_target_seeds: seeds.len(),
            table_targets_outside_exe: outside_targets.len(),
            reached_instructions: expanded.instructions.len(),
            indirect_sites: expanded.indirect_sites.len(),
            jalr_sites,
            jr_bios_sites: sources.bios_sites.len(),
            jr_table_sites: sources.table_sources.len(),
            jr_unclassified_sites: sources.unclassified_sites,
            jalr_bios_vector_sites: jalr_sources.bios_vector_sites,
            jalr_initialized_direct_sources: initialized_sources(
                jalr_sources.initialized_direct_sources,
            ),
            jalr_initialized_rooted_sources: initialized_sources(
                jalr_sources.initialized_rooted_sources,
            ),
            jalr_initialized_indexed_sources: initialized_indexed_sources,
            jalr_outside_text_sources: analyze_outside_text_sources(
                instructions,
                jalr_sources.outside_text_sources,
            ),
            jalr_computed_memory_sources: jalr_sources.computed_memory_sources,
            jalr_forwarded_register_sites: jalr_sources.forwarded_register_sites,
            jalr_forwarded_register_sources: forwarded_sources,
            jalr_unclassified_sites: jalr_sources.unclassified_sites,
            jalr_fresh_stack_arg5_sites: jalr_sources.fresh_stack_arg5_sites,
            constructor_reached: expanded
                .instructions
                .contains_key(&spec.constructor_runtime),
        });
    }
}

pub(super) fn read_word(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
) -> Result<u32, SubgameSeqTextBridgeError> {
    let offset = map
        .text_offset(runtime)
        .ok_or_else(|| error(format!("{} text bridge word가 EXE 밖임", map.name)))?;
    let bytes = text
        .get(offset..offset + 4)
        .ok_or_else(|| error(format!("{} text bridge word 범위 이탈", map.name)))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
}

pub(super) fn count_raw_pointer(raw: &[u8], target: u32) -> usize {
    let target = target.to_le_bytes();
    raw.windows(4).filter(|window| *window == target).count()
}
