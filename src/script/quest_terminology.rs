//! QUEST OP21 원문과 용어 후보 근거를 연결한다.
//!
//! 자매 프로젝트의 선택이나 현재 작품에서 만든 직역 후보를 이 프로젝트의
//! 승인으로 간주하지 않는다.
//! version 관리 TSV는 후보·출처·충돌만 보존하고, Rust가 현재 605개
//! stream에서 실제 등장 횟수와 검토 단위를 다시 계산한다. 이 결과는
//! 자동 치환이나 번역 제품 입력이 아니라 사람 승인용 원장이다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::quest_seq_translation_units::QuestSeqTranslationUnitAnalysis;

pub const QUEST_TERMINOLOGY_CANDIDATE_SCHEMA_VERSION: &str = "quest-terminology-candidates-v1";

const QUEST_TERM_CANDIDATES_TSV: &str =
    include_str!("../../assets/translations/quest_term_candidates.tsv");
const LEDGER_HEADER: &str = "term_id\tcategory\tjp\tcandidate_ko\tapproved_ko\tevidence_project\t\
     evidence_path\tevidence_state\tnotes";

/// 용어가 쓰이는 의미 범주.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuestTerminologyCategory {
    Character,
    Honorific,
    Item,
    Location,
    GameTerm,
    WorldTerm,
}

impl QuestTerminologyCategory {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Character => "character",
            Self::Honorific => "honorific",
            Self::Item => "item",
            Self::Location => "location",
            Self::GameTerm => "game-term",
            Self::WorldTerm => "world-term",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "character" => Some(Self::Character),
            "honorific" => Some(Self::Honorific),
            "item" => Some(Self::Item),
            "location" => Some(Self::Location),
            "game-term" => Some(Self::GameTerm),
            "world-term" => Some(Self::WorldTerm),
            _ => None,
        }
    }
}

/// 해당 표기가 가진 근거의 출처와 확정 수준.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuestTerminologyEvidenceState {
    CurrentSourceCandidate,
    SameTitleReference,
    SeriesApproved,
    ProjectDecision,
    OfficialReference,
    SiblingReference,
}

impl QuestTerminologyEvidenceState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::CurrentSourceCandidate => "current-source-candidate",
            Self::SameTitleReference => "same-title-reference",
            Self::SeriesApproved => "series-approved",
            Self::ProjectDecision => "project-decision",
            Self::OfficialReference => "official-reference",
            Self::SiblingReference => "sibling-reference",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "current-source-candidate" => Some(Self::CurrentSourceCandidate),
            "same-title-reference" => Some(Self::SameTitleReference),
            "series-approved" => Some(Self::SeriesApproved),
            "project-decision" => Some(Self::ProjectDecision),
            "official-reference" => Some(Self::OfficialReference),
            "sibling-reference" => Some(Self::SiblingReference),
            _ => None,
        }
    }
}

/// 이 프로젝트에서 내린 현재 결정 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestTerminologyDecision {
    PendingHumanApproval,
    ConflictingSiblingCandidates,
    HumanApproved,
}

impl QuestTerminologyDecision {
    pub const fn label(self) -> &'static str {
        match self {
            Self::PendingHumanApproval => "pending-human-approval",
            Self::ConflictingSiblingCandidates => "conflict-needs-human-decision",
            Self::HumanApproved => "human-approved",
        }
    }
}

/// 후보 하나를 지지하는 자매 또는 현재 작품의 근거.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct QuestTerminologyEvidence {
    pub project: String,
    pub path: String,
    pub state: QuestTerminologyEvidenceState,
    pub candidate_ko: String,
    pub notes: String,
}

/// 현재 QUEST 원문에 실제로 나타난 용어 후보 한 항목.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTerminologyCandidate {
    pub id: String,
    pub category: QuestTerminologyCategory,
    pub protected_source: String,
    pub source_occurrences: usize,
    pub matched_units: usize,
    pub candidate_ko: Vec<String>,
    pub approved_ko: Option<String>,
    pub decision: QuestTerminologyDecision,
    pub evidence: Vec<QuestTerminologyEvidence>,
}

/// 등장 용어와 자매 프로젝트 근거를 합친 사람 검토 원장.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTerminologyCandidateAnalysis {
    pub terms: Vec<QuestTerminologyCandidate>,
    pub evidence_rows: usize,
    pub pending_terms: usize,
    pub conflicting_terms: usize,
    pub approved_terms: usize,
}

/// 용어 후보 입력·현재 QUEST 원문·결정적 report 계약 불일치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTerminologyError(String);

impl fmt::Display for QuestTerminologyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestTerminologyError {}

fn error(message: impl Into<String>) -> QuestTerminologyError {
    QuestTerminologyError(message.into())
}

fn has_japanese_character(value: &str) -> bool {
    value.chars().any(|character| {
        matches!(
            character,
            '\u{3040}'..='\u{30ff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{4e00}'..='\u{9fff}'
                | '\u{ff66}'..='\u{ff9f}'
        )
    })
}

fn has_hangul_syllable(value: &str) -> bool {
    value
        .chars()
        .any(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
}

fn valid_term_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._".contains(&byte))
}

fn valid_project(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphabetic() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn valid_relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && value
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

#[derive(Debug)]
struct ParsedEvidenceRow {
    id: String,
    category: QuestTerminologyCategory,
    protected_source: String,
    candidate_ko: String,
    approved_ko: Option<String>,
    evidence: QuestTerminologyEvidence,
}

fn parse_ledger(input: &str) -> Result<Vec<ParsedEvidenceRow>, QuestTerminologyError> {
    if !input.ends_with('\n') || input.contains('\r') {
        return Err(error(
            "QUEST 용어 후보 TSV는 LF 하나로 끝나고 CR이 없어야 함",
        ));
    }
    let mut lines = input.lines();
    if lines.next() != Some(LEDGER_HEADER) {
        return Err(error("QUEST 용어 후보 TSV header 불일치"));
    }

    let mut rows = Vec::new();
    let mut evidence_keys = BTreeSet::new();
    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        let fields = line.split('\t').collect::<Vec<_>>();
        let [id, category, protected_source, candidate_ko, approved_ko, evidence_project, evidence_path, evidence_state, notes] =
            fields.as_slice()
        else {
            return Err(error(format!(
                "QUEST 용어 후보 TSV {line_number}행 열 수 불일치"
            )));
        };
        if !valid_term_id(id) {
            return Err(error(format!(
                "QUEST 용어 후보 TSV {line_number}행 term ID 오류"
            )));
        }
        let category = QuestTerminologyCategory::parse(category)
            .ok_or_else(|| error(format!("QUEST 용어 후보 TSV {line_number}행 category 오류")))?;
        if !has_japanese_character(protected_source) {
            return Err(error(format!(
                "QUEST 용어 후보 TSV {line_number}행 일본어 원문 누락"
            )));
        }
        if !has_hangul_syllable(candidate_ko) {
            return Err(error(format!(
                "QUEST 용어 후보 TSV {line_number}행 한글 후보 누락"
            )));
        }
        let approved_ko = if approved_ko.is_empty() {
            None
        } else {
            if !has_hangul_syllable(approved_ko) {
                return Err(error(format!(
                    "QUEST 용어 후보 TSV {line_number}행 승인 한글 누락"
                )));
            }
            Some((*approved_ko).to_owned())
        };
        if !valid_project(evidence_project) || !valid_relative_path(evidence_path) {
            return Err(error(format!(
                "QUEST 용어 후보 TSV {line_number}행 근거 경로 오류"
            )));
        }
        let state = QuestTerminologyEvidenceState::parse(evidence_state).ok_or_else(|| {
            error(format!(
                "QUEST 용어 후보 TSV {line_number}행 evidence state 오류"
            ))
        })?;
        if notes.is_empty() {
            return Err(error(format!(
                "QUEST 용어 후보 TSV {line_number}행 근거 설명 누락"
            )));
        }
        let evidence_key = (
            (*id).to_owned(),
            (*candidate_ko).to_owned(),
            (*evidence_project).to_owned(),
            (*evidence_path).to_owned(),
        );
        if !evidence_keys.insert(evidence_key) {
            return Err(error(format!(
                "QUEST 용어 후보 TSV {line_number}행 근거 중복"
            )));
        }
        rows.push(ParsedEvidenceRow {
            id: (*id).to_owned(),
            category,
            protected_source: (*protected_source).to_owned(),
            candidate_ko: (*candidate_ko).to_owned(),
            approved_ko,
            evidence: QuestTerminologyEvidence {
                project: (*evidence_project).to_owned(),
                path: (*evidence_path).to_owned(),
                state,
                candidate_ko: (*candidate_ko).to_owned(),
                notes: (*notes).to_owned(),
            },
        });
    }
    if rows.is_empty() {
        return Err(error("QUEST 용어 후보 TSV가 비어 있음"));
    }
    Ok(rows)
}

#[derive(Debug)]
struct TermAccumulator {
    category: QuestTerminologyCategory,
    protected_source: String,
    approved_ko: Option<String>,
    candidate_ko: BTreeSet<String>,
    evidence: Vec<QuestTerminologyEvidence>,
}

fn audit_from_ledger(
    translation_units: &QuestSeqTranslationUnitAnalysis,
    ledger: &str,
) -> Result<QuestTerminologyCandidateAnalysis, QuestTerminologyError> {
    let rows = parse_ledger(ledger)?;
    let evidence_rows = rows.len();
    let mut grouped = BTreeMap::<String, TermAccumulator>::new();
    for row in rows {
        let entry = grouped
            .entry(row.id.clone())
            .or_insert_with(|| TermAccumulator {
                category: row.category,
                protected_source: row.protected_source.clone(),
                approved_ko: row.approved_ko.clone(),
                candidate_ko: BTreeSet::new(),
                evidence: Vec::new(),
            });
        if entry.category != row.category
            || entry.protected_source != row.protected_source
            || entry.approved_ko != row.approved_ko
        {
            return Err(error(format!(
                "{}: 근거 행 사이 term metadata 불일치",
                row.id
            )));
        }
        entry.candidate_ko.insert(row.candidate_ko);
        entry.evidence.push(row.evidence);
    }

    let mut terms = Vec::with_capacity(grouped.len());
    for (id, mut entry) in grouped {
        if entry.candidate_ko.is_empty() || entry.evidence.is_empty() {
            return Err(error(format!("{id}: 후보 또는 근거가 없음")));
        }
        if let Some(approved) = entry.approved_ko.as_deref() {
            if !entry.candidate_ko.contains(approved) {
                return Err(error(format!("{id}: 승인 표기가 후보 집합에 없음")));
            }
        }
        let source_occurrences = translation_units
            .units
            .iter()
            .map(|unit| {
                unit.protected_source
                    .match_indices(&entry.protected_source)
                    .count()
            })
            .sum();
        let matched_units = translation_units
            .units
            .iter()
            .filter(|unit| unit.protected_source.contains(&entry.protected_source))
            .count();
        if source_occurrences == 0 || matched_units == 0 {
            return Err(error(format!("{id}: 현재 QUEST OP21 원문에 등장하지 않음")));
        }
        entry.evidence.sort();
        let candidate_ko = entry.candidate_ko.into_iter().collect::<Vec<_>>();
        let decision = if entry.approved_ko.is_some() {
            QuestTerminologyDecision::HumanApproved
        } else if candidate_ko.len() == 1 {
            QuestTerminologyDecision::PendingHumanApproval
        } else {
            QuestTerminologyDecision::ConflictingSiblingCandidates
        };
        terms.push(QuestTerminologyCandidate {
            id,
            category: entry.category,
            protected_source: entry.protected_source,
            source_occurrences,
            matched_units,
            candidate_ko,
            approved_ko: entry.approved_ko,
            decision,
            evidence: entry.evidence,
        });
    }

    let pending_terms = terms
        .iter()
        .filter(|term| term.decision == QuestTerminologyDecision::PendingHumanApproval)
        .count();
    let conflicting_terms = terms
        .iter()
        .filter(|term| term.decision == QuestTerminologyDecision::ConflictingSiblingCandidates)
        .count();
    let approved_terms = terms
        .iter()
        .filter(|term| term.decision == QuestTerminologyDecision::HumanApproved)
        .count();
    Ok(QuestTerminologyCandidateAnalysis {
        terms,
        evidence_rows,
        pending_terms,
        conflicting_terms,
        approved_terms,
    })
}

/// version 관리 후보와 현재 QUEST OP21 원문을 결합한다.
pub fn audit_quest_terminology_candidates(
    translation_units: &QuestSeqTranslationUnitAnalysis,
) -> Result<QuestTerminologyCandidateAnalysis, QuestTerminologyError> {
    audit_from_ledger(translation_units, QUEST_TERM_CANDIDATES_TSV)
}

/// 지원 원본과 현재 version 관리 용어 원장을 결합해 매번 검증한다.
pub fn verify_box_quest_terminology_candidates(
    translation_units: &QuestSeqTranslationUnitAnalysis,
) -> Result<QuestTerminologyCandidateAnalysis, QuestTerminologyError> {
    audit_quest_terminology_candidates(translation_units)
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

/// 현재 원문 등장 횟수와 후보·충돌·근거를 함께 내보내는 검토 TSV.
pub fn render_quest_terminology_candidates_tsv(
    analysis: &QuestTerminologyCandidateAnalysis,
) -> String {
    let mut output = String::from(
        "schema_version\tterm_id\tcategory\tprotected_source\tsource_occurrences\t\
         matched_units\tcandidate_ko\tapproved_ko\tdecision\tevidence_rows\t\
         evidence\tnotes\n",
    );
    for term in &analysis.terms {
        let evidence = term
            .evidence
            .iter()
            .map(|item| {
                format!(
                    "{}:{}[{}]=>{}",
                    item.project,
                    item.path,
                    item.state.label(),
                    item.candidate_ko
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        let notes = term
            .evidence
            .iter()
            .map(|item| item.notes.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(" | ");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            QUEST_TERMINOLOGY_CANDIDATE_SCHEMA_VERSION,
            term.id,
            term.category.label(),
            escape_tsv(&term.protected_source),
            term.source_occurrences,
            term.matched_units,
            escape_tsv(&term.candidate_ko.join(" | ")),
            escape_tsv(term.approved_ko.as_deref().unwrap_or("")),
            term.decision.label(),
            term.evidence.len(),
            escape_tsv(&evidence),
            escape_tsv(&notes),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

/// 현재 용어 원장과 원문 결합 결과의 lineage digest.
pub fn quest_terminology_report_sha256(analysis: &QuestTerminologyCandidateAnalysis) -> String {
    format!(
        "{:x}",
        Sha256::digest(render_quest_terminology_candidates_tsv(analysis).as_bytes())
    )
}

#[cfg(test)]
#[path = "quest_terminology_tests.rs"]
mod tests;
