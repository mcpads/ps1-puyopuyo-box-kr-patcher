//! 검증된 QUEST 수동 composition을 development selection과 page에 결속한다.

use ps1_puyopuyo_box::font::QuestCodeTable;
use ps1_puyopuyo_box::pipeline::{
    pack_quest_development_translation_pages,
    verify_box_quest_translation_selection_routing_with_layouts,
    QuestDevelopmentPagePackingAnalysis, QuestManualStreamLayout, QuestTranslationRoutingAnalysis,
    QuestTranslationSelectionRoutingAnalysis,
};
use ps1_puyopuyo_box::review::QuestManualCompositionAnalysis;
use ps1_puyopuyo_box::script::{
    select_quest_manual_compositions_for_development, QuestManualCompositionSelection,
    QuestSeqTextDocumentAnalysis, QuestTranslationSelectionAnalysis,
};

pub(super) struct QuestManualCompositionDevelopmentInput<'a> {
    pub manual_compositions: &'a QuestManualCompositionAnalysis,
    pub source_selection: &'a QuestTranslationSelectionAnalysis,
    pub text_documents: &'a QuestSeqTextDocumentAnalysis,
    pub source_routing: &'a QuestTranslationRoutingAnalysis,
    pub code_table: &'a QuestCodeTable,
}

pub(super) struct QuestManualCompositionDevelopment {
    pub selection: QuestTranslationSelectionAnalysis,
    pub routing: QuestTranslationSelectionRoutingAnalysis,
    pub layouts: Vec<QuestManualStreamLayout>,
    pub page_packing: QuestDevelopmentPagePackingAnalysis,
}

/// 사람이 작성한 선택·run만 development routing과 결정적 page packing에 전달한다.
pub(super) fn build_quest_manual_composition_development(
    input: QuestManualCompositionDevelopmentInput<'_>,
) -> Result<QuestManualCompositionDevelopment, String> {
    let manual_selections = input
        .manual_compositions
        .compositions
        .iter()
        .map(|composition| QuestManualCompositionSelection {
            stream_id: composition.stream_id.clone(),
            batch_id: composition.batch_id.clone(),
            selected_korean: composition.display_korean.clone(),
        })
        .collect::<Vec<_>>();
    let selection = select_quest_manual_compositions_for_development(
        input.source_selection,
        &manual_selections,
    )
    .map_err(|error| error.to_string())?;
    let layouts = input
        .manual_compositions
        .compositions
        .iter()
        .map(|composition| QuestManualStreamLayout {
            stream_id: composition.stream_id.clone(),
            selected_korean: composition.display_korean.clone(),
            candidate_glyph_run_lengths: composition.candidate_glyph_run_lengths.clone(),
        })
        .collect::<Vec<_>>();
    let routing = verify_box_quest_translation_selection_routing_with_layouts(
        input.text_documents,
        input.source_routing,
        &selection,
        &layouts,
    )
    .map_err(|error| error.to_string())?;
    let page_packing =
        pack_quest_development_translation_pages(&selection, &routing, input.code_table)
            .map_err(|error| error.to_string())?;
    let routing = page_packing.routing.clone();

    Ok(QuestManualCompositionDevelopment {
        selection,
        routing,
        layouts,
        page_packing,
    })
}
