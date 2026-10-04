//! QUEST glyph cache hook을 재배치 EXE에 설치하기 전의 기능 계약.
//!
//! hook 내부의 page·item text·font object route 관계와 설치할 typed site
//! 목록은 hook 계획이 소유한다. disc composer는 이 검산 결과를 받아 좌표와
//! Expected Write만 구성하며 각 site의 기능 의미를 다시 해석하지 않는다.

use crate::disasm::{
    QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME, QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME,
    QUEST_FONT_HOOK_SITE_RUNTIME, QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME,
    QUEST_GLYPH_RENDERER_RUNTIME, QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES,
    QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES, QUEST_ITEM_NAME_RESIDENCY_SITE_RUNTIME,
    QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES, QUEST_PAGE_CARRIER_START_RUNTIME,
    QUEST_SEQ_TEXT_CONSUMER_CALL_RUNTIME, QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES,
    QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME,
};
use crate::isa::VerifiedMachineCode;

use super::{error, QuestGlyphCacheHookPlan, QuestGlyphCacheHookPlanError};
use crate::pipeline::quest_glyph_cache_package::QuestGlyphCachePackage;
use crate::pipeline::quest_item_glyph_cache_dispatch::QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME;

const QUEST_ITEM_NAME_DIRECT_GLYPH_SITE_IDS: [&str;
    QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES.len()] = [
    "quest-item-name-direct-glyph-site-8003121c",
    "quest-item-name-direct-glyph-site-80032880",
    "quest-item-name-direct-glyph-site-800330d0",
    "quest-item-name-direct-glyph-site-80033264",
    "quest-item-name-direct-glyph-site-800333f4",
    "quest-item-name-direct-glyph-site-8003541c",
    "quest-item-name-direct-glyph-site-80035580",
    "quest-item-name-direct-glyph-site-800356e0",
    "quest-item-name-direct-glyph-site-80036d7c",
    "quest-item-name-direct-glyph-site-80036f10",
    "quest-item-name-direct-glyph-site-800370a0",
    "quest-item-name-direct-glyph-site-80037c58",
    "quest-item-name-direct-glyph-site-800389c8",
    "quest-item-name-direct-glyph-site-800392d4",
    "quest-item-name-direct-glyph-site-8003a774",
];
const QUEST_ITEM_NAME_WHOLE_STRING_SITE_IDS: [&str;
    QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES.len()] = [
    "quest-item-name-whole-string-site-80053388",
    "quest-item-name-whole-string-site-800614d0",
    "quest-item-name-whole-string-site-800618f4",
];
const QUEST_ITEM_ACQUISITION_WHOLE_STRING_SITE_IDS: [&str; 6] = [
    "quest-item-acquisition-whole-string-site-800614e4",
    "quest-item-acquisition-whole-string-site-800617b8",
    "quest-item-acquisition-whole-string-site-800617fc",
    "quest-item-acquisition-whole-string-site-800618e8",
    "quest-item-acquisition-whole-string-site-80061904",
    "quest-item-acquisition-whole-string-site-8006191c",
];
const QUEST_ITEM_DESCRIPTION_RESIDENCY_SITE_IDS: [&str; 3] = [
    "quest-item-description-residency-site-80038bf8",
    "quest-item-description-residency-site-80039504",
    "quest-item-description-residency-site-8003aa8c",
];
const QUEST_UI_CHOICE_DIRECT_GLYPH_SITE_IDS: [&str; 8] = [
    "quest-ui-choice-direct-glyph-site-80035f54",
    "quest-ui-choice-direct-glyph-site-80035fe8",
    "quest-ui-choice-direct-glyph-site-80037730",
    "quest-ui-choice-direct-glyph-site-800377e4",
    "quest-ui-choice-direct-glyph-site-80037da8",
    "quest-ui-choice-direct-glyph-site-80037e3c",
    "quest-ui-choice-direct-glyph-site-80039b04",
    "quest-ui-choice-direct-glyph-site-80039b98",
];

/// 재배치 EXE의 원본 위치에서 교체할 검산된 typed site.
#[derive(Clone, Copy)]
pub(in crate::pipeline) struct QuestGlyphCacheInstallationSite<'a> {
    pub(in crate::pipeline) code: &'a VerifiedMachineCode,
    pub(in crate::pipeline) id: &'static str,
    pub(in crate::pipeline) purpose: &'static str,
}

/// disc composer가 소비할 hook 설치 경계.
pub(in crate::pipeline) struct QuestGlyphCacheInstallation<'a> {
    pub(in crate::pipeline) body_end_runtime: u32,
    pub(in crate::pipeline) machine_sites: Vec<QuestGlyphCacheInstallationSite<'a>>,
}

/// package와 hook의 기능 관계를 검산하고 설치 site를 한 번만 분류한다.
pub(in crate::pipeline) fn verify_quest_glyph_cache_installation<'a>(
    package: &QuestGlyphCachePackage,
    hook: &'a QuestGlyphCacheHookPlan,
) -> Result<QuestGlyphCacheInstallation<'a>, QuestGlyphCacheHookPlanError> {
    let expected_body_runtime = QUEST_PAGE_CARRIER_START_RUNTIME
        .checked_add(
            u32::try_from(package.bytes.len())
                .map_err(|_| error("QUEST sparse cache package 길이가 u32를 넘음"))?,
        )
        .ok_or_else(|| error("QUEST sparse cache body runtime 파생 overflow"))?;
    let expected_korean_descriptor_runtime = QUEST_PAGE_CARRIER_START_RUNTIME
        .checked_add(
            u32::try_from(package.layout.korean_descriptor_offset)
                .map_err(|_| error("QUEST Korean descriptor offset이 u32를 넘음"))?,
        )
        .ok_or_else(|| error("QUEST Korean descriptor runtime 파생 overflow"))?;
    let body_end_runtime = hook
        .body_runtime
        .checked_add(
            u32::try_from(hook.body_bytes)
                .map_err(|_| error("QUEST sparse cache hook body 길이 변환 실패"))?,
        )
        .ok_or_else(|| error("QUEST sparse cache hook body runtime 끝 overflow"))?;
    let item_name_hook_exact = match (
        hook.item_name_page_id,
        hook.item_name_residency_site.as_ref(),
        hook.item_name_entry_runtime,
        hook.item_name_direct_glyph_entry_runtime,
        hook.item_name_whole_string_entry_runtime,
        hook.item_text_mode_residency_site.as_ref(),
        hook.item_text_mode_entry_runtime,
    ) {
        (None, None, None, None, None, None, None) => {
            hook.keeps_non_op21_objects_on_original_atlas
                && hook.item_name_direct_glyph_sites.is_empty()
                && hook.item_name_whole_string_sites.is_empty()
                && hook.item_acquisition_whole_string_sites.is_empty()
                && hook.ui_literal_choice_direct_glyph_sites.is_empty()
                && hook.menu_literal_sites.is_empty()
                && hook.world_map_label_whole_string_site.is_none()
                && hook.item_name_whole_string_active_runtime.is_none()
                && !hook.marks_item_name_objects_with_page
                && !hook.retries_item_name_before_state_advance
                && hook.item_text_page_ids.is_empty()
                && !hook.routes_item_text_pages_only_to_primary_bank
                && !hook.reuses_active_item_text_page_for_names
                && !hook.scopes_item_name_whole_string_renderer_route
                && !hook.scopes_item_acquisition_fixed_message_renderer_route
                && !hook.routes_ui_literal_choices_through_item_page
                && !hook.routes_world_map_labels_through_item_page
                && !hook.routes_item_description_direct_text_mode
        }
        (
            Some(page_id),
            Some(site),
            Some(entry_runtime),
            Some(direct_entry_runtime),
            Some(whole_string_entry_runtime),
            Some(text_mode_site),
            Some(text_mode_entry_runtime),
        ) => {
            page_id != 0
                && usize::from(page_id) <= package.plan.pages
                && hook.item_text_page_ids.first().copied() == Some(page_id)
                && hook
                    .item_text_page_ids
                    .last()
                    .is_some_and(|last| usize::from(*last) <= package.plan.pages)
                && site.base_addr() == QUEST_ITEM_NAME_RESIDENCY_SITE_RUNTIME
                && entry_runtime >= hook.body_runtime
                && entry_runtime < body_end_runtime
                && direct_entry_runtime >= hook.body_runtime
                && direct_entry_runtime < body_end_runtime
                && whole_string_entry_runtime >= hook.body_runtime
                && whole_string_entry_runtime < body_end_runtime
                && text_mode_site.base_addr() == QUEST_SEQ_TEXT_CONSUMER_CALL_RUNTIME
                && text_mode_entry_runtime >= hook.body_runtime
                && text_mode_entry_runtime < body_end_runtime
                && hook
                    .item_name_direct_glyph_sites
                    .iter()
                    .map(VerifiedMachineCode::base_addr)
                    .eq(QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES)
                && hook
                    .item_name_whole_string_sites
                    .iter()
                    .map(VerifiedMachineCode::base_addr)
                    .eq(QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES)
                && hook
                    .item_acquisition_whole_string_sites
                    .iter()
                    .map(VerifiedMachineCode::base_addr)
                    .eq(QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES)
                && hook
                    .ui_literal_choice_direct_glyph_sites
                    .iter()
                    .map(VerifiedMachineCode::base_addr)
                    .eq(QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES)
                && hook
                    .menu_literal_sites
                    .iter()
                    .map(VerifiedMachineCode::base_addr)
                    .eq(crate::disasm::QUEST_MENU_GLYPH_CALLS
                        .iter()
                        .map(|c| c.runtime))
                && hook
                    .world_map_label_whole_string_site
                    .as_ref()
                    .is_some_and(|site| {
                        site.base_addr() == QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME
                    })
                && hook.item_name_whole_string_active_runtime
                    == Some(QUEST_ITEM_NAME_WHOLE_STRING_ACTIVE_RUNTIME)
                && !hook.keeps_non_op21_objects_on_original_atlas
                && hook.marks_item_name_objects_with_page
                && hook.retries_item_name_before_state_advance
                && hook.routes_item_text_pages_only_to_primary_bank
                && hook.reuses_active_item_text_page_for_names
                && hook.scopes_item_name_whole_string_renderer_route
                && hook.scopes_item_acquisition_fixed_message_renderer_route
                && hook.routes_ui_literal_choices_through_item_page
                && hook.routes_world_map_labels_through_item_page
                && hook.routes_item_description_direct_text_mode
        }
        _ => false,
    };
    if package.bytes.len() != package.plan.package_bytes
        || !hook.package_materialized
        || hook.package_bytes != package.bytes.len()
        || hook.body_runtime != expected_body_runtime
        || hook.body.bytes().len() != hook.body_bytes
        || hook.site.base_addr() != QUEST_FONT_HOOK_SITE_RUNTIME
        || hook.original_font_guard_site.base_addr() != QUEST_GLYPH_RENDERER_RUNTIME
        || hook.font_space_guard_site.base_addr() != super::space_guard::SITE_RUNTIME
        || hook.font_object_marker_site.base_addr() != QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME
        || hook.font_draw_descriptor_site.base_addr() != QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME
        || hook.font_draw_texture_page_site.base_addr() != QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME
        || hook.font_explicit_texture_page_site.base_addr()
            != super::explicit_texture_page::SITE_RUNTIME
        || hook.original_font_guard_entry_runtime < hook.body_runtime
        || hook.original_font_guard_entry_runtime >= body_end_runtime
        || hook.font_object_marker_entry_runtime < hook.body_runtime
        || hook.font_object_marker_entry_runtime >= body_end_runtime
        || hook.font_draw_descriptor_entry_runtime < hook.body_runtime
        || hook.font_draw_descriptor_entry_runtime >= body_end_runtime
        || hook.font_draw_texture_page_entry_runtime < hook.body_runtime
        || hook.font_draw_texture_page_entry_runtime >= body_end_runtime
        || hook.korean_descriptor_runtime != expected_korean_descriptor_runtime
        || !hook.keeps_original_atlas_unchanged
        || !item_name_hook_exact
        || !hook.keeps_unselected_non_op21_objects_on_original_atlas
        || !hook.marks_op21_objects_with_route_page
        || !hook.selects_atlas_per_object
        || !hook.routes_marked_objects_through_per_object_texture_page
        || !hook.scans_marked_font_objects
        || !hook.selects_unpinned_atlas_bank
        || !hook.yields_only_when_all_atlas_banks_pinned
        || hook.runtime_verified
        || hook.build_eligible
    {
        return Err(error("QUEST sparse cache package/hook 입력 계약 drift"));
    }

    let expected_item_description_residency_sites =
        usize::from(hook.item_description_constructor_entry_runtime.is_some())
            * QUEST_ITEM_DESCRIPTION_RESIDENCY_SITE_IDS.len();
    if hook.item_description_residency_sites.len() != expected_item_description_residency_sites {
        return Err(error(format!(
            "QUEST ITEM_DAT 설명 residency site 분모 불일치: expected {}, got {}",
            expected_item_description_residency_sites,
            hook.item_description_residency_sites.len()
        )));
    }

    let mut machine_sites = vec![
        QuestGlyphCacheInstallationSite {
            code: &hook.font_space_guard_site,
            id: "quest-font-space-guard-site",
            purpose: "번역 page에서 원본 공백 코드에 배정된 글자도 그리기",
        },
        QuestGlyphCacheInstallationSite {
            code: &hook.site,
            id: "quest-glyph-cache-site",
            purpose: "OP21 entry를 sparse cache carrier body로 우회",
        },
        QuestGlyphCacheInstallationSite {
            code: &hook.original_font_guard_site,
            id: "quest-original-font-guard-site",
            purpose: "OP21 glyph renderer에 route page를 전달",
        },
        QuestGlyphCacheInstallationSite {
            code: &hook.font_object_marker_site,
            id: "quest-font-object-marker-site",
            purpose: "생성한 font object에 route page를 기록",
        },
        QuestGlyphCacheInstallationSite {
            code: &hook.font_draw_descriptor_site,
            id: "quest-font-draw-descriptor-site",
            purpose: "font object marker에 따라 descriptor atlas를 선택",
        },
        QuestGlyphCacheInstallationSite {
            code: &hook.font_draw_texture_page_site,
            id: "quest-font-draw-texture-page-site",
            purpose: "font object marker에 따라 texture page를 선택",
        },
    ];
    machine_sites.push(QuestGlyphCacheInstallationSite {
        code: &hook.font_explicit_texture_page_site,
        id: "quest-font-explicit-texture-page-site",
        purpose: "SPRT 앞에 DR_TPAGE를 연결해 한글·원본 texture page와 UV 경계 보존",
    });
    if let Some(site) = hook.item_name_residency_site.as_ref() {
        machine_sites.push(QuestGlyphCacheInstallationSite {
            code: site,
            id: "quest-item-name-residency-site",
            purpose: "ITEM_DAT 이름 glyph writer보다 먼저 supplemental page residency를 보장",
        });
    }
    machine_sites.extend(
        hook.item_name_direct_glyph_sites
            .iter()
            .zip(QUEST_ITEM_NAME_DIRECT_GLYPH_SITE_IDS)
            .map(|(code, id)| QuestGlyphCacheInstallationSite {
                code,
                id,
                purpose: "ITEM_DAT direct 이름 glyph call을 supplemental page 재시도 래퍼로 우회",
            }),
    );
    machine_sites.extend(
        hook.item_name_whole_string_sites
            .iter()
            .zip(QUEST_ITEM_NAME_WHOLE_STRING_SITE_IDS)
            .map(|(code, id)| QuestGlyphCacheInstallationSite {
                code,
                id,
                purpose: "ITEM_DAT whole-string 이름 draw call을 scoped page 재시도 래퍼로 우회",
            }),
    );
    machine_sites.extend(
        hook.item_acquisition_whole_string_sites
            .iter()
            .zip(QUEST_ITEM_ACQUISITION_WHOLE_STRING_SITE_IDS)
            .map(|(code, id)| QuestGlyphCacheInstallationSite {
                code,
                id,
                purpose: "아이템 획득 고정문을 공용 item page whole-string 래퍼로 우회",
            }),
    );
    machine_sites.extend(
        hook.ui_literal_choice_direct_glyph_sites
            .iter()
            .zip(QUEST_UI_CHOICE_DIRECT_GLYPH_SITE_IDS)
            .map(|(code, id)| QuestGlyphCacheInstallationSite {
                code,
                id,
                purpose: "공용 예/아니요 glyph call을 supplemental page 재시도 래퍼로 우회",
            }),
    );
    machine_sites.extend(
        hook.menu_literal_sites
            .iter()
            .zip(crate::disasm::QUEST_MENU_GLYPH_CALLS)
            .map(|(code, call)| QuestGlyphCacheInstallationSite {
                code,
                id: call.id,
                purpose: "상태·장비·도구 고정문을 item page glyph 경로로 연결",
            }),
    );
    if let Some(site) = hook.world_map_label_whole_string_site.as_ref() {
        machine_sites.push(QuestGlyphCacheInstallationSite {
            code: site,
            id: "quest-world-map-label-whole-string-site-800663e0",
            purpose: "world-map 지명 draw를 supplemental page whole-string 래퍼로 우회",
        });
    }
    if let Some(site) = hook.item_text_mode_residency_site.as_ref() {
        machine_sites.push(QuestGlyphCacheInstallationSite {
            code: site,
            id: "quest-item-description-text-mode-residency-site",
            purpose: "direct text consumer 전에 ITEM_DAT 설명 page residency를 보장",
        });
    }
    machine_sites.extend(
        hook.item_description_residency_sites
            .iter()
            .zip(QUEST_ITEM_DESCRIPTION_RESIDENCY_SITE_IDS)
            .map(|(code, id)| QuestGlyphCacheInstallationSite {
                code,
                id,
                purpose: "ITEM_DAT 설명 constructor 전에 대상 page residency를 보장",
            }),
    );

    Ok(QuestGlyphCacheInstallation {
        body_end_runtime,
        machine_sites,
    })
}

#[cfg(test)]
#[path = "installation_tests.rs"]
mod tests;
