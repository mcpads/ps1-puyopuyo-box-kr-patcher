//! QUEST 원문 stream의 표시 제어 구조를 사람 검토용으로 펼친다.
//!
//! 이 모듈은 번역문을 행이나 창에 배치하지 않는다. 원문 glyph span과 그
//! 뒤의 실제 control, 그리고 stream 안에서 확정 가능한 window·line 문맥만
//! 기록한다. stream 진입 전에 선택된 window처럼 현재 입력만으로 알 수 없는
//! 값은 추정하지 않고 `None`으로 남긴다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::script::{
    QuestControlMeaning, QuestSeqExtractedTextStream, QuestSeqExtractedTokenKind, QuestTextTerminal,
};

use super::QuestReviewQueue;

/// 원문 glyph span 뒤에서 실행되는 control 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSourceLayoutControl {
    pub token_index: usize,
    pub code: u8,
    pub meaning: QuestControlMeaning,
    pub raw_hex: String,
}

/// control 사이에 놓인 원문 glyph span 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSourceLayoutSpan {
    pub sequence: usize,
    pub protected_source: String,
    pub glyphs: usize,
    /// `FF06`으로 이 stream 안에서 확정된 window index.
    pub window_index: Option<u8>,
    /// `FF15` 이후 `FF04` 진행만으로 확정할 수 있는 0-based line index.
    pub line_index: Option<usize>,
    /// 이 stream 안에서 앞서 끝난 `FF0F` input wait 수.
    pub input_phase: usize,
    pub following_control: QuestSourceLayoutControl,
}

/// canonical OP21 stream 하나의 원문 표시 구조.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSourceLayout {
    pub stream_id: String,
    pub source_stream_sha256: String,
    pub spans: Vec<QuestSourceLayoutSpan>,
    pub glyphs: usize,
    pub controls: usize,
    pub unresolved_window_spans: usize,
    pub unresolved_line_spans: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSourceLayoutError(String);

impl fmt::Display for QuestSourceLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestSourceLayoutError {}

fn error(message: impl Into<String>) -> QuestSourceLayoutError {
    QuestSourceLayoutError(message.into())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn selected_window(raw: &[u8]) -> Option<u8> {
    match raw {
        [0x06, 0xFF, index, 0] => Some(*index),
        _ => None,
    }
}

fn current_line(window_index: Option<u8>, lines: &BTreeMap<u8, Option<usize>>) -> Option<usize> {
    window_index.and_then(|index| lines.get(&index).copied().flatten())
}

/// 원문 stream의 glyph/control 순서를 보존해 검토 가능한 표시 구조를 만든다.
pub fn build_quest_source_layout(
    stream: &QuestSeqExtractedTextStream,
) -> Result<QuestSourceLayout, QuestSourceLayoutError> {
    if stream.terminal != QuestTextTerminal::DeactivateAndAlign {
        return Err(error(format!(
            "{}: canonical FF03 종단 stream이 아님",
            stream.id()
        )));
    }

    let mut spans = Vec::new();
    let mut source = String::new();
    let mut glyphs_in_span = 0usize;
    let mut glyphs = 0usize;
    let mut controls = 0usize;
    let mut window_index = None;
    let mut lines = BTreeMap::<u8, Option<usize>>::new();
    let mut input_phase = 0usize;

    for token in &stream.tokens {
        match token.kind {
            QuestSeqExtractedTokenKind::Glyph => {
                let glyph = token.source.as_deref().ok_or_else(|| {
                    error(format!(
                        "{}: source를 해독하지 못한 glyph가 있음: token={}",
                        stream.id(),
                        token.index
                    ))
                })?;
                if glyph.chars().count() != 1 {
                    return Err(error(format!(
                        "{}: glyph source가 한 문자가 아님: token={}",
                        stream.id(),
                        token.index
                    )));
                }
                source.push_str(glyph);
                glyphs_in_span = glyphs_in_span
                    .checked_add(1)
                    .ok_or_else(|| error(format!("{}: span glyph 수 overflow", stream.id())))?;
                glyphs = glyphs
                    .checked_add(1)
                    .ok_or_else(|| error(format!("{}: glyph 수 overflow", stream.id())))?;
            }
            QuestSeqExtractedTokenKind::Control => {
                let code = token.control_code.ok_or_else(|| {
                    error(format!(
                        "{}: control code가 없음: token={}",
                        stream.id(),
                        token.index
                    ))
                })?;
                let meaning = token.control_meaning.ok_or_else(|| {
                    error(format!(
                        "{}: control 의미가 없음: token={}",
                        stream.id(),
                        token.index
                    ))
                })?;
                controls = controls
                    .checked_add(1)
                    .ok_or_else(|| error(format!("{}: control 수 overflow", stream.id())))?;
                spans.push(QuestSourceLayoutSpan {
                    sequence: spans.len() + 1,
                    protected_source: std::mem::take(&mut source),
                    glyphs: std::mem::take(&mut glyphs_in_span),
                    window_index,
                    line_index: current_line(window_index, &lines),
                    input_phase,
                    following_control: QuestSourceLayoutControl {
                        token_index: token.index,
                        code,
                        meaning,
                        raw_hex: hex(&token.raw),
                    },
                });

                match meaning {
                    QuestControlMeaning::SelectTextWindow => {
                        window_index = Some(selected_window(&token.raw).ok_or_else(|| {
                            error(format!(
                                "{}: FF06 raw가 확정 layout과 다름: token={}, raw={}",
                                stream.id(),
                                token.index,
                                hex(&token.raw)
                            ))
                        })?);
                    }
                    QuestControlMeaning::OpenTextWindow => {
                        if let Some(index) = window_index {
                            lines.insert(index, Some(0));
                        }
                    }
                    QuestControlMeaning::AdvanceTextLine => {
                        if let Some(index) = window_index {
                            let next = lines
                                .get(&index)
                                .copied()
                                .flatten()
                                .and_then(|line| line.checked_add(1));
                            lines.insert(index, next);
                        }
                    }
                    QuestControlMeaning::SetCursorPosition
                    | QuestControlMeaning::CloseTextWindow => {
                        if let Some(index) = window_index {
                            lines.insert(index, None);
                        }
                    }
                    QuestControlMeaning::StartInputWait => {
                        input_phase = input_phase.checked_add(1).ok_or_else(|| {
                            error(format!("{}: input phase overflow", stream.id()))
                        })?;
                    }
                    _ => {}
                }
            }
        }
    }

    if !source.is_empty() || glyphs_in_span != 0 {
        return Err(error(format!(
            "{}: terminal control 뒤가 아니라 glyph로 끝남",
            stream.id()
        )));
    }
    let terminal = spans
        .last()
        .ok_or_else(|| error(format!("{}: 빈 stream", stream.id())))?;
    if terminal.following_control.code != 0x03
        || terminal.following_control.meaning != QuestControlMeaning::DeactivateInterpreterAndAlign
    {
        return Err(error(format!(
            "{}: 마지막 control이 canonical FF03이 아님",
            stream.id()
        )));
    }

    let unresolved_window_spans = spans
        .iter()
        .filter(|span| span.glyphs != 0 && span.window_index.is_none())
        .count();
    let unresolved_line_spans = spans
        .iter()
        .filter(|span| span.glyphs != 0 && span.line_index.is_none())
        .count();
    Ok(QuestSourceLayout {
        stream_id: stream.id(),
        source_stream_sha256: hex(&stream.expected_raw_sha256),
        spans,
        glyphs,
        controls,
        unresolved_window_spans,
        unresolved_line_spans,
    })
}

/// 현재 검토 대기열의 각 항목을 정확히 하나의 원문 stream 구조와 결속한다.
pub fn build_quest_review_queue_source_layouts(
    queue: &QuestReviewQueue,
    streams: &[QuestSeqExtractedTextStream],
) -> Result<Vec<QuestSourceLayout>, QuestSourceLayoutError> {
    queue
        .entries
        .iter()
        .map(|entry| {
            let mut matches = streams
                .iter()
                .filter(|stream| stream.id() == entry.stream_id);
            let stream = matches.next().ok_or_else(|| {
                error(format!(
                    "QUEST 원문 표시 구조 stream이 없음: {}",
                    entry.stream_id
                ))
            })?;
            if matches.next().is_some() {
                return Err(error(format!(
                    "QUEST 원문 표시 구조 stream이 중복됨: {}",
                    entry.stream_id
                )));
            }
            build_quest_source_layout(stream)
        })
        .collect()
}

#[cfg(test)]
#[path = "quest_translation_source_layout_tests.rs"]
mod tests;
