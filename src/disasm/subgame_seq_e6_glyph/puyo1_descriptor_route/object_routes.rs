//! PUYO1 CHR object 생성 route와 allocator 호출 분모 검증.

use std::collections::{BTreeMap, BTreeSet};

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::super::binary_checks::require;
use super::super::{
    error, P1AllocatorCensus, SubgameSeqE6GlyphError, P1_ALLOCATOR_DIRECT_CALLS,
    P1_ALLOCATOR_DYNAMIC_TYPE_CALLS, P1_ALLOCATOR_IMMEDIATE_TYPE_CALLS,
    P1_DIRECT_CHR_OBJECT_ALLOCATOR_CALLS, P1_DIRECT_CHR_OBJECT_RESOURCE_STORES,
    P1_IMMEDIATE_TYPE9_CALLS, P1_INACTIVE_TYPE9_CALLS, P1_NON_OBJECT_LITERAL_006D_LOADS,
};

pub(super) fn verify_p1_direct_chr_object_routes(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<(Vec<u32>, usize, usize), SubgameSeqE6GlyphError> {
    let routes = [
        (
            0x8001_84C0,
            0x8001_84CC,
            0x8001_84D0,
            0x8001_84E4,
            0x8001_84E8,
            Some(0x8001_84EC),
            0x8001_84F0,
            Reg::V1,
        ),
        (
            0x8001_8DA8,
            0x8001_8DB4,
            0x8001_8DB8,
            0x8001_8DCC,
            0x8001_8DD0,
            Some(0x8001_8DD4),
            0x8001_8DD8,
            Reg::V1,
        ),
        (
            0x8001_8E9C,
            0x8001_8EA4,
            0x8001_8EA8,
            0x8001_8EC8,
            0x8001_8ECC,
            None,
            0x8001_8ED4,
            Reg::S1,
        ),
    ];
    let mut stores = Vec::new();
    let mut fixed = 0usize;
    let mut variable = 0usize;
    for (
        allocator_call,
        active_value,
        active_store,
        resource_value,
        resource_store,
        cell_value,
        cell_store,
        cell_source,
    ) in routes
    {
        require(
            instructions,
            map,
            allocator_call,
            K::Jal {
                target: 0x8001_53EC,
            },
        )?;
        require(
            instructions,
            map,
            allocator_call + 4,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: 9,
            },
        )?;
        require(
            instructions,
            map,
            active_value,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
        )?;
        require(
            instructions,
            map,
            active_store,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x70,
            },
        )?;
        require(
            instructions,
            map,
            resource_value,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 0x006D,
            },
        )?;
        require(
            instructions,
            map,
            resource_store,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x6C,
            },
        )?;
        if let Some(cell_value) = cell_value {
            require(
                instructions,
                map,
                cell_value,
                K::Addiu {
                    rt: Reg::V1,
                    rs: Reg::ZERO,
                    imm: 0x001C,
                },
            )?;
            fixed += 1;
        } else {
            variable += 1;
        }
        require(
            instructions,
            map,
            cell_store,
            K::Sh {
                rt: cell_source,
                base: Reg::V0,
                offset: 0x6A,
            },
        )?;
        stores.push(resource_store);
    }
    if stores != P1_DIRECT_CHR_OBJECT_RESOURCE_STORES || fixed != 2 || variable != 1 {
        return Err(error(format!(
            "PUYO1 direct CHR object route 분모 불일치: stores={stores:?}, fixed={fixed}, variable={variable}"
        )));
    }
    Ok((stores, fixed, variable))
}

pub(super) fn verify_p1_non_object_literal_006d_load(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<Vec<u32>, SubgameSeqE6GlyphError> {
    for (runtime, expected) in [
        (
            0x8005_CF2C,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 0x006D,
            },
        ),
        (
            0x8005_CF38,
            K::Sh {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0x26,
            },
        ),
        (
            0x8005_CF44,
            K::Jal {
                target: 0x8002_7B5C,
            },
        ),
    ] {
        require(instructions, map, runtime, expected)?;
    }
    Ok(P1_NON_OBJECT_LITERAL_006D_LOADS.to_vec())
}

pub(super) fn verify_p1_allocator_census(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<P1AllocatorCensus, SubgameSeqE6GlyphError> {
    let mut direct_calls = 0usize;
    let mut immediate_types = BTreeMap::<i16, usize>::new();
    let mut dynamic_type_calls = 0usize;
    let mut immediate_type9_calls = Vec::new();
    for (index, instruction) in instructions.iter().enumerate() {
        if instruction.kind
            != (K::Jal {
                target: 0x8001_53EC,
            })
        {
            continue;
        }
        direct_calls += 1;
        match instructions.get(index + 1) {
            Some(Instruction {
                addr,
                kind:
                    K::Addiu {
                        rt: Reg::A1,
                        rs: Reg::ZERO,
                        imm,
                    },
                ..
            }) if *addr == instruction.addr + 4 => {
                *immediate_types.entry(*imm).or_default() += 1;
                if *imm == 9 {
                    immediate_type9_calls.push(instruction.addr);
                }
            }
            _ => dynamic_type_calls += 1,
        }
    }
    let expected_types = BTreeMap::from([(1, 1), (2, 49), (3, 120), (4, 11), (5, 57), (9, 4)]);
    let immediate_type_calls = immediate_types.values().sum::<usize>();
    if direct_calls != P1_ALLOCATOR_DIRECT_CALLS
        || immediate_type_calls != P1_ALLOCATOR_IMMEDIATE_TYPE_CALLS
        || dynamic_type_calls != P1_ALLOCATOR_DYNAMIC_TYPE_CALLS
        || immediate_types != expected_types
        || immediate_type9_calls != P1_IMMEDIATE_TYPE9_CALLS
    {
        return Err(error(format!(
            "PUYO1 allocator direct/type 분모 불일치: direct={direct_calls}, immediate={immediate_type_calls}, dynamic={dynamic_type_calls}, types={immediate_types:?}, type9={immediate_type9_calls:?}"
        )));
    }

    for (runtime, expected) in [
        (
            0x8001_8F08,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8002,
            },
        ),
        (
            0x8001_8F0C,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: -0x71FC,
            },
        ),
        (
            P1_INACTIVE_TYPE9_CALLS[0],
            K::Jal {
                target: 0x8001_53EC,
            },
        ),
        (
            P1_INACTIVE_TYPE9_CALLS[0] + 4,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: 9,
            },
        ),
        (
            0x8001_8F38,
            K::Sh {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: 0x70,
            },
        ),
        (0x8001_8F54, K::Jr { rs: Reg::RA }),
    ] {
        require(instructions, map, runtime, expected)?;
    }
    let inactive_writes_glyph_selector = instructions.iter().any(|instruction| {
        instruction.addr > P1_INACTIVE_TYPE9_CALLS[0]
            && instruction.addr <= 0x8001_8F54
            && matches!(
                instruction.kind,
                K::Sb {
                    base: Reg::V0,
                    offset: 0x6A | 0x6C,
                    ..
                } | K::Sh {
                    base: Reg::V0,
                    offset: 0x6A | 0x6C,
                    ..
                } | K::Sw {
                    base: Reg::V0,
                    offset: 0x6A | 0x6C,
                    ..
                }
            )
    });
    if inactive_writes_glyph_selector {
        return Err(error(
            "PUYO1 inactive type-9 route가 resource/cell selector를 씀",
        ));
    }

    let direct_chr_object_calls = P1_DIRECT_CHR_OBJECT_ALLOCATOR_CALLS.to_vec();
    let inactive_type9_calls = P1_INACTIVE_TYPE9_CALLS.to_vec();
    let classified_type9 = direct_chr_object_calls
        .iter()
        .chain(&inactive_type9_calls)
        .copied()
        .collect::<BTreeSet<_>>();
    let immediate_type9 = immediate_type9_calls
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if classified_type9 != immediate_type9 {
        return Err(error(format!(
            "PUYO1 immediate type-9 분류 불완전: type9={immediate_type9:?}, classified={classified_type9:?}"
        )));
    }
    Ok(P1AllocatorCensus {
        direct_calls,
        immediate_type_calls,
        dynamic_type_calls,
        immediate_type9_calls,
        direct_chr_object_calls,
        inactive_type9_calls,
    })
}
