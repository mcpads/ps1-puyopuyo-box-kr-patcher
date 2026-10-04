//! DRAC descriptor root의 EXE 초기화 경로 검증.

use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{high_low, require, SubgameSeqDracGlyphError, SubgameSeqDracGlyphSpec};

pub(super) fn verify_descriptor_root_initialization(
    instructions: &[Instruction],
    spec: SubgameSeqDracGlyphSpec,
) -> Result<(), SubgameSeqDracGlyphError> {
    let (base_high, base_low) = high_low(spec.descriptor_base_runtime);
    let (root_high, root_low) = high_low(spec.descriptor_root_runtime);
    let checks = [
        (
            spec.descriptor_root_lui_runtime,
            K::Lui {
                rt: Reg::V1,
                imm: root_high,
            },
        ),
        (
            spec.descriptor_base_lui_runtime,
            K::Lui {
                rt: Reg::V0,
                imm: base_high,
            },
        ),
        (
            spec.descriptor_base_addiu_runtime,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: base_low,
            },
        ),
        (
            spec.descriptor_root_store_runtime,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: root_low,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, spec, address, expected)?;
    }
    Ok(())
}
