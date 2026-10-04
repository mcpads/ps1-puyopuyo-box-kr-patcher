//! 관측 token handler의 message object field 효과를 검증한다.

use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{
    error, require, SubgameSeqDracConsumerError, SubgameSeqDracConsumerSpec,
    BOX_SUBGAME_SEQ_DRAC_TOKEN_FIRST,
};

pub(super) fn verify_forbidden_state_handlers(
    instructions: &[Instruction],
    spec: SubgameSeqDracConsumerSpec,
) -> Result<(), SubgameSeqDracConsumerError> {
    let token_63 = *spec
        .dispatch_targets
        .first()
        .ok_or_else(|| error("DRAC token dispatch가 비어 있음"))?;
    let token_63_checks = [
        (
            token_63,
            K::Lw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0x98,
            },
        ),
        (
            token_63 + 4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: -1,
            },
        ),
        (
            token_63 + 8,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0xAC,
            },
        ),
        (
            token_63 + 0x0C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            token_63 + 0x10,
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: spec.token_63_global_gp_offset,
            },
        ),
        (
            token_63 + 0x18,
            K::Sw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0xA4,
            },
        ),
    ];
    for (address, expected) in token_63_checks {
        require(instructions, spec, address, expected)?;
    }

    match spec.token_78_handler_runtime {
        Some(token_78) => {
            let last = spec
                .dispatch_targets
                .last()
                .copied()
                .ok_or_else(|| error("DRAC token dispatch가 비어 있음"))?;
            if last != token_78
                || BOX_SUBGAME_SEQ_DRAC_TOKEN_FIRST as usize + spec.dispatch_targets.len() - 1
                    != 0x78
            {
                return Err(error(format!(
                    "{} DRAC token 0x78 dispatch 경계 불일치",
                    spec.exe
                )));
            }
            let checks = [
                (
                    token_78,
                    K::Addiu {
                        rt: Reg::V0,
                        rs: Reg::ZERO,
                        imm: 0x2B,
                    },
                ),
                (
                    token_78 + 4,
                    K::Sw {
                        rt: Reg::ZERO,
                        base: Reg::S0,
                        offset: 0x50,
                    },
                ),
                (
                    token_78 + 8,
                    K::Sw {
                        rt: Reg::ZERO,
                        base: Reg::S0,
                        offset: 0x54,
                    },
                ),
                (
                    token_78 + 0x10,
                    K::Sw {
                        rt: Reg::V0,
                        base: Reg::S0,
                        offset: 0xA4,
                    },
                ),
            ];
            for (address, expected) in checks {
                require(instructions, spec, address, expected)?;
            }
        }
        None => {
            if BOX_SUBGAME_SEQ_DRAC_TOKEN_FIRST as usize + spec.dispatch_targets.len() - 1 != 0x77 {
                return Err(error(format!(
                    "{} DRAC token dispatch 끝이 0x77이 아님",
                    spec.exe
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn verify_observed_handler_effects(
    instructions: &[Instruction],
    spec: SubgameSeqDracConsumerSpec,
) -> Result<(), SubgameSeqDracConsumerError> {
    let end = spec.end_handler_runtime;
    let checks = [
        (
            end,
            K::Lw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0x98,
            },
        ),
        (
            end + 4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            end + 0x0C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0xAC,
            },
        ),
        (
            spec.end_value_store_runtime,
            K::Sw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0xA4,
            },
        ),
        (
            spec.default_value_handler_runtime + 4,
            K::Sw {
                rt: Reg::A2,
                base: Reg::S0,
                offset: 0x98,
            },
        ),
        (
            spec.current_value_handler_runtime + 4,
            K::Sw {
                rt: Reg::A2,
                base: Reg::S0,
                offset: 0xA4,
            },
        ),
        (
            spec.line_break_handler_runtime,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x54,
            },
        ),
        (
            spec.line_break_handler_runtime + 4,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::S0,
                offset: 0x50,
            },
        ),
        (
            spec.line_break_handler_runtime + 8,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
        ),
        (
            spec.line_break_handler_runtime + 0x10,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x54,
            },
        ),
        (
            spec.column_advance_handler_runtime,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x50,
            },
        ),
        (
            spec.column_advance_handler_runtime + 4,
            K::Lw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0xA8,
            },
        ),
        (
            spec.column_advance_handler_runtime + 8,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
        ),
        (
            spec.column_advance_handler_runtime + 0x0C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x50,
            },
        ),
        (
            spec.column_advance_handler_runtime + 0x14,
            K::Sw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0xA4,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, spec, address, expected)?;
    }
    Ok(())
}
