//! QUEST 장면 batch의 원문·초벌·독립 검토·용어 후보를 결합한 사람 결정 자료.
//!
//! 이 모듈은 검토 결과나 자매 프로젝트 후보를 승인으로 바꾸지 않는다.
//! 선택한 batch의 모든 번역 단위가 독립 검토됐을 때만 한 행씩 결합하고,
//! 사람이 선택해야 할 문장과 용어를 별도 열로 드러낸다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::quest_terminology::{
    quest_terminology_report_sha256, QuestTerminologyCandidate, QuestTerminologyCandidateAnalysis,
    QuestTerminologyDecision,
};
use super::quest_translation_batches::QuestTranslationBatchRole;
use super::quest_translation_review::{
    QuestTranslationIndependentReviewStatus, QuestTranslationReviewAnalysis,
    QuestTranslationReviewEntry,
};

pub const QUEST_TRANSLATION_DECISION_PACKET_SCHEMA_VERSION: &str =
    "quest-translation-decision-packet-v2";
pub const QUEST_TRANSLATION_DECISION_WORKBOOK_SCHEMA_VERSION: &str =
    "quest-translation-decision-workbook-v1";
const BOX_QUEST_FIRST_SCENE_BATCH_ID: &str = "quest-town00-puyoman-results-001";

/// 독립 검토 상태에 따라 사람이 비교해야 할 선택 범위.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestTranslationReviewAction {
    ApproveReviewedTextOrRevise,
    ChooseDraftReviewedTextOrRevise,
    ResolveDependenciesAndApprove,
}

impl QuestTranslationReviewAction {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ApproveReviewedTextOrRevise => "approve-reviewed-or-revise",
            Self::ChooseDraftReviewedTextOrRevise => "choose-draft-review-or-revise",
            Self::ResolveDependenciesAndApprove => "resolve-dependencies-and-approve",
        }
    }
}

/// 장면 batch의 번역 단위 하나와 그 단위에 실제로 등장하는 용어 후보.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationDecisionEntry {
    pub review: QuestTranslationReviewEntry,
    pub terms: Vec<QuestTerminologyCandidate>,
    pub review_action: QuestTranslationReviewAction,
}

/// 장면 batch 전수와 사람 결정 대기 상태를 결합한 보고서 입력.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationDecisionAnalysis {
    pub batch_id: String,
    pub review_asset_sha256: String,
    pub translation_batch_report_sha256: String,
    pub terminology_report_sha256: String,
    pub entries: Vec<QuestTranslationDecisionEntry>,
    pub affected_terms: usize,
    pub review_agreements: usize,
    pub review_proposals: usize,
    pub review_human_decisions: usize,
    pub pending_term_decisions: usize,
    pub conflicting_term_decisions: usize,
    pub approved_term_decisions: usize,
    pub pending_human_decisions: usize,
    pub build_eligible_entries: usize,
}

/// 여러 장면에서 같은 사람 판단을 요구하는 결정 의존성의 영향 범위.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationDecisionDependency {
    pub id: String,
    pub batch_ids: Vec<String>,
    pub stream_ids: Vec<String>,
    pub occurrences: usize,
}

/// 용어·반복 결정·장면별 번역안을 한 파일에서 검토하기 위한 전역 자료.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationDecisionWorkbook {
    pub batch_decisions: Vec<QuestTranslationDecisionAnalysis>,
    pub terms: Vec<QuestTerminologyCandidate>,
    pub dependencies: Vec<QuestTranslationDecisionDependency>,
    pub entries: usize,
    pub review_agreements: usize,
    pub review_proposals: usize,
    pub review_human_decisions: usize,
    pub pending_term_decisions: usize,
    pub conflicting_term_decisions: usize,
    pub approved_term_decisions: usize,
    pub dependency_occurrences: usize,
    pub build_eligible_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationDecisionError(String);

impl fmt::Display for QuestTranslationDecisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestTranslationDecisionError {}

fn error(message: impl Into<String>) -> QuestTranslationDecisionError {
    QuestTranslationDecisionError(message.into())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn review_action(status: QuestTranslationIndependentReviewStatus) -> QuestTranslationReviewAction {
    match status {
        QuestTranslationIndependentReviewStatus::ReviewAgrees => {
            QuestTranslationReviewAction::ApproveReviewedTextOrRevise
        }
        QuestTranslationIndependentReviewStatus::ReviewProposesChange => {
            QuestTranslationReviewAction::ChooseDraftReviewedTextOrRevise
        }
        QuestTranslationIndependentReviewStatus::NeedsHumanDecision => {
            QuestTranslationReviewAction::ResolveDependenciesAndApprove
        }
    }
}

fn terminology_by_id(
    terminology: &QuestTerminologyCandidateAnalysis,
) -> Result<BTreeMap<&str, &QuestTerminologyCandidate>, QuestTranslationDecisionError> {
    let terms = terminology
        .terms
        .iter()
        .map(|term| (term.id.as_str(), term))
        .collect::<BTreeMap<_, _>>();
    if terms.len() != terminology.terms.len() {
        return Err(error("QUEST 용어 후보 ID가 중복됨"));
    }
    Ok(terms)
}

fn combine_decision_entry(
    review: &QuestTranslationReviewEntry,
    terms_by_id: &BTreeMap<&str, &QuestTerminologyCandidate>,
) -> Result<QuestTranslationDecisionEntry, QuestTranslationDecisionError> {
    if review.batch_entry.role != QuestTranslationBatchRole::Translate {
        return Err(error(format!(
            "{}: QUEST batch 결정 자료에 번역 대상이 아닌 행이 있음",
            review.batch_entry.stream_id
        )));
    }
    let independent_status = review.independent_review_status.ok_or_else(|| {
        error(format!(
            "{}: QUEST batch 독립 검토가 완료되지 않음",
            review.batch_entry.stream_id
        ))
    })?;
    if review.reviewed_korean.is_empty() {
        return Err(error(format!(
            "{}: QUEST batch 독립 검토 번역문이 비어 있음",
            review.batch_entry.stream_id
        )));
    }
    if review.batch_entry.matched_term_ids.len() != review.batch_entry.matched_term_decisions.len()
    {
        return Err(error(format!(
            "{}: 용어 ID와 결정 snapshot 수가 다름",
            review.batch_entry.stream_id
        )));
    }

    let mut terms = Vec::with_capacity(review.batch_entry.matched_term_ids.len());
    for (term_id, snapshot_decision) in review
        .batch_entry
        .matched_term_ids
        .iter()
        .zip(&review.batch_entry.matched_term_decisions)
    {
        let term = terms_by_id.get(term_id.as_str()).copied().ok_or_else(|| {
            error(format!(
                "{}: 결정 자료에 필요한 용어가 없음: {term_id}",
                review.batch_entry.stream_id
            ))
        })?;
        if term.decision.label() != *snapshot_decision {
            return Err(error(format!(
                "{}: 용어 결정 snapshot drift: {term_id}",
                review.batch_entry.stream_id
            )));
        }
        terms.push(term.clone());
    }

    Ok(QuestTranslationDecisionEntry {
        review: review.clone(),
        terms,
        review_action: review_action(independent_status),
    })
}

/// 선택한 장면 batch의 번역 단위를 사람 결정용 행으로 결합한다.
pub fn audit_quest_translation_batch_decisions(
    reviews: &QuestTranslationReviewAnalysis,
    terminology: &QuestTerminologyCandidateAnalysis,
    batch_id: &str,
) -> Result<QuestTranslationDecisionAnalysis, QuestTranslationDecisionError> {
    if batch_id.is_empty() {
        return Err(error("QUEST 결정 대상 batch ID가 비어 있음"));
    }
    let terms_by_id = terminology_by_id(terminology)?;
    let mut entries = reviews
        .entries
        .iter()
        .filter(|entry| {
            entry.batch_entry.batch_id == batch_id
                && entry.batch_entry.role == QuestTranslationBatchRole::Translate
        })
        .map(|entry| combine_decision_entry(entry, &terms_by_id))
        .collect::<Result<Vec<_>, _>>()?;
    if entries.is_empty() {
        return Err(error(format!("QUEST 결정 대상 batch가 없음: {batch_id}")));
    }
    entries.sort_by_key(|entry| entry.review.batch_entry.scene_order);

    let mut stream_ids = BTreeSet::new();
    let mut affected_term_ids = BTreeSet::new();
    let mut review_asset_hashes = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let expected_order = index + 1;
        if entry.review.batch_entry.scene_order != expected_order {
            return Err(error(format!(
                "QUEST batch 논리 순서가 연속이 아님: batch={batch_id}, \
                 expected={expected_order}, actual={}",
                entry.review.batch_entry.scene_order
            )));
        }
        if !stream_ids.insert(entry.review.batch_entry.stream_id.as_str()) {
            return Err(error(format!(
                "QUEST batch stream ID 중복: {}",
                entry.review.batch_entry.stream_id
            )));
        }
        let review_asset_sha256 = entry.review.review_asset_sha256.as_deref().ok_or_else(|| {
            error(format!(
                "{}: QUEST batch 독립 검토 자산 hash가 없음",
                entry.review.batch_entry.stream_id
            ))
        })?;
        review_asset_hashes.insert(review_asset_sha256);
        affected_term_ids.extend(entry.terms.iter().map(|term| term.id.as_str()));
    }
    if review_asset_hashes.len() != 1 {
        return Err(error(format!(
            "QUEST batch 결정 자료가 여러 독립 검토 자산에 걸침: {batch_id}"
        )));
    }
    let review_asset_sha256 = review_asset_hashes
        .into_iter()
        .next()
        .expect("결정 자료에는 검토 행이 하나 이상 있음")
        .to_owned();

    let affected_terms = affected_term_ids.len();
    let review_agreements = entries
        .iter()
        .filter(|entry| {
            entry.review.independent_review_status
                == Some(QuestTranslationIndependentReviewStatus::ReviewAgrees)
        })
        .count();
    let review_proposals = entries
        .iter()
        .filter(|entry| {
            entry.review.independent_review_status
                == Some(QuestTranslationIndependentReviewStatus::ReviewProposesChange)
        })
        .count();
    let review_human_decisions = entries
        .iter()
        .filter(|entry| {
            entry.review.independent_review_status
                == Some(QuestTranslationIndependentReviewStatus::NeedsHumanDecision)
        })
        .count();
    let affected_term_decisions = affected_term_ids
        .iter()
        .map(|term_id| {
            terms_by_id
                .get(term_id)
                .expect("결합된 용어 ID는 원장에 존재함")
                .decision
        })
        .collect::<Vec<_>>();
    let pending_term_decisions = affected_term_decisions
        .iter()
        .filter(|&&decision| decision != QuestTerminologyDecision::HumanApproved)
        .count();
    let conflicting_term_decisions = affected_term_decisions
        .iter()
        .filter(|&&decision| decision == QuestTerminologyDecision::ConflictingSiblingCandidates)
        .count();
    let approved_term_decisions = affected_term_decisions
        .iter()
        .filter(|&&decision| decision == QuestTerminologyDecision::HumanApproved)
        .count();

    Ok(QuestTranslationDecisionAnalysis {
        batch_id: batch_id.to_owned(),
        review_asset_sha256,
        translation_batch_report_sha256: reviews.translation_batch_report_sha256.clone(),
        terminology_report_sha256: quest_terminology_report_sha256(terminology),
        pending_human_decisions: entries.len(),
        build_eligible_entries: 0,
        entries,
        affected_terms,
        review_agreements,
        review_proposals,
        review_human_decisions,
        pending_term_decisions,
        conflicting_term_decisions,
        approved_term_decisions,
    })
}

/// 독립 검토 자산이 있는 모든 batch를 사람 결정 자료로 결합한다.
pub fn audit_quest_reviewed_batch_decisions(
    reviews: &QuestTranslationReviewAnalysis,
    terminology: &QuestTerminologyCandidateAnalysis,
) -> Result<Vec<QuestTranslationDecisionAnalysis>, QuestTranslationDecisionError> {
    let batch_ids = reviews
        .entries
        .iter()
        .filter(|entry| entry.review_asset_sha256.is_some())
        .map(|entry| entry.batch_entry.batch_id.as_str())
        .collect::<BTreeSet<_>>();
    if batch_ids.len() != reviews.review_assets {
        return Err(error(format!(
            "QUEST 독립 검토 자산과 결정 대상 batch 수가 다름: assets={}, batches={}",
            reviews.review_assets,
            batch_ids.len()
        )));
    }
    batch_ids
        .into_iter()
        .map(|batch_id| audit_quest_translation_batch_decisions(reviews, terminology, batch_id))
        .collect()
}

/// 모든 독립 검토 batch와 용어·반복 결정을 한 사람 검토 범위로 결합한다.
pub fn audit_quest_translation_decision_workbook(
    reviews: &QuestTranslationReviewAnalysis,
    terminology: &QuestTerminologyCandidateAnalysis,
) -> Result<QuestTranslationDecisionWorkbook, QuestTranslationDecisionError> {
    let batch_decisions = audit_quest_reviewed_batch_decisions(reviews, terminology)?;
    let mut stream_ids = BTreeSet::new();
    let mut dependencies = BTreeMap::<String, (BTreeSet<String>, BTreeSet<String>, usize)>::new();
    let mut entries = 0;
    let mut review_agreements = 0;
    let mut review_proposals = 0;
    let mut review_human_decisions = 0;

    for batch in &batch_decisions {
        entries += batch.entries.len();
        review_agreements += batch.review_agreements;
        review_proposals += batch.review_proposals;
        review_human_decisions += batch.review_human_decisions;
        for entry in &batch.entries {
            let stream_id = entry.review.batch_entry.stream_id.as_str();
            if !stream_ids.insert(stream_id) {
                return Err(error(format!(
                    "QUEST 전역 결정 자료의 stream ID가 중복됨: {stream_id}"
                )));
            }
            let mut entry_dependencies = BTreeSet::new();
            for dependency in &entry.review.decision_dependencies {
                if dependency.is_empty() {
                    return Err(error(format!(
                        "{stream_id}: QUEST 사람 결정 의존성 ID가 비어 있음"
                    )));
                }
                if !entry_dependencies.insert(dependency.as_str()) {
                    return Err(error(format!(
                        "{stream_id}: QUEST 사람 결정 의존성이 중복됨: {dependency}"
                    )));
                }
                let (batch_ids, dependency_stream_ids, occurrences) =
                    dependencies.entry(dependency.clone()).or_default();
                batch_ids.insert(batch.batch_id.clone());
                dependency_stream_ids.insert(stream_id.to_owned());
                *occurrences += 1;
            }
        }
    }

    if entries != reviews.reviewed_entries || entries != stream_ids.len() {
        return Err(error(format!(
            "QUEST 전역 결정 자료와 독립 검토 분모가 다름: decisions={entries}, \
             reviewed={}, unique_streams={}",
            reviews.reviewed_entries,
            stream_ids.len()
        )));
    }

    let dependencies = dependencies
        .into_iter()
        .map(
            |(id, (batch_ids, stream_ids, occurrences))| QuestTranslationDecisionDependency {
                id,
                batch_ids: batch_ids.into_iter().collect(),
                stream_ids: stream_ids.into_iter().collect(),
                occurrences,
            },
        )
        .collect::<Vec<_>>();
    let dependency_occurrences = dependencies
        .iter()
        .map(|dependency| dependency.occurrences)
        .sum();

    Ok(QuestTranslationDecisionWorkbook {
        batch_decisions,
        terms: terminology.terms.clone(),
        dependencies,
        entries,
        review_agreements,
        review_proposals,
        review_human_decisions,
        pending_term_decisions: terminology.pending_terms,
        conflicting_term_decisions: terminology.conflicting_terms,
        approved_term_decisions: terminology.approved_terms,
        dependency_occurrences,
        build_eligible_entries: 0,
    })
}

/// 현재 독립 검토와 용어 원장을 전역 사람 결정 자료로 매번 결합한다.
pub fn verify_box_quest_translation_decision_workbook(
    reviews: &QuestTranslationReviewAnalysis,
    terminology: &QuestTerminologyCandidateAnalysis,
) -> Result<QuestTranslationDecisionWorkbook, QuestTranslationDecisionError> {
    audit_quest_translation_decision_workbook(reviews, terminology)
}

/// 현재 입력에서 첫 장면 사람 결정 자료를 매번 결합한다.
pub fn verify_box_quest_first_scene_decisions(
    reviews: &QuestTranslationReviewAnalysis,
    terminology: &QuestTerminologyCandidateAnalysis,
) -> Result<QuestTranslationDecisionAnalysis, QuestTranslationDecisionError> {
    audit_quest_translation_batch_decisions(reviews, terminology, BOX_QUEST_FIRST_SCENE_BATCH_ID)
}

fn render_term_choices(terms: &[QuestTerminologyCandidate]) -> String {
    terms
        .iter()
        .map(|term| {
            format!(
                "{}[{}]=>{}[{}]",
                term.id,
                term.protected_source,
                term.candidate_ko.join(" | "),
                term.decision.label(),
            )
        })
        .collect::<Vec<_>>()
        .join(" || ")
}

fn render_term_evidence(terms: &[QuestTerminologyCandidate]) -> String {
    terms
        .iter()
        .flat_map(|term| {
            term.evidence.iter().map(|evidence| {
                format!(
                    "{}=>{}:{}[{}]=>{}",
                    term.id,
                    evidence.project,
                    evidence.path,
                    evidence.state.label(),
                    evidence.candidate_ko,
                )
            })
        })
        .collect::<Vec<_>>()
        .join(" || ")
}

fn render_workbook_term_evidence(term: &QuestTerminologyCandidate) -> String {
    term.evidence
        .iter()
        .map(|evidence| {
            format!(
                "{}:{}[{}]=>{} ({})",
                evidence.project,
                evidence.path,
                evidence.state.label(),
                evidence.candidate_ko,
                evidence.notes,
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

const DECISION_WORKBOOK_COLUMNS: [&str; 36] = [
    "schema_version",
    "row_kind",
    "decision_id",
    "decision_status",
    "source_occurrences",
    "affected_entries",
    "affected_batches",
    "batch_ids",
    "stream_ids",
    "term_category",
    "protected_source",
    "candidate_ko",
    "approved_ko",
    "evidence",
    "scene_id",
    "scene_title",
    "scene_order",
    "branch",
    "batch_id",
    "stream_id",
    "speaker",
    "addressee",
    "context_note",
    "draft_ko",
    "independent_review_status",
    "review_ko",
    "review_notes",
    "decision_dependencies",
    "term_choices",
    "term_evidence",
    "control_signature",
    "runtime_evidence",
    "human_action",
    "human_selected_ko",
    "human_decision_notes",
    "build_eligible",
];

fn append_workbook_row(output: &mut String, row: [String; 36]) {
    writeln!(output, "{}", row.join("\t")).expect("String write는 실패하지 않음");
}

/// 모든 용어·반복 결정과 원문·초벌·검토안을 한 TSV에서 대조한다.
pub fn render_quest_translation_decision_workbook_tsv(
    workbook: &QuestTranslationDecisionWorkbook,
) -> String {
    let mut output = format!("{}\n", DECISION_WORKBOOK_COLUMNS.join("\t"));
    let term_usage = workbook
        .batch_decisions
        .iter()
        .flat_map(|batch| {
            batch.entries.iter().flat_map(move |entry| {
                entry.terms.iter().map(move |term| {
                    (
                        term.id.as_str(),
                        batch.batch_id.as_str(),
                        entry.review.batch_entry.stream_id.as_str(),
                    )
                })
            })
        })
        .fold(
            BTreeMap::<&str, (BTreeSet<&str>, BTreeSet<&str>)>::new(),
            |mut usage, (term_id, batch_id, stream_id)| {
                let (batch_ids, stream_ids) = usage.entry(term_id).or_default();
                batch_ids.insert(batch_id);
                stream_ids.insert(stream_id);
                usage
            },
        );

    for term in &workbook.terms {
        let (batch_ids, stream_ids) = term_usage
            .get(term.id.as_str())
            .cloned()
            .unwrap_or_default();
        append_workbook_row(
            &mut output,
            [
                QUEST_TRANSLATION_DECISION_WORKBOOK_SCHEMA_VERSION.to_owned(),
                "terminology-decision".to_owned(),
                term.id.clone(),
                term.decision.label().to_owned(),
                term.source_occurrences.to_string(),
                term.matched_units.to_string(),
                batch_ids.len().to_string(),
                batch_ids.into_iter().collect::<Vec<_>>().join(","),
                stream_ids.into_iter().collect::<Vec<_>>().join(","),
                term.category.label().to_owned(),
                escape_tsv(&term.protected_source),
                escape_tsv(&term.candidate_ko.join(" | ")),
                escape_tsv(term.approved_ko.as_deref().unwrap_or("")),
                escape_tsv(&render_workbook_term_evidence(term)),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                "approve-term-candidate-or-revise".to_owned(),
                String::new(),
                String::new(),
                "no".to_owned(),
            ],
        );
    }

    for dependency in &workbook.dependencies {
        append_workbook_row(
            &mut output,
            [
                QUEST_TRANSLATION_DECISION_WORKBOOK_SCHEMA_VERSION.to_owned(),
                "shared-dependency-decision".to_owned(),
                dependency.id.clone(),
                "pending-human-approval".to_owned(),
                String::new(),
                dependency.stream_ids.len().to_string(),
                dependency.batch_ids.len().to_string(),
                dependency.batch_ids.join(","),
                dependency.stream_ids.join(","),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                dependency.id.clone(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                "resolve-shared-dependency".to_owned(),
                String::new(),
                String::new(),
                "no".to_owned(),
            ],
        );
    }

    for batch in &workbook.batch_decisions {
        for entry in &batch.entries {
            let review = &entry.review;
            append_workbook_row(
                &mut output,
                [
                    QUEST_TRANSLATION_DECISION_WORKBOOK_SCHEMA_VERSION.to_owned(),
                    "translation-entry-decision".to_owned(),
                    review.batch_entry.stream_id.clone(),
                    "pending-human-approval".to_owned(),
                    String::new(),
                    "1".to_owned(),
                    "1".to_owned(),
                    batch.batch_id.clone(),
                    review.batch_entry.stream_id.clone(),
                    String::new(),
                    escape_tsv(&review.batch_entry.protected_source),
                    String::new(),
                    String::new(),
                    String::new(),
                    review.batch_entry.scene_id.clone(),
                    escape_tsv(&review.batch_entry.scene_title),
                    review.batch_entry.scene_order.to_string(),
                    review.batch_entry.branch.clone(),
                    batch.batch_id.clone(),
                    review.batch_entry.stream_id.clone(),
                    escape_tsv(&review.batch_entry.speaker),
                    escape_tsv(&review.batch_entry.addressee),
                    escape_tsv(&review.batch_entry.context_note),
                    escape_tsv(&review.batch_entry.korean_draft),
                    review
                        .independent_review_status
                        .expect("결정 자료는 독립 검토 완료 행만 포함함")
                        .label()
                        .to_owned(),
                    escape_tsv(&review.reviewed_korean),
                    escape_tsv(&review.review_notes),
                    review.decision_dependencies.join(";"),
                    escape_tsv(&render_term_choices(&entry.terms)),
                    escape_tsv(&render_term_evidence(&entry.terms)),
                    review.batch_entry.control_signature.clone(),
                    review
                        .batch_entry
                        .runtime_evidence
                        .unwrap_or("static-only")
                        .to_owned(),
                    entry.review_action.label().to_owned(),
                    String::new(),
                    String::new(),
                    "no".to_owned(),
                ],
            );
        }
    }
    output
}

/// 현재 검토·용어 결합 결과로 만든 전역 결정 자료의 lineage digest.
pub fn quest_translation_decision_workbook_report_sha256(
    workbook: &QuestTranslationDecisionWorkbook,
) -> String {
    sha256(render_quest_translation_decision_workbook_tsv(workbook).as_bytes())
}

/// 장면 batch의 원문과 두 번역안, 문맥, 용어 선택지를 한 행에서 대조하는 TSV.
pub fn render_quest_translation_batch_decisions_tsv(
    analysis: &QuestTranslationDecisionAnalysis,
) -> String {
    let mut output = String::from(
        "schema_version\ttranslation_batch_report_sha256\treview_asset_sha256\t\
         terminology_report_sha256\tbatch_id\tscene_id\tscene_title\tscene_order\tbranch\t\
         stream_id\tspeaker\taddressee\tcontext_note\tprotected_source\tdraft_ko\t\
         independent_review_status\treview_ko\treview_notes\treview_decision_dependencies\t\
         matched_term_ids\tterm_choices\tterm_evidence\tpending_term_decisions\tapproved_terms\t\
         runtime_evidence\tcontrol_signature\thuman_action\thuman_decision_status\t\
         build_eligible\n",
    );
    for entry in &analysis.entries {
        let review = &entry.review;
        let pending_terms = entry
            .terms
            .iter()
            .filter(|term| term.decision != QuestTerminologyDecision::HumanApproved)
            .map(|term| term.id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let approved_terms = entry
            .terms
            .iter()
            .filter_map(|term| {
                term.approved_ko
                    .as_deref()
                    .map(|approved| format!("{}=>{approved}", term.id))
            })
            .collect::<Vec<_>>()
            .join(" | ");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
             {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tpending-human-approval\tno",
            QUEST_TRANSLATION_DECISION_PACKET_SCHEMA_VERSION,
            analysis.translation_batch_report_sha256,
            analysis.review_asset_sha256,
            analysis.terminology_report_sha256,
            review.batch_entry.batch_id,
            review.batch_entry.scene_id,
            escape_tsv(&review.batch_entry.scene_title),
            review.batch_entry.scene_order,
            review.batch_entry.branch,
            review.batch_entry.stream_id,
            escape_tsv(&review.batch_entry.speaker),
            escape_tsv(&review.batch_entry.addressee),
            escape_tsv(&review.batch_entry.context_note),
            escape_tsv(&review.batch_entry.protected_source),
            escape_tsv(&review.batch_entry.korean_draft),
            review
                .independent_review_status
                .expect("결정 자료는 독립 검토 완료 행만 포함함")
                .label(),
            escape_tsv(&review.reviewed_korean),
            escape_tsv(&review.review_notes),
            review.decision_dependencies.join(";"),
            review.batch_entry.matched_term_ids.join(","),
            escape_tsv(&render_term_choices(&entry.terms)),
            escape_tsv(&render_term_evidence(&entry.terms)),
            pending_terms,
            escape_tsv(&approved_terms),
            review.batch_entry.runtime_evidence.unwrap_or("static-only"),
            review.batch_entry.control_signature,
            entry.review_action.label(),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_translation_decisions_tests.rs"]
mod tests;
