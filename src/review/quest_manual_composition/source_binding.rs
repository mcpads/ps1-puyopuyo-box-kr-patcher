//! 수동 composition 원장을 검토 queue와 지원 원본 stream 모집단에 결속한다.

use std::collections::{BTreeMap, BTreeSet};

use crate::script::QuestSeqExtractedTextStream;

use super::layout_validation::validate_quest_manual_composition_layout;
use super::ledger::{canonical_source_punctuation, contains_hangul, QuestManualCompositionLedger};
use super::model::{
    composition_error, QuestManualCompositionAnalysis, QuestManualCompositionError,
    QuestManualCompositionSegment,
};
use crate::review::{QuestReviewQueue, QuestSourceLayout};

pub(super) fn audit_quest_manual_composition_source_bindings(
    queues: &[QuestReviewQueue],
    source_layouts: &[QuestSourceLayout],
    streams: &[QuestSeqExtractedTextStream],
    ledger: QuestManualCompositionLedger,
    require_complete_catalog: bool,
) -> Result<QuestManualCompositionAnalysis, QuestManualCompositionError> {
    if queues.is_empty() {
        return Err(composition_error(
            "QUEST 수동 composition 검토 대기열이 비어 있음",
        ));
    }
    let queue_entries = queues
        .iter()
        .flat_map(|queue| queue.entries.iter())
        .collect::<Vec<_>>();
    if queue_entries.len() != source_layouts.len()
        || queue_entries
            .iter()
            .zip(source_layouts)
            .any(|(entry, layout)| entry.stream_id != layout.stream_id)
    {
        return Err(composition_error(
            "QUEST 검토 대기열과 수동 composition 원문 분모 불일치",
        ));
    }
    let mut queue_batch_ids = BTreeSet::new();
    for queue in queues {
        if !queue_batch_ids.insert(queue.batch_id.as_str()) {
            return Err(composition_error(format!(
                "QUEST 수동 composition 검토 batch 중복: {}",
                queue.batch_id
            )));
        }
        if queue
            .entries
            .iter()
            .any(|entry| entry.batch_id != queue.batch_id)
        {
            return Err(composition_error(format!(
                "QUEST 수동 composition 검토 batch와 entry 불일치: {}",
                queue.batch_id
            )));
        }
    }

    let QuestManualCompositionLedger {
        compositions: all_rows,
        segments: segment_rows,
    } = ledger;
    let mut segments_by_id = BTreeMap::<String, Vec<QuestManualCompositionSegment>>::new();
    for segment_row in segment_rows {
        segments_by_id
            .entry(segment_row.composition_id)
            .or_default()
            .push(segment_row.segment);
    }
    let all_row_ids = all_rows
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    if segments_by_id
        .keys()
        .any(|id| !all_row_ids.contains(id.as_str()))
    {
        return Err(composition_error(
            "QUEST 수동 composition 없는 segment가 있음",
        ));
    }

    let all_row_batch_ids = all_rows
        .iter()
        .map(|row| row.batch_id.as_str())
        .collect::<BTreeSet<_>>();
    if require_complete_catalog && all_row_batch_ids != queue_batch_ids {
        return Err(composition_error(
            "QUEST 수동 composition 원장과 검토 batch 모집단 불일치",
        ));
    }
    let rows = all_rows
        .into_iter()
        .filter(|row| queue_batch_ids.contains(row.batch_id.as_str()))
        .collect::<Vec<_>>();
    let selected_row_ids = rows
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    segments_by_id.retain(|id, _| selected_row_ids.contains(id.as_str()));

    let mut entries_by_stream = BTreeMap::new();
    for entry in &queue_entries {
        if entries_by_stream
            .insert(entry.stream_id.as_str(), *entry)
            .is_some()
        {
            return Err(composition_error(format!(
                "QUEST 수동 composition 검토 stream 중복: {}",
                entry.stream_id
            )));
        }
    }
    let mut layouts_by_stream = BTreeMap::new();
    for layout in source_layouts {
        if layouts_by_stream
            .insert(layout.stream_id.as_str(), layout)
            .is_some()
        {
            return Err(composition_error(format!(
                "QUEST 수동 composition 원문 표시 구조 중복: {}",
                layout.stream_id
            )));
        }
    }
    let mut streams_by_id = BTreeMap::new();
    for stream in streams {
        let stream_id = stream.id();
        if streams_by_id.insert(stream_id.clone(), stream).is_some() {
            return Err(composition_error(format!(
                "QUEST 수동 composition 원문 추출 stream 중복: {stream_id}"
            )));
        }
    }

    let mut compositions = Vec::with_capacity(rows.len());
    for row in rows {
        let entry = entries_by_stream
            .get(row.stream_id.as_str())
            .ok_or_else(|| composition_error(format!("{}: 검토 대기열에 없는 stream", row.id)))?;
        if entry.batch_id != row.batch_id {
            return Err(composition_error(format!("{}: batch ID 불일치", row.id)));
        }
        let source_layout = layouts_by_stream
            .get(row.stream_id.as_str())
            .ok_or_else(|| composition_error(format!("{}: 원문 표시 구조 없음", row.id)))?;
        if source_layout.source_stream_sha256 != row.source_stream_sha256 {
            return Err(composition_error(format!(
                "{}: source stream SHA-256 drift",
                row.id
            )));
        }
        if !contains_hangul(&row.selected_korean) {
            let source_text = source_layout
                .spans
                .iter()
                .map(|span| span.protected_source.as_str())
                .collect::<String>();
            let candidate = canonical_source_punctuation(&row.selected_korean);
            if candidate.is_none() || candidate != canonical_source_punctuation(&source_text) {
                return Err(composition_error(format!(
                    "{}: 한글 없는 선택문은 원문 구두점과 같아야 함",
                    row.id
                )));
            }
        }
        let stream = streams_by_id
            .get(row.stream_id.as_str())
            .ok_or_else(|| composition_error(format!("{}: 원문 추출 stream 없음", row.id)))?;
        let segments = segments_by_id
            .remove(&row.id)
            .ok_or_else(|| composition_error(format!("{}: 수동 segment 없음", row.id)))?;
        compositions.push(validate_quest_manual_composition_layout(
            &row,
            segments,
            source_layout,
            stream,
        )?);
    }
    if !segments_by_id.is_empty() {
        return Err(composition_error(
            "QUEST 수동 composition segment 소비 누락",
        ));
    }
    // batch는 장면 문맥과 검토 dependency를 묶는 단위다. 같은 batch 안에서도
    // 사람 결정이 끝난 stream만 먼저 수동 조판할 수 있어야 한다. 원장에 없는
    // stream은 development selection이 원문으로 유지하며, 위 결속 검사는 원장에
    // 들어온 모든 composition이 실제 batch·stream에 속하는지만 엄격히 확인한다.
    compositions.sort_by_key(|composition| {
        queue_entries
            .iter()
            .position(|entry| entry.stream_id == composition.stream_id)
            .expect("수동 composition은 검토 대기열에 결속됨")
    });

    let geometry_verified = compositions
        .iter()
        .filter(|composition| composition.geometry_verified)
        .count();
    let layout_approved = compositions
        .iter()
        .filter(|composition| composition.layout_approved)
        .count();
    let product_eligible = compositions
        .iter()
        .filter(|composition| {
            entries_by_stream
                .get(composition.stream_id.as_str())
                .is_some_and(|entry| entry.approval_build_eligible)
                && composition.geometry_verified
                && composition.layout_approved
        })
        .count();
    Ok(QuestManualCompositionAnalysis {
        manually_authored: compositions.len(),
        geometry_verified,
        layout_approved,
        product_eligible,
        compositions,
    })
}
