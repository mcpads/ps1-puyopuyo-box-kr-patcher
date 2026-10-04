//! QUEST 기반 development disc 빌드가 공유하는 원본 재구성 입력.
//!
//! QUEST 분석 보고서 입력과 누적 기능 조정을 분리한다. 이 모듈은 원본과
//! version 관리 입력에서 QUEST build input을 재구성하고, 요청한 build
//! scope의 조정자에 한 번 넘기는 역할만 맡는다.

use std::path::Path;

use ps1_puyopuyo_box::analysis::{
    combine_quest_resident_text_units, verify_box_quest_battle_name_table,
    verify_box_quest_item_acquisition_text_units, verify_box_quest_resident_encounter_path,
    verify_box_quest_resident_encounter_text, verify_box_quest_resident_escape_text,
    verify_box_quest_resident_seq_frontiers, verify_box_quest_resident_seq_paths,
    verify_box_quest_resident_seq_spawn_path, verify_box_quest_resident_seq_transition,
    verify_box_quest_resident_text_units, verify_box_quest_ui_literal_units,
    QuestBattleNameTableAnalysis, QuestResidentEncounterTextAnalysis,
    QuestResidentTextUnitAnalysis, QuestSeqResourceCatalogLayout,
};
use ps1_puyopuyo_box::coords::ExeMap;
use ps1_puyopuyo_box::disasm::{
    verify_box_quest_item_acquisition, verify_box_quest_item_text_consumer,
    verify_box_quest_seq_registration, verify_box_quest_ui_literals,
};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::font::QuestCodeTable;
use ps1_puyopuyo_box::pipeline::{
    audit_quest_item_acquisition_text_compositions, audit_quest_item_acquisition_text_writer,
    audit_quest_item_text_writer, audit_quest_ui_literal_compositions,
    audit_quest_ui_literal_writer, build_current_development_disc, build_quest_development_disc,
    build_quest_encounter_text_development, build_quest_item_text_glyph_cache_development,
    verify_box_quest_manual_stream_writer_with_code_supply_and_layouts,
    CurrentDevelopmentDiscBuild, QuestDevelopmentDiscBuild, QuestDevelopmentDiscInput,
    QuestEncounterTextDevelopmentInput, QuestGlyphCacheHookPlan, QuestGlyphCachePackage,
    QuestItemTextGlyphCacheDevelopmentInput, QuestManualStreamCodeSupply,
    QuestManualStreamWriterAnalysis, QuestTranslationSelectionRoutingAnalysis,
};
use ps1_puyopuyo_box::review::audit_quest_manual_composition_catalog;
use ps1_puyopuyo_box::script::{
    audit_quest_item_description_drafts, audit_quest_item_name_drafts,
    verify_box_quest_item_text_units, verify_box_quest_translation_approvals,
};

use super::disc_commands::read_box_executables;
use super::quest_analysis_inputs::{
    read_quest_review_inputs, read_quest_translation_selection_inputs,
};
use super::quest_manual_composition_development::{
    build_quest_manual_composition_development, QuestManualCompositionDevelopmentInput,
};

pub(super) struct QuestDevelopmentDiscInputs {
    pub build: QuestDevelopmentDiscBuild,
    pub package: QuestGlyphCachePackage,
    pub hook: QuestGlyphCacheHookPlan,
    pub routing: QuestTranslationSelectionRoutingAnalysis,
    pub writer: QuestManualStreamWriterAnalysis,
}

pub(super) struct CurrentDevelopmentDiscInputs {
    pub build: CurrentDevelopmentDiscBuild,
    pub package: QuestGlyphCachePackage,
    pub hook: QuestGlyphCacheHookPlan,
    pub routing: QuestTranslationSelectionRoutingAnalysis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DevelopmentDiscScope {
    QuestOnly,
    Current,
}

enum DevelopmentDiscInputs {
    Quest(Box<QuestDevelopmentDiscInputs>),
    Current(Box<CurrentDevelopmentDiscInputs>),
}

struct QuestDevelopmentResidentTextSources {
    resident_units: QuestResidentTextUnitAnalysis,
    battle_names: QuestBattleNameTableAnalysis,
    resident_encounter_text: QuestResidentEncounterTextAnalysis,
}

fn audit_quest_development_resident_text_sources(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    seq_sources: &[(String, Vec<u8>)],
) -> Result<QuestDevelopmentResidentTextSources, String> {
    let registration =
        verify_box_quest_seq_registration(map, text).map_err(|error| error.to_string())?;
    let transition = verify_box_quest_resident_seq_transition(
        map,
        text,
        seq_sources.iter().map(|(name, _)| name.as_str()),
        QuestSeqResourceCatalogLayout {
            descriptor_root_runtime: registration.descriptor_root_runtime,
            descriptor_stride: registration.descriptor_stride,
            prefix_descriptor_rows: registration.prefix_descriptor_rows,
        },
    )
    .map_err(|error| error.to_string())?;
    let paths = verify_box_quest_resident_seq_paths(map, text, code_table)
        .map_err(|error| error.to_string())?;
    let frontiers = verify_box_quest_resident_seq_frontiers(map, text, &paths)
        .map_err(|error| error.to_string())?;
    let encounter_path =
        verify_box_quest_resident_encounter_path(map, text, code_table, &frontiers)
            .map_err(|error| error.to_string())?;
    let battle_names = verify_box_quest_battle_name_table(map, text, code_table)
        .map_err(|error| error.to_string())?;
    let resident_encounter_text = verify_box_quest_resident_encounter_text(
        map,
        text,
        code_table,
        &encounter_path,
        &battle_names,
    )
    .map_err(|error| error.to_string())?;
    let spawn =
        verify_box_quest_resident_seq_spawn_path(map, text, code_table, &frontiers, &transition)
            .map_err(|error| error.to_string())?;
    let base_resident_units =
        verify_box_quest_resident_text_units(map, text, code_table, &paths, &spawn)
            .map_err(|error| error.to_string())?;
    let escape = verify_box_quest_resident_escape_text(map, text, code_table, &encounter_path)
        .map_err(|error| error.to_string())?;
    let resident_units = combine_quest_resident_text_units([
        base_resident_units.units.as_slice(),
        escape.units.units.as_slice(),
    ])
    .map_err(|error| error.to_string())?;
    Ok(QuestDevelopmentResidentTextSources {
        resident_units,
        battle_names,
        resident_encounter_text,
    })
}

/// 사람이 작성한 QUEST 조판과 현재 기능 집합을 development image로 재구성한다.
fn read_development_disc_inputs(
    source_path: &Path,
    source: &VerifiedSourceImage,
    scope: DevelopmentDiscScope,
) -> Result<DevelopmentDiscInputs, String> {
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let inputs = read_quest_translation_selection_inputs(source_path)?;
    let review_inputs = read_quest_review_inputs(source_path)?;
    let approvals = verify_box_quest_translation_approvals(&review_inputs.workbook)
        .map_err(|error| error.to_string())?;
    let manual = audit_quest_manual_composition_catalog(
        &review_inputs.workbook,
        &approvals,
        &review_inputs.extraction.streams,
    )
    .map_err(|error| error.to_string())?;
    let development =
        build_quest_manual_composition_development(QuestManualCompositionDevelopmentInput {
            manual_compositions: &manual,
            source_selection: &inputs.selection,
            text_documents: &inputs.documents,
            source_routing: &inputs.source_routing,
            code_table: &inputs.code_table,
        })?;
    let selection = development.selection;
    let routing = development.routing;
    let explicit_layouts = development.layouts;
    let resident_sources = audit_quest_development_resident_text_sources(
        quest.map,
        &quest.text,
        &inputs.code_table,
        &inputs.seq_sources,
    )?;
    let item_data = inputs
        .seq_sources
        .iter()
        .find(|(name, _)| name == "ITEM_DAT.SEQ")
        .ok_or_else(|| "QUEST ITEM_DAT.SEQ 검증 입력이 없음".to_string())?;
    let item_units = verify_box_quest_item_text_units(&item_data.1, &inputs.code_table)
        .map_err(|error| error.to_string())?;
    let item_drafts =
        audit_quest_item_name_drafts(&item_units).map_err(|error| error.to_string())?;
    let item_description_drafts =
        audit_quest_item_description_drafts(&item_units).map_err(|error| error.to_string())?;
    let item_text_consumer = verify_box_quest_item_text_consumer(quest.map, &quest.text)
        .map_err(|error| error.to_string())?;
    let item_acquisition =
        verify_box_quest_item_acquisition(quest.map, &quest.text, &item_text_consumer)
            .map_err(|error| error.to_string())?;
    let item_acquisition_units = verify_box_quest_item_acquisition_text_units(&item_acquisition)
        .map_err(|error| error.to_string())?;
    let item_acquisition_compositions =
        audit_quest_item_acquisition_text_compositions(&item_acquisition_units)
            .map_err(|error| error.to_string())?;
    let ui_literals =
        verify_box_quest_ui_literals(quest.map, &quest.text).map_err(|error| error.to_string())?;
    let ui_literal_units =
        verify_box_quest_ui_literal_units(&ui_literals).map_err(|error| error.to_string())?;
    let ui_literal_compositions = audit_quest_ui_literal_compositions(&ui_literal_units)
        .map_err(|error| error.to_string())?;
    let item_development =
        build_quest_item_text_glyph_cache_development(QuestItemTextGlyphCacheDevelopmentInput {
            source,
            executable_map: quest.map,
            executable_text: &quest.text,
            item_units: &item_units,
            drafts: &item_drafts,
            description_drafts: &item_description_drafts,
            item_acquisition_used_glyphs: &item_acquisition_compositions.used_glyphs,
            ui_literal_used_glyphs: &ui_literal_compositions.used_glyphs,
            resident_units: &resident_sources.resident_units,
            selection: &selection,
            routing: &routing,
            code_table: &inputs.code_table,
        })
        .map_err(|error| error.to_string())?;
    let encounter_development =
        build_quest_encounter_text_development(QuestEncounterTextDevelopmentInput {
            source,
            executable_map: quest.map,
            executable_text: &quest.text,
            resident_units: &resident_sources.resident_units,
            encounter: &resident_sources.resident_encounter_text,
            battle_names: &resident_sources.battle_names,
            item_text: &item_development,
            selection: &selection,
            routing: &routing,
            code_table: &inputs.code_table,
        })
        .map_err(|error| error.to_string())?;
    let writer = verify_box_quest_manual_stream_writer_with_code_supply_and_layouts(
        &inputs.documents,
        &inputs.addresses,
        &selection,
        &routing,
        QuestManualStreamCodeSupply {
            code_table: &inputs.code_table,
            package: &encounter_development.package,
            encoding: &encounter_development.encoding,
        },
        &explicit_layouts,
    )
    .map_err(|error| error.to_string())?;
    let default_item_text_page_id = item_development
        .item_text_page_ids
        .first()
        .copied()
        .ok_or_else(|| "QUEST item text page가 비었음".to_owned())?;
    let item_text_writer = audit_quest_item_text_writer(
        quest.map,
        &quest.text,
        &item_data.1,
        &item_units,
        &item_drafts,
        &item_description_drafts,
        default_item_text_page_id,
        &item_development.description_page_assignments,
        &encounter_development.encoding,
        &encounter_development.package,
        &inputs.code_table,
    )
    .map_err(|error| error.to_string())?;
    let item_acquisition_text_writer = audit_quest_item_acquisition_text_writer(
        quest.map,
        &quest.text,
        &item_acquisition_units,
        &item_acquisition_compositions,
        default_item_text_page_id,
        &encounter_development.encoding,
        &encounter_development.package,
        &inputs.code_table,
    )
    .map_err(|error| error.to_string())?;
    let ui_literal_writer = audit_quest_ui_literal_writer(
        quest.map,
        &quest.text,
        &ui_literal_units,
        &ui_literal_compositions,
        default_item_text_page_id,
        &encounter_development.encoding,
        &encounter_development.package,
        &inputs.code_table,
    )
    .map_err(|error| error.to_string())?;
    let build_input = QuestDevelopmentDiscInput {
        source,
        manual_writer: &writer,
        package: &encounter_development.package,
        hook: &encounter_development.hook,
        resident_text_writer: &encounter_development.resident_text_writer,
        battle_name_writer: &encounter_development.battle_name_writer,
        item_text_writer: &item_text_writer,
        item_acquisition_text_writer: &item_acquisition_text_writer,
        ui_literal_writer: &ui_literal_writer,
        encoding: &encounter_development.encoding,
        code_table: &inputs.code_table,
    };
    match scope {
        DevelopmentDiscScope::QuestOnly => {
            let build =
                build_quest_development_disc(build_input).map_err(|error| error.to_string())?;
            Ok(DevelopmentDiscInputs::Quest(Box::new(
                QuestDevelopmentDiscInputs {
                    build,
                    package: encounter_development.package,
                    hook: encounter_development.hook,
                    routing,
                    writer,
                },
            )))
        }
        DevelopmentDiscScope::Current => {
            let build =
                build_current_development_disc(build_input).map_err(|error| error.to_string())?;
            Ok(DevelopmentDiscInputs::Current(Box::new(
                CurrentDevelopmentDiscInputs {
                    build,
                    package: encounter_development.package,
                    hook: encounter_development.hook,
                    routing,
                },
            )))
        }
    }
}

/// QUEST 기능만 포함한 development image를 재구성한다.
pub(super) fn read_quest_development_disc_inputs(
    source_path: &Path,
    source: &VerifiedSourceImage,
) -> Result<QuestDevelopmentDiscInputs, String> {
    match read_development_disc_inputs(source_path, source, DevelopmentDiscScope::QuestOnly)? {
        DevelopmentDiscInputs::Quest(inputs) => Ok(*inputs),
        DevelopmentDiscInputs::Current(_) => {
            Err("QUEST development scope routing drift".to_string())
        }
    }
}

/// 현재 활성 기능을 모두 포함한 누적 development image를 재구성한다.
pub(super) fn read_current_development_disc_inputs(
    source_path: &Path,
    source: &VerifiedSourceImage,
) -> Result<CurrentDevelopmentDiscInputs, String> {
    match read_development_disc_inputs(source_path, source, DevelopmentDiscScope::Current)? {
        DevelopmentDiscInputs::Current(inputs) => Ok(*inputs),
        DevelopmentDiscInputs::Quest(_) => {
            Err("current development scope routing drift".to_string())
        }
    }
}
