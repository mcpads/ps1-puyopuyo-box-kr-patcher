//! header와 message object에서 glyph 좌표로 이어지는 dataflow를 검증한다.

use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{error, high_low, require, SubgameSeqDracConsumerError, SubgameSeqDracConsumerSpec};

pub(super) fn verify_layout_dataflow(
    instructions: &[Instruction],
    spec: SubgameSeqDracConsumerSpec,
) -> Result<(), SubgameSeqDracConsumerError> {
    let (variant_high, variant_low) = high_low(spec.variant_state_runtime);
    let variant_checks = [
        (
            spec.variant_store_runtime - 4,
            K::Lui {
                rt: Reg::V0,
                imm: variant_high,
            },
        ),
        (
            spec.variant_store_runtime,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: variant_low,
            },
        ),
    ];
    for (address, expected) in variant_checks {
        require(instructions, spec, address, expected)?;
    }

    let header = spec.header_state_store_runtime;
    let header_checks = [
        (
            header,
            K::Sw {
                rt: Reg::S1,
                base: Reg::GP,
                offset: spec.header_width_gp_offset,
            },
        ),
        (
            header + 4,
            K::Sw {
                rt: Reg::S2,
                base: Reg::GP,
                offset: spec.header_rows_gp_offset,
            },
        ),
        (
            header + 8,
            K::Sw {
                rt: Reg::S0,
                base: Reg::GP,
                offset: spec.header_x_gp_offset,
            },
        ),
        (
            header + 0x0C,
            K::Sw {
                rt: Reg::S3,
                base: Reg::GP,
                offset: spec.header_y_gp_offset,
            },
        ),
    ];
    for (address, expected) in header_checks {
        require(instructions, spec, address, expected)?;
    }

    let glyph_checks = [
        (
            spec.glyph_header_x_load_runtime,
            K::Lw {
                rt: Reg::V1,
                base: Reg::GP,
                offset: spec.header_x_gp_offset,
            },
        ),
        (
            spec.glyph_column_load_runtime,
            K::Lw {
                rt: spec.glyph_column_reg,
                base: Reg::S0,
                offset: 0x50,
            },
        ),
        (
            spec.glyph_header_y_load_runtime,
            K::Lw {
                rt: spec.glyph_header_y_reg,
                base: Reg::GP,
                offset: spec.header_y_gp_offset,
            },
        ),
        (
            spec.glyph_x_scale_runtime,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 3,
            },
        ),
        (
            spec.glyph_x_scale_runtime + 4,
            K::Sll {
                rd: spec.glyph_column_reg,
                rt: spec.glyph_column_reg,
                sa: 4,
            },
        ),
        (
            spec.glyph_x_scale_runtime + 8,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: spec.glyph_column_reg,
            },
        ),
        (
            spec.glyph_x_scale_runtime + 0x0C,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 12,
            },
        ),
        (
            spec.glyph_x_store_runtime,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x50,
            },
        ),
        (
            spec.glyph_row_load_runtime,
            K::Lw {
                rt: spec.glyph_row_reg,
                base: Reg::S0,
                offset: 0x54,
            },
        ),
        (
            spec.glyph_row_scale_runtime,
            K::Sllv {
                rd: Reg::V1,
                rt: spec.glyph_row_reg,
                rs: Reg::A0,
            },
        ),
        (
            spec.glyph_row_scale_runtime + 4,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: spec.glyph_row_reg,
            },
        ),
        (
            spec.glyph_row_scale_runtime + 8,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: spec.glyph_header_y_reg,
            },
        ),
        (
            spec.glyph_row_scale_runtime + 0x0C,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 15,
            },
        ),
        (
            spec.glyph_y_store_runtime,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x54,
            },
        ),
    ];
    for (address, expected) in glyph_checks {
        require(instructions, spec, address, expected)?;
    }
    if !spec.dispatch_targets.contains(&spec.glyph_handler_runtime) {
        return Err(error(format!(
            "{} DRAC glyph handler가 token dispatch에 없음",
            spec.exe
        )));
    }
    let (update_high, update_low) = high_low(spec.message_update_runtime);
    for &(pointer_site, call_site) in spec.message_allocation_sites {
        let allocation_checks = [
            (
                pointer_site,
                K::Lui {
                    rt: Reg::A0,
                    imm: update_high,
                },
            ),
            (
                pointer_site + 4,
                K::Addiu {
                    rt: Reg::A0,
                    rs: Reg::A0,
                    imm: update_low,
                },
            ),
            (
                call_site,
                K::Jal {
                    target: spec.message_allocator_runtime,
                },
            ),
            (
                call_site + 4,
                K::Addiu {
                    rt: Reg::A1,
                    rs: Reg::ZERO,
                    imm: 5,
                },
            ),
        ];
        for (address, expected) in allocation_checks {
            require(instructions, spec, address, expected)?;
        }
    }
    require(
        instructions,
        spec,
        spec.message_allocator_callback_store_runtime,
        K::Sw {
            rt: Reg::A0,
            base: Reg::A2,
            offset: 0x18,
        },
    )?;
    require(
        instructions,
        spec,
        spec.message_update_runtime + 0x38,
        K::Lw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: 0xA4,
        },
    )?;
    Ok(())
}
