//! `COM_FONT.BTX` 일본어 surface 후보의 literal object writer.
//!
//! object field offset만 전수 검색하면 다른 구조체의 같은 offset까지 섞인다.
//! 이 모듈은 같은 base register에 `COM_FONT.BTX`의 정확한 resource ID와
//! surface 후보 cell을 함께 `sh`하는 bounded literal dataflow만 강한 정적
//! 후보로 승격한다. 계산·table·간접 source와 검색창 밖 writer는 미해결로
//! 보존하며 미사용으로 판정하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::font::BOX_COMMON_FONT_TEXT_SURFACES;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{linear, trace, BtxNameAnalysis, CommonFontDrawSiteAnalysis};

const DESCRIPTOR_NAME_POINTER_OFFSET: u32 = 4;
const DESCRIPTOR_STRIDE: u32 = 36;
const SOURCE_WINDOW_WORDS: usize = 32;
const RESOURCE_PAIR_WINDOW_WORDS: usize = 32;

/// 지원 원본의 분석 결과 digest. surface·writer·resource 결속을 함께 고정한다.
pub const BOX_COMMON_FONT_CELL_SOURCE_SHA256: [u8; 32] = [
    0xD0, 0x97, 0xC2, 0x31, 0x66, 0x32, 0xA2, 0xE6, 0x18, 0x0C, 0x55, 0x8F, 0x32, 0xA5, 0xC6, 0x99,
    0x21, 0x85, 0xE6, 0xF0, 0xA8, 0xF4, 0xF8, 0xA7, 0x30, 0xA2, 0x90, 0xB0, 0x45, 0x62, 0x90, 0xDE,
];

/// 일본어 surface 하나와 bounded object writer의 관계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontCellSourceRow {
    pub surface_id: &'static str,
    pub exe: &'static str,
    pub archive: &'static str,
    pub source_text: &'static str,
    pub candidate_cells: Vec<u16>,
    pub common_font_resource_id: u16,
    pub matched_cell: Option<u16>,
    pub cell_store_runtime: Option<u32>,
    pub resource_store_runtimes: Vec<u32>,
    pub entry_reached: bool,
    pub static_verdict: &'static str,
    pub runtime_draw_verified: bool,
}

/// 16개 일본어 surface의 literal source 분석.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontCellSourceAnalysis {
    pub rows: Vec<CommonFontCellSourceRow>,
    pub resolved_surfaces: usize,
    pub unresolved_surfaces: usize,
    pub literal_writer_sites: usize,
    pub entry_reached_writer_sites: usize,
    pub analysis_sha256: [u8; 32],
}

/// 원본·descriptor·dataflow 계약 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommonFontCellSourceError {
    DuplicateExecutable(&'static str),
    MissingExecutable(&'static str),
    TextLengthMismatch {
        exe: &'static str,
        expected: usize,
        actual: usize,
    },
    UnalignedText(&'static str),
    MissingCommonFontLink(&'static str),
    DuplicateCommonFontLink(&'static str),
    DescriptorPointerLayout(&'static str),
    MissingSurface(&'static str),
    AnalysisDigestMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

impl fmt::Display for CommonFontCellSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateExecutable(exe) => {
                write!(f, "COM_FONT cell source EXE 입력 중복: {exe}")
            }
            Self::MissingExecutable(exe) => write!(f, "COM_FONT cell source EXE 입력 누락: {exe}"),
            Self::TextLengthMismatch {
                exe,
                expected,
                actual,
            } => write!(
                f,
                "{exe} COM_FONT cell source text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText(exe) => {
                write!(f, "{exe} COM_FONT cell source text가 4바이트 정렬이 아님")
            }
            Self::MissingCommonFontLink(exe) => {
                write!(f, "{exe} COM_FONT.BTX descriptor link가 없음")
            }
            Self::DuplicateCommonFontLink(exe) => {
                write!(f, "{exe} COM_FONT.BTX descriptor link가 중복됨")
            }
            Self::DescriptorPointerLayout(exe) => write!(
                f,
                "{exe} COM_FONT.BTX descriptor name pointer가 36바이트 격자 +4가 아님"
            ),
            Self::MissingSurface(archive) => {
                write!(f, "{archive} COM_FONT 일본어 surface 원장이 없음")
            }
            Self::AnalysisDigestMismatch { expected, actual } => write!(
                f,
                "COM_FONT cell source digest 불일치: expected={}, actual={}",
                hex_digest(expected),
                hex_digest(actual)
            ),
        }
    }
}

impl Error for CommonFontCellSourceError {}

/// 입력된 EXE에서 bounded literal source를 분석한다.
pub fn analyze_common_font_cell_sources<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    names: &BtxNameAnalysis,
    draw_sites: &CommonFontDrawSiteAnalysis,
) -> Result<CommonFontCellSourceAnalysis, CommonFontCellSourceError> {
    let mut inputs = BTreeMap::new();
    for (map, text) in executables {
        if inputs.insert(map.name, (map, text)).is_some() {
            return Err(CommonFontCellSourceError::DuplicateExecutable(map.name));
        }
    }

    let mut rows = Vec::new();
    for site in &draw_sites.sites {
        let &(map, text) = inputs
            .get(site.exe)
            .ok_or(CommonFontCellSourceError::MissingExecutable(site.exe))?;
        if text.len() != map.text_len as usize {
            return Err(CommonFontCellSourceError::TextLengthMismatch {
                exe: site.exe,
                expected: map.text_len as usize,
                actual: text.len(),
            });
        }
        let instructions =
            linear::sweep(map, text).ok_or(CommonFontCellSourceError::UnalignedText(site.exe))?;
        let reached = trace(map, text, &[]).instructions;
        let resource_id = common_font_resource_id(site.exe, site.archive, names)?;
        let surfaces = BOX_COMMON_FONT_TEXT_SURFACES
            .iter()
            .filter(|surface| surface.archive == site.archive)
            .collect::<Vec<_>>();
        if surfaces.is_empty() {
            return Err(CommonFontCellSourceError::MissingSurface(site.archive));
        }

        let candidate_cells = surfaces
            .iter()
            .flat_map(|surface| {
                surface
                    .representative_runtime_cell_candidates
                    .iter()
                    .copied()
            })
            .collect::<BTreeSet<_>>();
        let mut matched = BTreeSet::new();

        for (index, instruction) in instructions.iter().enumerate() {
            let K::Sh { rt, base, offset } = instruction.kind else {
                continue;
            };
            if offset != site.cell_field_offset {
                continue;
            }
            let Some(cell) = literal_u16_before(&instructions, index, rt) else {
                continue;
            };
            if !candidate_cells.contains(&cell) {
                continue;
            }
            let resource_stores = paired_resource_stores(
                &instructions,
                index,
                base,
                site.resource_field_offset,
                resource_id,
            );
            if resource_stores.is_empty() {
                continue;
            }

            for surface in surfaces.iter().filter(|surface| {
                surface
                    .representative_runtime_cell_candidates
                    .contains(&cell)
            }) {
                matched.insert(surface.surface_id);
                rows.push(CommonFontCellSourceRow {
                    surface_id: surface.surface_id,
                    exe: site.exe,
                    archive: site.archive,
                    source_text: surface.source_text,
                    candidate_cells: surface.representative_runtime_cell_candidates.to_vec(),
                    common_font_resource_id: resource_id,
                    matched_cell: Some(cell),
                    cell_store_runtime: Some(instruction.addr),
                    resource_store_runtimes: resource_stores.clone(),
                    entry_reached: reached.contains_key(&instruction.addr),
                    static_verdict: "bounded-literal-cell-and-com-font-resource-writer",
                    runtime_draw_verified: false,
                });
            }
        }

        for surface in surfaces {
            if matched.contains(surface.surface_id) {
                continue;
            }
            rows.push(CommonFontCellSourceRow {
                surface_id: surface.surface_id,
                exe: site.exe,
                archive: site.archive,
                source_text: surface.source_text,
                candidate_cells: surface.representative_runtime_cell_candidates.to_vec(),
                common_font_resource_id: resource_id,
                matched_cell: None,
                cell_store_runtime: None,
                resource_store_runtimes: Vec::new(),
                entry_reached: false,
                static_verdict: "no-bounded-paired-literal-writer-computed-or-indirect-open",
                runtime_draw_verified: false,
            });
        }
    }

    rows.sort_by_key(|row| (row.exe, row.surface_id, row.cell_store_runtime));
    let resolved_surfaces = rows
        .iter()
        .filter(|row| row.cell_store_runtime.is_some())
        .map(|row| row.surface_id)
        .collect::<BTreeSet<_>>()
        .len();
    let unresolved_surfaces = rows
        .iter()
        .filter(|row| row.cell_store_runtime.is_none())
        .map(|row| row.surface_id)
        .collect::<BTreeSet<_>>()
        .len();
    let literal_writer_sites = rows
        .iter()
        .filter_map(|row| row.cell_store_runtime.map(|runtime| (row.exe, runtime)))
        .collect::<BTreeSet<_>>()
        .len();
    let entry_reached_writer_sites = rows
        .iter()
        .filter(|row| row.entry_reached)
        .filter_map(|row| row.cell_store_runtime.map(|runtime| (row.exe, runtime)))
        .collect::<BTreeSet<_>>()
        .len();
    let analysis_sha256 = digest_rows(&rows);

    Ok(CommonFontCellSourceAnalysis {
        rows,
        resolved_surfaces,
        unresolved_surfaces,
        literal_writer_sites,
        entry_reached_writer_sites,
        analysis_sha256,
    })
}

/// 9개 EXE와 지원 원본 digest를 모두 요구한다.
pub fn verify_box_common_font_cell_sources<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    names: &BtxNameAnalysis,
    draw_sites: &CommonFontDrawSiteAnalysis,
) -> Result<CommonFontCellSourceAnalysis, CommonFontCellSourceError> {
    let analysis = analyze_common_font_cell_sources(executables, names, draw_sites)?;
    for site in &draw_sites.sites {
        if !analysis.rows.iter().any(|row| row.exe == site.exe) {
            return Err(CommonFontCellSourceError::MissingExecutable(site.exe));
        }
    }
    if analysis.analysis_sha256 != BOX_COMMON_FONT_CELL_SOURCE_SHA256 {
        return Err(CommonFontCellSourceError::AnalysisDigestMismatch {
            expected: BOX_COMMON_FONT_CELL_SOURCE_SHA256,
            actual: analysis.analysis_sha256,
        });
    }
    Ok(analysis)
}

/// surface와 writer를 원문 대조 가능한 TSV로 만든다.
pub fn render_common_font_cell_sources_tsv(analysis: &CommonFontCellSourceAnalysis) -> String {
    let mut output = String::from(
        "surface_id\texe\tarchive\tsource_text\tcandidate_cells\tcommon_font_resource_id\t\
         matched_cell\tcell_store_runtime\tresource_store_runtimes\tentry_reached\t\
         static_verdict\truntime_draw_verified\n",
    );
    for row in &analysis.rows {
        let candidate_cells = row
            .candidate_cells
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let matched_cell = row
            .matched_cell
            .map_or_else(String::new, |cell| cell.to_string());
        let cell_store_runtime = row
            .cell_store_runtime
            .map_or_else(String::new, |runtime| format!("0x{runtime:08X}"));
        let resource_store_runtimes = row
            .resource_store_runtimes
            .iter()
            .map(|runtime| format!("0x{runtime:08X}"))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            row.surface_id,
            row.exe,
            row.archive,
            row.source_text.replace('\n', "\\n"),
            candidate_cells,
            row.common_font_resource_id,
            matched_cell,
            cell_store_runtime,
            resource_store_runtimes,
            row.entry_reached,
            row.static_verdict,
            row.runtime_draw_verified,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn common_font_resource_id(
    exe: &'static str,
    archive: &'static str,
    names: &BtxNameAnalysis,
) -> Result<u16, CommonFontCellSourceError> {
    let links = names
        .links
        .iter()
        .filter(|link| {
            link.exe == exe
                && link.archive == archive
                && link.asset_name.eq_ignore_ascii_case("COM_FONT.BTX")
        })
        .collect::<Vec<_>>();
    let [link] = links.as_slice() else {
        return Err(if links.is_empty() {
            CommonFontCellSourceError::MissingCommonFontLink(exe)
        } else {
            CommonFontCellSourceError::DuplicateCommonFontLink(exe)
        });
    };
    let delta = link
        .descriptor_pointer_runtime
        .checked_sub(link.descriptor_runtime_start)
        .and_then(|delta| delta.checked_sub(DESCRIPTOR_NAME_POINTER_OFFSET))
        .filter(|delta| delta.is_multiple_of(DESCRIPTOR_STRIDE))
        .ok_or(CommonFontCellSourceError::DescriptorPointerLayout(exe))?;
    u16::try_from(delta / DESCRIPTOR_STRIDE)
        .map_err(|_| CommonFontCellSourceError::DescriptorPointerLayout(exe))
}

fn paired_resource_stores(
    instructions: &[Instruction],
    cell_index: usize,
    object_base: Reg,
    resource_offset: i16,
    resource_id: u16,
) -> Vec<u32> {
    let start = cell_index.saturating_sub(RESOURCE_PAIR_WINDOW_WORDS);
    let end = (cell_index + RESOURCE_PAIR_WINDOW_WORDS + 1).min(instructions.len());
    let mut stores = instructions[start..end]
        .iter()
        .enumerate()
        .filter_map(|(relative, instruction)| {
            let index = start + relative;
            let K::Sh { rt, base, offset } = instruction.kind else {
                return None;
            };
            if base != object_base
                || offset != resource_offset
                || literal_u16_before(instructions, index, rt) != Some(resource_id)
                || base_is_clobbered(instructions, cell_index, index, object_base)
            {
                return None;
            }
            Some(instruction.addr)
        })
        .collect::<Vec<_>>();
    stores.sort_unstable();
    stores.dedup();
    stores
}

fn base_is_clobbered(
    instructions: &[Instruction],
    left: usize,
    right: usize,
    register: Reg,
) -> bool {
    let start = left.min(right) + 1;
    let end = left.max(right);
    instructions[start..end]
        .iter()
        .any(|instruction| instruction.dest_reg() == Some(register))
}

fn literal_u16_before(instructions: &[Instruction], before: usize, register: Reg) -> Option<u16> {
    let start = before.saturating_sub(SOURCE_WINDOW_WORDS);
    let value = literal_before_in_window(instructions, start, before, register, 0)?;
    u16::try_from(value).ok()
}

fn literal_before_in_window(
    instructions: &[Instruction],
    start: usize,
    before: usize,
    register: Reg,
    depth: usize,
) -> Option<u32> {
    if register == Reg::ZERO {
        return Some(0);
    }
    if depth > SOURCE_WINDOW_WORDS {
        return None;
    }
    let (index, definition) = instructions[start..before]
        .iter()
        .enumerate()
        .rev()
        .find(|(_, instruction)| instruction.dest_reg() == Some(register))
        .map(|(relative, instruction)| (start + relative, instruction))?;

    match definition.kind {
        K::Lui { rt, imm } if rt == register => Some(u32::from(imm) << 16),
        K::Addi { rt, rs, imm } | K::Addiu { rt, rs, imm } if rt == register => {
            literal_before_in_window(instructions, start, index, rs, depth + 1)
                .map(|base| base.wrapping_add(imm as i32 as u32))
        }
        K::Ori { rt, rs, imm } if rt == register => {
            literal_before_in_window(instructions, start, index, rs, depth + 1)
                .map(|base| base | u32::from(imm))
        }
        K::Addu { rd, rs, rt } | K::Or { rd, rs, rt } if rd == register => {
            let left = literal_before_in_window(instructions, start, index, rs, depth + 1)?;
            let right = literal_before_in_window(instructions, start, index, rt, depth + 1)?;
            Some(match definition.kind {
                K::Addu { .. } => left.wrapping_add(right),
                K::Or { .. } => left | right,
                _ => unreachable!(),
            })
        }
        K::Sll { rd, rt, sa } if rd == register => {
            literal_before_in_window(instructions, start, index, rt, depth + 1)
                .map(|value| value.wrapping_shl(u32::from(sa)))
        }
        _ => None,
    }
}

fn digest_rows(rows: &[CommonFontCellSourceRow]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for row in rows {
        digest.update(row.surface_id.as_bytes());
        digest.update([0]);
        digest.update(row.exe.as_bytes());
        digest.update([0]);
        digest.update(row.archive.as_bytes());
        digest.update([0]);
        digest.update(row.source_text.as_bytes());
        digest.update([0]);
        for cell in &row.candidate_cells {
            digest.update(cell.to_le_bytes());
        }
        digest.update(row.common_font_resource_id.to_le_bytes());
        digest.update(row.matched_cell.unwrap_or(u16::MAX).to_le_bytes());
        digest.update(row.cell_store_runtime.unwrap_or(u32::MAX).to_le_bytes());
        for runtime in &row.resource_store_runtimes {
            digest.update(runtime.to_le_bytes());
        }
        digest.update([u8::from(row.entry_reached)]);
        digest.update(row.static_verdict.as_bytes());
        digest.update([u8::from(row.runtime_draw_verified)]);
    }
    digest.finalize().into()
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "common_font_cell_source_tests.rs"]
mod common_font_cell_source_tests;
