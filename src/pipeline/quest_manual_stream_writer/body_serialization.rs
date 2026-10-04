use crate::script::{
    parse_quest_token, parse_quest_tokens, quest_control_spec, validate_quest_explicit_layout,
    QuestManualLayoutPlan, QuestSeqExtractedTextStream, QuestSeqExtractedTokenKind, QuestToken,
    TOWN00_SEQ_RUNTIME_BASE,
};

use super::super::quest_translation_routing::projected_stream_body_bytes;
use super::model::{error, QuestManualStreamWriterError};

fn serialize_body_with_plan(
    stream: &QuestSeqExtractedTextStream,
    body_offset: usize,
    glyph_codes: &[[u8; 2]],
    layout: &QuestManualLayoutPlan,
) -> Result<Vec<u8>, QuestManualStreamWriterError> {
    let mut output = Vec::new();
    let mut glyph_cursor = 0usize;
    for (run_index, run_length) in layout
        .candidate_glyph_run_lengths
        .iter()
        .copied()
        .enumerate()
    {
        let glyph_end = glyph_cursor
            .checked_add(run_length)
            .ok_or_else(|| error(format!("{}: glyph run 범위 overflow", stream.id())))?;
        output.extend(
            glyph_codes[glyph_cursor..glyph_end]
                .iter()
                .flatten()
                .copied(),
        );
        glyph_cursor = glyph_end;

        if let Some(control) = layout.controls.get(run_index) {
            let runtime = TOWN00_SEQ_RUNTIME_BASE
                .checked_add(
                    u32::try_from(body_offset)
                        .map_err(|_| error(format!("{}: body offset u32 초과", stream.id())))?,
                )
                .and_then(|value| value.checked_add(u32::try_from(output.len()).ok()?))
                .ok_or_else(|| error(format!("{}: control runtime overflow", stream.id())))?;
            if !matches!(
                parse_quest_token(&control.raw, runtime),
                Ok(QuestToken::Control { raw, .. }) if raw.len() == control.raw.len()
            ) {
                return Err(error(format!(
                    "{}: 이동한 nonterminal control 재파싱 실패",
                    stream.id()
                )));
            }
            output.extend_from_slice(&control.raw);
        }
    }
    if glyph_cursor != glyph_codes.len() {
        return Err(error(format!("{}: 선택문 glyph 배분 누락", stream.id())));
    }

    let terminal_runtime = TOWN00_SEQ_RUNTIME_BASE
        .checked_add(
            u32::try_from(body_offset)
                .map_err(|_| error(format!("{}: body offset u32 초과", stream.id())))?,
        )
        .and_then(|value| value.checked_add(u32::try_from(output.len()).ok()?))
        .ok_or_else(|| error(format!("{}: terminal runtime overflow", stream.id())))?;
    let terminal_bytes = quest_control_spec(0x03)
        .and_then(|spec| spec.layout.encoded_len(terminal_runtime))
        .ok_or_else(|| error(format!("{}: FF03 길이 계산 실패", stream.id())))?;
    let mut terminal = vec![0; terminal_bytes];
    terminal[..2].copy_from_slice(&[0x03, 0xFF]);
    output.extend_from_slice(&terminal);

    let expected = projected_stream_body_bytes(stream, body_offset, glyph_codes.len())
        .map_err(|source| error(source.to_string()))?;
    if output.len() != expected {
        return Err(error(format!(
            "{}: 직렬화 본문 길이 불일치: expected={expected}, actual={}",
            stream.id(),
            output.len()
        )));
    }
    let runtime = TOWN00_SEQ_RUNTIME_BASE
        .checked_add(
            u32::try_from(body_offset)
                .map_err(|_| error(format!("{}: body runtime u32 초과", stream.id())))?,
        )
        .ok_or_else(|| error(format!("{}: body runtime overflow", stream.id())))?;
    let parsed = parse_quest_tokens(&output, runtime).map_err(|source| {
        error(format!(
            "{}: 직렬화 본문 재파싱 실패: {source}",
            stream.id()
        ))
    })?;
    let glyphs = parsed
        .iter()
        .filter(|token| matches!(token, QuestToken::Glyph(_)))
        .count();
    let controls = parsed.len() - glyphs;
    let source_controls = stream
        .tokens
        .iter()
        .filter(|token| token.kind == QuestSeqExtractedTokenKind::Control)
        .count();
    if glyphs != glyph_codes.len()
        || controls != source_controls
        || !matches!(parsed.last(), Some(QuestToken::Control { spec, .. }) if spec.code == 0x03)
    {
        return Err(error(format!(
            "{}: 직렬화 glyph/control 분모 불일치",
            stream.id()
        )));
    }
    Ok(output)
}

pub(super) fn serialize_explicit_body(
    stream: &QuestSeqExtractedTextStream,
    body_offset: usize,
    glyph_codes: &[[u8; 2]],
    candidate_glyph_run_lengths: &[usize],
) -> Result<Vec<u8>, QuestManualStreamWriterError> {
    let layout = validate_quest_explicit_layout(stream, candidate_glyph_run_lengths)
        .map_err(|source| error(source.to_string()))?;
    if layout.candidate_glyph_run_lengths.iter().sum::<usize>() != glyph_codes.len() {
        return Err(error(format!(
            "{}: 명시적 layout glyph 합계 불일치",
            stream.id()
        )));
    }
    serialize_body_with_plan(stream, body_offset, glyph_codes, &layout)
}

/// 수동 run이 각각 원문 run 안에 들 때 원본 control offset과 전체 span을 보존한다.
pub(super) fn serialize_explicit_inline_body_preserving_source_offsets(
    stream: &QuestSeqExtractedTextStream,
    body_offset: usize,
    glyph_codes: &[[u8; 2]],
    candidate_glyph_run_lengths: &[usize],
    padding_code: [u8; 2],
) -> Result<Vec<u8>, QuestManualStreamWriterError> {
    if body_offset != stream.stream_offset {
        return Err(error(format!(
            "{}: inline body offset이 원본 stream offset과 다름",
            stream.id()
        )));
    }
    let layout = validate_quest_explicit_layout(stream, candidate_glyph_run_lengths)
        .map_err(|source| error(source.to_string()))?;
    if layout.candidate_glyph_run_lengths.iter().sum::<usize>() != glyph_codes.len() {
        return Err(error(format!(
            "{}: inline 명시적 layout glyph 합계 불일치",
            stream.id()
        )));
    }

    let mut output = Vec::with_capacity(stream.end_offset - stream.stream_offset);
    let mut glyph_cursor = 0usize;
    for (run_index, (&source_length, &candidate_length)) in layout
        .source_glyph_run_lengths
        .iter()
        .zip(&layout.candidate_glyph_run_lengths)
        .enumerate()
    {
        if candidate_length > source_length {
            return Err(error(format!(
                "{}: inline 수동 glyph run이 원본 span을 초과함: run={run_index}, source={source_length}, candidate={candidate_length}",
                stream.id()
            )));
        }
        let glyph_end = glyph_cursor
            .checked_add(candidate_length)
            .ok_or_else(|| error(format!("{}: inline glyph run 범위 overflow", stream.id())))?;
        output.extend(
            glyph_codes[glyph_cursor..glyph_end]
                .iter()
                .flatten()
                .copied(),
        );
        glyph_cursor = glyph_end;
        for _ in candidate_length..source_length {
            output.extend_from_slice(&padding_code);
        }
        if let Some(control) = layout.controls.get(run_index) {
            output.extend_from_slice(&control.raw);
        }
    }
    if glyph_cursor != glyph_codes.len() {
        return Err(error(format!(
            "{}: inline 선택문 glyph 배분 누락",
            stream.id()
        )));
    }

    let terminal = stream
        .tokens
        .last()
        .ok_or_else(|| error(format!("{}: inline terminal 누락", stream.id())))?;
    output.extend_from_slice(&terminal.raw);
    let source_body_bytes = stream
        .end_offset
        .checked_sub(stream.stream_offset)
        .ok_or_else(|| error(format!("{}: inline 원본 body 범위 역전", stream.id())))?;
    if output.len() != source_body_bytes {
        return Err(error(format!(
            "{}: inline 원본 span 길이 불일치: expected={source_body_bytes}, actual={}",
            stream.id(),
            output.len()
        )));
    }
    for token in stream
        .tokens
        .iter()
        .filter(|token| token.kind == QuestSeqExtractedTokenKind::Control)
    {
        let offset = token
            .offset
            .checked_sub(stream.stream_offset)
            .ok_or_else(|| error(format!("{}: inline control offset 역전", stream.id())))?;
        let end = offset
            .checked_add(token.raw.len())
            .ok_or_else(|| error(format!("{}: inline control 범위 overflow", stream.id())))?;
        if output.get(offset..end) != Some(token.raw.as_slice()) {
            return Err(error(format!(
                "{}: inline control 원본 offset 불일치: token={}",
                stream.id(),
                token.index
            )));
        }
    }
    let runtime = TOWN00_SEQ_RUNTIME_BASE
        .checked_add(
            u32::try_from(body_offset)
                .map_err(|_| error(format!("{}: inline body runtime u32 초과", stream.id())))?,
        )
        .ok_or_else(|| error(format!("{}: inline body runtime overflow", stream.id())))?;
    parse_quest_tokens(&output, runtime).map_err(|source| {
        error(format!(
            "{}: 원본 offset 보존 inline body 재파싱 실패: {source}",
            stream.id()
        ))
    })?;
    Ok(output)
}

pub(super) fn return_record(return_runtime: u32) -> Result<[u8; 4], QuestManualStreamWriterError> {
    super::super::quest_compact_return::encode_return(return_runtime).map_err(error)
}
