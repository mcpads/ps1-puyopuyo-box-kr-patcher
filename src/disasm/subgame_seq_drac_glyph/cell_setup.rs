//! OP28/sub12가 DRAC cell table을 descriptor에 설치하는 경로 검증.

use sha2::{Digest, Sha256};

use crate::coords::{ExeMap, RuntimeAssetMap};
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{
    error, hex, high_low, read_exe_word, read_i16, read_u16, require, SubgameSeqDracGlyphError,
    SubgameSeqDracGlyphSpec, BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES, BOX_SUBGAME_SEQ_DRAC_ATLAS_HEIGHT,
    BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH, BOX_SUBGAME_SEQ_DRAC_CELL_BASE,
    BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS, BOX_SUBGAME_SEQ_DRAC_CELL_GRID_ROWS,
    BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X, BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y,
    BOX_SUBGAME_SEQ_DRAC_CELL_HEADER_BYTES, BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT,
    BOX_SUBGAME_SEQ_DRAC_CELL_LAST, BOX_SUBGAME_SEQ_DRAC_CELL_STRIDE,
    BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET, BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_PREFIX_END,
    BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_PREFIX_SHA256, BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_RUNTIME,
    BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH, BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS,
    SUBGAME_SEQ_LOAD_DESTINATION,
};

pub(super) fn verify_cell_setup_dispatch(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
    spec: SubgameSeqDracGlyphSpec,
) -> Result<(), SubgameSeqDracGlyphError> {
    let opcode_handler = read_exe_word(map, text, spec.opcode_28_slot_runtime, "DRAC OP28 slot")?;
    if opcode_handler != spec.opcode_28_wrapper_runtime {
        return Err(error(format!(
            "{} DRAC OP28 wrapper 불일치: expected=0x{:08X}, actual=0x{opcode_handler:08X}",
            spec.exe, spec.opcode_28_wrapper_runtime
        )));
    }
    require(
        instructions,
        spec,
        spec.opcode_28_wrapper_runtime + 8,
        K::Jal {
            target: spec.subdispatcher_runtime,
        },
    )?;

    let (table_high, table_low) = high_low(spec.subdispatch_table_runtime);
    let dispatcher = spec.subdispatcher_runtime;
    let dispatcher_checks = [
        (
            dispatcher + 8,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            dispatcher + 0x0C,
            K::Lui {
                rt: Reg::V1,
                imm: table_high,
            },
        ),
        (
            dispatcher + 0x10,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            dispatcher + 0x14,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: table_low,
            },
        ),
        (
            dispatcher + 0x18,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            dispatcher + 0x1C,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            dispatcher + 0x20,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            dispatcher + 0x28,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
    ];
    for (address, expected) in dispatcher_checks {
        require(instructions, spec, address, expected)?;
    }

    let subopcode_slot = spec
        .subdispatch_table_runtime
        .checked_add(0x12 * 4)
        .ok_or_else(|| error("DRAC subdispatch slot 주소 overflow"))?;
    let subopcode_handler = read_exe_word(map, text, subopcode_slot, "DRAC OP28 sub12 slot")?;
    if subopcode_handler != spec.cell_setup_handler_runtime {
        return Err(error(format!(
            "{} DRAC OP28/sub12 handler 불일치: expected=0x{:08X}, actual=0x{subopcode_handler:08X}",
            spec.exe, spec.cell_setup_handler_runtime
        )));
    }

    let handler = spec.cell_setup_handler_runtime;
    let (root_high, root_low) = high_low(spec.descriptor_root_runtime);
    let selector_checks = [
        (
            handler + 0x1C,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S3,
                offset: 0x1C,
            },
        ),
        (
            handler + 0x24,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 4,
            },
        ),
        (
            handler + 0x30,
            K::Lui {
                rt: Reg::V0,
                imm: root_high,
            },
        ),
        (
            handler + 0x34,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 3,
            },
        ),
        (
            handler + 0x38,
            K::Lhu {
                rt: Reg::A0,
                base: Reg::A0,
                offset: 4,
            },
        ),
        (
            handler + 0x40,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: root_low,
            },
        ),
        (
            handler + 0x50,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
        ),
        (
            handler + 0x54,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            handler + 0x58,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A0,
                sa: 3,
            },
        ),
        (
            handler + 0x5C,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            handler + 0x60,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            handler + 0x64,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            handler + 0x68,
            K::Addu {
                rd: Reg::S1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
    ];
    for (address, expected) in selector_checks {
        require(instructions, spec, address, expected)?;
    }

    let flag_check_runtime = spec
        .cell_table_pointer_load_runtime
        .checked_sub(0x14)
        .ok_or_else(|| error("DRAC cell table flag check 주소 underflow"))?;
    let current_record_runtime = spec
        .cell_table_pointer_load_runtime
        .checked_sub(8)
        .ok_or_else(|| error("DRAC cell table record load 주소 underflow"))?;
    let count_record_runtime = spec
        .cell_count_load_runtime
        .checked_sub(8)
        .ok_or_else(|| error("DRAC cell count record load 주소 underflow"))?;
    let pointer_checks = [
        (
            flag_check_runtime,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::S2,
                imm: 4,
            },
        ),
        (
            current_record_runtime,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S3,
                offset: 0x1C,
            },
        ),
        (
            spec.cell_table_pointer_load_runtime,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0x0C,
            },
        ),
        (
            spec.descriptor_cell_table_store_runtime,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            count_record_runtime,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S3,
                offset: 0x1C,
            },
        ),
        (
            spec.cell_count_load_runtime,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 6,
            },
        ),
        (
            spec.descriptor_cell_count_store_runtime,
            K::Sh {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x20,
            },
        ),
    ];
    for (address, expected) in pointer_checks {
        require(instructions, spec, address, expected)?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DracCellLayout {
    pub(super) setup_record_runtime: u32,
    pub(super) table_prefix_sha256: String,
}

pub(super) fn verify_cell_setup_record_and_layout(
    raw: &[u8],
    spec: SubgameSeqDracGlyphSpec,
) -> Result<DracCellLayout, SubgameSeqDracGlyphError> {
    let raw_len = u32::try_from(raw.len()).map_err(|_| error("DRAC SEQ 길이가 u32를 넘음"))?;
    let source_map = RuntimeAssetMap::new(SUBGAME_SEQ_LOAD_DESTINATION, raw_len)
        .ok_or_else(|| error("DRAC SEQ runtime 사상 overflow"))?;
    let setup_record_runtime = source_map
        .runtime_addr(spec.cell_setup_record_offset)
        .ok_or_else(|| error("DRAC cell setup record가 SEQ 밖임"))?;
    let table_runtime = source_map
        .runtime_addr(BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET)
        .ok_or_else(|| error("DRAC cell table이 SEQ 밖임"))?;
    if table_runtime != BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_RUNTIME {
        return Err(error(format!(
            "{} DRAC cell table runtime 불일치: expected=0x{:08X}, actual=0x{table_runtime:08X}",
            spec.scope, BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_RUNTIME
        )));
    }

    let record = raw
        .get(spec.cell_setup_record_offset..spec.cell_setup_record_offset + 16)
        .ok_or_else(|| error("DRAC cell setup record 범위 이탈"))?;
    let mut expected_record = [0u8; 16];
    expected_record[0] = 0x28;
    expected_record[1] = 0x12;
    expected_record[2] = 0x97;
    expected_record[4..6].copy_from_slice(&spec.resource_id.to_le_bytes());
    expected_record[12..16].copy_from_slice(&table_runtime.to_le_bytes());
    if record != expected_record {
        return Err(error(format!(
            "{} DRAC cell setup 16B record 불일치",
            spec.scope
        )));
    }

    let header = raw
        .get(
            BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET
                ..BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET + BOX_SUBGAME_SEQ_DRAC_CELL_HEADER_BYTES,
        )
        .ok_or_else(|| error("DRAC cell table header 범위 이탈"))?;
    let header_values = (
        read_u16(header, 0)?,
        read_u16(header, 2)?,
        read_u16(header, 4)?,
        read_u16(header, 6)?,
        read_i16(header, 8)?,
        read_i16(header, 10)?,
    );
    if header_values
        != (
            0,
            0,
            BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH,
            BOX_SUBGAME_SEQ_DRAC_ATLAS_HEIGHT,
            0,
            0,
        )
    {
        return Err(error(format!(
            "{} DRAC cell table 1024x256 header 불일치: {header_values:?}",
            spec.scope
        )));
    }
    let declared_bytes = usize::from(header_values.2)
        .checked_mul(usize::from(header_values.3))
        .and_then(|pixels| pixels.checked_div(2))
        .ok_or_else(|| error("DRAC atlas byte 계산 overflow"))?;
    if declared_bytes != BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES {
        return Err(error(format!(
            "{} DRAC cell header와 BTX byte 수 불일치",
            spec.scope
        )));
    }

    for cell in BOX_SUBGAME_SEQ_DRAC_CELL_BASE..=BOX_SUBGAME_SEQ_DRAC_CELL_LAST {
        let ordinal = usize::from(cell - BOX_SUBGAME_SEQ_DRAC_CELL_BASE);
        let offset = BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET
            + BOX_SUBGAME_SEQ_DRAC_CELL_HEADER_BYTES
            + usize::from(cell) * BOX_SUBGAME_SEQ_DRAC_CELL_STRIDE as usize;
        let actual = (
            read_u16(raw, offset)?,
            read_u16(raw, offset + 2)?,
            read_u16(raw, offset + 4)?,
            read_u16(raw, offset + 6)?,
            read_i16(raw, offset + 8)?,
            read_i16(raw, offset + 10)?,
        );
        let expected = (
            BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X
                + u16::try_from(ordinal % BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS)
                    .expect("열은 u16 범위")
                    * BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH,
            BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y
                + u16::try_from(ordinal / BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS)
                    .expect("행은 u16 범위")
                    * BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT,
            BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH,
            BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT,
            0,
            0,
        );
        if actual != expected {
            return Err(error(format!(
                "{} DRAC cell {cell} 사각형 불일치: expected={expected:?}, actual={actual:?}",
                spec.scope
            )));
        }
    }
    if BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS * BOX_SUBGAME_SEQ_DRAC_CELL_GRID_ROWS
        != BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS
    {
        return Err(error("DRAC cell 8x7 격자와 repertoire 수 불일치"));
    }

    let prefix = raw
        .get(BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET..BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_PREFIX_END)
        .ok_or_else(|| error("DRAC cell table prefix 범위 이탈"))?;
    let table_prefix_sha256 = hex(&Sha256::digest(prefix));
    if table_prefix_sha256 != BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_PREFIX_SHA256 {
        return Err(error(format!(
            "{} DRAC cell table prefix SHA-256 불일치: {table_prefix_sha256}",
            spec.scope
        )));
    }
    Ok(DracCellLayout {
        setup_record_runtime,
        table_prefix_sha256,
    })
}
