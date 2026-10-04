//! 새 일본어 양성 BTX의 PP1/PP2 descriptor와 SEQ setup 차이.
//!
//! 같은 이름·같은 픽셀인 자산도 두 EXE에서 같은 setup을 쓴다고 가정하지
//! 않는다. 이 모듈은 `graphics::text_candidates`의 pending 계약을 기존
//! generic scanner에 연결하고, PP2 exact layout과 PP1 상충을 각각 고정한다.
//! 정적 setup은 실행·draw·화면 표시나 block 경계를 증명하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::disasm::BtxNameAnalysis;
use crate::graphics::text_candidates::BOX_PUYO_TEXT_SURFACE_PROVENANCE;
use crate::script::SubgameSeqLayoutAnalysis;

use super::btx_pending_static_cells::{
    audit_btx_static_cells_for_specs, BtxPendingStaticCellAnalysis, BtxPendingStaticCellError,
    BtxPendingStaticCellRow, BtxStaticCellAssetSpec,
};

const EXPECTED_FINDINGS: usize = 6;
const EXPECTED_PHYSICAL_ASSETS: usize = 10;
const EXPECTED_SETUP_RECORDS: usize = 10;
const EXPECTED_ROWS_WITH_SETUP: usize = 9;
const EXPECTED_EXACT_LAYOUTS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SetupExpectation {
    sequence: &'static str,
    record_offset: usize,
    table_offset: usize,
    atlas_width: u16,
    atlas_height: u16,
    exact_layout: bool,
}

#[derive(Debug, Clone, Copy)]
struct RowExpectation {
    finding_id: &'static str,
    exe: &'static str,
    asset_name: &'static str,
    resource_id: u16,
    plain_bytes: u32,
    setups: &'static [SetupExpectation],
}

const MM_PUYO1_SETUPS: [SetupExpectation; 0] = [];
const MM_PUYO2_SETUPS: [SetupExpectation; 1] = [SetupExpectation {
    sequence: "MMENU.SEQ",
    record_offset: 0x222C,
    table_offset: 0x142C,
    atlas_width: 128,
    atlas_height: 192,
    exact_layout: false,
}];
const OVER3_PUYO1_SETUPS: [SetupExpectation; 2] = [
    SetupExpectation {
        sequence: "PP1_TTL.SEQ",
        record_offset: 0x0818,
        table_offset: 0x0930,
        atlas_width: 256,
        atlas_height: 192,
        exact_layout: true,
    },
    SetupExpectation {
        sequence: "PP2_TTL.SEQ",
        record_offset: 0x0818,
        table_offset: 0x0930,
        atlas_width: 256,
        atlas_height: 192,
        exact_layout: true,
    },
];
const OVER3_PUYO2_SETUPS: [SetupExpectation; 1] = [SetupExpectation {
    sequence: "PP1_TTL.SEQ",
    record_offset: 0x0818,
    table_offset: 0x0930,
    atlas_width: 256,
    atlas_height: 192,
    exact_layout: true,
}];
const END10_PUYO1_SETUPS: [SetupExpectation; 1] = [SetupExpectation {
    sequence: "SSCO_SEQ.SEQ",
    record_offset: 0x0264,
    table_offset: 0x0008,
    atlas_width: 256,
    atlas_height: 176,
    exact_layout: false,
}];
const END10_PUYO2_SETUPS: [SetupExpectation; 1] = [SetupExpectation {
    sequence: "PP2_TTL.SEQ",
    record_offset: 0x358C,
    table_offset: 0x3AA8,
    atlas_width: 256,
    atlas_height: 160,
    exact_layout: true,
}];
const END15_PUYO1_SETUPS: [SetupExpectation; 1] = [SetupExpectation {
    sequence: "SWIL_SEQ.SEQ",
    record_offset: 0x0264,
    table_offset: 0x0008,
    atlas_width: 256,
    atlas_height: 176,
    exact_layout: false,
}];
const END15_PUYO2_SETUPS: [SetupExpectation; 1] = [SetupExpectation {
    sequence: "PP2_TTL.SEQ",
    record_offset: 0x4570,
    table_offset: 0x408C,
    atlas_width: 256,
    atlas_height: 224,
    exact_layout: true,
}];
const END16_PUYO1_SETUPS: [SetupExpectation; 1] = [SetupExpectation {
    sequence: "SWIT_SEQ.SEQ",
    record_offset: 0x0270,
    table_offset: 0x0008,
    atlas_width: 256,
    atlas_height: 176,
    exact_layout: false,
}];
const END16_PUYO2_SETUPS: [SetupExpectation; 1] = [SetupExpectation {
    sequence: "PP2_TTL.SEQ",
    record_offset: 0x45A0,
    table_offset: 0x40C0,
    atlas_width: 256,
    atlas_height: 256,
    exact_layout: true,
}];

const OVER3_TEXT_CELLS: [(u16, u16, u16, u16, i16, i16); 12] = [
    (0, 0, 48, 56, 0, 0),
    (48, 0, 48, 56, 0, 0),
    (96, 0, 48, 56, 0, 0),
    (144, 0, 48, 56, 0, 0),
    (192, 0, 48, 56, 0, 0),
    (0, 56, 48, 56, 0, 0),
    (48, 56, 48, 56, 0, 0),
    (96, 56, 48, 56, 0, 0),
    (144, 56, 48, 56, 0, 0),
    (192, 56, 48, 56, 0, 0),
    (0, 112, 64, 56, 8, 0),
    (0, 168, 192, 16, 0, 0),
];
const END15_CELLS: [(u16, u16, u16, u16, i16, i16); 3] = [
    (0, 0, 40, 96, 0, 0),
    (0, 96, 40, 128, 0, 0),
    (40, 0, 216, 224, 0, 0),
];
const END10_CELLS: [(u16, u16, u16, u16, i16, i16); 17] = [
    (0, 0, 72, 16, 0, 0),
    (72, 0, 80, 16, 0, 0),
    (152, 0, 56, 16, 0, 0),
    (0, 16, 120, 16, 0, 0),
    (0, 32, 144, 16, 0, 0),
    (0, 48, 120, 16, 0, 0),
    (0, 64, 96, 16, 0, 0),
    (96, 64, 96, 16, 0, 0),
    (0, 80, 80, 16, 0, 0),
    (80, 80, 96, 16, 0, 0),
    (0, 96, 152, 16, 0, 0),
    (0, 112, 136, 16, 0, 0),
    (136, 112, 80, 16, 0, 0),
    (0, 128, 56, 16, 0, 0),
    (56, 128, 64, 16, 0, 0),
    (120, 128, 120, 16, 0, 0),
    (0, 144, 144, 16, 0, 0),
];
const END16_CELLS: [(u16, u16, u16, u16, i16, i16); 4] = [
    (0, 0, 160, 128, 0, 0),
    (0, 128, 160, 128, 0, 0),
    (160, 0, 88, 96, 0, 0),
    (160, 96, 96, 96, 0, 0),
];

const ROW_EXPECTATIONS: [RowExpectation; EXPECTED_PHYSICAL_ASSETS] = [
    RowExpectation {
        finding_id: "BTXF-029",
        exe: "PUYO1.EXE",
        asset_name: "MM_MES.BTX",
        resource_id: 172,
        plain_bytes: 65_536,
        setups: &MM_PUYO1_SETUPS,
    },
    RowExpectation {
        finding_id: "BTXF-030",
        exe: "PUYO2.EXE",
        asset_name: "MM_MES.BTX",
        resource_id: 165,
        plain_bytes: 65_536,
        setups: &MM_PUYO2_SETUPS,
    },
    RowExpectation {
        finding_id: "BTXF-031",
        exe: "PUYO1.EXE",
        asset_name: "P1_OVER3.BTX",
        resource_id: 82,
        plain_bytes: 24_576,
        setups: &OVER3_PUYO1_SETUPS,
    },
    RowExpectation {
        finding_id: "BTXF-031",
        exe: "PUYO2.EXE",
        asset_name: "P1_OVER3.BTX",
        resource_id: 33,
        plain_bytes: 24_576,
        setups: &OVER3_PUYO2_SETUPS,
    },
    RowExpectation {
        finding_id: "BTXF-032",
        exe: "PUYO1.EXE",
        asset_name: "P2_END10.BTX",
        resource_id: 158,
        plain_bytes: 20_480,
        setups: &END10_PUYO1_SETUPS,
    },
    RowExpectation {
        finding_id: "BTXF-032",
        exe: "PUYO2.EXE",
        asset_name: "P2_END10.BTX",
        resource_id: 71,
        plain_bytes: 20_480,
        setups: &END10_PUYO2_SETUPS,
    },
    RowExpectation {
        finding_id: "BTXF-033",
        exe: "PUYO1.EXE",
        asset_name: "P2_END15.BTX",
        resource_id: 163,
        plain_bytes: 28_672,
        setups: &END15_PUYO1_SETUPS,
    },
    RowExpectation {
        finding_id: "BTXF-033",
        exe: "PUYO2.EXE",
        asset_name: "P2_END15.BTX",
        resource_id: 76,
        plain_bytes: 28_672,
        setups: &END15_PUYO2_SETUPS,
    },
    RowExpectation {
        finding_id: "BTXF-034",
        exe: "PUYO1.EXE",
        asset_name: "P2_END16.BTX",
        resource_id: 164,
        plain_bytes: 32_768,
        setups: &END16_PUYO1_SETUPS,
    },
    RowExpectation {
        finding_id: "BTXF-034",
        exe: "PUYO2.EXE",
        asset_name: "P2_END16.BTX",
        resource_id: 77,
        plain_bytes: 32_768,
        setups: &END16_PUYO2_SETUPS,
    },
];

#[derive(Debug)]
pub enum BtxTextSurfaceCellError {
    Static(BtxPendingStaticCellError),
    Contract(String),
}

impl fmt::Display for BtxTextSurfaceCellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static(source) => write!(f, "BTX text surface static 분석 실패: {source}"),
            Self::Contract(message) => f.write_str(message),
        }
    }
}

impl Error for BtxTextSurfaceCellError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Static(source) => Some(source),
            Self::Contract(_) => None,
        }
    }
}

impl From<BtxPendingStaticCellError> for BtxTextSurfaceCellError {
    fn from(source: BtxPendingStaticCellError) -> Self {
        Self::Static(source)
    }
}

fn error(message: impl Into<String>) -> BtxTextSurfaceCellError {
    BtxTextSurfaceCellError::Contract(message.into())
}

fn specs() -> Vec<BtxStaticCellAssetSpec<'static>> {
    BOX_PUYO_TEXT_SURFACE_PROVENANCE
        .iter()
        .map(|spec| BtxStaticCellAssetSpec {
            finding_id: spec.finding_id,
            archive_paths: spec.archive_paths,
            name: spec.name,
            review_layout: spec.review_layout,
        })
        .collect()
}

fn exact_row(
    analysis: &BtxPendingStaticCellAnalysis,
    expectation: RowExpectation,
) -> Result<&BtxPendingStaticCellRow, BtxTextSurfaceCellError> {
    let rows = analysis
        .rows
        .iter()
        .filter(|row| row.finding_id == expectation.finding_id && row.exe == expectation.exe)
        .collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        return Err(error(format!(
            "{} {} static 행 수가 1이 아님: {}",
            expectation.finding_id,
            expectation.exe,
            rows.len()
        )));
    };
    Ok(*row)
}

fn verify_row(
    row: &BtxPendingStaticCellRow,
    expectation: RowExpectation,
) -> Result<(), BtxTextSurfaceCellError> {
    if row.asset_name != expectation.asset_name
        || row.resource_id != expectation.resource_id
        || row.descriptor_plain_bytes != expectation.plain_bytes
        || row.initial_cell_table_runtime != 0
        || row.initial_cell_count != 0
        || row.setups.len() != expectation.setups.len()
    {
        return Err(error(format!(
            "{} {} descriptor/setup 계약 불일치: {row:?}",
            expectation.finding_id, expectation.exe
        )));
    }
    for (actual, expected) in row.setups.iter().zip(expectation.setups) {
        if actual.sequence != expected.sequence
            || actual.record_offset != expected.record_offset
            || actual.table_offset != expected.table_offset
            || actual.atlas_width != expected.atlas_width
            || actual.atlas_height != expected.atlas_height
            || actual.review_layout_matches != expected.exact_layout
            || actual.descriptor_plain_layout_matches != expected.exact_layout
        {
            return Err(error(format!(
                "{} {} setup 계약 불일치: expected={expected:?}, actual={actual:?}",
                expectation.finding_id, expectation.exe
            )));
        }
    }
    Ok(())
}

fn verify_inference_boundaries(
    analysis: &BtxPendingStaticCellAnalysis,
) -> Result<(), BtxTextSurfaceCellError> {
    for expectation in [ROW_EXPECTATIONS[2], ROW_EXPECTATIONS[3]] {
        let row = exact_row(analysis, expectation)?;
        for setup in &row.setups {
            if setup.inferred_cell_records.is_some() || !setup.inferred_cells.is_empty() {
                return Err(error(format!(
                    "{} P1_OVER3 full-span cell 추론을 허용하지 않음: {setup:?}",
                    row.exe
                )));
            }
            let prefix = setup.leading_cell_prefix.as_ref().ok_or_else(|| {
                error(format!("{} P1_OVER3 cell prefix 누락: {setup:?}", row.exe))
            })?;
            if prefix.valid_cells.len() != OVER3_TEXT_CELLS.len()
                || prefix.protected_tail_bytes != 320
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
                ) != (12, 0, 0, 0, 32768, -14659, -32611, true, false)
            {
                return Err(error(format!(
                    "{} P1_OVER3 cell prefix 경계 불일치: {prefix:?}",
                    row.exe
                )));
            }
            for (index, (cell, expected)) in
                prefix.valid_cells.iter().zip(OVER3_TEXT_CELLS).enumerate()
            {
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
                    index, expected.0, expected.1, expected.2, expected.3, expected.4, expected.5,
                    false, false,
                ) {
                    return Err(error(format!(
                        "{} P1_OVER3 text cell {index} 불일치: {cell:?}",
                        row.exe
                    )));
                }
            }
        }
    }
    let mm = exact_row(analysis, ROW_EXPECTATIONS[1])?.setups[0].clone();
    if (
        mm.inferred_cell_records,
        mm.valid_cells,
        mm.empty_cells,
        mm.outside_cells,
    ) != (Some(49), Some(6), Some(1), Some(42))
    {
        return Err(error(format!("PUYO2 MM_MES 상충 span 진단 불일치: {mm:?}")));
    }
    let end10 = exact_row(analysis, ROW_EXPECTATIONS[5])?.setups[0].clone();
    if (
        end10.inferred_cell_records,
        end10.table_padding_bytes,
        end10.valid_cells,
        end10.empty_cells,
        end10.outside_cells,
    ) != (Some(17), Some(4), Some(17), Some(0), Some(0))
    {
        return Err(error(format!("PUYO2 P2_END10 cell span 불일치: {end10:?}")));
    }
    for (index, (cell, expected)) in end10.inferred_cells.iter().zip(END10_CELLS).enumerate() {
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
            index, expected.0, expected.1, expected.2, expected.3, expected.4, expected.5, false,
            false,
        ) {
            return Err(error(format!(
                "PUYO2 P2_END10 cell {index} 불일치: {cell:?}"
            )));
        }
    }
    let end15 = exact_row(analysis, ROW_EXPECTATIONS[7])?.setups[0].clone();
    if (
        end15.inferred_cell_records,
        end15.table_padding_bytes,
        end15.valid_cells,
        end15.empty_cells,
        end15.outside_cells,
    ) != (Some(3), Some(4), Some(3), Some(0), Some(0))
    {
        return Err(error(format!("PUYO2 P2_END15 cell span 불일치: {end15:?}")));
    }
    for (index, (cell, expected)) in end15.inferred_cells.iter().zip(END15_CELLS).enumerate() {
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
            index, expected.0, expected.1, expected.2, expected.3, expected.4, expected.5, false,
            false,
        ) {
            return Err(error(format!(
                "PUYO2 P2_END15 cell {index} 불일치: {cell:?}"
            )));
        }
    }
    let end16 = exact_row(analysis, ROW_EXPECTATIONS[9])?.setups[0].clone();
    if end16.next_referenced_table_offset.is_some()
        || end16.next_referenced_overlay_offset != Some(0x4100)
        || end16.inferred_cell_records.is_some()
        || !end16.inferred_cells.is_empty()
        || end16.leading_cell_prefix.is_some()
    {
        return Err(error(format!("PUYO2 P2_END16 span 경계 불일치: {end16:?}")));
    }
    let span = end16
        .next_overlay_cell_span
        .as_ref()
        .ok_or_else(|| error(format!("PUYO2 P2_END16 overlay cell span 누락: {end16:?}")))?;
    if (
        span.records,
        span.padding_bytes,
        span.valid_cells,
        span.empty_cells,
        span.outside_cells,
    ) != (4, 4, 4, 0, 0)
    {
        return Err(error(format!(
            "PUYO2 P2_END16 overlay cell span 불일치: {span:?}"
        )));
    }
    for (index, (cell, expected)) in span.cells.iter().zip(END16_CELLS).enumerate() {
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
            index, expected.0, expected.1, expected.2, expected.3, expected.4, expected.5, false,
            false,
        ) {
            return Err(error(format!(
                "PUYO2 P2_END16 cell {index} 불일치: {cell:?}"
            )));
        }
    }
    Ok(())
}

/// 지원 원본의 새 PP1/PP2 일본어 양성 표면과 setup 상충을 검증한다.
pub fn verify_box_btx_text_surface_cells<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    layouts: &SubgameSeqLayoutAnalysis,
    names: &BtxNameAnalysis,
) -> Result<BtxPendingStaticCellAnalysis, BtxTextSurfaceCellError> {
    let analysis =
        audit_btx_static_cells_for_specs(&specs(), executables, sources, layouts, names)?;
    for (field, expected, actual) in [
        ("finding", EXPECTED_FINDINGS, analysis.findings),
        (
            "physical asset",
            EXPECTED_PHYSICAL_ASSETS,
            analysis.physical_assets,
        ),
        (
            "setup record",
            EXPECTED_SETUP_RECORDS,
            analysis.setup_records,
        ),
        (
            "setup 보유 physical row",
            EXPECTED_ROWS_WITH_SETUP,
            analysis.rows_with_setup,
        ),
        (
            "exact review layout",
            EXPECTED_EXACT_LAYOUTS,
            analysis.layout_matching_setup_records,
        ),
        (
            "exact plain layout",
            EXPECTED_EXACT_LAYOUTS,
            analysis.plain_layout_matching_setup_records,
        ),
    ] {
        if actual != expected {
            return Err(error(format!(
                "BTX text surface {field} 분모 불일치: expected={expected}, actual={actual}"
            )));
        }
    }
    for expectation in ROW_EXPECTATIONS {
        verify_row(exact_row(&analysis, expectation)?, expectation)?;
    }
    verify_inference_boundaries(&analysis)?;
    Ok(analysis)
}

#[cfg(test)]
#[path = "btx_text_surface_cells_tests.rs"]
mod tests;
