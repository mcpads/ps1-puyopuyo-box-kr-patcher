//! 사람 시각 양성 BTX family의 EXE embedded cell setup을 원본에 결속한다.
//!
//! descriptor block 초기화나 정적 setup record는 실제 실행·upload·draw가
//! 아니다. 이 모듈은 stable `family_key`에서 물리 사본과 자기 EXE를 다시
//! 찾고, typed descriptor 선택 경계와 embedded `OP28/sub12` setup을
//! 연결해 다음 runtime 조사 범위를 줄인다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::btx::PixelFormat;
use crate::coords::ExeMap;
use crate::disasm::{BtxDescriptorSelectionAnalysis, BtxNameAnalysis, BtxNameLink};
use crate::graphics::btx_family_candidates::{
    BtxFamilyCandidate, BtxFamilyCandidateAnalysis, BtxPhysicalCandidateCopy,
};
use crate::review::{BtxFamilyRoleAnalysis, BtxFamilyRoleBasis};

use super::btx_pending_static_cells::{
    descriptor_entry_runtime, resource_id, scan_setup_records, verify_initial_descriptor,
    BtxPendingStaticCellError, BtxPendingStaticCellRecord, BtxPendingStaticSetup, SetupScanContext,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtxFamilyStaticCellExtentBasis {
    NextReferencedTable,
    NextReferencedPointer,
    LeadingValidPrefix,
}

impl BtxFamilyStaticCellExtentBasis {
    pub const fn label(self) -> &'static str {
        match self {
            Self::NextReferencedTable => "next-referenced-table",
            Self::NextReferencedPointer => "next-referenced-pointer",
            Self::LeadingValidPrefix => "leading-valid-prefix",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyStaticCellSetup {
    pub setup_runtime: u32,
    pub table_runtime: u32,
    pub coordinate_width: u16,
    pub coordinate_height: u16,
    pub extent_basis: BtxFamilyStaticCellExtentBasis,
    pub cells: Vec<BtxPendingStaticCellRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyStaticCellRow {
    pub family_key: String,
    pub exe: &'static str,
    pub archive: String,
    pub asset_name: String,
    /// VDD 엔트리 표의 0-based 위치다.
    pub vdd_entry_index: usize,
    /// 선행 VDD descriptor를 포함하는 GRP descriptor block의 resource ID다.
    pub descriptor_resource_id: u16,
    pub descriptor_runtime: u32,
    pub descriptor_plain_bytes: u32,
    pub descriptor_selection_runtime: u32,
    pub review_width: usize,
    pub review_height: usize,
    pub setups: Vec<BtxFamilyStaticCellSetup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyStaticCellAnalysis {
    pub rows: Vec<BtxFamilyStaticCellRow>,
    pub families: usize,
    pub physical_assets: usize,
    pub setup_records: usize,
    pub rows_with_setup: usize,
    pub cell_records: usize,
    pub full_table_setups: usize,
    pub leading_prefix_setups: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtxFamilyStaticCellError(String);

impl fmt::Display for BtxFamilyStaticCellError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "BTX family embedded cell 감사 실패: {}", self.0)
    }
}

impl Error for BtxFamilyStaticCellError {}

impl From<BtxPendingStaticCellError> for BtxFamilyStaticCellError {
    fn from(source: BtxPendingStaticCellError) -> Self {
        Self(source.to_string())
    }
}

fn error(message: impl Into<String>) -> BtxFamilyStaticCellError {
    BtxFamilyStaticCellError(message.into())
}

fn exact_link<'a>(
    names: &'a BtxNameAnalysis,
    copy: &BtxPhysicalCandidateCopy,
) -> Result<&'a BtxNameLink, BtxFamilyStaticCellError> {
    let links = names
        .links
        .iter()
        .filter(|link| link.archive == copy.archive && link.asset_name == copy.name)
        .collect::<Vec<_>>();
    match links.as_slice() {
        [link] => Ok(*link),
        _ => Err(error(format!(
            "물리 BTX 이름표 연결 수가 1이 아님: {}:{}: {}",
            copy.archive,
            copy.name,
            links.len()
        ))),
    }
}

fn exact_candidate_members<'a>(
    candidates: &'a BtxFamilyCandidateAnalysis,
    family_key: &str,
    expected: usize,
) -> Result<Vec<&'a BtxFamilyCandidate>, BtxFamilyStaticCellError> {
    let members = candidates
        .candidates
        .iter()
        .filter(|candidate| candidate.family_key == family_key)
        .collect::<Vec<_>>();
    if members.len() != expected {
        return Err(error(format!(
            "{family_key} 구성원 수 불일치: role={expected}, candidates={}",
            members.len()
        )));
    }
    Ok(members)
}

fn static_setup_extent(
    setup: BtxPendingStaticSetup,
) -> Result<BtxFamilyStaticCellSetup, BtxFamilyStaticCellError> {
    let (extent_basis, cells) = if setup.outside_cells == Some(0)
        && setup.empty_cells == Some(0)
        && !setup.inferred_cells.is_empty()
    {
        (
            BtxFamilyStaticCellExtentBasis::NextReferencedTable,
            setup.inferred_cells,
        )
    } else if let Some(prefix) = setup.leading_cell_prefix {
        (
            BtxFamilyStaticCellExtentBasis::LeadingValidPrefix,
            prefix.valid_cells,
        )
    } else if let Some(span) = setup.next_overlay_cell_span {
        if span.empty_cells != 0 || span.outside_cells != 0 {
            return Err(error(format!(
                "0x{:08X} 다음 pointer 경계에 invalid cell이 있음: empty={}, outside={}",
                setup.record_runtime, span.empty_cells, span.outside_cells
            )));
        }
        (
            BtxFamilyStaticCellExtentBasis::NextReferencedPointer,
            span.cells,
        )
    } else {
        return Err(error(format!(
            "0x{:08X} setup의 보수적 cell extent를 닫지 못함",
            setup.record_runtime
        )));
    };
    if cells.is_empty() {
        return Err(error(format!(
            "0x{:08X} setup의 valid cell이 없음",
            setup.record_runtime
        )));
    }
    Ok(BtxFamilyStaticCellSetup {
        setup_runtime: setup.record_runtime,
        table_runtime: setup.table_runtime,
        coordinate_width: setup.atlas_width,
        coordinate_height: setup.atlas_height,
        extent_basis,
        cells,
    })
}

fn same_cell_geometry(left: &BtxFamilyStaticCellSetup, right: &BtxFamilyStaticCellSetup) -> bool {
    left.coordinate_width == right.coordinate_width
        && left.coordinate_height == right.coordinate_height
        && left.extent_basis == right.extent_basis
        && left.cells == right.cells
}

/// 사람 시각 후보 family와 EXE embedded setup을 교차한다.
pub fn audit_btx_family_static_cells<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    names: &BtxNameAnalysis,
    selections: &BtxDescriptorSelectionAnalysis,
    candidates: &BtxFamilyCandidateAnalysis,
    roles: &BtxFamilyRoleAnalysis,
) -> Result<BtxFamilyStaticCellAnalysis, BtxFamilyStaticCellError> {
    let executable_map = executables
        .into_iter()
        .map(|(map, text)| (map.name, (map, text)))
        .collect::<BTreeMap<_, _>>();
    let visual_roles = roles
        .rows
        .iter()
        .filter(|row| row.basis == BtxFamilyRoleBasis::HumanVisualCandidate)
        .collect::<Vec<_>>();
    if visual_roles.is_empty() {
        return Err(error("사람 시각 후보 family가 없음"));
    }

    let mut rows = Vec::new();
    for role in visual_roles {
        let layout = role.common_layout.ok_or_else(|| {
            error(format!(
                "{} 사람 시각 후보에 판독 layout이 없음",
                role.family_key
            ))
        })?;
        if layout.format != PixelFormat::Indexed4 {
            return Err(error(format!(
                "{} embedded cell scanner는 indexed4만 지원함",
                role.family_key
            )));
        }
        let members = exact_candidate_members(candidates, &role.family_key, role.unique_contents)?;
        let physical_copies = members
            .iter()
            .map(|candidate| candidate.physical_copies.len())
            .sum::<usize>();
        if physical_copies != role.physical_copies {
            return Err(error(format!(
                "{} 물리 사본 수 불일치: role={}, candidates={physical_copies}",
                role.family_key, role.physical_copies
            )));
        }
        for candidate in members {
            for copy in &candidate.physical_copies {
                let link = exact_link(names, copy)?;
                let &(map, text) = executable_map
                    .get(link.exe)
                    .ok_or_else(|| error(format!("{} EXE 입력 누락", link.exe)))?;
                let matching_selections = selections
                    .selections
                    .iter()
                    .filter(|selection| {
                        selection.exe == link.exe && selection.archive == copy.archive
                    })
                    .collect::<Vec<_>>();
                let [selection] = matching_selections.as_slice() else {
                    return Err(error(format!(
                        "{}:{} descriptor 선택 경계 수가 1이 아님: {}",
                        link.exe,
                        copy.archive,
                        matching_selections.len()
                    )));
                };
                let descriptor_runtime = descriptor_entry_runtime(link)?;
                let descriptor_resource_id = resource_id(link, descriptor_runtime)?;
                let (_, plain_bytes, initial_cell_table, initial_cell_count) =
                    verify_initial_descriptor(map, text, link, descriptor_runtime)?;
                if usize::try_from(plain_bytes).ok() != Some(candidate.decompressed_bytes) {
                    return Err(error(format!(
                        "{}:{} descriptor plain과 후보 원문 크기 불일치: descriptor={plain_bytes}, candidate={}",
                        copy.archive, copy.name, candidate.decompressed_bytes
                    )));
                }
                if initial_cell_table != 0 || initial_cell_count != 0 {
                    return Err(error(format!(
                        "{}:{} 초기 descriptor cell field가 0이 아님: pointer=0x{initial_cell_table:08X}, count={initial_cell_count}",
                        copy.archive, copy.name
                    )));
                }
                let setups = scan_setup_records(
                    link.exe,
                    text,
                    SetupScanContext {
                        initialized_len: text.len(),
                        load_base: map.load_addr,
                        resource_id: descriptor_resource_id,
                        review_width: layout.width,
                        review_height: layout.height,
                        format: layout.format,
                        descriptor_plain_bytes: plain_bytes,
                    },
                )?
                .into_iter()
                .map(static_setup_extent)
                .collect::<Result<Vec<_>, _>>()?;
                rows.push(BtxFamilyStaticCellRow {
                    family_key: role.family_key.clone(),
                    exe: link.exe,
                    archive: copy.archive.clone(),
                    asset_name: copy.name.clone(),
                    vdd_entry_index: copy.resource_index,
                    descriptor_resource_id,
                    descriptor_runtime,
                    descriptor_plain_bytes: plain_bytes,
                    descriptor_selection_runtime: selection.descriptor_select_runtime,
                    review_width: layout.width,
                    review_height: layout.height,
                    setups,
                });
            }
        }
    }
    rows.sort_by(|left, right| {
        left.family_key
            .cmp(&right.family_key)
            .then_with(|| left.exe.cmp(right.exe))
    });
    let family_keys = rows
        .iter()
        .map(|row| row.family_key.as_str())
        .collect::<BTreeSet<_>>();
    let setup_records = rows.iter().map(|row| row.setups.len()).sum();
    let rows_with_setup = rows.iter().filter(|row| !row.setups.is_empty()).count();
    let cell_records = rows
        .iter()
        .flat_map(|row| &row.setups)
        .map(|setup| setup.cells.len())
        .sum();
    let full_table_setups = rows
        .iter()
        .flat_map(|row| &row.setups)
        .filter(|setup| setup.extent_basis == BtxFamilyStaticCellExtentBasis::NextReferencedTable)
        .count();
    let leading_prefix_setups = rows
        .iter()
        .flat_map(|row| &row.setups)
        .filter(|setup| setup.extent_basis == BtxFamilyStaticCellExtentBasis::LeadingValidPrefix)
        .count();
    Ok(BtxFamilyStaticCellAnalysis {
        families: family_keys.len(),
        physical_assets: rows.len(),
        setup_records,
        rows_with_setup,
        cell_records,
        full_table_setups,
        leading_prefix_setups,
        rows,
    })
}

/// 지원 원본의 남은 사람 시각 후보 SP_FONT family와 embedded cell 경계를 검증한다.
pub fn verify_box_btx_family_static_cells<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    names: &BtxNameAnalysis,
    selections: &BtxDescriptorSelectionAnalysis,
    candidates: &BtxFamilyCandidateAnalysis,
    roles: &BtxFamilyRoleAnalysis,
) -> Result<BtxFamilyStaticCellAnalysis, BtxFamilyStaticCellError> {
    let analysis =
        audit_btx_family_static_cells(executables, names, selections, candidates, roles)?;
    for (field, expected, actual) in [
        ("family", 1, analysis.families),
        ("physical asset", 5, analysis.physical_assets),
        ("setup record", 5, analysis.setup_records),
        ("setup 보유 physical row", 5, analysis.rows_with_setup),
        ("cell record", 35, analysis.cell_records),
        ("full table setup", 5, analysis.full_table_setups),
        ("leading prefix setup", 0, analysis.leading_prefix_setups),
    ] {
        if actual != expected {
            return Err(error(format!(
                "지원 원본 {field} 분모 불일치: expected={expected}, actual={actual}"
            )));
        }
    }
    let rows_without_setup = analysis
        .rows
        .iter()
        .filter(|row| row.setups.is_empty())
        .map(|row| (row.family_key.as_str(), row.exe))
        .collect::<Vec<_>>();
    if !rows_without_setup.is_empty() {
        return Err(error(format!(
            "embedded setup 미발견 물리 경계 불일치: {rows_without_setup:?}"
        )));
    }
    if let Some(row) = analysis.rows.iter().find(|row| row.setups.len() != 1) {
        return Err(error(format!(
            "지원 원본 embedded setup이 물리 자산당 하나가 아님: {}:{}: {}",
            row.archive,
            row.asset_name,
            row.setups.len()
        )));
    }
    for family_key in analysis
        .rows
        .iter()
        .map(|row| row.family_key.as_str())
        .collect::<BTreeSet<_>>()
    {
        let family_rows = analysis
            .rows
            .iter()
            .filter(|row| row.family_key == family_key)
            .collect::<Vec<_>>();
        let representative = &family_rows[0].setups[0];
        if let Some(row) = family_rows
            .iter()
            .skip(1)
            .find(|row| !same_cell_geometry(representative, &row.setups[0]))
        {
            return Err(error(format!(
                "{family_key} 물리 사본의 embedded cell geometry가 다름: {}:{}",
                row.archive, row.asset_name
            )));
        }
    }
    let mut family_totals = BTreeMap::<&str, (usize, usize)>::new();
    for row in &analysis.rows {
        let totals = family_totals.entry(&row.family_key).or_default();
        totals.0 += row.setups.len();
        totals.1 += row
            .setups
            .iter()
            .map(|setup| setup.cells.len())
            .sum::<usize>();
    }
    let expected_totals = BTreeMap::from([("subgame:SP_FONT:indexed4", (5, 35))]);
    if family_totals != expected_totals {
        return Err(error(format!(
            "family별 setup/cell 분모 불일치: expected={expected_totals:?}, actual={family_totals:?}"
        )));
    }
    Ok(analysis)
}

#[cfg(test)]
#[path = "btx_family_static_cells_tests.rs"]
mod tests;
