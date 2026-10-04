//! QUEST 자연 조우 OP21 네 run의 수동 composition 입력.
//!
//! 동적 이름 뒤의 등장 문구와 두 버튼을 source run별 고정 cell 수에 맞춰
//! 사람이 직접 작성한다. parser는 원문·digest·run 분모·명시 pad만 검증하며
//! 조사 선택, 줄바꿈, 축약을 자동으로 만들지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::analysis::{QuestResidentTextRole, QuestResidentTextUnit};

use super::quest_resident_text_writer::{
    QuestResidentTextComposition, QuestResidentTextCompositionStatus,
    QuestResidentTextRunComposition, QUEST_RESIDENT_TEXT_COMPOSITION_STATUS,
};
use super::quest_translation_encoding::normalize_quest_translation_text;

const COMPOSITIONS: &str =
    include_str!("../../assets/translations/quest_resident_encounter_text_compositions.tsv");
const HEADER: &str = "unit_id\tsource_sha256\trun_index\tprotected_source\t\
                     development_text\tleading_pad_cells\ttrailing_pad_cells\tcandidate_a\t\
                     candidate_b\tcandidate_c\tdecision_status\tstatus\tnotes";

pub const QUEST_RESIDENT_ENCOUNTER_PADDING_STATUS: &str = "manual-padding";
pub const QUEST_RESIDENT_ENCOUNTER_HUMAN_CHOICE_STATUS: &str = "draft-needs-human-choice";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestResidentEncounterRunDecisionStatus {
    ManualPadding,
    DraftNeedsHumanChoice,
}

impl QuestResidentEncounterRunDecisionStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ManualPadding => QUEST_RESIDENT_ENCOUNTER_PADDING_STATUS,
            Self::DraftNeedsHumanChoice => QUEST_RESIDENT_ENCOUNTER_HUMAN_CHOICE_STATUS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentEncounterRunDecision {
    pub run_index: usize,
    pub protected_source: String,
    pub development_text: String,
    pub leading_pad_cells: usize,
    pub trailing_pad_cells: usize,
    pub candidates: Vec<String>,
    pub decision_status: QuestResidentEncounterRunDecisionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentEncounterCompositionAnalysis {
    pub composition: QuestResidentTextComposition,
    pub decisions: Vec<QuestResidentEncounterRunDecision>,
    pub used_glyphs: String,
    pub human_choice_runs: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompositionRow {
    unit_id: String,
    source_sha256: [u8; 32],
    run_index: usize,
    protected_source: String,
    development_text: String,
    leading_pad_cells: usize,
    trailing_pad_cells: usize,
    candidates: Vec<String>,
    decision_status: QuestResidentEncounterRunDecisionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentEncounterCompositionError(String);

impl fmt::Display for QuestResidentEncounterCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentEncounterCompositionError {}

fn error(message: impl Into<String>) -> QuestResidentEncounterCompositionError {
    QuestResidentEncounterCompositionError(message.into())
}

fn parse_usize(
    value: &str,
    field: &str,
    line_number: usize,
) -> Result<usize, QuestResidentEncounterCompositionError> {
    value.parse::<usize>().map_err(|_| {
        error(format!(
            "QUEST resident encounter composition {line_number}행 {field} 형식 오류: {value:?}"
        ))
    })
}

fn parse_sha256(
    value: &str,
    line_number: usize,
) -> Result<[u8; 32], QuestResidentEncounterCompositionError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(error(format!(
            "QUEST resident encounter composition {line_number}행 source SHA-256 형식 오류"
        )));
    }
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| {
            error(format!(
                "QUEST resident encounter composition {line_number}행 source SHA-256 해석 실패"
            ))
        })?;
    }
    Ok(digest)
}

fn parse_decision_status(
    value: &str,
    line_number: usize,
) -> Result<QuestResidentEncounterRunDecisionStatus, QuestResidentEncounterCompositionError> {
    match value {
        QUEST_RESIDENT_ENCOUNTER_PADDING_STATUS => {
            Ok(QuestResidentEncounterRunDecisionStatus::ManualPadding)
        }
        QUEST_RESIDENT_ENCOUNTER_HUMAN_CHOICE_STATUS => {
            Ok(QuestResidentEncounterRunDecisionStatus::DraftNeedsHumanChoice)
        }
        _ => Err(error(format!(
            "QUEST resident encounter composition {line_number}행 decision status 오류: {value:?}"
        ))),
    }
}

fn contains_japanese(text: &str) -> bool {
    text.chars().any(|glyph| {
        matches!(
            glyph,
            '\u{3040}'..='\u{30ff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{4e00}'..='\u{9fff}'
                | '\u{ff65}'..='\u{ff9f}'
        )
    })
}

fn normalize_candidate(
    value: &str,
    line_number: usize,
    role: &str,
) -> Result<String, QuestResidentEncounterCompositionError> {
    if value.is_empty() || contains_japanese(value) {
        return Err(error(format!(
            "QUEST resident encounter composition {line_number}행 {role}가 비었거나 일본어를 포함함"
        )));
    }
    normalize_quest_translation_text(value).map_err(|source| error(source.to_string()))
}

fn parse_rows(input: &str) -> Result<Vec<CompositionRow>, QuestResidentEncounterCompositionError> {
    let mut lines = input.lines();
    if lines.next() != Some(HEADER) {
        return Err(error(
            "QUEST resident encounter composition TSV header 불일치",
        ));
    }
    let mut rows = Vec::new();
    for (line_index, line) in lines.enumerate() {
        let line_number = line_index + 2;
        if line.is_empty() {
            return Err(error(format!(
                "QUEST resident encounter composition {line_number}행이 비었음"
            )));
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        let [unit_id, source_sha256, run_index, protected_source, development_text, leading_pad, trailing_pad, candidate_a, candidate_b, candidate_c, decision_status, status, notes] =
            fields.as_slice()
        else {
            return Err(error(format!(
                "QUEST resident encounter composition {line_number}행 열 수 불일치: {}",
                fields.len()
            )));
        };
        if unit_id.is_empty()
            || protected_source.is_empty()
            || notes.is_empty()
            || *status != QUEST_RESIDENT_TEXT_COMPOSITION_STATUS
        {
            return Err(error(format!(
                "QUEST resident encounter composition {line_number}행 필수 field/status 불일치"
            )));
        }
        let decision_status = parse_decision_status(decision_status, line_number)?;
        let leading_pad_cells = parse_usize(leading_pad, "leading_pad_cells", line_number)?;
        let trailing_pad_cells = parse_usize(trailing_pad, "trailing_pad_cells", line_number)?;
        let (development_text, candidates) = match decision_status {
            QuestResidentEncounterRunDecisionStatus::ManualPadding => {
                if !development_text.is_empty()
                    || !candidate_a.is_empty()
                    || !candidate_b.is_empty()
                    || !candidate_c.is_empty()
                    || protected_source.chars().any(|glyph| glyph != '\u{3000}')
                {
                    return Err(error(format!(
                        "QUEST resident encounter composition {line_number}행 padding 계약 불일치"
                    )));
                }
                (String::new(), Vec::new())
            }
            QuestResidentEncounterRunDecisionStatus::DraftNeedsHumanChoice => {
                let development = normalize_candidate(development_text, line_number, "개발안")?;
                let candidates = [candidate_a, candidate_b, candidate_c]
                    .into_iter()
                    .map(|candidate| normalize_candidate(candidate, line_number, "선택 후보"))
                    .collect::<Result<Vec<_>, _>>()?;
                if candidates.iter().collect::<BTreeSet<_>>().len() != 3
                    || !candidates.contains(&development)
                {
                    return Err(error(format!(
                        "QUEST resident encounter composition {line_number}행 세 후보/개발안 불일치"
                    )));
                }
                (development, candidates)
            }
        };
        rows.push(CompositionRow {
            unit_id: (*unit_id).to_owned(),
            source_sha256: parse_sha256(source_sha256, line_number)?,
            run_index: parse_usize(run_index, "run_index", line_number)?,
            protected_source: (*protected_source).to_owned(),
            development_text,
            leading_pad_cells,
            trailing_pad_cells,
            candidates,
            decision_status,
        });
    }
    if rows.is_empty() {
        return Err(error("QUEST resident encounter composition TSV가 비었음"));
    }
    Ok(rows)
}

fn bind_rows(
    rows: Vec<CompositionRow>,
    unit: &QuestResidentTextUnit,
) -> Result<QuestResidentEncounterCompositionAnalysis, QuestResidentEncounterCompositionError> {
    if unit.role != QuestResidentTextRole::EncounterMenu {
        return Err(error(
            "QUEST resident encounter composition에 다른 role을 전달함",
        ));
    }
    let mut rows_by_index = BTreeMap::new();
    for row in rows {
        if rows_by_index.insert(row.run_index, row).is_some() {
            return Err(error("QUEST resident encounter composition run index 중복"));
        }
    }
    let mut run_compositions = Vec::with_capacity(unit.runs.len());
    let mut decisions = Vec::with_capacity(unit.runs.len());
    let mut used_glyphs = BTreeSet::new();
    let mut human_choice_runs = 0usize;
    for source_run in &unit.runs {
        let row = rows_by_index.remove(&source_run.index).ok_or_else(|| {
            error(format!(
                "{}: QUEST resident encounter composition run {} 누락",
                unit.id, source_run.index
            ))
        })?;
        if row.unit_id != unit.id
            || row.source_sha256 != unit.expected_raw_sha256
            || row.protected_source != source_run.source
        {
            return Err(error(format!(
                "{}: QUEST resident encounter composition source/run drift",
                unit.id
            )));
        }
        let cells = row
            .leading_pad_cells
            .checked_add(row.development_text.chars().count())
            .and_then(|value| value.checked_add(row.trailing_pad_cells))
            .ok_or_else(|| error("QUEST resident encounter composition cell 합계 overflow"))?;
        if cells != source_run.glyph_occurrences {
            return Err(error(format!(
                "{}: QUEST resident encounter run {} cell 수 불일치: {cells}/{}",
                unit.id, source_run.index, source_run.glyph_occurrences
            )));
        }
        used_glyphs.extend(row.development_text.chars());
        if row.leading_pad_cells != 0 || row.trailing_pad_cells != 0 {
            used_glyphs.insert('\u{3000}');
        }
        if row.decision_status == QuestResidentEncounterRunDecisionStatus::DraftNeedsHumanChoice {
            human_choice_runs += 1;
        }
        run_compositions.push(QuestResidentTextRunComposition {
            run_index: row.run_index,
            text: row.development_text.clone(),
            leading_pad_cells: row.leading_pad_cells,
            trailing_pad_cells: row.trailing_pad_cells,
        });
        decisions.push(QuestResidentEncounterRunDecision {
            run_index: row.run_index,
            protected_source: row.protected_source,
            development_text: row.development_text,
            leading_pad_cells: row.leading_pad_cells,
            trailing_pad_cells: row.trailing_pad_cells,
            candidates: row.candidates,
            decision_status: row.decision_status,
        });
    }
    if !rows_by_index.is_empty() || used_glyphs.is_empty() {
        return Err(error(
            "QUEST resident encounter composition에 미소비 run이 남거나 glyph 집합이 비었음",
        ));
    }
    Ok(QuestResidentEncounterCompositionAnalysis {
        composition: QuestResidentTextComposition {
            unit_id: unit.id.clone(),
            source_sha256: unit.expected_raw_sha256,
            status: QuestResidentTextCompositionStatus::ManualDraftNeedsGeometry,
            runs: run_compositions,
        },
        decisions,
        used_glyphs: used_glyphs.into_iter().collect(),
        human_choice_runs,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    })
}

/// 내장 수동 composition을 source-bound 자연 조우 OP21에 결속한다.
pub fn audit_quest_resident_encounter_text_composition(
    unit: &QuestResidentTextUnit,
) -> Result<QuestResidentEncounterCompositionAnalysis, QuestResidentEncounterCompositionError> {
    bind_rows(parse_rows(COMPOSITIONS)?, unit)
}

#[cfg(test)]
#[path = "quest_resident_encounter_text_composition_tests.rs"]
mod tests;
