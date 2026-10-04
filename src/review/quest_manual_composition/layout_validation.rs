//! 사람이 작성한 한 문장의 segment와 원문 control 배치를 검증한다.

use std::collections::BTreeMap;

use crate::font::normalize_quest_display_text;
use crate::script::{
    validate_quest_explicit_layout, QuestControlMeaning, QuestSeqExtractedTextStream,
};

use super::ledger::CompositionRow;
use super::model::{
    composition_error, sha256_text, QuestManualComposition, QuestManualCompositionError,
    QuestManualCompositionSegment,
};
use crate::review::QuestSourceLayout;

pub(super) fn validate_quest_manual_composition_layout(
    row: &CompositionRow,
    mut segments: Vec<QuestManualCompositionSegment>,
    source_layout: &QuestSourceLayout,
    stream: &QuestSeqExtractedTextStream,
) -> Result<QuestManualComposition, QuestManualCompositionError> {
    segments.sort_by_key(|segment| segment.order);
    let mut text_cursor = 0usize;
    let mut previous_position = None;
    let mut previous_anchor = None;
    let mut distinct_position = None;
    let mut control_span_by_token = BTreeMap::new();
    let mut display_korean = String::new();
    for (span_index, span) in source_layout.spans.iter().enumerate() {
        if control_span_by_token
            .insert(span.following_control.token_index, span_index)
            .is_some()
        {
            return Err(composition_error(format!(
                "{}: 원문 control token index 중복",
                row.id
            )));
        }
    }
    let mut candidate_runs = vec![0usize; source_layout.spans.len()];

    for (index, segment) in segments.iter().enumerate() {
        if segment.order != index + 1 {
            return Err(composition_error(format!(
                "{}: segment_order 비연속",
                row.id
            )));
        }
        let position = (segment.input_phase, segment.phase_line);
        if previous_position.is_some_and(|previous| position < previous) {
            return Err(composition_error(format!(
                "{}: input phase/phase line 순서가 역전됨",
                row.id
            )));
        }
        if segment.selected_text_start < text_cursor {
            return Err(composition_error(format!(
                "{}: 선택문 text range가 겹치거나 역전됨: cursor={}, actual={}",
                row.id, text_cursor, segment.selected_text_start
            )));
        }
        let skipped = row
            .selected_korean
            .chars()
            .skip(text_cursor)
            .take(segment.selected_text_start - text_cursor)
            .collect::<String>();
        let preserve_inline_space = skipped == " " && previous_position == Some(position);
        if !skipped.is_empty() && (skipped != " " || previous_position.is_none()) {
            return Err(composition_error(format!(
                "{}: 선택문에서 수동 phase line·input phase 경계가 아닌 문자를 생략함",
                row.id
            )));
        }
        let segment_chars = segment.text.chars().count();
        let expected_end = segment
            .selected_text_start
            .checked_add(segment_chars)
            .ok_or_else(|| composition_error(format!("{}: text range overflow", row.id)))?;
        if segment.selected_text_end != expected_end {
            return Err(composition_error(format!(
                "{}: text range 끝 불일치",
                row.id
            )));
        }
        let source_text = row
            .selected_korean
            .chars()
            .skip(segment.selected_text_start)
            .take(segment_chars)
            .collect::<String>();
        if source_text != segment.text {
            return Err(composition_error(format!(
                "{}: segment text가 선택문 range와 다름",
                row.id
            )));
        }
        text_cursor = expected_end;

        let mut display_run = String::with_capacity(segment.text.len() + 1);
        if preserve_inline_space {
            display_run.push(' ');
        }
        display_run.push_str(&segment.text);
        display_korean.push_str(&display_run);

        if let (Some(previous), Some(previous_anchor)) = (previous_position, previous_anchor) {
            let meanings = source_layout
                .spans
                .iter()
                .filter(|span| {
                    span.following_control.token_index >= previous_anchor
                        && span.following_control.token_index
                            < segment.following_control_token_index
                })
                .map(|span| span.following_control.meaning)
                .collect::<Vec<_>>();
            let crosses_input_phase = meanings.contains(&QuestControlMeaning::StartInputWait);
            let advances_phase_line = meanings.contains(&QuestControlMeaning::AdvanceTextLine);
            match (previous, position) {
                (previous, current) if current == previous => {
                    if crosses_input_phase || advances_phase_line {
                        return Err(composition_error(format!(
                            "{}: 같은 input phase/phase line이 원문 입력 대기·줄 전환을 가로지름",
                            row.id
                        )));
                    }
                }
                ((previous_phase, previous_line), (current_phase, current_line))
                    if current_phase == previous_phase && current_line == previous_line + 1 =>
                {
                    if crosses_input_phase || !advances_phase_line {
                        return Err(composition_error(format!(
                            "{}: phase line 전환과 원문 FF04가 일치하지 않음",
                            row.id
                        )));
                    }
                }
                ((previous_phase, _), (current_phase, current_line))
                    if current_phase == previous_phase + 1
                        && current_line == 1
                        && !crosses_input_phase =>
                {
                    return Err(composition_error(format!(
                        "{}: input phase 전환 사이에 원문 FF0F 입력 대기가 없음",
                        row.id
                    )));
                }
                _ => {}
            }
        }
        if distinct_position != Some(position) {
            match distinct_position {
                None if position != (1, 1) => {
                    return Err(composition_error(format!(
                        "{}: 첫 조판 위치는 input phase 1 phase line 1이어야 함",
                        row.id
                    )));
                }
                Some((input_phase, phase_line))
                    if position != (input_phase, phase_line + 1)
                        && position != (input_phase + 1, 1) =>
                {
                    return Err(composition_error(format!(
                        "{}: input phase/phase line 위치가 비연속",
                        row.id
                    )));
                }
                _ => {}
            }
            distinct_position = Some(position);
        }
        if previous_anchor.is_some_and(|anchor| segment.following_control_token_index <= anchor) {
            return Err(composition_error(format!(
                "{}: control anchor 순서가 역전·중복됨",
                row.id
            )));
        }
        previous_anchor = Some(segment.following_control_token_index);
        previous_position = Some(position);
        let span_index = *control_span_by_token
            .get(&segment.following_control_token_index)
            .ok_or_else(|| {
                composition_error(format!(
                    "{}: 원문에 없는 control anchor token {}",
                    row.id, segment.following_control_token_index
                ))
            })?;
        let source_control = &source_layout.spans[span_index].following_control;
        if source_control.code != segment.following_control_code {
            return Err(composition_error(format!(
                "{}: control anchor code 불일치: token={}",
                row.id, segment.following_control_token_index
            )));
        }
        candidate_runs[span_index] = normalize_quest_display_text(&display_run)
            .map_err(|source| {
                composition_error(format!("{}: segment 정규화 실패: {source}", row.id))
            })?
            .chars()
            .count();
    }

    if text_cursor != row.selected_korean.chars().count() {
        return Err(composition_error(format!(
            "{}: 선택문 끝까지 수동 배치하지 않음",
            row.id
        )));
    }
    let normalized_selected_korean = normalize_quest_display_text(&row.selected_korean)
        .map_err(|source| composition_error(format!("{}: 선택문 정규화 실패: {source}", row.id)))?;
    let normalized_display_korean = normalize_quest_display_text(&display_korean)
        .map_err(|source| composition_error(format!("{}: 표시문 정규화 실패: {source}", row.id)))?;
    let plan = validate_quest_explicit_layout(stream, &candidate_runs).map_err(|source| {
        composition_error(format!("{}: 명시적 run 검증 실패: {source}", row.id))
    })?;
    if plan.candidate_glyph_run_lengths.iter().sum::<usize>()
        != normalized_display_korean.chars().count()
    {
        return Err(composition_error(format!(
            "{}: 정규화 glyph 배치 합계 불일치",
            row.id
        )));
    }

    Ok(QuestManualComposition {
        id: row.id.clone(),
        batch_id: row.batch_id.clone(),
        stream_id: row.stream_id.clone(),
        candidate_basis: row.candidate_basis,
        selected_korean: row.selected_korean.clone(),
        selected_text_sha256: row.selected_text_sha256.clone(),
        normalized_text_sha256: sha256_text(&normalized_selected_korean),
        display_korean,
        normalized_display_korean,
        source_stream_sha256: row.source_stream_sha256.clone(),
        geometry_profile_id: row.geometry_profile_id.clone(),
        geometry_sha256: row.geometry_sha256.clone(),
        status: row.status,
        notes: row.notes.clone(),
        segments,
        candidate_glyph_run_lengths: plan.candidate_glyph_run_lengths,
        geometry_verified: false,
        layout_approved: false,
    })
}
