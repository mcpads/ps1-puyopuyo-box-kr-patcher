//! ITEM_DAT callback·direct glyph·whole-string·설명 경로의 공용 page 요청 결속.
//!
//! mode별 frame 복귀와 page 요청 entry는 기존 전용 assembler가 만든다. 이
//! 모듈은 그 결과를 공용 loader의 성공·재시도와 결속하고, 각 요청이 같은
//! 검증된 page entry로만 향하도록 branch와 jump fixup을 소유한다.

use crate::isa::InstructionKind as K;

use super::super::super::quest_item_description_residency_hook::append_item_description_constructor_entry;
use super::super::super::quest_item_glyph_cache_dispatch::{
    append_item_return_paths, QuestItemReturnPaths,
};
use super::super::super::quest_item_name_glyph_cache_hook::append_item_name_entries;
use super::super::{QuestGlyphCacheHookPlanError, QuestItemTextPageMode};
use super::{patch_branch, patch_jump};

pub(super) struct ItemTextReturnRoutes {
    paths: QuestItemReturnPaths,
}

pub(super) struct ItemTextRouteEntries {
    pub(super) callback: Option<usize>,
    pub(super) direct_glyph: Option<usize>,
    pub(super) whole_string: Option<usize>,
    pub(super) text_mode: Option<usize>,
    pub(super) description_constructor: Option<usize>,
    pub(super) successful_non_op21_return: usize,
    pub(super) failed_non_op21_return: usize,
}

pub(super) fn append_item_return_routes(
    kinds: &mut Vec<K>,
    enabled: bool,
    particle_codes: &[(u16, u16)],
) -> Result<ItemTextReturnRoutes, QuestGlyphCacheHookPlanError> {
    let mut addition = Vec::new();
    super::item_object_particle::append_item_object_particle(&mut addition, particle_codes)?;
    let mut direct = Vec::new();
    super::item_object_particle::append_discard_object_particle(&mut direct, particle_codes)?;
    Ok(ItemTextReturnRoutes {
        paths: append_item_return_paths(kinds, enabled, &addition, &direct),
    })
}

pub(super) fn append_item_request_routes(
    kinds: &mut Vec<K>,
    return_routes: ItemTextReturnRoutes,
    item_text_pages: Option<QuestItemTextPageMode>,
    page_request_entry: usize,
    original_font_guard_return: usize,
    body_runtime: u32,
) -> Result<ItemTextRouteEntries, QuestGlyphCacheHookPlanError> {
    let item_name_page_id = item_text_pages.map(|pages| pages.default_name_page_id);
    let item_name_entries = append_item_name_entries(
        kinds,
        item_name_page_id,
        item_text_pages.map(|pages| (pages.first_page_id, pages.last_page_id)),
        item_text_pages.map_or(0, |pages| pages.shared_slots),
    );
    let description_constructor = item_text_pages
        .filter(|pages| pages.first_page_id < pages.last_page_id)
        .map(|pages| {
            append_item_description_constructor_entry(
                kinds,
                pages.first_page_id,
                pages.last_page_id,
            )
        });

    let successful_non_op21_return = return_routes
        .paths
        .success_direct_branch
        .map(|branch| branch - 1)
        .unwrap_or(original_font_guard_return);
    let failed_non_op21_return = return_routes
        .paths
        .retry_direct_branch
        .map(|branch| branch - 1)
        .unwrap_or(original_font_guard_return);

    for (source, target) in [
        (
            return_routes.paths.success_direct_branch,
            return_routes.paths.direct_success,
        ),
        (
            return_routes.paths.success_whole_string_branch,
            return_routes.paths.whole_string_success,
        ),
        (
            return_routes.paths.success_text_mode_branch,
            return_routes.paths.text_mode_success,
        ),
        (
            return_routes.paths.success_description_constructor_branch,
            return_routes.paths.description_constructor_success,
        ),
        (
            return_routes.paths.success_callback_branch,
            return_routes.paths.callback_success,
        ),
        (
            return_routes.paths.retry_direct_branch,
            return_routes.paths.direct_retry,
        ),
        (
            return_routes.paths.retry_whole_string_branch,
            return_routes.paths.whole_string_retry,
        ),
        (
            return_routes.paths.retry_text_mode_branch,
            return_routes.paths.text_mode_retry,
        ),
        (
            return_routes.paths.retry_description_constructor_branch,
            return_routes.paths.description_constructor_retry,
        ),
        (
            return_routes.paths.retry_callback_branch,
            return_routes.paths.callback_retry,
        ),
    ] {
        if let (Some(source), Some(target)) = (source, target) {
            patch_branch(kinds, source, target)?;
        }
    }

    for entry in [
        item_name_entries.callback.as_ref(),
        item_name_entries.direct_glyph.as_ref(),
        item_name_entries.whole_string.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        patch_jump(
            kinds,
            entry.page_request_jump,
            page_request_entry,
            body_runtime,
        )?;
    }
    if let Some(entry) = item_name_entries.text_mode.as_ref() {
        patch_jump(
            kinds,
            entry.page_request_jump,
            page_request_entry,
            body_runtime,
        )?;
        for branch in &entry.bypass_branches {
            patch_branch(kinds, *branch, entry.bypass)?;
        }
    }
    if let Some(entry) = description_constructor.as_ref() {
        patch_jump(
            kinds,
            entry.page_request_jump,
            page_request_entry,
            body_runtime,
        )?;
        for branch in &entry.bypass_branches {
            patch_branch(kinds, *branch, entry.bypass)?;
        }
    }

    Ok(ItemTextRouteEntries {
        callback: item_name_entries.callback.map(|entry| entry.entry),
        direct_glyph: item_name_entries.direct_glyph.map(|entry| entry.entry),
        whole_string: item_name_entries.whole_string.map(|entry| entry.entry),
        text_mode: item_name_entries.text_mode.map(|entry| entry.entry),
        description_constructor: description_constructor.map(|entry| entry.entry),
        successful_non_op21_return,
        failed_non_op21_return,
    })
}
