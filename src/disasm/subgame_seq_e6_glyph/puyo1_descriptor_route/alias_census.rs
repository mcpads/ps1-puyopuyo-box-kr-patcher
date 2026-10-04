//! PUYO1 descriptor의 alias, callback과 payload clone 계보 검증.

use std::collections::{BTreeSet, VecDeque};

use crate::coords::{normalize_kseg, ExeMap};
use crate::isa::{FlowKind, Instruction, InstructionKind as K, Reg};

use super::super::super::xref;
use super::super::binary_checks::{require, runtime_slice};
use super::super::{
    error, P1DescriptorAliasCensus, P1DescriptorCallbackLineageCensus,
    P1DescriptorPayloadCloneCensus, SubgameSeqE6GlyphError, P1_ASYNC_DISPATCH_END_RUNTIME,
    P1_ASYNC_DISPATCH_RUNTIME, P1_ASYNC_DISPATCH_TABLE_RUNTIME, P1_ASYNC_DISPATCH_TARGETS,
    P1_DESCRIPTOR_CACHE_CALLBACKS, P1_DESCRIPTOR_CACHE_LINK_STORES,
    P1_DESCRIPTOR_CALLBACK_COPY_LOOPS, P1_DESCRIPTOR_COMPLETION_CALLBACK_FIELD_WRITES,
    P1_DESCRIPTOR_COMPLETION_DESCRIPTOR_WRITES, P1_DESCRIPTOR_FIXED_CACHE_CALLBACKS,
    P1_DESCRIPTOR_FIXED_CALLBACK_CELL_READS, P1_DESCRIPTOR_FIXED_CALLBACK_WRITES,
    P1_DESCRIPTOR_FORWARDING_CALLS, P1_DESCRIPTOR_FORWARDING_UNIQUE_CALLEES,
    P1_DESCRIPTOR_PAYLOAD_COPY_LOOPS, P1_DESCRIPTOR_POINTER_ESCAPE_STORES,
    P1_DESCRIPTOR_SELECTOR_DIRECT_CALLEE_CALLS, P1_DESCRIPTOR_SELECTOR_LOCAL_CONSUMERS,
    P1_DESCRIPTOR_SELECTOR_UNIQUE_CALLEES, P1_DESCRIPTOR_SINGLETON_CALLBACK_WRITES,
    P1_DESCRIPTOR_SINGLETON_SNAPSHOT_ROOT_COMBINES, P1_DESCRIPTOR_SNAPSHOT_FIELD_WRITES,
    P1_DESCRIPTOR_SYNCHRONOUS_FIELD_READS, P1_DESCRIPTOR_SYNCHRONOUS_FIELD_WRITES,
    P1_DESCRIPTOR_TYPE_CACHE_ROOT_COMBINES, P1_DESCRIPTOR_TYPE_SNAPSHOT_LINK_STORES,
    P1_DESCRIPTOR_TYPE_SNAPSHOT_ROOT_COMBINES,
};

fn p1_instruction_at(
    instructions: &[Instruction],
    runtime: u32,
) -> Result<Instruction, SubgameSeqE6GlyphError> {
    instructions
        .binary_search_by_key(&runtime, |instruction| instruction.addr)
        .ok()
        .map(|index| instructions[index])
        .ok_or_else(|| error(format!("PUYO1 instruction 주소 없음: 0x{runtime:08X}")))
}

fn p1_advance_incoming_a0(instruction: Instruction, live: bool, uses: &mut BTreeSet<u32>) -> bool {
    if !live {
        return false;
    }
    if instruction.reads_reg(Reg::A0) {
        uses.insert(instruction.addr);
        return false;
    }
    instruction.dest_reg() != Some(Reg::A0)
}

/// 비동기 state dispatcher의 각 고정 target에서 incoming `$a0`가 소비되는지 본다.
///
/// caller가 descriptor를 `$a0`에 둔 채 이 함수에 들어와도 각 state가 이를 읽기
/// 전에 덮는다는 지원 원본 전용 경계다. target table 밖의 계산 jump나 다른
/// dispatcher로 일반화하지 않는다.
pub(in crate::disasm::subgame_seq_e6_glyph) fn p1_async_dispatch_incoming_a0_uses(
    instructions: &[Instruction],
    targets: &[u32],
    start: u32,
    end: u32,
) -> Result<Vec<u32>, SubgameSeqE6GlyphError> {
    let mut queue = targets.iter().copied().collect::<VecDeque<_>>();
    let mut visited = BTreeSet::new();
    let mut uses = BTreeSet::new();

    while let Some(runtime) = queue.pop_front() {
        if !visited.insert(runtime) {
            continue;
        }
        if runtime < start || runtime >= end {
            return Err(error(format!(
                "PUYO1 async dispatcher target 범위 이탈: 0x{runtime:08X}"
            )));
        }
        let instruction = p1_instruction_at(instructions, runtime)?;
        let live = p1_advance_incoming_a0(instruction, true, &mut uses);
        if !live {
            continue;
        }

        let advance_delay = |uses: &mut BTreeSet<u32>| {
            let delay = p1_instruction_at(instructions, runtime + 4)?;
            if delay.flow_kind().has_delay_slot() {
                return Err(error(format!(
                    "PUYO1 async dispatcher delay slot에 control 명령: 0x{:08X}",
                    delay.addr
                )));
            }
            Ok(p1_advance_incoming_a0(delay, true, uses))
        };

        match instruction.flow_kind() {
            FlowKind::Sequential => queue.push_back(runtime + 4),
            FlowKind::Branch { taken } => {
                if advance_delay(&mut uses)? {
                    queue.push_back(taken);
                    queue.push_back(runtime + 8);
                }
            }
            FlowKind::Jump { target } => {
                if advance_delay(&mut uses)? {
                    if target >= start && target < end {
                        queue.push_back(target);
                    } else {
                        uses.insert(runtime);
                    }
                }
            }
            FlowKind::Call { .. } | FlowKind::JumpRegister { .. } => {
                if advance_delay(&mut uses)? {
                    uses.insert(runtime);
                }
            }
            FlowKind::Return => {
                advance_delay(&mut uses)?;
            }
            FlowKind::Trap => {}
        }
    }
    Ok(uses.into_iter().collect())
}

fn p1_stores_with_base(
    instructions: &[Instruction],
    start: u32,
    end: u32,
    base: Reg,
) -> Vec<(u32, i16)> {
    instructions
        .iter()
        .filter(|instruction| instruction.addr >= start && instruction.addr < end)
        .filter_map(|instruction| {
            let (store_base, offset) = match instruction.kind {
                K::Sb { base, offset, .. }
                | K::Sh { base, offset, .. }
                | K::Sw { base, offset, .. }
                | K::Swl { base, offset, .. }
                | K::Swr { base, offset, .. }
                | K::Swc2 { base, offset, .. } => (base, offset),
                _ => return None,
            };
            (store_base == base).then_some((instruction.addr, offset))
        })
        .collect()
}

/// 지정한 수명 구간에서 해당 register를 base로 쓰는 메모리 접근만 모은다.
///
/// pointer 값 자체를 다른 곳에 저장하는 escape는 이 함수의 대상이 아니며,
/// caller가 별도 store 검산으로 소유한다.
pub(in crate::disasm::subgame_seq_e6_glyph) fn p1_memory_accesses_with_base(
    instructions: &[Instruction],
    start: u32,
    end: u32,
    base: Reg,
) -> (Vec<u32>, Vec<u32>) {
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    for instruction in instructions
        .iter()
        .filter(|instruction| instruction.addr >= start && instruction.addr < end)
    {
        match instruction.kind {
            K::Lb {
                base: memory_base, ..
            }
            | K::Lbu {
                base: memory_base, ..
            }
            | K::Lh {
                base: memory_base, ..
            }
            | K::Lhu {
                base: memory_base, ..
            }
            | K::Lw {
                base: memory_base, ..
            }
            | K::Lwl {
                base: memory_base, ..
            }
            | K::Lwr {
                base: memory_base, ..
            }
            | K::Lwc2 {
                base: memory_base, ..
            } if memory_base == base => reads.push(instruction.addr),
            K::Sb {
                base: memory_base, ..
            }
            | K::Sh {
                base: memory_base, ..
            }
            | K::Sw {
                base: memory_base, ..
            }
            | K::Swl {
                base: memory_base, ..
            }
            | K::Swr {
                base: memory_base, ..
            }
            | K::Swc2 {
                base: memory_base, ..
            } if memory_base == base => writes.push(instruction.addr),
            _ => {}
        }
    }
    (reads, writes)
}

/// 선택된 36바이트 descriptor의 singleton/type별 복제 경로를 검산한다.
///
/// 이 경로는 descriptor stride를 단순 selector 산술 이상으로 고정한다. 두
/// loop 모두 16바이트 두 번과 마지막 4바이트를 복사하며, type별 clone은
/// cache record와 `+0x14` 양방향 link를 다시 만든다.
fn verify_p1_descriptor_payload_clones(
    instructions: &[Instruction],
    map: &ExeMap,
    text: &[u8],
) -> Result<P1DescriptorPayloadCloneCensus, SubgameSeqE6GlyphError> {
    let address_loads = xref::find_address_loads(instructions, xref::DEFAULT_WINDOW);
    let singleton_snapshot_root_combines = address_loads
        .iter()
        .filter(|load| normalize_kseg(load.value) == 0x800B_37C0)
        .map(|load| load.combine_addr)
        .collect::<Vec<_>>();
    let type_snapshot_root_combines = address_loads
        .iter()
        .filter(|load| normalize_kseg(load.value) == 0x800B_3700)
        .map(|load| load.combine_addr)
        .collect::<Vec<_>>();
    if singleton_snapshot_root_combines != P1_DESCRIPTOR_SINGLETON_SNAPSHOT_ROOT_COMBINES
        || type_snapshot_root_combines != P1_DESCRIPTOR_TYPE_SNAPSHOT_ROOT_COMBINES
    {
        return Err(error(format!(
            "PUYO1 descriptor snapshot root 분모 불일치: singleton={singleton_snapshot_root_combines:?}, type={type_snapshot_root_combines:?}"
        )));
    }

    for (runtime, expected) in [
        (
            0x8001_AF24,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x37C0,
            },
        ),
        (
            0x8001_AF28,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::S1,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8001_AF2C,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::S1,
                imm: 0x20,
            },
        ),
        (
            0x8001_AF30,
            K::Lw {
                rt: Reg::A2,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            0x8001_AF3C,
            K::Lw {
                rt: Reg::T1,
                base: Reg::V1,
                offset: 0x0C,
            },
        ),
        (
            0x8001_AF40,
            K::Sw {
                rt: Reg::A2,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8001_AF4C,
            K::Sw {
                rt: Reg::T1,
                base: Reg::V0,
                offset: 0x0C,
            },
        ),
        (
            0x8001_AF50,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x10,
            },
        ),
        (
            0x8001_AF54,
            K::Bne {
                rs: Reg::V1,
                rt: Reg::A0,
                offset: -10,
            },
        ),
        (
            0x8001_AF58,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x10,
            },
        ),
        (
            0x8001_AF5C,
            K::Lw {
                rt: Reg::A2,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            0x8001_AF64,
            K::Sw {
                rt: Reg::A2,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8001_D87C,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A2,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8001_D884,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x37C0,
            },
        ),
        (
            0x8001_D888,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::V0,
                imm: 0x20,
            },
        ),
        (
            0x8001_D88C,
            K::Lw {
                rt: Reg::T1,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8001_D898,
            K::Lw {
                rt: Reg::T4,
                base: Reg::V0,
                offset: 0x0C,
            },
        ),
        (
            0x8001_D89C,
            K::Sw {
                rt: Reg::T1,
                base: Reg::A0,
                offset: 0,
            },
        ),
        (
            0x8001_D8A8,
            K::Sw {
                rt: Reg::T4,
                base: Reg::A0,
                offset: 0x0C,
            },
        ),
        (
            0x8001_D8AC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x10,
            },
        ),
        (
            0x8001_D8B0,
            K::Bne {
                rs: Reg::V0,
                rt: Reg::A1,
                offset: -10,
            },
        ),
        (
            0x8001_D8B4,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x10,
            },
        ),
        (
            0x8001_D8B8,
            K::Lw {
                rt: Reg::T1,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8001_D8C0,
            K::Sw {
                rt: Reg::T1,
                base: Reg::A0,
                offset: 0,
            },
        ),
    ] {
        require(instructions, map, runtime, expected)?;
    }

    for (start, end, source, destination, expected_reads, expected_writes) in [
        (
            0x8001_AF30,
            0x8001_AF68,
            Reg::V1,
            Reg::V0,
            vec![
                0x8001_AF30,
                0x8001_AF34,
                0x8001_AF38,
                0x8001_AF3C,
                0x8001_AF5C,
            ],
            vec![
                0x8001_AF40,
                0x8001_AF44,
                0x8001_AF48,
                0x8001_AF4C,
                0x8001_AF64,
            ],
        ),
        (
            0x8001_D88C,
            0x8001_D8C4,
            Reg::V0,
            Reg::A0,
            vec![
                0x8001_D88C,
                0x8001_D890,
                0x8001_D894,
                0x8001_D898,
                0x8001_D8B8,
            ],
            vec![
                0x8001_D89C,
                0x8001_D8A0,
                0x8001_D8A4,
                0x8001_D8A8,
                0x8001_D8C0,
            ],
        ),
    ] {
        let (reads, _) = p1_memory_accesses_with_base(instructions, start, end, source);
        let (_, writes) = p1_memory_accesses_with_base(instructions, start, end, destination);
        if reads != expected_reads || writes != expected_writes {
            return Err(error(format!(
                "PUYO1 descriptor 36-byte copy loop 접근 불일치: start=0x{start:08X}, reads={reads:?}, writes={writes:?}"
            )));
        }
    }

    for (runtime, expected) in [
        (
            0x8001_B114,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x18,
            },
        ),
        (
            0x8001_B120,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x1A,
            },
        ),
        (
            0x8001_B1A0,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x1C,
            },
        ),
        (
            0x8001_B1AC,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x20,
            },
        ),
        (
            0x8001_D8C4,
            K::Sw {
                rt: Reg::A2,
                base: Reg::V1,
                offset: 4,
            },
        ),
        (
            0x8001_D8C8,
            K::Sw {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 0x14,
            },
        ),
    ] {
        require(instructions, map, runtime, expected)?;
    }

    let snapshot_raw_pointer_words = text
        .chunks_exact(4)
        .filter(|word| {
            matches!(
                normalize_kseg(u32::from_le_bytes(
                    (*word).try_into().expect("4바이트 word"),
                )),
                0x800B_3700 | 0x800B_37C0
            )
        })
        .count();
    if snapshot_raw_pointer_words != 0 {
        return Err(error(format!(
            "PUYO1 descriptor snapshot raw pointer가 생김: {snapshot_raw_pointer_words}"
        )));
    }

    Ok(P1DescriptorPayloadCloneCensus {
        singleton_snapshot_root_combines,
        type_snapshot_root_combines,
        payload_copy_loops: P1_DESCRIPTOR_PAYLOAD_COPY_LOOPS.to_vec(),
        snapshot_field_writes: P1_DESCRIPTOR_SNAPSHOT_FIELD_WRITES.to_vec(),
        type_snapshot_link_stores: P1_DESCRIPTOR_TYPE_SNAPSHOT_LINK_STORES.to_vec(),
        snapshot_raw_pointer_words,
    })
}

/// singleton cache record에서 type별 record와 pool record로 이어지는 callback을 검산한다.
///
/// 계산된 destination으로 40바이트 record를 복사하는 두 loop까지 포함하지만,
/// 이 세 cache 집합 밖의 임의 pointer alias로 일반화하지 않는다.
fn verify_p1_descriptor_callback_lineage(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<P1DescriptorCallbackLineageCensus, SubgameSeqE6GlyphError> {
    for (runtime, expected) in [
        (
            0x8001_AF6C,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x3790,
            },
        ),
        (
            0x8001_AF80,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -0x30D4,
            },
        ),
        (
            0x8001_AF84,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 8,
            },
        ),
        (
            0x8001_AF8C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -0x2F14,
            },
        ),
        (
            0x8001_AF90,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x0C,
            },
        ),
        (
            0x8001_B064,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -0x4D80,
            },
        ),
        (
            0x8001_B068,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 8,
            },
        ),
        (
            0x8001_B098,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 8,
            },
        ),
        (
            0x8001_B09C,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::V1,
                imm: 0x3790,
            },
        ),
        (
            0x8001_B0A0,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 8,
            },
        ),
        (
            0x8001_B100,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -0x2EB0,
            },
        ),
        (
            0x8001_B104,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x0C,
            },
        ),
        (
            0x8001_B11C,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A0,
                imm: 0x3790,
            },
        ),
        (
            0x8001_B138,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x0C,
            },
        ),
        (
            0x8001_B140,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 0x0C,
            },
        ),
        (
            0x8001_B18C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -0x2EEC,
            },
        ),
        (
            0x8001_B190,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x0C,
            },
        ),
        (
            0x8001_B1B8,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x0C,
            },
        ),
        (
            0x8001_B1C4,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x3790,
            },
        ),
        (
            0x8001_B1C8,
            K::Sw {
                rt: Reg::A0,
                base: Reg::V1,
                offset: 0x0C,
            },
        ),
    ] {
        require(instructions, map, runtime, expected)?;
    }

    // singleton의 +8/+0x0C는 literal 초기값 뒤 세 bounded overwrite만 받는다.
    // 각 source field는 그 사이 call이나 같은 field write 없이 바로 전달된다.
    for (start, end, base, expected) in [
        (
            0x8001_B064,
            0x8001_B0A4,
            Reg::S0,
            vec![(0x8001_B068, 8), (0x8001_B07C, 0x18), (0x8001_B090, 0x18)],
        ),
        (0x8001_B100, 0x8001_B144, Reg::S0, vec![(0x8001_B104, 0x0C)]),
        (0x8001_B188, 0x8001_B1CC, Reg::S0, vec![(0x8001_B190, 0x0C)]),
    ] {
        let stores = p1_stores_with_base(instructions, start, end, base);
        if stores != expected {
            return Err(error(format!(
                "PUYO1 singleton callback source field write 불일치: start=0x{start:08X}, stores={stores:?}"
            )));
        }
    }

    for (runtime, expected) in [
        (
            0x8001_D7FC,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::V0,
                imm: 0x3790,
            },
        ),
        (
            0x8001_D800,
            K::Addiu {
                rt: Reg::T0,
                rs: Reg::A1,
                imm: 0x20,
            },
        ),
        (
            0x8001_D834,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x3660,
            },
        ),
        (
            0x8001_D840,
            K::Lw {
                rt: Reg::T1,
                base: Reg::A1,
                offset: 0,
            },
        ),
        (
            0x8001_D848,
            K::Lw {
                rt: Reg::T3,
                base: Reg::A1,
                offset: 8,
            },
        ),
        (
            0x8001_D84C,
            K::Lw {
                rt: Reg::T4,
                base: Reg::A1,
                offset: 0x0C,
            },
        ),
        (
            0x8001_D858,
            K::Sw {
                rt: Reg::T3,
                base: Reg::V0,
                offset: 8,
            },
        ),
        (
            0x8001_D85C,
            K::Sw {
                rt: Reg::T4,
                base: Reg::V0,
                offset: 0x0C,
            },
        ),
        (
            0x8001_D864,
            K::Bne {
                rs: Reg::A1,
                rt: Reg::T0,
                offset: -10,
            },
        ),
        (
            0x8001_D868,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x10,
            },
        ),
        (
            0x8001_D8C4,
            K::Sw {
                rt: Reg::A2,
                base: Reg::V1,
                offset: 4,
            },
        ),
        (
            0x8001_D8C8,
            K::Sw {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 0x14,
            },
        ),
        (
            0x8001_D904,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x3660,
            },
        ),
        (
            0x8001_D948,
            K::Jal {
                target: 0x8001_D2CC,
            },
        ),
        (
            0x8001_D95C,
            K::Lw {
                rt: Reg::A2,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            0x8001_D964,
            K::Lw {
                rt: Reg::T0,
                base: Reg::V1,
                offset: 8,
            },
        ),
        (
            0x8001_D968,
            K::Lw {
                rt: Reg::T1,
                base: Reg::V1,
                offset: 0x0C,
            },
        ),
        (
            0x8001_D974,
            K::Sw {
                rt: Reg::T0,
                base: Reg::V0,
                offset: 8,
            },
        ),
        (
            0x8001_D978,
            K::Sw {
                rt: Reg::T1,
                base: Reg::V0,
                offset: 0x0C,
            },
        ),
        (
            0x8001_D980,
            K::Bne {
                rs: Reg::V1,
                rt: Reg::A0,
                offset: -10,
            },
        ),
        (
            0x8001_D984,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x10,
            },
        ),
        (
            0x8001_DA5C,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x3660,
            },
        ),
        (
            0x8001_DA64,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: 4,
            },
        ),
    ] {
        require(instructions, map, runtime, expected)?;
    }

    let address_loads = xref::find_address_loads(instructions, xref::DEFAULT_WINDOW);
    let type_cache_root_combines = address_loads
        .iter()
        .filter(|load| load.value == 0x800B_3660)
        .map(|load| load.combine_addr)
        .collect::<Vec<_>>();
    if type_cache_root_combines != P1_DESCRIPTOR_TYPE_CACHE_ROOT_COMBINES {
        return Err(error(format!(
            "PUYO1 descriptor type-cache root 분모 불일치: {type_cache_root_combines:?}"
        )));
    }

    // completion callback은 descriptor +0x10을 채우고 link +0x14만 해제한다.
    // cache +8은 streaming callback 또는 이미 제한된 +0x0C 값으로만 바뀐다.
    for (runtime, expected) in [
        (
            0x8001_B298,
            K::Lw {
                rt: Reg::S1,
                base: Reg::S0,
                offset: 4,
            },
        ),
        (
            0x8001_B2A8,
            K::Lw {
                rt: Reg::V1,
                base: Reg::S1,
                offset: 0x0C,
            },
        ),
        (
            0x8001_B334,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x10,
            },
        ),
        (
            0x8001_B34C,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: -0x2FB8,
            },
        ),
        (
            0x8001_B350,
            K::Sw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 8,
            },
        ),
        (
            0x8001_B384,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 4,
            },
        ),
        (
            0x8001_B38C,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: 0x14,
            },
        ),
        (
            0x8001_B390,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x0C,
            },
        ),
        (
            0x8001_B394,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8001_B398,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        (
            0x8001_B39C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 8,
            },
        ),
    ] {
        require(instructions, map, runtime, expected)?;
    }

    let mut cache_callbacks = P1_DESCRIPTOR_FIXED_CACHE_CALLBACKS.to_vec();
    cache_callbacks.push(0x8001_B280);
    cache_callbacks.sort_unstable();
    if cache_callbacks != P1_DESCRIPTOR_CACHE_CALLBACKS {
        return Err(error(format!(
            "PUYO1 descriptor cache callback 집합 불일치: {cache_callbacks:?}"
        )));
    }

    Ok(P1DescriptorCallbackLineageCensus {
        singleton_callback_writes: P1_DESCRIPTOR_SINGLETON_CALLBACK_WRITES.to_vec(),
        cache_callbacks,
        callback_copy_loops: P1_DESCRIPTOR_CALLBACK_COPY_LOOPS.to_vec(),
        completion_callback_field_writes: P1_DESCRIPTOR_COMPLETION_CALLBACK_FIELD_WRITES.to_vec(),
        completion_descriptor_writes: P1_DESCRIPTOR_COMPLETION_DESCRIPTOR_WRITES.to_vec(),
        type_cache_root_combines,
    })
}

pub(super) fn verify_p1_descriptor_alias_census(
    instructions: &[Instruction],
    map: &ExeMap,
    text: &[u8],
) -> Result<P1DescriptorAliasCensus, SubgameSeqE6GlyphError> {
    let callback_lineage = verify_p1_descriptor_callback_lineage(instructions, map)?;
    let payload_clones = verify_p1_descriptor_payload_clones(instructions, map, text)?;
    let call_specs = [
        (
            0x8001_AD60,
            0x8001_D2CC,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S2,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8001_AF94,
            0x8001_D2CC,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S1,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8001_CD8C,
            0x8001_D2CC,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S2,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8001_D5F0,
            0x8001_CC50,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            0x8001_D6C0,
            0x8001_C968,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            0x8001_D724,
            0x8001_CD28,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            0x8005_8A34,
            0x8001_C77C,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
    ];
    let mut direct_callee_calls = Vec::new();
    let mut unique_callees = BTreeSet::new();
    for (runtime, target, delay) in call_specs {
        require(instructions, map, runtime, K::Jal { target })?;
        require(instructions, map, runtime + 4, delay)?;
        direct_callee_calls.push(runtime);
        unique_callees.insert(target);
    }
    let unique_callees = unique_callees.into_iter().collect::<Vec<_>>();
    if direct_callee_calls != P1_DESCRIPTOR_SELECTOR_DIRECT_CALLEE_CALLS
        || unique_callees != P1_DESCRIPTOR_SELECTOR_UNIQUE_CALLEES
    {
        return Err(error(format!(
            "PUYO1 descriptor selector direct callee 분모 불일치: calls={direct_callee_calls:?}, callees={unique_callees:?}"
        )));
    }

    for (runtime, target, delay) in [
        (
            0x8001_C7A0,
            0x8001_C824,
            Some(K::Addu {
                rd: Reg::A0,
                rs: Reg::S2,
                rt: Reg::ZERO,
            }),
        ),
        (
            0x8001_C7F8,
            0x8001_CC50,
            Some(K::Addu {
                rd: Reg::A0,
                rs: Reg::S2,
                rt: Reg::ZERO,
            }),
        ),
        (0x8001_C834, 0x8001_D25C, None),
        (
            0x8001_C850,
            0x8001_D3BC,
            Some(K::Addu {
                rd: Reg::A0,
                rs: Reg::S1,
                rt: Reg::ZERO,
            }),
        ),
    ] {
        require(instructions, map, runtime, K::Jal { target })?;
        if let Some(expected) = delay {
            require(instructions, map, runtime + 4, expected)?;
        }
    }
    require(
        instructions,
        map,
        0x8001_C82C,
        K::Addu {
            rd: Reg::S1,
            rs: Reg::A0,
            rt: Reg::ZERO,
        },
    )?;
    let forwarding_calls = P1_DESCRIPTOR_FORWARDING_CALLS.to_vec();
    let forwarding_unique_callees = P1_DESCRIPTOR_FORWARDING_UNIQUE_CALLEES.to_vec();

    let mut synchronous_field_reads = Vec::new();
    let mut synchronous_field_writes = Vec::new();
    for (start, end, base) in [
        (0x8001_AD5C, 0x8001_AE78, Reg::S2),
        (0x8001_AF0C, 0x8001_B254, Reg::S1),
        (0x8001_C784, 0x8001_C808, Reg::S2),
        (0x8001_C82C, 0x8001_C86C, Reg::S1),
        (0x8001_C970, 0x8001_C9A8, Reg::S0),
        (0x8001_CA14, 0x8001_CA4C, Reg::S0),
        (0x8001_CC60, 0x8001_CC88, Reg::S0),
        (0x8001_CCC8, 0x8001_CCE8, Reg::S0),
        (0x8001_CD28, 0x8001_CD44, Reg::A0),
        (0x8001_CD88, 0x8001_CE6C, Reg::S2),
        (0x8001_D25C, 0x8001_D29C, Reg::A0),
        (0x8001_D2CC, 0x8001_D30C, Reg::A0),
        (0x8001_D3BC, 0x8001_D43C, Reg::A0),
        (0x8001_DAF4, 0x8001_DAFC, Reg::V0),
        (0x8002_4554, 0x8002_455C, Reg::V0),
    ] {
        let (reads, writes) = p1_memory_accesses_with_base(instructions, start, end, base);
        synchronous_field_reads.extend(reads);
        synchronous_field_writes.extend(writes);
    }
    synchronous_field_reads.sort_unstable();
    synchronous_field_writes.sort_unstable();
    if synchronous_field_reads != P1_DESCRIPTOR_SYNCHRONOUS_FIELD_READS
        || synchronous_field_writes != P1_DESCRIPTOR_SYNCHRONOUS_FIELD_WRITES
    {
        return Err(error(format!(
            "PUYO1 descriptor synchronous field 접근 분모 불일치: reads={synchronous_field_reads:?}, writes={synchronous_field_writes:?}"
        )));
    }

    for (runtime, expected) in [
        (
            0x8001_CA20,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x14,
            },
        ),
        (
            0x8001_CCCC,
            K::Lw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0x14,
            },
        ),
        (
            0x8001_DAF8,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8002_4558,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0x10,
            },
        ),
    ] {
        require(instructions, map, runtime, expected)?;
    }
    let local_consumers = P1_DESCRIPTOR_SELECTOR_LOCAL_CONSUMERS.to_vec();

    for (runtime, expected) in [
        (
            0x8001_AF74,
            K::Sw {
                rt: Reg::S1,
                base: Reg::V1,
                offset: 4,
            },
        ),
        (
            0x8001_D27C,
            K::Sw {
                rt: Reg::A0,
                base: Reg::V1,
                offset: 4,
            },
        ),
        (
            0x8001_D2EC,
            K::Sw {
                rt: Reg::A0,
                base: Reg::V1,
                offset: 4,
            },
        ),
        (
            0x8001_D280,
            K::Sw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0x14,
            },
        ),
        (
            0x8001_D2F0,
            K::Sw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0x14,
            },
        ),
    ] {
        require(instructions, map, runtime, expected)?;
    }

    let address_loads = xref::find_address_loads(instructions, xref::DEFAULT_WINDOW);
    for (callback, expected_combines) in [
        (0x8001_CF2C, vec![0x8001_AF80, 0x8001_D278, 0x8001_D2E8]),
        (0x8001_D048, vec![0x8001_B34C]),
        (0x8001_D0EC, vec![0x8001_AF8C, 0x8001_D28C, 0x8001_D2FC]),
        (0x8001_D114, vec![0x8001_B12C, 0x8001_B18C]),
        (0x8001_D150, vec![0x8001_B100, 0x8001_D11C]),
    ] {
        let combines = address_loads
            .iter()
            .filter(|load| load.value == callback)
            .map(|load| load.combine_addr)
            .collect::<Vec<_>>();
        if combines != expected_combines {
            return Err(error(format!(
                "PUYO1 descriptor fixed callback xref 불일치: callback=0x{callback:08X}, combines={combines:?}"
            )));
        }
    }

    for (runtime, expected) in [
        (
            0x8001_D01C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 4,
            },
        ),
        (
            0x8001_D068,
            K::Lw {
                rt: Reg::S1,
                base: Reg::S0,
                offset: 4,
            },
        ),
        (
            0x8001_D09C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 4,
            },
        ),
        (
            0x8001_D100,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 4,
            },
        ),
        (
            0x8001_D130,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 4,
            },
        ),
        (
            0x8001_D160,
            K::Lw {
                rt: Reg::A1,
                base: Reg::S0,
                offset: 4,
            },
        ),
        (
            0x8001_D23C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 4,
            },
        ),
    ] {
        require(instructions, map, runtime, expected)?;
    }

    let alias_segments = [
        (0x8001_D01C, 0x8001_D028, Reg::V0, vec![(0x8001_D024, 0x14)]),
        (0x8001_D068, 0x8001_D0EC, Reg::S1, vec![(0x8001_D084, 0x10)]),
        (0x8001_D09C, 0x8001_D0A8, Reg::V0, vec![(0x8001_D0A4, 0x14)]),
        (0x8001_D100, 0x8001_D10C, Reg::V0, vec![(0x8001_D108, 0x14)]),
        (0x8001_D130, 0x8001_D13C, Reg::V0, vec![(0x8001_D138, 0x14)]),
        (0x8001_D160, 0x8001_D214, Reg::A1, Vec::new()),
        (0x8001_D23C, 0x8001_D248, Reg::V0, vec![(0x8001_D244, 0x14)]),
    ];
    let mut fixed_callback_writes = Vec::new();
    for (start, end, base, expected) in alias_segments {
        let stores = p1_stores_with_base(instructions, start, end, base);
        if stores != expected {
            return Err(error(format!(
                "PUYO1 descriptor callback alias store 불일치: start=0x{start:08X}, stores={stores:?}"
            )));
        }
        fixed_callback_writes.extend(stores.into_iter().map(|(runtime, _)| runtime));
    }
    fixed_callback_writes.sort_unstable();
    if fixed_callback_writes != P1_DESCRIPTOR_FIXED_CALLBACK_WRITES {
        return Err(error(format!(
            "PUYO1 descriptor fixed callback write 분모 불일치: {fixed_callback_writes:?}"
        )));
    }

    for (runtime, expected) in [
        (
            0x8001_D180,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 0x1C,
            },
        ),
        (
            0x8001_D194,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 0x1C,
            },
        ),
        (
            0x8001_D1A8,
            K::Lh {
                rt: Reg::V1,
                base: Reg::A1,
                offset: 0x20,
            },
        ),
    ] {
        require(instructions, map, runtime, expected)?;
    }

    require(
        instructions,
        map,
        0x8001_B638,
        K::Lw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0,
        },
    )?;
    require(instructions, map, 0x8001_B640, K::Jr { rs: Reg::V0 })?;
    let table = runtime_slice(
        map,
        text,
        P1_ASYNC_DISPATCH_TABLE_RUNTIME,
        P1_ASYNC_DISPATCH_TARGETS.len() * 4,
    )?;
    let async_targets = table
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().expect("4바이트 word")))
        .collect::<Vec<_>>();
    if async_targets != P1_ASYNC_DISPATCH_TARGETS {
        return Err(error(format!(
            "PUYO1 async dispatcher target table 불일치: {async_targets:?}"
        )));
    }
    let async_incoming_uses = p1_async_dispatch_incoming_a0_uses(
        instructions,
        &async_targets,
        P1_ASYNC_DISPATCH_RUNTIME,
        P1_ASYNC_DISPATCH_END_RUNTIME,
    )?;
    if !async_incoming_uses.is_empty() {
        return Err(error(format!(
            "PUYO1 async dispatcher가 incoming descriptor를 소비함: {async_incoming_uses:?}"
        )));
    }

    Ok(P1DescriptorAliasCensus {
        direct_callee_calls,
        local_consumers,
        unique_callees,
        forwarding_calls,
        forwarding_unique_callees,
        synchronous_field_reads,
        synchronous_field_writes,
        pointer_escape_stores: P1_DESCRIPTOR_POINTER_ESCAPE_STORES.to_vec(),
        cache_link_stores: P1_DESCRIPTOR_CACHE_LINK_STORES.to_vec(),
        fixed_cache_callbacks: P1_DESCRIPTOR_FIXED_CACHE_CALLBACKS.to_vec(),
        fixed_callback_writes,
        fixed_callback_cell_reads: P1_DESCRIPTOR_FIXED_CALLBACK_CELL_READS.to_vec(),
        async_dispatch_targets: async_targets.len(),
        async_dispatch_incoming_uses: async_incoming_uses.len(),
        singleton_callback_writes: callback_lineage.singleton_callback_writes,
        cache_callbacks: callback_lineage.cache_callbacks,
        callback_copy_loops: callback_lineage.callback_copy_loops,
        completion_callback_field_writes: callback_lineage.completion_callback_field_writes,
        completion_descriptor_writes: callback_lineage.completion_descriptor_writes,
        type_cache_root_combines: callback_lineage.type_cache_root_combines,
        singleton_snapshot_root_combines: payload_clones.singleton_snapshot_root_combines,
        type_snapshot_root_combines: payload_clones.type_snapshot_root_combines,
        payload_copy_loops: payload_clones.payload_copy_loops,
        snapshot_field_writes: payload_clones.snapshot_field_writes,
        type_snapshot_link_stores: payload_clones.type_snapshot_link_stores,
        snapshot_raw_pointer_words: payload_clones.snapshot_raw_pointer_words,
    })
}
