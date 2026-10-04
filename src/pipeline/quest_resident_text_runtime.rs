//! QUEST 상주 문구의 upload·재사용·창 geometry runtime 검증.
//!
//! 같은 development image에서 시작 메뉴의 page upload와 작별 문구의
//! resident reuse를 각각 exact RAM/GPU/event/screenshot에 결속한다. 창은
//! 역할을 드러내는 selection renderer 또는 input indicator로 고르며, 관측한
//! 좌표를 보고할 뿐 사람 조판·표시 승인을 대신하지 않는다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::analysis::{QUEST_RESIDENT_FAREWELL_TEXT_ID, QUEST_RESIDENT_START_MENU_TEXT_ID};
use crate::coords::PSX_MAIN_RAM_LEN;
use crate::disasm::{
    QUEST_FONT_HOOK_CONTINUATION_RUNTIME, QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME,
    QUEST_FONT_UPLOAD_TARGET_RUNTIME, QUEST_PAGE_CARRIER_START_RUNTIME,
};
use crate::isa::{InstructionKind as K, Reg};
use crate::runtime::{
    inspect_quest_font_windows_ram, QuestFontWindowCensus, QuestFontWindowGeometry,
    QuestFontWindowSlot,
};

use super::quest_development_disc::{
    QuestDevelopmentDiscBuild, QuestDevelopmentDiscResidentTextReport,
    QUEST_DEVELOPMENT_DISC_SCHEMA_VERSION,
};
use super::quest_glyph_atlas_bank_policy::QuestGlyphAtlasBank;
use super::quest_glyph_cache_hook_plan::{
    QuestGlyphCacheHookPlan, QUEST_GLYPH_CACHE_RENDERER_PAGE_RUNTIME,
    QUEST_GLYPH_CACHE_ROUTE_PAGE_RUNTIME,
};
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_glyph_cache_residency::{
    expected_resident_package, png_dimensions, QuestGlyphCacheScratchExpectation,
};
use super::quest_glyph_cache_upload_runtime::{
    runtime_slice, slot_rect, uploaded_cell, verify_upload_events,
};
use super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;
use super::quest_resident_text_writer::QuestResidentTextWrite;

pub const QUEST_RESIDENT_TEXT_RUNTIME_SCHEMA_VERSION: &str = "quest-resident-text-runtime-v1";

const SCREENSHOT_WIDTH: u32 = 280;
const SCREENSHOT_HEIGHT: u32 = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestResidentTextWindowRole {
    Selection,
    InputIndicator,
}

impl QuestResidentTextWindowRole {
    const fn label(self) -> &'static str {
        match self {
            Self::Selection => "selection-renderer",
            Self::InputIndicator => "input-indicator",
        }
    }
}

/// 한 화면에서 역할로 고른 text window와 실제 parent geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextWindowObservation {
    pub role: QuestResidentTextWindowRole,
    pub slot_index: usize,
    pub state_runtime: u32,
    pub parent_runtime: u32,
    pub cell_width: u8,
    pub cell_height: u8,
    pub line_height: u8,
    pub geometry: QuestFontWindowGeometry,
}

/// 상주 문구 한 화면의 exact runtime 증거.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextRuntimeScreen {
    pub screen_id: &'static str,
    pub unit_id: String,
    pub record_runtime: u32,
    pub page_id: u8,
    pub page_glyphs: usize,
    pub ram_sha256: String,
    pub gpu_sha256: String,
    pub events_sha256: String,
    pub screenshot_sha256: String,
    pub screenshot_width: u32,
    pub screenshot_height: u32,
    pub event_count: usize,
    pub dropped_events: u64,
    pub load_image_calls: usize,
    pub draw_sync_calls: usize,
    pub primary_commit_events: usize,
    pub primary_resident_events: usize,
    pub expected_cells_sha256: String,
    pub uploaded_cells_sha256: String,
    pub window: QuestResidentTextWindowObservation,
    pub package_exact: bool,
    pub site_exact: bool,
    pub body_exact: bool,
    pub page_state_exact: bool,
    pub uploaded_cells_exact: bool,
    pub event_path_exact: bool,
    pub screenshot_shape_exact: bool,
    pub runtime_verified: bool,
}

/// 시작 메뉴 upload와 작별 문구 reuse를 같은 development image에 묶은 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextRuntimeReport {
    pub schema_version: &'static str,
    pub image_sha256: String,
    pub package_runtime: u32,
    pub package_bytes: usize,
    pub package_sha256: String,
    pub site_runtime: u32,
    pub site_bytes: usize,
    pub body_runtime: u32,
    pub body_bytes: usize,
    pub screens: Vec<QuestResidentTextRuntimeScreen>,
    pub natural_route_verified: bool,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub display_approved: bool,
    pub runtime_verified: bool,
    pub release_eligible: bool,
}

/// 검증 명령이 읽는 두 frozen checkpoint.
pub struct QuestResidentTextRuntimeEvidence<'a> {
    pub start_menu_ram: &'a [u8],
    pub start_menu_gpu: &'a [u8],
    pub start_menu_events: &'a [u8],
    pub start_menu_screenshot: &'a [u8],
    pub farewell_ram: &'a [u8],
    pub farewell_gpu: &'a [u8],
    pub farewell_events: &'a [u8],
    pub farewell_screenshot: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextRuntimeError(String);

impl fmt::Display for QuestResidentTextRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentTextRuntimeError {}

fn error(message: impl Into<String>) -> QuestResidentTextRuntimeError {
    QuestResidentTextRuntimeError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HookRuntimePoints {
    upload_return: u32,
    sync_return: u32,
    primary_commit: u32,
    secondary_commit: u32,
    primary_resident: u32,
    return_to_consumer: u32,
}

fn derive_hook_runtime_points(
    hook: &QuestGlyphCacheHookPlan,
) -> Result<HookRuntimePoints, QuestResidentTextRuntimeError> {
    let find = |predicate: &dyn Fn(K) -> bool, label: &str| {
        hook.body
            .instructions()
            .iter()
            .find(|instruction| predicate(instruction.kind))
            .map(|instruction| instruction.addr)
            .ok_or_else(|| error(format!("QUEST resident hook에 {label}이 없음")))
    };
    let upload_call = find(
        &|kind| {
            kind == K::Jal {
                target: QUEST_FONT_UPLOAD_TARGET_RUNTIME,
            }
        },
        "LoadImage call",
    )?;
    let sync_call = find(
        &|kind| {
            kind == K::Jal {
                target: QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME,
            }
        },
        "DrawSync call",
    )?;
    let primary_commit = find(
        &|kind| {
            kind == K::Sb {
                rt: Reg::S1,
                base: Reg::T0,
                offset: QuestGlyphAtlasBank::Primary.state_offset(),
            }
        },
        "primary page commit",
    )?;
    let secondary_commit = find(
        &|kind| {
            kind == K::Sb {
                rt: Reg::S1,
                base: Reg::T0,
                offset: QuestGlyphAtlasBank::Secondary.state_offset(),
            }
        },
        "secondary page commit",
    )?;
    let primary_resident = find(
        &|kind| {
            kind == K::Sw {
                rt: Reg::S1,
                base: Reg::SP,
                offset: 8,
            }
        },
        "primary resident branch",
    )?;
    let return_to_consumer = find(
        &|kind| {
            kind == K::J {
                target: QUEST_FONT_HOOK_CONTINUATION_RUNTIME,
            }
        },
        "consumer return",
    )?;
    Ok(HookRuntimePoints {
        upload_return: upload_call
            .checked_add(8)
            .ok_or_else(|| error("QUEST LoadImage return 주소 overflow"))?,
        sync_return: sync_call
            .checked_add(8)
            .ok_or_else(|| error("QUEST DrawSync return 주소 overflow"))?,
        primary_commit,
        secondary_commit,
        primary_resident,
        return_to_consumer,
    })
}

fn event_u64(event: &Value, path: &[&str]) -> Option<u64> {
    path.iter()
        .try_fold(event, |value, key| value.get(*key))?
        .as_u64()
}

fn event_register(event: &Value, register: &str) -> Option<u64> {
    event_u64(event, &["registers", register])
}

struct ParsedEvents {
    dropped: u64,
    events: Vec<Value>,
}

fn parse_events(raw: &[u8]) -> Result<ParsedEvents, QuestResidentTextRuntimeError> {
    let document: Value = serde_json::from_slice(raw)
        .map_err(|source| error(format!("QUEST resident event JSON 해석 실패: {source}")))?;
    let dropped = document
        .get("dropped")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("QUEST resident event JSON에 dropped가 없음"))?;
    let events = document
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| error("QUEST resident event JSON에 events 배열이 없음"))?;
    Ok(ParsedEvents {
        dropped,
        events: events.clone(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EventPathEvidence {
    event_count: usize,
    dropped_events: u64,
    load_image_calls: usize,
    draw_sync_calls: usize,
    primary_commit_events: usize,
    primary_resident_events: usize,
}

fn verify_start_menu_event_path(
    hook: &QuestGlyphCacheHookPlan,
    record_runtime: u32,
    page_id: u8,
    expected_calls: usize,
    scratch_runtime: u32,
    raw: &[u8],
) -> Result<EventPathEvidence, QuestResidentTextRuntimeError> {
    let points = derive_hook_runtime_points(hook)?;
    let parsed = parse_events(raw)?;
    let dropped_events = parsed.dropped;
    let events = &parsed.events;
    let upload_evidence = verify_upload_events(hook, expected_calls, raw)
        .map_err(|source| error(source.to_string()))?;
    let upload_events = events
        .iter()
        .filter(|event| {
            event_u64(event, &["pc"]) == Some(u64::from(QUEST_FONT_UPLOAD_TARGET_RUNTIME))
                && event_register(event, "ra") == Some(u64::from(points.upload_return))
        })
        .collect::<Vec<_>>();
    let sync_events = events
        .iter()
        .filter(|event| {
            event_u64(event, &["pc"]) == Some(u64::from(QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME))
                && event_register(event, "ra") == Some(u64::from(points.sync_return))
        })
        .collect::<Vec<_>>();
    let primary_commits = events
        .iter()
        .filter(|event| event_u64(event, &["pc"]) == Some(u64::from(points.primary_commit)))
        .collect::<Vec<_>>();
    let secondary_commits = events
        .iter()
        .filter(|event| event_u64(event, &["pc"]) == Some(u64::from(points.secondary_commit)))
        .count();
    let primary_residents = events
        .iter()
        .filter(|event| event_u64(event, &["pc"]) == Some(u64::from(points.primary_resident)))
        .count();
    let returns = events
        .iter()
        .filter(|event| event_u64(event, &["pc"]) == Some(u64::from(points.return_to_consumer)))
        .count();
    let calls_exact = upload_events.iter().enumerate().all(|(index, event)| {
        event_register(event, "s0") == Some(u64::from(record_runtime))
            && event_register(event, "s1") == Some(u64::from(page_id))
            && event_register(event, "s5") == u64::try_from(index).ok()
            && event_register(event, "a1") == Some(u64::from(scratch_runtime))
    }) && sync_events.iter().enumerate().all(|(index, event)| {
        event_register(event, "s0") == Some(u64::from(record_runtime))
            && event_register(event, "s1") == Some(u64::from(page_id))
            && event_register(event, "s5") == u64::try_from(index).ok()
            && event_register(event, "a0") == Some(0)
    });
    let commit_exact = primary_commits.len() == 1
        && event_register(primary_commits[0], "s0") == Some(u64::from(record_runtime))
        && event_register(primary_commits[0], "s1") == Some(u64::from(page_id))
        && event_register(primary_commits[0], "s5") == u64::try_from(expected_calls).ok()
        && event_register(primary_commits[0], "t0")
            == Some(u64::from(QUEST_PAGE_HOOK_STATE_RUNTIME));
    if dropped_events != 0
        || upload_evidence.dropped_events != 0
        || upload_evidence.load_image_calls != expected_calls
        || upload_evidence.draw_sync_calls != expected_calls
        || upload_evidence.page_commit_events != 1
        || !calls_exact
        || !commit_exact
        || secondary_commits != 0
        || primary_residents != 0
        || returns != 1
    {
        return Err(error(format!(
            "QUEST 시작 메뉴 upload event path 불일치: dropped={dropped_events}, \
             load={}, sync={}, primary_commit={}, secondary_commit={secondary_commits}, \
             primary_resident={primary_residents}, return={returns}, \
             calls_exact={calls_exact}, commit_exact={commit_exact}",
            upload_events.len(),
            sync_events.len(),
            primary_commits.len(),
        )));
    }
    Ok(EventPathEvidence {
        event_count: events.len(),
        dropped_events,
        load_image_calls: upload_events.len(),
        draw_sync_calls: sync_events.len(),
        primary_commit_events: primary_commits.len(),
        primary_resident_events: 0,
    })
}

fn verify_farewell_event_path(
    hook: &QuestGlyphCacheHookPlan,
    record_runtime: u32,
    page_id: u8,
    raw: &[u8],
) -> Result<EventPathEvidence, QuestResidentTextRuntimeError> {
    let points = derive_hook_runtime_points(hook)?;
    verify_farewell_event_path_with_points(points, record_runtime, page_id, raw)
}

fn verify_farewell_event_path_with_points(
    points: HookRuntimePoints,
    record_runtime: u32,
    page_id: u8,
    raw: &[u8],
) -> Result<EventPathEvidence, QuestResidentTextRuntimeError> {
    let parsed = parse_events(raw)?;
    let dropped_events = parsed.dropped;
    let events = &parsed.events;
    let hook_uploads = events
        .iter()
        .filter(|event| {
            event_u64(event, &["pc"]) == Some(u64::from(QUEST_FONT_UPLOAD_TARGET_RUNTIME))
                && event_register(event, "ra") == Some(u64::from(points.upload_return))
        })
        .count();
    let hook_syncs = events
        .iter()
        .filter(|event| {
            event_u64(event, &["pc"]) == Some(u64::from(QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME))
                && event_register(event, "ra") == Some(u64::from(points.sync_return))
        })
        .count();
    let primary_commits = events
        .iter()
        .filter(|event| event_u64(event, &["pc"]) == Some(u64::from(points.primary_commit)))
        .count();
    let secondary_commits = events
        .iter()
        .filter(|event| event_u64(event, &["pc"]) == Some(u64::from(points.secondary_commit)))
        .count();
    let resident_events = events
        .iter()
        .filter(|event| event_u64(event, &["pc"]) == Some(u64::from(points.primary_resident)))
        .collect::<Vec<_>>();
    let returns = events
        .iter()
        .filter(|event| event_u64(event, &["pc"]) == Some(u64::from(points.return_to_consumer)))
        .count();
    let resident_exact = resident_events.len() == 1
        && event_register(resident_events[0], "s0") == Some(u64::from(record_runtime))
        && event_register(resident_events[0], "s1") == Some(u64::from(page_id))
        && event_register(resident_events[0], "t0")
            == Some(u64::from(QUEST_PAGE_HOOK_STATE_RUNTIME))
        && event_register(resident_events[0], "t1") == Some(u64::from(page_id));
    if dropped_events != 0
        || hook_uploads != 0
        || hook_syncs != 0
        || primary_commits != 0
        || secondary_commits != 0
        || !resident_exact
        || returns != 1
    {
        return Err(error(format!(
            "QUEST 작별 resident event path 불일치: dropped={dropped_events}, \
             load={hook_uploads}, sync={hook_syncs}, primary_commit={primary_commits}, \
             secondary_commit={secondary_commits}, resident={}, return={returns}, \
             resident_exact={resident_exact}",
            resident_events.len(),
        )));
    }
    Ok(EventPathEvidence {
        event_count: events.len(),
        dropped_events,
        load_image_calls: hook_uploads,
        draw_sync_calls: hook_syncs,
        primary_commit_events: primary_commits,
        primary_resident_events: resident_events.len(),
    })
}

fn resident_write<'a>(
    resident: &'a QuestDevelopmentDiscResidentTextReport,
    unit_id: &str,
) -> Result<&'a QuestResidentTextWrite, QuestResidentTextRuntimeError> {
    let matches = resident
        .writes
        .iter()
        .filter(|write| write.unit_id == unit_id)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [write] => Ok(write),
        _ => Err(error(format!(
            "QUEST resident unit 분모 불일치: unit={unit_id}, count={}",
            matches.len()
        ))),
    }
}

fn verify_resident_ram(
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    page_id: u8,
    ram: &[u8],
) -> Result<(bool, bool, bool, bool), QuestResidentTextRuntimeError> {
    if ram.len() != PSX_MAIN_RAM_LEN {
        return Err(error(format!(
            "QUEST resident RAM 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
            ram.len()
        )));
    }
    let expected_package = expected_resident_package(
        package,
        QuestGlyphCacheScratchExpectation::UploadedPage(page_id),
    )
    .map_err(|source| error(source.to_string()))?;
    let loaded_package = runtime_slice(
        ram,
        QUEST_PAGE_CARRIER_START_RUNTIME,
        expected_package.len(),
        "QUEST resident package",
    )
    .map_err(|source| error(source.to_string()))?;
    let loaded_site = runtime_slice(
        ram,
        hook.site.base_addr(),
        hook.site.bytes().len(),
        "QUEST resident hook site",
    )
    .map_err(|source| error(source.to_string()))?;
    let loaded_body = runtime_slice(
        ram,
        hook.body_runtime,
        hook.body.bytes().len(),
        "QUEST resident hook body",
    )
    .map_err(|source| error(source.to_string()))?;
    let page_state = [
        runtime_slice(ram, QUEST_PAGE_HOOK_STATE_RUNTIME, 1, "primary page state")
            .map_err(|source| error(source.to_string()))?[0],
        runtime_slice(
            ram,
            QUEST_GLYPH_CACHE_ROUTE_PAGE_RUNTIME,
            1,
            "route page state",
        )
        .map_err(|source| error(source.to_string()))?[0],
        runtime_slice(
            ram,
            QUEST_GLYPH_CACHE_RENDERER_PAGE_RUNTIME,
            1,
            "renderer page state",
        )
        .map_err(|source| error(source.to_string()))?[0],
        runtime_slice(
            ram,
            QUEST_PAGE_HOOK_STATE_RUNTIME
                + u32::try_from(QuestGlyphAtlasBank::Secondary.state_offset())
                    .map_err(|_| error("secondary page state offset 변환 실패"))?,
            1,
            "secondary page state",
        )
        .map_err(|source| error(source.to_string()))?[0],
    ];
    let package_exact = loaded_package == expected_package;
    let site_exact = loaded_site == hook.site.bytes();
    let body_exact = loaded_body == hook.body.bytes();
    let page_state_exact = page_state == [page_id, page_id, page_id, 0];
    if !package_exact || !site_exact || !body_exact || !page_state_exact {
        return Err(error(format!(
            "QUEST resident RAM 불일치: package/site/body/page={package_exact}/{site_exact}/{body_exact}/{page_state_exact}, page_state={page_state:?}"
        )));
    }
    Ok((package_exact, site_exact, body_exact, page_state_exact))
}

fn verify_page_cells(
    package: &QuestGlyphCachePackage,
    page_id: u8,
    gpu: &[u8],
) -> Result<(usize, String, String), QuestResidentTextRuntimeError> {
    let indices = package
        .page_catalog_indices(page_id)
        .map_err(|source| error(source.to_string()))?;
    let mut expected_cells = Vec::new();
    let mut actual_cells = Vec::new();
    for (slot, index) in indices.iter().copied().enumerate() {
        let expected = package
            .glyph_cell(index)
            .map_err(|source| error(source.to_string()))?;
        let (x_words, y) = slot_rect(package, slot).map_err(|source| error(source.to_string()))?;
        let actual = uploaded_cell(gpu, x_words, y).map_err(|source| error(source.to_string()))?;
        expected_cells.extend_from_slice(&expected);
        actual_cells.extend_from_slice(&actual);
    }
    if expected_cells != actual_cells {
        return Err(error(format!(
            "QUEST resident page VRAM cell 불일치: page={page_id}, glyphs={}",
            indices.len()
        )));
    }
    Ok((
        indices.len(),
        digest(&expected_cells),
        digest(&actual_cells),
    ))
}

fn select_window(
    census: &QuestFontWindowCensus,
    role: QuestResidentTextWindowRole,
) -> Result<QuestResidentTextWindowObservation, QuestResidentTextRuntimeError> {
    let matches = census
        .slots
        .iter()
        .filter(|slot| match role {
            QuestResidentTextWindowRole::Selection => slot.selection_renderer_runtime != 0,
            QuestResidentTextWindowRole::InputIndicator => slot.input_indicator_runtime != 0,
        })
        .collect::<Vec<_>>();
    let [slot] = matches.as_slice() else {
        return Err(error(format!(
            "QUEST resident window 역할 분모 불일치: role={}, count={}",
            role.label(),
            matches.len()
        )));
    };
    window_observation(slot, role)
}

fn window_observation(
    slot: &QuestFontWindowSlot,
    role: QuestResidentTextWindowRole,
) -> Result<QuestResidentTextWindowObservation, QuestResidentTextRuntimeError> {
    let geometry = slot.geometry.ok_or_else(|| {
        error(format!(
            "QUEST resident window parent geometry 누락: role={}, slot={}",
            role.label(),
            slot.index
        ))
    })?;
    if slot.cell_width == 0 || slot.cell_height == 0 || slot.line_height == 0 {
        return Err(error(format!(
            "QUEST resident window cell geometry가 0임: role={}, slot={}",
            role.label(),
            slot.index
        )));
    }
    Ok(QuestResidentTextWindowObservation {
        role,
        slot_index: slot.index,
        state_runtime: slot.state_runtime,
        parent_runtime: slot.parent_runtime,
        cell_width: slot.cell_width,
        cell_height: slot.cell_height,
        line_height: slot.line_height,
        geometry,
    })
}

struct ScreenInputs<'a> {
    screen_id: &'static str,
    write: &'a QuestResidentTextWrite,
    ram: &'a [u8],
    gpu: &'a [u8],
    events: &'a [u8],
    screenshot: &'a [u8],
    window_role: QuestResidentTextWindowRole,
    event_path: EventPathEvidence,
}

fn verify_screen(
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    inputs: ScreenInputs<'_>,
) -> Result<QuestResidentTextRuntimeScreen, QuestResidentTextRuntimeError> {
    let ScreenInputs {
        screen_id,
        write,
        ram,
        gpu,
        events,
        screenshot,
        window_role,
        event_path,
    } = inputs;
    let (package_exact, site_exact, body_exact, page_state_exact) =
        verify_resident_ram(package, hook, write.page_id, ram)?;
    let (page_glyphs, expected_cells_sha256, uploaded_cells_sha256) =
        verify_page_cells(package, write.page_id, gpu)?;
    let census = inspect_quest_font_windows_ram(ram).map_err(|source| error(source.to_string()))?;
    let window = select_window(&census, window_role)?;
    let (screenshot_width, screenshot_height) = png_dimensions(screenshot)
        .ok_or_else(|| error(format!("QUEST {screen_id} screenshot PNG 해석 실패")))?;
    let screenshot_shape_exact =
        (screenshot_width, screenshot_height) == (SCREENSHOT_WIDTH, SCREENSHOT_HEIGHT);
    if !screenshot_shape_exact {
        return Err(error(format!(
            "QUEST {screen_id} screenshot 크기 불일치: expected={SCREENSHOT_WIDTH}x{SCREENSHOT_HEIGHT}, actual={screenshot_width}x{screenshot_height}"
        )));
    }
    Ok(QuestResidentTextRuntimeScreen {
        screen_id,
        unit_id: write.unit_id.clone(),
        record_runtime: write.record_runtime,
        page_id: write.page_id,
        page_glyphs,
        ram_sha256: digest(ram),
        gpu_sha256: digest(gpu),
        events_sha256: digest(events),
        screenshot_sha256: digest(screenshot),
        screenshot_width,
        screenshot_height,
        event_count: event_path.event_count,
        dropped_events: event_path.dropped_events,
        load_image_calls: event_path.load_image_calls,
        draw_sync_calls: event_path.draw_sync_calls,
        primary_commit_events: event_path.primary_commit_events,
        primary_resident_events: event_path.primary_resident_events,
        expected_cells_sha256,
        uploaded_cells_sha256,
        window,
        package_exact,
        site_exact,
        body_exact,
        page_state_exact,
        uploaded_cells_exact: true,
        event_path_exact: true,
        screenshot_shape_exact,
        runtime_verified: true,
    })
}

/// current development build와 두 checkpoint의 upload·reuse 소비를 검증한다.
pub fn verify_quest_resident_text_runtime(
    build: &QuestDevelopmentDiscBuild,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    evidence: QuestResidentTextRuntimeEvidence<'_>,
) -> Result<QuestResidentTextRuntimeReport, QuestResidentTextRuntimeError> {
    let resident = &build.report.resident_text;
    if build.report.schema_version != QUEST_DEVELOPMENT_DISC_SCHEMA_VERSION
        || build.report.disc.image_sha256 != digest(&build.image)
        || build.report.disc.package_sha256 != package.sha256
        || build.report.disc.package_bytes != package.bytes.len()
        || build.report.disc.site_runtime != hook.site.base_addr()
        || build.report.disc.body_runtime != hook.body_runtime
        || build.report.disc.body_bytes != hook.body.bytes().len()
        || !resident.final_exact
        || resident.translation_approved
        || resident.layout_approved
        || resident.runtime_verified
        || resident.product_eligible
        || build.report.runtime_verified
        || build.report.product_eligible
    {
        return Err(error("QUEST resident runtime build 입력 계약 drift"));
    }
    let start_write = resident_write(resident, QUEST_RESIDENT_START_MENU_TEXT_ID)?;
    let farewell_write = resident_write(resident, QUEST_RESIDENT_FAREWELL_TEXT_ID)?;
    if start_write.page_id == 0 || start_write.page_id != farewell_write.page_id {
        return Err(error("QUEST resident 시작/작별 page 결속 불일치"));
    }
    let page_glyphs = package
        .page_catalog_indices(start_write.page_id)
        .map_err(|source| error(source.to_string()))?
        .len();
    let scratch_runtime = QUEST_PAGE_CARRIER_START_RUNTIME
        .checked_add(
            u32::try_from(package.layout.scratch_offset)
                .map_err(|_| error("QUEST resident scratch offset 범위 이탈"))?,
        )
        .ok_or_else(|| error("QUEST resident scratch runtime overflow"))?;
    let start_path = verify_start_menu_event_path(
        hook,
        start_write.record_runtime,
        start_write.page_id,
        page_glyphs,
        scratch_runtime,
        evidence.start_menu_events,
    )?;
    let farewell_path = verify_farewell_event_path(
        hook,
        farewell_write.record_runtime,
        farewell_write.page_id,
        evidence.farewell_events,
    )?;
    let start_menu = verify_screen(
        package,
        hook,
        ScreenInputs {
            screen_id: "start-menu",
            write: start_write,
            ram: evidence.start_menu_ram,
            gpu: evidence.start_menu_gpu,
            events: evidence.start_menu_events,
            screenshot: evidence.start_menu_screenshot,
            window_role: QuestResidentTextWindowRole::Selection,
            event_path: start_path,
        },
    )?;
    let farewell = verify_screen(
        package,
        hook,
        ScreenInputs {
            screen_id: "farewell",
            write: farewell_write,
            ram: evidence.farewell_ram,
            gpu: evidence.farewell_gpu,
            events: evidence.farewell_events,
            screenshot: evidence.farewell_screenshot,
            window_role: QuestResidentTextWindowRole::InputIndicator,
            event_path: farewell_path,
        },
    )?;
    Ok(QuestResidentTextRuntimeReport {
        schema_version: QUEST_RESIDENT_TEXT_RUNTIME_SCHEMA_VERSION,
        image_sha256: build.report.disc.image_sha256.clone(),
        package_runtime: QUEST_PAGE_CARRIER_START_RUNTIME,
        package_bytes: package.bytes.len(),
        package_sha256: package.sha256.clone(),
        site_runtime: hook.site.base_addr(),
        site_bytes: hook.site.bytes().len(),
        body_runtime: hook.body_runtime,
        body_bytes: hook.body.bytes().len(),
        screens: vec![start_menu, farewell],
        natural_route_verified: false,
        translation_approved: false,
        layout_approved: false,
        display_approved: false,
        runtime_verified: true,
        release_eligible: false,
    })
}

/// 두 화면을 한 행씩 기록하는 runtime TSV를 만든다.
pub fn render_quest_resident_text_runtime_tsv(report: &QuestResidentTextRuntimeReport) -> String {
    let mut output = String::from(
        "schema_version\tscreen_id\timage_sha256\tpackage_runtime\tpackage_bytes\tpackage_sha256\t\
         site_runtime\tsite_bytes\tbody_runtime\tbody_bytes\tunit_id\trecord_runtime\tpage_id\t\
         page_glyphs\tram_sha256\tgpu_sha256\tevents_sha256\tscreenshot_sha256\t\
         screenshot_width\tscreenshot_height\tevent_count\tdropped_events\tload_image_calls\t\
         draw_sync_calls\tprimary_commit_events\tprimary_resident_events\t\
         expected_cells_sha256\tuploaded_cells_sha256\twindow_role\twindow_slot_index\t\
         window_state_runtime\twindow_parent_runtime\tcell_width\tcell_height\tline_height\t\
         parent_x_px\tparent_y_px\tparent_width_px\tparent_height_px\tframe_origin_x_px\t\
         frame_origin_y_px\tpackage_exact\tsite_exact\tbody_exact\tpage_state_exact\t\
         uploaded_cells_exact\tevent_path_exact\tscreenshot_shape_exact\truntime_verified\t\
         natural_route_verified\ttranslation_approved\tlayout_approved\tdisplay_approved\t\
         release_eligible\n",
    );
    for screen in &report.screens {
        let geometry = screen.window.geometry;
        writeln!(
            output,
            "{}\t{}\t{}\t0x{:08X}\t{}\t{}\t0x{:08X}\t{}\t0x{:08X}\t{}\t{}\t\
             0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
             {}\t{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
             {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            report.schema_version,
            screen.screen_id,
            report.image_sha256,
            report.package_runtime,
            report.package_bytes,
            report.package_sha256,
            report.site_runtime,
            report.site_bytes,
            report.body_runtime,
            report.body_bytes,
            screen.unit_id,
            screen.record_runtime,
            screen.page_id,
            screen.page_glyphs,
            screen.ram_sha256,
            screen.gpu_sha256,
            screen.events_sha256,
            screen.screenshot_sha256,
            screen.screenshot_width,
            screen.screenshot_height,
            screen.event_count,
            screen.dropped_events,
            screen.load_image_calls,
            screen.draw_sync_calls,
            screen.primary_commit_events,
            screen.primary_resident_events,
            screen.expected_cells_sha256,
            screen.uploaded_cells_sha256,
            screen.window.role.label(),
            screen.window.slot_index,
            screen.window.state_runtime,
            screen.window.parent_runtime,
            screen.window.cell_width,
            screen.window.cell_height,
            screen.window.line_height,
            geometry.parent_x_px,
            geometry.parent_y_px,
            geometry.parent_width_px,
            geometry.parent_height_px,
            geometry.frame_origin_x_px,
            geometry.frame_origin_y_px,
            screen.package_exact,
            screen.site_exact,
            screen.body_exact,
            screen.page_state_exact,
            screen.uploaded_cells_exact,
            screen.event_path_exact,
            screen.screenshot_shape_exact,
            screen.runtime_verified,
            report.natural_route_verified,
            report.translation_approved,
            report.layout_approved,
            report.display_approved,
            report.release_eligible,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_resident_text_runtime_tests.rs"]
mod tests;
