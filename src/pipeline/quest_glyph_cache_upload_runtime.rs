//! QUEST sparse glyph cache 첫 active page의 RAM·VRAM 검증.
//!
//! frozen dump를 exact active-upload build와 다시 결속한다. carrier package의
//! scratch는 실행 중 마지막 glyph로 바뀌므로 그 최종 상태까지 합성해 비교하고,
//! page assignment 125개는 package에 직렬화된 RECT와 catalog cell을 직접 따라
//! VRAM에서 하나씩 대조한다.

use std::error::Error;
use std::fmt;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::coords::{psx_main_ram_offset, PSX_MAIN_RAM_LEN};
use crate::disasm::QUEST_PAGE_CARRIER_START_RUNTIME;

use super::quest_glyph_cache_hook_plan::QuestGlyphCacheHookPlan;
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_glyph_cache_plan::{
    QUEST_GLYPH_CACHE_CELL_BYTES, QUEST_GLYPH_CACHE_SLOT_RECT_BYTES,
};
use super::quest_glyph_cache_upload_poc::{
    QuestGlyphCacheUploadPocBuild, QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID,
};
use super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;

pub const QUEST_GLYPH_CACHE_UPLOAD_RUNTIME_SCHEMA_VERSION: &str =
    "quest-glyph-cache-upload-runtime-v1";

const PSX_GPU_RAM_BYTES: usize = 1024 * 512 * 2;
const PSX_GPU_ROW_BYTES: usize = 1024 * 2;
const QUEST_GLYPH_CACHE_CELL_ROWS: usize = 16;
const QUEST_GLYPH_CACHE_CELL_ROW_BYTES: usize =
    QUEST_GLYPH_CACHE_CELL_BYTES / QUEST_GLYPH_CACHE_CELL_ROWS;

/// 첫 active page commit 뒤 exact resident package와 VRAM 대조 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheUploadRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub ram_sha256: String,
    pub gpu_sha256: String,
    pub events_sha256: String,
    pub package_runtime: u32,
    pub package_bytes: usize,
    pub resident_package_sha256: String,
    pub site_runtime: u32,
    pub site_bytes: usize,
    pub body_runtime: u32,
    pub body_bytes: usize,
    pub page_state_runtime: u32,
    pub page_state: u8,
    pub page_glyphs: usize,
    pub expected_cells_sha256: String,
    pub uploaded_cells_sha256: String,
    pub event_count: usize,
    pub dropped_events: u64,
    pub load_image_calls: usize,
    pub draw_sync_calls: usize,
    pub page_commit_events: usize,
    pub resident_package_exact: bool,
    pub site_exact: bool,
    pub body_exact: bool,
    pub page_state_committed: bool,
    pub uploaded_cells_exact: bool,
    pub upload_event_sequence_exact: bool,
    pub runtime_verified: bool,
    pub release_eligible: bool,
}

/// 첫 active page runtime 결과를 한 행 TSV로 만든다.
pub fn render_quest_glyph_cache_upload_runtime_tsv(
    report: &QuestGlyphCacheUploadRuntimeReport,
) -> String {
    format!(
        "schema_version\timage_sha256\tram_sha256\tgpu_sha256\tevents_sha256\tpackage_runtime\t\
         package_bytes\tresident_package_sha256\tsite_runtime\tsite_bytes\tbody_runtime\t\
         body_bytes\tpage_state_runtime\tpage_state\tpage_glyphs\texpected_cells_sha256\t\
         uploaded_cells_sha256\tevent_count\tdropped_events\tload_image_calls\t\
         draw_sync_calls\tpage_commit_events\tresident_package_exact\tsite_exact\t\
         body_exact\tpage_state_committed\tuploaded_cells_exact\t\
         upload_event_sequence_exact\truntime_verified\trelease_eligible\n\
         {}\t{}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t0x{:08X}\t{}\t\
         0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        report.image_sha256,
        report.ram_sha256,
        report.gpu_sha256,
        report.events_sha256,
        report.package_runtime,
        report.package_bytes,
        report.resident_package_sha256,
        report.site_runtime,
        report.site_bytes,
        report.body_runtime,
        report.body_bytes,
        report.page_state_runtime,
        report.page_state,
        report.page_glyphs,
        report.expected_cells_sha256,
        report.uploaded_cells_sha256,
        report.event_count,
        report.dropped_events,
        report.load_image_calls,
        report.draw_sync_calls,
        report.page_commit_events,
        report.resident_package_exact,
        report.site_exact,
        report.body_exact,
        report.page_state_committed,
        report.uploaded_cells_exact,
        report.upload_event_sequence_exact,
        report.runtime_verified,
        report.release_eligible,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheUploadRuntimeError(String);

impl fmt::Display for QuestGlyphCacheUploadRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestGlyphCacheUploadRuntimeError {}

fn error(message: impl Into<String>) -> QuestGlyphCacheUploadRuntimeError {
    QuestGlyphCacheUploadRuntimeError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct UploadEventEvidence {
    pub(super) event_count: usize,
    pub(super) dropped_events: u64,
    pub(super) load_image_calls: usize,
    pub(super) draw_sync_calls: usize,
    pub(super) page_commit_events: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UploadEventContract {
    upload_pc: u64,
    upload_return: u64,
    sync_pc: u64,
    sync_return: u64,
    commit_pc: u64,
}

fn event_u64(event: &Value, path: &[&str]) -> Option<u64> {
    path.iter()
        .try_fold(event, |value, key| value.get(*key))?
        .as_u64()
}

pub(super) fn verify_upload_events(
    hook: &QuestGlyphCacheHookPlan,
    expected_calls: usize,
    raw: &[u8],
) -> Result<UploadEventEvidence, QuestGlyphCacheUploadRuntimeError> {
    let upload_call = hook
        .body
        .instructions()
        .iter()
        .find(|instruction| {
            instruction.kind
                == crate::isa::InstructionKind::Jal {
                    target: crate::disasm::QUEST_FONT_UPLOAD_TARGET_RUNTIME,
                }
        })
        .ok_or_else(|| error("QUEST sparse cache body에 LoadImage call이 없음"))?;
    let sync_call = hook
        .body
        .instructions()
        .iter()
        .find(|instruction| {
            instruction.kind
                == crate::isa::InstructionKind::Jal {
                    target: crate::disasm::QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME,
                }
        })
        .ok_or_else(|| error("QUEST sparse cache body에 DrawSync call이 없음"))?;
    let commit = hook
        .body
        .instructions()
        .iter()
        .find(|instruction| {
            instruction.kind
                == crate::isa::InstructionKind::Sb {
                    rt: crate::isa::Reg::S1,
                    base: crate::isa::Reg::T0,
                    offset: 0,
                }
        })
        .ok_or_else(|| error("QUEST sparse cache body에 page commit이 없음"))?;
    verify_upload_events_with_contract(
        UploadEventContract {
            upload_pc: u64::from(crate::disasm::QUEST_FONT_UPLOAD_TARGET_RUNTIME),
            upload_return: u64::from(upload_call.addr + 8),
            sync_pc: u64::from(crate::disasm::QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME),
            sync_return: u64::from(sync_call.addr + 8),
            commit_pc: u64::from(commit.addr),
        },
        expected_calls,
        raw,
    )
}

fn verify_upload_events_with_contract(
    contract: UploadEventContract,
    expected_calls: usize,
    raw: &[u8],
) -> Result<UploadEventEvidence, QuestGlyphCacheUploadRuntimeError> {
    let document: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST sparse cache event JSON 해석 실패: {source}")))?;
    let dropped_events = document
        .get("dropped")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("QUEST sparse cache event JSON에 dropped가 없음"))?;
    let events = document
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| error("QUEST sparse cache event JSON에 events 배열이 없음"))?;
    let UploadEventContract {
        upload_pc,
        upload_return,
        sync_pc,
        sync_return,
        commit_pc,
    } = contract;

    let body_events = events
        .iter()
        .filter_map(|event| {
            let pc = event_u64(event, &["pc"])?;
            let ra = event_u64(event, &["registers", "ra"]);
            let s5 = event_u64(event, &["registers", "s5"]);
            if (pc == upload_pc && ra == Some(upload_return))
                || (pc == sync_pc && ra == Some(sync_return))
                || pc == commit_pc
            {
                Some((pc, ra, s5))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let expected_events = expected_calls * 2 + 1;
    let sequence_exact = body_events.len() == expected_events
        && (0..expected_calls).all(|index| {
            body_events[index * 2] == (upload_pc, Some(upload_return), Some(index as u64))
                && body_events[index * 2 + 1] == (sync_pc, Some(sync_return), Some(index as u64))
        })
        && body_events.last().map(|event| event.0) == Some(commit_pc);
    let load_image_calls = body_events
        .iter()
        .filter(|event| event.0 == upload_pc)
        .count();
    let draw_sync_calls = body_events
        .iter()
        .filter(|event| event.0 == sync_pc)
        .count();
    let page_commit_events = body_events
        .iter()
        .filter(|event| event.0 == commit_pc)
        .count();
    if dropped_events != 0
        || load_image_calls != expected_calls
        || draw_sync_calls != expected_calls
        || page_commit_events != 1
        || !sequence_exact
    {
        return Err(error(format!(
            "QUEST sparse cache upload event 불일치: dropped={dropped_events}, load={load_image_calls}, sync={draw_sync_calls}, commit={page_commit_events}, sequence={sequence_exact}"
        )));
    }
    Ok(UploadEventEvidence {
        event_count: events.len(),
        dropped_events,
        load_image_calls,
        draw_sync_calls,
        page_commit_events,
    })
}

pub(super) fn runtime_slice<'a>(
    ram: &'a [u8],
    runtime: u32,
    len: usize,
    label: &str,
) -> Result<&'a [u8], QuestGlyphCacheUploadRuntimeError> {
    let start = psx_main_ram_offset(runtime, len)
        .ok_or_else(|| error(format!("{label}: PS1 main RAM 범위 이탈")))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| error(format!("{label}: RAM 범위 overflow")))?;
    ram.get(start..end)
        .ok_or_else(|| error(format!("{label}: RAM dump 범위 이탈")))
}

pub(super) fn slot_rect(
    package: &QuestGlyphCachePackage,
    slot_offset: usize,
) -> Result<(usize, usize), QuestGlyphCacheUploadRuntimeError> {
    let start = package
        .layout
        .slot_rects_offset
        .checked_add(
            slot_offset
                .checked_mul(QUEST_GLYPH_CACHE_SLOT_RECT_BYTES)
                .ok_or_else(|| error("QUEST sparse cache RECT offset overflow"))?,
        )
        .ok_or_else(|| error("QUEST sparse cache RECT start overflow"))?;
    let end = start
        .checked_add(QUEST_GLYPH_CACHE_SLOT_RECT_BYTES)
        .ok_or_else(|| error("QUEST sparse cache RECT end overflow"))?;
    let raw = package
        .bytes
        .get(start..end)
        .ok_or_else(|| error("QUEST sparse cache RECT package 범위 이탈"))?;
    Ok((
        usize::from(u16::from_le_bytes([raw[0], raw[1]])),
        usize::from(u16::from_le_bytes([raw[2], raw[3]])),
    ))
}

pub(super) fn uploaded_cell(
    gpu: &[u8],
    x_words: usize,
    y: usize,
) -> Result<Vec<u8>, QuestGlyphCacheUploadRuntimeError> {
    if gpu.len() != PSX_GPU_RAM_BYTES {
        return Err(error(format!(
            "PS1 GPU RAM dump 길이 불일치: expected={PSX_GPU_RAM_BYTES}, actual={}",
            gpu.len()
        )));
    }
    let mut cell = Vec::with_capacity(QUEST_GLYPH_CACHE_CELL_BYTES);
    for row in 0..QUEST_GLYPH_CACHE_CELL_ROWS {
        let start = y
            .checked_add(row)
            .and_then(|row| row.checked_mul(PSX_GPU_ROW_BYTES))
            .and_then(|offset| x_words.checked_mul(2).and_then(|x| offset.checked_add(x)))
            .ok_or_else(|| error("QUEST sparse cache VRAM cell offset overflow"))?;
        let end = start
            .checked_add(QUEST_GLYPH_CACHE_CELL_ROW_BYTES)
            .ok_or_else(|| error("QUEST sparse cache VRAM cell end overflow"))?;
        cell.extend_from_slice(
            gpu.get(start..end)
                .ok_or_else(|| error("QUEST sparse cache VRAM cell 범위 이탈"))?,
        );
    }
    Ok(cell)
}

/// exact active build의 resident bytes, page state와 page 62 VRAM cell을 검증한다.
pub fn verify_quest_glyph_cache_upload_runtime(
    build: &QuestGlyphCacheUploadPocBuild,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    ram: &[u8],
    gpu: &[u8],
    events: &[u8],
) -> Result<QuestGlyphCacheUploadRuntimeReport, QuestGlyphCacheUploadRuntimeError> {
    if ram.len() != PSX_MAIN_RAM_LEN {
        return Err(error(format!(
            "PS1 main RAM dump 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
            ram.len()
        )));
    }
    if gpu.len() != PSX_GPU_RAM_BYTES {
        return Err(error(format!(
            "PS1 GPU RAM dump 길이 불일치: expected={PSX_GPU_RAM_BYTES}, actual={}",
            gpu.len()
        )));
    }
    let catalog_indices = package
        .page_catalog_indices(QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID)
        .map_err(|source| error(source.to_string()))?;
    if build.report.disc.image_sha256 != digest(&build.image)
        || build.report.input.page_id != QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID
        || build.report.input.page_glyphs != catalog_indices.len()
        || build.report.runtime_verified
        || build.report.release_eligible
        || package.sha256 != digest(&package.bytes)
        || build.report.disc.package_sha256 != package.sha256
        || build.report.disc.package_bytes != package.bytes.len()
        || build.report.disc.site_runtime != hook.site.base_addr()
        || build.report.disc.body_runtime != hook.body_runtime
        || build.report.disc.body_bytes != hook.body.bytes().len()
        || hook.runtime_verified
        || hook.build_eligible
    {
        return Err(error("QUEST sparse cache active runtime 입력 계약 drift"));
    }

    let mut expected_package = package.bytes.clone();
    let last_catalog_index = *catalog_indices
        .last()
        .ok_or_else(|| error("QUEST sparse cache active page가 비어 있음"))?;
    let final_scratch = package
        .glyph_cell(last_catalog_index)
        .map_err(|source| error(source.to_string()))?;
    let scratch_end = package
        .layout
        .scratch_offset
        .checked_add(final_scratch.len())
        .ok_or_else(|| error("QUEST sparse cache scratch end overflow"))?;
    let expected_scratch = expected_package
        .get_mut(package.layout.scratch_offset..scratch_end)
        .ok_or_else(|| error("QUEST sparse cache scratch package 범위 이탈"))?;
    expected_scratch.copy_from_slice(&final_scratch);
    let loaded_package = runtime_slice(
        ram,
        QUEST_PAGE_CARRIER_START_RUNTIME,
        expected_package.len(),
        "QUEST sparse cache resident package",
    )?;
    let loaded_site = runtime_slice(
        ram,
        hook.site.base_addr(),
        hook.site.bytes().len(),
        "QUEST sparse cache site",
    )?;
    let loaded_body = runtime_slice(
        ram,
        hook.body_runtime,
        hook.body.bytes().len(),
        "QUEST sparse cache body",
    )?;
    let page_state = runtime_slice(
        ram,
        QUEST_PAGE_HOOK_STATE_RUNTIME,
        1,
        "QUEST sparse cache page state",
    )?[0];

    let mut expected_cells =
        Vec::with_capacity(catalog_indices.len() * QUEST_GLYPH_CACHE_CELL_BYTES);
    let mut actual_cells = Vec::with_capacity(expected_cells.capacity());
    let mut mismatched_cells = Vec::new();
    for (slot_offset, catalog_index) in catalog_indices.iter().copied().enumerate() {
        let expected = package
            .glyph_cell(catalog_index)
            .map_err(|source| error(source.to_string()))?;
        let (x_words, y) = slot_rect(package, slot_offset)?;
        let actual = uploaded_cell(gpu, x_words, y)?;
        if actual != expected {
            let restore = package
                .original_atlas_source_cell(slot_offset)
                .map_err(|source| error(source.to_string()))?;
            mismatched_cells.push((slot_offset, catalog_index, x_words, y, actual == restore));
        }
        expected_cells.extend_from_slice(&expected);
        actual_cells.extend_from_slice(&actual);
    }

    let resident_package_exact = loaded_package == expected_package;
    let site_exact = loaded_site == hook.site.bytes();
    let body_exact = loaded_body == hook.body.bytes();
    let page_state_committed = page_state == QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID;
    let uploaded_cells_exact = actual_cells == expected_cells;
    let event_evidence = verify_upload_events(hook, catalog_indices.len(), events)?;
    if !resident_package_exact
        || !site_exact
        || !body_exact
        || !page_state_committed
        || !uploaded_cells_exact
    {
        return Err(error(format!(
            "QUEST sparse cache active runtime 불일치: package/site/body/page/cells={resident_package_exact}/{site_exact}/{body_exact}/{page_state_committed}/{uploaded_cells_exact}, mismatched_cells={mismatched_cells:?}"
        )));
    }

    Ok(QuestGlyphCacheUploadRuntimeReport {
        schema_version: QUEST_GLYPH_CACHE_UPLOAD_RUNTIME_SCHEMA_VERSION,
        image_sha256: build.report.disc.image_sha256.clone(),
        ram_sha256: digest(ram),
        gpu_sha256: digest(gpu),
        events_sha256: digest(events),
        package_runtime: QUEST_PAGE_CARRIER_START_RUNTIME,
        package_bytes: loaded_package.len(),
        resident_package_sha256: digest(loaded_package),
        site_runtime: hook.site.base_addr(),
        site_bytes: loaded_site.len(),
        body_runtime: hook.body_runtime,
        body_bytes: loaded_body.len(),
        page_state_runtime: QUEST_PAGE_HOOK_STATE_RUNTIME,
        page_state,
        page_glyphs: catalog_indices.len(),
        expected_cells_sha256: digest(&expected_cells),
        uploaded_cells_sha256: digest(&actual_cells),
        event_count: event_evidence.event_count,
        dropped_events: event_evidence.dropped_events,
        load_image_calls: event_evidence.load_image_calls,
        draw_sync_calls: event_evidence.draw_sync_calls,
        page_commit_events: event_evidence.page_commit_events,
        resident_package_exact,
        site_exact,
        body_exact,
        page_state_committed,
        uploaded_cells_exact,
        upload_event_sequence_exact: true,
        runtime_verified: true,
        release_eligible: false,
    })
}

#[cfg(test)]
#[path = "quest_glyph_cache_upload_runtime_tests.rs"]
mod tests;
