//! 사람이 직접 작성한 QUEST 번역문과 control anchor 배치를 검증한다.
//!
//! 이 모듈은 공개 경계만 소유한다. `ledger`는 version 관리 TSV의 구문을,
//! `layout_validation`은 한 문장의 수동 배치를, `source_binding`은 검토 queue와
//! 지원 원본 모집단의 결속을 맡는다. 어느 경로도 번역문이나 줄바꿈을 만들지
//! 않는다.

mod layout_validation;
mod ledger;
mod model;
mod source_binding;

use crate::script::QuestSeqExtractedTextStream;

use self::ledger::{
    read_versioned_quest_manual_composition_ledger, versioned_quest_manual_composition_batch_ids,
};
use self::source_binding::audit_quest_manual_composition_source_bindings;
use super::{QuestReviewQueue, QuestSourceLayout};

pub use self::model::{
    QuestCompositionCandidateBasis, QuestManualComposition, QuestManualCompositionAnalysis,
    QuestManualCompositionError, QuestManualCompositionSegment, QuestManualCompositionStatus,
    QUEST_MANUAL_COMPOSITION_SCHEMA_VERSION, QUEST_MANUAL_COMPOSITION_SEGMENT_SCHEMA_VERSION,
};

pub(super) fn quest_manual_composition_batch_ids(
) -> Result<Vec<String>, QuestManualCompositionError> {
    versioned_quest_manual_composition_batch_ids()
}

/// version 관리 수동 원장을 현재 검토 batch와 지원 원본에서 다시 만든 stream에 대조한다.
pub fn audit_quest_manual_compositions(
    queue: &QuestReviewQueue,
    source_layouts: &[QuestSourceLayout],
    streams: &[QuestSeqExtractedTextStream],
) -> Result<QuestManualCompositionAnalysis, QuestManualCompositionError> {
    let ledger = read_versioned_quest_manual_composition_ledger()?;
    audit_quest_manual_composition_source_bindings(
        std::slice::from_ref(queue),
        source_layouts,
        streams,
        ledger,
        false,
    )
}

pub(super) fn audit_quest_manual_composition_catalog_inputs(
    queues: &[QuestReviewQueue],
    source_layouts: &[QuestSourceLayout],
    streams: &[QuestSeqExtractedTextStream],
) -> Result<QuestManualCompositionAnalysis, QuestManualCompositionError> {
    let ledger = read_versioned_quest_manual_composition_ledger()?;
    audit_quest_manual_composition_source_bindings(queues, source_layouts, streams, ledger, true)
}

#[cfg(test)]
#[path = "quest_manual_composition_tests.rs"]
mod tests;
