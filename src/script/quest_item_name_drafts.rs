//! `ITEM_DAT.SEQ` 이름의 수동 한국어 초안과 보호 원문 결속.
//!
//! 이 원장은 사람이 작성한 이름 후보를 source-bound unit에 연결한다. 자동화는
//! ID·원문·분모·문자 집합만 검산하며 번역문을 작성하거나 독립 검토·조판·표시
//! 승인을 대신하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::{QuestItemTextRole, QuestItemTextUnitAnalysis};

pub const QUEST_ITEM_NAME_DRAFT_SCHEMA_VERSION: &str = "quest-item-name-draft";

const DRAFTS: &str = include_str!("../../assets/translations/quest_item_name_drafts.tsv");
const HEADER: &str = "schema_version\tunit_id\tprotected_source\tdraft_korean\tstatus\tnote";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestItemNameDraftStatus {
    DraftNeedsIndependentReview,
    DraftNeedsHumanChoice,
}

impl QuestItemNameDraftStatus {
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
pub struct QuestItemNameDraftEntry {
    pub unit_id: String,
    pub protected_source: String,
    pub draft_korean: String,
    pub status: QuestItemNameDraftStatus,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemNameDraftAnalysis {
    pub entries: Vec<QuestItemNameDraftEntry>,
    pub used_glyphs: String,
    pub draft_needs_independent_review: usize,
    pub draft_needs_human_choice: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub product_eligible: bool,
    pub asset_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemNameDraftError(String);

impl fmt::Display for QuestItemNameDraftError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemNameDraftError {}

fn error(message: impl Into<String>) -> QuestItemNameDraftError {
    QuestItemNameDraftError(message.into())
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

fn parse_drafts(raw: &str) -> Result<Vec<QuestItemNameDraftEntry>, QuestItemNameDraftError> {
    let mut lines = raw.lines();
    if lines.next().unwrap_or_default() != HEADER {
        return Err(error("QUEST item name 초안 header 불일치"));
    }

    let mut entries = Vec::new();
    let mut unit_ids = BTreeSet::new();
    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        if line.is_empty() {
            return Err(error(format!(
                "QUEST item name 초안 빈 행: line={line_number}"
            )));
        }
        let columns = line.split('\t').collect::<Vec<_>>();
        let [schema_version, unit_id, protected_source, draft_korean, status, note] =
            columns.as_slice()
        else {
            return Err(error(format!(
                "QUEST item name 초안 열 수 불일치: line={line_number}"
            )));
        };
        if *schema_version != QUEST_ITEM_NAME_DRAFT_SCHEMA_VERSION {
            return Err(error(format!(
                "QUEST item name 초안 schema 불일치: line={line_number}"
            )));
        }
        if unit_id.is_empty()
            || protected_source.is_empty()
            || draft_korean.is_empty()
            || note.is_empty()
            || columns.iter().any(|field| {
                field
                    .chars()
                    .any(|glyph| glyph == '\r' || glyph.is_control())
            })
        {
            return Err(error(format!(
                "QUEST item name 초안 필수 값이 비었거나 control 문자가 있음: line={line_number}"
            )));
        }
        if contains_japanese(draft_korean) {
            return Err(error(format!(
                "QUEST item name 한국어 초안에 일본어가 남음: line={line_number}, id={unit_id}"
            )));
        }
        let status = QuestItemNameDraftStatus::parse(status).ok_or_else(|| {
            error(format!(
                "QUEST item name 초안 status 미지원: line={line_number}, value={status}"
            ))
        })?;
        if !unit_ids.insert((*unit_id).to_owned()) {
            return Err(error(format!(
                "QUEST item name 초안 unit ID 중복: {unit_id}"
            )));
        }
        entries.push(QuestItemNameDraftEntry {
            unit_id: (*unit_id).to_owned(),
            protected_source: (*protected_source).to_owned(),
            draft_korean: (*draft_korean).to_owned(),
            status,
            note: (*note).to_owned(),
        });
    }
    Ok(entries)
}

fn audit_drafts(
    raw: &str,
    item_units: &QuestItemTextUnitAnalysis,
) -> Result<QuestItemNameDraftAnalysis, QuestItemNameDraftError> {
    let entries = parse_drafts(raw)?;
    let source_names = item_units
        .units
        .iter()
        .filter(|unit| unit.role == QuestItemTextRole::Name)
        .map(|unit| (unit.id.as_str(), unit.protected_source.as_str()))
        .collect::<BTreeMap<_, _>>();
    if source_names.len() != item_units.name_units
        || source_names.len() != item_units.unique_records
    {
        return Err(error("QUEST item name source unit 분모 drift"));
    }
    if entries.len() != source_names.len() {
        return Err(error(format!(
            "QUEST item name 초안 분모 불일치: source={}, draft={}",
            source_names.len(),
            entries.len()
        )));
    }

    for entry in &entries {
        let source = source_names.get(entry.unit_id.as_str()).ok_or_else(|| {
            error(format!(
                "QUEST item name 초안이 source unit에 없음: {}",
                entry.unit_id
            ))
        })?;
        if *source != entry.protected_source {
            return Err(error(format!(
                "QUEST item name 보호 원문 drift: {}",
                entry.unit_id
            )));
        }
    }
    let draft_ids = entries
        .iter()
        .map(|entry| entry.unit_id.as_str())
        .collect::<BTreeSet<_>>();
    let source_ids = source_names.keys().copied().collect::<BTreeSet<_>>();
    if draft_ids != source_ids {
        return Err(error(
            "QUEST item name 초안 unit 누락 또는 비대상 unit 포함",
        ));
    }

    let used_glyphs = entries
        .iter()
        .flat_map(|entry| entry.draft_korean.chars())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<String>();
    let draft_needs_independent_review = entries
        .iter()
        .filter(|entry| entry.status == QuestItemNameDraftStatus::DraftNeedsIndependentReview)
        .count();
    let draft_needs_human_choice = entries
        .iter()
        .filter(|entry| entry.status == QuestItemNameDraftStatus::DraftNeedsHumanChoice)
        .count();
    if used_glyphs.is_empty()
        || draft_needs_independent_review + draft_needs_human_choice != entries.len()
    {
        return Err(error("QUEST item name 초안 집계 drift"));
    }

    Ok(QuestItemNameDraftAnalysis {
        entries,
        used_glyphs,
        draft_needs_independent_review,
        draft_needs_human_choice,
        translation_approved: false,
        layout_approved: false,
        product_eligible: false,
        asset_sha256: format!("{:x}", Sha256::digest(raw.as_bytes())),
    })
}

/// 버전 관리된 수동 초안을 현재 `ITEM_DAT.SEQ` 이름 unit 전수에 결속한다.
pub fn audit_quest_item_name_drafts(
    item_units: &QuestItemTextUnitAnalysis,
) -> Result<QuestItemNameDraftAnalysis, QuestItemNameDraftError> {
    audit_drafts(DRAFTS, item_units)
}

#[cfg(test)]
#[path = "quest_item_name_drafts_tests.rs"]
mod tests;
