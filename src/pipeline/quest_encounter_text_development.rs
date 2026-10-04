//! QUEST 자연 조우 이름·등장문·선택지의 공통 page 개발 입력.
//!
//! 기존 resident/item page 뒤에서 page ID를 파생하고 59개 전투 이름과
//! 조우 OP21 네 run이 실제로 쓰는 문자 합집합을 한 page에 공급한다. 기존
//! page의 문자→code 배정은 그대로 보존하며, encounter OP21과 고정 이름
//! slot writer를 같은 encoding/package에 결속한다. 번역·geometry·runtime·
//! 제품 승인은 올리지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::analysis::{
    combine_quest_resident_text_units, QuestBattleNameTableAnalysis,
    QuestResidentEncounterTextAnalysis, QuestResidentTextUnitAnalysis,
};
use crate::coords::ExeMap;
use crate::disc::VerifiedSourceImage;
use crate::font::QuestCodeTable;
use crate::script::QuestTranslationSelectionAnalysis;

use super::quest_battle_name_compositions::{
    audit_quest_battle_name_compositions, QuestBattleNameCompositionAnalysis,
};
use super::quest_battle_name_writer::{
    audit_quest_battle_name_writer, verify_quest_battle_name_writer_code_supply,
    QuestBattleNameWriterAnalysis,
};
use super::quest_glyph_cache_hook_plan::QuestGlyphCacheHookPlan;
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_glyph_cache_plan::QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX;
use super::quest_item_text_glyph_cache_development::QuestItemTextGlyphCacheDevelopment;
use super::quest_manual_stream_writer::quest_page_code_maps;
use super::quest_resident_encounter_text_composition::{
    audit_quest_resident_encounter_text_composition, QuestResidentEncounterCompositionAnalysis,
};
use super::quest_resident_text_compositions::audit_quest_resident_text_compositions;
use super::quest_resident_text_writer::{
    audit_quest_resident_text_writer, QuestResidentTextPageAssignment,
    QuestResidentTextWriterAnalysis,
};
use super::quest_translation_encoding::{
    align_quest_page_code_prefix, build_quest_translation_encoding_plan_with_supplemental_pages,
    quest_translation_page_storage_glyphs, QuestTranslationEncodingPlan,
    QuestTranslationEncodingSupplementalPage,
};
use super::quest_translation_glyph_cache_hook_plan::verify_box_quest_translation_glyph_cache_hook_plan_with_item_text_pages;
use super::quest_translation_glyph_cache_package::verify_box_quest_translation_glyph_cache_package_with_encoding;
use super::quest_translation_selection_routing::QuestTranslationSelectionRoutingAnalysis;

/// 조우 한 화면에 필요한 전체 문자와 파생 page 용량.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestEncounterTextPageDemand {
    pub page_id: u8,
    pub battle_name_used_glyphs: String,
    pub encounter_used_glyphs: String,
    pub shared_glyphs: String,
    pub used_glyphs: String,
    pub storage_glyphs: String,
    pub dynamic_slot_capacity: usize,
}

/// 기존 page를 보존하면서 조우 page와 두 writer를 결속한 개발 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestEncounterTextDevelopment {
    pub page: QuestEncounterTextPageDemand,
    pub encoding: QuestTranslationEncodingPlan,
    pub package: QuestGlyphCachePackage,
    pub hook: QuestGlyphCacheHookPlan,
    pub resident_text_writer: QuestResidentTextWriterAnalysis,
    pub battle_name_writer: QuestBattleNameWriterAnalysis,
    pub previous_page_codes_exact: bool,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// 현재 item text 개발 입력에 자연 조우 page를 더하는 데 필요한 source-bound 입력.
pub struct QuestEncounterTextDevelopmentInput<'a> {
    pub source: &'a VerifiedSourceImage,
    pub executable_map: &'a ExeMap,
    pub executable_text: &'a [u8],
    pub resident_units: &'a QuestResidentTextUnitAnalysis,
    pub encounter: &'a QuestResidentEncounterTextAnalysis,
    pub battle_names: &'a QuestBattleNameTableAnalysis,
    pub item_text: &'a QuestItemTextGlyphCacheDevelopment,
    pub selection: &'a QuestTranslationSelectionAnalysis,
    pub routing: &'a QuestTranslationSelectionRoutingAnalysis,
    pub code_table: &'a QuestCodeTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestEncounterTextDevelopmentError(String);

impl fmt::Display for QuestEncounterTextDevelopmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestEncounterTextDevelopmentError {}

fn error(message: impl Into<String>) -> QuestEncounterTextDevelopmentError {
    QuestEncounterTextDevelopmentError(message.into())
}

/// 두 source-bound composition의 실제 사용 문자 합집합과 다음 page를 계산한다.
pub fn derive_quest_encounter_text_page_demand(
    previous_page_id: u8,
    battle_names: &QuestBattleNameCompositionAnalysis,
    encounter: &QuestResidentEncounterCompositionAnalysis,
    code_table: &QuestCodeTable,
) -> Result<QuestEncounterTextPageDemand, QuestEncounterTextDevelopmentError> {
    if previous_page_id == 0
        || battle_names.used_glyphs.is_empty()
        || encounter.used_glyphs.is_empty()
        || battle_names.translation_approved
        || battle_names.layout_approved
        || battle_names.runtime_verified
        || battle_names.product_eligible
        || encounter.translation_approved
        || encounter.layout_approved
        || encounter.runtime_verified
        || encounter.product_eligible
    {
        return Err(error("QUEST encounter page demand 입력 경계 drift"));
    }
    let page_id = previous_page_id
        .checked_add(1)
        .ok_or_else(|| error("QUEST encounter page ID overflow"))?;
    let battle_set = battle_names.used_glyphs.chars().collect::<BTreeSet<_>>();
    let encounter_set = encounter.used_glyphs.chars().collect::<BTreeSet<_>>();
    let shared_glyphs = battle_set
        .intersection(&encounter_set)
        .copied()
        .collect::<String>();
    let used = battle_set
        .union(&encounter_set)
        .copied()
        .collect::<BTreeSet<_>>();
    let storage = quest_translation_page_storage_glyphs(&used, code_table)
        .map_err(|source| error(source.to_string()))?;
    let dynamic_slot_capacity = code_table
        .codes()
        .len()
        .checked_sub(QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX)
        .ok_or_else(|| error("QUEST encounter page 동적 slot 범위 역전"))?;
    if storage.is_empty() || storage.len() > dynamic_slot_capacity {
        return Err(error(format!(
            "QUEST encounter page glyph 수요가 동적 slot 용량을 넘음: demand={}, capacity={dynamic_slot_capacity}",
            storage.len()
        )));
    }
    Ok(QuestEncounterTextPageDemand {
        page_id,
        battle_name_used_glyphs: battle_set.into_iter().collect(),
        encounter_used_glyphs: encounter_set.into_iter().collect(),
        shared_glyphs,
        used_glyphs: used.into_iter().collect(),
        storage_glyphs: storage.into_iter().collect(),
        dynamic_slot_capacity,
    })
}

fn combine_resident_units(
    resident: &QuestResidentTextUnitAnalysis,
    encounter: &QuestResidentEncounterTextAnalysis,
) -> Result<QuestResidentTextUnitAnalysis, QuestEncounterTextDevelopmentError> {
    let encounter_unit = [encounter.unit.clone()];
    combine_quest_resident_text_units([resident.units.as_slice(), encounter_unit.as_slice()])
        .map_err(|source| error(source.to_string()))
}

/// 기존 item text package 뒤에 조우 공통 page를 붙이고 두 writer를 검산한다.
pub fn build_quest_encounter_text_development(
    input: QuestEncounterTextDevelopmentInput<'_>,
) -> Result<QuestEncounterTextDevelopment, QuestEncounterTextDevelopmentError> {
    let QuestEncounterTextDevelopmentInput {
        source,
        executable_map,
        executable_text,
        resident_units,
        encounter,
        battle_names,
        item_text,
        selection,
        routing,
        code_table,
    } = input;
    let previous_page_id = item_text
        .item_text_page_ids
        .last()
        .copied()
        .ok_or_else(|| error("QUEST encounter development 이전 item page가 비었음"))?;
    if item_text.item_text_page_ids.len() != item_text.item_text_page_used_glyphs.len()
        || item_text.resident.package.plan.pages != usize::from(previous_page_id)
        || item_text
            .resident
            .encoding
            .pages
            .last()
            .map(|page| page.page_id)
            != Some(previous_page_id)
        || item_text.item_text_page_ids.first().copied()
            != item_text.resident.page_id.checked_add(1)
        || item_text
            .item_text_page_ids
            .windows(2)
            .any(|pair| pair[1] != pair[0] + 1)
        || item_text.shared_item_page_glyphs.is_empty()
        || item_text.translation_approved
        || item_text.layout_approved
        || item_text.item_writer_linked
        || item_text.runtime_verified
        || item_text.product_eligible
    {
        return Err(error("QUEST encounter development 기존 page 경계 drift"));
    }

    let battle_name_compositions = audit_quest_battle_name_compositions(battle_names)
        .map_err(|source| error(source.to_string()))?;
    let encounter_composition = audit_quest_resident_encounter_text_composition(&encounter.unit)
        .map_err(|source| error(source.to_string()))?;
    let page = derive_quest_encounter_text_page_demand(
        previous_page_id,
        &battle_name_compositions,
        &encounter_composition,
        code_table,
    )?;

    let mut supplemental_pages = Vec::with_capacity(item_text.item_text_page_ids.len() + 2);
    supplemental_pages.push(QuestTranslationEncodingSupplementalPage {
        page_id: item_text.resident.page_id,
        used_glyphs: item_text.resident.used_glyphs.clone(),
    });
    supplemental_pages.extend(
        item_text
            .item_text_page_ids
            .iter()
            .copied()
            .zip(item_text.item_text_page_used_glyphs.iter().cloned())
            .map(
                |(page_id, used_glyphs)| QuestTranslationEncodingSupplementalPage {
                    page_id,
                    used_glyphs,
                },
            ),
    );
    supplemental_pages.push(QuestTranslationEncodingSupplementalPage {
        page_id: page.page_id,
        used_glyphs: page.used_glyphs.clone(),
    });
    let mut encoding = build_quest_translation_encoding_plan_with_supplemental_pages(
        selection,
        routing,
        code_table,
        &supplemental_pages,
    )
    .map_err(|source| error(source.to_string()))?;
    if item_text.item_text_page_ids.len() >= 2 {
        align_quest_page_code_prefix(
            &mut encoding,
            &item_text.item_text_page_ids,
            &item_text.shared_item_page_glyphs,
        )
        .map_err(|source| error(source.to_string()))?;
    }
    let mut package =
        verify_box_quest_translation_glyph_cache_package_with_encoding(source, routing, &encoding)
            .map_err(|source| error(source.to_string()))?;
    // encounter page를 추가해도 기존 메뉴의 폰트 identity를 보존한다.
    let materials =
        super::quest_glyph_cache_plan::build_quest_glyph_cache_materials_with_menu_font(
            source,
            package.plan.schema_version,
            encoding
                .pages
                .iter()
                .map(|p| (p.page_id, p.glyphs.chars().collect()))
                .collect(),
            encoding.repertoire.chars().collect(),
            item_text.resident.package.menu_font_page,
        )
        .map_err(|e| error(e.to_string()))?;
    package = super::quest_glyph_cache_package::serialize_quest_glyph_cache_materials(
        materials,
        package.schema_version,
    )
    .map_err(|e| error(e.to_string()))?;
    let previous_page_codes = quest_page_code_maps(
        &item_text.resident.encoding,
        &item_text.resident.package,
        code_table,
    )
    .map_err(|source| error(source.to_string()))?;
    let current_page_codes = quest_page_code_maps(&encoding, &package, code_table)
        .map_err(|source| error(source.to_string()))?;
    let previous_page_codes_exact = previous_page_codes
        .iter()
        .all(|(page_id, codes)| current_page_codes.get(page_id) == Some(codes));
    if !previous_page_codes_exact {
        return Err(error("QUEST encounter 추가 뒤 기존 page code가 달라짐"));
    }
    // 이름별로 검산한 조사 분류만 이어 받는다. 같은 code map임을 먼저 확인한다.
    for page_id in &item_text.item_text_page_ids {
        let index = usize::from(*page_id) - 1;
        let required = &item_text.resident.package.object_particle_codes[index];
        let supplied = &mut package.object_particle_codes[index];
        if !required.iter().all(|pair| supplied.contains(pair)) {
            return Err(error("QUEST encounter 추가 뒤 item 조사 공급 불일치"));
        }
        supplied.retain(|pair| required.contains(pair));
    }
    let hook = verify_box_quest_translation_glyph_cache_hook_plan_with_item_text_pages(
        executable_map,
        executable_text,
        &package,
        routing,
        &item_text.item_text_page_ids,
    )
    .map_err(|source| error(source.to_string()))?;

    let combined_units = combine_resident_units(resident_units, encounter)?;
    let mut compositions = audit_quest_resident_text_compositions(resident_units)
        .map_err(|source| error(source.to_string()))?;
    compositions.push(encounter_composition.composition.clone());
    let mut assignments = resident_units
        .units
        .iter()
        .map(|unit| QuestResidentTextPageAssignment {
            unit_id: unit.id.clone(),
            page_id: item_text.resident.page_id,
        })
        .collect::<Vec<_>>();
    assignments.push(QuestResidentTextPageAssignment {
        unit_id: encounter.unit.id.clone(),
        page_id: page.page_id,
    });
    let resident_text_writer = audit_quest_resident_text_writer(
        &combined_units,
        &compositions,
        &assignments,
        &encoding,
        &package,
        code_table,
    )
    .map_err(|source| error(source.to_string()))?;
    let battle_name_writer = audit_quest_battle_name_writer(
        executable_map,
        executable_text,
        battle_names,
        &battle_name_compositions,
        page.page_id,
        &encoding,
        &package,
        code_table,
    )
    .map_err(|source| error(source.to_string()))?;
    verify_quest_battle_name_writer_code_supply(
        &battle_name_writer,
        &encoding,
        &package,
        code_table,
    )
    .map_err(|source| error(source.to_string()))?;

    let encounter_write = resident_text_writer
        .writes
        .iter()
        .find(|write| write.unit_id == encounter.unit.id)
        .ok_or_else(|| error("QUEST encounter resident write가 없음"))?;
    if package.plan.pages != usize::from(page.page_id)
        || encoding.pages.last().map(|entry| entry.page_id) != Some(page.page_id)
        || encoding.page_glyphs(page.page_id) != Some(page.storage_glyphs.as_str())
        || !previous_page_codes_exact
        || hook.item_text_page_ids != item_text.item_text_page_ids
        || hook.item_name_page_id != item_text.item_text_page_ids.first().copied()
        || hook.runtime_verified
        || hook.build_eligible
        || resident_text_writer.units != resident_units.units.len() + 1
        || encounter_write.page_id != page.page_id
        || resident_text_writer
            .writes
            .iter()
            .filter(|write| write.unit_id != encounter.unit.id)
            .any(|write| write.page_id != item_text.resident.page_id)
        || battle_name_writer.page_id != page.page_id
        || battle_name_writer.physical_names != battle_names.entries.len()
        || battle_name_writer.used_glyphs != page.battle_name_used_glyphs
        || resident_text_writer.translation_approved
        || resident_text_writer.layout_approved
        || resident_text_writer.runtime_verified
        || resident_text_writer.product_eligible
        || battle_name_writer.translation_approved
        || battle_name_writer.layout_approved
        || battle_name_writer.runtime_verified
        || battle_name_writer.product_eligible
    {
        return Err(error("QUEST encounter development 최종 결속 drift"));
    }

    Ok(QuestEncounterTextDevelopment {
        page,
        encoding,
        package,
        hook,
        resident_text_writer,
        battle_name_writer,
        previous_page_codes_exact,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    })
}

#[cfg(test)]
#[path = "quest_encounter_text_development_tests.rs"]
mod tests;
