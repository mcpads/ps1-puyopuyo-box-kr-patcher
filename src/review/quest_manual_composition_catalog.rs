//! version 관리된 QUEST 수동 composition의 전체 build scope를 조립한다.
//!
//! composition 원장에 실제로 등장하는 batch만 검토 자료와 원문 stream에
//! 결속한다. 문장 후보나 독립 검토안은 이 경로에서 새 선택으로 만들지 않는다.

use std::error::Error;
use std::fmt;

use crate::script::{
    QuestSeqExtractedTextStream, QuestTranslationApprovalAnalysis, QuestTranslationDecisionWorkbook,
};

use super::quest_manual_composition::{
    audit_quest_manual_composition_catalog_inputs, quest_manual_composition_batch_ids,
};
use super::{
    build_quest_review_queue_source_layouts, build_quest_translation_review_queue,
    QuestManualCompositionAnalysis,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestManualCompositionCatalogError(String);

impl fmt::Display for QuestManualCompositionCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestManualCompositionCatalogError {}

fn error(message: impl Into<String>) -> QuestManualCompositionCatalogError {
    QuestManualCompositionCatalogError(message.into())
}

/// 원장에 사람이 작성한 모든 batch를 현재 검토·원문 기준선에 결속한다.
pub fn audit_quest_manual_composition_catalog(
    workbook: &QuestTranslationDecisionWorkbook,
    approvals: &QuestTranslationApprovalAnalysis,
    streams: &[QuestSeqExtractedTextStream],
) -> Result<QuestManualCompositionAnalysis, QuestManualCompositionCatalogError> {
    let batch_ids = quest_manual_composition_batch_ids().map_err(|source| {
        error(format!(
            "QUEST 수동 composition batch 목록 검증 실패: {source}"
        ))
    })?;
    let mut queues = Vec::with_capacity(batch_ids.len());
    let mut source_layouts = Vec::new();
    for batch_id in batch_ids {
        let queue = build_quest_translation_review_queue(workbook, approvals, &batch_id).map_err(
            |source| {
                error(format!(
                    "QUEST 수동 composition 검토 batch 조립 실패 ({batch_id}): {source}"
                ))
            },
        )?;
        let layouts =
            build_quest_review_queue_source_layouts(&queue, streams).map_err(|source| {
                error(format!(
                    "QUEST 수동 composition 원문 표시 구조 조립 실패 ({batch_id}): {source}"
                ))
            })?;
        source_layouts.extend(layouts);
        queues.push(queue);
    }
    audit_quest_manual_composition_catalog_inputs(&queues, &source_layouts, streams).map_err(
        |source| {
            error(format!(
                "QUEST 수동 composition 전체 원장 검증 실패: {source}"
            ))
        },
    )
}
