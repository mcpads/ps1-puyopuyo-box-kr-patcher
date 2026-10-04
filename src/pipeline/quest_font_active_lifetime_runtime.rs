//! QUEST text-window active byte의 실제 producer와 clear 수명 검증.
//!
//! `FF15` handler의 active store 직전·직후 RAM과 이후 안정화 RAM을 exact
//! transition image에 결속한다. stream pointer 개입 표본과 input-only 자연
//! 인카운터 표본을 별도 schema로 검증해 실행 사실, 자연 도달, cold boot와
//! release 적격을 서로 승격하지 않는다.

use std::error::Error;
use std::fmt;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::coords::{psx_main_ram_offset, PSX_MAIN_RAM_LEN};
use crate::disasm::{
    QUEST_FONT_STATE_BASE_RUNTIME, QUEST_FONT_STATE_RECORD_COUNT, QUEST_FONT_STATE_RECORD_STRIDE,
    QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET,
};

pub const QUEST_FONT_ACTIVE_LIFETIME_RUNTIME_SCHEMA_VERSION: &str =
    "quest-font-active-lifetime-runtime-v1";
pub const QUEST_FONT_ACTIVE_NATURAL_RUNTIME_SCHEMA_VERSION: &str =
    "quest-font-active-natural-runtime-v1";

const QUEST_FONT_ACTIVE_OFFSET: u32 = QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET as u32 + 1;
const QUEST_FONT_ACTIVE_STATE_INDEX: usize = 1;
const QUEST_FONT_STATE_RESET_RUNTIME: u32 = 0x8002_C754;
const QUEST_FONT_ACTIVE_SET_RUNTIME: u32 = 0x8003_E504;
const QUEST_FONT_ACTIVE_CLEAR_RUNTIME: u32 = 0x8003_BD48;

/// `FF15` active store 한 명령과 이후 clear의 exact 검증 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestFontActiveLifetimeRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub before_ram_sha256: String,
    pub after_ram_sha256: String,
    pub settled_ram_sha256: String,
    pub events_sha256: String,
    pub state_base_runtime: u32,
    pub state_stride: usize,
    pub state_count: usize,
    pub active_offset: u32,
    pub active_state_index: usize,
    pub active_runtime: u32,
    pub reset_runtime: u32,
    pub producer_runtime: u32,
    pub clear_runtime: u32,
    pub producer_diff_bytes: usize,
    pub event_count: usize,
    pub dropped_events: u64,
    pub before_all_inactive: bool,
    pub producer_diff_exact: bool,
    pub selected_state_active_after: bool,
    pub producer_event_exact: bool,
    pub clear_event_exact: bool,
    pub settled_all_inactive: bool,
    pub active_lifetime_verified: bool,
    pub route_naturally_reached: bool,
    pub release_eligible: bool,
}

/// 자연 인카운터의 input-only 도달 선언과 active 수명 검증 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestFontActiveNaturalRuntimeReport {
    pub transition: QuestFontActiveLifetimeRuntimeReport,
    pub checkpoint_sha256: String,
    pub route_evidence_sha256: String,
    pub message_png_sha256: String,
    pub choice_png_sha256: String,
    pub tracking_run_id: String,
    pub producer_frame: u64,
    pub producer_samples: u64,
    pub origin_load_state: bool,
    pub gamepad_input_only_after_checkpoint: bool,
    pub memory_write_count_after_checkpoint: u64,
    pub cold_boot_route_reached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestFontActiveLifetimeRuntimeError(String);

impl fmt::Display for QuestFontActiveLifetimeRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestFontActiveLifetimeRuntimeError {}

fn error(message: impl Into<String>) -> QuestFontActiveLifetimeRuntimeError {
    QuestFontActiveLifetimeRuntimeError(message.into())
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn active_runtime(index: usize) -> Option<u32> {
    if index >= QUEST_FONT_STATE_RECORD_COUNT {
        return None;
    }
    QUEST_FONT_STATE_BASE_RUNTIME
        .checked_add(u32::try_from(index.checked_mul(QUEST_FONT_STATE_RECORD_STRIDE)?).ok()?)?
        .checked_add(QUEST_FONT_ACTIVE_OFFSET)
}

fn active_values(ram: &[u8]) -> Result<Vec<u8>, QuestFontActiveLifetimeRuntimeError> {
    (0..QUEST_FONT_STATE_RECORD_COUNT)
        .map(|index| {
            let runtime = active_runtime(index)
                .ok_or_else(|| error("QUEST font active runtime 주소 overflow"))?;
            let offset = psx_main_ram_offset(runtime, 1)
                .ok_or_else(|| error("QUEST font active runtime이 main RAM 범위 밖임"))?;
            ram.get(offset)
                .copied()
                .ok_or_else(|| error("QUEST font active RAM dump 범위 이탈"))
        })
        .collect()
}

fn event_u32(
    event: &Value,
    path: &[&str],
    label: &str,
) -> Result<u32, QuestFontActiveLifetimeRuntimeError> {
    let value = path
        .iter()
        .try_fold(event, |value, key| value.get(*key))
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("QUEST font active event에 {label}가 없음")))?;
    u32::try_from(value).map_err(|_| error(format!("QUEST font active event {label} 범위 이탈")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveEvent {
    pc: u32,
    v0: u32,
    v1: u32,
    a0: u32,
}

fn parse_events(
    raw: &[u8],
    expected_count: usize,
) -> Result<(Vec<ActiveEvent>, u64), QuestFontActiveLifetimeRuntimeError> {
    let root: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST font active event JSON 해석 실패: {source}")))?;
    let dropped = root
        .get("dropped")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("QUEST font active event dropped가 없음"))?;
    if dropped != 0 {
        return Err(error(format!(
            "QUEST font active event가 {dropped}개 유실됨"
        )));
    }
    let events = root
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| error("QUEST font active events 배열이 없음"))?;
    if events.len() != expected_count {
        return Err(error(format!(
            "QUEST font active event 수 불일치: expected={expected_count}, actual={}",
            events.len()
        )));
    }
    events
        .iter()
        .map(|event| {
            Ok(ActiveEvent {
                pc: event_u32(event, &["pc"], "pc")?,
                v0: event_u32(event, &["registers", "v0"], "v0")?,
                v1: event_u32(event, &["registers", "v1"], "v1")?,
                a0: event_u32(event, &["registers", "a0"], "a0")?,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|events| (events, dropped))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveEventSequence {
    ResetProducerClear,
    ProducerClear,
}

impl ActiveEventSequence {
    fn count(self) -> usize {
        match self {
            Self::ResetProducerClear => 3,
            Self::ProducerClear => 2,
        }
    }
}

fn diff_offsets(before: &[u8], after: &[u8]) -> Vec<usize> {
    before
        .iter()
        .zip(after)
        .enumerate()
        .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
        .collect()
}

struct ActiveLifetimeEvidence<'a> {
    schema_version: &'static str,
    image_sha256: &'a str,
    before_ram: &'a [u8],
    after_ram: &'a [u8],
    settled_ram: &'a [u8],
    events_raw: &'a [u8],
    event_sequence: ActiveEventSequence,
    route_naturally_reached: bool,
}

fn verify_bound(
    evidence: ActiveLifetimeEvidence<'_>,
) -> Result<QuestFontActiveLifetimeRuntimeReport, QuestFontActiveLifetimeRuntimeError> {
    let ActiveLifetimeEvidence {
        schema_version,
        image_sha256,
        before_ram,
        after_ram,
        settled_ram,
        events_raw,
        event_sequence,
        route_naturally_reached,
    } = evidence;
    if !is_sha256_hex(image_sha256) {
        return Err(error(format!(
            "QUEST font active image SHA-256 형식 불일치: actual={image_sha256}"
        )));
    }
    for (label, ram) in [
        ("before", before_ram),
        ("after", after_ram),
        ("settled", settled_ram),
    ] {
        if ram.len() != PSX_MAIN_RAM_LEN {
            return Err(error(format!(
                "QUEST font active {label} RAM 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
                ram.len()
            )));
        }
    }

    let active_runtime = active_runtime(QUEST_FONT_ACTIVE_STATE_INDEX)
        .ok_or_else(|| error("QUEST font active 선택 state 주소 overflow"))?;
    let active_offset = psx_main_ram_offset(active_runtime, 1)
        .ok_or_else(|| error("QUEST font active 선택 state가 main RAM 범위 밖임"))?;
    let before_active = active_values(before_ram)?;
    let after_active = active_values(after_ram)?;
    let settled_active = active_values(settled_ram)?;
    let before_all_inactive = before_active.iter().all(|value| *value == 0);
    let selected_state_active_after = after_active
        .iter()
        .enumerate()
        .all(|(index, value)| *value == u8::from(index == QUEST_FONT_ACTIVE_STATE_INDEX));
    let settled_all_inactive = settled_active.iter().all(|value| *value == 0);
    let producer_diffs = diff_offsets(before_ram, after_ram);
    let producer_diff_exact = producer_diffs == [active_offset]
        && before_ram[active_offset] == 0
        && after_ram[active_offset] == 1;

    let (events, dropped_events) = parse_events(events_raw, event_sequence.count())?;
    let state_zero_runtime = QUEST_FONT_STATE_BASE_RUNTIME;
    let state_one_runtime = QUEST_FONT_STATE_BASE_RUNTIME
        + u32::try_from(QUEST_FONT_STATE_RECORD_STRIDE)
            .map_err(|_| error("QUEST font state stride가 u32 범위를 벗어남"))?;
    let producer = ActiveEvent {
        pc: QUEST_FONT_ACTIVE_SET_RUNTIME,
        v0: state_one_runtime,
        v1: 1,
        a0: state_zero_runtime,
    };
    let (producer_event_exact, clear_event) = match event_sequence {
        ActiveEventSequence::ResetProducerClear => {
            let reset = ActiveEvent {
                pc: QUEST_FONT_STATE_RESET_RUNTIME,
                v0: state_zero_runtime,
                v1: 0,
                a0: 0,
            };
            (events[0] == reset && events[1] == producer, events[2])
        }
        ActiveEventSequence::ProducerClear => (events[0] == producer, events[1]),
    };
    let clear_event_exact =
        clear_event.pc == QUEST_FONT_ACTIVE_CLEAR_RUNTIME && clear_event.a0 == state_one_runtime;

    if !before_all_inactive
        || !producer_diff_exact
        || !selected_state_active_after
        || !producer_event_exact
        || !clear_event_exact
        || !settled_all_inactive
    {
        return Err(error(format!(
            "QUEST font active lifetime 불일치: before_inactive={before_all_inactive}, diff_exact={producer_diff_exact}, after_active={selected_state_active_after}, producer_event={producer_event_exact}, clear_event={clear_event_exact}, settled_inactive={settled_all_inactive}"
        )));
    }

    Ok(QuestFontActiveLifetimeRuntimeReport {
        schema_version,
        image_sha256: image_sha256.to_owned(),
        before_ram_sha256: digest(before_ram),
        after_ram_sha256: digest(after_ram),
        settled_ram_sha256: digest(settled_ram),
        events_sha256: digest(events_raw),
        state_base_runtime: QUEST_FONT_STATE_BASE_RUNTIME,
        state_stride: QUEST_FONT_STATE_RECORD_STRIDE,
        state_count: QUEST_FONT_STATE_RECORD_COUNT,
        active_offset: QUEST_FONT_ACTIVE_OFFSET,
        active_state_index: QUEST_FONT_ACTIVE_STATE_INDEX,
        active_runtime,
        reset_runtime: QUEST_FONT_STATE_RESET_RUNTIME,
        producer_runtime: QUEST_FONT_ACTIVE_SET_RUNTIME,
        clear_runtime: QUEST_FONT_ACTIVE_CLEAR_RUNTIME,
        producer_diff_bytes: producer_diffs.len(),
        event_count: events.len(),
        dropped_events,
        before_all_inactive,
        producer_diff_exact,
        selected_state_active_after,
        producer_event_exact,
        clear_event_exact,
        settled_all_inactive,
        active_lifetime_verified: true,
        route_naturally_reached,
        release_eligible: false,
    })
}

/// exact current image에서 `FF15` active producer와 clear 수명을 검증한다.
pub fn verify_quest_font_active_lifetime_runtime(
    image: &[u8],
    before_ram: &[u8],
    after_ram: &[u8],
    settled_ram: &[u8],
    events_raw: &[u8],
) -> Result<QuestFontActiveLifetimeRuntimeReport, QuestFontActiveLifetimeRuntimeError> {
    verify_bound(ActiveLifetimeEvidence {
        schema_version: QUEST_FONT_ACTIVE_LIFETIME_RUNTIME_SCHEMA_VERSION,
        image_sha256: &digest(image),
        before_ram,
        after_ram,
        settled_ram,
        events_raw,
        event_sequence: ActiveEventSequence::ResetProducerClear,
        route_naturally_reached: false,
    })
}

fn route_value<'a>(
    root: &'a Value,
    key: &str,
) -> Result<&'a Value, QuestFontActiveLifetimeRuntimeError> {
    root.get(key)
        .ok_or_else(|| error(format!("QUEST font active 자연 경로에 {key}가 없음")))
}

fn route_string(root: &Value, key: &str) -> Result<String, QuestFontActiveLifetimeRuntimeError> {
    route_value(root, key)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| error(format!("QUEST font active 자연 경로 {key}가 문자열이 아님")))
}

fn route_u64(root: &Value, key: &str) -> Result<u64, QuestFontActiveLifetimeRuntimeError> {
    route_value(root, key)?
        .as_u64()
        .ok_or_else(|| error(format!("QUEST font active 자연 경로 {key}가 정수가 아님")))
}

fn route_bool(root: &Value, key: &str) -> Result<bool, QuestFontActiveLifetimeRuntimeError> {
    route_value(root, key)?
        .as_bool()
        .ok_or_else(|| error(format!("QUEST font active 자연 경로 {key}가 bool이 아님")))
}

#[derive(Debug)]
struct NaturalRouteEvidence {
    checkpoint_sha256: String,
    route_evidence_sha256: String,
    message_png_sha256: String,
    choice_png_sha256: String,
    tracking_run_id: String,
    producer_frame: u64,
    producer_samples: u64,
    origin_load_state: bool,
    gamepad_input_only_after_checkpoint: bool,
    memory_write_count_after_checkpoint: u64,
}

fn verify_natural_route_evidence(
    image_sha256: &str,
    checkpoint_state: &[u8],
    route_evidence_raw: &[u8],
    message_png: &[u8],
    choice_png: &[u8],
) -> Result<NaturalRouteEvidence, QuestFontActiveLifetimeRuntimeError> {
    let route: Value = serde_json::from_slice(route_evidence_raw).map_err(|source| {
        error(format!(
            "QUEST font active 자연 경로 JSON 해석 실패: {source}"
        ))
    })?;
    let checkpoint_sha256 = digest(checkpoint_state);
    let message_png_sha256 = digest(message_png);
    let choice_png_sha256 = digest(choice_png);
    let tracking_run_id = route_string(&route, "tracking_run_id")?;
    let producer_frame = route_u64(&route, "producer_frame")?;
    let producer_samples = route_u64(&route, "producer_samples")?;
    let origin_load_state = route_bool(&route, "origin_load_state")?;
    let gamepad_input_only_after_checkpoint =
        route_bool(&route, "gamepad_input_only_after_checkpoint")?;
    let memory_write_count_after_checkpoint =
        route_u64(&route, "memory_write_count_after_checkpoint")?;
    let route_matches = route_string(&route, "schema_version")?
        == QUEST_FONT_ACTIVE_NATURAL_RUNTIME_SCHEMA_VERSION
        && route_string(&route, "image_sha256")? == image_sha256
        && route_string(&route, "checkpoint_sha256")? == checkpoint_sha256
        && route_string(&route, "message_png_sha256")? == message_png_sha256
        && route_string(&route, "choice_png_sha256")? == choice_png_sha256
        && route_string(&route, "natural_event")? == "random_encounter_choice"
        && tracking_run_id.len() == 26
        && producer_frame > 0
        && producer_samples >= 2
        && origin_load_state
        && gamepad_input_only_after_checkpoint
        && memory_write_count_after_checkpoint == 0;
    if !route_matches {
        return Err(error(
            "QUEST font active 자연 경로 선언 또는 결속 digest가 일치하지 않음",
        ));
    }

    Ok(NaturalRouteEvidence {
        checkpoint_sha256,
        route_evidence_sha256: digest(route_evidence_raw),
        message_png_sha256,
        choice_png_sha256,
        tracking_run_id,
        producer_frame,
        producer_samples,
        origin_load_state,
        gamepad_input_only_after_checkpoint,
        memory_write_count_after_checkpoint,
    })
}

/// 자연 인카운터 검증에 함께 결속하는 exact 입력 묶음.
pub struct QuestFontActiveNaturalRuntimeInputs<'a> {
    pub image: &'a [u8],
    pub checkpoint_state: &'a [u8],
    pub before_ram: &'a [u8],
    pub after_ram: &'a [u8],
    pub settled_ram: &'a [u8],
    pub events_raw: &'a [u8],
    pub route_evidence_raw: &'a [u8],
    pub message_png: &'a [u8],
    pub choice_png: &'a [u8],
}

/// exact checkpoint 뒤 input-only 자연 인카운터의 active 수명을 검증한다.
pub fn verify_quest_font_active_natural_runtime(
    inputs: QuestFontActiveNaturalRuntimeInputs<'_>,
) -> Result<QuestFontActiveNaturalRuntimeReport, QuestFontActiveLifetimeRuntimeError> {
    let QuestFontActiveNaturalRuntimeInputs {
        image,
        checkpoint_state,
        before_ram,
        after_ram,
        settled_ram,
        events_raw,
        route_evidence_raw,
        message_png,
        choice_png,
    } = inputs;
    let image_sha256 = digest(image);
    let transition = verify_bound(ActiveLifetimeEvidence {
        schema_version: QUEST_FONT_ACTIVE_NATURAL_RUNTIME_SCHEMA_VERSION,
        image_sha256: &image_sha256,
        before_ram,
        after_ram,
        settled_ram,
        events_raw,
        event_sequence: ActiveEventSequence::ProducerClear,
        route_naturally_reached: true,
    })?;
    let route = verify_natural_route_evidence(
        &image_sha256,
        checkpoint_state,
        route_evidence_raw,
        message_png,
        choice_png,
    )?;

    Ok(QuestFontActiveNaturalRuntimeReport {
        transition,
        checkpoint_sha256: route.checkpoint_sha256,
        route_evidence_sha256: route.route_evidence_sha256,
        message_png_sha256: route.message_png_sha256,
        choice_png_sha256: route.choice_png_sha256,
        tracking_run_id: route.tracking_run_id,
        producer_frame: route.producer_frame,
        producer_samples: route.producer_samples,
        origin_load_state: route.origin_load_state,
        gamepad_input_only_after_checkpoint: route.gamepad_input_only_after_checkpoint,
        memory_write_count_after_checkpoint: route.memory_write_count_after_checkpoint,
        cold_boot_route_reached: false,
    })
}

/// QUEST font active producer/clear 검증 결과를 한 행 TSV로 만든다.
pub fn render_quest_font_active_lifetime_runtime_tsv(
    report: &QuestFontActiveLifetimeRuntimeReport,
) -> String {
    format!(
        "schema_version\timage_sha256\tbefore_ram_sha256\tafter_ram_sha256\t\
         settled_ram_sha256\tevents_sha256\tstate_base_runtime\tstate_stride\t\
         state_count\tactive_offset\tactive_state_index\tactive_runtime\t\
         reset_runtime\tproducer_runtime\tclear_runtime\tproducer_diff_bytes\t\
         event_count\tdropped_events\tbefore_all_inactive\tproducer_diff_exact\t\
         selected_state_active_after\tproducer_event_exact\tclear_event_exact\t\
         settled_all_inactive\tactive_lifetime_verified\troute_naturally_reached\t\
         release_eligible\n\
         {}\t{}\t{}\t{}\t{}\t{}\t0x{:08X}\t0x{:X}\t{}\t0x{:X}\t{}\t\
         0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        report.image_sha256,
        report.before_ram_sha256,
        report.after_ram_sha256,
        report.settled_ram_sha256,
        report.events_sha256,
        report.state_base_runtime,
        report.state_stride,
        report.state_count,
        report.active_offset,
        report.active_state_index,
        report.active_runtime,
        report.reset_runtime,
        report.producer_runtime,
        report.clear_runtime,
        report.producer_diff_bytes,
        report.event_count,
        report.dropped_events,
        report.before_all_inactive,
        report.producer_diff_exact,
        report.selected_state_active_after,
        report.producer_event_exact,
        report.clear_event_exact,
        report.settled_all_inactive,
        report.active_lifetime_verified,
        report.route_naturally_reached,
        report.release_eligible,
    )
}

/// 자연 인카운터 active 수명과 input lineage를 한 행 TSV로 만든다.
pub fn render_quest_font_active_natural_runtime_tsv(
    report: &QuestFontActiveNaturalRuntimeReport,
) -> String {
    let transition = &report.transition;
    format!(
        "schema_version\timage_sha256\tcheckpoint_sha256\tbefore_ram_sha256\t\
         after_ram_sha256\tsettled_ram_sha256\tevents_sha256\troute_evidence_sha256\t\
         message_png_sha256\tchoice_png_sha256\ttracking_run_id\tproducer_frame\t\
         producer_samples\tactive_state_index\tactive_runtime\tproducer_runtime\t\
         clear_runtime\tproducer_diff_bytes\tevent_count\tdropped_events\t\
         origin_load_state\tgamepad_input_only_after_checkpoint\t\
         memory_write_count_after_checkpoint\tactive_lifetime_verified\t\
         route_naturally_reached\tcold_boot_route_reached\trelease_eligible\n\
         {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
         0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        transition.schema_version,
        transition.image_sha256,
        report.checkpoint_sha256,
        transition.before_ram_sha256,
        transition.after_ram_sha256,
        transition.settled_ram_sha256,
        transition.events_sha256,
        report.route_evidence_sha256,
        report.message_png_sha256,
        report.choice_png_sha256,
        report.tracking_run_id,
        report.producer_frame,
        report.producer_samples,
        transition.active_state_index,
        transition.active_runtime,
        transition.producer_runtime,
        transition.clear_runtime,
        transition.producer_diff_bytes,
        transition.event_count,
        transition.dropped_events,
        report.origin_load_state,
        report.gamepad_input_only_after_checkpoint,
        report.memory_write_count_after_checkpoint,
        transition.active_lifetime_verified,
        transition.route_naturally_reached,
        report.cold_boot_route_reached,
        transition.release_eligible,
    )
}

#[cfg(test)]
#[path = "quest_font_active_lifetime_runtime_tests.rs"]
mod tests;
