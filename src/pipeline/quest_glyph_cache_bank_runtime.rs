//! QUEST glyph cache의 secondary bank 선택·소비 runtime 검증.
//!
//! exact development image의 cold route 또는 명시적 state-load 후속 전이를
//! 구분해 검증한다. primary marker가 살아 있는 동안 다른 logical
//! page를 secondary에 commit하고, 같은 draw list에서 두 texture page가 object
//! marker대로 소비되는지를 묶는다. state load를 쓴 증거는 cold route로
//! 승격하지 않고, 두 provenance flag가 같으면 입력을 거부한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::disasm::{
    QUEST_FONT_OBJECT_LIST_HEAD_RUNTIME, QUEST_FONT_OBJECT_LIST_SENTINEL_RUNTIME,
    QUEST_FONT_OBJECT_NEXT_OFFSET, QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET,
    QUEST_OBJECT_POOL_RECORD_BYTES, QUEST_OBJECT_POOL_RECORD_COUNT, QUEST_OBJECT_POOL_RUNTIME,
    QUEST_PAGE_CARRIER_CAPACITY_BYTES, QUEST_PAGE_CARRIER_START_RUNTIME,
};
use crate::isa::{InstructionKind as K, Reg};
use crate::runtime::MednafenPsxState;
use crate::script::TOWN00_SEQ_RUNTIME_BASE;

use super::current_development_disc::CurrentDevelopmentDiscBuild;
use super::quest_glyph_atlas_bank_policy::{encode_quest_glyph_atlas_marker, QuestGlyphAtlasBank};
use super::quest_glyph_cache_hook_plan::{
    QuestGlyphCacheHookPlan, QUEST_GLYPH_CACHE_KOREAN_TEXTURE_PAGE,
    QUEST_GLYPH_CACHE_SECONDARY_TEXTURE_PAGE,
};
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_glyph_cache_residency::{
    expected_resident_package, png_dimensions, QuestGlyphCacheScratchExpectation,
};
use super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;
use super::quest_translation_selection_routing::{
    QuestTranslationSelectionRoute, QuestTranslationSelectionRoutingAnalysis,
};

pub const QUEST_GLYPH_CACHE_BANK_RUNTIME_SCHEMA_VERSION: &str = "quest-glyph-cache-bank-runtime-v1";
const CPU_STATE_SCHEMA_VERSION: &str = "quest-glyph-cache-bank-runtime-cpu-state-v1";
const SCREENSHOT_WIDTH: u32 = 280;
const SCREENSHOT_HEIGHT: u32 = 240;

#[derive(Debug, Clone, Copy)]
pub struct QuestGlyphCacheBankRuntimeEvidence<'a> {
    pub before_commit_state: &'a [u8],
    pub commit_cpu_state: &'a [u8],
    pub after_visible_state: &'a [u8],
    pub after_visible_screenshot: &'a [u8],
    pub draw_events: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheBankRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub stream_id: String,
    pub batch_id: String,
    pub record_runtime: u32,
    pub primary_page: u8,
    pub secondary_page: u8,
    pub primary_marker: u8,
    pub secondary_marker: u8,
    pub secondary_commit_runtime: u32,
    pub page_glyphs: usize,
    pub before_state_sha256: String,
    pub cpu_state_sha256: String,
    pub after_state_sha256: String,
    pub screenshot_sha256: String,
    pub draw_events_sha256: String,
    pub launch_id: String,
    pub tracking_run_id: String,
    pub before_live_objects: usize,
    pub after_live_objects: usize,
    pub primary_draws: usize,
    pub secondary_draws: usize,
    pub dropped_events: u64,
    pub resident_package_exact: bool,
    pub site_exact: bool,
    pub body_exact: bool,
    pub carrier_suffix_zero: bool,
    pub secondary_selection_verified: bool,
    pub secondary_commit_verified: bool,
    pub per_object_texture_page_verified: bool,
    pub atlas_bank_policy_runtime_verified: bool,
    pub downstream_input_transition_verified: bool,
    pub cold_route_verified: bool,
    pub all_banks_pinned_runtime_verified: bool,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub release_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheBankRuntimeError(String);

impl fmt::Display for QuestGlyphCacheBankRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestGlyphCacheBankRuntimeError {}

fn error(message: impl Into<String>) -> QuestGlyphCacheBankRuntimeError {
    QuestGlyphCacheBankRuntimeError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn json_u32(document: &Value, key: &str) -> Result<u32, QuestGlyphCacheBankRuntimeError> {
    document
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| error(format!("QUEST bank CPU state에 {key} u32가 없음")))
}

fn json_string(document: &Value, key: &str) -> Result<String, QuestGlyphCacheBankRuntimeError> {
    let value = document
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| error(format!("QUEST bank CPU state에 {key} 문자열이 없음")))?;
    if value.is_empty() || value.contains(['\t', '\r', '\n']) {
        return Err(error(format!(
            "QUEST bank CPU state {key}가 비었거나 제어 문자를 포함함"
        )));
    }
    Ok(value.to_string())
}

fn verify_route_provenance(document: &Value) -> Result<bool, QuestGlyphCacheBankRuntimeError> {
    let state_load_intervention = document
        .get("state_load_intervention")
        .and_then(Value::as_bool)
        .ok_or_else(|| error("QUEST bank CPU state에 state_load_intervention bool이 없음"))?;
    let cold_route_verified = document
        .get("cold_route_verified")
        .and_then(Value::as_bool)
        .ok_or_else(|| error("QUEST bank CPU state에 cold_route_verified bool이 없음"))?;
    if state_load_intervention == cold_route_verified {
        return Err(error(
            "QUEST bank CPU state의 state-load/cold route provenance가 배타적이지 않음",
        ));
    }
    Ok(cold_route_verified)
}

fn secondary_commit_runtime(
    hook: &QuestGlyphCacheHookPlan,
) -> Result<u32, QuestGlyphCacheBankRuntimeError> {
    let matches = hook
        .body
        .instructions()
        .iter()
        .filter(|instruction| {
            instruction.kind
                == K::Sb {
                    rt: Reg::S1,
                    base: Reg::T0,
                    offset: QuestGlyphAtlasBank::Secondary.state_offset(),
                }
        })
        .map(|instruction| instruction.addr)
        .collect::<Vec<_>>();
    let [runtime] = matches.as_slice() else {
        return Err(error(format!(
            "QUEST hook secondary commit 명령 수 불일치: {}",
            matches.len()
        )));
    };
    Ok(*runtime)
}

fn exact_state_range(
    state: &MednafenPsxState<'_>,
    runtime: u32,
    expected: &[u8],
    label: &str,
) -> Result<(), QuestGlyphCacheBankRuntimeError> {
    let actual = state
        .read_bytes(runtime, expected.len())
        .map_err(|source| error(format!("{label} state 읽기 실패: {source}")))?;
    if actual != expected {
        let first = actual
            .iter()
            .zip(expected)
            .position(|(actual, expected)| actual != expected)
            .ok_or_else(|| error(format!("{label} state 비교 내부 불일치")))?;
        return Err(error(format!(
            "{label} state 불일치: runtime=0x{runtime:08X}, offset=0x{first:X}, expected=0x{:02X}, actual=0x{:02X}",
            expected[first], actual[first]
        )));
    }
    Ok(())
}

fn verify_resident_cache_installation(
    state: &MednafenPsxState<'_>,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    scratch_page: u8,
) -> Result<(), QuestGlyphCacheBankRuntimeError> {
    let expected_package = expected_resident_package(
        package,
        QuestGlyphCacheScratchExpectation::UploadedPage(scratch_page),
    )
    .map_err(|source| error(source.to_string()))?;
    exact_state_range(
        state,
        QUEST_PAGE_CARRIER_START_RUNTIME,
        &expected_package,
        "QUEST bank package",
    )?;
    exact_state_range(
        state,
        hook.site.base_addr(),
        hook.site.bytes(),
        "QUEST bank OP21 site",
    )?;
    exact_state_range(
        state,
        hook.original_font_guard_site.base_addr(),
        hook.original_font_guard_site.bytes(),
        "QUEST bank original font guard",
    )?;
    exact_state_range(
        state,
        hook.body_runtime,
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
    let suffix = state
        .read_bytes(body_end, suffix_bytes)
        .map_err(|source| error(format!("QUEST bank suffix 읽기 실패: {source}")))?;
    if let Some(offset) = suffix.iter().position(|byte| *byte != 0) {
        return Err(error(format!(
            "QUEST bank carrier suffix가 0이 아님: runtime=0x{body_end:08X}, offset=0x{offset:X}, value=0x{:02X}",
            suffix[offset]
        )));
    }
    Ok(())
}

fn page_state(state: &MednafenPsxState<'_>) -> Result<[u8; 4], QuestGlyphCacheBankRuntimeError> {
    state
        .read_bytes(QUEST_PAGE_HOOK_STATE_RUNTIME, 4)
        .map_err(|source| error(format!("QUEST bank page state 읽기 실패: {source}")))?
        .try_into()
        .map_err(|_| error("QUEST bank page state 길이 불일치"))
}

fn require_page_state(
    actual: [u8; 4],
    expected: [u8; 4],
    label: &str,
) -> Result<(), QuestGlyphCacheBankRuntimeError> {
    if actual != expected {
        return Err(error(format!(
            "QUEST bank {label} page state 불일치: expected={expected:02X?}, actual={actual:02X?}"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LiveObject {
    runtime: u32,
    marker: u8,
}

fn read_live_objects(
    state: &MednafenPsxState<'_>,
) -> Result<Vec<LiveObject>, QuestGlyphCacheBankRuntimeError> {
    let pool_bytes = u32::try_from(
        QUEST_OBJECT_POOL_RECORD_BYTES
            .checked_mul(QUEST_OBJECT_POOL_RECORD_COUNT)
            .ok_or_else(|| error("QUEST object pool 길이 overflow"))?,
    )
    .map_err(|_| error("QUEST object pool 길이 범위 이탈"))?;
    let pool_end = QUEST_OBJECT_POOL_RUNTIME
        .checked_add(pool_bytes)
        .ok_or_else(|| error("QUEST object pool end overflow"))?;
    let mut runtime = state
        .read_u32_le(QUEST_FONT_OBJECT_LIST_HEAD_RUNTIME)
        .map_err(|source| error(format!("QUEST font object list head 읽기 실패: {source}")))?;
    let mut visited = BTreeSet::new();
    let mut objects = Vec::new();
    while runtime != QUEST_FONT_OBJECT_LIST_SENTINEL_RUNTIME {
        if objects.len() >= QUEST_OBJECT_POOL_RECORD_COUNT {
            return Err(error(
                "QUEST font object list가 bounded count 안에서 끝나지 않음",
            ));
        }
        if runtime < QUEST_OBJECT_POOL_RUNTIME
            || runtime >= pool_end
            || !(runtime - QUEST_OBJECT_POOL_RUNTIME)
                .is_multiple_of(u32::try_from(QUEST_OBJECT_POOL_RECORD_BYTES).expect("0xE0"))
        {
            return Err(error(format!(
                "QUEST font object list pointer가 pool record가 아님: 0x{runtime:08X}"
            )));
        }
        if !visited.insert(runtime) {
            return Err(error(format!(
                "QUEST font object list cycle: 0x{runtime:08X}"
            )));
        }
        let marker_runtime = runtime
            .checked_add(
                u32::try_from(QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET)
                    .map_err(|_| error("QUEST marker offset가 음수임"))?,
            )
            .ok_or_else(|| error("QUEST marker runtime overflow"))?;
        let next_runtime = runtime
            .checked_add(
                u32::try_from(QUEST_FONT_OBJECT_NEXT_OFFSET)
                    .map_err(|_| error("QUEST next offset가 음수임"))?,
            )
            .ok_or_else(|| error("QUEST next runtime overflow"))?;
        objects.push(LiveObject {
            runtime,
            marker: state
                .read_u8(marker_runtime)
                .map_err(|source| error(format!("QUEST object marker 읽기 실패: {source}")))?,
        });
        runtime = state
            .read_u32_le(next_runtime)
            .map_err(|source| error(format!("QUEST object next 읽기 실패: {source}")))?;
    }
    Ok(objects)
}

fn verify_commit_cpu_state(
    raw: &[u8],
    hook: &QuestGlyphCacheHookPlan,
    route: &QuestTranslationSelectionRoute,
    record_runtime: u32,
    page_glyphs: usize,
) -> Result<(u32, String, String, bool), QuestGlyphCacheBankRuntimeError> {
    let document: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST bank CPU state JSON 실패: {source}")))?;
    if document.get("schema_version").and_then(Value::as_str) != Some(CPU_STATE_SCHEMA_VERSION) {
        return Err(error("QUEST bank CPU state schema 불일치"));
    }
    let cold_route_verified = verify_route_provenance(&document)?;
    let commit_runtime = secondary_commit_runtime(hook)?;
    let expected = [
        ("g0.PC", commit_runtime),
        (
            "g0.NPC",
            commit_runtime
                .checked_add(4)
                .ok_or_else(|| error("QUEST bank commit NPC overflow"))?,
        ),
        ("g0.s0", record_runtime),
        ("g0.s1", u32::from(route.page_id)),
        (
            "g0.s5",
            u32::try_from(page_glyphs).map_err(|_| error("QUEST page glyph 수 범위 이탈"))?,
        ),
        ("g0.t0", QUEST_PAGE_HOOK_STATE_RUNTIME),
    ];
    for (key, expected) in expected {
        let actual = json_u32(&document, key)?;
        if actual != expected {
            return Err(error(format!(
                "QUEST bank CPU state {key} 불일치: expected=0x{expected:08X}, actual=0x{actual:08X}"
            )));
        }
    }
    if !json_u32(&document, "g0.sp")?.is_multiple_of(8) {
        return Err(error("QUEST bank CPU state stack 정렬 불일치"));
    }
    Ok((
        commit_runtime,
        json_string(&document, "launch_id")?,
        json_string(&document, "tracking_run_id")?,
        cold_route_verified,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DrawEvidence {
    dropped: u64,
    primary: usize,
    secondary: usize,
}

fn verify_draw_events(
    raw: &[u8],
    hook: &QuestGlyphCacheHookPlan,
    objects: &[LiveObject],
    primary_marker: u8,
    secondary_marker: u8,
) -> Result<DrawEvidence, QuestGlyphCacheBankRuntimeError> {
    let continuation = hook
        .font_draw_texture_page_site
        .base_addr()
        .checked_add(8)
        .ok_or_else(|| error("QUEST texture page continuation overflow"))?;
    verify_draw_events_at(raw, continuation, objects, primary_marker, secondary_marker)
}

fn verify_draw_events_at(
    raw: &[u8],
    continuation: u32,
    objects: &[LiveObject],
    primary_marker: u8,
    secondary_marker: u8,
) -> Result<DrawEvidence, QuestGlyphCacheBankRuntimeError> {
    let document: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST bank draw event JSON 실패: {source}")))?;
    let dropped = document
        .get("dropped")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("QUEST bank draw event dropped가 없음"))?;
    let events = document
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| error("QUEST bank draw event 배열이 없음"))?;
    if dropped != 0 || events.len() != objects.len() {
        return Err(error(format!(
            "QUEST bank draw event 분모 불일치: dropped={dropped}, events={}, objects={}",
            events.len(),
            objects.len()
        )));
    }
    let mut primary = 0;
    let mut secondary = 0;
    for (index, (event, object)) in events.iter().zip(objects).enumerate() {
        let event_u32 = |path: &[&str], label: &str| {
            path.iter()
                .try_fold(event, |value, key| value.get(*key))
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| error(format!("QUEST draw event {index}에 {label}가 없음")))
        };
        let pc = event_u32(&["pc"], "pc")?;
        let object_runtime = event_u32(&["registers", "s1"], "s1")?;
        let texture_page = event_u32(&["registers", "v0"], "v0")?;
        let expected_texture_page = if object.marker == primary_marker {
            primary += 1;
            u32::from(QUEST_GLYPH_CACHE_KOREAN_TEXTURE_PAGE)
        } else if object.marker == secondary_marker {
            secondary += 1;
            u32::from(QUEST_GLYPH_CACHE_SECONDARY_TEXTURE_PAGE)
        } else {
            return Err(error(format!(
                "QUEST draw object marker가 두 bank 밖임: object=0x{:08X}, marker={}",
                object.runtime, object.marker
            )));
        };
        if pc != continuation
            || object_runtime != object.runtime
            || texture_page != expected_texture_page
        {
            return Err(error(format!(
                "QUEST draw event {index} 불일치: pc=0x{pc:08X}, object=0x{object_runtime:08X}, \
                 tpage=0x{texture_page:04X}, expected_pc=0x{continuation:08X}, \
                 expected_object=0x{:08X}, expected_tpage=0x{expected_texture_page:04X}",
                object.runtime
            )));
        }
    }
    if primary == 0 || secondary == 0 {
        return Err(error(format!(
            "QUEST draw event가 두 bank를 함께 소비하지 않음: primary={primary}, secondary={secondary}"
        )));
    }
    Ok(DrawEvidence {
        dropped,
        primary,
        secondary,
    })
}

/// downstream state에서 관측한 secondary bank 선택·표시를 exact build에 묶는다.
pub fn verify_quest_glyph_cache_bank_runtime(
    build: &CurrentDevelopmentDiscBuild,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    stream_id: &str,
    primary_page: u8,
    evidence: QuestGlyphCacheBankRuntimeEvidence<'_>,
) -> Result<QuestGlyphCacheBankRuntimeReport, QuestGlyphCacheBankRuntimeError> {
    if build.report.quest.disc.image_sha256 != digest(&build.image) {
        return Err(error("QUEST bank current development image identity drift"));
    }
    let routes = routing
        .routes
        .iter()
        .filter(|route| route.stream_id == stream_id)
        .collect::<Vec<_>>();
    let [route] = routes.as_slice() else {
        return Err(error(format!(
            "QUEST bank stream route 수 불일치: stream={stream_id}, routes={}",
            routes.len()
        )));
    };
    if route.sequence != "TOWN00.SEQ" || route.page_id == primary_page || primary_page == 0 {
        return Err(error(format!(
            "QUEST bank route/page 계약 불일치: sequence={}, primary={primary_page}, secondary={}",
            route.sequence, route.page_id
        )));
    }
    let record_runtime = TOWN00_SEQ_RUNTIME_BASE
        .checked_add(
            u32::try_from(route.record_offset)
                .map_err(|_| error("QUEST bank record offset 범위 이탈"))?,
        )
        .ok_or_else(|| error("QUEST bank record runtime overflow"))?;
    let page_glyphs = package
        .page_catalog_indices(route.page_id)
        .map_err(|source| error(source.to_string()))?
        .len();
    let secondary_marker =
        encode_quest_glyph_atlas_marker(route.page_id, QuestGlyphAtlasBank::Secondary)
            .map_err(|source| error(source.to_string()))?;
    let primary_marker =
        encode_quest_glyph_atlas_marker(primary_page, QuestGlyphAtlasBank::Primary)
            .map_err(|source| error(source.to_string()))?;

    let before = MednafenPsxState::parse(evidence.before_commit_state)
        .map_err(|source| error(format!("QUEST bank before state 실패: {source}")))?;
    let after = MednafenPsxState::parse(evidence.after_visible_state)
        .map_err(|source| error(format!("QUEST bank after state 실패: {source}")))?;
    verify_resident_cache_installation(&before, package, hook, route.page_id)?;
    verify_resident_cache_installation(&after, package, hook, route.page_id)?;
    require_page_state(
        page_state(&before)?,
        [primary_page, primary_marker, primary_marker, 0],
        "commit 전",
    )?;
    require_page_state(
        page_state(&after)?,
        [
            primary_page,
            secondary_marker,
            secondary_marker,
            route.page_id,
        ],
        "표시 뒤",
    )?;

    let before_objects = read_live_objects(&before)?;
    if before_objects.is_empty()
        || before_objects
            .iter()
            .any(|object| object.marker != primary_marker)
    {
        return Err(error(format!(
            "QUEST bank commit 전 primary pin 분모 불일치: objects={}, markers={:?}",
            before_objects.len(),
            before_objects
                .iter()
                .map(|object| object.marker)
                .collect::<Vec<_>>()
        )));
    }
    let after_objects = read_live_objects(&after)?;
    if after_objects.len() <= before_objects.len()
        || !after_objects
            .iter()
            .take(before_objects.len())
            .eq(before_objects.iter())
        || after_objects
            .iter()
            .skip(before_objects.len())
            .any(|object| object.marker != secondary_marker)
    {
        return Err(error(format!(
            "QUEST bank 표시 뒤 object list 불일치: before={}, after={}",
            before_objects.len(),
            after_objects.len()
        )));
    }

    let (commit_runtime, launch_id, tracking_run_id, cold_route_verified) =
        verify_commit_cpu_state(
            evidence.commit_cpu_state,
            hook,
            route,
            record_runtime,
            page_glyphs,
        )?;
    let draw = verify_draw_events(
        evidence.draw_events,
        hook,
        &after_objects,
        primary_marker,
        secondary_marker,
    )?;
    if png_dimensions(evidence.after_visible_screenshot)
        != Some((SCREENSHOT_WIDTH, SCREENSHOT_HEIGHT))
    {
        return Err(error("QUEST bank screenshot shape 불일치"));
    }

    Ok(QuestGlyphCacheBankRuntimeReport {
        schema_version: QUEST_GLYPH_CACHE_BANK_RUNTIME_SCHEMA_VERSION,
        image_sha256: build.report.quest.disc.image_sha256.clone(),
        stream_id: route.stream_id.clone(),
        batch_id: route.batch_id.clone(),
        record_runtime,
        primary_page,
        secondary_page: route.page_id,
        primary_marker,
        secondary_marker,
        secondary_commit_runtime: commit_runtime,
        page_glyphs,
        before_state_sha256: digest(evidence.before_commit_state),
        cpu_state_sha256: digest(evidence.commit_cpu_state),
        after_state_sha256: digest(evidence.after_visible_state),
        screenshot_sha256: digest(evidence.after_visible_screenshot),
        draw_events_sha256: digest(evidence.draw_events),
        launch_id,
        tracking_run_id,
        before_live_objects: before_objects.len(),
        after_live_objects: after_objects.len(),
        primary_draws: draw.primary,
        secondary_draws: draw.secondary,
        dropped_events: draw.dropped,
        resident_package_exact: true,
        site_exact: true,
        body_exact: true,
        carrier_suffix_zero: true,
        secondary_selection_verified: true,
        secondary_commit_verified: true,
        per_object_texture_page_verified: true,
        atlas_bank_policy_runtime_verified: true,
        downstream_input_transition_verified: true,
        cold_route_verified,
        all_banks_pinned_runtime_verified: false,
        translation_approved: false,
        layout_approved: false,
        release_eligible: false,
    })
}

pub fn render_quest_glyph_cache_bank_runtime_tsv(
    report: &QuestGlyphCacheBankRuntimeReport,
) -> String {
    let mut out = String::from(concat!(
        "schema_version\timage_sha256\tstream_id\tbatch_id\trecord_runtime\tprimary_page\t",
        "secondary_page\tprimary_marker\tsecondary_marker\tsecondary_commit_runtime\tpage_glyphs\t",
        "before_state_sha256\tcpu_state_sha256\tafter_state_sha256\tscreenshot_sha256\t",
        "draw_events_sha256\tlaunch_id\ttracking_run_id\tbefore_live_objects\tafter_live_objects\t",
        "primary_draws\tsecondary_draws\tdropped_events\tresident_package_exact\tsite_exact\t",
        "body_exact\tcarrier_suffix_zero\tsecondary_selection_verified\tsecondary_commit_verified\t",
        "per_object_texture_page_verified\tatlas_bank_policy_runtime_verified\t",
        "downstream_input_transition_verified\tcold_route_verified\t",
        "all_banks_pinned_runtime_verified\ttranslation_approved\tlayout_approved\trelease_eligible\n",
    ));
    writeln!(
        out,
        "{}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        report.schema_version,
        report.image_sha256,
        report.stream_id,
        report.batch_id,
        report.record_runtime,
        report.primary_page,
        report.secondary_page,
        report.primary_marker,
        report.secondary_marker,
        report.secondary_commit_runtime,
        report.page_glyphs,
        report.before_state_sha256,
        report.cpu_state_sha256,
        report.after_state_sha256,
        report.screenshot_sha256,
        report.draw_events_sha256,
        report.launch_id,
        report.tracking_run_id,
        report.before_live_objects,
        report.after_live_objects,
        report.primary_draws,
        report.secondary_draws,
        report.dropped_events,
        report.resident_package_exact,
        report.site_exact,
        report.body_exact,
        report.carrier_suffix_zero,
        report.secondary_selection_verified,
        report.secondary_commit_verified,
        report.per_object_texture_page_verified,
        report.atlas_bank_policy_runtime_verified,
        report.downstream_input_transition_verified,
        report.cold_route_verified,
        report.all_banks_pinned_runtime_verified,
        report.translation_approved,
        report.layout_approved,
        report.release_eligible,
    )
    .expect("String write는 실패하지 않음");
    out
}

#[cfg(test)]
#[path = "quest_glyph_cache_bank_runtime_tests.rs"]
mod tests;
