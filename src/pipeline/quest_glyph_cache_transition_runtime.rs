//! QUEST sparse glyph cache의 `62 → 62 → 50 → 0` runtime 검증.
//!
//! exact 전환 build와 네 OP21 checkpoint를 다시 결속한다. 각 checkpoint의
//! entry state, entry/return RAM, return GPU와 lifecycle event를 함께 검사해
//! 같은 page의 upload 생략, 다른 Korean page의 cell 교체, 비대상 record의
//! 원문 복구를 구분한다. 화면 PNG는 구조와 digest만 묶으며 문장·layout
//! 승인을 대신하지 않는다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::coords::{psx_main_ram_offset, PSX_MAIN_RAM_LEN};
use crate::disasm::{
    QUEST_FONT_STATE_BASE_RUNTIME, QUEST_FONT_STATE_RECORD_COUNT, QUEST_FONT_STATE_RECORD_STRIDE,
    QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME, QUEST_FONT_UPLOAD_TARGET_RUNTIME,
    QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET, QUEST_HOOK_GP_RUNTIME,
    QUEST_PAGE_CARRIER_START_RUNTIME,
};
use crate::script::TOWN00_SEQ_RUNTIME_BASE;

use super::quest_glyph_cache_hook_plan::QuestGlyphCacheHookPlan;
use super::quest_glyph_cache_hook_plan::{
    QUEST_GLYPH_CACHE_UPLOAD_ROWS, QUEST_GLYPH_CACHE_UPLOAD_WORD_WIDTH,
};
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_glyph_cache_plan::{
    QUEST_GLYPH_CACHE_CELL_BYTES, QUEST_GLYPH_CACHE_SLOT_RECT_BYTES,
};
use super::quest_glyph_cache_transition_poc::{
    QuestGlyphCacheTransitionPocBuild, QUEST_GLYPH_CACHE_TRANSITION_SECOND_PAGE_ID,
    QUEST_GLYPH_CACHE_TRANSITION_SECOND_RECORD_OFFSET, QUEST_GLYPH_CACHE_TRANSITION_THIRD_PAGE_ID,
    QUEST_GLYPH_CACHE_TRANSITION_THIRD_RECORD_OFFSET,
};
use super::quest_glyph_cache_upload_poc::{
    QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID, QUEST_GLYPH_CACHE_UPLOAD_POC_RECORD_OFFSET,
};
use super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;

pub const QUEST_GLYPH_CACHE_TRANSITION_RUNTIME_SCHEMA_VERSION: &str =
    "quest-glyph-cache-transition-runtime-v1";
pub const QUEST_GLYPH_CACHE_TRANSITION_RESTORE_PAGE_ID: u8 = 0;
pub const QUEST_GLYPH_CACHE_TRANSITION_RESTORE_RECORD_OFFSET: usize = 0x36F0;

const QUEST_BODY_RETURN_RUNTIME: u32 = 0x8002_25E0;
const QUEST_OBJECT_ACTIVE_OFFSET: u32 = 0;
const QUEST_OBJECT_STREAM_OFFSET: u32 = 0x1C;
const QUEST_FONT_STATE_ACTIVE_OFFSET: u32 = QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET as u32 + 1;
const PSX_GPU_RAM_BYTES: usize = 1024 * 512 * 2;
const PSX_GPU_ROW_BYTES: usize = 1024 * 2;
const PNG_WIDTH: u32 = 280;
const PNG_HEIGHT: u32 = 240;

/// OP21 하나의 entry와 return을 묶은 Emucap 증거.
#[derive(Debug, Clone, Copy)]
pub struct QuestGlyphCacheTransitionRuntimeCheckpoint<'a> {
    pub entry_state: &'a [u8],
    pub entry_ram: &'a [u8],
    pub return_ram: &'a [u8],
    pub return_gpu: &'a [u8],
    pub lifecycle_events: &'a [u8],
}

/// 두 Korean page와 다음 원문 복구까지의 runtime 증거.
#[derive(Debug, Clone, Copy)]
pub struct QuestGlyphCacheTransitionRuntimeEvidence<'a> {
    pub first: QuestGlyphCacheTransitionRuntimeCheckpoint<'a>,
    pub second: QuestGlyphCacheTransitionRuntimeCheckpoint<'a>,
    pub third: QuestGlyphCacheTransitionRuntimeCheckpoint<'a>,
    pub restored: QuestGlyphCacheTransitionRuntimeCheckpoint<'a>,
    pub first_visible_png: &'a [u8],
    pub first_visible_page2_png: &'a [u8],
    pub first_visible_page3_png: &'a [u8],
    pub second_visible_png: &'a [u8],
    pub second_visible_page2_png: &'a [u8],
    pub third_visible_png: &'a [u8],
    pub third_visible_page2_png: &'a [u8],
    pub restored_visible_png: &'a [u8],
}

/// `62 → 62 → 50 → 0` 실행 검증 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheTransitionRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub ram_sha256: Vec<String>,
    pub gpu_sha256: Vec<String>,
    pub entry_state_sha256: Vec<String>,
    pub lifecycle_events_sha256: Vec<String>,
    pub visible_png_sha256: Vec<String>,
    pub record_runtimes: Vec<u32>,
    pub object_runtimes: Vec<u32>,
    pub page_states: Vec<u8>,
    pub page_glyphs: Vec<usize>,
    pub load_image_calls: Vec<usize>,
    pub draw_sync_calls: Vec<usize>,
    pub event_count: usize,
    pub dropped_events: u64,
    pub resident_inputs_exact: bool,
    pub uploaded_cells_exact: bool,
    pub same_page_upload_skipped: bool,
    pub different_page_upload_exact: bool,
    pub original_page_restored: bool,
    pub visible_capture_shapes_exact: bool,
    pub runtime_verified: bool,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub release_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheTransitionRuntimeError(String);

impl fmt::Display for QuestGlyphCacheTransitionRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestGlyphCacheTransitionRuntimeError {}

fn error(message: impl Into<String>) -> QuestGlyphCacheTransitionRuntimeError {
    QuestGlyphCacheTransitionRuntimeError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn runtime_slice<'a>(
    ram: &'a [u8],
    runtime: u32,
    len: usize,
    label: &str,
) -> Result<&'a [u8], QuestGlyphCacheTransitionRuntimeError> {
    let start = psx_main_ram_offset(runtime, len)
        .ok_or_else(|| error(format!("{label}: PS1 main RAM 범위 이탈")))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| error(format!("{label}: RAM 범위 overflow")))?;
    ram.get(start..end)
        .ok_or_else(|| error(format!("{label}: RAM dump 범위 이탈")))
}

fn runtime_u32(
    ram: &[u8],
    runtime: u32,
    label: &str,
) -> Result<u32, QuestGlyphCacheTransitionRuntimeError> {
    let raw: [u8; 4] = runtime_slice(ram, runtime, 4, label)?
        .try_into()
        .map_err(|_| error(format!("{label}: u32 길이 불일치")))?;
    Ok(u32::from_le_bytes(raw))
}

fn json_u32(
    value: &Value,
    path: &[&str],
    label: &str,
) -> Result<u32, QuestGlyphCacheTransitionRuntimeError> {
    let raw = path
        .iter()
        .try_fold(value, |value, key| value.get(*key))
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("QUEST sparse transition JSON에 {label}가 없음")))?;
    u32::try_from(raw).map_err(|_| error(format!("QUEST sparse transition {label} 범위 이탈")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EntryState {
    object_runtime: u32,
    gp: u32,
    ra: u32,
    sp: u32,
}

fn parse_entry_state(
    raw: &[u8],
    body_runtime: u32,
) -> Result<EntryState, QuestGlyphCacheTransitionRuntimeError> {
    let state: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST sparse transition state JSON 실패: {source}")))?;
    let pc = json_u32(&state, &["g0.PC"], "entry PC")?;
    let entry = EntryState {
        object_runtime: json_u32(&state, &["g0.a0"], "entry a0")?,
        gp: json_u32(&state, &["g0.gp"], "entry gp")?,
        ra: json_u32(&state, &["g0.ra"], "entry ra")?,
        sp: json_u32(&state, &["g0.sp"], "entry sp")?,
    };
    if pc != body_runtime
        || entry.gp != QUEST_HOOK_GP_RUNTIME
        || entry.ra != QUEST_BODY_RETURN_RUNTIME
    {
        return Err(error("QUEST sparse transition entry register 계약 불일치"));
    }
    Ok(entry)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LifecycleEvidence {
    event_count: usize,
    dropped_events: u64,
    load_image_calls: usize,
    draw_sync_calls: usize,
}

fn call_return(
    hook: &QuestGlyphCacheHookPlan,
    target: u32,
    label: &str,
) -> Result<u32, QuestGlyphCacheTransitionRuntimeError> {
    hook.body
        .instructions()
        .iter()
        .find(|instruction| instruction.kind == crate::isa::InstructionKind::Jal { target })
        .map(|instruction| instruction.addr + 8)
        .ok_or_else(|| {
            error(format!(
                "QUEST sparse transition body에 {label} call이 없음"
            ))
        })
}

fn verify_lifecycle(
    raw: &[u8],
    hook: &QuestGlyphCacheHookPlan,
    entry: EntryState,
    expected_calls: usize,
    scratch_offset: usize,
) -> Result<LifecycleEvidence, QuestGlyphCacheTransitionRuntimeError> {
    let document: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST sparse transition event JSON 실패: {source}")))?;
    let dropped_events = document
        .get("dropped")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("QUEST sparse transition event JSON에 dropped가 없음"))?;
    let events = document
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| error("QUEST sparse transition event JSON에 events가 없음"))?;
    let expected_event_count = expected_calls
        .checked_mul(2)
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| error("QUEST sparse transition event 수 overflow"))?;
    if dropped_events != 0 || events.len() != expected_event_count {
        return Err(error(format!(
            "QUEST sparse transition event 수 불일치: dropped={dropped_events}, expected={expected_event_count}, actual={}",
            events.len()
        )));
    }

    let upload_return = call_return(hook, QUEST_FONT_UPLOAD_TARGET_RUNTIME, "LoadImage")?;
    let sync_return = call_return(hook, QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME, "DrawSync")?;
    let scratch_runtime = QUEST_PAGE_CARRIER_START_RUNTIME
        .checked_add(
            u32::try_from(scratch_offset)
                .map_err(|_| error("QUEST sparse transition scratch offset 범위 이탈"))?,
        )
        .ok_or_else(|| error("QUEST sparse transition scratch runtime overflow"))?;
    for index in 0..expected_calls {
        let upload = &events[index * 2];
        let sync = &events[index * 2 + 1];
        if json_u32(upload, &["pc"], "upload pc")? != QUEST_FONT_UPLOAD_TARGET_RUNTIME
            || json_u32(upload, &["registers", "ra"], "upload ra")? != upload_return
            || json_u32(upload, &["registers", "a1"], "upload a1")? != scratch_runtime
            || usize::try_from(json_u32(upload, &["registers", "s5"], "upload s5")?)
                .map_err(|_| error("QUEST sparse transition upload s5 범위 이탈"))?
                != index
            || json_u32(sync, &["pc"], "sync pc")? != QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME
            || json_u32(sync, &["registers", "ra"], "sync ra")? != sync_return
            || json_u32(sync, &["registers", "a0"], "sync a0")? != 0
            || usize::try_from(json_u32(sync, &["registers", "s5"], "sync s5")?)
                .map_err(|_| error("QUEST sparse transition sync s5 범위 이탈"))?
                != index
        {
            return Err(error(format!(
                "QUEST sparse transition upload/sync 순서 불일치: index={index}"
            )));
        }
    }
    let returned = events
        .last()
        .ok_or_else(|| error("QUEST sparse transition return event가 없음"))?;
    if json_u32(returned, &["pc"], "return pc")? != QUEST_BODY_RETURN_RUNTIME
        || json_u32(returned, &["registers", "a0"], "return a0")? != entry.object_runtime
        || json_u32(returned, &["registers", "gp"], "return gp")? != entry.gp
        || json_u32(returned, &["registers", "ra"], "return ra")? != entry.ra
        || json_u32(returned, &["registers", "sp"], "return sp")? != entry.sp
    {
        return Err(error("QUEST sparse transition return register 계약 불일치"));
    }
    Ok(LifecycleEvidence {
        event_count: events.len(),
        dropped_events,
        load_image_calls: expected_calls,
        draw_sync_calls: expected_calls,
    })
}

fn slot_rect(
    package: &QuestGlyphCachePackage,
    slot_offset: usize,
) -> Result<(usize, usize), QuestGlyphCacheTransitionRuntimeError> {
    let start = package
        .layout
        .slot_rects_offset
        .checked_add(
            slot_offset
                .checked_mul(QUEST_GLYPH_CACHE_SLOT_RECT_BYTES)
                .ok_or_else(|| error("QUEST sparse transition RECT offset overflow"))?,
        )
        .ok_or_else(|| error("QUEST sparse transition RECT start overflow"))?;
    let raw = package
        .bytes
        .get(start..start + QUEST_GLYPH_CACHE_SLOT_RECT_BYTES)
        .ok_or_else(|| error("QUEST sparse transition RECT package 범위 이탈"))?;
    Ok((
        usize::from(u16::from_le_bytes([raw[0], raw[1]])),
        usize::from(u16::from_le_bytes([raw[2], raw[3]])),
    ))
}

fn uploaded_cell(
    gpu: &[u8],
    x_words: usize,
    y: usize,
) -> Result<Vec<u8>, QuestGlyphCacheTransitionRuntimeError> {
    if gpu.len() != PSX_GPU_RAM_BYTES {
        return Err(error(format!(
            "PS1 GPU RAM dump 길이 불일치: expected={PSX_GPU_RAM_BYTES}, actual={}",
            gpu.len()
        )));
    }
    let row_bytes = usize::from(QUEST_GLYPH_CACHE_UPLOAD_WORD_WIDTH) * 2;
    let mut cell = Vec::with_capacity(QUEST_GLYPH_CACHE_CELL_BYTES);
    for row in 0..usize::from(QUEST_GLYPH_CACHE_UPLOAD_ROWS) {
        let start = y
            .checked_add(row)
            .and_then(|row| row.checked_mul(PSX_GPU_ROW_BYTES))
            .and_then(|offset| x_words.checked_mul(2).and_then(|x| offset.checked_add(x)))
            .ok_or_else(|| error("QUEST sparse transition VRAM cell offset overflow"))?;
        let end = start
            .checked_add(row_bytes)
            .ok_or_else(|| error("QUEST sparse transition VRAM cell end overflow"))?;
        cell.extend_from_slice(
            gpu.get(start..end)
                .ok_or_else(|| error("QUEST sparse transition VRAM cell 범위 이탈"))?,
        );
    }
    if cell.len() != QUEST_GLYPH_CACHE_CELL_BYTES {
        return Err(error("QUEST sparse transition VRAM cell 길이 불일치"));
    }
    Ok(cell)
}

fn expected_page_cells(
    package: &QuestGlyphCachePackage,
    page_id: u8,
) -> Result<Vec<Vec<u8>>, QuestGlyphCacheTransitionRuntimeError> {
    if page_id == QUEST_GLYPH_CACHE_TRANSITION_RESTORE_PAGE_ID {
        return (0..package.plan.max_page_glyphs)
            .map(|slot| {
                package
                    .original_atlas_source_cell(slot)
                    .map_err(|source| error(source.to_string()))
            })
            .collect();
    }
    package
        .page_catalog_indices(page_id)
        .map_err(|source| error(source.to_string()))?
        .iter()
        .map(|index| {
            package
                .glyph_cell(*index)
                .map_err(|source| error(source.to_string()))
        })
        .collect()
}

fn expected_resident_package(
    package: &QuestGlyphCachePackage,
    expected_cells: &[Vec<u8>],
) -> Result<Vec<u8>, QuestGlyphCacheTransitionRuntimeError> {
    let mut expected = package.bytes.clone();
    let final_cell = expected_cells
        .last()
        .ok_or_else(|| error("QUEST sparse transition page cell이 비어 있음"))?;
    let end = package
        .layout
        .scratch_offset
        .checked_add(final_cell.len())
        .ok_or_else(|| error("QUEST sparse transition scratch end overflow"))?;
    expected
        .get_mut(package.layout.scratch_offset..end)
        .ok_or_else(|| error("QUEST sparse transition scratch package 범위 이탈"))?
        .copy_from_slice(final_cell);
    Ok(expected)
}

fn font_states_inactive(ram: &[u8]) -> Result<bool, QuestGlyphCacheTransitionRuntimeError> {
    for index in 0..QUEST_FONT_STATE_RECORD_COUNT {
        let runtime = QUEST_FONT_STATE_BASE_RUNTIME
            + u32::try_from(index * QUEST_FONT_STATE_RECORD_STRIDE)
                .map_err(|_| error("QUEST sparse transition font state offset 범위 이탈"))?
            + QUEST_FONT_STATE_ACTIVE_OFFSET;
        if runtime_slice(ram, runtime, 1, "QUEST sparse transition font active")? != [0] {
            return Ok(false);
        }
    }
    Ok(true)
}

fn png_shape_exact(raw: &[u8]) -> bool {
    raw.get(..8) == Some(b"\x89PNG\r\n\x1a\n")
        && raw.get(12..16) == Some(b"IHDR")
        && raw
            .get(16..20)
            .and_then(|raw| raw.try_into().ok())
            .map(u32::from_be_bytes)
            == Some(PNG_WIDTH)
        && raw
            .get(20..24)
            .and_then(|raw| raw.try_into().ok())
            .map(u32::from_be_bytes)
            == Some(PNG_HEIGHT)
}

fn expected_lifecycle_event_count(
    calls: &[usize],
) -> Result<usize, QuestGlyphCacheTransitionRuntimeError> {
    calls.iter().try_fold(0usize, |total, calls| {
        calls
            .checked_mul(2)
            .and_then(|events| events.checked_add(1))
            .and_then(|events| total.checked_add(events))
            .ok_or_else(|| error("QUEST sparse transition 전체 event 수 overflow"))
    })
}

#[derive(Debug, Clone, Copy)]
struct CheckpointExpectation<'a> {
    label: &'a str,
    record_runtime: u32,
    page_id: u8,
    page_glyphs: usize,
    calls: usize,
}

#[derive(Debug, Clone)]
struct VerifiedCheckpoint {
    ram_sha256: String,
    gpu_sha256: String,
    state_sha256: String,
    events_sha256: String,
    object_runtime: u32,
    page_glyphs: usize,
    lifecycle: LifecycleEvidence,
}

fn verify_checkpoint(
    checkpoint: QuestGlyphCacheTransitionRuntimeCheckpoint<'_>,
    expectation: CheckpointExpectation<'_>,
    build: &QuestGlyphCacheTransitionPocBuild,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
) -> Result<VerifiedCheckpoint, QuestGlyphCacheTransitionRuntimeError> {
    if checkpoint.entry_ram.len() != PSX_MAIN_RAM_LEN
        || checkpoint.return_ram.len() != PSX_MAIN_RAM_LEN
        || checkpoint.return_gpu.len() != PSX_GPU_RAM_BYTES
    {
        return Err(error(format!(
            "QUEST {} dump 길이 불일치: entry RAM={}, return RAM={}, GPU={}",
            expectation.label,
            checkpoint.entry_ram.len(),
            checkpoint.return_ram.len(),
            checkpoint.return_gpu.len()
        )));
    }
    let entry = parse_entry_state(checkpoint.entry_state, hook.body_runtime)
        .map_err(|source| error(format!("QUEST {}: {source}", expectation.label)))?;
    let entry_active = runtime_u32(
        checkpoint.entry_ram,
        entry.object_runtime + QUEST_OBJECT_ACTIVE_OFFSET,
        "QUEST sparse transition entry object active",
    )?;
    let entry_stream = runtime_u32(
        checkpoint.entry_ram,
        entry.object_runtime + QUEST_OBJECT_STREAM_OFFSET,
        "QUEST sparse transition entry object stream",
    )?;
    let return_active = runtime_u32(
        checkpoint.return_ram,
        entry.object_runtime + QUEST_OBJECT_ACTIVE_OFFSET,
        "QUEST sparse transition return object active",
    )?;
    let return_stream = runtime_u32(
        checkpoint.return_ram,
        entry.object_runtime + QUEST_OBJECT_STREAM_OFFSET,
        "QUEST sparse transition return object stream",
    )?;
    let page_state = runtime_slice(
        checkpoint.return_ram,
        QUEST_PAGE_HOOK_STATE_RUNTIME,
        1,
        "QUEST sparse transition page state",
    )?[0];
    if entry_active != 0
        || entry_stream != expectation.record_runtime
        || return_active != 1
        || return_stream != expectation.record_runtime + 4
        || page_state != expectation.page_id
        || !font_states_inactive(checkpoint.return_ram)?
    {
        return Err(error(format!(
            "QUEST {} object/page 상태 불일치: entry active/stream={entry_active}/0x{entry_stream:08X}, return active/stream={return_active}/0x{return_stream:08X}, page={page_state}",
            expectation.label
        )));
    }

    let expected_cells = expected_page_cells(package, expectation.page_id)?;
    if expected_cells.len() != expectation.page_glyphs {
        return Err(error(format!(
            "QUEST {} page glyph 수 불일치: expected={}, actual={}",
            expectation.label,
            expectation.page_glyphs,
            expected_cells.len()
        )));
    }
    let expected_package = expected_resident_package(package, &expected_cells)?;
    if runtime_slice(
        checkpoint.return_ram,
        QUEST_PAGE_CARRIER_START_RUNTIME,
        expected_package.len(),
        "QUEST sparse transition resident package",
    )? != expected_package
        || runtime_slice(
            checkpoint.return_ram,
            hook.site.base_addr(),
            hook.site.bytes().len(),
            "QUEST sparse transition hook site",
        )? != hook.site.bytes()
        || runtime_slice(
            checkpoint.return_ram,
            hook.body_runtime,
            hook.body.bytes().len(),
            "QUEST sparse transition hook body",
        )? != hook.body.bytes()
    {
        return Err(error(format!(
            "QUEST {} resident package/site/body 불일치",
            expectation.label
        )));
    }
    for (slot, expected) in expected_cells.iter().enumerate() {
        let (x_words, y) = slot_rect(package, slot)?;
        let actual = uploaded_cell(checkpoint.return_gpu, x_words, y)?;
        if actual != *expected {
            return Err(error(format!(
                "QUEST {} VRAM cell 불일치: slot={slot}",
                expectation.label
            )));
        }
    }
    let lifecycle = verify_lifecycle(
        checkpoint.lifecycle_events,
        hook,
        entry,
        expectation.calls,
        package.layout.scratch_offset,
    )?;
    if build.report.disc.package_sha256 != package.sha256 {
        return Err(error("QUEST sparse transition build/package digest 불일치"));
    }
    Ok(VerifiedCheckpoint {
        ram_sha256: digest(checkpoint.return_ram),
        gpu_sha256: digest(checkpoint.return_gpu),
        state_sha256: digest(checkpoint.entry_state),
        events_sha256: digest(checkpoint.lifecycle_events),
        object_runtime: entry.object_runtime,
        page_glyphs: expected_cells.len(),
        lifecycle,
    })
}

fn record_runtime(offset: usize) -> Result<u32, QuestGlyphCacheTransitionRuntimeError> {
    TOWN00_SEQ_RUNTIME_BASE
        .checked_add(
            u32::try_from(offset)
                .map_err(|_| error("QUEST sparse transition record offset 범위 이탈"))?,
        )
        .ok_or_else(|| error("QUEST sparse transition record runtime overflow"))
}

/// exact sparse build의 `62 → 62 → 50 → 0` lifecycle을 검증한다.
pub fn verify_quest_glyph_cache_transition_runtime(
    build: &QuestGlyphCacheTransitionPocBuild,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    evidence: QuestGlyphCacheTransitionRuntimeEvidence<'_>,
) -> Result<QuestGlyphCacheTransitionRuntimeReport, QuestGlyphCacheTransitionRuntimeError> {
    if build.report.disc.image_sha256 != digest(&build.image)
        || build.report.disc.package_sha256 != package.sha256
        || package.sha256 != digest(&package.bytes)
        || build.report.disc.package_bytes != package.bytes.len()
        || build.report.disc.site_runtime != hook.site.base_addr()
        || build.report.disc.body_runtime != hook.body_runtime
        || build.report.disc.body_bytes != hook.body.bytes().len()
        || build.report.runtime_verified
        || build.report.release_eligible
        || hook.runtime_verified
        || hook.build_eligible
    {
        return Err(error("QUEST sparse transition runtime 입력 계약 drift"));
    }
    let [first_plan, second_plan, third_plan] = build.report.streams.as_slice() else {
        return Err(error("QUEST sparse transition runtime stream 수 불일치"));
    };
    let expectations = [
        CheckpointExpectation {
            label: "first",
            record_runtime: record_runtime(QUEST_GLYPH_CACHE_UPLOAD_POC_RECORD_OFFSET)?,
            page_id: QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID,
            page_glyphs: first_plan.page_glyphs,
            calls: first_plan.page_glyphs,
        },
        CheckpointExpectation {
            label: "second",
            record_runtime: record_runtime(QUEST_GLYPH_CACHE_TRANSITION_SECOND_RECORD_OFFSET)?,
            page_id: QUEST_GLYPH_CACHE_TRANSITION_SECOND_PAGE_ID,
            page_glyphs: second_plan.page_glyphs,
            calls: 0,
        },
        CheckpointExpectation {
            label: "third",
            record_runtime: record_runtime(QUEST_GLYPH_CACHE_TRANSITION_THIRD_RECORD_OFFSET)?,
            page_id: QUEST_GLYPH_CACHE_TRANSITION_THIRD_PAGE_ID,
            page_glyphs: third_plan.page_glyphs,
            calls: third_plan.page_glyphs,
        },
        CheckpointExpectation {
            label: "restored",
            record_runtime: record_runtime(QUEST_GLYPH_CACHE_TRANSITION_RESTORE_RECORD_OFFSET)?,
            page_id: QUEST_GLYPH_CACHE_TRANSITION_RESTORE_PAGE_ID,
            page_glyphs: package.plan.max_page_glyphs,
            calls: package.plan.max_page_glyphs,
        },
    ];
    let checkpoints = [
        evidence.first,
        evidence.second,
        evidence.third,
        evidence.restored,
    ];
    let verified = checkpoints
        .into_iter()
        .zip(expectations)
        .map(|(checkpoint, expectation)| {
            verify_checkpoint(checkpoint, expectation, build, package, hook)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let visible = [
        evidence.first_visible_png,
        evidence.first_visible_page2_png,
        evidence.first_visible_page3_png,
        evidence.second_visible_png,
        evidence.second_visible_page2_png,
        evidence.third_visible_png,
        evidence.third_visible_page2_png,
        evidence.restored_visible_png,
    ];
    let visible_capture_shapes_exact = visible.iter().all(|png| png_shape_exact(png));
    if !visible_capture_shapes_exact {
        return Err(error("QUEST sparse transition visible PNG shape 불일치"));
    }
    let page_states = expectations
        .iter()
        .map(|expectation| expectation.page_id)
        .collect::<Vec<_>>();
    let load_image_calls = verified
        .iter()
        .map(|checkpoint| checkpoint.lifecycle.load_image_calls)
        .collect::<Vec<_>>();
    let draw_sync_calls = verified
        .iter()
        .map(|checkpoint| checkpoint.lifecycle.draw_sync_calls)
        .collect::<Vec<_>>();
    let event_count = verified
        .iter()
        .map(|checkpoint| checkpoint.lifecycle.event_count)
        .sum();
    let dropped_events = verified
        .iter()
        .map(|checkpoint| checkpoint.lifecycle.dropped_events)
        .sum();
    let page_glyphs = verified
        .iter()
        .map(|checkpoint| checkpoint.page_glyphs)
        .collect::<Vec<_>>();
    let expected_page_glyphs = expectations
        .iter()
        .map(|expectation| expectation.page_glyphs)
        .collect::<Vec<_>>();
    let expected_calls = expectations
        .iter()
        .map(|expectation| expectation.calls)
        .collect::<Vec<_>>();
    let expected_event_count = expected_lifecycle_event_count(&expected_calls)?;
    let same_page_upload_skipped = load_image_calls.get(1) == Some(&0)
        && draw_sync_calls.get(1) == Some(&0)
        && page_states.get(0..2) == Some(&[62, 62]);
    let different_page_upload_exact = load_image_calls.get(2) == page_glyphs.get(2)
        && draw_sync_calls.get(2) == page_glyphs.get(2)
        && page_states.get(2) == Some(&50);
    let original_page_restored = load_image_calls.get(3) == page_glyphs.get(3)
        && draw_sync_calls.get(3) == page_glyphs.get(3)
        && page_states.get(3) == Some(&0);
    if page_states != [62, 62, 50, 0]
        || page_glyphs != expected_page_glyphs
        || load_image_calls != expected_calls
        || draw_sync_calls != expected_calls
        || event_count != expected_event_count
        || dropped_events != 0
        || !same_page_upload_skipped
        || !different_page_upload_exact
        || !original_page_restored
    {
        return Err(error("QUEST sparse transition runtime sequence 불일치"));
    }

    Ok(QuestGlyphCacheTransitionRuntimeReport {
        schema_version: QUEST_GLYPH_CACHE_TRANSITION_RUNTIME_SCHEMA_VERSION,
        image_sha256: build.report.disc.image_sha256.clone(),
        ram_sha256: verified
            .iter()
            .map(|checkpoint| checkpoint.ram_sha256.clone())
            .collect(),
        gpu_sha256: verified
            .iter()
            .map(|checkpoint| checkpoint.gpu_sha256.clone())
            .collect(),
        entry_state_sha256: verified
            .iter()
            .map(|checkpoint| checkpoint.state_sha256.clone())
            .collect(),
        lifecycle_events_sha256: verified
            .iter()
            .map(|checkpoint| checkpoint.events_sha256.clone())
            .collect(),
        visible_png_sha256: visible.iter().map(|png| digest(png)).collect(),
        record_runtimes: expectations
            .iter()
            .map(|expectation| expectation.record_runtime)
            .collect(),
        object_runtimes: verified
            .iter()
            .map(|checkpoint| checkpoint.object_runtime)
            .collect(),
        page_states,
        page_glyphs,
        load_image_calls,
        draw_sync_calls,
        event_count,
        dropped_events,
        resident_inputs_exact: true,
        uploaded_cells_exact: true,
        same_page_upload_skipped,
        different_page_upload_exact,
        original_page_restored,
        visible_capture_shapes_exact,
        runtime_verified: true,
        translation_approved: false,
        layout_approved: false,
        release_eligible: false,
    })
}

fn join<T: fmt::Display>(values: &[T]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn join_runtime(values: &[u32]) -> String {
    values
        .iter()
        .map(|value| format!("0x{value:08X}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// sparse Korean page 전환 결과를 한 행 TSV로 만든다.
pub fn render_quest_glyph_cache_transition_runtime_tsv(
    report: &QuestGlyphCacheTransitionRuntimeReport,
) -> String {
    let mut out = String::from(
        "schema_version\timage_sha256\tram_sha256\tgpu_sha256\tentry_state_sha256\t\
         lifecycle_events_sha256\tvisible_png_sha256\trecord_runtimes\tobject_runtimes\t\
         page_states\tpage_glyphs\tload_image_calls\tdraw_sync_calls\tevent_count\t\
         dropped_events\tresident_inputs_exact\tuploaded_cells_exact\t\
         same_page_upload_skipped\tdifferent_page_upload_exact\toriginal_page_restored\t\
         visible_capture_shapes_exact\truntime_verified\ttranslation_approved\t\
         layout_approved\trelease_eligible\n",
    );
    writeln!(
        out,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        report.schema_version,
        report.image_sha256,
        join(&report.ram_sha256),
        join(&report.gpu_sha256),
        join(&report.entry_state_sha256),
        join(&report.lifecycle_events_sha256),
        join(&report.visible_png_sha256),
        join_runtime(&report.record_runtimes),
        join_runtime(&report.object_runtimes),
        join(&report.page_states),
        join(&report.page_glyphs),
        join(&report.load_image_calls),
        join(&report.draw_sync_calls),
        report.event_count,
        report.dropped_events,
        report.resident_inputs_exact,
        report.uploaded_cells_exact,
        report.same_page_upload_skipped,
        report.different_page_upload_exact,
        report.original_page_restored,
        report.visible_capture_shapes_exact,
        report.runtime_verified,
        report.translation_approved,
        report.layout_approved,
        report.release_eligible,
    )
    .expect("String write는 실패하지 않음");
    out
}

#[cfg(test)]
#[path = "quest_glyph_cache_transition_runtime_tests.rs"]
mod tests;
