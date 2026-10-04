//! PUYO1 alternate static descriptor initializer의 typed 명령 검증.

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::super::binary_checks::{high_low, require};
use super::super::{
    SubgameSeqE6GlyphError, SubgameSeqE6GlyphSpec, P1_STATIC_INITIALIZER_RESOURCE,
    P1_STATIC_INITIALIZER_RUNTIME,
};

pub(super) fn verify_p1_static_initializer(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<(), SubgameSeqE6GlyphError> {
    for (runtime, expected) in [
        (
            0x8002_A2BC,
            K::Sh {
                rt: Reg::A0,
                base: Reg::GP,
                offset: 0x04EC,
            },
        ),
        (
            0x8002_A2C0,
            K::Andi {
                rt: Reg::V1,
                rs: Reg::A0,
                imm: 0xFFFF,
            },
        ),
        (
            0x8002_A2C4,
            K::Lui {
                rt: Reg::A0,
                imm: 0x800B,
            },
        ),
        (
            0x8002_A2C8,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V1,
                sa: 3,
            },
        ),
        (
            0x8002_A2CC,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8002_A2D0,
            K::Lw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0x4330,
            },
        ),
        (
            0x8002_A2D4,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8002_A2D8,
            K::Sh {
                rt: Reg::A3,
                base: Reg::GP,
                offset: 0x04EE,
            },
        ),
        (
            0x8002_A2DC,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8002_A2E0,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8009,
            },
        ),
        (
            0x8002_A2E4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -0x61C4,
            },
        ),
        (
            0x8002_A2E8,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x10,
            },
        ),
        (
            0x8002_A2EC,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8009,
            },
        ),
        (
            0x8002_A2F0,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -0x5BA4,
            },
        ),
        (
            0x8002_A2F4,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x1C,
            },
        ),
        (
            0x8002_A2F8,
            K::Sh {
                rt: Reg::ZERO,
                base: Reg::V1,
                offset: 0x20,
            },
        ),
        (
            0x8002_A2FC,
            K::Sh {
                rt: Reg::A1,
                base: Reg::V1,
                offset: 0x18,
            },
        ),
        (0x8002_A300, K::Jr { rs: Reg::RA }),
        (
            0x8002_A304,
            K::Sh {
                rt: Reg::A2,
                base: Reg::V1,
                offset: 0x1A,
            },
        ),
    ] {
        require(instructions, map, runtime, expected)?;
    }
    Ok(())
}

pub(super) fn verify_p1_static_initializer_calls(
    instructions: &[Instruction],
    map: &ExeMap,
    spec: SubgameSeqE6GlyphSpec,
) -> Result<(), SubgameSeqE6GlyphError> {
    let (root_high, root_low) = high_low(spec.descriptor_root_runtime);
    let (base_high, base_low) = high_low(spec.descriptor_base_runtime);
    for (runtime, expected) in [
        (
            0x8002_A508,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::ZERO,
                imm: P1_STATIC_INITIALIZER_RESOURCE as i16,
            },
        ),
        (
            0x8002_A50C,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: 0x0100,
            },
        ),
        (
            0x8002_A510,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: 0x01E0,
            },
        ),
        (
            0x8002_A514,
            K::Addiu {
                rt: Reg::A3,
                rs: Reg::ZERO,
                imm: 0x03F0,
            },
        ),
        (
            0x8002_A518,
            K::Lui {
                rt: Reg::V1,
                imm: root_high,
            },
        ),
        (
            0x8002_A51C,
            K::Lui {
                rt: Reg::V0,
                imm: base_high,
            },
        ),
        (
            0x8002_A520,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: base_low,
            },
        ),
        (
            0x8002_A524,
            K::Jal {
                target: P1_STATIC_INITIALIZER_RUNTIME,
            },
        ),
        (
            0x8002_A528,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: root_low,
            },
        ),
        (
            0x8002_A770,
            K::Lui {
                rt: Reg::V1,
                imm: root_high,
            },
        ),
        (
            0x8002_A774,
            K::Lui {
                rt: Reg::V0,
                imm: base_high,
            },
        ),
        (
            0x8002_A778,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: base_low,
            },
        ),
        (
            0x8002_A77C,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::ZERO,
                imm: P1_STATIC_INITIALIZER_RESOURCE as i16,
            },
        ),
        (
            0x8002_A780,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: 0x0100,
            },
        ),
        (
            0x8002_A784,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: 0x01E0,
            },
        ),
        (
            0x8002_A788,
            K::Addiu {
                rt: Reg::A3,
                rs: Reg::ZERO,
                imm: 0x03F0,
            },
        ),
        (
            0x8002_A790,
            K::Jal {
                target: P1_STATIC_INITIALIZER_RUNTIME,
            },
        ),
        (
            0x8002_A794,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: root_low,
            },
        ),
    ] {
        require(instructions, map, runtime, expected)?;
    }
    Ok(())
}
