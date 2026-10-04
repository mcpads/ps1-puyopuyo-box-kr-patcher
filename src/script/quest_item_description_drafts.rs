//! `ITEM_DAT.SEQ` 설명의 수동 한국어 초안과 control 배치 결속.
//!
//! 원장은 한국어와 `{FFxx:HEX}` control을 한 필드에 함께 기록한다. 따라서
//! 개행 `FF04`를 비롯한 배치는 사람이 직접 작성한 입력이며, 이 모듈은
//! source unit 전수·보호 원문·control raw 순서·문자 집합만 검산한다.
//! 번역·조판·표시 승인과 제품 사용 가능 여부는 별도 HITL gate가 소유한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::{
    QuestItemTextRole, QuestItemTextTokenKind, QuestItemTextUnit, QuestItemTextUnitAnalysis,
};

pub const QUEST_ITEM_DESCRIPTION_DRAFT_SCHEMA_VERSION: &str = "quest-item-description-draft";

const DRAFTS: &str = include_str!("../../assets/translations/quest_item_description_drafts.tsv");
const HEADER: &str = "schema_version\tunit_id\tprotected_source\tdraft_annotated\tstatus\tnote";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestItemDescriptionDraftStatus {
    DraftNeedsIndependentReview,
    DraftNeedsHumanChoice,
}

impl QuestItemDescriptionDraftStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::DraftNeedsIndependentReview => "draft-needs-independent-review",
            Self::DraftNeedsHumanChoice => "draft-needs-human-choice",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "draft-needs-independent-review" => Some(Self::DraftNeedsIndependentReview),
            "draft-needs-human-choice" => Some(Self::DraftNeedsHumanChoice),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestItemDescriptionDraftPart {
    Literal(String),
    Control(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemDescriptionDraftEntry {
    pub unit_id: String,
    pub protected_source: String,
    pub draft_annotated: String,
    pub draft_korean: String,
    pub control_raw: Vec<Vec<u8>>,
    pub parts: Vec<QuestItemDescriptionDraftPart>,
    pub status: QuestItemDescriptionDraftStatus,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemDescriptionDraftAnalysis {
    pub entries: Vec<QuestItemDescriptionDraftEntry>,
    pub used_glyphs: String,
    pub glyph_occurrences: usize,
    pub control_occurrences: usize,
    pub manual_line_breaks: usize,
    pub draft_needs_independent_review: usize,
    pub draft_needs_human_choice: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub product_eligible: bool,
    pub asset_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemDescriptionDraftError(String);

impl fmt::Display for QuestItemDescriptionDraftError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemDescriptionDraftError {}

fn error(message: impl Into<String>) -> QuestItemDescriptionDraftError {
    QuestItemDescriptionDraftError(message.into())
}

fn contains_japanese(raw: &str) -> bool {
    raw.chars().any(|glyph| {
        matches!(
            glyph,
            '\u{3040}'..='\u{30ff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{4e00}'..='\u{9fff}'
                | '\u{ff65}'..='\u{ff9f}'
        )
    })
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn decode_control_hex(raw: &str) -> Result<Vec<u8>, QuestItemDescriptionDraftError> {
    if raw.is_empty() || !raw.len().is_multiple_of(2) {
        return Err(error("QUEST item 설명 control HEX 길이가 올바르지 않음"));
    }
    raw.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0])
                .ok_or_else(|| error("QUEST item 설명 control HEX가 대문자 16진수가 아님"))?;
            let low = hex_nibble(pair[1])
                .ok_or_else(|| error("QUEST item 설명 control HEX가 대문자 16진수가 아님"))?;
            Ok((high << 4) | low)
        })
        .collect()
}

struct ParsedAnnotatedDraft {
    literal: String,
    controls: Vec<Vec<u8>>,
    parts: Vec<QuestItemDescriptionDraftPart>,
}

fn parse_annotated(raw: &str) -> Result<ParsedAnnotatedDraft, QuestItemDescriptionDraftError> {
    let mut literal = String::new();
    let mut controls = Vec::new();
    let mut parts = Vec::new();
    let mut cursor = 0usize;

    while cursor < raw.len() {
        let remaining = &raw[cursor..];
        let Some(open_relative) = remaining.find('{') else {
            if remaining.contains('}') {
                return Err(error("QUEST item 설명 annotation 닫는 괄호가 고립됨"));
            }
            literal.push_str(remaining);
            if !remaining.is_empty() {
                parts.push(QuestItemDescriptionDraftPart::Literal(remaining.to_owned()));
            }
            break;
        };
        let open = cursor + open_relative;
        let plain = &raw[cursor..open];
        if plain.contains('}') {
            return Err(error("QUEST item 설명 annotation 닫는 괄호가 고립됨"));
        }
        literal.push_str(plain);
        if !plain.is_empty() {
            parts.push(QuestItemDescriptionDraftPart::Literal(plain.to_owned()));
        }
        let after_open = open + 1;
        let close_relative = raw[after_open..]
            .find('}')
            .ok_or_else(|| error("QUEST item 설명 annotation 닫는 괄호가 없음"))?;
        let close = after_open + close_relative;
        let annotation = &raw[after_open..close];
        let (label, hex) = annotation
            .split_once(':')
            .ok_or_else(|| error("QUEST item 설명 annotation에 label/HEX 구분자가 없음"))?;
        let control = decode_control_hex(hex)?;
        let code = *control
            .first()
            .ok_or_else(|| error("QUEST item 설명 control raw가 비었음"))?;
        if control.get(1) != Some(&0xFF) || label != format!("FF{code:02X}") {
            return Err(error(format!(
                "QUEST item 설명 control label/raw 불일치: {{{annotation}}}"
            )));
        }
        parts.push(QuestItemDescriptionDraftPart::Control(control.clone()));
        controls.push(control);
        cursor = close + 1;
    }

    if literal.is_empty() {
        return Err(error("QUEST item 설명 한국어 literal이 비었음"));
    }
    if contains_japanese(&literal) {
        return Err(error("QUEST item 설명 한국어 초안에 일본어가 남음"));
    }
    Ok(ParsedAnnotatedDraft {
        literal,
        controls,
        parts,
    })
}

fn parse_drafts(
    raw: &str,
) -> Result<Vec<QuestItemDescriptionDraftEntry>, QuestItemDescriptionDraftError> {
    let mut lines = raw.lines();
    if lines.next().unwrap_or_default() != HEADER {
        return Err(error("QUEST item 설명 초안 header 불일치"));
    }

    let mut entries = Vec::new();
    let mut unit_ids = BTreeSet::new();
    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        if line.is_empty() {
            return Err(error(format!(
                "QUEST item 설명 초안 빈 행: line={line_number}"
            )));
        }
        let columns = line.split('\t').collect::<Vec<_>>();
        let [schema_version, unit_id, protected_source, draft_annotated, status, note] =
            columns.as_slice()
        else {
            return Err(error(format!(
                "QUEST item 설명 초안 열 수 불일치: line={line_number}"
            )));
        };
        if *schema_version != QUEST_ITEM_DESCRIPTION_DRAFT_SCHEMA_VERSION {
            return Err(error(format!(
                "QUEST item 설명 초안 schema 불일치: line={line_number}"
            )));
        }
        if unit_id.is_empty()
            || protected_source.is_empty()
            || draft_annotated.is_empty()
            || note.is_empty()
            || columns.iter().any(|field| {
                field
                    .chars()
                    .any(|glyph| glyph == '\r' || glyph.is_control())
            })
        {
            return Err(error(format!(
                "QUEST item 설명 초안 필수 값이 비었거나 control 문자가 있음: line={line_number}"
            )));
        }
        let status = QuestItemDescriptionDraftStatus::parse(status).ok_or_else(|| {
            error(format!(
                "QUEST item 설명 초안 status 미지원: line={line_number}, value={status}"
            ))
        })?;
        let parsed = parse_annotated(draft_annotated).map_err(|cause| {
            error(format!(
                "QUEST item 설명 annotation 오류: line={line_number}, id={unit_id}: {cause}"
            ))
        })?;
        if !unit_ids.insert((*unit_id).to_owned()) {
            return Err(error(format!(
                "QUEST item 설명 초안 unit ID 중복: {unit_id}"
            )));
        }
        entries.push(QuestItemDescriptionDraftEntry {
            unit_id: (*unit_id).to_owned(),
            protected_source: (*protected_source).to_owned(),
            draft_annotated: (*draft_annotated).to_owned(),
            draft_korean: parsed.literal,
            control_raw: parsed.controls,
            parts: parsed.parts,
            status,
            note: (*note).to_owned(),
        });
    }
    Ok(entries)
}

fn source_controls(unit: &QuestItemTextUnit) -> Vec<Vec<u8>> {
    unit.tokens
        .iter()
        .filter(|token| token.kind == QuestItemTextTokenKind::Control)
        .map(|token| token.raw.clone())
        .collect()
}

fn audit_drafts(
    raw: &str,
    item_units: &QuestItemTextUnitAnalysis,
) -> Result<QuestItemDescriptionDraftAnalysis, QuestItemDescriptionDraftError> {
    let entries = parse_drafts(raw)?;
    let source_descriptions = item_units
        .units
        .iter()
        .filter(|unit| unit.role == QuestItemTextRole::Description)
        .map(|unit| (unit.id.as_str(), unit))
        .collect::<BTreeMap<_, _>>();
    if source_descriptions.len() != item_units.description_units
        || source_descriptions.len() != item_units.unique_records
    {
        return Err(error("QUEST item 설명 source unit 분모 drift"));
    }
    if entries.len() != source_descriptions.len() {
        return Err(error(format!(
            "QUEST item 설명 초안 분모 불일치: source={}, draft={}",
            source_descriptions.len(),
            entries.len()
        )));
    }

    for entry in &entries {
        let source = source_descriptions
            .get(entry.unit_id.as_str())
            .ok_or_else(|| {
                error(format!(
                    "QUEST item 설명 초안이 source unit에 없음: {}",
                    entry.unit_id
                ))
            })?;
        if source.protected_source != entry.protected_source {
            return Err(error(format!(
                "QUEST item 설명 보호 원문 drift: {}",
                entry.unit_id
            )));
        }
        if source_controls(source) != entry.control_raw {
            return Err(error(format!(
                "QUEST item 설명 control raw 순서 drift: {}",
                entry.unit_id
            )));
        }
    }
    let draft_ids = entries
        .iter()
        .map(|entry| entry.unit_id.as_str())
        .collect::<BTreeSet<_>>();
    let source_ids = source_descriptions.keys().copied().collect::<BTreeSet<_>>();
    if draft_ids != source_ids {
        return Err(error(
            "QUEST item 설명 초안 unit 누락 또는 비대상 unit 포함",
        ));
    }

    let used_glyphs = entries
        .iter()
        .flat_map(|entry| entry.draft_korean.chars())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<String>();
    let glyph_occurrences = entries
        .iter()
        .map(|entry| entry.draft_korean.chars().count())
        .sum();
    let control_occurrences = entries.iter().map(|entry| entry.control_raw.len()).sum();
    let manual_line_breaks = entries
        .iter()
        .flat_map(|entry| &entry.control_raw)
        .filter(|raw| raw.first() == Some(&0x04))
        .count();
    let draft_needs_independent_review = entries
        .iter()
        .filter(|entry| {
            entry.status == QuestItemDescriptionDraftStatus::DraftNeedsIndependentReview
        })
        .count();
    let draft_needs_human_choice = entries
        .iter()
        .filter(|entry| entry.status == QuestItemDescriptionDraftStatus::DraftNeedsHumanChoice)
        .count();
    if used_glyphs.is_empty()
        || glyph_occurrences == 0
        || control_occurrences == 0
        || draft_needs_independent_review + draft_needs_human_choice != entries.len()
    {
        return Err(error("QUEST item 설명 초안 집계 drift"));
    }

    Ok(QuestItemDescriptionDraftAnalysis {
        entries,
        used_glyphs,
        glyph_occurrences,
        control_occurrences,
        manual_line_breaks,
        draft_needs_independent_review,
        draft_needs_human_choice,
        translation_approved: false,
        layout_approved: false,
        product_eligible: false,
        asset_sha256: format!("{:x}", Sha256::digest(raw.as_bytes())),
    })
}

/// 버전 관리된 수동 초안을 현재 `ITEM_DAT.SEQ` 설명 unit 전수에 결속한다.
pub fn audit_quest_item_description_drafts(
    item_units: &QuestItemTextUnitAnalysis,
) -> Result<QuestItemDescriptionDraftAnalysis, QuestItemDescriptionDraftError> {
    audit_drafts(DRAFTS, item_units)
}

#[cfg(test)]
#[path = "quest_item_description_drafts_tests.rs"]
mod tests;
