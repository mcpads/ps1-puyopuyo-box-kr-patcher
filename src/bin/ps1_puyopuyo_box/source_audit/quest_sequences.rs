//! QUEST SEQ text 모집단, page 공급과 opcode 경계를 검증한다.

#[path = "quest_sequences/execution_paths.rs"]
mod execution_paths;
#[path = "quest_sequences/glyph_page_supply.rs"]
mod glyph_page_supply;
#[path = "quest_sequences/item_acquisition_route.rs"]
mod item_acquisition_route;
#[path = "quest_sequences/item_text_development.rs"]
mod item_text_development;
#[path = "quest_sequences/poc_font_plan.rs"]
mod poc_font_plan;
#[path = "quest_sequences/resource_transitions.rs"]
mod resource_transitions;
#[path = "quest_sequences/suke_confrontation.rs"]
mod suke_confrontation;
#[path = "quest_sequences/text_sources.rs"]
mod text_sources;
#[path = "quest_sequences/translation_development.rs"]
mod translation_development;

use execution_paths::{audit_quest_execution_paths, QuestExecutionPathAuditInputs};
use glyph_page_supply::{audit_quest_glyph_page_supply, QuestGlyphPageSupplyAuditInputs};
use item_acquisition_route::{
    audit_quest_item_acquisition_route, QuestItemAcquisitionRouteAuditInputs,
};
use item_text_development::{audit_quest_development, QuestDevelopmentAuditInputs};
use poc_font_plan::audit_quest_poc_font_plan;
use resource_transitions::audit_quest_resource_transitions;
use text_sources::{audit_quest_text_sources, QuestTextSourceAudit, QuestTextSourceAuditInputs};
use translation_development::{
    audit_quest_translation_development, QuestTranslationDevelopmentAudit,
    QuestTranslationDevelopmentAuditInputs,
};

use super::{
    audit_quest_vm, ArchiveAuditData, BiosFontAnalysis, Box32mSummary, QuestFontConsumerSummary,
    SourceAuditContext,
};
use crate::cli::analysis_inputs::read_quest_seq_sources;
use crate::cli::{quest_code_table_from_executable, RPG_SEQ_PATH};

pub(super) fn audit_quest_sequences(
    context: &SourceAuditContext<'_>,
    archives: &ArchiveAuditData,
    reserved: &Box32mSummary,
    bios_font: &BiosFontAnalysis,
    quest_font_consumer: &QuestFontConsumerSummary,
) -> Result<(), String> {
    let source = context.source;
    let iso = &context.iso;
    let box_executables = &context.box_executables;
    let cnx_archives = archives.archive_copies();
    let quest_runtime_executable = box_executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let quest_map = quest_runtime_executable.map;
    let quest_text = &quest_runtime_executable.text;
    let quest_vm = audit_quest_vm(quest_runtime_executable)?;
    let (_, rpg_seq) = cnx_archives
        .iter()
        .find(|(source_id, _)| *source_id == RPG_SEQ_PATH)
        .ok_or_else(|| format!("{RPG_SEQ_PATH} archive가 없음"))?;
    let quest_seq_sources = read_quest_seq_sources(rpg_seq)?;
    let code_table = quest_code_table_from_executable(quest_map, quest_text)?;
    let QuestTextSourceAudit {
        town00,
        seq_addresses: quest_seq_addresses,
        item_sequence,
        item_units: quest_item_text,
        item_name_drafts: quest_item_name_drafts,
        item_description_drafts: quest_item_description_drafts,
        text_extraction: quest_seq_text_extraction,
        text_documents: quest_seq_text_documents,
    } = audit_quest_text_sources(QuestTextSourceAuditInputs {
        archive: rpg_seq,
        sequences: &quest_seq_sources,
        executable_map: quest_map,
        executable_text: quest_text,
        code_table: &code_table,
    })?;
    let QuestTranslationDevelopmentAudit {
        translation_batches: quest_translation_batches,
        translation_reviews: quest_translation_reviews,
        translation_routing: quest_translation_routing,
        development_selection: quest_development_selection,
        development_routing: quest_development_routing,
        development_layouts: quest_development_layouts,
        development_page_packing: quest_development_page_packing,
    } = audit_quest_translation_development(QuestTranslationDevelopmentAuditInputs {
        source,
        executable_map: quest_map,
        executable_text: quest_text,
        text_extraction: &quest_seq_text_extraction,
        text_documents: &quest_seq_text_documents,
        seq_addresses: &quest_seq_addresses,
        code_table: &code_table,
    })?;
    audit_quest_glyph_page_supply(QuestGlyphPageSupplyAuditInputs {
        source,
        iso,
        reserved,
        executable_map: quest_map,
        executable_text: quest_text,
        bios_font,
        font_consumer: quest_font_consumer,
        translation_batches: quest_translation_batches,
        translation_reviews: quest_translation_reviews,
        translation_routing: quest_translation_routing,
        text_extraction: &quest_seq_text_extraction,
        code_table: &code_table,
    })?;
    let quest_execution_paths = audit_quest_execution_paths(QuestExecutionPathAuditInputs {
        executable_map: quest_map,
        executable_text: quest_text,
        sequences: &quest_seq_sources,
        seq_addresses: &quest_seq_addresses,
        text_extraction: &quest_seq_text_extraction,
        code_table: &code_table,
        registration: &quest_vm.registration,
        child_activation: &quest_vm.child_activation,
        child_configuration: &quest_vm.child_configuration,
    })?;
    audit_quest_item_acquisition_route(QuestItemAcquisitionRouteAuditInputs {
        sequences: &quest_seq_sources,
        text_paths: &quest_execution_paths.text_paths,
        consumer: &quest_vm.item_acquisition,
    })?;
    audit_quest_development(QuestDevelopmentAuditInputs {
        source,
        executable_map: quest_map,
        executable_text: quest_text,
        item_sequence,
        item_units: &quest_item_text,
        name_drafts: &quest_item_name_drafts,
        description_drafts: &quest_item_description_drafts,
        item_acquisition: &quest_vm.item_acquisition,
        resident_units: &quest_execution_paths.resident_text_units,
        battle_names: &quest_execution_paths.battle_names,
        resident_encounter_text: &quest_execution_paths.resident_encounter_text,
        selection: &quest_development_selection,
        routing: &quest_development_routing,
        documents: &quest_seq_text_documents,
        addresses: &quest_seq_addresses,
        layouts: &quest_development_layouts,
        page_packing: &quest_development_page_packing,
        code_table: &code_table,
    })?;
    audit_quest_resource_transitions(
        &quest_seq_sources,
        &quest_execution_paths.registration,
        &quest_execution_paths.text_frontiers,
    )?;
    audit_quest_poc_font_plan(&town00, &quest_seq_sources, &code_table)?;
    Ok(())
}
