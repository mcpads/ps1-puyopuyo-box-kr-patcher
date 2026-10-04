//! 아이템 획득 고정문을 위한 version 관리 수동 composition 입력.
//!
//! 각 행은 source-bound NUL 문자열 하나에 결속한다. parser는 선택문을
//! 정규화하고 원문 digest·단위 분모를 검증하지만, 번역·창 geometry·실행
//! 표시는 승인하지 않는다. 행 전환은 원본 상태기의 호출 순서가 소유하며
//! 이 자산은 자동 개행을 만들지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::analysis::QuestItemAcquisitionTextUnitAnalysis;
use crate::disasm::QuestItemAcquisitionMessageRole;

use super::quest_translation_encoding::normalize_quest_translation_text;

const COMPOSITIONS: &str =
    include_str!("../../assets/translations/quest_item_acquisition_text_compositions.tsv");
const HEADER: &str = "unit_id\tsource_sha256\tselected_text\tstatus\tnotes";
pub const QUEST_ITEM_ACQUISITION_TEXT_COMPOSITION_STATUS: &str =
    "manual-draft-needs-runtime-geometry";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestItemAcquisitionTextCompositionStatus {
    ManualDraftNeedsRuntimeGeometry,
}

impl QuestItemAcquisitionTextCompositionStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ManualDraftNeedsRuntimeGeometry => QUEST_ITEM_ACQUISITION_TEXT_COMPOSITION_STATUS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemAcquisitionTextComposition {
    pub unit_id: String,
    pub source_sha256: [u8; 32],
    pub selected_text: String,
    pub status: QuestItemAcquisitionTextCompositionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemAcquisitionTextCompositionAnalysis {
    pub compositions: Vec<QuestItemAcquisitionTextComposition>,
    pub used_glyphs: String,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompositionRow {
    unit_id: String,
    source_sha256: [u8; 32],
    selected_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemAcquisitionTextCompositionError(String);

impl fmt::Display for QuestItemAcquisitionTextCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemAcquisitionTextCompositionError {}

fn error(message: impl Into<String>) -> QuestItemAcquisitionTextCompositionError {
    QuestItemAcquisitionTextCompositionError(message.into())
}

fn parse_sha256(
    value: &str,
    line_number: usize,
) -> Result<[u8; 32], QuestItemAcquisitionTextCompositionError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(error(format!(
            "QUEST item acquisition composition {line_number}행 source SHA-256 형식 오류"
        )));
    }
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| {
            error(format!(
                "QUEST item acquisition composition {line_number}행 source SHA-256 해석 실패"
            ))
        })?;
    }
    Ok(digest)
}

fn parse_rows(
    input: &str,
) -> Result<Vec<CompositionRow>, QuestItemAcquisitionTextCompositionError> {
    let mut lines = input.lines();
    if lines.next() != Some(HEADER) {
        return Err(error(
            "QUEST item acquisition composition TSV header 불일치",
        ));
    }
    let mut rows = Vec::new();
    for (line_index, line) in lines.enumerate() {
        let line_number = line_index + 2;
        if line.is_empty() {
            return Err(error(format!(
                "QUEST item acquisition composition {line_number}행이 비었음"
            )));
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        let [unit_id, source_sha256, selected_text, status, notes] = fields.as_slice() else {
            return Err(error(format!(
                "QUEST item acquisition composition {line_number}행 열 수 불일치: {}",
                fields.len()
            )));
        };
        if unit_id.is_empty()
            || notes.is_empty()
            || *status != QUEST_ITEM_ACQUISITION_TEXT_COMPOSITION_STATUS
        {
            return Err(error(format!(
                "QUEST item acquisition composition {line_number}행 식별자/status/notes 불일치"
            )));
        }
        rows.push(CompositionRow {
            unit_id: (*unit_id).to_owned(),
            source_sha256: parse_sha256(source_sha256, line_number)?,
            selected_text: (*selected_text).to_owned(),
        });
    }
    if rows.is_empty() {
        return Err(error("QUEST item acquisition composition TSV가 비었음"));
    }
    Ok(rows)
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

fn bind_rows(
    rows: Vec<CompositionRow>,
    units: &QuestItemAcquisitionTextUnitAnalysis,
) -> Result<QuestItemAcquisitionTextCompositionAnalysis, QuestItemAcquisitionTextCompositionError> {
    let source_by_id = units
        .units
        .iter()
        .map(|unit| (unit.id.as_str(), unit))
        .collect::<BTreeMap<_, _>>();
    if source_by_id.len() != units.units.len() {
        return Err(error(
            "QUEST item acquisition composition source unit ID 중복",
        ));
    }

    let mut seen = BTreeSet::new();
    let mut by_id = BTreeMap::new();
    for row in rows {
        let source = source_by_id.get(row.unit_id.as_str()).ok_or_else(|| {
            error(format!(
                "QUEST item acquisition composition의 미등록 unit: {}",
                row.unit_id
            ))
        })?;
        if !seen.insert(row.unit_id.clone()) || row.source_sha256 != source.source_sha256 {
            return Err(error(format!(
                "{}: QUEST item acquisition composition 중복 또는 source SHA-256 drift",
                row.unit_id
            )));
        }
        if (row.selected_text.is_empty()
            && source.role != QuestItemAcquisitionMessageRole::AbandonParticle)
            || contains_japanese(&row.selected_text)
        {
            return Err(error(format!(
                "{}: QUEST item acquisition composition 선택문이 비었거나 일본어가 남음",
                row.unit_id
            )));
        }
        let normalized = normalize_quest_translation_text(&row.selected_text)
            .map_err(|source| error(source.to_string()))?;
        by_id.insert(
            row.unit_id.clone(),
            QuestItemAcquisitionTextComposition {
                unit_id: row.unit_id,
                source_sha256: row.source_sha256,
                selected_text: normalized,
                status: QuestItemAcquisitionTextCompositionStatus::ManualDraftNeedsRuntimeGeometry,
            },
        );
    }

    let mut compositions = Vec::with_capacity(units.units.len());
    let mut used_glyphs = BTreeSet::new();
    for unit in &units.units {
        let composition = by_id.remove(unit.id.as_str()).ok_or_else(|| {
            error(format!(
                "{}: QUEST item acquisition composition 누락",
                unit.id
            ))
        })?;
        used_glyphs.extend(composition.selected_text.chars());
        compositions.push(composition);
    }
    if !by_id.is_empty() || used_glyphs.is_empty() {
        return Err(error(
            "QUEST item acquisition composition에 미소비 unit이 남거나 glyph 집합이 비었음",
        ));
    }

    Ok(QuestItemAcquisitionTextCompositionAnalysis {
        compositions,
        used_glyphs: used_glyphs.into_iter().collect(),
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    })
}

/// 내장 수동 composition을 source-bound 고정 문자열 전수에 결속한다.
pub fn audit_quest_item_acquisition_text_compositions(
    units: &QuestItemAcquisitionTextUnitAnalysis,
) -> Result<QuestItemAcquisitionTextCompositionAnalysis, QuestItemAcquisitionTextCompositionError> {
    bind_rows(parse_rows(COMPOSITIONS)?, units)
}

#[cfg(test)]
#[path = "quest_item_acquisition_text_compositions_tests.rs"]
mod tests;
