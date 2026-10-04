//! QUEST 수동 조판과 원본 control 골격의 저수준 배치 경계를 검증한다.
//!
//! 제품과 사람 검토용 재현은 명시적으로 입력된 run 배치만 받는다.

use std::error::Error;
use std::fmt;

use crate::script::{QuestSeqExtractedTextStream, QuestSeqExtractedTokenKind, QuestTextTerminal};

/// 사람이 작성한 선택문과 control 앞 glyph run을 결속한 개발 빌드 입력.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestManualStreamLayout {
    pub stream_id: String,
    pub selected_korean: String,
    pub candidate_glyph_run_lengths: Vec<usize>,
}

/// terminal 앞에서 보존할 원본 control 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestManualLayoutControl {
    pub code: u8,
    pub raw: Vec<u8>,
}

/// 원본과 후보의 glyph run 및 그 사이 control 골격.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestManualLayoutPlan {
    pub source_glyph_run_lengths: Vec<usize>,
    pub candidate_glyph_run_lengths: Vec<usize>,
    pub controls: Vec<QuestManualLayoutControl>,
    pub terminal_code: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestManualLayoutError(String);

impl fmt::Display for QuestManualLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestManualLayoutError {}

fn error(message: impl Into<String>) -> QuestManualLayoutError {
    QuestManualLayoutError(message.into())
}

fn terminal_index(stream: &QuestSeqExtractedTextStream) -> Result<usize, QuestManualLayoutError> {
    let index = stream
        .tokens
        .len()
        .checked_sub(1)
        .ok_or_else(|| error(format!("{}: token이 비었음", stream.id())))?;
    let terminal = &stream.tokens[index];
    if stream.terminal != QuestTextTerminal::DeactivateAndAlign
        || terminal.kind != QuestSeqExtractedTokenKind::Control
        || terminal.control_code != Some(0x03)
        || terminal.raw.len() < 6
        || terminal.raw[..2] != [0x03, 0xFF]
        || terminal.raw[2..].iter().any(|byte| *byte != 0)
    {
        return Err(error(format!(
            "{}: manual layout이 지원하는 canonical FF03 종단이 아님",
            stream.id()
        )));
    }
    Ok(index)
}

fn source_layout_skeleton(
    stream: &QuestSeqExtractedTextStream,
) -> Result<(Vec<usize>, Vec<QuestManualLayoutControl>), QuestManualLayoutError> {
    let terminal = terminal_index(stream)?;
    let mut source_runs = vec![0usize];
    let mut controls = Vec::new();
    for token in &stream.tokens[..terminal] {
        match token.kind {
            QuestSeqExtractedTokenKind::Glyph => {
                *source_runs.last_mut().expect("초기 glyph run") += 1;
            }
            QuestSeqExtractedTokenKind::Control => {
                controls.push(QuestManualLayoutControl {
                    code: token
                        .control_code
                        .ok_or_else(|| error(format!("{}: control code가 없음", stream.id())))?,
                    raw: token.raw.clone(),
                });
                source_runs.push(0);
            }
        }
    }
    let source_glyphs = source_runs.iter().sum::<usize>();
    if source_runs.len() != controls.len() + 1 || source_glyphs == 0 {
        return Err(error(format!(
            "{}: 원문 glyph run/control 골격 불일치",
            stream.id()
        )));
    }
    Ok((source_runs, controls))
}

/// 사람이 지정한 각 control 앞 glyph 수를 원문 골격과 대조한다.
pub fn validate_quest_explicit_layout(
    stream: &QuestSeqExtractedTextStream,
    candidate_glyph_run_lengths: &[usize],
) -> Result<QuestManualLayoutPlan, QuestManualLayoutError> {
    let (source_glyph_run_lengths, controls) = source_layout_skeleton(stream)?;
    if candidate_glyph_run_lengths.len() != source_glyph_run_lengths.len() {
        return Err(error(format!(
            "{}: 명시적 layout run 수 불일치: expected={}, actual={}",
            stream.id(),
            source_glyph_run_lengths.len(),
            candidate_glyph_run_lengths.len()
        )));
    }
    let candidate_glyphs = candidate_glyph_run_lengths
        .iter()
        .try_fold(0usize, |sum, run| sum.checked_add(*run))
        .ok_or_else(|| {
            error(format!(
                "{}: 명시적 layout glyph 합계 overflow",
                stream.id()
            ))
        })?;
    if candidate_glyphs == 0 {
        return Err(error(format!("{}: 후보 glyph가 비었음", stream.id())));
    }
    Ok(QuestManualLayoutPlan {
        source_glyph_run_lengths,
        candidate_glyph_run_lengths: candidate_glyph_run_lengths.to_vec(),
        controls,
        terminal_code: 0x03,
    })
}

#[cfg(test)]
#[path = "quest_manual_layout_tests.rs"]
mod tests;
