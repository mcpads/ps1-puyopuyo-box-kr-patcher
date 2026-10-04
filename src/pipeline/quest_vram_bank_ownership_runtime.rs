//! QUEST 한글 atlas bank의 bounded VRAM 점유 보고서.
//!
//! raw GPU dump만 보면 다른 revision의 savestate를 현재 image 실행으로
//! 오인할 수 있다. 따라서 current development build의 package, hook site,
//! original-font guard와 body가 같은 RAM에 exact로 상주하는지 먼저 검증한
//! 뒤에만 두 bank 직사각형의 점유와 page-state별 assignment cell을 보고한다.
//! cell exact는 해당 순간의 residency만 뜻하며 전역 소유권이나 실제 draw를
//! 뜻하지 않는다. 이 보고서만으로 release 안전성을 승인하지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::{psx_main_ram_offset, PSX_MAIN_RAM_LEN};
use crate::disasm::{QUEST_PAGE_CARRIER_CAPACITY_BYTES, QUEST_PAGE_CARRIER_START_RUNTIME};
use crate::graphics::runtime_layout::{VRAM_BYTE_LEN, VRAM_WIDTH_WORDS};

use super::current_development_disc::CurrentDevelopmentDiscBuild;
use super::quest_glyph_cache_hook_plan::{
    QuestGlyphCacheHookPlan, QUEST_GLYPH_CACHE_SECONDARY_VRAM_X,
};
use super::quest_glyph_cache_package::{
    QuestGlyphCachePackage, QUEST_GLYPH_CACHE_VRAM_X, QUEST_GLYPH_CACHE_VRAM_Y,
};
use super::quest_glyph_cache_plan::QUEST_GLYPH_CACHE_CELL_BYTES;
use super::quest_glyph_cache_upload_runtime::{slot_rect, uploaded_cell};
use super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;

pub const QUEST_VRAM_BANK_OWNERSHIP_RUNTIME_SCHEMA_VERSION: &str =
    "quest-vram-bank-ownership-runtime-v2";

const BANK_WIDTH_WORDS: usize = 64;
const BANK_HEIGHT_ROWS: usize = 256;
const BYTES_PER_WORD: usize = 2;
const VRAM_ROW_BYTES: usize = VRAM_WIDTH_WORDS * BYTES_PER_WORD;

/// 한 VRAM bank 직사각형의 내용 분류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestVramBankContent {
    Zero,
    UniformWord(u16),
    Mixed,
}

impl QuestVramBankContent {
    fn label(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::UniformWord(_) => "uniform-word",
            Self::Mixed => "mixed",
        }
    }

    fn uniform_word(self) -> Option<u16> {
        match self {
            Self::UniformWord(word) => Some(word),
            Self::Zero | Self::Mixed => None,
        }
    }
}

/// 한 bank의 bounded occupancy 관측.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestVramBankOccupancy {
    pub x_words: usize,
    pub y: usize,
    pub width_words: usize,
    pub height_rows: usize,
    pub bytes: usize,
    pub nonzero_words: usize,
    pub unique_words: usize,
    pub content: QuestVramBankContent,
    pub sha256: String,
}

/// page state가 가리키는 한 bank의 assignment cell 대조.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestVramBankResidentCells {
    pub page_id: u8,
    pub glyphs: usize,
    pub expected_sha256: String,
    pub actual_sha256: String,
    pub mismatched_slots: Vec<usize>,
    pub exact: bool,
}

/// exact current RAM과 함께 귀속한 두 bank 점유 보고서.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestVramBankOwnershipRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub ram_sha256: String,
    pub gpu_sha256: String,
    pub page_state_runtime: u32,
    pub page_state: [u8; 4],
    pub scratch_catalog_index: Option<u16>,
    pub scratch_zero: bool,
    pub primary: QuestVramBankOccupancy,
    pub secondary: QuestVramBankOccupancy,
    pub primary_resident_cells: Option<QuestVramBankResidentCells>,
    pub secondary_resident_cells: Option<QuestVramBankResidentCells>,
    pub resident_cells_exact: bool,
    pub resident_installation_exact: bool,
    pub ownership_verified: bool,
    pub draw_verified: bool,
    pub release_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestVramBankOwnershipRuntimeError(String);

impl fmt::Display for QuestVramBankOwnershipRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestVramBankOwnershipRuntimeError {}

fn error(message: impl Into<String>) -> QuestVramBankOwnershipRuntimeError {
    QuestVramBankOwnershipRuntimeError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn runtime_slice<'a>(
    ram: &'a [u8],
    runtime: u32,
    len: usize,
    label: &str,
) -> Result<&'a [u8], QuestVramBankOwnershipRuntimeError> {
    let start = psx_main_ram_offset(runtime, len)
        .ok_or_else(|| error(format!("{label} runtime 범위가 PS1 main RAM 밖임")))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| error(format!("{label} RAM 범위 overflow")))?;
    ram.get(start..end)
        .ok_or_else(|| error(format!("{label} RAM dump가 잘림")))
}

fn require_exact(
    actual: &[u8],
    expected: &[u8],
    label: &str,
) -> Result<(), QuestVramBankOwnershipRuntimeError> {
    if actual == expected {
        return Ok(());
    }
    let first = actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| actual != expected)
        .unwrap_or_else(|| actual.len().min(expected.len()));
    Err(error(format!(
        "{label} 불일치: offset=0x{first:X}, expected_bytes={}, actual_bytes={}",
        expected.len(),
        actual.len()
    )))
}

fn scratch_catalog_index(
    package: &QuestGlyphCachePackage,
    loaded_scratch: &[u8],
) -> Result<(bool, Option<u16>), QuestVramBankOwnershipRuntimeError> {
    if loaded_scratch.len() != QUEST_GLYPH_CACHE_CELL_BYTES {
        return Err(error(format!(
            "QUEST bank scratch 길이 불일치: expected={QUEST_GLYPH_CACHE_CELL_BYTES}, actual={}",
            loaded_scratch.len()
        )));
    }
    if loaded_scratch.iter().all(|byte| *byte == 0) {
        return Ok((true, None));
    }
    for index in 0..package.plan.glyphs {
        let index =
            u16::try_from(index).map_err(|_| error("QUEST bank glyph catalog index 범위 이탈"))?;
        let cell = package
            .glyph_cell(index)
            .map_err(|source| error(source.to_string()))?;
        if cell == loaded_scratch {
            return Ok((false, Some(index)));
        }
    }
    Err(error(
        "QUEST bank scratch가 zero도 catalog glyph cell도 아님",
    ))
}

fn verify_resident_installation(
    build: &CurrentDevelopmentDiscBuild,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    ram: &[u8],
) -> Result<(bool, Option<u16>), QuestVramBankOwnershipRuntimeError> {
    if build.report.quest.disc.image_sha256 != digest(&build.image)
        || build.report.quest.disc.package_sha256 != package.sha256
        || package.sha256 != digest(&package.bytes)
        || build.report.quest.disc.package_bytes != package.bytes.len()
        || build.report.quest.disc.site_runtime != hook.site.base_addr()
        || build.report.quest.disc.body_runtime != hook.body_runtime
        || build.report.quest.disc.body_bytes != hook.body.bytes().len()
        || !hook.package_materialized
        || hook.runtime_verified
        || hook.build_eligible
    {
        return Err(error("QUEST VRAM bank runtime 입력 계약 drift"));
    }

    let loaded_package = runtime_slice(
        ram,
        QUEST_PAGE_CARRIER_START_RUNTIME,
        package.bytes.len(),
        "QUEST bank package",
    )?;
    let prefix_end = package.layout.scratch_offset;
    require_exact(
        loaded_package
            .get(..prefix_end)
            .ok_or_else(|| error("QUEST bank package prefix 범위 이탈"))?,
        package
            .bytes
            .get(..prefix_end)
            .ok_or_else(|| error("QUEST bank expected package prefix 범위 이탈"))?,
        "QUEST bank package immutable prefix",
    )?;
    let scratch_end = prefix_end
        .checked_add(package.layout.scratch_bytes)
        .ok_or_else(|| error("QUEST bank scratch end overflow"))?;
    if scratch_end != package.bytes.len() {
        return Err(error("QUEST bank scratch가 package tail이 아님"));
    }
    let (scratch_zero, scratch_catalog_index) = scratch_catalog_index(
        package,
        loaded_package
            .get(prefix_end..scratch_end)
            .ok_or_else(|| error("QUEST bank loaded scratch 범위 이탈"))?,
    )?;

    require_exact(
        runtime_slice(
            ram,
            hook.site.base_addr(),
            hook.site.bytes().len(),
            "QUEST bank OP21 site",
        )?,
        hook.site.bytes(),
        "QUEST bank OP21 site",
    )?;
    require_exact(
        runtime_slice(
            ram,
            hook.original_font_guard_site.base_addr(),
            hook.original_font_guard_site.bytes().len(),
            "QUEST bank original-font guard",
        )?,
        hook.original_font_guard_site.bytes(),
        "QUEST bank original-font guard",
    )?;
    require_exact(
        runtime_slice(
            ram,
            hook.body_runtime,
            hook.body.bytes().len(),
            "QUEST bank body",
        )?,
        hook.body.bytes(),
        "QUEST bank body",
    )?;

    let carrier_end = QUEST_PAGE_CARRIER_START_RUNTIME
        .checked_add(
            u32::try_from(QUEST_PAGE_CARRIER_CAPACITY_BYTES)
                .map_err(|_| error("QUEST bank carrier 길이 범위 이탈"))?,
        )
        .ok_or_else(|| error("QUEST bank carrier end overflow"))?;
    let body_end = hook
        .body_runtime
        .checked_add(
            u32::try_from(hook.body.bytes().len())
                .map_err(|_| error("QUEST bank body 길이 범위 이탈"))?,
        )
        .ok_or_else(|| error("QUEST bank body end overflow"))?;
    let suffix_bytes = usize::try_from(
        carrier_end
            .checked_sub(body_end)
            .ok_or_else(|| error("QUEST bank body가 carrier를 넘음"))?,
    )
    .map_err(|_| error("QUEST bank suffix 길이 범위 이탈"))?;
    let suffix = runtime_slice(ram, body_end, suffix_bytes, "QUEST bank carrier suffix")?;
    if let Some(offset) = suffix.iter().position(|byte| *byte != 0) {
        return Err(error(format!(
            "QUEST bank carrier suffix가 0이 아님: offset=0x{offset:X}, value=0x{:02X}",
            suffix[offset]
        )));
    }

    Ok((scratch_zero, scratch_catalog_index))
}

fn bank_bytes(gpu: &[u8], x_words: usize) -> Result<Vec<u8>, QuestVramBankOwnershipRuntimeError> {
    let mut result = Vec::with_capacity(BANK_WIDTH_WORDS * BANK_HEIGHT_ROWS * BYTES_PER_WORD);
    for row in 0..BANK_HEIGHT_ROWS {
        let start = QUEST_GLYPH_CACHE_VRAM_Y
            .checked_add(row)
            .and_then(|y| y.checked_mul(VRAM_ROW_BYTES))
            .and_then(|offset| {
                x_words
                    .checked_mul(BYTES_PER_WORD)
                    .and_then(|x| offset.checked_add(x))
            })
            .ok_or_else(|| error("QUEST bank VRAM offset overflow"))?;
        let end = start
            .checked_add(BANK_WIDTH_WORDS * BYTES_PER_WORD)
            .ok_or_else(|| error("QUEST bank VRAM row end overflow"))?;
        result.extend_from_slice(
            gpu.get(start..end)
                .ok_or_else(|| error("QUEST bank VRAM row 범위 이탈"))?,
        );
    }
    Ok(result)
}

fn summarize_bank(
    gpu: &[u8],
    x_words: usize,
) -> Result<QuestVramBankOccupancy, QuestVramBankOwnershipRuntimeError> {
    let bytes = bank_bytes(gpu, x_words)?;
    let words = bytes
        .chunks_exact(BYTES_PER_WORD)
        .map(|raw| u16::from_le_bytes([raw[0], raw[1]]))
        .collect::<Vec<_>>();
    let nonzero_words = words.iter().filter(|word| **word != 0).count();
    let unique = words.iter().copied().collect::<BTreeSet<_>>();
    let content = if nonzero_words == 0 {
        QuestVramBankContent::Zero
    } else if unique.len() == 1 {
        QuestVramBankContent::UniformWord(words[0])
    } else {
        QuestVramBankContent::Mixed
    };
    Ok(QuestVramBankOccupancy {
        x_words,
        y: QUEST_GLYPH_CACHE_VRAM_Y,
        width_words: BANK_WIDTH_WORDS,
        height_rows: BANK_HEIGHT_ROWS,
        bytes: bytes.len(),
        nonzero_words,
        unique_words: unique.len(),
        content,
        sha256: digest(&bytes),
    })
}

fn expected_page_assignments(
    package: &QuestGlyphCachePackage,
    page_id: u8,
) -> Result<Vec<(usize, usize, Vec<u8>)>, QuestVramBankOwnershipRuntimeError> {
    let indices = package
        .page_catalog_indices(page_id)
        .map_err(|source| error(source.to_string()))?;
    indices
        .iter()
        .copied()
        .enumerate()
        .map(|(slot, catalog_index)| {
            let (x_words, y) =
                slot_rect(package, slot).map_err(|source| error(source.to_string()))?;
            let cell = package
                .glyph_cell(catalog_index)
                .map_err(|source| error(source.to_string()))?;
            Ok((x_words, y, cell))
        })
        .collect()
}

fn summarize_assigned_cells(
    gpu: &[u8],
    page_id: u8,
    bank_x_words: usize,
    assignments: &[(usize, usize, Vec<u8>)],
) -> Result<QuestVramBankResidentCells, QuestVramBankOwnershipRuntimeError> {
    let bank_x_delta = bank_x_words
        .checked_sub(QUEST_GLYPH_CACHE_VRAM_X)
        .ok_or_else(|| error("QUEST resident cell bank x가 primary보다 작음"))?;
    let mut expected = Vec::with_capacity(assignments.len() * QUEST_GLYPH_CACHE_CELL_BYTES);
    let mut actual = Vec::with_capacity(expected.capacity());
    let mut mismatched_slots = Vec::new();
    for (slot, (x_words, y, expected_cell)) in assignments.iter().enumerate() {
        if expected_cell.len() != QUEST_GLYPH_CACHE_CELL_BYTES {
            return Err(error(format!(
                "QUEST resident expected cell 길이 불일치: slot={slot}, expected={QUEST_GLYPH_CACHE_CELL_BYTES}, actual={}",
                expected_cell.len()
            )));
        }
        let actual_x_words = x_words
            .checked_add(bank_x_delta)
            .ok_or_else(|| error("QUEST resident cell x overflow"))?;
        let actual_cell =
            uploaded_cell(gpu, actual_x_words, *y).map_err(|source| error(source.to_string()))?;
        if actual_cell != *expected_cell {
            mismatched_slots.push(slot);
        }
        expected.extend_from_slice(expected_cell);
        actual.extend_from_slice(&actual_cell);
    }
    if assignments.is_empty() {
        return Err(error(format!(
            "QUEST resident page assignment가 비어 있음: page={page_id}"
        )));
    }
    Ok(QuestVramBankResidentCells {
        page_id,
        glyphs: assignments.len(),
        expected_sha256: digest(&expected),
        actual_sha256: digest(&actual),
        exact: mismatched_slots.is_empty(),
        mismatched_slots,
    })
}

fn summarize_resident_page(
    package: &QuestGlyphCachePackage,
    gpu: &[u8],
    page_id: u8,
    bank_x_words: usize,
) -> Result<Option<QuestVramBankResidentCells>, QuestVramBankOwnershipRuntimeError> {
    // 두-bank object atlas에서 0은 Korean page가 배정되지 않은 초기 상태다.
    if page_id == 0 {
        return Ok(None);
    }
    let assignments = expected_page_assignments(package, page_id)?;
    summarize_assigned_cells(gpu, page_id, bank_x_words, &assignments).map(Some)
}

/// exact current installation을 먼저 검증한 뒤 두 bank 점유를 보고한다.
pub fn verify_quest_vram_bank_ownership_runtime(
    build: &CurrentDevelopmentDiscBuild,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    ram: &[u8],
    gpu: &[u8],
) -> Result<QuestVramBankOwnershipRuntimeReport, QuestVramBankOwnershipRuntimeError> {
    if ram.len() != PSX_MAIN_RAM_LEN {
        return Err(error(format!(
            "PS1 main RAM dump 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
            ram.len()
        )));
    }
    if gpu.len() != VRAM_BYTE_LEN {
        return Err(error(format!(
            "PS1 GPU RAM dump 길이 불일치: expected={VRAM_BYTE_LEN}, actual={}",
            gpu.len()
        )));
    }
    let (scratch_zero, scratch_catalog_index) =
        verify_resident_installation(build, package, hook, ram)?;
    let page_state: [u8; 4] = runtime_slice(
        ram,
        QUEST_PAGE_HOOK_STATE_RUNTIME,
        4,
        "QUEST bank page state",
    )?
    .try_into()
    .map_err(|_| error("QUEST bank page state 길이 불일치"))?;
    let primary_resident_cells =
        summarize_resident_page(package, gpu, page_state[0], QUEST_GLYPH_CACHE_VRAM_X)?;
    let secondary_resident_cells = summarize_resident_page(
        package,
        gpu,
        page_state[3],
        QUEST_GLYPH_CACHE_SECONDARY_VRAM_X,
    )?;
    let resident_cells = [
        primary_resident_cells.as_ref(),
        secondary_resident_cells.as_ref(),
    ];
    let resident_cells_exact = resident_cells.iter().flatten().next().is_some()
        && resident_cells.iter().flatten().all(|cells| cells.exact);

    Ok(QuestVramBankOwnershipRuntimeReport {
        schema_version: QUEST_VRAM_BANK_OWNERSHIP_RUNTIME_SCHEMA_VERSION,
        image_sha256: build.report.quest.disc.image_sha256.clone(),
        ram_sha256: digest(ram),
        gpu_sha256: digest(gpu),
        page_state_runtime: QUEST_PAGE_HOOK_STATE_RUNTIME,
        page_state,
        scratch_catalog_index,
        scratch_zero,
        primary: summarize_bank(gpu, QUEST_GLYPH_CACHE_VRAM_X)?,
        secondary: summarize_bank(gpu, QUEST_GLYPH_CACHE_SECONDARY_VRAM_X)?,
        primary_resident_cells,
        secondary_resident_cells,
        resident_cells_exact,
        resident_installation_exact: true,
        ownership_verified: false,
        draw_verified: false,
        release_eligible: false,
    })
}

/// bounded bank 점유 보고서를 한 행 TSV로 만든다.
pub fn render_quest_vram_bank_ownership_runtime_tsv(
    report: &QuestVramBankOwnershipRuntimeReport,
) -> String {
    let scratch_catalog_index = report
        .scratch_catalog_index
        .map(|value| value.to_string())
        .unwrap_or_default();
    let primary_uniform = report
        .primary
        .content
        .uniform_word()
        .map(|value| format!("0x{value:04X}"))
        .unwrap_or_default();
    let secondary_uniform = report
        .secondary
        .content
        .uniform_word()
        .map(|value| format!("0x{value:04X}"))
        .unwrap_or_default();
    let render_resident = |resident: Option<&QuestVramBankResidentCells>| {
        resident.map_or_else(
            || {
                (
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                )
            },
            |resident| {
                (
                    resident.glyphs.to_string(),
                    resident.expected_sha256.clone(),
                    resident.actual_sha256.clone(),
                    resident
                        .mismatched_slots
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                    resident.exact.to_string(),
                )
            },
        )
    };
    let primary_resident = render_resident(report.primary_resident_cells.as_ref());
    let secondary_resident = render_resident(report.secondary_resident_cells.as_ref());
    format!(
        "schema_version\timage_sha256\tram_sha256\tgpu_sha256\tpage_state_runtime\t\
         primary_page\troute_page\trenderer_page\tsecondary_page\tscratch_zero\t\
         scratch_catalog_index\tprimary_x_words\tprimary_y\tprimary_bytes\t\
         primary_nonzero_words\tprimary_unique_words\tprimary_content\t\
         primary_uniform_word\tprimary_sha256\tsecondary_x_words\tsecondary_y\t\
         secondary_bytes\tsecondary_nonzero_words\tsecondary_unique_words\t\
         secondary_content\tsecondary_uniform_word\tsecondary_sha256\t\
         primary_resident_glyphs\tprimary_expected_cells_sha256\t\
         primary_actual_cells_sha256\tprimary_mismatched_slots\tprimary_cells_exact\t\
         secondary_resident_glyphs\tsecondary_expected_cells_sha256\t\
         secondary_actual_cells_sha256\tsecondary_mismatched_slots\tsecondary_cells_exact\t\
         resident_cells_exact\t\
         resident_installation_exact\townership_verified\tdraw_verified\trelease_eligible\n\
         {}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        report.image_sha256,
        report.ram_sha256,
        report.gpu_sha256,
        report.page_state_runtime,
        report.page_state[0],
        report.page_state[1],
        report.page_state[2],
        report.page_state[3],
        report.scratch_zero,
        scratch_catalog_index,
        report.primary.x_words,
        report.primary.y,
        report.primary.bytes,
        report.primary.nonzero_words,
        report.primary.unique_words,
        report.primary.content.label(),
        primary_uniform,
        report.primary.sha256,
        report.secondary.x_words,
        report.secondary.y,
        report.secondary.bytes,
        report.secondary.nonzero_words,
        report.secondary.unique_words,
        report.secondary.content.label(),
        secondary_uniform,
        report.secondary.sha256,
        primary_resident.0,
        primary_resident.1,
        primary_resident.2,
        primary_resident.3,
        primary_resident.4,
        secondary_resident.0,
        secondary_resident.1,
        secondary_resident.2,
        secondary_resident.3,
        secondary_resident.4,
        report.resident_cells_exact,
        report.resident_installation_exact,
        report.ownership_verified,
        report.draw_verified,
        report.release_eligible,
    )
}

#[cfg(test)]
#[path = "quest_vram_bank_ownership_runtime_tests.rs"]
mod tests;
