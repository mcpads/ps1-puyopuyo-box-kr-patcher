//! `QUEST.EXE` 상주 문구의 version 관리 수동 composition 입력.
//!
//! 각 행은 source-bound unit의 glyph run 하나와 앞·뒤 pad cell을 직접
//! 지정한다. parser는 내장 TSV를 source SHA와 run 분모에 다시 결속하며,
//! 문구 의미와 화면 geometry를 승인하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::analysis::QuestResidentTextUnitAnalysis;

use super::quest_resident_text_writer::{
    QuestResidentTextComposition, QuestResidentTextCompositionStatus,
    QuestResidentTextRunComposition, QUEST_RESIDENT_TEXT_COMPOSITION_STATUS,
};
use super::quest_translation_encoding::normalize_quest_translation_text;

const COMPOSITIONS: &str =
    include_str!("../../assets/translations/quest_resident_text_compositions.tsv");
const HEADER: &str = "unit_id\tsource_sha256\trun_index\tselected_text\tleading_pad_cells\t\
                     trailing_pad_cells\tstatus\tnotes";

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompositionRow {
    unit_id: String,
    source_sha256: [u8; 32],
    run: QuestResidentTextRunComposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompositionSource {
    unit_id: String,
    source_sha256: [u8; 32],
    runs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextCompositionError(String);

impl fmt::Display for QuestResidentTextCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentTextCompositionError {}

fn error(message: impl Into<String>) -> QuestResidentTextCompositionError {
    QuestResidentTextCompositionError(message.into())
}

fn parse_sha256(
    value: &str,
    line_number: usize,
) -> Result<[u8; 32], QuestResidentTextCompositionError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(error(format!(
            "QUEST resident composition {line_number}행 source SHA-256 형식 오류"
        )));
    }
    let mut digest = [0u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| {
            error(format!(
                "QUEST resident composition {line_number}행 source SHA-256 해석 실패"
            ))
        })?;
    }
    Ok(digest)
}

fn parse_usize(
    value: &str,
    field: &str,
    line_number: usize,
) -> Result<usize, QuestResidentTextCompositionError> {
    value.parse::<usize>().map_err(|_| {
        error(format!(
            "QUEST resident composition {line_number}행 {field} 형식 오류: {value:?}"
        ))
    })
}

fn parse_rows(input: &str) -> Result<Vec<CompositionRow>, QuestResidentTextCompositionError> {
    let mut lines = input.lines();
    if lines.next() != Some(HEADER) {
        return Err(error("QUEST resident composition TSV header 불일치"));
    }
    let mut rows = Vec::new();
    for (line_index, line) in lines.enumerate() {
        let line_number = line_index + 2;
        if line.is_empty() {
            return Err(error(format!(
                "QUEST resident composition {line_number}행이 비었음"
            )));
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        let [unit_id, source_sha256, run_index, selected_text, leading_pad, trailing_pad, status, notes] =
            fields.as_slice()
        else {
            return Err(error(format!(
                "QUEST resident composition {line_number}행 열 수 불일치: {}",
                fields.len()
            )));
        };
        if unit_id.is_empty()
            || notes.is_empty()
            || *status != QUEST_RESIDENT_TEXT_COMPOSITION_STATUS
        {
            return Err(error(format!(
                "QUEST resident composition {line_number}행 식별자/status/notes 불일치"
            )));
        }
        rows.push(CompositionRow {
            unit_id: (*unit_id).to_string(),
            source_sha256: parse_sha256(source_sha256, line_number)?,
            run: QuestResidentTextRunComposition {
                run_index: parse_usize(run_index, "run_index", line_number)?,
                text: (*selected_text).to_string(),
                leading_pad_cells: parse_usize(leading_pad, "leading_pad_cells", line_number)?,
                trailing_pad_cells: parse_usize(trailing_pad, "trailing_pad_cells", line_number)?,
            },
        });
    }
    if rows.is_empty() {
        return Err(error("QUEST resident composition TSV가 비었음"));
    }
    Ok(rows)
}

fn bind_rows(
    rows: Vec<CompositionRow>,
    sources: &[CompositionSource],
) -> Result<Vec<QuestResidentTextComposition>, QuestResidentTextCompositionError> {
    let source_by_id = sources
        .iter()
        .map(|source| (source.unit_id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    if source_by_id.len() != sources.len() {
        return Err(error("QUEST resident composition source unit ID 중복"));
    }

    let mut rows_by_id = BTreeMap::<String, Vec<CompositionRow>>::new();
    for row in rows {
        let source = source_by_id.get(row.unit_id.as_str()).ok_or_else(|| {
            error(format!(
                "QUEST resident composition의 미등록 unit: {}",
                row.unit_id
            ))
        })?;
        if row.source_sha256 != source.source_sha256 {
            return Err(error(format!(
                "{}: QUEST resident composition source SHA-256 drift",
                row.unit_id
            )));
        }
        rows_by_id.entry(row.unit_id.clone()).or_default().push(row);
    }

    let mut compositions = Vec::with_capacity(sources.len());
    for source in sources {
        let mut rows = rows_by_id.remove(&source.unit_id).ok_or_else(|| {
            error(format!(
                "{}: QUEST resident composition 누락",
                source.unit_id
            ))
        })?;
        rows.sort_by_key(|row| row.run.run_index);
        if rows.len() != source.runs
            || rows
                .iter()
                .enumerate()
                .any(|(expected, row)| row.run.run_index != expected)
        {
            return Err(error(format!(
                "{}: QUEST resident composition run 분모/순서 불일치",
                source.unit_id
            )));
        }
        compositions.push(QuestResidentTextComposition {
            unit_id: source.unit_id.clone(),
            source_sha256: source.source_sha256,
            status: QuestResidentTextCompositionStatus::ManualDraftNeedsGeometry,
            runs: rows.into_iter().map(|row| row.run).collect(),
        });
    }
    if !rows_by_id.is_empty() {
        return Err(error("QUEST resident composition에 미소비 unit 행이 남음"));
    }
    Ok(compositions)
}

/// 내장 수동 composition을 source-bound resident unit에 결속한다.
pub fn audit_quest_resident_text_compositions(
    units: &QuestResidentTextUnitAnalysis,
) -> Result<Vec<QuestResidentTextComposition>, QuestResidentTextCompositionError> {
    let sources = units
        .units
        .iter()
        .map(|unit| CompositionSource {
            unit_id: unit.id.clone(),
            source_sha256: unit.expected_raw_sha256,
            runs: unit.runs.len(),
        })
        .collect::<Vec<_>>();
    bind_rows(parse_rows(COMPOSITIONS)?, &sources)
}

/// supplemental page 계산에 쓸 정규화된 실제 사용 문자 집합을 만든다.
pub fn quest_resident_text_composition_used_glyphs(
    compositions: &[QuestResidentTextComposition],
) -> Result<String, QuestResidentTextCompositionError> {
    let mut glyphs = BTreeSet::new();
    for composition in compositions {
        for run in &composition.runs {
            let normalized = normalize_quest_translation_text(&run.text)
                .map_err(|source| error(source.to_string()))?;
            glyphs.extend(normalized.chars());
            if run.leading_pad_cells != 0 || run.trailing_pad_cells != 0 {
                glyphs.insert('\u{3000}');
            }
        }
    }
    if glyphs.is_empty() {
        return Err(error(
            "QUEST resident composition의 사용 glyph 집합이 비었음",
        ));
    }
    Ok(glyphs.into_iter().collect())
}

#[cfg(test)]
#[path = "quest_resident_text_compositions_tests.rs"]
mod tests;
