//! PUYO1/PUYO2 `MM_MES.BTX`의 EXE별 정적 경계.
//!
//! 두 EXE를 합치지 않는다. PUYO1에는 canonical `OP28/sub12` setup이 없고,
//! PUYO2 `MMENU.SEQ`에는 resource 165 setup과 6-cell 선두 prefix가 있다.
//! 뒤의 508바이트는 다른 구조이므로 cell table로 해석하지 않는다. 이
//! source 계약은 setup 실행, draw, 실제 화면 표시를 증명하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{decode, InstructionKind as K, Reg};

use super::btx_pending_static_cells::{
    BtxPendingStaticCellAnalysis, BtxPendingStaticCellPrefix, BtxPendingStaticCellRow,
};

const PP1_DESCRIPTOR_RUNTIME: u32 = 0x8006_9678;
const PP2_DESCRIPTOR_RUNTIME: u32 = 0x8006_55D0;
const PP1_RESOURCE_ID: u16 = 172;
const PP2_RESOURCE_ID: u16 = 165;
const PP1_STORED_BYTES: u32 = 43_649;
const PP2_STORED_BYTES: u32 = 43_285;
const PLAIN_BYTES: u32 = 65_536;

const PP2_SETUP_RECORD_RUNTIME: u32 = 0x801A_849C;
const PP2_TABLE_RUNTIME: u32 = 0x801A_769C;
const PP2_NEXT_TABLE_OFFSET: usize = 0x1688;
const PP2_HANDLER_RUNTIME: u32 = 0x8001_9930;
const PP2_OPCODE_28_SLOT_RUNTIME: u32 = 0x8008_B7E0;
const PP2_OPCODE_28_WRAPPER_RUNTIME: u32 = 0x8002_3364;
const PP2_SUBDISPATCHER_RUNTIME: u32 = 0x8001_B184;
const PP2_SUBDISPATCH_TABLE_RUNTIME: u32 = 0x8008_B59C;

/// 지원 원본에서 닫힌 `MM_MES`의 EXE별 정적 owner 요약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmMesStaticOwner {
    pub pp1_resource_id: u16,
    pub pp1_descriptor_runtime: u32,
    pub pp1_stored_bytes: u32,
    pub pp1_setup_records: usize,
    pub pp2_resource_id: u16,
    pub pp2_descriptor_runtime: u32,
    pub pp2_stored_bytes: u32,
    pub pp2_setup_record_runtime: u32,
    pub pp2_table_runtime: u32,
    pub pp2_table_width: u16,
    pub pp2_table_height: u16,
    pub pp2_record_plus_6: u16,
    pub pp2_valid_prefix_cells: usize,
    pub pp2_protected_tail_bytes: usize,
    pub pp2_handler_runtime: u32,
}

/// `MM_MES` EXE별 정적 source gate 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmMesStaticError(String);

impl fmt::Display for MmMesStaticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for MmMesStaticError {}

fn error(message: impl Into<String>) -> MmMesStaticError {
    MmMesStaticError(message.into())
}

fn exact_row<'a>(
    analysis: &'a BtxPendingStaticCellAnalysis,
    finding_id: &str,
    exe: &str,
) -> Result<&'a BtxPendingStaticCellRow, MmMesStaticError> {
    let rows = analysis
        .rows
        .iter()
        .filter(|row| row.finding_id == finding_id && row.exe == exe)
        .collect::<Vec<_>>();
    match rows.as_slice() {
        [row] => Ok(*row),
        _ => Err(error(format!(
            "{finding_id}:{exe} MM_MES static 행 수가 1이 아님: {}",
            rows.len()
        ))),
    }
}

fn executable_slice<'a>(
    map: &ExeMap,
    text: &'a [u8],
    runtime: u32,
    len: usize,
) -> Result<&'a [u8], MmMesStaticError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "PUYO2 MM_MES runtime 주소가 EXE text 밖임: 0x{runtime:08X}"
        ))
    })?;
    text.get(offset..offset + len).ok_or_else(|| {
        error(format!(
            "PUYO2 MM_MES EXE text 범위 이탈: 0x{runtime:08X}+{len}"
        ))
    })
}

fn read_u32(raw: &[u8]) -> u32 {
    u32::from_le_bytes(raw[..4].try_into().expect("4바이트 word"))
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), MmMesStaticError> {
    let actual = decode(runtime, read_u32(executable_slice(map, text, runtime, 4)?));
    if actual.kind != expected {
        return Err(error(format!(
            "PUYO2 MM_MES typed 명령 불일치: address=0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn verify_pp2_prefix(prefix: &BtxPendingStaticCellPrefix) -> Result<(), MmMesStaticError> {
    let expected = [
        (0, 0, 64, 48),
        (0, 48, 64, 48),
        (0, 144, 80, 48),
        (64, 0, 64, 48),
        (64, 48, 64, 48),
        (0, 96, 64, 48),
    ];
    if prefix.protected_tail_bytes != 508
        || prefix.valid_cells.len() != expected.len()
        || (
            prefix.boundary_record.index,
            prefix.boundary_record.x,
            prefix.boundary_record.y,
            prefix.boundary_record.width,
            prefix.boundary_record.height,
            prefix.boundary_record.anchor_x,
            prefix.boundary_record.anchor_y,
            prefix.boundary_record.empty,
            prefix.boundary_record.outside_atlas,
        ) != (6, 0, 0, 0, 0x8000, -32768, -32736, true, false)
    {
        return Err(error(format!(
            "PUYO2 MM_MES cell prefix 경계 불일치: {prefix:?}"
        )));
    }
    for (index, (cell, expected)) in prefix.valid_cells.iter().zip(expected).enumerate() {
        if (
            cell.index,
            cell.x,
            cell.y,
            cell.width,
            cell.height,
            cell.anchor_x,
            cell.anchor_y,
            cell.empty,
            cell.outside_atlas,
        ) != (
            index, expected.0, expected.1, expected.2, expected.3, 0, 0, false, false,
        ) {
            return Err(error(format!("PUYO2 MM_MES cell {index} 불일치: {cell:?}")));
        }
    }
    Ok(())
}

fn verify_pp2_handler(map: &ExeMap, text: &[u8]) -> Result<(), MmMesStaticError> {
    if read_u32(executable_slice(map, text, PP2_OPCODE_28_SLOT_RUNTIME, 4)?)
        != PP2_OPCODE_28_WRAPPER_RUNTIME
    {
        return Err(error("PUYO2 OP28 wrapper pointer 불일치"));
    }
    require_instruction(
        map,
        text,
        PP2_OPCODE_28_WRAPPER_RUNTIME + 8,
        K::Jal {
            target: PP2_SUBDISPATCHER_RUNTIME,
        },
    )?;
    let sub12_slot = PP2_SUBDISPATCH_TABLE_RUNTIME + 0x12 * 4;
    if read_u32(executable_slice(map, text, sub12_slot, 4)?) != PP2_HANDLER_RUNTIME {
        return Err(error("PUYO2 OP28/sub12 handler pointer 불일치"));
    }

    for (runtime, expected) in [
        (
            0x8001_9BEC,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S3,
                offset: 0x1C,
            },
        ),
        (
            0x8001_9BF4,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0x0C,
            },
        ),
        (
            0x8001_9BFC,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            0x8001_9C00,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S3,
                offset: 0x1C,
            },
        ),
        (
            0x8001_9C08,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 6,
            },
        ),
        (
            0x8001_9C10,
            K::Sh {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x20,
            },
        ),
    ] {
        require_instruction(map, text, runtime, expected)?;
    }
    Ok(())
}

/// generic 분모 결과에서 두 `MM_MES`만 꺼내 EXE별 정적 계약으로 닫는다.
pub fn verify_box_mm_mes_static_owner<'a>(
    analysis: &BtxPendingStaticCellAnalysis,
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
) -> Result<MmMesStaticOwner, MmMesStaticError> {
    let pp1 = exact_row(analysis, "BTXF-029", "PUYO1.EXE")?;
    let pp2 = exact_row(analysis, "BTXF-030", "PUYO2.EXE")?;
    if pp1.resource_id != PP1_RESOURCE_ID
        || pp1.descriptor_runtime != PP1_DESCRIPTOR_RUNTIME
        || pp1.descriptor_stored_bytes != PP1_STORED_BYTES
        || pp1.descriptor_plain_bytes != PLAIN_BYTES
        || pp1.initial_cell_table_runtime != 0
        || pp1.initial_cell_count != 0
        || !pp1.setups.is_empty()
    {
        return Err(error(format!("PUYO1 MM_MES owner 계약 불일치: {pp1:?}")));
    }
    if pp2.resource_id != PP2_RESOURCE_ID
        || pp2.descriptor_runtime != PP2_DESCRIPTOR_RUNTIME
        || pp2.descriptor_stored_bytes != PP2_STORED_BYTES
        || pp2.descriptor_plain_bytes != PLAIN_BYTES
        || pp2.initial_cell_table_runtime != 0
        || pp2.initial_cell_count != 0
        || pp2.setups.len() != 1
    {
        return Err(error(format!("PUYO2 MM_MES owner 계약 불일치: {pp2:?}")));
    }
    let setup = &pp2.setups[0];
    if setup.sequence != "MMENU.SEQ"
        || setup.record_runtime != PP2_SETUP_RECORD_RUNTIME
        || setup.table_runtime != PP2_TABLE_RUNTIME
        || setup.record_cell_count != 1
        || setup.atlas_width != 128
        || setup.atlas_height != 192
        || setup.review_layout_matches
        || setup.descriptor_plain_layout_matches
        || setup.next_referenced_table_offset != Some(PP2_NEXT_TABLE_OFFSET)
        || (
            setup.inferred_cell_records,
            setup.valid_cells,
            setup.empty_cells,
            setup.outside_cells,
        ) != (Some(49), Some(6), Some(1), Some(42))
    {
        return Err(error(format!("PUYO2 MM_MES setup 계약 불일치: {setup:?}")));
    }
    let prefix = setup
        .leading_cell_prefix
        .as_ref()
        .ok_or_else(|| error("PUYO2 MM_MES 보수적 cell prefix 누락"))?;
    verify_pp2_prefix(prefix)?;

    let inputs = executables
        .into_iter()
        .filter(|(map, _)| map.name == "PUYO2.EXE")
        .collect::<Vec<_>>();
    let [(map, text)] = inputs.as_slice() else {
        return Err(error(format!(
            "PUYO2 EXE 입력 수가 1이 아님: {}",
            inputs.len()
        )));
    };
    verify_pp2_handler(map, text)?;

    Ok(MmMesStaticOwner {
        pp1_resource_id: pp1.resource_id,
        pp1_descriptor_runtime: pp1.descriptor_runtime,
        pp1_stored_bytes: pp1.descriptor_stored_bytes,
        pp1_setup_records: pp1.setups.len(),
        pp2_resource_id: pp2.resource_id,
        pp2_descriptor_runtime: pp2.descriptor_runtime,
        pp2_stored_bytes: pp2.descriptor_stored_bytes,
        pp2_setup_record_runtime: setup.record_runtime,
        pp2_table_runtime: setup.table_runtime,
        pp2_table_width: setup.atlas_width,
        pp2_table_height: setup.atlas_height,
        pp2_record_plus_6: setup.record_cell_count,
        pp2_valid_prefix_cells: prefix.valid_cells.len(),
        pp2_protected_tail_bytes: prefix.protected_tail_bytes,
        pp2_handler_runtime: PP2_HANDLER_RUNTIME,
    })
}

#[cfg(test)]
#[path = "mm_mes_static_tests.rs"]
mod tests;
