//! 선택된 QUEST 번역문 package와 route를 결속한 typed hook 계획.
//!
//! package의 파생 layout과 실제 분모를 assembler 입력으로 넘기고 선언한
//! R3000A profile로 연속 해석·재조립한다. 제품 writer와 runtime 승격은
//! 별도이며, package+hook 소비 관계가 확정된 이 결과의 digest는 보고서에서
//! 관측한다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::disasm::{
    QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES, QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES,
    QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES, QUEST_SEQ_TEXT_CONSUMER_CALL_RUNTIME,
};

use super::quest_glyph_cache_hook_plan::{
    verify_quest_glyph_cache_hook_plan_for_package_with_item_text_pages, QuestGlyphCacheHookPlan,
};
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_translation_selection_routing::QuestTranslationSelectionRoutingAnalysis;

pub const QUEST_TRANSLATION_GLYPH_CACHE_HOOK_SCHEMA_VERSION: &str =
    "quest-translation-glyph-cache-hook-plan-v2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationGlyphCacheHookPlanError(String);

impl fmt::Display for QuestTranslationGlyphCacheHookPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestTranslationGlyphCacheHookPlanError {}

fn error(message: impl Into<String>) -> QuestTranslationGlyphCacheHookPlanError {
    QuestTranslationGlyphCacheHookPlanError(message.into())
}

/// 선택 package의 실제 layout과 route 분모로 typed hook을 조립한다.
pub fn verify_box_quest_translation_glyph_cache_hook_plan(
    map: &ExeMap,
    text: &[u8],
    package: &QuestGlyphCachePackage,
    routing: &QuestTranslationSelectionRoutingAnalysis,
) -> Result<QuestGlyphCacheHookPlan, QuestTranslationGlyphCacheHookPlanError> {
    verify_box_quest_translation_glyph_cache_hook_plan_with_item_text_pages(
        map,
        text,
        package,
        routing,
        &[],
    )
}

/// package에 포함된 item 이름 page를 callback retry hook과 함께 검증한다.
pub fn verify_box_quest_translation_glyph_cache_hook_plan_with_item_name_page(
    map: &ExeMap,
    text: &[u8],
    package: &QuestGlyphCachePackage,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    item_name_page_id: u8,
) -> Result<QuestGlyphCacheHookPlan, QuestTranslationGlyphCacheHookPlanError> {
    verify_box_quest_translation_glyph_cache_hook_plan_with_item_text_pages(
        map,
        text,
        package,
        routing,
        &[item_name_page_id],
    )
}

/// item 이름과 설명이 공유하는 연속 page 범위를 primary-only hook에 결속한다.
pub fn verify_box_quest_translation_glyph_cache_hook_plan_with_item_text_pages(
    map: &ExeMap,
    text: &[u8],
    package: &QuestGlyphCachePackage,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    item_text_page_ids: &[u8],
) -> Result<QuestGlyphCacheHookPlan, QuestTranslationGlyphCacheHookPlanError> {
    let item_name_page_id = item_text_page_ids.first().copied();
    let requires_item_description_residency = item_text_page_ids.len() > 1;
    let plan = verify_quest_glyph_cache_hook_plan_for_package_with_item_text_pages(
        map,
        text,
        package,
        routing.routes.len(),
        routing.relocated,
        QUEST_TRANSLATION_GLYPH_CACHE_HOOK_SCHEMA_VERSION,
        item_text_page_ids,
    )
    .map_err(|source| error(source.to_string()))?;
    let expected_body_runtime = crate::disasm::QUEST_PAGE_CARRIER_START_RUNTIME
        .checked_add(
            u32::try_from(package.bytes.len())
                .map_err(|_| error("QUEST 번역 glyph hook package 길이가 u32를 넘음"))?,
        )
        .ok_or_else(|| error("QUEST 번역 glyph hook body runtime overflow"))?;
    if plan.schema_version != QUEST_TRANSLATION_GLYPH_CACHE_HOOK_SCHEMA_VERSION
        || plan.package_bytes != package.bytes.len()
        || plan.body_runtime != expected_body_runtime
        || plan.max_page_uploads != package.plan.max_page_glyphs
        || plan.relocated_routes != routing.relocated
        || !plan.redirects_relocated_streams
        || !plan.validates_page_ids
        || !plan.scans_marked_font_objects
        || !plan.decodes_independent_cells
        || !plan.keeps_original_atlas_unchanged
        || plan.keeps_non_op21_objects_on_original_atlas != item_name_page_id.is_none()
        || !plan.keeps_unselected_non_op21_objects_on_original_atlas
        || !plan.marks_op21_objects_with_route_page
        || plan.marks_item_name_objects_with_page != item_name_page_id.is_some()
        || plan.retries_item_name_before_state_advance != item_name_page_id.is_some()
        || plan.routes_item_text_pages_only_to_primary_bank != item_name_page_id.is_some()
        || plan.reuses_active_item_text_page_for_names != item_name_page_id.is_some()
        || plan.scopes_item_name_whole_string_renderer_route != item_name_page_id.is_some()
        || plan.scopes_item_acquisition_fixed_message_renderer_route != item_name_page_id.is_some()
        || plan.item_name_page_id != item_name_page_id
        || plan.item_text_page_ids != item_text_page_ids
        || plan.item_name_residency_site.is_some() != item_name_page_id.is_some()
        || plan.item_name_entry_runtime.is_some() != item_name_page_id.is_some()
        || plan.item_name_direct_glyph_entry_runtime.is_some() != item_name_page_id.is_some()
        || plan.item_name_whole_string_entry_runtime.is_some() != item_name_page_id.is_some()
        || plan.item_text_mode_entry_runtime.is_some() != item_name_page_id.is_some()
        || plan.item_text_mode_residency_site.is_some() != item_name_page_id.is_some()
        || plan.routes_item_description_direct_text_mode != item_name_page_id.is_some()
        || plan.guards_item_description_constructor_with_residency
            != requires_item_description_residency
        || plan.item_description_residency_sites.len()
            != usize::from(requires_item_description_residency) * 3
        || plan.item_description_constructor_entry_runtime.is_some()
            != requires_item_description_residency
        || plan.item_name_whole_string_active_runtime.is_some() != item_name_page_id.is_some()
        || if item_name_page_id.is_some() {
            !plan
                .item_name_direct_glyph_sites
                .iter()
                .map(|site| site.base_addr())
                .eq(QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES)
        } else {
            !plan.item_name_direct_glyph_sites.is_empty()
        }
        || if item_name_page_id.is_some() {
            !plan
                .item_name_whole_string_sites
                .iter()
                .map(|site| site.base_addr())
                .eq(QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES)
        } else {
            !plan.item_name_whole_string_sites.is_empty()
        }
        || if item_name_page_id.is_some() {
            !plan
                .item_acquisition_whole_string_sites
                .iter()
                .map(|site| site.base_addr())
                .eq(QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES)
        } else {
            !plan.item_acquisition_whole_string_sites.is_empty()
        }
        || plan
            .item_text_mode_residency_site
            .as_ref()
            .is_some_and(|site| site.base_addr() != QUEST_SEQ_TEXT_CONSUMER_CALL_RUNTIME)
        || !plan.selects_atlas_per_object
        || !plan.routes_marked_objects_through_per_object_texture_page
        || !plan.waits_for_draw_sync_before_scratch_reuse
        || !plan.waits_for_draw_sync_before_commit
        || !plan.selects_unpinned_atlas_bank
        || !plan.yields_only_when_all_atlas_banks_pinned
        || !plan.runtime_decode_failure_fail_closed
        || !plan.r3000a_load_delays_safe
        || !plan.package_materialized
        || plan.batched_upload_runtime_verified
        || plan.atlas_bank_policy_runtime_verified
        || plan.runtime_verified
        || plan.build_eligible
    {
        return Err(error("QUEST 번역 glyph cache hook 판정 drift"));
    }
    Ok(plan)
}

/// 선택 package와 typed site/body의 현재 소비 계약을 기록하는 TSV.
pub fn render_quest_translation_glyph_cache_hook_plan_tsv(
    package: &QuestGlyphCachePackage,
    plan: &QuestGlyphCacheHookPlan,
) -> String {
    let site_sha256 = format!("{:x}", Sha256::digest(plan.site.bytes()));
    let original_font_guard_site_sha256 = format!(
        "{:x}",
        Sha256::digest(plan.original_font_guard_site.bytes())
    );
    let item_name_page_id = plan
        .item_name_page_id
        .map(|page_id| page_id.to_string())
        .unwrap_or_default();
    let item_name_residency_site_runtime = plan
        .item_name_residency_site
        .as_ref()
        .map(|site| format!("0x{:08X}", site.base_addr()))
        .unwrap_or_default();
    let item_name_residency_site_sha256 = plan
        .item_name_residency_site
        .as_ref()
        .map(|site| format!("{:x}", Sha256::digest(site.bytes())))
        .unwrap_or_default();
    let item_name_entry_runtime = plan
        .item_name_entry_runtime
        .map(|runtime| format!("0x{runtime:08X}"))
        .unwrap_or_default();
    let item_name_direct_glyph_entry_runtime = plan
        .item_name_direct_glyph_entry_runtime
        .map(|runtime| format!("0x{runtime:08X}"))
        .unwrap_or_default();
    let item_name_direct_glyph_site_runtimes = plan
        .item_name_direct_glyph_sites
        .iter()
        .map(|site| format!("0x{:08X}", site.base_addr()))
        .collect::<Vec<_>>()
        .join(",");
    let item_name_whole_string_entry_runtime = plan
        .item_name_whole_string_entry_runtime
        .map(|runtime| format!("0x{runtime:08X}"))
        .unwrap_or_default();
    let item_name_whole_string_active_runtime = plan
        .item_name_whole_string_active_runtime
        .map(|runtime| format!("0x{runtime:08X}"))
        .unwrap_or_default();
    let item_name_whole_string_site_runtimes = plan
        .item_name_whole_string_sites
        .iter()
        .map(|site| format!("0x{:08X}", site.base_addr()))
        .collect::<Vec<_>>()
        .join(",");
    let item_acquisition_whole_string_site_runtimes = plan
        .item_acquisition_whole_string_sites
        .iter()
        .map(|site| format!("0x{:08X}", site.base_addr()))
        .collect::<Vec<_>>()
        .join(",");
    let item_text_mode_residency_site_runtime = plan
        .item_text_mode_residency_site
        .as_ref()
        .map(|site| format!("0x{:08X}", site.base_addr()))
        .unwrap_or_default();
    let item_text_mode_entry_runtime = plan
        .item_text_mode_entry_runtime
        .map(|runtime| format!("0x{runtime:08X}"))
        .unwrap_or_default();
    let body_sha256 = format!("{:x}", Sha256::digest(plan.body.bytes()));
    let mut output = String::from(
        "schema_version\tpackage_bytes\tpackage_sha256\tbody_runtime\tbody_bytes\t\
         site_sha256\toriginal_font_guard_site_sha256\t\
         original_font_guard_entry_runtime\titem_name_page_id\t\
         item_name_residency_site_runtime\titem_name_residency_site_sha256\t\
         item_name_entry_runtime\titem_name_direct_glyph_entry_runtime\t\
         item_name_direct_glyph_site_runtimes\titem_name_whole_string_entry_runtime\t\
         item_name_whole_string_active_runtime\titem_name_whole_string_site_runtimes\t\
         item_acquisition_whole_string_site_runtimes\t\
         item_text_mode_residency_site_runtime\titem_text_mode_entry_runtime\t\
         item_description_direct_text_mode\t\
         item_name_primary_only\titem_name_whole_string_scoped\t\
         body_sha256\tcarrier_used_bytes\t\
         carrier_remaining_bytes\t\
         max_page_uploads\trelocated_routes\ttyped_source_verified\tr3000a_load_delays_safe\t\
         runtime_verified\tbuild_eligible\n",
    );
    writeln!(
        output,
        "{}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        plan.schema_version,
        package.bytes.len(),
        package.sha256,
        plan.body_runtime,
        plan.body_bytes,
        site_sha256,
        original_font_guard_site_sha256,
        plan.original_font_guard_entry_runtime,
        item_name_page_id,
        item_name_residency_site_runtime,
        item_name_residency_site_sha256,
        item_name_entry_runtime,
        item_name_direct_glyph_entry_runtime,
        item_name_direct_glyph_site_runtimes,
        item_name_whole_string_entry_runtime,
        item_name_whole_string_active_runtime,
        item_name_whole_string_site_runtimes,
        item_acquisition_whole_string_site_runtimes,
        item_text_mode_residency_site_runtime,
        item_text_mode_entry_runtime,
        plan.routes_item_description_direct_text_mode,
        plan.routes_item_text_pages_only_to_primary_bank,
        plan.scopes_item_name_whole_string_renderer_route,
        body_sha256,
        plan.carrier_used_bytes,
        plan.carrier_remaining_bytes,
        plan.max_page_uploads,
        plan.relocated_routes,
        plan.site.typed_source_id().is_some()
            && plan.site.checked_profile_id().is_some()
            && plan.body.typed_source_id().is_some()
            && plan.body.checked_profile_id().is_some()
            && plan.item_name_residency_site.as_ref().is_none_or(|site| {
                site.typed_source_id().is_some() && site.checked_profile_id().is_some()
            })
            && plan.item_name_direct_glyph_sites.iter().all(|site| {
                site.typed_source_id().is_some() && site.checked_profile_id().is_some()
            })
            && plan.item_name_whole_string_sites.iter().all(|site| {
                site.typed_source_id().is_some() && site.checked_profile_id().is_some()
            })
            && plan.item_acquisition_whole_string_sites.iter().all(|site| {
                site.typed_source_id().is_some() && site.checked_profile_id().is_some()
            })
            && plan.item_text_mode_residency_site.as_ref().is_none_or(|site| {
                site.typed_source_id().is_some() && site.checked_profile_id().is_some()
            })
            && plan.item_description_residency_sites.iter().all(|site| {
                site.typed_source_id().is_some() && site.checked_profile_id().is_some()
            }),
        plan.r3000a_load_delays_safe,
        plan.runtime_verified,
        plan.build_eligible,
    )
    .expect("String write는 실패하지 않음");
    output
}

#[cfg(test)]
#[path = "quest_translation_glyph_cache_hook_plan_tests.rs"]
mod tests;
