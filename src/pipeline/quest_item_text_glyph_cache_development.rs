//! QUEST 아이템 텍스트 전반을 공용 glyph cache page에 결속하는 개발 입력.
//!
//! source-bound 이름·설명과 획득 고정문에 결속된 한국어 초안 repertoire로
//! supplemental page, typed callback hook, carrier 예산을 함께 검증한다. 독립
//! 검토·조판 승인, 재삽입·runtime·제품 적격을 뜻하지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::analysis::QuestResidentTextUnitAnalysis;
use crate::coords::ExeMap;
use crate::disasm::QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES;
use crate::disc::VerifiedSourceImage;
use crate::font::QuestCodeTable;
use crate::script::{
    QuestItemDescriptionDraftAnalysis, QuestItemNameDraftAnalysis, QuestItemTextUnitAnalysis,
    QuestTranslationSelectionAnalysis,
};

use super::quest_glyph_cache_hook_plan::QuestGlyphCacheHookPlan;
use super::quest_glyph_cache_plan::QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX;
use super::quest_resident_text_development::{
    build_quest_resident_text_development_inputs_with_aligned_following_pages,
    QuestResidentTextDevelopmentInputs,
};
use super::quest_translation_encoding::{
    normalize_quest_translation_text, quest_translation_page_storage_glyphs,
};
use super::quest_translation_glyph_cache_hook_plan::verify_box_quest_translation_glyph_cache_hook_plan_with_item_text_pages;
use super::quest_translation_selection_routing::QuestTranslationSelectionRoutingAnalysis;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemTextGlyphCacheDevelopment {
    pub item_text_page_ids: Vec<u8>,
    pub name_units: usize,
    pub description_units: usize,
    pub name_used_glyphs: String,
    pub item_acquisition_used_glyphs: String,
    pub ui_literal_used_glyphs: String,
    pub shared_item_page_glyphs: String,
    pub combined_item_text_used_glyphs: String,
    pub combined_item_text_storage_glyphs: String,
    pub single_page_dynamic_slot_capacity: usize,
    pub single_page_fits: bool,
    pub item_text_page_used_glyphs: Vec<String>,
    pub item_text_page_glyphs: Vec<String>,
    pub description_page_assignments: Vec<QuestItemDescriptionPageAssignment>,
    pub resident: QuestResidentTextDevelopmentInputs,
    pub hook: QuestGlyphCacheHookPlan,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub item_writer_linked: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// 설명 unit이 사용할 결합 glyph page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemDescriptionPageAssignment {
    pub unit_id: String,
    pub page_id: u8,
}

/// 이름·설명·획득 고정문 page와 hook을 같은 source revision에서 만드는 입력.
pub struct QuestItemTextGlyphCacheDevelopmentInput<'a> {
    pub source: &'a VerifiedSourceImage,
    pub executable_map: &'a ExeMap,
    pub executable_text: &'a [u8],
    pub item_units: &'a QuestItemTextUnitAnalysis,
    pub drafts: &'a QuestItemNameDraftAnalysis,
    pub description_drafts: &'a QuestItemDescriptionDraftAnalysis,
    pub item_acquisition_used_glyphs: &'a str,
    pub ui_literal_used_glyphs: &'a str,
    pub resident_units: &'a QuestResidentTextUnitAnalysis,
    pub selection: &'a QuestTranslationSelectionAnalysis,
    pub routing: &'a QuestTranslationSelectionRoutingAnalysis,
    pub code_table: &'a QuestCodeTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemTextGlyphCacheDevelopmentError(String);

impl fmt::Display for QuestItemTextGlyphCacheDevelopmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemTextGlyphCacheDevelopmentError {}

fn error(message: impl Into<String>) -> QuestItemTextGlyphCacheDevelopmentError {
    QuestItemTextGlyphCacheDevelopmentError(message.into())
}

fn draft_name_glyphs(
    item_units: &QuestItemTextUnitAnalysis,
    drafts: &QuestItemNameDraftAnalysis,
) -> Result<String, QuestItemTextGlyphCacheDevelopmentError> {
    if drafts.entries.len() != item_units.name_units
        || drafts.entries.len() != item_units.unique_records
        || drafts.used_glyphs.is_empty()
        || drafts.translation_approved
        || drafts.layout_approved
        || drafts.product_eligible
    {
        return Err(error("QUEST item name 수동 초안 분모 또는 승인 경계 drift"));
    }
    let mut exceptions = super::quest_item_name_particles::pronunciations().map_err(error)?;
    for entry in &drafts.entries {
        super::quest_item_name_particles::pronunciation_tail(&entry.unit_id, &entry.draft_korean)
            .map_err(error)?;
        exceptions.remove(&entry.unit_id);
        if entry.draft_korean.chars().count() >= 64 {
            return Err(error("QUEST item 이름이 목적격 조사 탐색 상한을 넘음"));
        }
    }
    if !exceptions.is_empty() {
        return Err(error("미소비 item 이름 발음 예외"));
    }
    Ok(normalize_quest_translation_text(&drafts.used_glyphs)
        .map_err(|source| error(source.to_string()))?
        .chars()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

fn draft_description_glyphs(
    item_units: &QuestItemTextUnitAnalysis,
    drafts: &QuestItemDescriptionDraftAnalysis,
) -> Result<String, QuestItemTextGlyphCacheDevelopmentError> {
    if drafts.entries.len() != item_units.description_units
        || drafts.entries.len() != item_units.unique_records
        || drafts.used_glyphs.is_empty()
        || drafts.translation_approved
        || drafts.layout_approved
        || drafts.product_eligible
    {
        return Err(error(
            "QUEST item description 수동 초안 분모 또는 승인 경계 drift",
        ));
    }
    Ok(normalize_quest_translation_text(&drafts.used_glyphs)
        .map_err(|source| error(source.to_string()))?
        .chars()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

#[derive(Debug)]
struct DescriptionPageGroup {
    unit_ids: Vec<String>,
    used_glyphs: BTreeSet<char>,
}

type DescriptionGlyphEntry = (String, BTreeSet<char>, BTreeSet<char>);

fn partition_description_glyph_entries(
    shared_glyphs: &BTreeSet<char>,
    mut entries: Vec<DescriptionGlyphEntry>,
    code_table: &QuestCodeTable,
) -> Result<Vec<DescriptionPageGroup>, QuestItemTextGlyphCacheDevelopmentError> {
    entries.sort_by(|left, right| {
        right
            .2
            .len()
            .cmp(&left.2.len())
            .then_with(|| left.0.cmp(&right.0))
    });

    let dynamic_slot_capacity = code_table
        .codes()
        .len()
        .checked_sub(QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX)
        .ok_or_else(|| error("QUEST item text 동적 code slot 범위 역전"))?;
    for page_count in 1..=entries.len() {
        let mut groups = (0..page_count)
            .map(|_| DescriptionPageGroup {
                unit_ids: Vec::new(),
                used_glyphs: shared_glyphs.clone(),
            })
            .collect::<Vec<_>>();
        let mut exclusive_by_group = vec![BTreeSet::<char>::new(); page_count];
        for (unit_id, used, exclusive) in &entries {
            let (group_index, merged_exclusive) = exclusive_by_group
                .iter()
                .enumerate()
                .map(|(index, current)| {
                    let merged = current.union(exclusive).copied().collect::<BTreeSet<_>>();
                    (index, merged, current.len(), groups[index].unit_ids.len())
                })
                .min_by_key(|(index, merged, current_len, units)| {
                    (merged.len(), merged.len() - current_len, *units, *index)
                })
                .map(|(index, merged, _, _)| (index, merged))
                .ok_or_else(|| error("QUEST item description page group가 비었음"))?;
            exclusive_by_group[group_index] = merged_exclusive;
            groups[group_index].unit_ids.push(unit_id.clone());
            groups[group_index].used_glyphs.extend(used);
        }
        if groups.iter().any(|group| group.unit_ids.is_empty()) {
            continue;
        }
        let fits = groups.iter().all(|group| {
            quest_translation_page_storage_glyphs(&group.used_glyphs, code_table)
                .is_ok_and(|glyphs| glyphs.len() <= dynamic_slot_capacity)
        });
        if fits {
            for group in &mut groups {
                group.unit_ids.sort();
            }
            return Ok(groups);
        }
    }
    Err(error(format!(
        "QUEST item description을 공용 item code page에 분할할 수 없음: shared={}, capacity={dynamic_slot_capacity}, largest={}",
        shared_glyphs.len(), entries.iter().map(|(_, used, _)| shared_glyphs.union(used).count()).max().unwrap_or(0),
    )))
}

fn partition_item_description_pages(
    shared_used_glyphs: &str,
    ui_used_glyphs: &str,
    item_units: &QuestItemTextUnitAnalysis,
    drafts: &QuestItemDescriptionDraftAnalysis,
    code_table: &QuestCodeTable,
) -> Result<Vec<DescriptionPageGroup>, QuestItemTextGlyphCacheDevelopmentError> {
    let shared_glyphs = normalize_quest_translation_text(shared_used_glyphs)
        .map_err(|source| error(source.to_string()))?
        .chars()
        .collect::<BTreeSet<_>>();
    let source_ids = item_units
        .units
        .iter()
        .filter(|unit| unit.role == crate::script::QuestItemTextRole::Description)
        .map(|unit| unit.id.as_str())
        .collect::<BTreeSet<_>>();
    if source_ids.len() != item_units.description_units || drafts.entries.len() != source_ids.len()
    {
        return Err(error("QUEST item description page 분모 불일치"));
    }

    let entries = drafts
        .entries
        .iter()
        .map(|entry| {
            if !source_ids.contains(entry.unit_id.as_str()) {
                return Err(error(format!(
                    "QUEST item description page 비대상 unit: {}",
                    entry.unit_id
                )));
            }
            let used = normalize_quest_translation_text(&entry.draft_korean)
                .map_err(|source| error(source.to_string()))?
                .chars()
                .collect::<BTreeSet<_>>();
            let exclusive = used
                .difference(&shared_glyphs)
                .copied()
                .collect::<BTreeSet<_>>();
            Ok((entry.unit_id.clone(), used, exclusive))
        })
        .collect::<Result<Vec<_>, _>>()?;
    partition_with_ui_page(&shared_glyphs, ui_used_glyphs, entries, code_table)
}

// 메뉴는 첫 page의 Denki로, 설명은 뒤 page의 갈무리로 공급한다.
// 이름·조사 code prefix는 모든 page에서 동일하며 폰트 cell만 다르다.
fn partition_with_ui_page(
    shared: &BTreeSet<char>,
    ui: &str,
    entries: Vec<DescriptionGlyphEntry>,
    code_table: &QuestCodeTable,
) -> Result<Vec<DescriptionPageGroup>, QuestItemTextGlyphCacheDevelopmentError> {
    let capacity = code_table.codes().len() - QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX;
    let first = DescriptionPageGroup {
        unit_ids: Vec::new(),
        used_glyphs: shared.iter().copied().chain(ui.chars()).collect(),
    };
    if quest_translation_page_storage_glyphs(&first.used_glyphs, code_table)
        .map_err(|e| error(e.to_string()))?
        .len()
        > capacity
    {
        return Err(error("QUEST 메뉴 page가 동적 slot 수용량을 넘음"));
    }
    let mut groups = vec![first];
    groups.extend(partition_description_glyph_entries(
        shared, entries, code_table,
    )?);
    Ok(groups)
}

#[cfg(test)]
#[path = "quest_item_text_glyph_cache_development_tests.rs"]
mod tests;

/// 수동 item text page를 실제 번역 package 뒤에 붙여 hook 예산을 검증한다.
pub fn build_quest_item_text_glyph_cache_development(
    input: QuestItemTextGlyphCacheDevelopmentInput<'_>,
) -> Result<QuestItemTextGlyphCacheDevelopment, QuestItemTextGlyphCacheDevelopmentError> {
    let QuestItemTextGlyphCacheDevelopmentInput {
        source,
        executable_map,
        executable_text,
        item_units,
        drafts,
        description_drafts,
        item_acquisition_used_glyphs,
        ui_literal_used_glyphs,
        resident_units,
        selection,
        routing,
        code_table,
    } = input;
    let name_used_glyphs = draft_name_glyphs(item_units, drafts)?;
    let item_acquisition_used_glyphs =
        normalize_quest_translation_text(item_acquisition_used_glyphs)
            .map_err(|source| error(source.to_string()))?
            .chars()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<String>();
    if item_acquisition_used_glyphs.is_empty() {
        return Err(error("QUEST item acquisition 고정문 사용 glyph가 비었음"));
    }
    let ui_literal_used_glyphs = normalize_quest_translation_text(ui_literal_used_glyphs)
        .map_err(|source| error(source.to_string()))?
        .chars()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<String>();
    if ui_literal_used_glyphs.is_empty() {
        return Err(error("QUEST UI literal 사용 glyph가 비었음"));
    }
    let shared_ui = super::quest_ui_literal_compositions::shared_item_ui_glyphs()
        .map_err(|source| error(source.to_string()))?;
    let shared_item_used_glyphs = name_used_glyphs
        .chars()
        .chain(item_acquisition_used_glyphs.chars())
        .chain(
            ui_literal_used_glyphs
                .chars()
                .filter(|glyph| shared_ui.contains(glyph)),
        )
        .chain(['을', '를'])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<String>();
    let description_used_glyphs = draft_description_glyphs(item_units, description_drafts)?;
    let combined_item_text_used_glyphs = shared_item_used_glyphs
        .chars()
        .chain(ui_literal_used_glyphs.chars())
        .chain(description_used_glyphs.chars())
        .collect::<BTreeSet<_>>();
    let combined_item_text_storage_glyphs =
        quest_translation_page_storage_glyphs(&combined_item_text_used_glyphs, code_table)
            .map_err(|source| error(source.to_string()))?;
    let single_page_dynamic_slot_capacity = code_table
        .codes()
        .len()
        .checked_sub(QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX)
        .ok_or_else(|| error("QUEST item text 동적 code slot 범위 역전"))?;
    let single_page_fits =
        combined_item_text_storage_glyphs.len() <= single_page_dynamic_slot_capacity;
    let description_groups = partition_item_description_pages(
        &shared_item_used_glyphs,
        &ui_literal_used_glyphs,
        item_units,
        description_drafts,
        code_table,
    )?;
    let item_text_page_used_glyphs = description_groups
        .iter()
        .map(|group| group.used_glyphs.iter().copied().collect::<String>())
        .collect::<Vec<_>>();
    let mut resident = build_quest_resident_text_development_inputs_with_aligned_following_pages(
        source,
        resident_units,
        selection,
        routing,
        code_table,
        &item_text_page_used_glyphs,
        &shared_item_used_glyphs,
    )
    .map_err(|source| error(source.to_string()))?;
    let item_text_page_ids = (0..description_groups.len())
        .map(|index| {
            resident
                .page_id
                .checked_add(
                    u8::try_from(index + 1)
                        .map_err(|_| error("QUEST item text supplemental page 수가 u8을 넘음"))?,
                )
                .ok_or_else(|| error("QUEST item text supplemental page ID overflow"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let default_name_page_id = *item_text_page_ids
        .first()
        .ok_or_else(|| error("QUEST item text supplemental page가 비었음"))?;
    let final_item_text_page_id = *item_text_page_ids
        .last()
        .ok_or_else(|| error("QUEST item text supplemental page가 비었음"))?;
    // 이름의 마지막 글자만 조사 분류에 필요하다. 설명에 등장한 다른 음절은
    // 실행 코드로 펼치지 않는다. writer가 모든 이름의 공급을 다시 검산한다.
    let page_codes = super::quest_manual_stream_writer::quest_page_code_maps(
        &resident.encoding,
        &resident.package,
        code_table,
    )
    .map_err(|source| error(source.to_string()))?;
    for page_id in &item_text_page_ids {
        let codes = page_codes
            .get(page_id)
            .ok_or_else(|| error("QUEST item 조사 page code 누락"))?;
        let mut required = BTreeSet::new();
        for entry in &drafts.entries {
            let normalized = normalize_quest_translation_text(&entry.draft_korean)
                .map_err(|source| error(source.to_string()))?;
            let last = normalized
                .chars()
                .last()
                .ok_or_else(|| error("빈 item 이름"))?;
            let tail = super::quest_item_name_particles::pronunciation_tail(
                &entry.unit_id,
                &entry.draft_korean,
            )
            .map_err(error)?;
            let particle = crate::text::select_object_particle(tail)
                .map_err(|source| error(source.to_string()))?;
            let code = |glyph| {
                codes
                    .get(&glyph)
                    .copied()
                    .map(u16::from_le_bytes)
                    .ok_or_else(|| error("QUEST item 조사 glyph code 누락"))
            };
            required.insert((code(last)?, code(particle)?));
        }
        let supplied = &mut resident.package.object_particle_codes[usize::from(*page_id) - 1];
        if !required.iter().all(|pair| supplied.contains(pair)) {
            return Err(error("QUEST item 이름 끝글자 조사 공급 불일치"));
        }
        supplied.retain(|pair| required.contains(pair));
    }
    let hook = verify_box_quest_translation_glyph_cache_hook_plan_with_item_text_pages(
        executable_map,
        executable_text,
        &resident.package,
        routing,
        &item_text_page_ids,
    )
    .map_err(|source| error(source.to_string()))?;
    let item_text_page_glyphs = item_text_page_ids
        .iter()
        .map(|page_id| {
            resident
                .encoding
                .page_glyphs(*page_id)
                .map(str::to_owned)
                .ok_or_else(|| {
                    error(format!(
                        "QUEST item text supplemental encoding page가 없음: {page_id}"
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let shared_used_set = shared_item_used_glyphs.chars().collect::<BTreeSet<_>>();
    let shared_item_page_glyphs = item_text_page_glyphs
        .first()
        .ok_or_else(|| error("QUEST item text encoding page가 비었음"))?
        .chars()
        .filter(|glyph| shared_used_set.contains(glyph))
        .collect::<String>();
    let description_page_assignments = description_groups
        .iter()
        .zip(&item_text_page_ids)
        .flat_map(|(group, page_id)| {
            group
                .unit_ids
                .iter()
                .map(|unit_id| QuestItemDescriptionPageAssignment {
                    unit_id: unit_id.clone(),
                    page_id: *page_id,
                })
        })
        .collect::<Vec<_>>();

    let pages_exact = item_text_page_used_glyphs
        .iter()
        .zip(&item_text_page_glyphs)
        .all(|(used, encoded)| {
            quest_translation_page_storage_glyphs(
                &used.chars().collect::<BTreeSet<_>>(),
                code_table,
            )
            .is_ok_and(|expected| encoded.chars().collect::<BTreeSet<_>>() == expected)
        });
    let expected_item_text_repertoire = shared_item_used_glyphs
        .chars()
        .chain(ui_literal_used_glyphs.chars())
        .chain(description_used_glyphs.chars())
        .collect::<BTreeSet<_>>();
    let actual_item_text_repertoire = item_text_page_used_glyphs
        .iter()
        .flat_map(|glyphs| glyphs.chars())
        .collect::<BTreeSet<_>>();
    let common_prefix_exact = !shared_item_page_glyphs.is_empty()
        && item_text_page_glyphs
            .iter()
            .all(|glyphs| glyphs.starts_with(&shared_item_page_glyphs));
    if resident.package.plan.pages != usize::from(final_item_text_page_id)
        || !pages_exact
        || actual_item_text_repertoire != expected_item_text_repertoire
        || !common_prefix_exact
        || description_groups.len() < 2
        || !description_groups[0].unit_ids.is_empty()
        || resident.package.menu_font_page != Some(default_name_page_id)
        || description_page_assignments.len() != item_units.description_units
        || hook.item_name_page_id != Some(default_name_page_id)
        || hook.item_name_residency_site.is_none()
        || hook.item_name_entry_runtime.is_none()
        || hook.item_name_direct_glyph_entry_runtime.is_none()
        || hook.item_name_whole_string_entry_runtime.is_none()
        || hook.item_text_mode_entry_runtime.is_none()
        || hook.item_text_mode_residency_site.is_none()
        || hook.item_name_whole_string_active_runtime.is_none()
        || !hook
            .item_name_whole_string_sites
            .iter()
            .map(|site| site.base_addr())
            .eq(crate::disasm::QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES)
        || hook.item_acquisition_whole_string_sites.len()
            != QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES.len()
        || !hook.marks_item_name_objects_with_page
        || !hook.retries_item_name_before_state_advance
        || !hook.routes_item_text_pages_only_to_primary_bank
        || !hook.reuses_active_item_text_page_for_names
        || !hook.scopes_item_name_whole_string_renderer_route
        || !hook.scopes_item_acquisition_fixed_message_renderer_route
        || !hook.routes_ui_literal_choices_through_item_page
        || !hook.routes_world_map_labels_through_item_page
        || hook.ui_literal_choice_direct_glyph_sites.len() != 8
        || hook.world_map_label_whole_string_site.is_none()
        || !hook.routes_item_description_direct_text_mode
        || hook.guards_item_description_constructor_with_residency != (description_groups.len() > 1)
        || hook.item_description_residency_sites.len()
            != usize::from(description_groups.len() > 1) * 3
        || hook.item_description_constructor_entry_runtime.is_some()
            != (description_groups.len() > 1)
        || hook.keeps_non_op21_objects_on_original_atlas
        || !hook.keeps_unselected_non_op21_objects_on_original_atlas
        || hook.runtime_verified
        || hook.build_eligible
    {
        return Err(error("QUEST item name glyph cache development 결속 drift"));
    }

    Ok(QuestItemTextGlyphCacheDevelopment {
        item_text_page_ids,
        name_units: item_units.name_units,
        description_units: item_units.description_units,
        name_used_glyphs,
        item_acquisition_used_glyphs,
        ui_literal_used_glyphs,
        shared_item_page_glyphs,
        combined_item_text_used_glyphs: combined_item_text_used_glyphs.into_iter().collect(),
        combined_item_text_storage_glyphs: combined_item_text_storage_glyphs.into_iter().collect(),
        single_page_dynamic_slot_capacity,
        single_page_fits,
        item_text_page_used_glyphs,
        item_text_page_glyphs,
        description_page_assignments,
        resident,
        hook,
        translation_approved: drafts.translation_approved,
        layout_approved: drafts.layout_approved,
        item_writer_linked: false,
        runtime_verified: false,
        product_eligible: false,
    })
}
