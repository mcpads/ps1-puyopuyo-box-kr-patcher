//! GP-relative resource state와 auxiliary object 소비 경로 검증.

use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{
    error, high_low, require, SubgameSeqDracGlyphError, SubgameSeqDracGlyphSpec,
    BOX_SUBGAME_SEQ_DRAC_AUXILIARY_CELL, BOX_SUBGAME_SEQ_DRAC_AUXILIARY_CELL_TABLE_OFFSET,
    BOX_SUBGAME_SEQ_DRAC_AUXILIARY_NEXT_CELL_TABLE_OFFSET,
    BOX_SUBGAME_SEQ_DRAC_AUXILIARY_OBJECT_SITES,
};

pub(super) fn verify_main_resource_consumers(
    instructions: &[Instruction],
    spec: SubgameSeqDracGlyphSpec,
) -> Result<(), SubgameSeqDracGlyphError> {
    let (gp_high, gp_low) = high_low(spec.gp_runtime);
    require(
        instructions,
        spec,
        spec.gp_lui_runtime,
        K::Lui {
            rt: Reg::GP,
            imm: gp_high,
        },
    )?;
    require(
        instructions,
        spec,
        spec.gp_addiu_runtime,
        K::Addiu {
            rt: Reg::GP,
            rs: Reg::GP,
            imm: gp_low,
        },
    )?;
    let actual_accesses = instructions
        .iter()
        .filter_map(|instruction| {
            let (base, offset) = memory_base_and_offset(instruction.kind)?;
            (base == Reg::GP && offset == spec.resource_gp_offset).then_some(instruction.addr)
        })
        .collect::<Vec<_>>();
    let expected_accesses = [
        spec.resource_global_load_runtime,
        spec.e2_handler_runtime + 0x10,
    ];
    if actual_accesses != expected_accesses {
        return Err(error(format!(
            "{} DRAC main resource GP 접근 집합 불일치: expected={expected_accesses:08X?}, \
             actual={actual_accesses:08X?}",
            spec.exe
        )));
    }
    Ok(())
}

pub(super) fn gp_runtime_address(gp_runtime: u32, offset: i16) -> Option<u32> {
    gp_runtime.checked_add_signed(i32::from(offset))
}

pub(super) fn memory_base_and_offset(kind: K) -> Option<(Reg, i16)> {
    match kind {
        K::Lb { base, offset, .. }
        | K::Lbu { base, offset, .. }
        | K::Lh { base, offset, .. }
        | K::Lhu { base, offset, .. }
        | K::Lw { base, offset, .. }
        | K::Lwl { base, offset, .. }
        | K::Lwr { base, offset, .. }
        | K::Sb { base, offset, .. }
        | K::Sh { base, offset, .. }
        | K::Sw { base, offset, .. }
        | K::Swl { base, offset, .. }
        | K::Swr { base, offset, .. } => Some((base, offset)),
        _ => None,
    }
}

pub(super) fn verify_auxiliary_consumers(
    instructions: &[Instruction],
    spec: SubgameSeqDracGlyphSpec,
) -> Result<(), SubgameSeqDracGlyphError> {
    let actual_accesses = instructions
        .iter()
        .filter_map(|instruction| {
            let (base, offset) = memory_base_and_offset(instruction.kind)?;
            (base == Reg::GP && offset == spec.auxiliary_gp_offset).then_some(instruction.addr)
        })
        .collect::<Vec<_>>();
    let expected_accesses = [
        spec.auxiliary_descriptor_load_runtime,
        spec.auxiliary_object_load_runtimes[0],
        spec.auxiliary_object_load_runtimes[1],
        spec.e2_handler_runtime + 0x24,
    ];
    if actual_accesses != expected_accesses {
        return Err(error(format!(
            "{} DRAC auxiliary GP 접근 집합 불일치: expected={expected_accesses:08X?}, \
             actual={actual_accesses:08X?}",
            spec.exe
        )));
    }

    let descriptor = spec.auxiliary_descriptor_load_runtime;
    let (root_high, root_low) = high_low(spec.descriptor_root_runtime);
    let descriptor_checks = [
        (
            descriptor - 4,
            K::Lui {
                rt: Reg::A1,
                imm: root_high,
            },
        ),
        (
            descriptor,
            K::Lw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: spec.auxiliary_gp_offset,
            },
        ),
        (
            descriptor + 8,
            K::Sll {
                rd: Reg::A0,
                rt: Reg::V0,
                sa: 3,
            },
        ),
        (
            descriptor + 0x0C,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A0,
                rt: Reg::V0,
            },
        ),
        (
            descriptor + 0x10,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: root_low,
            },
        ),
        (
            descriptor + 0x14,
            K::Sll {
                rd: Reg::A0,
                rt: Reg::A0,
                sa: 2,
            },
        ),
        (
            descriptor + 0x18,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            descriptor + 0x1C,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x1C,
            },
        ),
        (
            spec.auxiliary_next_cell_store_runtime,
            K::Sh {
                rt: Reg::V0,
                base: Reg::V1,
                offset: BOX_SUBGAME_SEQ_DRAC_AUXILIARY_NEXT_CELL_TABLE_OFFSET,
            },
        ),
        (
            descriptor + 0x40,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: root_low,
            },
        ),
        (
            descriptor + 0x48,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            descriptor + 0x4C,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x1C,
            },
        ),
        (
            spec.auxiliary_cell_store_runtime,
            K::Sh {
                rt: Reg::V0,
                base: Reg::V1,
                offset: BOX_SUBGAME_SEQ_DRAC_AUXILIARY_CELL_TABLE_OFFSET,
            },
        ),
    ];
    for (address, expected) in descriptor_checks {
        require(instructions, spec, address, expected)?;
    }

    for index in 0..BOX_SUBGAME_SEQ_DRAC_AUXILIARY_OBJECT_SITES {
        let allocator = spec.auxiliary_object_allocator_runtimes[index];
        let load = spec.auxiliary_object_load_runtimes[index];
        if load != allocator + 0x10
            || spec.auxiliary_object_cell_store_runtimes[index] != allocator + 0x18
            || spec.auxiliary_object_resource_store_runtimes[index] != allocator + 0x30
        {
            return Err(error(format!(
                "{} DRAC auxiliary object {index} 주소 관계 불일치",
                spec.exe
            )));
        }
        let object_checks = [
            (
                allocator,
                K::Jal {
                    target: spec.auxiliary_object_allocator_target,
                },
            ),
            (
                load,
                K::Lhu {
                    rt: Reg::A0,
                    base: Reg::GP,
                    offset: spec.auxiliary_gp_offset,
                },
            ),
            (
                load + 4,
                K::Addiu {
                    rt: Reg::V1,
                    rs: Reg::ZERO,
                    imm: BOX_SUBGAME_SEQ_DRAC_AUXILIARY_CELL as i16,
                },
            ),
            (
                spec.auxiliary_object_cell_store_runtimes[index],
                K::Sh {
                    rt: Reg::V1,
                    base: Reg::V0,
                    offset: 0x6A,
                },
            ),
            (
                spec.auxiliary_object_resource_store_runtimes[index],
                K::Sh {
                    rt: Reg::A0,
                    base: Reg::V0,
                    offset: 0x6C,
                },
            ),
        ];
        for (address, expected) in object_checks {
            require(instructions, spec, address, expected)?;
        }
    }
    Ok(())
}
