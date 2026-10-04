//! E1/E2 command가 repertoire와 resource를 object state로 옮기는 경로 검증.

use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{high_low, require, SubgameSeqDracGlyphError, SubgameSeqDracGlyphSpec};

pub(super) fn verify_repertoire_handler(
    instructions: &[Instruction],
    spec: SubgameSeqDracGlyphSpec,
) -> Result<(), SubgameSeqDracGlyphError> {
    let start = spec.e1_handler_runtime;
    let (buffer_high, buffer_low) = high_low(spec.repertoire_buffer_runtime);
    let checks = [
        (
            start,
            K::Addu {
                rd: Reg::T1,
                rs: Reg::A0,
                rt: Reg::ZERO,
            },
        ),
        (
            start + 4,
            K::Lw {
                rt: Reg::V0,
                base: Reg::T1,
                offset: 0x1C,
            },
        ),
        (
            start + 8,
            K::Addu {
                rd: Reg::A3,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            start + 0x0C,
            K::Lw {
                rt: Reg::T0,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            start + 0x14,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::T0,
                offset: 0,
            },
        ),
        (
            start + 0x18,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x65,
            },
        ),
        (
            start + 0x1C,
            K::Bne {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 5,
            },
        ),
        (
            start + 0x3C,
            K::Lui {
                rt: Reg::V1,
                imm: buffer_high,
            },
        ),
        (
            start + 0x40,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: buffer_low,
            },
        ),
        (
            start + 0x54,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            start + 0x5C,
            K::Sb {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0,
            },
        ),
        (
            start + 0x60,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 0,
            },
        ),
        (
            start + 0x68,
            K::Sb {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            start + 0xA8,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::ZERO,
                imm: 0x65,
            },
        ),
        (
            start + 0xAC,
            K::Sb {
                rt: Reg::A0,
                base: Reg::A1,
                offset: 0,
            },
        ),
        (
            start + 0xB0,
            K::Sb {
                rt: Reg::A0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            start + 0xBC,
            K::Sb {
                rt: Reg::A0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            start + 0xC0,
            K::Sb {
                rt: Reg::A0,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            start + 0xC4,
            K::Lw {
                rt: Reg::V0,
                base: Reg::T1,
                offset: 0x1C,
            },
        ),
        (
            start + 0xCC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 8,
            },
        ),
        (start + 0xD0, K::Jr { rs: Reg::RA }),
        (
            start + 0xD4,
            K::Sw {
                rt: Reg::V0,
                base: Reg::T1,
                offset: 0x1C,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, spec, address, expected)?;
    }
    Ok(())
}

pub(super) fn verify_resource_handler(
    instructions: &[Instruction],
    spec: SubgameSeqDracGlyphSpec,
) -> Result<(), SubgameSeqDracGlyphError> {
    let start = spec.e2_handler_runtime;
    let checks = [
        (
            start,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            start + 4,
            K::Lw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            start + 8,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 2,
            },
        ),
        (
            start + 0x10,
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: spec.resource_gp_offset,
            },
        ),
        (
            start + 0x14,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V1,
                rt: Reg::ZERO,
            },
        ),
        (
            start + 0x18,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::V1,
                offset: 4,
            },
        ),
        (
            start + 0x1C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 8,
            },
        ),
        (
            start + 0x20,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            start + 0x24,
            K::Sw {
                rt: Reg::V1,
                base: Reg::GP,
                offset: spec.auxiliary_gp_offset,
            },
        ),
        (start + 0x28, K::Jr { rs: Reg::RA }),
    ];
    for (address, expected) in checks {
        require(instructions, spec, address, expected)?;
    }
    Ok(())
}
