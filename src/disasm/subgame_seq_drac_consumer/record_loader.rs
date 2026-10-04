//! 현재 message record의 header와 본문을 읽는 EXE 명령열을 검증한다.

use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{require, SubgameSeqDracConsumerError, SubgameSeqDracConsumerSpec};

pub(super) fn verify_record_loader(
    instructions: &[Instruction],
    spec: SubgameSeqDracConsumerSpec,
) -> Result<(), SubgameSeqDracConsumerError> {
    require(
        instructions,
        spec,
        spec.current_record_load_runtime,
        K::Lw {
            rt: Reg::V0,
            base: Reg::S6,
            offset: 0x1C,
        },
    )?;
    require(
        instructions,
        spec,
        spec.message_pointer_load_runtime,
        K::Lw {
            rt: Reg::S5,
            base: Reg::V0,
            offset: 4,
        },
    )?;
    let header = spec.header_load_runtime;
    let header_checks = [
        (
            header,
            K::Lbu {
                rt: Reg::S0,
                base: Reg::S5,
                offset: 0,
            },
        ),
        (
            header + 4,
            K::Lbu {
                rt: Reg::S3,
                base: Reg::S5,
                offset: 1,
            },
        ),
        (
            header + 8,
            K::Lbu {
                rt: Reg::S1,
                base: Reg::S5,
                offset: 2,
            },
        ),
        (
            header + 0x0C,
            K::Lbu {
                rt: Reg::S2,
                base: Reg::S5,
                offset: 3,
            },
        ),
        (
            header + 0x10,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::S0,
                imm: -1,
            },
        ),
        (
            header + 0x14,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::S3,
                imm: -1,
            },
        ),
        (
            header + 0x18,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::S1,
                imm: 1,
            },
        ),
        (
            spec.header_call_runtime,
            K::Jal {
                target: spec.header_consumer_runtime,
            },
        ),
        (
            spec.header_call_runtime + 4,
            K::Addiu {
                rt: Reg::A3,
                rs: Reg::S2,
                imm: 1,
            },
        ),
    ];
    for (address, expected) in header_checks {
        require(instructions, spec, address, expected)?;
    }

    let body = spec.body_first_load_runtime;
    let body_checks = [
        (
            body,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::S5,
                offset: 4,
            },
        ),
        (
            body + 4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x65,
            },
        ),
        (
            body + 8,
            K::Bne {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 5,
            },
        ),
        (
            body + 0x10,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::S5,
                offset: 5,
            },
        ),
        (
            body + 0x18,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::V1,
                offset: 25,
            },
        ),
        (
            spec.tokenizer_call_runtime,
            K::Jal {
                target: spec.tokenizer_runtime,
            },
        ),
    ];
    for (address, expected) in body_checks {
        require(instructions, spec, address, expected)?;
    }
    Ok(())
}
