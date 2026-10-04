//! QUEST source-bound text writer를 최종 disc 보고 경계로 바꾼다.

use std::collections::BTreeMap;

use crate::script::{QuestItemNameDraftStatus, QuestItemTextRole};

use super::{
    error, sha256, QuestDevelopmentDiscBattleNameReport, QuestDevelopmentDiscError,
    QuestDevelopmentDiscItemAcquisitionTextReport, QuestDevelopmentDiscItemDescriptionReport,
    QuestDevelopmentDiscItemNameReport, QuestDevelopmentDiscResidentTextReport,
    QuestDevelopmentDiscUiLiteralReport,
};
use crate::pipeline::quest_battle_name_compositions::QuestBattleNameDecisionStatus;
use crate::pipeline::quest_battle_name_writer::{
    verify_quest_battle_name_writer_provenance, QuestBattleNameWriterAnalysis,
};
use crate::pipeline::quest_item_acquisition_text_compositions::QuestItemAcquisitionTextCompositionStatus;
use crate::pipeline::quest_item_acquisition_text_writer::{
    verify_quest_item_acquisition_text_writer_provenance, QuestItemAcquisitionTextWriterAnalysis,
};
use crate::pipeline::quest_item_name_writer::verify_quest_item_name_writer_provenance;
use crate::pipeline::quest_item_text_writer::{
    verify_quest_item_text_writer_provenance, QuestItemTextWriterAnalysis,
};
use crate::pipeline::quest_resident_text_writer::{
    verify_quest_resident_text_writer_provenance, QuestResidentTextCompositionStatus,
    QuestResidentTextWriterAnalysis,
};
use crate::pipeline::quest_ui_literal_compositions::QuestUiLiteralCompositionStatus;
use crate::pipeline::quest_ui_literal_writer::{
    verify_quest_ui_literal_writer_provenance, QuestUiLiteralWriterAnalysis,
};

pub(super) fn prepare_battle_name_report(
    writer: &QuestBattleNameWriterAnalysis,
) -> Result<QuestDevelopmentDiscBattleNameReport, QuestDevelopmentDiscError> {
    verify_quest_battle_name_writer_provenance(writer)
        .map_err(|source| error(source.to_string()))?;
    let source_bytes = writer.writes.iter().try_fold(0usize, |total, write| {
        total
            .checked_add(write.source.len())
            .ok_or_else(|| error("QUEST development battle name source byte 합계 overflow"))
    })?;
    let output_bytes = writer.writes.iter().try_fold(0usize, |total, write| {
        total
            .checked_add(write.output.len())
            .ok_or_else(|| error("QUEST development battle name output byte 합계 overflow"))
    })?;
    let glyph_cells = writer.writes.iter().try_fold(0usize, |total, write| {
        total
            .checked_add(write.glyph_cells)
            .ok_or_else(|| error("QUEST development battle name glyph cell 합계 overflow"))
    })?;
    let selected_drafts = writer
        .writes
        .iter()
        .filter(|write| {
            write.decision_status == QuestBattleNameDecisionStatus::DraftNeedsIndependentReview
        })
        .count();
    let needs_human_choice = writer
        .writes
        .iter()
        .filter(|write| {
            write.decision_status == QuestBattleNameDecisionStatus::DraftNeedsHumanChoice
        })
        .count();
    if writer.page_id == 0
        || writer.writes.is_empty()
        || writer.physical_names != writer.writes.len()
        || writer.unique_sources == 0
        || writer.unique_sources > writer.physical_names
        || writer.source_bytes != source_bytes
        || writer.output_bytes != output_bytes
        || writer.glyph_cells != glyph_cells
        || writer.selected_drafts != selected_drafts
        || writer.needs_human_choice != needs_human_choice
        || selected_drafts + needs_human_choice != writer.physical_names
        || source_bytes != output_bytes
        || writer.writes.iter().any(|write| {
            write.page_id != writer.page_id
                || write.source.len() != write.output.len()
                || write.source_sha256 != sha256(&write.source)
                || write.output_sha256 != sha256(&write.output)
        })
        || !writer.source_exact
        || !writer.nul_terminal_exact
        || writer.japanese_glyphs_remaining != 0
        || writer.translation_approved
        || writer.layout_approved
        || writer.runtime_verified
        || writer.product_eligible
    {
        return Err(error(
            "QUEST development battle name writer 입력 계약 drift",
        ));
    }
    Ok(QuestDevelopmentDiscBattleNameReport {
        writes: writer.writes.clone(),
        page_id: writer.page_id,
        physical_names: writer.physical_names,
        unique_sources: writer.unique_sources,
        glyph_cells,
        source_bytes,
        output_bytes,
        selected_drafts,
        needs_human_choice,
        source_exact: writer.source_exact,
        nul_terminal_exact: writer.nul_terminal_exact,
        final_exact: false,
        japanese_glyphs_remaining: writer.japanese_glyphs_remaining,
        translation_approved: writer.translation_approved,
        layout_approved: writer.layout_approved,
        runtime_verified: writer.runtime_verified,
        product_eligible: writer.product_eligible,
    })
}

pub(super) fn prepare_ui_literal_report(
    writer: &QuestUiLiteralWriterAnalysis,
) -> Result<QuestDevelopmentDiscUiLiteralReport, QuestDevelopmentDiscError> {
    verify_quest_ui_literal_writer_provenance(writer)
        .map_err(|source| error(source.to_string()))?;
    let source_bytes = writer.writes.iter().map(|write| write.source.len()).sum();
    let output_bytes = writer.writes.iter().map(|write| write.output.len()).sum();
    let glyph_cells = writer.writes.iter().map(|write| write.glyph_cells).sum();
    if writer.page_id == 0
        || writer.units != crate::analysis::BOX_QUEST_UI_LITERAL_UNITS
        || writer.units != writer.writes.len()
        || writer.world_map_labels != 12
        || writer.binary_choices != 2
        || writer.source_bytes != source_bytes
        || writer.output_bytes != output_bytes
        || writer.glyph_cells != glyph_cells
        || source_bytes != output_bytes
        || writer.writes.iter().any(|write| {
            write.page_id != writer.page_id
                || write.status != QuestUiLiteralCompositionStatus::ManualDraftNeedsRuntimeGeometry
        })
        || !writer.source_exact
        || !writer.nul_terminal_exact
        || writer.japanese_glyphs_remaining != 0
        || writer.translation_approved
        || writer.layout_approved
        || writer.runtime_verified
        || writer.product_eligible
    {
        return Err(error("QUEST development UI literal writer 입력 계약 drift"));
    }
    Ok(QuestDevelopmentDiscUiLiteralReport {
        writes: writer.writes.clone(),
        page_id: writer.page_id,
        units: writer.units,
        world_map_labels: writer.world_map_labels,
        binary_choices: writer.binary_choices,
        glyph_cells,
        source_bytes,
        output_bytes,
        status: QuestUiLiteralCompositionStatus::ManualDraftNeedsRuntimeGeometry,
        source_exact: writer.source_exact,
        nul_terminal_exact: writer.nul_terminal_exact,
        final_exact: false,
        japanese_glyphs_remaining: writer.japanese_glyphs_remaining,
        translation_approved: writer.translation_approved,
        layout_approved: writer.layout_approved,
        runtime_verified: writer.runtime_verified,
        product_eligible: writer.product_eligible,
    })
}

pub(super) fn prepare_item_acquisition_text_report(
    writer: &QuestItemAcquisitionTextWriterAnalysis,
) -> Result<QuestDevelopmentDiscItemAcquisitionTextReport, QuestDevelopmentDiscError> {
    verify_quest_item_acquisition_text_writer_provenance(writer)
        .map_err(|source| error(source.to_string()))?;
    let source_bytes = writer.writes.iter().map(|write| write.source.len()).sum();
    let output_bytes = writer.writes.iter().map(|write| write.output.len()).sum();
    let glyph_cells = writer.writes.iter().map(|write| write.glyph_cells).sum();
    if writer.page_id == 0
        || writer.writes.is_empty()
        || writer.units != writer.writes.len()
        || writer.source_bytes != source_bytes
        || writer.output_bytes != output_bytes
        || writer.glyph_cells != glyph_cells
        || source_bytes != output_bytes
        || writer.writes.iter().any(|write| {
            write.page_id != writer.page_id
                || write.status
                    != QuestItemAcquisitionTextCompositionStatus::ManualDraftNeedsRuntimeGeometry
        })
        || !writer.source_exact
        || !writer.nul_terminal_exact
        || writer.japanese_glyphs_remaining != 0
        || writer.translation_approved
        || writer.layout_approved
        || writer.runtime_verified
        || writer.product_eligible
    {
        return Err(error(
            "QUEST development item acquisition writer 입력 계약 drift",
        ));
    }
    Ok(QuestDevelopmentDiscItemAcquisitionTextReport {
        writes: writer.writes.clone(),
        page_id: writer.page_id,
        units: writer.units,
        glyph_cells,
        source_bytes,
        output_bytes,
        status: QuestItemAcquisitionTextCompositionStatus::ManualDraftNeedsRuntimeGeometry,
        source_exact: writer.source_exact,
        nul_terminal_exact: writer.nul_terminal_exact,
        final_exact: false,
        japanese_glyphs_remaining: writer.japanese_glyphs_remaining,
        translation_approved: writer.translation_approved,
        layout_approved: writer.layout_approved,
        runtime_verified: writer.runtime_verified,
        product_eligible: writer.product_eligible,
    })
}

pub(super) fn prepare_resident_text_report(
    writer: &QuestResidentTextWriterAnalysis,
) -> Result<QuestDevelopmentDiscResidentTextReport, QuestDevelopmentDiscError> {
    verify_quest_resident_text_writer_provenance(writer)
        .map_err(|source| error(source.to_string()))?;
    let source_bytes = writer.writes.iter().try_fold(0usize, |total, write| {
        total
            .checked_add(write.source.len())
            .ok_or_else(|| error("QUEST development resident source byte 합계 overflow"))
    })?;
    let output_bytes = writer.writes.iter().try_fold(0usize, |total, write| {
        total
            .checked_add(write.output.len())
            .ok_or_else(|| error("QUEST development resident output byte 합계 overflow"))
    })?;
    let glyph_cells = writer.writes.iter().try_fold(0usize, |total, write| {
        total
            .checked_add(write.glyph_cells)
            .ok_or_else(|| error("QUEST development resident glyph cell 합계 overflow"))
    })?;
    if writer.writes.is_empty()
        || writer.units != writer.writes.len()
        || writer.glyph_cells != glyph_cells
        || source_bytes != output_bytes
        || writer.status != QuestResidentTextCompositionStatus::ManualDraftNeedsGeometry
        || !writer.source_exact
        || !writer.controls_exact
        || !writer.terminal_exact
        || writer.japanese_glyphs_remaining != 0
        || writer.translation_approved
        || writer.layout_approved
        || writer.runtime_verified
        || writer.product_eligible
    {
        return Err(error("QUEST development resident writer 입력 계약 drift"));
    }
    Ok(QuestDevelopmentDiscResidentTextReport {
        writes: writer.writes.clone(),
        units: writer.units,
        glyph_cells,
        source_bytes,
        output_bytes,
        status: writer.status,
        source_exact: writer.source_exact,
        controls_exact: writer.controls_exact,
        terminal_exact: writer.terminal_exact,
        final_exact: false,
        japanese_glyphs_remaining: writer.japanese_glyphs_remaining,
        translation_approved: writer.translation_approved,
        layout_approved: writer.layout_approved,
        runtime_verified: writer.runtime_verified,
        product_eligible: writer.product_eligible,
    })
}

pub(super) fn prepare_item_name_report(
    item: &QuestItemTextWriterAnalysis,
) -> Result<QuestDevelopmentDiscItemNameReport, QuestDevelopmentDiscError> {
    verify_quest_item_text_writer_provenance(item).map_err(|source| error(source.to_string()))?;
    let writer = &item.name;
    verify_quest_item_name_writer_provenance(writer).map_err(|source| error(source.to_string()))?;
    let draft_needs_independent_review = writer
        .writes
        .iter()
        .filter(|write| write.status == QuestItemNameDraftStatus::DraftNeedsIndependentReview)
        .count();
    let draft_needs_human_choice = writer
        .writes
        .iter()
        .filter(|write| write.status == QuestItemNameDraftStatus::DraftNeedsHumanChoice)
        .count();
    if writer.page_id == 0
        || writer.writes.is_empty()
        || writer.name_units != writer.writes.len()
        || writer.pointer_writes != writer.name_units + writer.description_units
        || item.relink.control.name_units != writer.name_units
        || item.relink.control.description_units != writer.description_units
        || item.relink.control.output_bytes != item.relink.control.output.len()
        || item.source_sequence_sha256.len() != 64
        || item.output_sequence_sha256 != sha256(&item.relink.control.output)
        || !writer.source_exact
        || !writer.pointer_readback_exact
        || !writer.nul_terminal_exact
        || !item.controls_exact
        || !item.terminal_exact
        || !writer.source_exact_outside_relinked_fields
        || writer.japanese_glyphs_remaining != 0
        || writer.translation_approved
        || writer.layout_approved
        || writer.runtime_verified
        || writer.product_eligible
        || draft_needs_independent_review + draft_needs_human_choice != writer.name_units
    {
        return Err(error("QUEST item name writer 수동 개발 입력 계약 drift"));
    }
    let relocated_names = item
        .relink
        .control
        .relocated_units
        .iter()
        .filter(|unit| unit.role == QuestItemTextRole::Name)
        .map(|unit| (unit.id.as_str(), unit))
        .collect::<BTreeMap<_, _>>();
    let writes = writer
        .writes
        .iter()
        .map(|write| {
            let relocated = relocated_names.get(write.unit_id.as_str()).ok_or_else(|| {
                error(format!(
                    "QUEST item name final relink unit 누락: {}",
                    write.unit_id
                ))
            })?;
            let mut write = write.clone();
            write.relocated_text_offset = relocated.relocated_text_offset;
            write.relocated_pointer = relocated.relocated_pointer;
            Ok(write)
        })
        .collect::<Result<Vec<_>, QuestDevelopmentDiscError>>()?;
    Ok(QuestDevelopmentDiscItemNameReport {
        writes,
        page_id: writer.page_id,
        units: writer.name_units,
        glyph_cells: writer.glyph_cells,
        source_name_bytes: writer.source_name_bytes,
        output_name_bytes: writer.output_name_bytes,
        source_sequence_bytes: item.relink.control.source_bytes,
        output_sequence_bytes: item.relink.control.output_bytes,
        source_sequence_sha256: item.source_sequence_sha256.clone(),
        output_sequence_sha256: item.output_sequence_sha256.clone(),
        active_window_remaining: item.active_window_remaining,
        draft_needs_independent_review,
        draft_needs_human_choice,
        source_exact: writer.source_exact,
        pointer_readback_exact: writer.pointer_readback_exact,
        nul_terminal_exact: writer.nul_terminal_exact,
        combined_relink_exact: true,
        final_exact: false,
        japanese_glyphs_remaining: writer.japanese_glyphs_remaining,
        translation_approved: writer.translation_approved,
        layout_approved: writer.layout_approved,
        runtime_verified: writer.runtime_verified,
        product_eligible: writer.product_eligible,
    })
}

pub(super) fn prepare_item_description_report(
    writer: &QuestItemTextWriterAnalysis,
) -> Result<QuestDevelopmentDiscItemDescriptionReport, QuestDevelopmentDiscError> {
    verify_quest_item_text_writer_provenance(writer).map_err(|source| error(source.to_string()))?;
    if writer.description_page_ids.is_empty()
        || writer.description_page_ids.contains(&0)
        || writer.description_writes.is_empty()
        || writer.description_units != writer.description_writes.len()
        || writer.description_glyph_cells
            != writer
                .description_writes
                .iter()
                .map(|write| write.glyph_cells)
                .sum::<usize>()
        || writer.description_controls
            != writer
                .description_writes
                .iter()
                .map(|write| write.control_occurrences)
                .sum::<usize>()
        || !writer.source_exact
        || !writer.pointer_readback_exact
        || !writer.controls_exact
        || !writer.terminal_exact
        || !writer.source_exact_outside_relinked_fields
        || writer.japanese_glyphs_remaining != 0
        || writer.translation_approved
        || writer.layout_approved
        || writer.runtime_verified
        || writer.product_eligible
        || writer.draft_needs_independent_review + writer.draft_needs_human_choice
            != writer.description_units
    {
        return Err(error(
            "QUEST item description writer 수동 개발 입력 계약 drift",
        ));
    }
    Ok(QuestDevelopmentDiscItemDescriptionReport {
        writes: writer.description_writes.clone(),
        page_ids: writer.description_page_ids.clone(),
        units: writer.description_units,
        glyph_cells: writer.description_glyph_cells,
        controls: writer.description_controls,
        source_description_bytes: writer.source_description_bytes,
        output_description_bytes: writer.output_description_bytes,
        active_window_remaining: writer.active_window_remaining,
        draft_needs_independent_review: writer.draft_needs_independent_review,
        draft_needs_human_choice: writer.draft_needs_human_choice,
        source_exact: writer.source_exact,
        pointer_readback_exact: writer.pointer_readback_exact,
        controls_exact: writer.controls_exact,
        terminal_exact: writer.terminal_exact,
        final_exact: false,
        japanese_glyphs_remaining: writer.japanese_glyphs_remaining,
        translation_approved: writer.translation_approved,
        layout_approved: writer.layout_approved,
        runtime_verified: writer.runtime_verified,
        product_eligible: writer.product_eligible,
    })
}
