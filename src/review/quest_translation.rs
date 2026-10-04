//! QUEST 전역 결정 자료를 browser가 표시할 읽기 전용 자료로 바꾼다.
//!
//! 이 모듈은 후보를 승인하지 않는다. 현재 승인 원장에 실제 행이 있을 때만
//! 선택값을 표시하며, 문장·용어·공통 의존성의 준비 상태를 서로 구분한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::script::{
    QuestTerminologyDecision, QuestTranslationApproval, QuestTranslationApprovalAnalysis,
    QuestTranslationApprovalKind, QuestTranslationDecisionEntry, QuestTranslationDecisionWorkbook,
};

/// 번역 후보가 만들어진 검토 단계.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuestReviewCandidateSource {
    Draft,
    IndependentReview,
}

impl QuestReviewCandidateSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Draft => "초벌",
            Self::IndependentReview => "독립 검토안",
        }
    }
}

/// 같은 문자열을 중복 선택지로 만들지 않은 번역 후보.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestReviewCandidate {
    pub korean: String,
    pub sources: Vec<QuestReviewCandidateSource>,
}

/// 번역문에 적용되는 승인 용어와 근거.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestReviewTerm {
    pub id: String,
    pub protected_source: String,
    pub approved_korean: Option<String>,
    pub decision: String,
    pub evidence: Vec<String>,
}

/// 여러 번역문이 함께 참조하는 사람 결정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestReviewDependency {
    pub id: String,
    pub baseline_sha256: String,
    pub occurrences: usize,
    pub batch_ids: Vec<String>,
    pub stream_ids: Vec<String>,
    pub selected_value: Option<String>,
    pub approval_notes: Option<String>,
}

/// 검토 화면에 표시할 번역 단위 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestReviewEntry {
    pub batch_id: String,
    pub scene_order: usize,
    pub scene_title: String,
    pub sequence_context: String,
    pub branch: String,
    pub stream_id: String,
    pub baseline_sha256: String,
    pub speaker: String,
    pub addressee: String,
    pub context_note: String,
    pub protected_source: String,
    pub control_signature: String,
    pub runtime_evidence: Option<String>,
    pub work_notes: String,
    pub independent_review_status: String,
    pub review_notes: String,
    pub review_action: String,
    pub candidates: Vec<QuestReviewCandidate>,
    pub terms: Vec<QuestReviewTerm>,
    pub dependency_ids: Vec<String>,
    pub selected_korean: Option<String>,
    pub approval_notes: Option<String>,
    pub approval_build_eligible: bool,
    pub waiting_reasons: Vec<String>,
}

/// 한 batch의 대기열과 현재 승인 진행 상태.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestReviewQueue {
    pub batch_id: String,
    pub entries: Vec<QuestReviewEntry>,
    pub dependencies: Vec<QuestReviewDependency>,
    pub approval_manifest_sha256: String,
    pub approved_entries: usize,
    pub approval_build_eligible_entries: usize,
}

/// 전역 결정 자료에서 파생한 batch별 검토 진행 상태.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestReviewBatchSummary {
    pub batch_id: String,
    pub scene_title: String,
    pub entries: usize,
    pub approved_entries: usize,
    pub approval_build_eligible_entries: usize,
}

/// hard-coded batch 목록 없이 현재 전역 결정 자료 전체를 나타내는 색인.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestReviewCatalog {
    pub batches: Vec<QuestReviewBatchSummary>,
    pub entries: usize,
    pub approved_entries: usize,
    pub approval_build_eligible_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestReviewQueueError(String);

impl fmt::Display for QuestReviewQueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestReviewQueueError {}

fn error(message: impl Into<String>) -> QuestReviewQueueError {
    QuestReviewQueueError(message.into())
}

fn translation_candidates(entry: &QuestTranslationDecisionEntry) -> Vec<QuestReviewCandidate> {
    let draft = &entry.review.batch_entry.korean_draft;
    let reviewed = &entry.review.reviewed_korean;
    if draft == reviewed {
        vec![QuestReviewCandidate {
            korean: draft.clone(),
            sources: vec![
                QuestReviewCandidateSource::Draft,
                QuestReviewCandidateSource::IndependentReview,
            ],
        }]
    } else {
        vec![
            QuestReviewCandidate {
                korean: draft.clone(),
                sources: vec![QuestReviewCandidateSource::Draft],
            },
            QuestReviewCandidate {
                korean: reviewed.clone(),
                sources: vec![QuestReviewCandidateSource::IndependentReview],
            },
        ]
    }
}

fn approval_by_key(
    approvals: &QuestTranslationApprovalAnalysis,
) -> Result<
    BTreeMap<(QuestTranslationApprovalKind, &str), &QuestTranslationApproval>,
    QuestReviewQueueError,
> {
    let mut by_key = BTreeMap::new();
    for approval in &approvals.approvals {
        let key = (approval.kind, approval.decision_id.as_str());
        if by_key.insert(key, approval).is_some() {
            return Err(error(format!(
                "QUEST 검토 승인 ID가 중복됨: {}:{}",
                approval.kind.label(),
                approval.decision_id
            )));
        }
    }
    Ok(by_key)
}

/// 전역 결정 자료에서 한 batch의 검토 대기열을 만든다.
pub fn build_quest_translation_review_queue(
    workbook: &QuestTranslationDecisionWorkbook,
    approvals: &QuestTranslationApprovalAnalysis,
    batch_id: &str,
) -> Result<QuestReviewQueue, QuestReviewQueueError> {
    let batch = workbook
        .batch_decisions
        .iter()
        .find(|batch| batch.batch_id == batch_id)
        .ok_or_else(|| error(format!("QUEST 검토 batch가 없음: {batch_id}")))?;
    let baseline_by_key = approvals
        .baselines
        .iter()
        .map(|baseline| {
            (
                (baseline.kind, baseline.decision_id.as_str()),
                baseline.baseline_sha256.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if baseline_by_key.len() != approvals.baselines.len() {
        return Err(error("QUEST 검토 승인 기준선 ID가 중복됨"));
    }
    let approval_by_key = approval_by_key(approvals)?;
    let approved_dependency_ids = approval_by_key
        .keys()
        .filter(|(kind, _)| *kind == QuestTranslationApprovalKind::SharedDependency)
        .map(|(_, decision_id)| *decision_id)
        .collect::<BTreeSet<_>>();

    let mut entries = Vec::with_capacity(batch.entries.len());
    for decision in &batch.entries {
        let batch_entry = &decision.review.batch_entry;
        let key = (
            QuestTranslationApprovalKind::TranslationEntry,
            batch_entry.stream_id.as_str(),
        );
        let baseline_sha256 = baseline_by_key.get(&key).ok_or_else(|| {
            error(format!(
                "QUEST 검토 번역문 승인 기준선이 없음: {}",
                batch_entry.stream_id
            ))
        })?;
        let approval = approval_by_key.get(&key).copied();
        let mut waiting_reasons = Vec::new();
        if approval.is_none() {
            waiting_reasons.push("문장 미승인".to_owned());
        }
        if decision
            .terms
            .iter()
            .any(|term| term.decision != QuestTerminologyDecision::HumanApproved)
        {
            waiting_reasons.push("용어 미승인".to_owned());
        }
        if decision
            .review
            .decision_dependencies
            .iter()
            .any(|dependency| !approved_dependency_ids.contains(dependency.as_str()))
        {
            waiting_reasons.push("공통 의존성 미승인".to_owned());
        }
        let approval_build_eligible = waiting_reasons.is_empty();
        let terms = decision
            .terms
            .iter()
            .map(|term| QuestReviewTerm {
                id: term.id.clone(),
                protected_source: term.protected_source.clone(),
                approved_korean: term.approved_ko.clone(),
                decision: term.decision.label().to_owned(),
                evidence: term
                    .evidence
                    .iter()
                    .map(|evidence| {
                        format!(
                            "{} / {} / {} / {} / {}",
                            evidence.project,
                            evidence.path,
                            evidence.state.label(),
                            evidence.candidate_ko,
                            evidence.notes
                        )
                    })
                    .collect(),
            })
            .collect();
        entries.push(QuestReviewEntry {
            batch_id: batch.batch_id.clone(),
            scene_order: batch_entry.scene_order,
            scene_title: batch_entry.scene_title.clone(),
            sequence_context: batch_entry.sequence_context.clone(),
            branch: batch_entry.branch.clone(),
            stream_id: batch_entry.stream_id.clone(),
            baseline_sha256: (*baseline_sha256).to_owned(),
            speaker: batch_entry.speaker.clone(),
            addressee: batch_entry.addressee.clone(),
            context_note: batch_entry.context_note.clone(),
            protected_source: batch_entry.protected_source.clone(),
            control_signature: batch_entry.control_signature.clone(),
            runtime_evidence: batch_entry.runtime_evidence.map(str::to_owned),
            work_notes: batch_entry.work_notes.clone(),
            independent_review_status: decision
                .review
                .independent_review_status
                .expect("결정 자료의 독립 검토는 완료됨")
                .label()
                .to_owned(),
            review_notes: decision.review.review_notes.clone(),
            review_action: decision.review_action.label().to_owned(),
            candidates: translation_candidates(decision),
            terms,
            dependency_ids: decision.review.decision_dependencies.clone(),
            selected_korean: approval.map(|approval| approval.selected_korean.clone()),
            approval_notes: approval.map(|approval| approval.notes.clone()),
            approval_build_eligible,
            waiting_reasons,
        });
    }
    entries.sort_by(|left, right| {
        left.scene_order
            .cmp(&right.scene_order)
            .then(left.stream_id.cmp(&right.stream_id))
    });

    let referenced_dependencies = entries
        .iter()
        .flat_map(|entry| entry.dependency_ids.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    let mut dependencies = Vec::new();
    for dependency_id in referenced_dependencies {
        let dependency = workbook
            .dependencies
            .iter()
            .find(|dependency| dependency.id == dependency_id)
            .ok_or_else(|| {
                error(format!(
                    "QUEST 검토 공통 의존성 자료가 없음: {dependency_id}"
                ))
            })?;
        let key = (
            QuestTranslationApprovalKind::SharedDependency,
            dependency.id.as_str(),
        );
        if !baseline_by_key.contains_key(&key) {
            return Err(error(format!(
                "QUEST 검토 공통 의존성 승인 기준선이 없음: {}",
                dependency.id
            )));
        }
        let approval = approval_by_key.get(&key).copied();
        dependencies.push(QuestReviewDependency {
            id: dependency.id.clone(),
            baseline_sha256: baseline_by_key
                .get(&key)
                .expect("앞에서 확인한 공통 의존성 기준선")
                .to_string(),
            occurrences: dependency.occurrences,
            batch_ids: dependency.batch_ids.clone(),
            stream_ids: dependency.stream_ids.clone(),
            selected_value: approval.map(|approval| approval.selected_korean.clone()),
            approval_notes: approval.map(|approval| approval.notes.clone()),
        });
    }

    let approved_entries = entries
        .iter()
        .filter(|entry| entry.selected_korean.is_some())
        .count();
    let approval_build_eligible_entries = entries
        .iter()
        .filter(|entry| entry.approval_build_eligible)
        .count();
    Ok(QuestReviewQueue {
        batch_id: batch.batch_id.clone(),
        entries,
        dependencies,
        approval_manifest_sha256: approvals.manifest_sha256.clone(),
        approved_entries,
        approval_build_eligible_entries,
    })
}

/// 전역 결정 자료의 모든 batch를 현재 승인 원장에 결속해 검토 색인을 만든다.
pub fn build_quest_translation_review_catalog(
    workbook: &QuestTranslationDecisionWorkbook,
    approvals: &QuestTranslationApprovalAnalysis,
) -> Result<QuestReviewCatalog, QuestReviewQueueError> {
    if workbook.batch_decisions.is_empty() {
        return Err(error("QUEST 검토 전역 결정 자료에 batch가 없음"));
    }

    let mut batch_ids = BTreeSet::new();
    let mut batches = Vec::with_capacity(workbook.batch_decisions.len());
    let mut entries = 0usize;
    let mut approved_entries = 0usize;
    let mut approval_build_eligible_entries = 0usize;
    for batch in &workbook.batch_decisions {
        if !batch_ids.insert(batch.batch_id.as_str()) {
            return Err(error(format!(
                "QUEST 검토 전역 결정 자료의 batch ID가 중복됨: {}",
                batch.batch_id
            )));
        }
        let queue = build_quest_translation_review_queue(workbook, approvals, &batch.batch_id)?;
        let scene_title = queue
            .entries
            .first()
            .map(|entry| entry.scene_title.clone())
            .ok_or_else(|| error(format!("QUEST 검토 batch가 비어 있음: {}", batch.batch_id)))?;
        if queue
            .entries
            .iter()
            .any(|entry| entry.scene_title != scene_title)
        {
            return Err(error(format!(
                "QUEST 검토 batch 안의 장면 제목이 일치하지 않음: {}",
                batch.batch_id
            )));
        }
        entries = entries
            .checked_add(queue.entries.len())
            .ok_or_else(|| error("QUEST 검토 전역 항목 수 overflow"))?;
        approved_entries = approved_entries
            .checked_add(queue.approved_entries)
            .ok_or_else(|| error("QUEST 검토 전역 승인 수 overflow"))?;
        approval_build_eligible_entries = approval_build_eligible_entries
            .checked_add(queue.approval_build_eligible_entries)
            .ok_or_else(|| error("QUEST 검토 전역 build-eligible 수 overflow"))?;
        batches.push(QuestReviewBatchSummary {
            batch_id: queue.batch_id,
            scene_title,
            entries: queue.entries.len(),
            approved_entries: queue.approved_entries,
            approval_build_eligible_entries: queue.approval_build_eligible_entries,
        });
    }
    if entries != workbook.entries {
        return Err(error(format!(
            "QUEST 검토 전역 항목 분모 불일치: expected={}, actual={entries}",
            workbook.entries
        )));
    }

    Ok(QuestReviewCatalog {
        batches,
        entries,
        approved_entries,
        approval_build_eligible_entries,
    })
}

#[cfg(test)]
#[path = "quest_translation_tests.rs"]
mod tests;
