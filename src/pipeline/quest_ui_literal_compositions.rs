//! QUEST 지명·공용 선택지를 위한 version 관리 수동 composition 입력.
//!
//! 각 행은 padding을 포함한 source-bound literal slot 하나에 결속한다.
//! parser는 원문 digest·전수 분모·문자 정규화만 검증하며 문구를 자동으로
//! 줄바꿈하거나 축약하지 않는다. 번역·geometry·실행 표시는 미승인이다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::analysis::QuestUiLiteralUnitAnalysis;

use super::quest_translation_encoding::normalize_quest_translation_text;

const COMPOSITIONS: &str =
    include_str!("../../assets/translations/quest_ui_literal_compositions.tsv");
const HEADER: &str = "unit_id\tsource_sha256\tselected_text\tstatus\tnotes";
pub const QUEST_UI_LITERAL_COMPOSITION_STATUS: &str = "manual-draft-needs-runtime-geometry";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestUiLiteralCompositionStatus {
    ManualDraftNeedsRuntimeGeometry,
}

impl QuestUiLiteralCompositionStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ManualDraftNeedsRuntimeGeometry => QUEST_UI_LITERAL_COMPOSITION_STATUS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestUiLiteralComposition {
    pub unit_id: String,
    pub source_sha256: [u8; 32],
    pub selected_text: String,
    pub status: QuestUiLiteralCompositionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestUiLiteralCompositionAnalysis {
    pub compositions: Vec<QuestUiLiteralComposition>,
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
pub struct QuestUiLiteralCompositionError(String);

impl fmt::Display for QuestUiLiteralCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestUiLiteralCompositionError {}

fn error(message: impl Into<String>) -> QuestUiLiteralCompositionError {
    QuestUiLiteralCompositionError(message.into())
}

fn parse_sha256(
    value: &str,
    line_number: usize,
) -> Result<[u8; 32], QuestUiLiteralCompositionError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(error(format!(
            "QUEST UI literal composition {line_number}행 source SHA-256 형식 오류"
        )));
    }
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| {
            error(format!(
                "QUEST UI literal composition {line_number}행 source SHA-256 해석 실패"
            ))
        })?;
    }
    Ok(digest)
}

fn parse_rows(input: &str) -> Result<Vec<CompositionRow>, QuestUiLiteralCompositionError> {
    let mut lines = input.lines();
    if lines.next() != Some(HEADER) {
        return Err(error("QUEST UI literal composition TSV header 불일치"));
    }
    let mut rows = Vec::new();
    for (line_index, line) in lines.enumerate() {
        let line_number = line_index + 2;
        if line.is_empty() {
            return Err(error(format!(
                "QUEST UI literal composition {line_number}행이 비었음"
            )));
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        let [unit_id, source_sha256, selected_text, status, notes] = fields.as_slice() else {
            return Err(error(format!(
                "QUEST UI literal composition {line_number}행 열 수 불일치: {}",
                fields.len()
            )));
        };
        if unit_id.is_empty()
            || selected_text.is_empty()
            || notes.is_empty()
            || *status != QUEST_UI_LITERAL_COMPOSITION_STATUS
        {
            return Err(error(format!(
                "QUEST UI literal composition {line_number}행 식별자/선택문/status/notes 불일치"
            )));
        }
        rows.push(CompositionRow {
            unit_id: (*unit_id).to_owned(),
            source_sha256: parse_sha256(source_sha256, line_number)?,
            selected_text: (*selected_text).to_owned(),
        });
    }
    if rows.is_empty() {
        return Err(error("QUEST UI literal composition TSV가 비었음"));
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
    units: &QuestUiLiteralUnitAnalysis,
) -> Result<QuestUiLiteralCompositionAnalysis, QuestUiLiteralCompositionError> {
    let source_by_id = units
        .units
        .iter()
        .map(|unit| (unit.id.as_str(), unit))
        .collect::<BTreeMap<_, _>>();
    if source_by_id.len() != units.units.len() {
        return Err(error("QUEST UI literal composition source unit ID 중복"));
    }

    let mut seen = BTreeSet::new();
    let mut by_id = BTreeMap::new();
    for row in rows {
        let source = source_by_id.get(row.unit_id.as_str()).ok_or_else(|| {
            error(format!(
                "QUEST UI literal composition의 미등록 unit: {}",
                row.unit_id
            ))
        })?;
        if !seen.insert(row.unit_id.clone()) || row.source_sha256 != source.source_sha256 {
            return Err(error(format!(
                "{}: QUEST UI literal composition 중복 또는 source SHA-256 drift",
                row.unit_id
            )));
        }
        if contains_japanese(&row.selected_text) {
            return Err(error(format!(
                "{}: QUEST UI literal composition 선택문에 일본어가 남음",
                row.unit_id
            )));
        }
        let normalized = normalize_quest_translation_text(&row.selected_text)
            .map_err(|source| error(source.to_string()))?;
        by_id.insert(
            row.unit_id.clone(),
            QuestUiLiteralComposition {
                unit_id: row.unit_id,
                source_sha256: row.source_sha256,
                selected_text: normalized,
                status: QuestUiLiteralCompositionStatus::ManualDraftNeedsRuntimeGeometry,
            },
        );
    }

    let mut compositions = Vec::with_capacity(units.units.len());
    let mut used_glyphs = BTreeSet::new();
    for unit in &units.units {
        let composition = by_id
            .remove(unit.id.as_str())
            .ok_or_else(|| error(format!("{}: QUEST UI literal composition 누락", unit.id)))?;
        used_glyphs.extend(composition.selected_text.chars());
        compositions.push(composition);
    }
    if !by_id.is_empty() || used_glyphs.is_empty() {
        return Err(error(
            "QUEST UI literal composition에 미소비 unit이 남거나 glyph 집합이 비었음",
        ));
    }

    Ok(QuestUiLiteralCompositionAnalysis {
        compositions,
        used_glyphs: used_glyphs.into_iter().collect(),
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    })
}

// 도구 설명과 동시에 보이는 기존 메뉴·선택지·지명은 모든 item page에 남긴다.
// 카드·비행 전용 문구만 첫 page로 분리한다.
pub(super) fn shared_item_ui_glyphs() -> Result<BTreeSet<char>, QuestUiLiteralCompositionError> {
    let mut rows = parse_rows(COMPOSITIONS)?;
    rows.extend(parse_rows(include_str!(
        "../../assets/translations/quest_menu_literal_compositions.tsv"
    ))?);
    let text = rows
        .iter()
        .map(|row| row.selected_text.as_str())
        .collect::<String>();
    Ok(normalize_quest_translation_text(&text)
        .map_err(|source| error(source.to_string()))?
        .chars()
        .collect())
}

/// 역할별 내장 수동 composition을 source-bound UI literal에 결속한다.
pub fn audit_quest_ui_literal_compositions(
    units: &QuestUiLiteralUnitAnalysis,
) -> Result<QuestUiLiteralCompositionAnalysis, QuestUiLiteralCompositionError> {
    let mut rows = parse_rows(COMPOSITIONS)?;
    rows.extend(parse_rows(include_str!(
        "../../assets/translations/quest_menu_literal_compositions.tsv"
    ))?);
    rows.extend(parse_rows(include_str!(
        "../../assets/translations/quest_card_flight_literal_compositions.tsv"
    ))?);
    bind_rows(rows, units)
}

#[cfg(test)]
#[path = "quest_ui_literal_compositions_tests.rs"]
mod tests;
