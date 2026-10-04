//! QUEST development carrier의 추가 대표 장면 runtime 증거 검증.
//!
//! current development image를 cold boot에서 진행해 얻은 native state와
//! descriptor hook hit-time event를 exact build에 결속한다. write breakpoint
//! 기록은 관측 구간과 보호 범위가 맞는지만 검증하며, 미적중을 전역
//! 미접근이나 release 안전성으로 일반화하지 않는다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::disasm::{
    QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET, QUEST_PAGE_CARRIER_END_RUNTIME,
    QUEST_PAGE_CARRIER_START_RUNTIME,
};
use crate::runtime::MednafenPsxState;

use super::quest_development_disc::QuestDevelopmentDiscBuild;
use super::quest_glyph_cache_hook_plan::QuestGlyphCacheHookPlan;
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_glyph_cache_residency::{
    verify_quest_development_disc_residency, QuestGlyphCacheResidencyReport,
};

pub const QUEST_DEVELOPMENT_CARRIER_SCENE_RUNTIME_SCHEMA_VERSION: &str =
    "quest-development-carrier-scene-runtime-v1";
const OBSERVATION_SCHEMA_VERSION: &str = "quest-development-carrier-scene-observation-v1";

/// mutable scratch를 제외하고 write watch를 건 carrier 범위.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestDevelopmentCarrierProtectedRange {
    pub start_runtime: u32,
    pub end_runtime_inclusive: u32,
    pub hits: u64,
}

/// 한 장면에서 얻은 raw runtime 증거 묶음.
#[derive(Debug, Clone, Copy)]
pub struct QuestDevelopmentCarrierSceneRuntimeEvidence<'a> {
    pub native_state: &'a [u8],
    pub screenshot_png: &'a [u8],
    pub descriptor_events: &'a [u8],
    pub write_observation: &'a [u8],
}

/// exact development build의 한 추가 장면에 귀속한 검증 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentCarrierSceneRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub state_sha256: String,
    pub screenshot_sha256: String,
    pub descriptor_events_sha256: String,
    pub observation_sha256: String,
    pub scene_id: String,
    pub launch_id: String,
    pub tracking_run_id: String,
    pub start_frame: u64,
    pub end_frame: u64,
    pub observed_frames: u64,
    pub expected_page: u8,
    pub descriptor_entry_runtime: u32,
    pub descriptor_event_count: usize,
    pub descriptor_dropped_events: u64,
    pub object_runtime: u32,
    pub object_page_marker_runtime: u32,
    pub object_page_marker: u8,
    pub mutable_scratch_runtime: u32,
    pub mutable_scratch_bytes: usize,
    pub protected_ranges: Vec<QuestDevelopmentCarrierProtectedRange>,
    pub protected_bytes: usize,
    pub protected_write_hits: u64,
    pub residency_verified: bool,
    pub descriptor_consumer_verified: bool,
    pub protected_write_observation_accepted: bool,
    pub bounded_scene_evidence_verified: bool,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub release_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentCarrierSceneRuntimeError(String);

impl fmt::Display for QuestDevelopmentCarrierSceneRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestDevelopmentCarrierSceneRuntimeError {}

fn error(message: impl Into<String>) -> QuestDevelopmentCarrierSceneRuntimeError {
    QuestDevelopmentCarrierSceneRuntimeError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn value_at<'a>(root: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(root, |value, key| value.get(*key))
}

fn required_u64(
    root: &Value,
    path: &[&str],
    label: &str,
) -> Result<u64, QuestDevelopmentCarrierSceneRuntimeError> {
    value_at(root, path)
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("QUEST carrier scene JSON에 {label}가 없음")))
}

fn required_u32(
    root: &Value,
    path: &[&str],
    label: &str,
) -> Result<u32, QuestDevelopmentCarrierSceneRuntimeError> {
    u32::try_from(required_u64(root, path, label)?)
        .map_err(|_| error(format!("QUEST carrier scene {label} 범위 이탈")))
}

fn required_bool(
    root: &Value,
    path: &[&str],
    label: &str,
) -> Result<bool, QuestDevelopmentCarrierSceneRuntimeError> {
    value_at(root, path)
        .and_then(Value::as_bool)
        .ok_or_else(|| error(format!("QUEST carrier scene JSON에 {label}가 없음")))
}

fn required_string(
    root: &Value,
    path: &[&str],
    label: &str,
) -> Result<String, QuestDevelopmentCarrierSceneRuntimeError> {
    let value = value_at(root, path)
        .and_then(Value::as_str)
        .ok_or_else(|| error(format!("QUEST carrier scene JSON에 {label}가 없음")))?;
    if value.trim().is_empty() {
        return Err(error(format!("QUEST carrier scene {label}가 비어 있음")));
    }
    if value.contains(['\t', '\r', '\n']) {
        return Err(error(format!(
            "QUEST carrier scene {label}에 TSV 제어 문자가 있음"
        )));
    }
    Ok(value.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DescriptorEvidence {
    event_count: usize,
    dropped_events: u64,
    object_runtime: u32,
    marker_runtime: u32,
    marker: u8,
}

fn verify_descriptor_event(
    raw: &[u8],
    expected_entry_runtime: u32,
    expected_page: u8,
    state: &MednafenPsxState<'_>,
) -> Result<DescriptorEvidence, QuestDevelopmentCarrierSceneRuntimeError> {
    let document: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST descriptor event JSON 해석 실패: {source}")))?;
    let dropped_events = required_u64(&document, &["dropped"], "dropped")?;
    let events = document
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| error("QUEST descriptor event JSON에 events 배열이 없음"))?;
    if dropped_events != 0 || events.len() != 1 {
        return Err(error(format!(
            "QUEST descriptor event 수 불일치: dropped={dropped_events}, events={}",
            events.len()
        )));
    }
    let event = &events[0];
    let actual_entry_runtime = required_u32(event, &["pc"], "pc")?;
    if actual_entry_runtime != expected_entry_runtime {
        return Err(error(format!(
            "QUEST descriptor entry 불일치: expected=0x{expected_entry_runtime:08X}, actual=0x{actual_entry_runtime:08X}"
        )));
    }
    let object_runtime = required_u32(event, &["registers", "s1"], "s1")?;
    if !object_runtime.is_multiple_of(4) {
        return Err(error(format!(
            "QUEST descriptor object 주소 정렬 불일치: 0x{object_runtime:08X}"
        )));
    }
    let marker_offset = u32::try_from(QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET)
        .map_err(|_| error("QUEST font object page marker offset가 음수임"))?;
    let marker_runtime = object_runtime
        .checked_add(marker_offset)
        .ok_or_else(|| error("QUEST descriptor object marker runtime overflow"))?;
    let marker = state.read_u8(marker_runtime).map_err(|source| {
        error(format!(
            "QUEST descriptor object marker 읽기 실패: {source}"
        ))
    })?;
    if marker != expected_page {
        return Err(error(format!(
            "QUEST descriptor object page marker 불일치: runtime=0x{marker_runtime:08X}, expected={expected_page}, actual={marker}"
        )));
    }
    Ok(DescriptorEvidence {
        event_count: events.len(),
        dropped_events,
        object_runtime,
        marker_runtime,
        marker,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WriteObservation {
    scene_id: String,
    launch_id: String,
    tracking_run_id: String,
    start_frame: u64,
    end_frame: u64,
    ranges: Vec<QuestDevelopmentCarrierProtectedRange>,
}

fn expected_protected_ranges(
    scratch_runtime: u32,
    scratch_bytes: usize,
) -> Result<Vec<QuestDevelopmentCarrierProtectedRange>, QuestDevelopmentCarrierSceneRuntimeError> {
    let scratch_bytes =
        u32::try_from(scratch_bytes).map_err(|_| error("QUEST carrier scratch 길이 범위 이탈"))?;
    let scratch_end = scratch_runtime
        .checked_add(scratch_bytes)
        .ok_or_else(|| error("QUEST carrier scratch end overflow"))?;
    if scratch_runtime <= QUEST_PAGE_CARRIER_START_RUNTIME
        || scratch_end >= QUEST_PAGE_CARRIER_END_RUNTIME
    {
        return Err(error(format!(
            "QUEST carrier scratch가 내부 가변 범위가 아님: 0x{scratch_runtime:08X}..0x{scratch_end:08X}"
        )));
    }
    Ok(vec![
        QuestDevelopmentCarrierProtectedRange {
            start_runtime: QUEST_PAGE_CARRIER_START_RUNTIME,
            end_runtime_inclusive: scratch_runtime - 1,
            hits: 0,
        },
        QuestDevelopmentCarrierProtectedRange {
            start_runtime: scratch_end,
            end_runtime_inclusive: QUEST_PAGE_CARRIER_END_RUNTIME - 1,
            hits: 0,
        },
    ])
}

fn parse_observed_ranges(
    document: &Value,
) -> Result<Vec<QuestDevelopmentCarrierProtectedRange>, QuestDevelopmentCarrierSceneRuntimeError> {
    document
        .get("protected_write_ranges")
        .and_then(Value::as_array)
        .ok_or_else(|| error("QUEST carrier observation에 protected_write_ranges가 없음"))?
        .iter()
        .enumerate()
        .map(|(index, range)| {
            Ok(QuestDevelopmentCarrierProtectedRange {
                start_runtime: required_u32(
                    range,
                    &["start_runtime"],
                    &format!("protected_write_ranges[{index}].start_runtime"),
                )?,
                end_runtime_inclusive: required_u32(
                    range,
                    &["end_runtime_inclusive"],
                    &format!("protected_write_ranges[{index}].end_runtime_inclusive"),
                )?,
                hits: required_u64(
                    range,
                    &["hits"],
                    &format!("protected_write_ranges[{index}].hits"),
                )?,
            })
        })
        .collect()
}

fn verify_write_observation(
    raw: &[u8],
    scratch_runtime: u32,
    scratch_bytes: usize,
) -> Result<WriteObservation, QuestDevelopmentCarrierSceneRuntimeError> {
    let document: Value = serde_json::from_slice(raw).map_err(|source| {
        error(format!(
            "QUEST carrier observation JSON 해석 실패: {source}"
        ))
    })?;
    let schema_version = required_string(&document, &["schema_version"], "schema_version")?;
    if schema_version != OBSERVATION_SCHEMA_VERSION {
        return Err(error(format!(
            "QUEST carrier observation schema 불일치: {schema_version}"
        )));
    }
    let cold_boot = required_bool(&document, &["cold_boot"], "cold_boot")?;
    let guest_advance_completed = required_bool(
        &document,
        &["guest_advance_completed"],
        "guest_advance_completed",
    )?;
    let state_loads = required_u64(&document, &["state_loads"], "state_loads")?;
    let memory_writes = required_u64(&document, &["memory_writes"], "memory_writes")?;
    let resets = required_u64(&document, &["resets"], "resets")?;
    if !cold_boot
        || !guest_advance_completed
        || state_loads != 0
        || memory_writes != 0
        || resets != 0
    {
        return Err(error(format!(
            "QUEST carrier 관측 경로 계약 불일치: cold_boot={cold_boot}, completed={guest_advance_completed}, state_loads={state_loads}, memory_writes={memory_writes}, resets={resets}"
        )));
    }
    let start_frame = required_u64(&document, &["start_frame"], "start_frame")?;
    let end_frame = required_u64(&document, &["end_frame"], "end_frame")?;
    if end_frame <= start_frame {
        return Err(error(format!(
            "QUEST carrier 관측 frame 범위 불일치: {start_frame}..{end_frame}"
        )));
    }
    let observed_scratch_runtime = required_u32(
        &document,
        &["mutable_scratch", "start_runtime"],
        "mutable_scratch.start_runtime",
    )?;
    let observed_scratch_bytes = usize::try_from(required_u64(
        &document,
        &["mutable_scratch", "bytes"],
        "mutable_scratch.bytes",
    )?)
    .map_err(|_| error("QUEST carrier mutable scratch 길이 범위 이탈"))?;
    if observed_scratch_runtime != scratch_runtime || observed_scratch_bytes != scratch_bytes {
        return Err(error(format!(
            "QUEST carrier mutable scratch 불일치: expected=0x{scratch_runtime:08X}/{scratch_bytes}, actual=0x{observed_scratch_runtime:08X}/{observed_scratch_bytes}"
        )));
    }
    let expected_ranges = expected_protected_ranges(scratch_runtime, scratch_bytes)?;
    let ranges = parse_observed_ranges(&document)?;
    if ranges != expected_ranges {
        return Err(error(format!(
            "QUEST carrier 보호 write 범위 불일치: expected={expected_ranges:?}, actual={ranges:?}"
        )));
    }
    Ok(WriteObservation {
        scene_id: required_string(&document, &["scene_id"], "scene_id")?,
        launch_id: required_string(&document, &["launch_id"], "launch_id")?,
        tracking_run_id: required_string(&document, &["tracking_run_id"], "tracking_run_id")?,
        start_frame,
        end_frame,
        ranges,
    })
}

fn protected_bytes(
    ranges: &[QuestDevelopmentCarrierProtectedRange],
) -> Result<usize, QuestDevelopmentCarrierSceneRuntimeError> {
    ranges.iter().try_fold(0usize, |total, range| {
        let bytes = range
            .end_runtime_inclusive
            .checked_sub(range.start_runtime)
            .and_then(|delta| delta.checked_add(1))
            .ok_or_else(|| error("QUEST carrier 보호 범위가 역전됨"))?;
        total
            .checked_add(
                usize::try_from(bytes).map_err(|_| error("QUEST carrier 보호 길이 범위 이탈"))?,
            )
            .ok_or_else(|| error("QUEST carrier 보호 길이 합계 overflow"))
    })
}

/// exact development image의 추가 대표 장면 증거를 carrier 계약에 결속한다.
pub fn verify_quest_development_carrier_scene_runtime(
    build: &QuestDevelopmentDiscBuild,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    expected_page: u8,
    evidence: QuestDevelopmentCarrierSceneRuntimeEvidence<'_>,
) -> Result<QuestDevelopmentCarrierSceneRuntimeReport, QuestDevelopmentCarrierSceneRuntimeError> {
    let QuestDevelopmentCarrierSceneRuntimeEvidence {
        native_state,
        screenshot_png,
        descriptor_events,
        write_observation,
    } = evidence;
    let residency: QuestGlyphCacheResidencyReport = verify_quest_development_disc_residency(
        build,
        package,
        hook,
        expected_page,
        native_state,
        screenshot_png,
    )
    .map_err(|source| error(source.to_string()))?;
    let state = MednafenPsxState::parse(native_state).map_err(|source| {
        error(format!(
            "QUEST carrier scene native state 해석 실패: {source}"
        ))
    })?;
    let descriptor = verify_descriptor_event(
        descriptor_events,
        hook.font_draw_descriptor_entry_runtime,
        expected_page,
        &state,
    )?;

    let scratch_runtime = QUEST_PAGE_CARRIER_START_RUNTIME
        .checked_add(
            u32::try_from(package.layout.scratch_offset)
                .map_err(|_| error("QUEST carrier scratch offset 범위 이탈"))?,
        )
        .ok_or_else(|| error("QUEST carrier scratch runtime overflow"))?;
    let scratch_end = scratch_runtime
        .checked_add(
            u32::try_from(package.layout.scratch_bytes)
                .map_err(|_| error("QUEST carrier scratch 길이 범위 이탈"))?,
        )
        .ok_or_else(|| error("QUEST carrier scratch end overflow"))?;
    if scratch_end != hook.body_runtime {
        return Err(error(format!(
            "QUEST carrier scratch/body 경계 불일치: scratch_end=0x{scratch_end:08X}, body=0x{:08X}",
            hook.body_runtime
        )));
    }
    let observation = verify_write_observation(
        write_observation,
        scratch_runtime,
        package.layout.scratch_bytes,
    )?;
    let protected_bytes = protected_bytes(&observation.ranges)?;
    let protected_write_hits = observation.ranges.iter().map(|range| range.hits).sum();
    let observed_frames = observation.end_frame - observation.start_frame;

    Ok(QuestDevelopmentCarrierSceneRuntimeReport {
        schema_version: QUEST_DEVELOPMENT_CARRIER_SCENE_RUNTIME_SCHEMA_VERSION,
        image_sha256: residency.image_sha256,
        state_sha256: residency.state_sha256,
        screenshot_sha256: residency.screenshot_sha256,
        descriptor_events_sha256: digest(descriptor_events),
        observation_sha256: digest(write_observation),
        scene_id: observation.scene_id,
        launch_id: observation.launch_id,
        tracking_run_id: observation.tracking_run_id,
        start_frame: observation.start_frame,
        end_frame: observation.end_frame,
        observed_frames,
        expected_page,
        descriptor_entry_runtime: hook.font_draw_descriptor_entry_runtime,
        descriptor_event_count: descriptor.event_count,
        descriptor_dropped_events: descriptor.dropped_events,
        object_runtime: descriptor.object_runtime,
        object_page_marker_runtime: descriptor.marker_runtime,
        object_page_marker: descriptor.marker,
        mutable_scratch_runtime: scratch_runtime,
        mutable_scratch_bytes: package.layout.scratch_bytes,
        protected_ranges: observation.ranges,
        protected_bytes,
        protected_write_hits,
        residency_verified: residency.residency_verified,
        descriptor_consumer_verified: true,
        protected_write_observation_accepted: true,
        bounded_scene_evidence_verified: residency.residency_verified,
        translation_approved: false,
        layout_approved: false,
        release_eligible: false,
    })
}

/// 추가 대표 장면의 bounded carrier 결과를 한 행 TSV로 만든다.
pub fn render_quest_development_carrier_scene_runtime_tsv(
    report: &QuestDevelopmentCarrierSceneRuntimeReport,
) -> String {
    let ranges = report
        .protected_ranges
        .iter()
        .map(|range| {
            format!(
                "0x{:08X}-0x{:08X}:{}",
                range.start_runtime, range.end_runtime_inclusive, range.hits
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut out = String::from(
        "schema_version\timage_sha256\tstate_sha256\tscreenshot_sha256\t\
         descriptor_events_sha256\tobservation_sha256\tscene_id\tlaunch_id\ttracking_run_id\t\
         start_frame\tend_frame\tobserved_frames\texpected_page\tdescriptor_entry_runtime\t\
         descriptor_event_count\tdescriptor_dropped_events\tobject_runtime\t\
         object_page_marker_runtime\tobject_page_marker\tmutable_scratch_runtime\t\
         mutable_scratch_bytes\tprotected_ranges\tprotected_bytes\tprotected_write_hits\t\
         residency_verified\tdescriptor_consumer_verified\t\
         protected_write_observation_accepted\tbounded_scene_evidence_verified\t\
         translation_approved\tlayout_approved\trelease_eligible\n",
    );
    writeln!(
        out,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t\
         0x{:08X}\t0x{:08X}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        report.schema_version,
        report.image_sha256,
        report.state_sha256,
        report.screenshot_sha256,
        report.descriptor_events_sha256,
        report.observation_sha256,
        report.scene_id,
        report.launch_id,
        report.tracking_run_id,
        report.start_frame,
        report.end_frame,
        report.observed_frames,
        report.expected_page,
        report.descriptor_entry_runtime,
        report.descriptor_event_count,
        report.descriptor_dropped_events,
        report.object_runtime,
        report.object_page_marker_runtime,
        report.object_page_marker,
        report.mutable_scratch_runtime,
        report.mutable_scratch_bytes,
        ranges,
        report.protected_bytes,
        report.protected_write_hits,
        report.residency_verified,
        report.descriptor_consumer_verified,
        report.protected_write_observation_accepted,
        report.bounded_scene_evidence_verified,
        report.translation_approved,
        report.layout_approved,
        report.release_eligible,
    )
    .expect("String write는 실패하지 않음");
    out
}

#[cfg(test)]
#[path = "quest_development_carrier_scene_runtime_tests.rs"]
mod tests;
