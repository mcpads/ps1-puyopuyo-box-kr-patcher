//! QUEST 두 대사 공용 page와 다음 원본 page 복귀의 runtime 검증.
//!
//! exact 두 대사 PoC를 다시 만들고 첫 Korean record, 같은 page를 쓰는 두
//! 번째 Korean record, 다음 비대상 record의 세 checkpoint를 결속한다.
//! RAM·GPU와 hit-time event를 함께 검사해 upload/skip/restore 순서를 닫고,
//! 화면 PNG는 구조와 digest만 묶는다. 문장·layout 승인은 사람이 별도로 한다.

use std::error::Error;
use std::fmt;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::coords::{exe_map, psx_main_ram_offset, PSX_MAIN_RAM_LEN};
use crate::disasm::{
    verify_box_quest_hook_space_candidates, QUEST_FONT_STATE_BASE_RUNTIME,
    QUEST_FONT_STATE_RECORD_COUNT, QUEST_FONT_STATE_RECORD_STRIDE,
    QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME, QUEST_FONT_UPLOAD_TARGET_RUNTIME,
    QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET, QUEST_HOOK_GP_RUNTIME,
    QUEST_PAGE_CARRIER_START_RUNTIME,
};
use crate::disc::{Iso9660, PsxExe, VerifiedSourceImage};
use crate::script::TOWN00_SEQ_RUNTIME_BASE;

use super::quest_hook_poc::{verify_quest_hook_noop_sources, QUEST_HOOK_BODY_RUNTIME};
use super::quest_page_first_dialogue_poc::{
    assemble_two_dialogue_transition_body, build_quest_page_two_dialogues, uploaded_font_page,
    BOX_QUEST_PAGE_TWO_DIALOGUES_IMAGE_SHA256, QUEST_PAGE_FIRST_DIALOGUE_PAGE_ID,
    QUEST_PAGE_FIRST_DIALOGUE_RECORD_OFFSET, QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME,
    QUEST_PAGE_RESTORE_PAGE_ID, QUEST_PAGE_RESTORE_SOURCE_RUNTIME,
    QUEST_PAGE_SECOND_DIALOGUE_RECORD_OFFSET, QUEST_PAGE_SECOND_DIALOGUE_RECORD_RUNTIME,
};
use super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;
use super::quest_poc::prepare_quest_two_dialogue_assets;
use super::{QuestSecondDialogueAssets, QUEST_EXE_ISO_PATH};

pub const QUEST_PAGE_TWO_DIALOGUES_RUNTIME_SCHEMA_VERSION: &str =
    "quest-page-two-dialogues-runtime-v1";
pub const QUEST_PAGE_TWO_DIALOGUES_RESTORE_RECORD_OFFSET: u32 = 0x362C;
pub const QUEST_PAGE_TWO_DIALOGUES_RESTORE_RECORD_RUNTIME: u32 =
    TOWN00_SEQ_RUNTIME_BASE + QUEST_PAGE_TWO_DIALOGUES_RESTORE_RECORD_OFFSET;

const QUEST_PAGE_BODY_RETURN_RUNTIME: u32 = 0x8002_25E0;
const QUEST_OBJECT_ACTIVE_OFFSET: u32 = 0;
const QUEST_OBJECT_STREAM_OFFSET: u32 = 0x1C;
const QUEST_FONT_STATE_ACTIVE_OFFSET: u32 = QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET as u32 + 1;
const PNG_WIDTH: u32 = 280;
const PNG_HEIGHT: u32 = 240;

/// record 하나의 body return checkpoint와 그 앞뒤 event.
#[derive(Debug, Clone, Copy)]
pub struct QuestPageTwoDialoguesRuntimeCheckpoint<'a> {
    pub ram: &'a [u8],
    pub gpu: &'a [u8],
    pub entry_events: &'a [u8],
    pub lifecycle_events: &'a [u8],
}

/// 세 record와 네 visible 화면을 묶은 runtime 증거.
#[derive(Debug, Clone, Copy)]
pub struct QuestPageTwoDialoguesRuntimeEvidence<'a> {
    pub first: QuestPageTwoDialoguesRuntimeCheckpoint<'a>,
    pub second: QuestPageTwoDialoguesRuntimeCheckpoint<'a>,
    pub restored: QuestPageTwoDialoguesRuntimeCheckpoint<'a>,
    pub first_visible_png: &'a [u8],
    pub second_visible_png: &'a [u8],
    pub second_visible_page2_png: &'a [u8],
    pub restored_visible_png: &'a [u8],
}

/// upload/skip/restore runtime 검증 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPageTwoDialoguesRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub first_ram_sha256: String,
    pub first_gpu_sha256: String,
    pub second_ram_sha256: String,
    pub second_gpu_sha256: String,
    pub restored_ram_sha256: String,
    pub restored_gpu_sha256: String,
    pub first_entry_events_sha256: String,
    pub first_lifecycle_events_sha256: String,
    pub second_entry_events_sha256: String,
    pub second_lifecycle_events_sha256: String,
    pub restored_entry_events_sha256: String,
    pub restored_lifecycle_events_sha256: String,
    pub first_visible_sha256: String,
    pub second_visible_sha256: String,
    pub second_visible_page2_sha256: String,
    pub restored_visible_sha256: String,
    pub first_record_runtime: u32,
    pub second_record_runtime: u32,
    pub restored_record_runtime: u32,
    pub first_object_runtime: u32,
    pub second_object_runtime: u32,
    pub restored_object_runtime: u32,
    pub body_runtime: u32,
    pub body_bytes: usize,
    pub first_page_state: u8,
    pub second_page_state: u8,
    pub restored_page_state: u8,
    pub first_upload_calls: usize,
    pub second_upload_calls: usize,
    pub restored_upload_calls: usize,
    pub first_sync_calls: usize,
    pub second_sync_calls: usize,
    pub restored_sync_calls: usize,
    pub event_count: usize,
    pub dropped_events: u64,
    pub resident_inputs_exact: bool,
    pub first_page_exact: bool,
    pub second_page_exact: bool,
    pub restored_page_exact: bool,
    pub second_upload_skipped: bool,
    pub visible_capture_shapes_exact: bool,
    pub runtime_verified: bool,
    pub layout_approved: bool,
    pub release_eligible: bool,
}

/// runtime 결과를 한 행 TSV로 만든다.
pub fn render_quest_page_two_dialogues_runtime_tsv(
    report: &QuestPageTwoDialoguesRuntimeReport,
) -> String {
    format!(
        "schema_version\timage_sha256\tfirst_ram_sha256\tfirst_gpu_sha256\t\
         second_ram_sha256\tsecond_gpu_sha256\trestored_ram_sha256\t\
         restored_gpu_sha256\tfirst_entry_events_sha256\t\
         first_lifecycle_events_sha256\tsecond_entry_events_sha256\t\
         second_lifecycle_events_sha256\trestored_entry_events_sha256\t\
         restored_lifecycle_events_sha256\tfirst_visible_sha256\t\
         second_visible_sha256\tsecond_visible_page2_sha256\t\
         restored_visible_sha256\tfirst_record_runtime\tsecond_record_runtime\t\
         restored_record_runtime\tfirst_object_runtime\tsecond_object_runtime\t\
         restored_object_runtime\tbody_runtime\tbody_bytes\tfirst_page_state\t\
         second_page_state\trestored_page_state\tfirst_upload_calls\t\
         second_upload_calls\trestored_upload_calls\tfirst_sync_calls\t\
         second_sync_calls\trestored_sync_calls\tevent_count\tdropped_events\t\
         resident_inputs_exact\tfirst_page_exact\tsecond_page_exact\t\
         restored_page_exact\tsecond_upload_skipped\tvisible_capture_shapes_exact\t\
         runtime_verified\tlayout_approved\trelease_eligible\n\
         {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
         0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t\
         0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        report.image_sha256,
        report.first_ram_sha256,
        report.first_gpu_sha256,
        report.second_ram_sha256,
        report.second_gpu_sha256,
        report.restored_ram_sha256,
        report.restored_gpu_sha256,
        report.first_entry_events_sha256,
        report.first_lifecycle_events_sha256,
        report.second_entry_events_sha256,
        report.second_lifecycle_events_sha256,
        report.restored_entry_events_sha256,
        report.restored_lifecycle_events_sha256,
        report.first_visible_sha256,
        report.second_visible_sha256,
        report.second_visible_page2_sha256,
        report.restored_visible_sha256,
        report.first_record_runtime,
        report.second_record_runtime,
        report.restored_record_runtime,
        report.first_object_runtime,
        report.second_object_runtime,
        report.restored_object_runtime,
        report.body_runtime,
        report.body_bytes,
        report.first_page_state,
        report.second_page_state,
        report.restored_page_state,
        report.first_upload_calls,
        report.second_upload_calls,
        report.restored_upload_calls,
        report.first_sync_calls,
        report.second_sync_calls,
        report.restored_sync_calls,
        report.event_count,
        report.dropped_events,
        report.resident_inputs_exact,
        report.first_page_exact,
        report.second_page_exact,
        report.restored_page_exact,
        report.second_upload_skipped,
        report.visible_capture_shapes_exact,
        report.runtime_verified,
        report.layout_approved,
        report.release_eligible,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPageTwoDialoguesRuntimeError(String);

impl fmt::Display for QuestPageTwoDialoguesRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestPageTwoDialoguesRuntimeError {}

fn error(message: impl Into<String>) -> QuestPageTwoDialoguesRuntimeError {
    QuestPageTwoDialoguesRuntimeError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn runtime_slice<'a>(
    ram: &'a [u8],
    runtime: u32,
    len: usize,
    label: &str,
) -> Result<&'a [u8], QuestPageTwoDialoguesRuntimeError> {
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
) -> Result<u32, QuestPageTwoDialoguesRuntimeError> {
    let raw: [u8; 4] = runtime_slice(ram, runtime, 4, label)?
        .try_into()
        .map_err(|_| error(format!("{label}: u32 길이 불일치")))?;
    Ok(u32::from_le_bytes(raw))
}

fn event_u32(
    event: &Value,
    path: &[&str],
    label: &str,
) -> Result<u32, QuestPageTwoDialoguesRuntimeError> {
    let value = path
        .iter()
        .try_fold(event, |value, key| value.get(*key))
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("QUEST 두 대사 event에 {label}가 없음")))?;
    u32::try_from(value).map_err(|_| error(format!("QUEST 두 대사 event {label} 범위 이탈")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeEvent {
    pc: u32,
    a0: u32,
    a1: u32,
    gp: u32,
    ra: u32,
    sp: u32,
}

fn parse_events(
    raw: &[u8],
    label: &str,
) -> Result<Vec<RuntimeEvent>, QuestPageTwoDialoguesRuntimeError> {
    let document: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST {label} event JSON 해석 실패: {source}")))?;
    let dropped = document
        .get("dropped")
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("QUEST {label} event JSON에 dropped가 없음")))?;
    if dropped != 0 {
        return Err(error(format!("QUEST {label} event dropped={dropped}")));
    }
    document
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| error(format!("QUEST {label} event JSON에 events가 없음")))?
        .iter()
        .map(|event| {
            Ok(RuntimeEvent {
                pc: event_u32(event, &["pc"], "pc")?,
                a0: event_u32(event, &["registers", "a0"], "a0")?,
                a1: event_u32(event, &["registers", "a1"], "a1")?,
                gp: event_u32(event, &["registers", "gp"], "gp")?,
                ra: event_u32(event, &["registers", "ra"], "ra")?,
                sp: event_u32(event, &["registers", "sp"], "sp")?,
            })
        })
        .collect()
}

fn require_entry(
    events: &[RuntimeEvent],
    label: &str,
) -> Result<RuntimeEvent, QuestPageTwoDialoguesRuntimeError> {
    let [event] = events else {
        return Err(error(format!(
            "QUEST {label} entry event 수 불일치: {}",
            events.len()
        )));
    };
    if event.pc != QUEST_HOOK_BODY_RUNTIME
        || event.gp != QUEST_HOOK_GP_RUNTIME
        || event.ra != QUEST_PAGE_BODY_RETURN_RUNTIME
    {
        return Err(error(format!("QUEST {label} entry register 계약 불일치")));
    }
    Ok(*event)
}

fn require_lifecycle(
    events: &[RuntimeEvent],
    entry: RuntimeEvent,
    expected_source: Option<u32>,
    label: &str,
) -> Result<(usize, usize), QuestPageTwoDialoguesRuntimeError> {
    let expected_pcs: &[u32] = if expected_source.is_some() {
        &[
            QUEST_FONT_UPLOAD_TARGET_RUNTIME,
            QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME,
            QUEST_PAGE_BODY_RETURN_RUNTIME,
        ]
    } else {
        &[QUEST_PAGE_BODY_RETURN_RUNTIME]
    };
    let actual_pcs = events.iter().map(|event| event.pc).collect::<Vec<_>>();
    if actual_pcs != expected_pcs {
        return Err(error(format!(
            "QUEST {label} lifecycle PC 불일치: expected={expected_pcs:08X?}, actual={actual_pcs:08X?}"
        )));
    }
    if let Some(expected_source) = expected_source {
        if events[0].a1 != expected_source || events[1].a0 != 0 {
            return Err(error(format!(
                "QUEST {label} upload source 또는 DrawSync 인자 불일치"
            )));
        }
    }
    let returned = events
        .last()
        .ok_or_else(|| error(format!("QUEST {label} return event가 없음")))?;
    if returned.a0 != entry.a0
        || returned.gp != entry.gp
        || returned.ra != entry.ra
        || returned.sp != entry.sp
    {
        return Err(error(format!("QUEST {label} return register 계약 불일치")));
    }
    Ok((
        usize::from(expected_source.is_some()),
        usize::from(expected_source.is_some()),
    ))
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

fn font_states_inactive(ram: &[u8]) -> Result<bool, QuestPageTwoDialoguesRuntimeError> {
    (0..QUEST_FONT_STATE_RECORD_COUNT)
        .map(|index| {
            let runtime = QUEST_FONT_STATE_BASE_RUNTIME
                + u32::try_from(index * QUEST_FONT_STATE_RECORD_STRIDE)
                    .map_err(|_| error("QUEST font state offset 범위 이탈"))?
                + QUEST_FONT_STATE_ACTIVE_OFFSET;
            Ok(runtime_slice(ram, runtime, 1, "QUEST font active")? == [0])
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|states| states.into_iter().all(|inactive| inactive))
}

struct StaticRuntimeInputs {
    site: Vec<u8>,
    body: Vec<u8>,
    korean_page: Vec<u8>,
    original_page: Vec<u8>,
    image_sha256: String,
}

fn static_runtime_inputs(
    source: &VerifiedSourceImage,
    second: &QuestSecondDialogueAssets,
) -> Result<StaticRuntimeInputs, QuestPageTwoDialoguesRuntimeError> {
    let prepared = prepare_quest_two_dialogue_assets(source, second)
        .map_err(|source| error(source.to_string()))?;
    let build = build_quest_page_two_dialogues(source, second)
        .map_err(|source| error(source.to_string()))?;
    if build.report.image_sha256 != BOX_QUEST_PAGE_TWO_DIALOGUES_IMAGE_SHA256
        || build.report.translation_approved
        || build.report.layout_approved
        || build.report.runtime_verified
        || build.report.release_eligible
    {
        return Err(error("QUEST 두 대사 runtime build 계약 drift"));
    }
    for offset in [
        QUEST_PAGE_FIRST_DIALOGUE_RECORD_OFFSET,
        QUEST_PAGE_SECOND_DIALOGUE_RECORD_OFFSET,
        QUEST_PAGE_TWO_DIALOGUES_RESTORE_RECORD_OFFSET,
    ] {
        let start = usize::try_from(offset).map_err(|_| error("QUEST record offset 범위 이탈"))?;
        if prepared.new_town_plain.get(start..start + 4) != Some(&[0x21, 0, 0, 0]) {
            return Err(error(format!(
                "QUEST TOWN00 OP21 원본 계약 불일치: 0x{offset:04X}"
            )));
        }
    }

    let source_iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let original_quest_raw = source_iso
        .read_file(QUEST_EXE_ISO_PATH)
        .map_err(|source| error(source.to_string()))?;
    let original_exe =
        PsxExe::parse(&original_quest_raw).map_err(|source| error(source.to_string()))?;
    let map = exe_map("QUEST.EXE").ok_or_else(|| error("QUEST.EXE 좌표 사상이 없음"))?;
    original_exe
        .verify_map(map)
        .map_err(|source| error(source.to_string()))?;
    let candidates = verify_box_quest_hook_space_candidates(map, original_exe.text())
        .map_err(|source| error(source.to_string()))?;
    let sources = verify_quest_hook_noop_sources(map, original_exe.text(), &candidates)
        .map_err(|source| error(source.to_string()))?;
    let body =
        assemble_two_dialogue_transition_body().map_err(|source| error(source.to_string()))?;

    Ok(StaticRuntimeInputs {
        site: sources.site.bytes().to_vec(),
        body: body.bytes().to_vec(),
        korean_page: prepared.new_font_plain,
        original_page: prepared.original_font_plain,
        image_sha256: build.report.image_sha256,
    })
}

fn verify_checkpoint(
    checkpoint: QuestPageTwoDialoguesRuntimeCheckpoint<'_>,
    entry: RuntimeEvent,
    record_runtime: u32,
    page_state: u8,
    expected_page: &[u8],
    inputs: &StaticRuntimeInputs,
    label: &str,
) -> Result<(), QuestPageTwoDialoguesRuntimeError> {
    if checkpoint.ram.len() != PSX_MAIN_RAM_LEN {
        return Err(error(format!(
            "QUEST {label} RAM 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
            checkpoint.ram.len()
        )));
    }
    if runtime_slice(
        checkpoint.ram,
        crate::disasm::QUEST_FONT_HOOK_SITE_RUNTIME,
        inputs.site.len(),
        "QUEST two-dialogue hook site",
    )? != inputs.site
        || runtime_slice(
            checkpoint.ram,
            QUEST_HOOK_BODY_RUNTIME,
            inputs.body.len(),
            "QUEST two-dialogue hook body",
        )? != inputs.body
        || runtime_slice(
            checkpoint.ram,
            QUEST_PAGE_CARRIER_START_RUNTIME,
            inputs.korean_page.len(),
            "QUEST resident Korean page",
        )? != inputs.korean_page
        || runtime_slice(
            checkpoint.ram,
            QUEST_PAGE_RESTORE_SOURCE_RUNTIME,
            inputs.original_page.len(),
            "QUEST resident original page",
        )? != inputs.original_page
    {
        return Err(error(format!("QUEST {label} resident 입력 불일치")));
    }
    let actual_page_state = runtime_slice(
        checkpoint.ram,
        QUEST_PAGE_HOOK_STATE_RUNTIME,
        1,
        "QUEST two-dialogue page state",
    )?[0];
    let object_active = runtime_u32(
        checkpoint.ram,
        entry.a0 + QUEST_OBJECT_ACTIVE_OFFSET,
        "QUEST two-dialogue object active",
    )?;
    let object_stream = runtime_u32(
        checkpoint.ram,
        entry.a0 + QUEST_OBJECT_STREAM_OFFSET,
        "QUEST two-dialogue object stream",
    )?;
    let uploaded =
        uploaded_font_page(checkpoint.gpu).map_err(|source| error(source.to_string()))?;
    if actual_page_state != page_state
        || object_active != 1
        || object_stream != record_runtime + 4
        || uploaded != expected_page
        || !font_states_inactive(checkpoint.ram)?
    {
        return Err(error(format!(
            "QUEST {label} runtime 상태 불일치: page={actual_page_state}/{page_state}, object_active={object_active}, stream=0x{object_stream:08X}/0x{:08X}, uploaded={}, font_inactive={}",
            record_runtime + 4,
            uploaded == expected_page,
            font_states_inactive(checkpoint.ram)?
        )));
    }
    Ok(())
}

/// 두 Korean record의 upload/skip과 다음 비대상 record의 원본 복귀를 검증한다.
pub fn verify_quest_page_two_dialogues_runtime(
    source: &VerifiedSourceImage,
    second: &QuestSecondDialogueAssets,
    evidence: QuestPageTwoDialoguesRuntimeEvidence<'_>,
) -> Result<QuestPageTwoDialoguesRuntimeReport, QuestPageTwoDialoguesRuntimeError> {
    let inputs = static_runtime_inputs(source, second)?;
    let first_entry_events = parse_events(evidence.first.entry_events, "first entry")?;
    let first_lifecycle_events = parse_events(evidence.first.lifecycle_events, "first lifecycle")?;
    let second_entry_events = parse_events(evidence.second.entry_events, "second entry")?;
    let second_lifecycle_events =
        parse_events(evidence.second.lifecycle_events, "second lifecycle")?;
    let restored_entry_events = parse_events(evidence.restored.entry_events, "restored entry")?;
    let restored_lifecycle_events =
        parse_events(evidence.restored.lifecycle_events, "restored lifecycle")?;

    let first_entry = require_entry(&first_entry_events, "first")?;
    let second_entry = require_entry(&second_entry_events, "second")?;
    let restored_entry = require_entry(&restored_entry_events, "restored")?;
    if first_entry.a0 != second_entry.a0 || restored_entry.a0 == first_entry.a0 {
        return Err(error(
            "QUEST 두 Korean record 또는 복귀 object identity 불일치",
        ));
    }
    let (first_upload_calls, first_sync_calls) = require_lifecycle(
        &first_lifecycle_events,
        first_entry,
        Some(QUEST_PAGE_CARRIER_START_RUNTIME),
        "first",
    )?;
    let (second_upload_calls, second_sync_calls) =
        require_lifecycle(&second_lifecycle_events, second_entry, None, "second")?;
    let (restored_upload_calls, restored_sync_calls) = require_lifecycle(
        &restored_lifecycle_events,
        restored_entry,
        Some(QUEST_PAGE_RESTORE_SOURCE_RUNTIME),
        "restored",
    )?;

    verify_checkpoint(
        evidence.first,
        first_entry,
        QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME,
        QUEST_PAGE_FIRST_DIALOGUE_PAGE_ID,
        &inputs.korean_page,
        &inputs,
        "first",
    )?;
    verify_checkpoint(
        evidence.second,
        second_entry,
        QUEST_PAGE_SECOND_DIALOGUE_RECORD_RUNTIME,
        QUEST_PAGE_FIRST_DIALOGUE_PAGE_ID,
        &inputs.korean_page,
        &inputs,
        "second",
    )?;
    verify_checkpoint(
        evidence.restored,
        restored_entry,
        QUEST_PAGE_TWO_DIALOGUES_RESTORE_RECORD_RUNTIME,
        QUEST_PAGE_RESTORE_PAGE_ID,
        &inputs.original_page,
        &inputs,
        "restored",
    )?;

    let visible_capture_shapes_exact = [
        evidence.first_visible_png,
        evidence.second_visible_png,
        evidence.second_visible_page2_png,
        evidence.restored_visible_png,
    ]
    .iter()
    .all(|raw| png_shape_exact(raw));
    if !visible_capture_shapes_exact {
        return Err(error("QUEST 두 대사 visible PNG shape 불일치"));
    }

    Ok(QuestPageTwoDialoguesRuntimeReport {
        schema_version: QUEST_PAGE_TWO_DIALOGUES_RUNTIME_SCHEMA_VERSION,
        image_sha256: inputs.image_sha256,
        first_ram_sha256: digest(evidence.first.ram),
        first_gpu_sha256: digest(evidence.first.gpu),
        second_ram_sha256: digest(evidence.second.ram),
        second_gpu_sha256: digest(evidence.second.gpu),
        restored_ram_sha256: digest(evidence.restored.ram),
        restored_gpu_sha256: digest(evidence.restored.gpu),
        first_entry_events_sha256: digest(evidence.first.entry_events),
        first_lifecycle_events_sha256: digest(evidence.first.lifecycle_events),
        second_entry_events_sha256: digest(evidence.second.entry_events),
        second_lifecycle_events_sha256: digest(evidence.second.lifecycle_events),
        restored_entry_events_sha256: digest(evidence.restored.entry_events),
        restored_lifecycle_events_sha256: digest(evidence.restored.lifecycle_events),
        first_visible_sha256: digest(evidence.first_visible_png),
        second_visible_sha256: digest(evidence.second_visible_png),
        second_visible_page2_sha256: digest(evidence.second_visible_page2_png),
        restored_visible_sha256: digest(evidence.restored_visible_png),
        first_record_runtime: QUEST_PAGE_FIRST_DIALOGUE_RECORD_RUNTIME,
        second_record_runtime: QUEST_PAGE_SECOND_DIALOGUE_RECORD_RUNTIME,
        restored_record_runtime: QUEST_PAGE_TWO_DIALOGUES_RESTORE_RECORD_RUNTIME,
        first_object_runtime: first_entry.a0,
        second_object_runtime: second_entry.a0,
        restored_object_runtime: restored_entry.a0,
        body_runtime: QUEST_HOOK_BODY_RUNTIME,
        body_bytes: inputs.body.len(),
        first_page_state: QUEST_PAGE_FIRST_DIALOGUE_PAGE_ID,
        second_page_state: QUEST_PAGE_FIRST_DIALOGUE_PAGE_ID,
        restored_page_state: QUEST_PAGE_RESTORE_PAGE_ID,
        first_upload_calls,
        second_upload_calls,
        restored_upload_calls,
        first_sync_calls,
        second_sync_calls,
        restored_sync_calls,
        event_count: first_entry_events.len()
            + first_lifecycle_events.len()
            + second_entry_events.len()
            + second_lifecycle_events.len()
            + restored_entry_events.len()
            + restored_lifecycle_events.len(),
        dropped_events: 0,
        resident_inputs_exact: true,
        first_page_exact: true,
        second_page_exact: true,
        restored_page_exact: true,
        second_upload_skipped: second_upload_calls == 0 && second_sync_calls == 0,
        visible_capture_shapes_exact,
        runtime_verified: true,
        layout_approved: false,
        release_eligible: false,
    })
}

#[cfg(test)]
#[path = "quest_page_two_dialogues_runtime_tests.rs"]
mod tests;
