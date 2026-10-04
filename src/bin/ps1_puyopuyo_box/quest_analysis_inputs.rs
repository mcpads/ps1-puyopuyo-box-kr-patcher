//! QUEST text 분석 보고서가 공유하는 원본 재구성 입력.

use std::path::Path;

use ps1_puyopuyo_box::analysis::{
    verify_box_quest_seq_flag_branches, verify_box_quest_seq_resource_transitions,
    verify_box_quest_seq_text_frontiers, verify_box_quest_seq_text_paths,
    QuestSeqFlagBranchAnalysis, QuestSeqResourceCatalogLayout, QuestSeqResourceTransitionAnalysis,
    QuestSeqTextFrontierAnalysis, QuestSeqTextPathAnalysis,
};
use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::compress::{decompress, is_cnx};
use ps1_puyopuyo_box::disasm::{
    verify_box_quest_seq_base_compares, verify_box_quest_seq_base_direct_advances,
    verify_box_quest_seq_base_record_advances, verify_box_quest_seq_base_transitions,
    verify_box_quest_seq_direct_advances, verify_box_quest_seq_flag_query,
    verify_box_quest_seq_op28, verify_box_quest_seq_pointer_consumer,
    verify_box_quest_seq_registration,
};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::font::QuestCodeTable;
use ps1_puyopuyo_box::pipeline::{
    verify_box_quest_glyph_cache_hook_plan, verify_box_quest_glyph_cache_package,
    verify_box_quest_translation_routing, verify_box_quest_translation_selection_routing,
    QuestGlyphCacheHookPlan, QuestGlyphCachePackage, QuestTranslationRoutingAnalysis,
    QuestTranslationSelectionRoutingAnalysis,
};
use ps1_puyopuyo_box::script::{
    audit_quest_seq_addresses, verify_box_quest_item_text_units, verify_box_quest_seq_addresses,
    verify_box_quest_seq_text_candidates, verify_box_quest_seq_text_documents,
    verify_box_quest_seq_text_extraction, verify_box_quest_seq_text_roots,
    verify_box_quest_seq_text_streams, verify_box_quest_seq_translation_units,
    verify_box_quest_terminology_candidates, verify_box_quest_translation_batches,
    verify_box_quest_translation_decision_workbook, verify_box_quest_translation_reviews,
    verify_box_quest_translation_selection, QuestItemTextUnitAnalysis, QuestSeqAddressAnalysis,
    QuestSeqTextCandidateAnalysis, QuestSeqTextDocumentAnalysis, QuestSeqTextExtractionAnalysis,
    QuestSeqTextRootAnalysis, QuestSeqTextStreamAnalysis, QuestTranslationBatchAnalysis,
    QuestTranslationDecisionWorkbook, QuestTranslationSelectionAnalysis, QUEST_ITEM_TEXT_SEQUENCE,
};

use super::analysis_inputs::read_quest_seq_sources;
use super::disc_commands::read_box_executables;
use super::{quest_code_table_from_executable, RPG_SEQ_PATH};

pub(super) struct QuestGlyphCacheControlInputs {
    pub batches: QuestTranslationBatchAnalysis,
    pub routing: QuestTranslationRoutingAnalysis,
    pub package: QuestGlyphCachePackage,
    pub hook: QuestGlyphCacheHookPlan,
}

/// local 검토 화면이 원본에서 다시 만든 추출 결과와 전역 결정 자료.
pub(super) struct QuestReviewInputs {
    pub extraction: QuestSeqTextExtractionAnalysis,
    pub workbook: QuestTranslationDecisionWorkbook,
}

/// 승인 UI가 PoC 산출물이 아니라 지원 원본과 version 관리 입력에서 재구성되게 한다.
pub(super) fn read_quest_review_inputs(source_path: &Path) -> Result<QuestReviewInputs, String> {
    let extraction = read_quest_seq_text_extraction_analysis(source_path)?;
    let units =
        verify_box_quest_seq_translation_units(&extraction).map_err(|error| error.to_string())?;
    let terminology =
        verify_box_quest_terminology_candidates(&units).map_err(|error| error.to_string())?;
    let batches = verify_box_quest_translation_batches(&extraction, &units, &terminology)
        .map_err(|error| error.to_string())?;
    let reviews =
        verify_box_quest_translation_reviews(&batches).map_err(|error| error.to_string())?;
    let workbook = verify_box_quest_translation_decision_workbook(&reviews, &terminology)
        .map_err(|error| error.to_string())?;
    Ok(QuestReviewInputs {
        extraction,
        workbook,
    })
}

/// sparse cache의 build와 runtime 검증이 공유하는 지원 원본 분석을 재구성한다.
pub(super) fn read_quest_glyph_cache_control_inputs(
    source: &VerifiedSourceImage,
) -> Result<QuestGlyphCacheControlInputs, String> {
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let code_table = quest_code_table_from_executable(quest.map, &quest.text)?;
    let seq_vdd = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|error| error.to_string())?;
    let seq_archive = VddArchive::parse(&seq_vdd).map_err(|error| error.to_string())?;
    let sources = read_quest_seq_sources(&seq_archive)?;
    let addresses = audit_quest_seq_addresses(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let extraction = verify_box_quest_seq_text_extraction(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &code_table,
    )
    .map_err(|error| error.to_string())?;
    let documents = verify_box_quest_seq_text_documents(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &extraction,
    )
    .map_err(|error| error.to_string())?;
    let units =
        verify_box_quest_seq_translation_units(&extraction).map_err(|error| error.to_string())?;
    let terminology =
        verify_box_quest_terminology_candidates(&units).map_err(|error| error.to_string())?;
    let batches = verify_box_quest_translation_batches(&extraction, &units, &terminology)
        .map_err(|error| error.to_string())?;
    let routing = verify_box_quest_translation_routing(&documents, &addresses, &batches)
        .map_err(|error| error.to_string())?;
    let package = verify_box_quest_glyph_cache_package(source, &batches, &routing)
        .map_err(|error| error.to_string())?;
    let hook = verify_box_quest_glyph_cache_hook_plan(quest.map, &quest.text, &package, &routing)
        .map_err(|error| error.to_string())?;
    Ok(QuestGlyphCacheControlInputs {
        batches,
        routing,
        package,
        hook,
    })
}

pub(super) fn read_quest_seq_sources_from_disc(
    source_path: &Path,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let vdd_raw = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&vdd_raw)
        .map_err(|error| format!("{RPG_SEQ_PATH} VDD 파싱 실패: {error}"))?;
    read_quest_seq_sources(&archive)
}

pub(super) fn read_quest_item_text_analysis(
    source_path: &Path,
) -> Result<QuestItemTextUnitAnalysis, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let code_table = quest_code_table_from_executable(quest.map, &quest.text)?;
    let vdd_raw = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&vdd_raw)
        .map_err(|error| format!("{RPG_SEQ_PATH} VDD 파싱 실패: {error}"))?;
    let entry = archive
        .find(QUEST_ITEM_TEXT_SEQUENCE)
        .ok_or_else(|| format!("{RPG_SEQ_PATH}:{QUEST_ITEM_TEXT_SEQUENCE} 엔트리가 없음"))?;
    let compressed = archive.read_file(entry);
    if !is_cnx(compressed) {
        return Err(format!(
            "{RPG_SEQ_PATH}:{QUEST_ITEM_TEXT_SEQUENCE}가 CNX v2가 아님"
        ));
    }
    let raw = decompress(compressed)
        .map_err(|error| format!("{RPG_SEQ_PATH}:{QUEST_ITEM_TEXT_SEQUENCE}: {error}"))?;
    verify_box_quest_item_text_units(&raw, &code_table).map_err(|error| error.to_string())
}

pub(super) fn read_quest_seq_text_analysis(
    source_path: &Path,
) -> Result<QuestSeqTextCandidateAnalysis, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest_executable = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let code_table =
        quest_code_table_from_executable(quest_executable.map, &quest_executable.text)?;
    let vdd_raw = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&vdd_raw)
        .map_err(|error| format!("{RPG_SEQ_PATH} VDD 파싱 실패: {error}"))?;
    let sources = read_quest_seq_sources(&archive)?;
    verify_box_quest_seq_text_candidates(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &code_table,
    )
    .map_err(|error| error.to_string())
}

pub(super) fn read_quest_seq_text_stream_analysis(
    source_path: &Path,
) -> Result<QuestSeqTextStreamAnalysis, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest_executable = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let code_table =
        quest_code_table_from_executable(quest_executable.map, &quest_executable.text)?;
    let vdd_raw = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&vdd_raw)
        .map_err(|error| format!("{RPG_SEQ_PATH} VDD 파싱 실패: {error}"))?;
    let sources = read_quest_seq_sources(&archive)?;
    verify_box_quest_seq_text_streams(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &code_table,
    )
    .map_err(|error| error.to_string())
}

pub(super) fn read_quest_seq_text_extraction_analysis(
    source_path: &Path,
) -> Result<QuestSeqTextExtractionAnalysis, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest_executable = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let code_table =
        quest_code_table_from_executable(quest_executable.map, &quest_executable.text)?;
    let sources = read_quest_seq_sources_from_disc(source_path)?;
    verify_box_quest_seq_text_extraction(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &code_table,
    )
    .map_err(|error| error.to_string())
}

pub(super) fn read_quest_seq_text_document_analysis(
    source_path: &Path,
) -> Result<QuestSeqTextDocumentAnalysis, String> {
    let sources = read_quest_seq_sources_from_disc(source_path)?;
    let extraction = read_quest_seq_text_extraction_analysis(source_path)?;
    verify_box_quest_seq_text_documents(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &extraction,
    )
    .map_err(|error| error.to_string())
}

pub(super) struct QuestTranslationSelectionInputs {
    pub addresses: QuestSeqAddressAnalysis,
    pub documents: QuestSeqTextDocumentAnalysis,
    pub code_table: QuestCodeTable,
    pub seq_sources: Vec<(String, Vec<u8>)>,
    pub selection: QuestTranslationSelectionAnalysis,
    pub source_routing: QuestTranslationRoutingAnalysis,
    pub routing: QuestTranslationSelectionRoutingAnalysis,
}

/// 검토 선택과 원본 QUEST SEQ route의 공통 보고서 입력을 재구성한다.
pub(super) fn read_quest_translation_selection_inputs(
    source_path: &Path,
) -> Result<QuestTranslationSelectionInputs, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let code_table = quest_code_table_from_executable(quest.map, &quest.text)?;
    let sources = read_quest_seq_sources_from_disc(source_path)?;
    let addresses = audit_quest_seq_addresses(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    verify_box_quest_seq_addresses(&addresses).map_err(|error| error.to_string())?;
    let extraction = verify_box_quest_seq_text_extraction(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &code_table,
    )
    .map_err(|error| error.to_string())?;
    let documents = verify_box_quest_seq_text_documents(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &extraction,
    )
    .map_err(|error| error.to_string())?;
    let units =
        verify_box_quest_seq_translation_units(&extraction).map_err(|error| error.to_string())?;
    let terminology =
        verify_box_quest_terminology_candidates(&units).map_err(|error| error.to_string())?;
    let batches = verify_box_quest_translation_batches(&extraction, &units, &terminology)
        .map_err(|error| error.to_string())?;
    let reviews =
        verify_box_quest_translation_reviews(&batches).map_err(|error| error.to_string())?;
    let selection =
        verify_box_quest_translation_selection(&reviews).map_err(|error| error.to_string())?;
    let source_routing = verify_box_quest_translation_routing(&documents, &addresses, &batches)
        .map_err(|error| error.to_string())?;
    let routing =
        verify_box_quest_translation_selection_routing(&documents, &source_routing, &selection)
            .map_err(|error| error.to_string())?;
    Ok(QuestTranslationSelectionInputs {
        addresses,
        documents,
        code_table,
        seq_sources: sources,
        selection,
        source_routing,
        routing,
    })
}

/// 검토 선택을 원본 QUEST SEQ route에 결속한 보고서 입력을 재구성한다.
pub(super) fn read_quest_translation_selection_routing_analysis(
    source_path: &Path,
) -> Result<QuestTranslationSelectionRoutingAnalysis, String> {
    Ok(read_quest_translation_selection_inputs(source_path)?.routing)
}

pub(super) fn read_quest_seq_text_root_analysis(
    source_path: &Path,
) -> Result<QuestSeqTextRootAnalysis, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest_executable = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let code_table =
        quest_code_table_from_executable(quest_executable.map, &quest_executable.text)?;
    let vdd_raw = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&vdd_raw)
        .map_err(|error| format!("{RPG_SEQ_PATH} VDD 파싱 실패: {error}"))?;
    let sources = read_quest_seq_sources(&archive)?;
    verify_box_quest_seq_text_roots(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &code_table,
    )
    .map_err(|error| error.to_string())
}

pub(super) fn read_quest_seq_text_path_analysis(
    source_path: &Path,
) -> Result<QuestSeqTextPathAnalysis, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest_executable = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    verify_box_quest_seq_base_direct_advances(quest_executable.map, &quest_executable.text)
        .map_err(|error| error.to_string())?;
    verify_box_quest_seq_base_record_advances(quest_executable.map, &quest_executable.text)
        .map_err(|error| error.to_string())?;
    verify_box_quest_seq_base_compares(quest_executable.map, &quest_executable.text)
        .map_err(|error| error.to_string())?;
    verify_box_quest_seq_direct_advances(quest_executable.map, &quest_executable.text)
        .map_err(|error| error.to_string())?;
    verify_box_quest_seq_base_transitions(quest_executable.map, &quest_executable.text)
        .map_err(|error| error.to_string())?;
    verify_box_quest_seq_op28(quest_executable.map, &quest_executable.text)
        .map_err(|error| error.to_string())?;
    let code_table =
        quest_code_table_from_executable(quest_executable.map, &quest_executable.text)?;
    let vdd_raw = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&vdd_raw)
        .map_err(|error| format!("{RPG_SEQ_PATH} VDD 파싱 실패: {error}"))?;
    let sources = read_quest_seq_sources(&archive)?;
    verify_box_quest_seq_text_paths(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &code_table,
    )
    .map_err(|error| error.to_string())
}

pub(super) fn read_quest_seq_text_frontier_analysis(
    source_path: &Path,
) -> Result<QuestSeqTextFrontierAnalysis, String> {
    let paths = read_quest_seq_text_path_analysis(source_path)?;
    let sources = read_quest_seq_sources_from_disc(source_path)?;
    verify_box_quest_seq_text_frontiers(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &paths,
    )
    .map_err(|error| error.to_string())
}

pub(super) fn read_quest_seq_flag_branch_analysis(
    source_path: &Path,
) -> Result<QuestSeqFlagBranchAnalysis, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    verify_box_quest_seq_flag_query(quest.map, &quest.text).map_err(|error| error.to_string())?;
    verify_box_quest_seq_pointer_consumer(quest.map, &quest.text)
        .map_err(|error| error.to_string())?;

    let sources = read_quest_seq_sources_from_disc(source_path)?;
    let addresses = audit_quest_seq_addresses(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    verify_box_quest_seq_addresses(&addresses).map_err(|error| error.to_string())?;
    verify_box_quest_seq_flag_branches(
        sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &addresses,
    )
    .map_err(|error| error.to_string())
}

pub(super) fn read_quest_seq_resource_transition_analysis(
    source_path: &Path,
) -> Result<QuestSeqResourceTransitionAnalysis, String> {
    let frontiers = read_quest_seq_text_frontier_analysis(source_path)?;
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let registration = verify_box_quest_seq_registration(quest.map, &quest.text)
        .map_err(|error| error.to_string())?;
    let sources = read_quest_seq_sources_from_disc(source_path)?;
    verify_box_quest_seq_resource_transitions(
        sources.iter().map(|(name, _)| name.as_str()),
        QuestSeqResourceCatalogLayout {
            descriptor_root_runtime: registration.descriptor_root_runtime,
            descriptor_stride: registration.descriptor_stride,
            prefix_descriptor_rows: registration.prefix_descriptor_rows,
        },
        &frontiers,
    )
    .map_err(|error| error.to_string())
}
