//! QUEST development full-disc의 최대 page 전환 실행 검증.
//!
//! exact build의 body entry에 다른 SEQ의 inline stream을 옮기고 이전 page
//! state를 설정한 synthetic 증거를 소비한다. 현재 package에서 최대 page와
//! upload 수를 다시 파생하고, entry/return RAM, LoadImage/DrawSync event,
//! GPU cell과 화면을 한 경계로 결속한다. 이 결과는 cache 전이와 최대 용량의
//! 실행 증거이며 해당 stream의 자연 게임 진행 도달이나 사람 승인을 뜻하지
//! 않는다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::compress::decompress;
use crate::coords::{psx_main_ram_offset, PSX_MAIN_RAM_LEN};
use crate::disasm::{
    QUEST_FONT_STATE_BASE_RUNTIME, QUEST_FONT_STATE_RECORD_COUNT, QUEST_FONT_STATE_RECORD_STRIDE,
    QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME, QUEST_FONT_UPLOAD_TARGET_RUNTIME,
    QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET, QUEST_HOOK_GP_RUNTIME,
    QUEST_PAGE_CARRIER_CAPACITY_BYTES, QUEST_PAGE_CARRIER_START_RUNTIME,
};
use crate::disc::{DiscImage, Iso9660};
use crate::isa::InstructionKind;
use crate::script::{parse_quest_tokens, QuestToken, TOWN00_SEQ_RUNTIME_BASE};

use super::quest_development_disc::{
    QuestDevelopmentDiscBuild, QUEST_DEVELOPMENT_DISC_SCHEMA_VERSION,
};
use super::quest_glyph_cache_hook_plan::{
    QuestGlyphCacheHookPlan, QUEST_GLYPH_CACHE_UPLOAD_ROWS, QUEST_GLYPH_CACHE_UPLOAD_WORD_WIDTH,
};
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_glyph_cache_plan::{
    QUEST_GLYPH_CACHE_CELL_BYTES, QUEST_GLYPH_CACHE_SLOT_RECT_BYTES,
};
use super::quest_manual_stream_writer::QuestManualStreamWriterAnalysis;
use super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;
use super::quest_translation_selection_routing::{
    QuestTranslationSelectionPlacement, QuestTranslationSelectionRoute,
    QuestTranslationSelectionRoutingAnalysis,
};

pub const QUEST_DEVELOPMENT_MAX_PAGE_RUNTIME_SCHEMA_VERSION: &str =
    "quest-development-max-page-runtime-v1";

const RPG_SEQ_PATH: &str = "/QUEST/RPG_SEQ.VDD;1";
const TOWN00_SEQUENCE: &str = "TOWN00.SEQ";
const SYNTHETIC_CARRIER_RECORD_OFFSET: usize = 0x21CC;
const OBJECT_ACTIVE_OFFSET: usize = 0;
const OBJECT_STREAM_OFFSET: usize = 0x1C;
const OBJECT_STREAM_FIELD_END: usize = 0x20;
const BODY_RETURN_RUNTIME: u32 = 0x8002_25E0;
const FONT_STATE_ACTIVE_OFFSET: u32 = QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET as u32 + 1;
const PSX_GPU_RAM_BYTES: usize = 1024 * 512 * 2;
const PSX_GPU_ROW_BYTES: usize = 1024 * 2;
const PNG_WIDTH: u32 = 280;
const PNG_HEIGHT: u32 = 240;

/// 최대 page 전환을 묶는 Emucap 증거.
#[derive(Debug, Clone, Copy)]
pub struct QuestDevelopmentMaxPageRuntimeEvidence<'a> {
    pub stream_id: &'a str,
    pub entry_state_json: &'a [u8],
    pub entry_ram: &'a [u8],
    pub return_state_json: &'a [u8],
    pub return_ram: &'a [u8],
    pub return_gpu: &'a [u8],
    pub lifecycle_events_json: &'a [u8],
    pub visible_png: &'a [u8],
}

/// 다른 page에서 현재 최대 page로 옮긴 실행 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentMaxPageRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub stream_id: String,
    pub sequence: String,
    pub placement: &'static str,
    pub natural_record_runtime: u32,
    pub injection_record_runtime: u32,
    pub payload_bytes: usize,
    pub payload_sha256: String,
    pub previous_page_id: u8,
    pub target_page_id: u8,
    pub max_page_glyphs: usize,
    pub object_runtime: u32,
    pub entry_state_sha256: String,
    pub entry_ram_sha256: String,
    pub return_state_sha256: String,
    pub return_ram_sha256: String,
    pub return_gpu_sha256: String,
    pub lifecycle_events_sha256: String,
    pub visible_png_sha256: String,
    pub visible_png_width: u32,
    pub visible_png_height: u32,
    pub load_image_calls: usize,
    pub draw_sync_calls: usize,
    pub event_count: usize,
    pub dropped_events: u64,
    pub disc_sequence_exact: bool,
    pub injected_town_sequence_exact: bool,
    pub body_entry_exact: bool,
    pub body_return_exact: bool,
    pub page_transition_exact: bool,
    pub resident_inputs_exact: bool,
    pub uploaded_cells_exact: bool,
    pub lifecycle_exact: bool,
    pub visible_capture_shape_exact: bool,
    pub synthetic_stream_intervention: bool,
    pub synthetic_previous_page_intervention: bool,
    pub runtime_verified: bool,
    pub natural_reach_verified: bool,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub product_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentMaxPageRuntimeError(String);

impl fmt::Display for QuestDevelopmentMaxPageRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestDevelopmentMaxPageRuntimeError {}

fn error(message: impl Into<String>) -> QuestDevelopmentMaxPageRuntimeError {
    QuestDevelopmentMaxPageRuntimeError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn runtime_slice<'a>(
    ram: &'a [u8],
    runtime: u32,
    len: usize,
    label: &str,
) -> Result<&'a [u8], QuestDevelopmentMaxPageRuntimeError> {
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
) -> Result<u32, QuestDevelopmentMaxPageRuntimeError> {
    let raw: [u8; 4] = runtime_slice(ram, runtime, 4, label)?
        .try_into()
        .map_err(|_| error(format!("{label}: u32 길이 불일치")))?;
    Ok(u32::from_le_bytes(raw))
}

fn json_u32(
    value: &Value,
    key: &str,
    label: &str,
) -> Result<u32, QuestDevelopmentMaxPageRuntimeError> {
    let raw = value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("QUEST 최대 page state에 {label}가 없음")))?;
    u32::try_from(raw).map_err(|_| error(format!("QUEST 최대 page {label} 범위 이탈")))
}

fn event_register(
    value: &Value,
    register: &str,
    label: &str,
) -> Result<u32, QuestDevelopmentMaxPageRuntimeError> {
    let raw = value
        .get("registers")
        .and_then(|registers| registers.get(register))
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("QUEST 최대 page event에 {label}가 없음")))?;
    u32::try_from(raw).map_err(|_| error(format!("QUEST 최대 page event {label} 범위 이탈")))
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
) -> Result<EntryState, QuestDevelopmentMaxPageRuntimeError> {
    let state: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST 최대 page entry state JSON 실패: {source}")))?;
    let pc = json_u32(&state, "g0.PC", "entry PC")?;
    let entry = EntryState {
        object_runtime: json_u32(&state, "g0.a0", "entry a0")?,
        gp: json_u32(&state, "g0.gp", "entry gp")?,
        ra: json_u32(&state, "g0.ra", "entry ra")?,
        sp: json_u32(&state, "g0.sp", "entry sp")?,
    };
    if pc != body_runtime || entry.gp != QUEST_HOOK_GP_RUNTIME || entry.ra != BODY_RETURN_RUNTIME {
        return Err(error("QUEST 최대 page body entry register 계약 불일치"));
    }
    psx_main_ram_offset(entry.object_runtime, OBJECT_STREAM_FIELD_END)
        .ok_or_else(|| error("QUEST 최대 page object가 PS1 main RAM 밖임"))?;
    Ok(entry)
}

fn verify_return_state(
    raw: &[u8],
    entry: EntryState,
) -> Result<(), QuestDevelopmentMaxPageRuntimeError> {
    let state: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST 최대 page return state JSON 실패: {source}")))?;
    if json_u32(&state, "g0.PC", "return PC")? != BODY_RETURN_RUNTIME
        || json_u32(&state, "g0.a0", "return a0")? != entry.object_runtime
        || json_u32(&state, "g0.gp", "return gp")? != entry.gp
        || json_u32(&state, "g0.ra", "return ra")? != entry.ra
        || json_u32(&state, "g0.sp", "return sp")? != entry.sp
    {
        return Err(error("QUEST 최대 page body return register 계약 불일치"));
    }
    Ok(())
}

fn png_dimensions(raw: &[u8]) -> Option<(u32, u32)> {
    if raw.get(..8) != Some(b"\x89PNG\r\n\x1a\n") || raw.get(12..16) != Some(b"IHDR") {
        return None;
    }
    Some((
        u32::from_be_bytes(raw.get(16..20)?.try_into().ok()?),
        u32::from_be_bytes(raw.get(20..24)?.try_into().ok()?),
    ))
}

fn checked_runtime(
    base: u32,
    offset: usize,
    label: &str,
) -> Result<u32, QuestDevelopmentMaxPageRuntimeError> {
    base.checked_add(
        u32::try_from(offset).map_err(|_| error(format!("{label}: offset u32 범위 이탈")))?,
    )
    .ok_or_else(|| error(format!("{label}: runtime overflow")))
}

fn exact_disc_sequence(
    build: &QuestDevelopmentDiscBuild,
    sequence_name: &str,
    expected: &[u8],
) -> Result<(), QuestDevelopmentMaxPageRuntimeError> {
    let image = DiscImage::from_bytes(build.image.clone())
        .map_err(|source| error(format!("QUEST 최대 page disc 파싱 실패: {source}")))?;
    let iso = Iso9660::parse(&image)
        .map_err(|source| error(format!("QUEST 최대 page ISO 파싱 실패: {source}")))?;
    let vdd_raw = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|source| error(format!("QUEST 최대 page VDD 읽기 실패: {source}")))?;
    let archive = VddArchive::parse(&vdd_raw)
        .map_err(|source| error(format!("QUEST 최대 page VDD 파싱 실패: {source}")))?;
    let entry = archive.find(sequence_name).ok_or_else(|| {
        error(format!(
            "QUEST 최대 page disc에 {sequence_name} 엔트리가 없음"
        ))
    })?;
    let actual = decompress(archive.read_file(entry))
        .map_err(|source| error(format!("{sequence_name}: CNX 해제 실패: {source}")))?;
    if actual != expected {
        return Err(error(format!(
            "{sequence_name}: exact disc와 writer sequence 불일치"
        )));
    }
    Ok(())
}

fn max_page_glyphs(
    package: &QuestGlyphCachePackage,
) -> Result<usize, QuestDevelopmentMaxPageRuntimeError> {
    let mut maximum = 0usize;
    for page in 1..=package.plan.pages {
        let page_id =
            u8::try_from(page).map_err(|_| error("QUEST 최대 page directory가 u8 ID를 넘음"))?;
        maximum = maximum.max(
            package
                .page_catalog_indices(page_id)
                .map_err(|source| error(source.to_string()))?
                .len(),
        );
    }
    if maximum == 0 || maximum != package.plan.max_page_glyphs {
        return Err(error("QUEST 최대 page glyph 파생값 drift"));
    }
    Ok(maximum)
}

fn route_payload<'a>(
    writer: &'a QuestManualStreamWriterAnalysis,
    route: &QuestTranslationSelectionRoute,
) -> Result<&'a [u8], QuestDevelopmentMaxPageRuntimeError> {
    if route.placement != QuestTranslationSelectionPlacement::InlineCapacityCandidate {
        return Err(error(
            "QUEST 최대 page synthetic 표본은 inline route여야 함",
        ));
    }
    let bytes = route
        .projected_stream_bytes
        .ok_or_else(|| error("QUEST 최대 page route projected 길이 누락"))?;
    let sequence = writer
        .sequences
        .iter()
        .find(|sequence| sequence.sequence == route.sequence)
        .ok_or_else(|| error(format!("{}: writer sequence 누락", route.sequence)))?;
    let end = route
        .record_offset
        .checked_add(bytes)
        .ok_or_else(|| error("QUEST 최대 page route payload end overflow"))?;
    sequence
        .output
        .get(route.record_offset..end)
        .ok_or_else(|| error("QUEST 최대 page route payload 범위 이탈"))
}

fn expected_page_cells(
    package: &QuestGlyphCachePackage,
    page_id: u8,
) -> Result<Vec<Vec<u8>>, QuestDevelopmentMaxPageRuntimeError> {
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
) -> Result<Vec<u8>, QuestDevelopmentMaxPageRuntimeError> {
    let final_cell = expected_cells
        .last()
        .ok_or_else(|| error("QUEST 최대 page cell이 비어 있음"))?;
    if final_cell.len() != package.layout.scratch_bytes {
        return Err(error("QUEST 최대 page 마지막 cell과 scratch 길이 불일치"));
    }
    let mut expected = package.bytes.clone();
    let end = package
        .layout
        .scratch_offset
        .checked_add(final_cell.len())
        .ok_or_else(|| error("QUEST 최대 page scratch end overflow"))?;
    expected
        .get_mut(package.layout.scratch_offset..end)
        .ok_or_else(|| error("QUEST 최대 page scratch 범위 이탈"))?
        .copy_from_slice(final_cell);
    Ok(expected)
}

fn carrier_inputs_exact(
    ram: &[u8],
    package: &[u8],
    hook: &QuestGlyphCacheHookPlan,
) -> Result<bool, QuestDevelopmentMaxPageRuntimeError> {
    if runtime_slice(
        ram,
        QUEST_PAGE_CARRIER_START_RUNTIME,
        package.len(),
        "QUEST 최대 page resident package",
    )? != package
        || runtime_slice(
            ram,
            hook.site.base_addr(),
            hook.site.bytes().len(),
            "QUEST 최대 page hook site",
        )? != hook.site.bytes()
        || runtime_slice(
            ram,
            hook.body_runtime,
            hook.body.bytes().len(),
            "QUEST 최대 page hook body",
        )? != hook.body.bytes()
    {
        return Ok(false);
    }
    let carrier_end = checked_runtime(
        QUEST_PAGE_CARRIER_START_RUNTIME,
        QUEST_PAGE_CARRIER_CAPACITY_BYTES,
        "QUEST 최대 page carrier end",
    )?;
    let suffix_runtime = checked_runtime(
        hook.body_runtime,
        hook.body.bytes().len(),
        "QUEST 최대 page body end",
    )?;
    let suffix_bytes = usize::try_from(
        carrier_end
            .checked_sub(suffix_runtime)
            .ok_or_else(|| error("QUEST 최대 page body가 carrier를 넘음"))?,
    )
    .map_err(|_| error("QUEST 최대 page carrier suffix 범위 이탈"))?;
    Ok(runtime_slice(
        ram,
        suffix_runtime,
        suffix_bytes,
        "QUEST 최대 page carrier suffix",
    )?
    .iter()
    .all(|byte| *byte == 0))
}

fn font_states_inactive(ram: &[u8]) -> Result<bool, QuestDevelopmentMaxPageRuntimeError> {
    for index in 0..QUEST_FONT_STATE_RECORD_COUNT {
        let runtime = checked_runtime(
            QUEST_FONT_STATE_BASE_RUNTIME,
            index
                .checked_mul(QUEST_FONT_STATE_RECORD_STRIDE)
                .and_then(|offset| offset.checked_add(FONT_STATE_ACTIVE_OFFSET as usize))
                .ok_or_else(|| error("QUEST 최대 page font state offset overflow"))?,
            "QUEST 최대 page font active",
        )?;
        if runtime_slice(ram, runtime, 1, "QUEST 최대 page font active")? != [0] {
            return Ok(false);
        }
    }
    Ok(true)
}

fn slot_rect(
    package: &QuestGlyphCachePackage,
    slot: usize,
) -> Result<(usize, usize), QuestDevelopmentMaxPageRuntimeError> {
    let start = package
        .layout
        .slot_rects_offset
        .checked_add(
            slot.checked_mul(QUEST_GLYPH_CACHE_SLOT_RECT_BYTES)
                .ok_or_else(|| error("QUEST 최대 page RECT offset overflow"))?,
        )
        .ok_or_else(|| error("QUEST 최대 page RECT start overflow"))?;
    let end = start
        .checked_add(QUEST_GLYPH_CACHE_SLOT_RECT_BYTES)
        .ok_or_else(|| error("QUEST 최대 page RECT end overflow"))?;
    let raw = package
        .bytes
        .get(start..end)
        .ok_or_else(|| error("QUEST 최대 page RECT package 범위 이탈"))?;
    Ok((
        usize::from(u16::from_le_bytes([raw[0], raw[1]])),
        usize::from(u16::from_le_bytes([raw[2], raw[3]])),
    ))
}

fn uploaded_cell(
    gpu: &[u8],
    x_words: usize,
    y: usize,
) -> Result<Vec<u8>, QuestDevelopmentMaxPageRuntimeError> {
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
            .ok_or_else(|| error("QUEST 최대 page VRAM cell offset overflow"))?;
        let end = start
            .checked_add(row_bytes)
            .ok_or_else(|| error("QUEST 최대 page VRAM cell end overflow"))?;
        cell.extend_from_slice(
            gpu.get(start..end)
                .ok_or_else(|| error("QUEST 최대 page VRAM cell 범위 이탈"))?,
        );
    }
    if cell.len() != QUEST_GLYPH_CACHE_CELL_BYTES {
        return Err(error("QUEST 최대 page VRAM cell 길이 불일치"));
    }
    Ok(cell)
}

fn call_return(
    hook: &QuestGlyphCacheHookPlan,
    target: u32,
    label: &str,
) -> Result<u32, QuestDevelopmentMaxPageRuntimeError> {
    hook.body
        .instructions()
        .iter()
        .find(|instruction| instruction.kind == InstructionKind::Jal { target })
        .map(|instruction| instruction.addr + 8)
        .ok_or_else(|| error(format!("QUEST 최대 page body에 {label} call이 없음")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LifecycleEvidence {
    event_count: usize,
    dropped_events: u64,
    load_image_calls: usize,
    draw_sync_calls: usize,
}

fn verify_lifecycle(
    raw: &[u8],
    hook: &QuestGlyphCacheHookPlan,
    package: &QuestGlyphCachePackage,
    entry: EntryState,
    page_id: u8,
    expected_calls: usize,
) -> Result<LifecycleEvidence, QuestDevelopmentMaxPageRuntimeError> {
    let document: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST 최대 page event JSON 실패: {source}")))?;
    let dropped_events = document
        .get("dropped")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("QUEST 최대 page event JSON에 dropped가 없음"))?;
    let events = document
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| error("QUEST 최대 page event JSON에 events가 없음"))?;
    let expected_event_count = expected_calls
        .checked_mul(2)
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| error("QUEST 최대 page event 수 overflow"))?;
    if dropped_events != 0 || events.len() != expected_event_count {
        return Err(error(format!(
            "QUEST 최대 page event 수 불일치: dropped={dropped_events}, expected={expected_event_count}, actual={}",
            events.len()
        )));
    }

    let upload_return = call_return(hook, QUEST_FONT_UPLOAD_TARGET_RUNTIME, "LoadImage")?;
    let sync_return = call_return(hook, QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME, "DrawSync")?;
    let scratch_runtime = checked_runtime(
        QUEST_PAGE_CARRIER_START_RUNTIME,
        package.layout.scratch_offset,
        "QUEST 최대 page scratch",
    )?;
    for index in 0..expected_calls {
        let upload = &events[index * 2];
        let sync = &events[index * 2 + 1];
        if json_u32(upload, "pc", "upload pc")? != QUEST_FONT_UPLOAD_TARGET_RUNTIME
            || event_register(upload, "ra", "upload ra")? != upload_return
            || event_register(upload, "a1", "upload a1")? != scratch_runtime
            || event_register(upload, "s1", "upload s1")? != u32::from(page_id)
            || usize::try_from(event_register(upload, "s5", "upload s5")?)
                .map_err(|_| error("QUEST 최대 page upload s5 범위 이탈"))?
                != index
            || json_u32(sync, "pc", "sync pc")? != QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME
            || event_register(sync, "ra", "sync ra")? != sync_return
            || event_register(sync, "a0", "sync a0")? != 0
            || event_register(sync, "s1", "sync s1")? != u32::from(page_id)
            || usize::try_from(event_register(sync, "s5", "sync s5")?)
                .map_err(|_| error("QUEST 최대 page sync s5 범위 이탈"))?
                != index
        {
            return Err(error(format!(
                "QUEST 최대 page upload/sync 순서 불일치: index={index}"
            )));
        }
    }
    let returned = events
        .last()
        .ok_or_else(|| error("QUEST 최대 page return event가 없음"))?;
    if json_u32(returned, "pc", "return pc")? != BODY_RETURN_RUNTIME
        || event_register(returned, "a0", "return a0")? != entry.object_runtime
        || event_register(returned, "gp", "return gp")? != entry.gp
        || event_register(returned, "ra", "return ra")? != entry.ra
        || event_register(returned, "sp", "return sp")? != entry.sp
    {
        return Err(error("QUEST 최대 page return event register 계약 불일치"));
    }
    Ok(LifecycleEvidence {
        event_count: events.len(),
        dropped_events,
        load_image_calls: expected_calls,
        draw_sync_calls: expected_calls,
    })
}

/// exact full-disc에서 synthetic 다른 page→최대 page 전이를 검증한다.
pub fn verify_quest_development_max_page_runtime(
    build: &QuestDevelopmentDiscBuild,
    writer: &QuestManualStreamWriterAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    evidence: QuestDevelopmentMaxPageRuntimeEvidence<'_>,
) -> Result<QuestDevelopmentMaxPageRuntimeReport, QuestDevelopmentMaxPageRuntimeError> {
    let QuestDevelopmentMaxPageRuntimeEvidence {
        stream_id,
        entry_state_json,
        entry_ram,
        return_state_json,
        return_ram,
        return_gpu,
        lifecycle_events_json,
        visible_png,
    } = evidence;
    let disc = &build.report.disc;
    if build.report.schema_version != QUEST_DEVELOPMENT_DISC_SCHEMA_VERSION
        || digest(&build.image) != disc.image_sha256
        || disc.package_sha256 != package.sha256
        || disc.package_bytes != package.bytes.len()
        || disc.site_runtime != hook.site.base_addr()
        || disc.body_runtime != hook.body_runtime
        || disc.body_bytes != hook.body.bytes().len()
        || build.report.sequence_entries != writer.sequences.len()
        || build.report.selected_routes != writer.selected_routes
        || build.report.retained_source_routes != writer.retained_source_routes
        || build.report.runtime_verified
        || build.report.product_eligible
        || hook.runtime_verified
        || hook.build_eligible
    {
        return Err(error("QUEST 최대 page runtime 정적 입력 계약 drift"));
    }
    if entry_ram.len() != PSX_MAIN_RAM_LEN
        || return_ram.len() != PSX_MAIN_RAM_LEN
        || return_gpu.len() != PSX_GPU_RAM_BYTES
    {
        return Err(error(format!(
            "QUEST 최대 page dump 길이 불일치: entry RAM={}, return RAM={}, GPU={}",
            entry_ram.len(),
            return_ram.len(),
            return_gpu.len()
        )));
    }

    let route = routing
        .routes
        .iter()
        .find(|route| route.stream_id == stream_id)
        .ok_or_else(|| error(format!("QUEST 최대 page stream ID가 없음: {stream_id}")))?;
    let payload = route_payload(writer, route)?;
    let maximum = max_page_glyphs(package)?;
    let expected_cells = expected_page_cells(package, route.page_id)?;
    if expected_cells.len() != maximum
        || route.routed_record.get(1) != Some(&route.page_id)
        || payload.get(..4) != Some(route.routed_record.as_slice())
        || route.record_offset % 4 != SYNTHETIC_CARRIER_RECORD_OFFSET % 4
    {
        return Err(error("QUEST 최대 page route/package 결속 drift"));
    }

    let sequence = writer
        .sequences
        .iter()
        .find(|sequence| sequence.sequence == route.sequence)
        .ok_or_else(|| error(format!("{}: writer sequence 누락", route.sequence)))?;
    let town = writer
        .sequences
        .iter()
        .find(|sequence| sequence.sequence == TOWN00_SEQUENCE)
        .ok_or_else(|| error("QUEST 최대 page TOWN00 writer sequence 누락"))?;
    let carrier = routing
        .routes
        .iter()
        .find(|candidate| {
            candidate.sequence == TOWN00_SEQUENCE
                && candidate.record_offset == SYNTHETIC_CARRIER_RECORD_OFFSET
        })
        .ok_or_else(|| error("QUEST 최대 page synthetic carrier route 누락"))?;
    if payload.len() > carrier.source_stream_bytes {
        return Err(error("QUEST 최대 page payload가 synthetic carrier를 넘음"));
    }
    exact_disc_sequence(build, &route.sequence, &sequence.output)?;
    exact_disc_sequence(build, TOWN00_SEQUENCE, &town.output)?;

    let natural_record_runtime = checked_runtime(
        TOWN00_SEQ_RUNTIME_BASE,
        route.record_offset,
        "QUEST 최대 page natural record",
    )?;
    let injection_record_runtime = checked_runtime(
        TOWN00_SEQ_RUNTIME_BASE,
        SYNTHETIC_CARRIER_RECORD_OFFSET,
        "QUEST 최대 page injection record",
    )?;
    if natural_record_runtime == injection_record_runtime {
        return Err(error("QUEST 최대 page 표본이 synthetic stream 이동이 아님"));
    }
    let injection_body_runtime = checked_runtime(
        injection_record_runtime,
        4,
        "QUEST 최대 page injection body",
    )?;
    let parsed = parse_quest_tokens(
        payload
            .get(4..)
            .ok_or_else(|| error("QUEST 최대 page payload에 body가 없음"))?,
        injection_body_runtime,
    )
    .map_err(|source| error(format!("QUEST 최대 page 이동 body 재파싱 실패: {source}")))?;
    let glyphs = parsed
        .iter()
        .filter(|token| matches!(token, QuestToken::Glyph(_)))
        .count();
    if Some(glyphs) != route.selected_glyphs
        || !matches!(parsed.last(), Some(QuestToken::Control { spec, .. }) if spec.code == 0x03)
    {
        return Err(error("QUEST 최대 page 이동 body glyph/terminal drift"));
    }

    let mut expected_town = town.output.clone();
    let injection_end = SYNTHETIC_CARRIER_RECORD_OFFSET
        .checked_add(payload.len())
        .ok_or_else(|| error("QUEST 최대 page injection end overflow"))?;
    expected_town
        .get_mut(SYNTHETIC_CARRIER_RECORD_OFFSET..injection_end)
        .ok_or_else(|| error("QUEST 최대 page injection이 TOWN00 범위를 넘음"))?
        .copy_from_slice(payload);
    let entry_town = runtime_slice(
        entry_ram,
        TOWN00_SEQ_RUNTIME_BASE,
        expected_town.len(),
        "QUEST 최대 page entry TOWN00",
    )?;
    let return_town = runtime_slice(
        return_ram,
        TOWN00_SEQ_RUNTIME_BASE,
        expected_town.len(),
        "QUEST 최대 page return TOWN00",
    )?;
    if entry_town != expected_town || return_town != expected_town {
        return Err(error("QUEST 최대 page synthetic TOWN00 sequence 불일치"));
    }

    let entry = parse_entry_state(entry_state_json, hook.body_runtime)?;
    verify_return_state(return_state_json, entry)?;
    let object_active_runtime = checked_runtime(
        entry.object_runtime,
        OBJECT_ACTIVE_OFFSET,
        "QUEST 최대 page object active",
    )?;
    let object_stream_runtime = checked_runtime(
        entry.object_runtime,
        OBJECT_STREAM_OFFSET,
        "QUEST 최대 page object stream",
    )?;
    let entry_active = runtime_u32(
        entry_ram,
        object_active_runtime,
        "QUEST 최대 page entry object active",
    )?;
    let entry_stream = runtime_u32(
        entry_ram,
        object_stream_runtime,
        "QUEST 최대 page entry object stream",
    )?;
    let return_active = runtime_u32(
        return_ram,
        object_active_runtime,
        "QUEST 최대 page return object active",
    )?;
    let return_stream = runtime_u32(
        return_ram,
        object_stream_runtime,
        "QUEST 최대 page return object stream",
    )?;
    let expected_return_stream =
        checked_runtime(injection_record_runtime, 4, "QUEST 최대 page return stream")?;
    let previous_page_id = runtime_slice(
        entry_ram,
        QUEST_PAGE_HOOK_STATE_RUNTIME,
        1,
        "QUEST 최대 page entry page state",
    )?[0];
    let target_page_id = runtime_slice(
        return_ram,
        QUEST_PAGE_HOOK_STATE_RUNTIME,
        1,
        "QUEST 최대 page return page state",
    )?[0];
    if entry_active != 0
        || entry_stream != injection_record_runtime
        || return_active != 1
        || return_stream != expected_return_stream
        || previous_page_id == 0
        || previous_page_id == route.page_id
        || target_page_id != route.page_id
        || !font_states_inactive(entry_ram)?
        || !font_states_inactive(return_ram)?
    {
        return Err(error(format!(
            "QUEST 최대 page object/page 상태 불일치: entry active/stream={entry_active}/0x{entry_stream:08X}, return active/stream={return_active}/0x{return_stream:08X}, page={previous_page_id}->{target_page_id}"
        )));
    }

    let expected_return_package = expected_resident_package(package, &expected_cells)?;
    let entry_carrier_exact = carrier_inputs_exact(entry_ram, &package.bytes, hook)?;
    let return_carrier_exact = carrier_inputs_exact(return_ram, &expected_return_package, hook)?;
    if !entry_carrier_exact || !return_carrier_exact {
        return Err(error(
            "QUEST 최대 page resident package/site/body/suffix 불일치",
        ));
    }
    for (slot, expected) in expected_cells.iter().enumerate() {
        let (x_words, y) = slot_rect(package, slot)?;
        let actual = uploaded_cell(return_gpu, x_words, y)?;
        if actual != *expected {
            return Err(error(format!(
                "QUEST 최대 page VRAM cell 불일치: slot={slot}"
            )));
        }
    }
    let lifecycle = verify_lifecycle(
        lifecycle_events_json,
        hook,
        package,
        entry,
        route.page_id,
        maximum,
    )?;

    let (visible_png_width, visible_png_height) = png_dimensions(visible_png)
        .ok_or_else(|| error("QUEST 최대 page visible PNG header 불일치"))?;
    let visible_capture_shape_exact =
        visible_png_width == PNG_WIDTH && visible_png_height == PNG_HEIGHT;
    if !visible_capture_shape_exact {
        return Err(error(format!(
            "QUEST 최대 page visible PNG 크기 불일치: expected={PNG_WIDTH}x{PNG_HEIGHT}, actual={visible_png_width}x{visible_png_height}"
        )));
    }

    Ok(QuestDevelopmentMaxPageRuntimeReport {
        schema_version: QUEST_DEVELOPMENT_MAX_PAGE_RUNTIME_SCHEMA_VERSION,
        image_sha256: disc.image_sha256.clone(),
        stream_id: route.stream_id.clone(),
        sequence: route.sequence.clone(),
        placement: route.placement.label(),
        natural_record_runtime,
        injection_record_runtime,
        payload_bytes: payload.len(),
        payload_sha256: digest(payload),
        previous_page_id,
        target_page_id,
        max_page_glyphs: maximum,
        object_runtime: entry.object_runtime,
        entry_state_sha256: digest(entry_state_json),
        entry_ram_sha256: digest(entry_ram),
        return_state_sha256: digest(return_state_json),
        return_ram_sha256: digest(return_ram),
        return_gpu_sha256: digest(return_gpu),
        lifecycle_events_sha256: digest(lifecycle_events_json),
        visible_png_sha256: digest(visible_png),
        visible_png_width,
        visible_png_height,
        load_image_calls: lifecycle.load_image_calls,
        draw_sync_calls: lifecycle.draw_sync_calls,
        event_count: lifecycle.event_count,
        dropped_events: lifecycle.dropped_events,
        disc_sequence_exact: true,
        injected_town_sequence_exact: true,
        body_entry_exact: true,
        body_return_exact: true,
        page_transition_exact: true,
        resident_inputs_exact: true,
        uploaded_cells_exact: true,
        lifecycle_exact: true,
        visible_capture_shape_exact,
        synthetic_stream_intervention: true,
        synthetic_previous_page_intervention: true,
        runtime_verified: true,
        natural_reach_verified: false,
        translation_approved: false,
        layout_approved: false,
        product_eligible: false,
    })
}

/// 최대 page 전환 결과를 한 행 TSV로 만든다.
pub fn render_quest_development_max_page_runtime_tsv(
    report: &QuestDevelopmentMaxPageRuntimeReport,
) -> String {
    let mut output = String::from(
        "schema_version\timage_sha256\tstream_id\tsequence\tplacement\t\
         natural_record_runtime\tinjection_record_runtime\tpayload_bytes\tpayload_sha256\t\
         previous_page_id\ttarget_page_id\tmax_page_glyphs\tobject_runtime\t\
         entry_state_sha256\tentry_ram_sha256\treturn_state_sha256\treturn_ram_sha256\t\
         return_gpu_sha256\tlifecycle_events_sha256\tvisible_png_sha256\t\
         visible_png_width\tvisible_png_height\tload_image_calls\tdraw_sync_calls\t\
         event_count\tdropped_events\tdisc_sequence_exact\tinjected_town_sequence_exact\t\
         body_entry_exact\tbody_return_exact\tpage_transition_exact\tresident_inputs_exact\t\
         uploaded_cells_exact\tlifecycle_exact\tvisible_capture_shape_exact\t\
         synthetic_stream_intervention\tsynthetic_previous_page_intervention\t\
         runtime_verified\tnatural_reach_verified\ttranslation_approved\tlayout_approved\t\
         product_eligible\n",
    );
    writeln!(
        output,
        "{}\t{}\t{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        report.schema_version,
        report.image_sha256,
        report.stream_id,
        report.sequence,
        report.placement,
        report.natural_record_runtime,
        report.injection_record_runtime,
        report.payload_bytes,
        report.payload_sha256,
        report.previous_page_id,
        report.target_page_id,
        report.max_page_glyphs,
        report.object_runtime,
        report.entry_state_sha256,
        report.entry_ram_sha256,
        report.return_state_sha256,
        report.return_ram_sha256,
        report.return_gpu_sha256,
        report.lifecycle_events_sha256,
        report.visible_png_sha256,
        report.visible_png_width,
        report.visible_png_height,
        report.load_image_calls,
        report.draw_sync_calls,
        report.event_count,
        report.dropped_events,
        report.disc_sequence_exact,
        report.injected_town_sequence_exact,
        report.body_entry_exact,
        report.body_return_exact,
        report.page_transition_exact,
        report.resident_inputs_exact,
        report.uploaded_cells_exact,
        report.lifecycle_exact,
        report.visible_capture_shape_exact,
        report.synthetic_stream_intervention,
        report.synthetic_previous_page_intervention,
        report.runtime_verified,
        report.natural_reach_verified,
        report.translation_approved,
        report.layout_approved,
        report.product_eligible,
    )
    .expect("String write는 실패하지 않음");
    output
}

#[cfg(test)]
#[path = "quest_development_max_page_runtime_tests.rs"]
mod tests;
