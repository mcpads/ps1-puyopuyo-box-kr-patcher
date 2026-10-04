//! Tokenizer가 pair index와 resource를 glyph object field로 옮기는 경로 검증.

use crate::isa::{FlowKind, Instruction, InstructionKind as K, Reg};

use super::{
    error, high_low, require, SubgameSeqDracGlyphError, SubgameSeqDracGlyphSpec,
    BOX_SUBGAME_SEQ_DRAC_CELL_BASE,
};

pub(super) fn verify_main_glyph_object_route(
    instructions: &[Instruction],
    spec: SubgameSeqDracGlyphSpec,
) -> Result<(), SubgameSeqDracGlyphError> {
    let allocator = spec.glyph_object_allocator_runtime;
    let (callback_high, callback_low) = high_low(spec.glyph_object_callback_runtime);
    let allocator_checks = [
        (
            allocator - 8,
            K::Lui {
                rt: Reg::A0,
                imm: callback_high,
            },
        ),
        (
            allocator - 4,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: callback_low,
            },
        ),
        (
            allocator,
            K::Jal {
                target: spec.auxiliary_object_allocator_target,
            },
        ),
    ];
    for (address, expected) in allocator_checks {
        require(instructions, spec, address, expected)?;
    }

    let expected_instructions =
        usize::try_from((spec.resource_object_store_runtime - spec.cell_index_runtime) / 4 + 1)
            .expect("DRAC glyph block 길이는 usize 범위");
    let block = instructions
        .iter()
        .filter(|instruction| {
            (spec.cell_index_runtime..=spec.resource_object_store_runtime)
                .contains(&instruction.addr)
        })
        .collect::<Vec<_>>();
    if block.len() != expected_instructions {
        return Err(error(format!(
            "{} DRAC main glyph block 명령 수 불일치: expected={expected_instructions}, actual={}",
            spec.exe,
            block.len()
        )));
    }
    let branch_sites = block
        .iter()
        .filter(|instruction| instruction.flow_kind() != FlowKind::Sequential)
        .map(|instruction| instruction.addr)
        .collect::<Vec<_>>();
    if !branch_sites.is_empty() {
        return Err(error(format!(
            "{} DRAC main glyph cell→resource 사이 제어 흐름 발견: {branch_sites:08X?}",
            spec.exe
        )));
    }
    Ok(())
}

pub(super) fn verify_tokenizer(
    instructions: &[Instruction],
    spec: SubgameSeqDracGlyphSpec,
) -> Result<(), SubgameSeqDracGlyphError> {
    let start = spec.tokenizer_runtime;
    let (message_high, message_low) = high_low(spec.message_buffer_runtime);
    let (repertoire_high, repertoire_low) = high_low(spec.repertoire_buffer_runtime);
    let (token_high, token_low) = high_low(spec.token_table_runtime);
    let address_checks = [
        (
            start + 4,
            K::Lui {
                rt: Reg::A0,
                imm: message_high,
            },
        ),
        (
            start + 8,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A0,
                offset: message_low,
            },
        ),
        (
            start + 0x18,
            K::Lui {
                rt: Reg::V0,
                imm: repertoire_high,
            },
        ),
        (
            start + 0x1C,
            K::Addiu {
                rt: Reg::T2,
                rs: Reg::V0,
                imm: repertoire_low,
            },
        ),
        (
            start + 0x20,
            K::Addiu {
                rt: Reg::T3,
                rs: Reg::A0,
                imm: message_low,
            },
        ),
        (
            start + 0x28,
            K::Lui {
                rt: Reg::V0,
                imm: token_high,
            },
        ),
        (
            start + 0x2C,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::V0,
                imm: token_low,
            },
        ),
    ];
    for (address, expected) in address_checks {
        require(instructions, spec, address, expected)?;
    }

    let scan = spec.pair_lookup_runtime;
    let scan_checks = [
        (
            scan,
            K::Addu {
                rd: Reg::T1,
                rs: Reg::V1,
                rt: Reg::ZERO,
            },
        ),
        (
            scan + 4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::A3,
                imm: 1,
            },
        ),
        (
            scan + 8,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::T2,
                offset: 0,
            },
        ),
        (
            scan + 0x10,
            K::Bne {
                rs: Reg::V1,
                rt: Reg::T1,
                offset: 6,
            },
        ),
        (
            scan + 0x14,
            K::Addu {
                rd: Reg::T0,
                rs: Reg::V0,
                rt: Reg::T3,
            },
        ),
        (
            scan + 0x18,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::T2,
                offset: 1,
            },
        ),
        (
            scan + 0x1C,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::T0,
                offset: 0,
            },
        ),
        (
            scan + 0x24,
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 16,
            },
        ),
        (
            scan + 0x2C,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 2,
            },
        ),
        (
            scan + 0x30,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::A0,
                rt: Reg::T2,
            },
        ),
        (
            scan + 0x34,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            scan + 0x3C,
            K::Bne {
                rs: Reg::V0,
                rt: Reg::T1,
                offset: -4,
            },
        ),
        (
            scan + 0x40,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 2,
            },
        ),
        (
            scan + 0x44,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: -2,
            },
        ),
        (
            scan + 0x48,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::A0,
                rt: Reg::T2,
            },
        ),
        (
            scan + 0x4C,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            scan + 0x50,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::T0,
                offset: 0,
            },
        ),
        (
            scan + 0x58,
            K::Bne {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: -11,
            },
        ),
        (
            scan + 0x5C,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 2,
            },
        ),
        (
            scan + 0x60,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: -2,
            },
        ),
        (
            scan + 0x64,
            K::Srl {
                rd: Reg::V0,
                rt: Reg::A0,
                sa: 31,
            },
        ),
        (
            scan + 0x68,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::A0,
                rt: Reg::V0,
            },
        ),
        (
            scan + 0x6C,
            K::Sra {
                rd: Reg::A0,
                rt: Reg::V0,
                sa: 1,
            },
        ),
        (
            scan + 0x70,
            K::Sw {
                rt: Reg::A0,
                base: Reg::A2,
                offset: 0,
            },
        ),
    ];
    for (address, expected) in scan_checks {
        require(instructions, spec, address, expected)?;
    }
    if spec.pair_index_store_runtime != scan + 0x70 {
        return Err(error(format!(
            "{} DRAC pair index store 주소 불일치",
            spec.exe
        )));
    }

    let cell = spec.cell_index_runtime;
    let cell_checks = [
        (
            cell,
            K::Andi {
                rt: Reg::V1,
                rs: Reg::S1,
                imm: 0xFF,
            },
        ),
        (
            spec.cell_index_add_runtime,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: BOX_SUBGAME_SEQ_DRAC_CELL_BASE as i16,
            },
        ),
        (
            spec.cell_index_store_runtime,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x6A,
            },
        ),
        (
            spec.resource_global_load_runtime,
            K::Lhu {
                rt: Reg::A1,
                base: Reg::GP,
                offset: spec.resource_gp_offset,
            },
        ),
        (
            spec.resource_object_store_runtime,
            K::Sh {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0x6C,
            },
        ),
    ];
    for (address, expected) in cell_checks {
        require(instructions, spec, address, expected)?;
    }
    Ok(())
}
