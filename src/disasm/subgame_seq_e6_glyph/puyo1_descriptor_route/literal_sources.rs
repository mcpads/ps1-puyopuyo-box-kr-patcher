//! PUYO1 literal cell/resource writer와 descriptor root census.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use crate::coords::{normalize_kseg, ExeMap, PSX_EXE_HEADER_LEN};
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::super::super::{
    subgame_seq_descriptor_ingress::{
        descriptor_resource_x36_select, descriptor_root_literal_combines,
    },
    xref,
};
use super::super::binary_checks::{require, runtime_slice};
use super::super::{
    error, hex, DescriptorCellCountWriter, P1DescriptorRootCensus, SubgameSeqE6GlyphError,
    SubgameSeqE6GlyphSpec, DESCRIPTOR_BYTES, P1_CACHE_RECORD_HALFWORD_20_STORE,
    P1_DESCRIPTOR_ROOT_ALTERNATE_BRANCH_COMBINES, P1_DESCRIPTOR_ROOT_LITERAL,
    P1_DESCRIPTOR_ROOT_LITERAL_COMBINES, P1_DESCRIPTOR_ROOT_REGISTRATION_COMBINES,
    P1_DESCRIPTOR_ROOT_REGISTRATION_STORES, P1_DESCRIPTOR_ROOT_RESOURCE_X36_SELECTORS,
    P1_DESCRIPTOR_TABLE_ADDRESS_LOADS, P1_DESCRIPTOR_TABLE_END_RUNTIME,
    P1_DESCRIPTOR_TABLE_RAW_POINTER_LOCATIONS, P1_DESCRIPTOR_TABLE_RECORDS,
    P1_DESCRIPTOR_TABLE_SHA256, P1_GPU_PRIMITIVE_HALFWORD_20_STORES,
};

pub(in crate::disasm::subgame_seq_e6_glyph) fn paired_descriptor_cell_count_writers(
    instructions: &[Instruction],
) -> Vec<DescriptorCellCountWriter> {
    let mut writers = Vec::new();
    for (index, instruction) in instructions.iter().enumerate() {
        let K::Sw {
            rt: cell_source,
            base,
            offset: 0x1C,
        } = instruction.kind
        else {
            continue;
        };
        if base == Reg::SP {
            continue;
        }
        if let Some((count_store_runtime, count_source)) = instructions
            .iter()
            .skip(index + 1)
            .take(8)
            .find_map(|probe| match probe.kind {
                K::Sh {
                    rt,
                    base: count_base,
                    offset: 0x20,
                } if count_base == base => Some((probe.addr, rt)),
                _ => None,
            })
        {
            writers.push(DescriptorCellCountWriter {
                cell_store_runtime: instruction.addr,
                count_store_runtime,
                base,
                cell_source,
                count_source,
            });
        }
    }
    writers
}

pub(in crate::disasm::subgame_seq_e6_glyph) fn literal_non_stack_halfword_20_stores(
    instructions: &[Instruction],
) -> Vec<u32> {
    instructions
        .iter()
        .filter_map(|instruction| match instruction.kind {
            K::Sh {
                base, offset: 0x20, ..
            } if base != Reg::SP => Some(instruction.addr),
            _ => None,
        })
        .collect()
}

pub(in crate::disasm::subgame_seq_e6_glyph) fn has_nearby_halfword_store(
    instructions: &[Instruction],
    center: usize,
    base: Reg,
    offset: i16,
) -> bool {
    let start = center.saturating_sub(64);
    let end = (center + 65).min(instructions.len());
    instructions[start..end].iter().any(|instruction| {
        matches!(
            instruction.kind,
            K::Sh {
                base: store_base,
                offset: store_offset,
                ..
            } if store_base == base && store_offset == offset
        )
    })
}

pub(super) fn verify_gpu_primitive_halfword_20_stores(
    instructions: &[Instruction],
) -> Result<Vec<u32>, SubgameSeqE6GlyphError> {
    let mut found = Vec::new();
    for runtime in P1_GPU_PRIMITIVE_HALFWORD_20_STORES {
        let index = instructions
            .binary_search_by_key(&runtime, |instruction| instruction.addr)
            .map_err(|_| {
                error(format!(
                    "PUYO1 GPU primitive +0x20 store 누락: 0x{runtime:08X}"
                ))
            })?;
        let K::Sh {
            base, offset: 0x20, ..
        } = instructions[index].kind
        else {
            return Err(error(format!(
                "PUYO1 GPU primitive +0x20 store 형식 불일치: 0x{runtime:08X}"
            )));
        };
        for sibling_offset in [0x08, 0x0A, 0x10, 0x12, 0x18, 0x1A, 0x22] {
            if !has_nearby_halfword_store(instructions, index, base, sibling_offset) {
                return Err(error(format!(
                    "PUYO1 GPU primitive 좌표 sibling 누락: 0x{runtime:08X}:+0x{sibling_offset:02X}"
                )));
            }
        }
        found.push(runtime);
    }
    Ok(found)
}

pub(super) fn verify_p1_cache_record_halfword_20_store(
    instructions: &[Instruction],
    map: &ExeMap,
    text: &[u8],
) -> Result<Vec<u32>, SubgameSeqE6GlyphError> {
    for (runtime, expected) in [
        (
            0x8007_55F4,
            K::Jal {
                target: 0x8007_095C,
            },
        ),
        (
            0x8007_55F8,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x4F60,
            },
        ),
        (
            0x8007_560C,
            K::Lui {
                rt: Reg::S5,
                imm: 0x800E,
            },
        ),
        (
            0x8007_5610,
            K::Addiu {
                rt: Reg::S5,
                rs: Reg::S5,
                imm: -0x6DE8,
            },
        ),
        (
            P1_CACHE_RECORD_HALFWORD_20_STORE,
            K::Sh {
                rt: Reg::V0,
                base: Reg::S5,
                offset: 0x20,
            },
        ),
        (
            P1_CACHE_RECORD_HALFWORD_20_STORE + 8,
            K::Sb {
                rt: Reg::V1,
                base: Reg::S5,
                offset: 0x22,
            },
        ),
        (
            0x8007_5728,
            K::Addiu {
                rt: Reg::S3,
                rs: Reg::S3,
                imm: 0x18,
            },
        ),
        (
            0x8007_572C,
            K::Addiu {
                rt: Reg::S1,
                rs: Reg::S1,
                imm: 0x18,
            },
        ),
        (
            0x8007_5744,
            K::Addiu {
                rt: Reg::S4,
                rs: Reg::S4,
                imm: 0x18,
            },
        ),
    ] {
        require(instructions, map, runtime, expected)?;
    }
    if runtime_slice(map, text, 0x8001_4F60, 28)? != b"CD_cachefile: searching...\n\0" {
        return Err(error("PUYO1 cache record 함수 식별 문자열 불일치"));
    }
    Ok(vec![P1_CACHE_RECORD_HALFWORD_20_STORE])
}

pub(in crate::disasm::subgame_seq_e6_glyph) fn literal_resource_006d_loads(
    instructions: &[Instruction],
) -> Vec<u32> {
    instructions
        .iter()
        .filter_map(|instruction| match instruction.kind {
            K::Addiu {
                rs: Reg::ZERO,
                imm: 0x006D,
                ..
            }
            | K::Ori {
                rs: Reg::ZERO,
                imm: 0x006D,
                ..
            } => Some(instruction.addr),
            _ => None,
        })
        .collect()
}

pub(super) fn verify_p1_descriptor_root_census(
    instructions: &[Instruction],
    map: &ExeMap,
    text: &[u8],
    spec: SubgameSeqE6GlyphSpec,
) -> Result<P1DescriptorRootCensus, SubgameSeqE6GlyphError> {
    let literal_combines =
        descriptor_root_literal_combines(instructions, P1_DESCRIPTOR_ROOT_LITERAL, 8);
    if literal_combines != P1_DESCRIPTOR_ROOT_LITERAL_COMBINES {
        return Err(error(format!(
            "PUYO1 descriptor root literal combine 분모 불일치: {literal_combines:?}"
        )));
    }
    let resource_x36_selectors = literal_combines
        .iter()
        .copied()
        .filter(|runtime| {
            descriptor_resource_x36_select(instructions, *runtime, P1_DESCRIPTOR_ROOT_LITERAL, 25)
                .is_some()
        })
        .collect::<Vec<_>>();
    if resource_x36_selectors != P1_DESCRIPTOR_ROOT_RESOURCE_X36_SELECTORS {
        return Err(error(format!(
            "PUYO1 descriptor root resource×36 selector 분모 불일치: {resource_x36_selectors:?}"
        )));
    }

    for runtime in P1_DESCRIPTOR_ROOT_ALTERNATE_BRANCH_COMBINES {
        require(
            instructions,
            map,
            runtime - 4,
            K::J {
                target: runtime + 0x10,
            },
        )?;
    }
    for (runtime, checks) in [
        (
            0x8001_B5C0,
            [
                (
                    0x8001_B5C4,
                    K::Sll {
                        rd: Reg::V0,
                        rt: Reg::A3,
                        sa: 2,
                    },
                ),
                (
                    0x8001_B5C8,
                    K::Addu {
                        rd: Reg::V0,
                        rs: Reg::V0,
                        rt: Reg::V1,
                    },
                ),
                (
                    0x8001_B5CC,
                    K::Sw {
                        rt: Reg::FP,
                        base: Reg::V0,
                        offset: 0,
                    },
                ),
            ],
        ),
        (
            0x8002_434C,
            [
                (
                    0x8002_4358,
                    K::Sll {
                        rd: Reg::V0,
                        rt: Reg::V0,
                        sa: 2,
                    },
                ),
                (
                    0x8002_435C,
                    K::Addu {
                        rd: Reg::V0,
                        rs: Reg::V0,
                        rt: Reg::V1,
                    },
                ),
                (
                    0x8002_4360,
                    K::Sw {
                        rt: Reg::A0,
                        base: Reg::V0,
                        offset: 0,
                    },
                ),
            ],
        ),
    ] {
        if !P1_DESCRIPTOR_ROOT_REGISTRATION_COMBINES.contains(&runtime) {
            return Err(error("PUYO1 descriptor root registration spec 불일치"));
        }
        for (address, expected) in checks {
            require(instructions, map, address, expected)?;
        }
    }

    let classified = resource_x36_selectors
        .iter()
        .chain(&P1_DESCRIPTOR_ROOT_ALTERNATE_BRANCH_COMBINES)
        .chain(&P1_DESCRIPTOR_ROOT_REGISTRATION_COMBINES)
        .copied()
        .collect::<BTreeSet<_>>();
    let literal = literal_combines.iter().copied().collect::<BTreeSet<_>>();
    if classified != literal {
        return Err(error(format!(
            "PUYO1 descriptor root literal combine 분류 불완전: literal={literal:?}, classified={classified:?}"
        )));
    }

    let descriptor_table_bytes = P1_DESCRIPTOR_TABLE_RECORDS
        .checked_mul(usize::try_from(DESCRIPTOR_BYTES).expect("descriptor stride는 usize 범위"))
        .ok_or_else(|| error("PUYO1 descriptor table 크기 overflow"))?;
    let descriptor_table_end = spec
        .descriptor_base_runtime
        .checked_add(
            u32::try_from(descriptor_table_bytes)
                .map_err(|_| error("PUYO1 descriptor table 크기가 u32를 넘음"))?,
        )
        .ok_or_else(|| error("PUYO1 descriptor table 끝주소 overflow"))?;
    if descriptor_table_end != P1_DESCRIPTOR_TABLE_END_RUNTIME {
        return Err(error(format!(
            "PUYO1 descriptor table 끝주소 불일치: 0x{descriptor_table_end:08X}"
        )));
    }
    let descriptor_table = runtime_slice(
        map,
        text,
        spec.descriptor_base_runtime,
        descriptor_table_bytes,
    )?;
    let descriptor_table_sha256 = hex(&Sha256::digest(descriptor_table));
    if descriptor_table_sha256 != P1_DESCRIPTOR_TABLE_SHA256
        || runtime_slice(map, text, descriptor_table_end, 12)? != b"PP1_GRP.VDD\0"
    {
        return Err(error(format!(
            "PUYO1 descriptor table 분모 또는 경계 불일치: records={}, end=0x{descriptor_table_end:08X}, sha256={descriptor_table_sha256}",
            P1_DESCRIPTOR_TABLE_RECORDS
        )));
    }

    let address_loads = xref::find_address_loads(instructions, xref::DEFAULT_WINDOW);
    let descriptor_table_loads = address_loads
        .iter()
        .filter(|load| {
            let value = normalize_kseg(load.value);
            (spec.descriptor_base_runtime..descriptor_table_end).contains(&value)
        })
        .collect::<Vec<_>>();
    let descriptor_table_address_loads = descriptor_table_loads
        .iter()
        .map(|load| load.combine_addr)
        .collect::<Vec<_>>();
    let descriptor_table_preadjusted_address_loads = descriptor_table_loads
        .iter()
        .filter(|load| normalize_kseg(load.value) != spec.descriptor_base_runtime)
        .count();
    if descriptor_table_address_loads != P1_DESCRIPTOR_TABLE_ADDRESS_LOADS
        || descriptor_table_preadjusted_address_loads != 0
    {
        return Err(error(format!(
            "PUYO1 descriptor table literal 주소 분모 불일치: loads={descriptor_table_address_loads:?}, preadjusted={descriptor_table_preadjusted_address_loads}"
        )));
    }

    let mut descriptor_table_raw_pointer_locations = Vec::new();
    let mut descriptor_table_preadjusted_raw_pointers = 0usize;
    for (index, word) in text.chunks_exact(4).enumerate() {
        let value = normalize_kseg(u32::from_le_bytes(word.try_into().expect("4바이트 word")));
        if !(spec.descriptor_base_runtime..descriptor_table_end).contains(&value) {
            continue;
        }
        let text_offset = u64::try_from(index)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| error("PUYO1 raw pointer text offset overflow"))?;
        let file_offset = PSX_EXE_HEADER_LEN
            .checked_add(text_offset)
            .ok_or_else(|| error("PUYO1 raw pointer file offset overflow"))?;
        let location = map
            .runtime_addr(file_offset)
            .ok_or_else(|| error("PUYO1 raw pointer 위치가 EXE 밖임"))?;
        descriptor_table_raw_pointer_locations.push(location);
        if value != spec.descriptor_base_runtime {
            descriptor_table_preadjusted_raw_pointers += 1;
        }
    }
    if descriptor_table_raw_pointer_locations != P1_DESCRIPTOR_TABLE_RAW_POINTER_LOCATIONS
        || descriptor_table_preadjusted_raw_pointers != 0
    {
        return Err(error(format!(
            "PUYO1 descriptor table raw pointer 분모 불일치: locations={descriptor_table_raw_pointer_locations:?}, preadjusted={descriptor_table_preadjusted_raw_pointers}"
        )));
    }

    let chr_descriptor_runtime = spec
        .descriptor_base_runtime
        .checked_add(u32::from(spec.glyph_resource) * DESCRIPTOR_BYTES)
        .ok_or_else(|| error("PUYO1 CHR descriptor 주소 overflow"))?;
    let chr_descriptor_alias = chr_descriptor_runtime | 0x2000_0000;
    let chr_descriptor_direct_address_loads = address_loads
        .iter()
        .filter(|load| load.value == chr_descriptor_runtime || load.value == chr_descriptor_alias)
        .count();
    let chr_descriptor_raw_pointer_words = text
        .chunks_exact(4)
        .filter(|word| {
            let value = u32::from_le_bytes((*word).try_into().expect("4바이트 word"));
            value == chr_descriptor_runtime || value == chr_descriptor_alias
        })
        .count();
    if chr_descriptor_direct_address_loads != 0 || chr_descriptor_raw_pointer_words != 0 {
        return Err(error(format!(
            "PUYO1 CHR descriptor direct 참조가 생김: address/raw={chr_descriptor_direct_address_loads}/{chr_descriptor_raw_pointer_words}"
        )));
    }

    Ok(P1DescriptorRootCensus {
        literal_combines,
        resource_x36_selectors,
        alternate_branch_combines: P1_DESCRIPTOR_ROOT_ALTERNATE_BRANCH_COMBINES.to_vec(),
        registration_stores: P1_DESCRIPTOR_ROOT_REGISTRATION_STORES.to_vec(),
        chr_descriptor_direct_address_loads,
        chr_descriptor_raw_pointer_words,
        descriptor_table_records: P1_DESCRIPTOR_TABLE_RECORDS,
        descriptor_table_sha256,
        descriptor_table_address_loads,
        descriptor_table_preadjusted_address_loads,
        descriptor_table_raw_pointer_locations,
        descriptor_table_preadjusted_raw_pointers,
    })
}
