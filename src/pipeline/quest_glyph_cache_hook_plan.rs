//! QUEST OP21 route와 sparse glyph cache를 결속하는 typed hook 계획.
//!
//! carrier 앞에는 정렬된 cache package를 두고 그 직후에 실행 코드를 둔다.
//! 하위 machine-code module은 OP21의 page/target metadata를 해석하고,
//! page가 바뀌면 bitmap과 13-bit row index로 저장한 cell을
//! 96바이트 scratch에 해제해 정렬된 VRAM slot에 올린다.
//! 각 cell은 `LoadImage` 직후 `DrawSync(0)`으로 소비를 끝낸 뒤 같은 scratch를
//! 다시 쓴다. 이 단계는 writer나 release build가 아니며, active window 충돌
//! 정책과 carrier 실행 수명은 runtime으로 닫기 전까지 명시적으로 미검증이다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::disasm::{
    verify_box_quest_font_hook_site, verify_box_quest_font_object_lifetime,
    verify_box_quest_hook_space_candidates, verify_box_quest_item_acquisition,
    verify_box_quest_item_description_selection, verify_box_quest_item_name_callback,
    verify_box_quest_item_text_consumer, verify_box_quest_page_carrier, verify_box_quest_text_mode,
    verify_box_quest_ui_literals, QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME,
    QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME, QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME,
    QUEST_PAGE_CARRIER_CAPACITY_BYTES, QUEST_PAGE_CARRIER_START_RUNTIME,
};
use crate::isa::{VerifiedMachineCode, VerifyError};

use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_glyph_cache_package::QUEST_GLYPH_CACHE_VRAM_X;
use super::quest_glyph_cache_plan::{
    QUEST_GLYPH_CACHE_SCRATCH_ALIGNMENT, QUEST_GLYPH_CACHE_SCRATCH_BYTES,
};
use super::quest_hook_poc::verify_quest_hook_noop_sources;
use super::quest_item_acquisition_glyph_cache_hook::assemble_item_acquisition_whole_string_sites;
use super::quest_item_description_residency_hook::assemble_item_description_residency_sites;
use super::quest_item_glyph_cache_dispatch::QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME;
use super::quest_item_name_glyph_cache_hook::{
    assemble_item_name_direct_glyph_sites, assemble_item_name_residency_site,
    assemble_item_name_whole_string_sites, assemble_item_text_mode_residency_site,
};
use super::quest_page_hook_plan::QUEST_PAGE_HOOK_STATE_RUNTIME;
use super::quest_translation_routing::QuestTranslationRoutingAnalysis;
use super::quest_ui_glyph_cache_hook::{
    assemble_quest_ui_choice_direct_glyph_sites, assemble_world_map_label_whole_string_site,
};

mod explicit_texture_page;
mod installation;
mod machine_code;
mod space_guard;

pub(super) use installation::verify_quest_glyph_cache_installation;

use machine_code::{
    assemble_body_for_package, assemble_body_for_package_with_item_text_pages,
    assemble_original_font_guard_site_for_runtime, assemble_redirect_site_for_runtime,
    assemble_site_for_runtime,
};

pub const QUEST_GLYPH_CACHE_HOOK_SCHEMA_VERSION: &str = "quest-glyph-cache-hook-plan-v3";
pub const QUEST_GLYPH_CACHE_HOOK_SITE_SOURCE_ID: &str =
    "pipeline::quest_glyph_cache_hook_plan::op21_site_jump";
pub const QUEST_GLYPH_CACHE_HOOK_BODY_SOURCE_ID: &str =
    "pipeline::quest_glyph_cache_hook_plan::route_and_sparse_upload_body";
pub const QUEST_ORIGINAL_FONT_GUARD_SITE_SOURCE_ID: &str =
    "pipeline::quest_glyph_cache_hook_plan::original_font_guard_site";
pub const QUEST_FONT_OBJECT_MARKER_SITE_SOURCE_ID: &str =
    "pipeline::quest_glyph_cache_hook_plan::font_object_marker_site";
pub const QUEST_FONT_DRAW_DESCRIPTOR_SITE_SOURCE_ID: &str =
    "pipeline::quest_glyph_cache_hook_plan::font_draw_descriptor_site";
pub const QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_SOURCE_ID: &str =
    "pipeline::quest_glyph_cache_hook_plan::font_draw_texture_page_site";

pub const QUEST_GLYPH_CACHE_UPLOAD_WORD_WIDTH: u16 = 3;
pub const QUEST_GLYPH_CACHE_UPLOAD_ROWS: u16 = 16;

pub const QUEST_GLYPH_CACHE_ROUTE_PAGE_RUNTIME: u32 = QUEST_PAGE_HOOK_STATE_RUNTIME + 1;
pub const QUEST_GLYPH_CACHE_RENDERER_PAGE_RUNTIME: u32 = QUEST_PAGE_HOOK_STATE_RUNTIME + 2;
pub(super) const QUEST_GLYPH_CACHE_FRAME_BYTES: i16 = 72;
pub(super) const QUEST_GLYPH_CACHE_ASSIGNMENT_MODE_STACK_OFFSET: i16 = 64;
pub const QUEST_GLYPH_CACHE_KOREAN_TEXTURE_PAGE: u16 = 0x003E;
pub const QUEST_GLYPH_CACHE_SECONDARY_VRAM_X: usize = QUEST_GLYPH_CACHE_VRAM_X + 64;
pub const QUEST_GLYPH_CACHE_SECONDARY_TEXTURE_PAGE: u16 = 0x003F;

/// package와 typed hook의 정적 수용량 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheHookPlan {
    pub schema_version: &'static str,
    pub site: VerifiedMachineCode,
    pub item_name_residency_site: Option<VerifiedMachineCode>,
    pub item_name_direct_glyph_sites: Vec<VerifiedMachineCode>,
    pub item_name_whole_string_sites: Vec<VerifiedMachineCode>,
    pub item_acquisition_whole_string_sites: Vec<VerifiedMachineCode>,
    pub menu_literal_sites: Vec<VerifiedMachineCode>,
    pub ui_literal_choice_direct_glyph_sites: Vec<VerifiedMachineCode>,
    pub world_map_label_whole_string_site: Option<VerifiedMachineCode>,
    pub item_text_mode_residency_site: Option<VerifiedMachineCode>,
    pub item_description_residency_sites: Vec<VerifiedMachineCode>,
    pub original_font_guard_site: VerifiedMachineCode,
    pub font_space_guard_site: VerifiedMachineCode,
    pub font_object_marker_site: VerifiedMachineCode,
    pub font_draw_descriptor_site: VerifiedMachineCode,
    pub font_draw_texture_page_site: VerifiedMachineCode,
    pub font_explicit_texture_page_site: VerifiedMachineCode,
    pub body: VerifiedMachineCode,
    pub package_bytes: usize,
    pub body_runtime: u32,
    pub body_bytes: usize,
    pub original_font_guard_entry_runtime: u32,
    pub item_name_entry_runtime: Option<u32>,
    pub item_name_direct_glyph_entry_runtime: Option<u32>,
    pub item_name_whole_string_entry_runtime: Option<u32>,
    pub item_text_mode_entry_runtime: Option<u32>,
    pub item_description_constructor_entry_runtime: Option<u32>,
    pub item_name_whole_string_active_runtime: Option<u32>,
    pub item_name_page_id: Option<u8>,
    pub item_text_page_ids: Vec<u8>,
    pub font_object_marker_entry_runtime: u32,
    pub font_draw_descriptor_entry_runtime: u32,
    pub font_draw_texture_page_entry_runtime: u32,
    pub korean_descriptor_runtime: u32,
    pub carrier_used_bytes: usize,
    pub carrier_remaining_bytes: usize,
    pub max_page_uploads: usize,
    pub relocated_routes: usize,
    pub redirects_relocated_streams: bool,
    pub validates_page_ids: bool,
    pub scans_marked_font_objects: bool,
    pub decodes_independent_cells: bool,
    pub keeps_original_atlas_unchanged: bool,
    pub keeps_non_op21_objects_on_original_atlas: bool,
    pub keeps_unselected_non_op21_objects_on_original_atlas: bool,
    pub marks_op21_objects_with_route_page: bool,
    pub marks_item_name_objects_with_page: bool,
    pub retries_item_name_before_state_advance: bool,
    pub routes_item_text_pages_only_to_primary_bank: bool,
    pub reuses_active_item_text_page_for_names: bool,
    pub scopes_item_name_whole_string_renderer_route: bool,
    pub scopes_item_acquisition_fixed_message_renderer_route: bool,
    pub routes_ui_literal_choices_through_item_page: bool,
    pub routes_world_map_labels_through_item_page: bool,
    pub routes_item_description_direct_text_mode: bool,
    pub guards_item_description_constructor_with_residency: bool,
    pub selects_atlas_per_object: bool,
    pub routes_marked_objects_through_per_object_texture_page: bool,
    pub waits_for_draw_sync_before_scratch_reuse: bool,
    pub waits_for_draw_sync_before_commit: bool,
    pub selects_unpinned_atlas_bank: bool,
    pub yields_only_when_all_atlas_banks_pinned: bool,
    pub runtime_decode_failure_fail_closed: bool,
    pub r3000a_load_delays_safe: bool,
    pub package_materialized: bool,
    pub batched_upload_runtime_verified: bool,
    pub atlas_bank_policy_runtime_verified: bool,
    pub runtime_verified: bool,
    pub build_eligible: bool,
}

#[derive(Debug, Clone, Copy)]
struct QuestItemTextPageMode {
    default_name_page_id: u8,
    first_page_id: u8,
    last_page_id: u8,
    shared_slots: u16,
}

#[derive(Debug)]
pub enum QuestGlyphCacheHookPlanError {
    Message(String),
    MachineCode(VerifyError),
}

impl fmt::Display for QuestGlyphCacheHookPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => f.write_str(message),
            Self::MachineCode(source) => {
                write!(f, "QUEST sparse glyph cache hook ISA 검산 실패: {source}")
            }
        }
    }
}

impl Error for QuestGlyphCacheHookPlanError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MachineCode(source) => Some(source),
            Self::Message(_) => None,
        }
    }
}

impl From<VerifyError> for QuestGlyphCacheHookPlanError {
    fn from(value: VerifyError) -> Self {
        Self::MachineCode(value)
    }
}

fn error(message: impl Into<String>) -> QuestGlyphCacheHookPlanError {
    QuestGlyphCacheHookPlanError::Message(message.into())
}

fn item_text_page_mode(
    item_text_page_ids: &[u8],
) -> Result<Option<QuestItemTextPageMode>, QuestGlyphCacheHookPlanError> {
    if item_text_page_ids.is_empty() {
        return Ok(None);
    }
    let first_page_id = item_text_page_ids[0];
    let last_page_id = *item_text_page_ids
        .last()
        .ok_or_else(|| error("QUEST item text page ID가 비었음"))?;
    if first_page_id == 0
        || !item_text_page_ids
            .iter()
            .copied()
            .eq(first_page_id..=last_page_id)
    {
        return Err(error("QUEST item text page ID가 연속 범위가 아님"));
    }
    Ok(Some(QuestItemTextPageMode {
        default_name_page_id: first_page_id,
        first_page_id,
        last_page_id,
        shared_slots: 0,
    }))
}

pub(super) fn verify_quest_glyph_cache_hook_plan_for_package(
    map: &ExeMap,
    text: &[u8],
    package: &QuestGlyphCachePackage,
    route_count: usize,
    relocated_routes: usize,
    schema_version: &'static str,
) -> Result<QuestGlyphCacheHookPlan, QuestGlyphCacheHookPlanError> {
    verify_quest_glyph_cache_hook_plan_for_package_with_item_name_page(
        map,
        text,
        package,
        route_count,
        relocated_routes,
        schema_version,
        None,
    )
}

pub(super) fn verify_quest_glyph_cache_hook_plan_for_package_with_item_name_page(
    map: &ExeMap,
    text: &[u8],
    package: &QuestGlyphCachePackage,
    route_count: usize,
    relocated_routes: usize,
    schema_version: &'static str,
    item_name_page_id: Option<u8>,
) -> Result<QuestGlyphCacheHookPlan, QuestGlyphCacheHookPlanError> {
    let item_text_page_ids = item_name_page_id.into_iter().collect::<Vec<_>>();
    verify_quest_glyph_cache_hook_plan_for_package_with_item_text_pages(
        map,
        text,
        package,
        route_count,
        relocated_routes,
        schema_version,
        &item_text_page_ids,
    )
}

pub(super) fn verify_quest_glyph_cache_hook_plan_for_package_with_item_text_pages(
    map: &ExeMap,
    text: &[u8],
    package: &QuestGlyphCachePackage,
    route_count: usize,
    relocated_routes: usize,
    schema_version: &'static str,
    item_text_page_ids: &[u8],
) -> Result<QuestGlyphCacheHookPlan, QuestGlyphCacheHookPlanError> {
    let item_text_pages = item_text_page_mode(item_text_page_ids)?;
    let item_name_page_id = item_text_pages.map(|pages| pages.default_name_page_id);
    let cache = &package.plan;
    verify_box_quest_font_hook_site(map, text).map_err(|source| error(source.to_string()))?;
    verify_box_quest_font_object_lifetime(map, text).map_err(|source| error(source.to_string()))?;
    let candidates = verify_box_quest_hook_space_candidates(map, text)
        .map_err(|source| error(source.to_string()))?;
    verify_quest_hook_noop_sources(map, text, &candidates)
        .map_err(|source| error(source.to_string()))?;
    let carrier =
        verify_box_quest_page_carrier(map, text).map_err(|source| error(source.to_string()))?;
    if carrier.capacity_bytes != QUEST_PAGE_CARRIER_CAPACITY_BYTES
        || !cache.package_fits_carrier
        || cache.pages == 0
        || cache.glyphs == 0
        || cache.max_page_glyphs == 0
        || package.layout.package_bytes != cache.package_bytes
        || package.bytes.len() != cache.package_bytes
        || package.layout.scratch_bytes != QUEST_GLYPH_CACHE_SCRATCH_BYTES
        || !package
            .layout
            .scratch_offset
            .is_multiple_of(QUEST_GLYPH_CACHE_SCRATCH_ALIGNMENT)
        || !package.sections_exact
        || !package.catalog_readback_exact
        || !package.original_atlas_source_roundtrip_exact
        || !package.page_assignment_readback_exact
        || !package.slot_rect_readback_exact
        || !package.korean_descriptor_readback_exact
        || !package.scratch_zero
    {
        return Err(error("QUEST sparse glyph cache hook package 분모 drift"));
    }
    if route_count == 0 || relocated_routes > route_count {
        return Err(error("QUEST sparse glyph cache hook route 분모 drift"));
    }
    if item_text_pages.is_some_and(|pages| usize::from(pages.last_page_id) > cache.pages) {
        return Err(error(
            "QUEST item text page ID가 package의 유효 page 범위 밖임",
        ));
    }
    if item_name_page_id.is_some() {
        verify_box_quest_item_name_callback(map, text)
            .map_err(|source| error(source.to_string()))?;
        let item_text = verify_box_quest_item_text_consumer(map, text)
            .map_err(|source| error(source.to_string()))?;
        verify_box_quest_item_acquisition(map, text, &item_text)
            .map_err(|source| error(source.to_string()))?;
        verify_box_quest_text_mode(map, text).map_err(|source| error(source.to_string()))?;
        verify_box_quest_ui_literals(map, text).map_err(|source| error(source.to_string()))?;
        let active_offset = map
            .text_offset(QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME)
            .ok_or_else(|| error("QUEST item name whole-string active byte 사상 실패"))?;
        if text.get(active_offset).copied() != Some(0) {
            return Err(error(
                "QUEST item name whole-string active byte 원본이 0이 아님",
            ));
        }
    }
    let item_description_selection_paths =
        if item_text_pages.is_some_and(|pages| pages.first_page_id < pages.last_page_id) {
            let summary = verify_box_quest_item_description_selection(map, text)
                .map_err(|source| error(source.to_string()))?;
            let paths = summary
                .selection_paths
                .try_into()
                .map_err(|_| error("QUEST item description residency path 분모가 3이 아님"))?;
            Some(paths)
        } else {
            None
        };
    let body_offset = package.bytes.len();
    let body_runtime = QUEST_PAGE_CARRIER_START_RUNTIME
        .checked_add(
            u32::try_from(body_offset)
                .map_err(|_| error("QUEST sparse glyph cache hook body offset이 u32를 넘음"))?,
        )
        .ok_or_else(|| error("QUEST sparse glyph cache hook body runtime overflow"))?;
    if !body_runtime.is_multiple_of(4) {
        return Err(error("QUEST sparse glyph cache hook package 정렬 drift"));
    }
    let site = assemble_site_for_runtime(body_runtime)?;
    let assembled_body = if item_text_pages.is_none() {
        assemble_body_for_package(package, body_runtime)?
    } else {
        assemble_body_for_package_with_item_text_pages(package, body_runtime, item_text_pages)?
    };
    let item_name_residency_site = assembled_body
        .item_name_entry_runtime
        .map(assemble_item_name_residency_site)
        .transpose()?;
    let item_name_direct_glyph_sites = assembled_body
        .item_name_direct_glyph_entry_runtime
        .map(assemble_item_name_direct_glyph_sites)
        .transpose()?
        .unwrap_or_default();
    let item_name_whole_string_sites = assembled_body
        .item_name_whole_string_entry_runtime
        .map(assemble_item_name_whole_string_sites)
        .transpose()?
        .unwrap_or_default();
    let item_acquisition_whole_string_sites = assembled_body
        .item_name_whole_string_entry_runtime
        .map(assemble_item_acquisition_whole_string_sites)
        .transpose()?
        .unwrap_or_default();
    let ui_literal_choice_direct_glyph_sites = assembled_body
        .item_name_direct_glyph_entry_runtime
        .map(assemble_quest_ui_choice_direct_glyph_sites)
        .transpose()?
        .unwrap_or_default();
    let menu_literal_sites = assembled_body
        .item_name_direct_glyph_entry_runtime
        .zip(assembled_body.item_name_whole_string_entry_runtime)
        .map(|(direct, whole)| {
            super::quest_ui_glyph_cache_hook::assemble_menu_literal_sites(direct, whole)
        })
        .transpose()?
        .unwrap_or_default();
    let world_map_label_whole_string_site = assembled_body
        .item_name_whole_string_entry_runtime
        .map(assemble_world_map_label_whole_string_site)
        .transpose()?;
    let item_text_mode_residency_site = assembled_body
        .item_text_mode_entry_runtime
        .map(assemble_item_text_mode_residency_site)
        .transpose()?;
    if item_description_selection_paths.is_some()
        != assembled_body
            .item_description_constructor_entry_runtime
            .is_some()
    {
        return Err(error(
            "QUEST item description residency site와 entry 결속 불일치",
        ));
    }
    let item_description_residency_sites = item_description_selection_paths
        .as_ref()
        .zip(assembled_body.item_description_constructor_entry_runtime)
        .map(|(paths, entry_runtime)| {
            assemble_item_description_residency_sites(paths, entry_runtime)
                .map_err(QuestGlyphCacheHookPlanError::from)
        })
        .transpose()?
        .unwrap_or_default();
    let original_font_guard_site = assemble_original_font_guard_site_for_runtime(
        assembled_body.original_font_guard_entry_runtime,
    )?;
    let font_space_guard_site =
        space_guard::assemble(map, text, assembled_body.font_space_guard_entry_runtime)?;
    let font_object_marker_site = assemble_redirect_site_for_runtime(
        QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME,
        assembled_body.font_object_marker_entry_runtime,
        QUEST_FONT_OBJECT_MARKER_SITE_SOURCE_ID,
    )?;
    let font_draw_descriptor_site = assemble_redirect_site_for_runtime(
        QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME,
        assembled_body.font_draw_descriptor_entry_runtime,
        QUEST_FONT_DRAW_DESCRIPTOR_SITE_SOURCE_ID,
    )?;
    let font_draw_texture_page_site = assemble_redirect_site_for_runtime(
        QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME,
        assembled_body.font_draw_texture_page_entry_runtime,
        QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_SOURCE_ID,
    )?;
    let font_explicit_texture_page_site = explicit_texture_page::assemble(
        map,
        text,
        assembled_body.font_sprite_texture_page_entry_runtime,
    )?;
    let body = assembled_body.code;
    let body_bytes = body.bytes().len();
    let carrier_used_bytes = body_offset
        .checked_add(body_bytes)
        .ok_or_else(|| error("QUEST sparse glyph cache carrier 사용량 overflow"))?;
    if carrier_used_bytes > carrier.capacity_bytes {
        return Err(error(format!(
            "QUEST sparse glyph cache code가 carrier를 초과함: used={carrier_used_bytes}, data={body_offset}, code={body_bytes}, capacity={}, glyphs={}, dictionary_rows={}, dictionary_bytes={}, cell_indices={}, page_assignments={}",
            carrier.capacity_bytes,
            cache.glyphs,
            cache.catalog.dictionary_rows,
            cache.catalog.dictionary_bytes,
            cache.catalog.cell_index_bytes,
            cache.page_assignment_bytes
        )));
    }

    Ok(QuestGlyphCacheHookPlan {
        schema_version,
        site,
        item_name_residency_site,
        item_name_direct_glyph_sites,
        item_name_whole_string_sites,
        item_acquisition_whole_string_sites,
        ui_literal_choice_direct_glyph_sites,
        menu_literal_sites,
        world_map_label_whole_string_site,
        item_text_mode_residency_site,
        item_description_residency_sites,
        original_font_guard_site,
        font_space_guard_site,
        font_object_marker_site,
        font_draw_descriptor_site,
        font_draw_texture_page_site,
        font_explicit_texture_page_site,
        body,
        package_bytes: cache.package_bytes,
        body_runtime,
        body_bytes,
        original_font_guard_entry_runtime: assembled_body.original_font_guard_entry_runtime,
        item_name_entry_runtime: assembled_body.item_name_entry_runtime,
        item_name_direct_glyph_entry_runtime: assembled_body.item_name_direct_glyph_entry_runtime,
        item_name_whole_string_entry_runtime: assembled_body.item_name_whole_string_entry_runtime,
        item_text_mode_entry_runtime: assembled_body.item_text_mode_entry_runtime,
        item_description_constructor_entry_runtime: assembled_body
            .item_description_constructor_entry_runtime,
        item_name_whole_string_active_runtime: item_name_page_id
            .map(|_| QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME),
        item_name_page_id,
        item_text_page_ids: item_text_page_ids.to_vec(),
        font_object_marker_entry_runtime: assembled_body.font_object_marker_entry_runtime,
        font_draw_descriptor_entry_runtime: assembled_body.font_draw_descriptor_entry_runtime,
        font_draw_texture_page_entry_runtime: assembled_body.font_draw_texture_page_entry_runtime,
        korean_descriptor_runtime: assembled_body.korean_descriptor_runtime,
        carrier_used_bytes,
        carrier_remaining_bytes: carrier.capacity_bytes - carrier_used_bytes,
        max_page_uploads: cache.max_page_glyphs,
        relocated_routes,
        redirects_relocated_streams: true,
        validates_page_ids: true,
        scans_marked_font_objects: true,
        decodes_independent_cells: true,
        keeps_original_atlas_unchanged: true,
        keeps_non_op21_objects_on_original_atlas: item_name_page_id.is_none(),
        keeps_unselected_non_op21_objects_on_original_atlas: true,
        marks_op21_objects_with_route_page: true,
        marks_item_name_objects_with_page: item_name_page_id.is_some(),
        retries_item_name_before_state_advance: item_name_page_id.is_some(),
        routes_item_text_pages_only_to_primary_bank: item_name_page_id.is_some(),
        reuses_active_item_text_page_for_names: item_name_page_id.is_some(),
        scopes_item_name_whole_string_renderer_route: item_name_page_id.is_some(),
        scopes_item_acquisition_fixed_message_renderer_route: item_name_page_id.is_some(),
        routes_ui_literal_choices_through_item_page: item_name_page_id.is_some(),
        routes_world_map_labels_through_item_page: item_name_page_id.is_some(),
        routes_item_description_direct_text_mode: item_name_page_id.is_some(),
        guards_item_description_constructor_with_residency: item_description_selection_paths
            .is_some(),
        selects_atlas_per_object: true,
        routes_marked_objects_through_per_object_texture_page: true,
        waits_for_draw_sync_before_scratch_reuse: true,
        waits_for_draw_sync_before_commit: true,
        selects_unpinned_atlas_bank: true,
        yields_only_when_all_atlas_banks_pinned: true,
        runtime_decode_failure_fail_closed: true,
        r3000a_load_delays_safe: true,
        package_materialized: true,
        batched_upload_runtime_verified: false,
        atlas_bank_policy_runtime_verified: false,
        runtime_verified: false,
        build_eligible: false,
    })
}

/// 현재 provisional route/cache에서 파생한 layout으로 typed hook을 검증한다.
pub fn verify_box_quest_glyph_cache_hook_plan(
    map: &ExeMap,
    text: &[u8],
    package: &QuestGlyphCachePackage,
    routing: &QuestTranslationRoutingAnalysis,
) -> Result<QuestGlyphCacheHookPlan, QuestGlyphCacheHookPlanError> {
    let cache = &package.plan;
    if cache.pages != routing.pages
        || routing.routes.is_empty()
        || routing
            .routes
            .iter()
            .any(|route| usize::from(route.page_id) > cache.pages || route.page_id == 0)
    {
        return Err(error(
            "QUEST sparse glyph cache hook route/cache 분모 drift",
        ));
    }
    let plan = verify_quest_glyph_cache_hook_plan_for_package(
        map,
        text,
        package,
        routing.routes.len(),
        routing.relocated_candidates,
        QUEST_GLYPH_CACHE_HOOK_SCHEMA_VERSION,
    )?;
    let expected_body_runtime = QUEST_PAGE_CARRIER_START_RUNTIME
        .checked_add(
            u32::try_from(package.bytes.len())
                .map_err(|_| error("QUEST sparse glyph hook package 길이가 u32를 넘음"))?,
        )
        .ok_or_else(|| error("QUEST sparse glyph hook body runtime overflow"))?;
    if plan.package_bytes != package.bytes.len()
        || plan.body_runtime != expected_body_runtime
        || plan.max_page_uploads != cache.max_page_glyphs
        || plan.relocated_routes != routing.relocated_candidates
    {
        return Err(error("QUEST sparse glyph cache hook 파생 주소 drift"));
    }
    Ok(plan)
}
