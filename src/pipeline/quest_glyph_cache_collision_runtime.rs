//! 이전 QUEST one-bank sparse glyph cache의 active-window 충돌 재시도 검증.
//!
//! 첫 OP21 body entry에서 하나 이상의 font active byte만 개입한 RAM, 충돌 분기
//! return RAM, 개입 해제 뒤 재진입 RAM을 exact active build와 다시 결속한다.
//! breakpoint event와 RAM을 함께 검사해 object·stream·page를 소비하지 않고
//! dispatcher yield 뒤 같은 OP21을 재시도하는 경계만 승인한다. 현재 두-bank
//! object-lifetime hook의 검증기로 재사용하지 않고 명시적으로 실패한다.

use std::error::Error;
use std::fmt;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::coords::{psx_main_ram_offset, PSX_MAIN_RAM_LEN};
use crate::disasm::{
    QUEST_FONT_STATE_BASE_RUNTIME, QUEST_FONT_STATE_RECORD_COUNT, QUEST_FONT_STATE_RECORD_STRIDE,
    QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET, QUEST_HOOK_GP_RUNTIME,
    QUEST_PAGE_CARRIER_START_RUNTIME, QUEST_SEQ_DISPATCH_YIELD_GP_OFFSET,
};
use crate::script::TOWN00_SEQ_RUNTIME_BASE;

use super::quest_glyph_cache_hook_plan::QuestGlyphCacheHookPlan;
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_glyph_cache_transition_poc::{
    QuestGlyphCacheTransitionPocBuild, QUEST_GLYPH_CACHE_TRANSITION_THIRD_PAGE_ID,
    QUEST_GLYPH_CACHE_TRANSITION_THIRD_RECORD_OFFSET,
};
use super::quest_glyph_cache_upload_poc::{
    QuestGlyphCacheUploadPocBuild, QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID,
    QUEST_GLYPH_CACHE_UPLOAD_POC_RECORD_OFFSET,
};
use super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;

pub const QUEST_GLYPH_CACHE_COLLISION_RUNTIME_SCHEMA_VERSION: &str =
    "quest-glyph-cache-collision-runtime-v2";
pub const QUEST_GLYPH_CACHE_TRANSITION_COLLISION_RUNTIME_SCHEMA_VERSION: &str =
    "quest-glyph-cache-transition-collision-runtime-v2";

const QUEST_GLYPH_CACHE_BODY_RETURN_RUNTIME: u32 = 0x8002_25E0;
const QUEST_GLYPH_CACHE_OBJECT_ACTIVE_OFFSET: u32 = 0;
const QUEST_GLYPH_CACHE_OBJECT_STREAM_OFFSET: u32 = 0x1C;
const QUEST_GLYPH_CACHE_BODY_STACK_BYTES: u32 = 64;
const QUEST_FONT_STATE_FIRST_ACTIVE_RUNTIME: u32 =
    QUEST_FONT_STATE_BASE_RUNTIME + QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET as u32 + 1;
const QUEST_GLYPH_CACHE_COLLISION_YIELD_RUNTIME: u32 =
    QUEST_HOOK_GP_RUNTIME + QUEST_SEQ_DISPATCH_YIELD_GP_OFFSET as u32;
const QUEST_GLYPH_CACHE_COLLISION_STREAM_RUNTIME: u32 =
    TOWN00_SEQ_RUNTIME_BASE + QUEST_GLYPH_CACHE_UPLOAD_POC_RECORD_OFFSET as u32;

#[derive(Debug, Clone, Copy)]
struct CollisionRuntimeBinding<'a> {
    schema_version: &'static str,
    image_sha256: &'a str,
    stream_runtime: u32,
    stream_bytes: usize,
    stream_sha256: &'a str,
    page_state: u8,
    resident_scratch_page: Option<u8>,
}

/// 강제 active 충돌과 다음 interpreter pass 재진입의 exact 검증 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheCollisionRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub pristine_ram_sha256: String,
    pub forced_ram_sha256: String,
    pub yielded_ram_sha256: String,
    pub retry_ram_sha256: String,
    pub entry_events_sha256: String,
    pub yield_events_sha256: String,
    pub retry_events_sha256: String,
    pub body_runtime: u32,
    pub body_bytes: usize,
    pub object_runtime: u32,
    pub stream_runtime: u32,
    pub first_active_runtime: u32,
    pub active_state_indices: Vec<usize>,
    pub yield_runtime: u32,
    pub page_state_runtime: u32,
    pub forced_diff_bytes: usize,
    pub collision_diff_bytes: usize,
    pub collision_stack_diff_bytes: usize,
    pub event_count: usize,
    pub dropped_events: u64,
    pub resident_inputs_exact: bool,
    pub intervention_isolated: bool,
    pub object_not_consumed: bool,
    pub stream_not_consumed: bool,
    pub page_not_committed: bool,
    pub dispatch_yield_set: bool,
    pub retry_same_object: bool,
    pub retry_same_stream: bool,
    pub dispatch_yield_reset: bool,
    pub active_collision_policy_verified: bool,
    pub runtime_verified: bool,
    pub release_eligible: bool,
}

/// active-window 충돌 runtime 결과를 한 행 TSV로 만든다.
pub fn render_quest_glyph_cache_collision_runtime_tsv(
    report: &QuestGlyphCacheCollisionRuntimeReport,
) -> String {
    format!(
        "schema_version\timage_sha256\tpristine_ram_sha256\tforced_ram_sha256\t\
         yielded_ram_sha256\tretry_ram_sha256\tentry_events_sha256\t\
         yield_events_sha256\tretry_events_sha256\tbody_runtime\tbody_bytes\t\
         object_runtime\tstream_runtime\tfirst_active_runtime\tactive_state_indices\t\
         active_state_count\tyield_runtime\t\
         page_state_runtime\tforced_diff_bytes\tcollision_diff_bytes\t\
         collision_stack_diff_bytes\tevent_count\tdropped_events\t\
         resident_inputs_exact\tintervention_isolated\tobject_not_consumed\t\
         stream_not_consumed\tpage_not_committed\tdispatch_yield_set\t\
         retry_same_object\tretry_same_stream\tdispatch_yield_reset\t\
         active_collision_policy_verified\truntime_verified\trelease_eligible\n\
         {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:08X}\t{}\t0x{:08X}\t\
         0x{:08X}\t0x{:08X}\t{}\t{}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t\
         {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        report.schema_version,
        report.image_sha256,
        report.pristine_ram_sha256,
        report.forced_ram_sha256,
        report.yielded_ram_sha256,
        report.retry_ram_sha256,
        report.entry_events_sha256,
        report.yield_events_sha256,
        report.retry_events_sha256,
        report.body_runtime,
        report.body_bytes,
        report.object_runtime,
        report.stream_runtime,
        report.first_active_runtime,
        report
            .active_state_indices
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(","),
        report.active_state_indices.len(),
        report.yield_runtime,
        report.page_state_runtime,
        report.forced_diff_bytes,
        report.collision_diff_bytes,
        report.collision_stack_diff_bytes,
        report.event_count,
        report.dropped_events,
        report.resident_inputs_exact,
        report.intervention_isolated,
        report.object_not_consumed,
        report.stream_not_consumed,
        report.page_not_committed,
        report.dispatch_yield_set,
        report.retry_same_object,
        report.retry_same_stream,
        report.dispatch_yield_reset,
        report.active_collision_policy_verified,
        report.runtime_verified,
        report.release_eligible,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheCollisionRuntimeError(String);

impl fmt::Display for QuestGlyphCacheCollisionRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestGlyphCacheCollisionRuntimeError {}

fn error(message: impl Into<String>) -> QuestGlyphCacheCollisionRuntimeError {
    QuestGlyphCacheCollisionRuntimeError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn runtime_slice<'a>(
    ram: &'a [u8],
    runtime: u32,
    len: usize,
    label: &str,
) -> Result<&'a [u8], QuestGlyphCacheCollisionRuntimeError> {
    let start = psx_main_ram_offset(runtime, len)
        .ok_or_else(|| error(format!("{label}: PS1 main RAM 범위 이탈")))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| error(format!("{label}: RAM 범위 overflow")))?;
    ram.get(start..end)
        .ok_or_else(|| error(format!("{label}: RAM dump 범위 이탈")))
}

fn runtime_u8(
    ram: &[u8],
    runtime: u32,
    label: &str,
) -> Result<u8, QuestGlyphCacheCollisionRuntimeError> {
    Ok(runtime_slice(ram, runtime, 1, label)?[0])
}

fn runtime_u32(
    ram: &[u8],
    runtime: u32,
    label: &str,
) -> Result<u32, QuestGlyphCacheCollisionRuntimeError> {
    let bytes: [u8; 4] = runtime_slice(ram, runtime, 4, label)?
        .try_into()
        .map_err(|_| error(format!("{label}: u32 길이 불일치")))?;
    Ok(u32::from_le_bytes(bytes))
}

fn event_u32(
    event: &Value,
    path: &[&str],
    label: &str,
) -> Result<u32, QuestGlyphCacheCollisionRuntimeError> {
    let value = path
        .iter()
        .try_fold(event, |value, key| value.get(*key))
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("QUEST collision event에 {label}가 없음")))?;
    u32::try_from(value).map_err(|_| error(format!("QUEST collision event {label} 범위 이탈")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CollisionEvent {
    pc: u32,
    a0: u32,
    gp: u32,
    ra: u32,
    sp: u32,
    v0: u32,
}

fn parse_single_event(
    raw: &[u8],
    label: &str,
) -> Result<CollisionEvent, QuestGlyphCacheCollisionRuntimeError> {
    let document: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST {label} event JSON 해석 실패: {source}")))?;
    let dropped = document
        .get("dropped")
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("QUEST {label} event JSON에 dropped가 없음")))?;
    let events = document
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| error(format!("QUEST {label} event JSON에 events가 없음")))?;
    if dropped != 0 || events.len() != 1 {
        return Err(error(format!(
            "QUEST {label} event 수 불일치: dropped={dropped}, events={}",
            events.len()
        )));
    }
    let event = &events[0];
    Ok(CollisionEvent {
        pc: event_u32(event, &["pc"], "pc")?,
        a0: event_u32(event, &["registers", "a0"], "a0")?,
        gp: event_u32(event, &["registers", "gp"], "gp")?,
        ra: event_u32(event, &["registers", "ra"], "ra")?,
        sp: event_u32(event, &["registers", "sp"], "sp")?,
        v0: event_u32(event, &["registers", "v0"], "v0")?,
    })
}

fn diff_offsets(left: &[u8], right: &[u8]) -> Vec<usize> {
    left.iter()
        .zip(right)
        .enumerate()
        .filter_map(|(offset, (left, right))| (left != right).then_some(offset))
        .collect()
}

fn font_active_values(ram: &[u8]) -> Result<Vec<u8>, QuestGlyphCacheCollisionRuntimeError> {
    (0..QUEST_FONT_STATE_RECORD_COUNT)
        .map(|index| {
            let runtime = font_active_runtime(index)?;
            runtime_u8(ram, runtime, "QUEST font active")
        })
        .collect()
}

fn font_active_runtime(index: usize) -> Result<u32, QuestGlyphCacheCollisionRuntimeError> {
    if index >= QUEST_FONT_STATE_RECORD_COUNT {
        return Err(error("QUEST font state index 범위 이탈"));
    }
    QUEST_FONT_STATE_FIRST_ACTIVE_RUNTIME
        .checked_add(
            u32::try_from(index * QUEST_FONT_STATE_RECORD_STRIDE)
                .map_err(|_| error("QUEST font state offset 범위 이탈"))?,
        )
        .ok_or_else(|| error("QUEST font active runtime overflow"))
}

fn active_intervention_indices(
    pristine_ram: &[u8],
    forced_ram: &[u8],
) -> Result<Vec<usize>, QuestGlyphCacheCollisionRuntimeError> {
    let pristine_active = font_active_values(pristine_ram)?;
    let forced_active = font_active_values(forced_ram)?;
    let indices = forced_active
        .iter()
        .enumerate()
        .filter_map(|(index, active)| (*active == 1).then_some(index))
        .collect::<Vec<_>>();
    if indices.is_empty()
        || pristine_active.iter().any(|active| *active != 0)
        || forced_active.iter().any(|active| *active > 1)
    {
        return Err(error(
            "QUEST collision active 개입은 pristine 0에서 하나 이상의 슬롯만 1이어야 함",
        ));
    }
    let expected_offsets = indices
        .iter()
        .map(|index| {
            let runtime = font_active_runtime(*index)?;
            psx_main_ram_offset(runtime, 1)
                .ok_or_else(|| error("QUEST collision active RAM offset 범위 이탈"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if diff_offsets(pristine_ram, forced_ram) != expected_offsets {
        return Err(error(
            "QUEST collision 개입이 선택한 font active byte 밖을 변경함",
        ));
    }
    Ok(indices)
}

fn resident_input_exact(
    ram: &[u8],
    expected_package: &[u8],
    hook: &QuestGlyphCacheHookPlan,
) -> Result<bool, QuestGlyphCacheCollisionRuntimeError> {
    Ok(runtime_slice(
        ram,
        QUEST_PAGE_CARRIER_START_RUNTIME,
        expected_package.len(),
        "QUEST collision resident package",
    )? == expected_package
        && runtime_slice(
            ram,
            hook.site.base_addr(),
            hook.site.bytes().len(),
            "QUEST collision hook site",
        )? == hook.site.bytes()
        && runtime_slice(
            ram,
            hook.body_runtime,
            hook.body.bytes().len(),
            "QUEST collision hook body",
        )? == hook.body.bytes())
}

fn expected_resident_package(
    package: &QuestGlyphCachePackage,
    scratch_page: Option<u8>,
) -> Result<Vec<u8>, QuestGlyphCacheCollisionRuntimeError> {
    let mut expected = package.bytes.clone();
    let Some(page_id) = scratch_page else {
        return Ok(expected);
    };
    let last_index = package
        .page_catalog_indices(page_id)
        .map_err(|source| error(source.to_string()))?
        .last()
        .copied()
        .ok_or_else(|| error("QUEST collision scratch page가 비어 있음"))?;
    let final_cell = package
        .glyph_cell(last_index)
        .map_err(|source| error(source.to_string()))?;
    let end = package
        .layout
        .scratch_offset
        .checked_add(final_cell.len())
        .ok_or_else(|| error("QUEST collision scratch end overflow"))?;
    expected
        .get_mut(package.layout.scratch_offset..end)
        .ok_or_else(|| error("QUEST collision scratch package 범위 이탈"))?
        .copy_from_slice(&final_cell);
    Ok(expected)
}

/// 첫 active upload에서 active byte 개입과 같은 OP21 재진입을 검증한다.
#[allow(clippy::too_many_arguments)]
pub fn verify_quest_glyph_cache_collision_runtime(
    build: &QuestGlyphCacheUploadPocBuild,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    pristine_ram: &[u8],
    forced_ram: &[u8],
    yielded_ram: &[u8],
    retry_ram: &[u8],
    entry_events: &[u8],
    yield_events: &[u8],
    retry_events: &[u8],
) -> Result<QuestGlyphCacheCollisionRuntimeReport, QuestGlyphCacheCollisionRuntimeError> {
    let page_glyphs = package
        .page_catalog_indices(QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID)
        .map_err(|source| error(source.to_string()))?
        .len();
    if build.report.disc.image_sha256 != digest(&build.image)
        || build.report.input.page_id != QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID
        || build.report.input.page_glyphs != page_glyphs
        || build.report.runtime_verified
        || build.report.release_eligible
        || package.sha256 != digest(&package.bytes)
        || build.report.disc.package_sha256 != package.sha256
        || build.report.disc.package_bytes != package.bytes.len()
        || build.report.disc.site_runtime != hook.site.base_addr()
        || build.report.disc.body_runtime != hook.body_runtime
        || build.report.disc.body_bytes != hook.body.bytes().len()
    {
        return Err(error(
            "QUEST sparse cache collision runtime build 계약 drift",
        ));
    }
    verify_collision_runtime_bound(
        CollisionRuntimeBinding {
            schema_version: QUEST_GLYPH_CACHE_COLLISION_RUNTIME_SCHEMA_VERSION,
            image_sha256: &build.report.disc.image_sha256,
            stream_runtime: QUEST_GLYPH_CACHE_COLLISION_STREAM_RUNTIME,
            stream_bytes: build.report.input.source_stream_bytes,
            stream_sha256: &build.report.input.replacement_stream_sha256,
            page_state: 0,
            resident_scratch_page: None,
        },
        package,
        hook,
        pristine_ram,
        forced_ram,
        yielded_ram,
        retry_ram,
        entry_events,
        yield_events,
        retry_events,
    )
}

/// page 62에서 page 50으로 바뀌기 직전의 active 충돌과 재진입을 검증한다.
#[allow(clippy::too_many_arguments)]
pub fn verify_quest_glyph_cache_transition_collision_runtime(
    build: &QuestGlyphCacheTransitionPocBuild,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    pristine_ram: &[u8],
    forced_ram: &[u8],
    yielded_ram: &[u8],
    retry_ram: &[u8],
    entry_events: &[u8],
    yield_events: &[u8],
    retry_events: &[u8],
) -> Result<QuestGlyphCacheCollisionRuntimeReport, QuestGlyphCacheCollisionRuntimeError> {
    let [first, second, third] = build.report.streams.as_slice() else {
        return Err(error("QUEST transition collision stream 수 불일치"));
    };
    if build.report.disc.image_sha256 != digest(&build.image)
        || first.page_id != QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID
        || second.page_id != QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID
        || third.page_id != QUEST_GLYPH_CACHE_TRANSITION_THIRD_PAGE_ID
        || third.record_offset != QUEST_GLYPH_CACHE_TRANSITION_THIRD_RECORD_OFFSET
        || build.report.runtime_verified
        || build.report.release_eligible
        || package.sha256 != digest(&package.bytes)
        || build.report.disc.package_sha256 != package.sha256
        || build.report.disc.package_bytes != package.bytes.len()
        || build.report.disc.site_runtime != hook.site.base_addr()
        || build.report.disc.body_runtime != hook.body_runtime
        || build.report.disc.body_bytes != hook.body.bytes().len()
    {
        return Err(error(
            "QUEST sparse cache transition collision build 계약 drift",
        ));
    }
    verify_collision_runtime_bound(
        CollisionRuntimeBinding {
            schema_version: QUEST_GLYPH_CACHE_TRANSITION_COLLISION_RUNTIME_SCHEMA_VERSION,
            image_sha256: &build.report.disc.image_sha256,
            stream_runtime: TOWN00_SEQ_RUNTIME_BASE
                + QUEST_GLYPH_CACHE_TRANSITION_THIRD_RECORD_OFFSET as u32,
            stream_bytes: third.replacement_stream_bytes,
            stream_sha256: &third.replacement_stream_sha256,
            page_state: QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID,
            resident_scratch_page: Some(QUEST_GLYPH_CACHE_UPLOAD_POC_PAGE_ID),
        },
        package,
        hook,
        pristine_ram,
        forced_ram,
        yielded_ram,
        retry_ram,
        entry_events,
        yield_events,
        retry_events,
    )
}

#[allow(clippy::too_many_arguments)]
fn verify_collision_runtime_bound(
    binding: CollisionRuntimeBinding<'_>,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    pristine_ram: &[u8],
    forced_ram: &[u8],
    yielded_ram: &[u8],
    retry_ram: &[u8],
    entry_events: &[u8],
    yield_events: &[u8],
    retry_events: &[u8],
) -> Result<QuestGlyphCacheCollisionRuntimeReport, QuestGlyphCacheCollisionRuntimeError> {
    if hook.selects_unpinned_atlas_bank {
        return Err(error(
            "이전 one-bank active collision bundle로 현재 두-bank atlas hook을 검증할 수 없음",
        ));
    }
    for (label, ram) in [
        ("pristine", pristine_ram),
        ("forced", forced_ram),
        ("yielded", yielded_ram),
        ("retry", retry_ram),
    ] {
        if ram.len() != PSX_MAIN_RAM_LEN {
            return Err(error(format!(
                "QUEST collision {label} RAM 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
                ram.len()
            )));
        }
    }
    if !is_sha256_hex(binding.image_sha256)
        || !is_sha256_hex(&package.sha256)
        || hook.yields_only_when_all_atlas_banks_pinned
        || hook.atlas_bank_policy_runtime_verified
        || hook.runtime_verified
        || hook.build_eligible
    {
        return Err(error(
            "QUEST sparse cache collision runtime 입력 계약 drift",
        ));
    }

    let entry_event = parse_single_event(entry_events, "collision entry")?;
    let yield_event = parse_single_event(yield_events, "collision yield")?;
    let retry_event = parse_single_event(retry_events, "collision retry")?;
    if entry_event.pc != hook.body_runtime
        || entry_event.gp != QUEST_HOOK_GP_RUNTIME
        || entry_event.ra != QUEST_GLYPH_CACHE_BODY_RETURN_RUNTIME
        || yield_event.pc != QUEST_GLYPH_CACHE_BODY_RETURN_RUNTIME
        || yield_event.a0 != entry_event.a0
        || yield_event.gp != entry_event.gp
        || yield_event.ra != entry_event.ra
        || yield_event.sp != entry_event.sp
        || yield_event.v0 != 1
        || retry_event.pc != hook.body_runtime
        || retry_event.a0 != entry_event.a0
        || retry_event.gp != entry_event.gp
        || retry_event.ra != entry_event.ra
        || retry_event.sp != entry_event.sp
    {
        return Err(error("QUEST collision breakpoint register 계약 불일치"));
    }

    let object_runtime = entry_event.a0;
    let object_active_runtime = object_runtime + QUEST_GLYPH_CACHE_OBJECT_ACTIVE_OFFSET;
    let object_stream_runtime = object_runtime + QUEST_GLYPH_CACHE_OBJECT_STREAM_OFFSET;
    let yield_offset = psx_main_ram_offset(QUEST_GLYPH_CACHE_COLLISION_YIELD_RUNTIME, 4)
        .ok_or_else(|| error("QUEST collision yield RAM offset 범위 이탈"))?;
    let stack_start_runtime = entry_event
        .sp
        .checked_sub(QUEST_GLYPH_CACHE_BODY_STACK_BYTES)
        .ok_or_else(|| error("QUEST collision body stack underflow"))?;
    let stack_start = psx_main_ram_offset(
        stack_start_runtime,
        QUEST_GLYPH_CACHE_BODY_STACK_BYTES as usize,
    )
    .ok_or_else(|| error("QUEST collision body stack RAM 범위 이탈"))?;
    let stack_end = stack_start + QUEST_GLYPH_CACHE_BODY_STACK_BYTES as usize;

    let forced_diffs = diff_offsets(pristine_ram, forced_ram);
    let active_state_indices = active_intervention_indices(pristine_ram, forced_ram)?;
    let intervention_isolated = true;
    let collision_diffs = diff_offsets(forced_ram, yielded_ram);
    let collision_writes_bounded = collision_diffs
        .iter()
        .all(|offset| *offset == yield_offset || (stack_start..stack_end).contains(offset));
    let collision_stack_diff_bytes = collision_diffs
        .iter()
        .filter(|offset| (stack_start..stack_end).contains(offset))
        .count();

    let pristine_active = font_active_values(pristine_ram)?;
    let forced_active = font_active_values(forced_ram)?;
    let yielded_active = font_active_values(yielded_ram)?;
    let retry_active = font_active_values(retry_ram)?;
    let expected_forced_active = (0..QUEST_FONT_STATE_RECORD_COUNT)
        .map(|index| u8::from(active_state_indices.contains(&index)))
        .collect::<Vec<_>>();

    let object_not_consumed = [pristine_ram, forced_ram, yielded_ram]
        .iter()
        .all(|ram| runtime_u32(ram, object_active_runtime, "QUEST object active") == Ok(0));
    let stream_not_consumed = [pristine_ram, forced_ram, yielded_ram].iter().all(|ram| {
        runtime_u32(ram, object_stream_runtime, "QUEST object stream") == Ok(binding.stream_runtime)
    });
    let page_not_committed = [pristine_ram, forced_ram, yielded_ram, retry_ram]
        .iter()
        .all(|ram| {
            runtime_u8(ram, QUEST_PAGE_HOOK_STATE_RUNTIME, "QUEST page state")
                == Ok(binding.page_state)
        });
    let dispatch_yield_set = runtime_u32(
        forced_ram,
        QUEST_GLYPH_CACHE_COLLISION_YIELD_RUNTIME,
        "QUEST forced yield",
    )? == 0
        && runtime_u32(
            yielded_ram,
            QUEST_GLYPH_CACHE_COLLISION_YIELD_RUNTIME,
            "QUEST yielded yield",
        )? == 1;
    let retry_same_object = runtime_u32(retry_ram, object_active_runtime, "QUEST retry active")?
        == 0
        && retry_event.a0 == object_runtime;
    let retry_same_stream = runtime_u32(retry_ram, object_stream_runtime, "QUEST retry stream")?
        == binding.stream_runtime;
    let dispatch_yield_reset = runtime_u32(
        retry_ram,
        QUEST_GLYPH_CACHE_COLLISION_YIELD_RUNTIME,
        "QUEST retry yield",
    )? == 0;
    let expected_package = expected_resident_package(package, binding.resident_scratch_page)?;
    let resident_inputs_exact = [pristine_ram, forced_ram, yielded_ram, retry_ram]
        .iter()
        .map(|ram| {
            Ok(resident_input_exact(ram, &expected_package, hook)?
                && digest(runtime_slice(
                    ram,
                    binding.stream_runtime,
                    binding.stream_bytes,
                    "QUEST collision replacement stream",
                )?) == binding.stream_sha256)
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .all(|exact| exact);

    let active_states_exact = pristine_active.iter().all(|active| *active == 0)
        && forced_active == expected_forced_active
        && yielded_active == expected_forced_active
        && retry_active.iter().all(|active| *active == 0);
    if !resident_inputs_exact
        || !intervention_isolated
        || !collision_writes_bounded
        || !active_states_exact
        || !object_not_consumed
        || !stream_not_consumed
        || !page_not_committed
        || !dispatch_yield_set
        || !retry_same_object
        || !retry_same_stream
        || !dispatch_yield_reset
    {
        return Err(error(format!(
            "QUEST collision runtime 불일치: resident/intervention/writes/active/object/stream/page/yield/retry_object/retry_stream/reset={resident_inputs_exact}/{intervention_isolated}/{collision_writes_bounded}/{active_states_exact}/{object_not_consumed}/{stream_not_consumed}/{page_not_committed}/{dispatch_yield_set}/{retry_same_object}/{retry_same_stream}/{dispatch_yield_reset}"
        )));
    }

    Ok(QuestGlyphCacheCollisionRuntimeReport {
        schema_version: binding.schema_version,
        image_sha256: binding.image_sha256.to_owned(),
        pristine_ram_sha256: digest(pristine_ram),
        forced_ram_sha256: digest(forced_ram),
        yielded_ram_sha256: digest(yielded_ram),
        retry_ram_sha256: digest(retry_ram),
        entry_events_sha256: digest(entry_events),
        yield_events_sha256: digest(yield_events),
        retry_events_sha256: digest(retry_events),
        body_runtime: hook.body_runtime,
        body_bytes: hook.body.bytes().len(),
        object_runtime,
        stream_runtime: binding.stream_runtime,
        first_active_runtime: font_active_runtime(active_state_indices[0])?,
        active_state_indices,
        yield_runtime: QUEST_GLYPH_CACHE_COLLISION_YIELD_RUNTIME,
        page_state_runtime: QUEST_PAGE_HOOK_STATE_RUNTIME,
        forced_diff_bytes: forced_diffs.len(),
        collision_diff_bytes: collision_diffs.len(),
        collision_stack_diff_bytes,
        event_count: 3,
        dropped_events: 0,
        resident_inputs_exact,
        intervention_isolated,
        object_not_consumed,
        stream_not_consumed,
        page_not_committed,
        dispatch_yield_set,
        retry_same_object,
        retry_same_stream,
        dispatch_yield_reset,
        active_collision_policy_verified: true,
        runtime_verified: true,
        release_eligible: false,
    })
}

#[cfg(test)]
#[path = "quest_glyph_cache_collision_runtime_tests.rs"]
mod tests;
