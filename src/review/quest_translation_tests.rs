use super::*;
use crate::script::{
    audit_quest_translation_approval_manifest, QuestSeqTranslationUnitPolicy,
    QuestTranslationBatchEntry, QuestTranslationBatchRole, QuestTranslationContextConfidence,
    QuestTranslationDecisionAnalysis, QuestTranslationDecisionDependency,
    QuestTranslationIndependentReviewStatus, QuestTranslationReviewAction,
    QuestTranslationReviewEntry, QuestTranslationReviewRequirement, QuestTranslationWorkStatus,
    QUEST_TRANSLATION_APPROVAL_COLUMNS,
};

const TEST_BATCH_ID: &str = "quest-suke-skeleton-tea-confrontation-001";

fn decision(scene_order: usize, reviewed_matches_draft: bool) -> QuestTranslationDecisionEntry {
    let stream_id = format!("stream-{scene_order}");
    let draft = format!("아르르「초벌 {scene_order}");
    let reviewed = if reviewed_matches_draft {
        draft.clone()
    } else {
        format!("아르르「검토 {scene_order}")
    };
    let dependency_ids = if scene_order == 4 {
        vec!["quest.suke.long_silence.layout".to_owned()]
    } else {
        Vec::new()
    };
    QuestTranslationDecisionEntry {
        review: QuestTranslationReviewEntry {
            batch_entry: QuestTranslationBatchEntry {
                batch_id: TEST_BATCH_ID.to_owned(),
                batch_context_sha256: "batch-context".to_owned(),
                scene_id: "suke-tea".to_owned(),
                scene_title: "스켈톤T와 차 대치".to_owned(),
                sequence_context: "SUKE.SEQ 대치 장면".to_owned(),
                source_evidence: "원본 stream".to_owned(),
                scene_order,
                branch: "shared".to_owned(),
                stream_id,
                role: QuestTranslationBatchRole::Translate,
                speaker_id: "arle".to_owned(),
                speaker: "아르르".to_owned(),
                addressee_id: "skeleton-t".to_owned(),
                addressee: "스켈톤T".to_owned(),
                confidence: QuestTranslationContextConfidence::Confirmed,
                context_note: "대치 중 대화".to_owned(),
                protected_raw_hex: "0000".to_owned(),
                protected_source: format!("アルル「原文{scene_order}"),
                translation_policy: QuestSeqTranslationUnitPolicy::JapaneseTranslationCandidate,
                control_signature: "FF03".to_owned(),
                runtime_evidence: None,
                matched_term_ids: Vec::new(),
                matched_term_decisions: Vec::new(),
                korean_draft: draft,
                work_status: QuestTranslationWorkStatus::DraftNeedsIndependentReview,
                work_notes: "초벌 근거".to_owned(),
            },
            requirement: QuestTranslationReviewRequirement::HumanContextDecision,
            independent_review_status: Some(if reviewed_matches_draft {
                QuestTranslationIndependentReviewStatus::ReviewAgrees
            } else {
                QuestTranslationIndependentReviewStatus::ReviewProposesChange
            }),
            reviewed_korean: reviewed,
            decision_dependencies: dependency_ids,
            review_notes: "독립 검토 근거".to_owned(),
            review_asset_sha256: Some("review-asset".to_owned()),
        },
        terms: Vec::new(),
        review_action: if reviewed_matches_draft {
            QuestTranslationReviewAction::ApproveReviewedTextOrRevise
        } else {
            QuestTranslationReviewAction::ChooseDraftReviewedTextOrRevise
        },
    }
}

fn workbook() -> QuestTranslationDecisionWorkbook {
    let entries = [3, 1, 2, 4, 5, 6, 7]
        .into_iter()
        .map(|order| decision(order, order % 2 == 1))
        .collect::<Vec<_>>();
    QuestTranslationDecisionWorkbook {
        batch_decisions: vec![QuestTranslationDecisionAnalysis {
            batch_id: TEST_BATCH_ID.to_owned(),
            review_asset_sha256: "review-asset".to_owned(),
            translation_batch_report_sha256: "translation-batch".to_owned(),
            terminology_report_sha256: "terminology".to_owned(),
            entries,
            affected_terms: 0,
            review_agreements: 4,
            review_proposals: 3,
            review_human_decisions: 0,
            pending_term_decisions: 0,
            conflicting_term_decisions: 0,
            approved_term_decisions: 0,
            pending_human_decisions: 7,
            build_eligible_entries: 0,
        }],
        terms: Vec::new(),
        dependencies: vec![QuestTranslationDecisionDependency {
            id: "quest.suke.long_silence.layout".to_owned(),
            batch_ids: vec![TEST_BATCH_ID.to_owned()],
            stream_ids: vec!["stream-4".to_owned()],
            occurrences: 1,
        }],
        entries: 7,
        review_agreements: 4,
        review_proposals: 3,
        review_human_decisions: 0,
        pending_term_decisions: 0,
        conflicting_term_decisions: 0,
        approved_term_decisions: 0,
        dependency_occurrences: 1,
        build_eligible_entries: 0,
    }
}

fn approvals(workbook: &QuestTranslationDecisionWorkbook) -> QuestTranslationApprovalAnalysis {
    let blank = format!("{QUEST_TRANSLATION_APPROVAL_COLUMNS}\n");
    let baseline = audit_quest_translation_approval_manifest(workbook, &blank).unwrap();
    let dependency = baseline
        .baselines
        .iter()
        .find(|baseline| baseline.kind == QuestTranslationApprovalKind::SharedDependency)
        .unwrap();
    let entry = baseline
        .baselines
        .iter()
        .find(|baseline| baseline.decision_id == "stream-1")
        .unwrap();
    let input = format!(
        "{QUEST_TRANSLATION_APPROVAL_COLUMNS}\n\
         {}\t{}\t{}\t{}\t{}\n\
         {}\t{}\t{}\t{}\t{}\n",
        dependency.kind.label(),
        dependency.decision_id,
        dependency.baseline_sha256,
        "원문의 네 줄 침묵 길이를 보존한다",
        "원문과 검토안을 대조함",
        entry.kind.label(),
        entry.decision_id,
        entry.baseline_sha256,
        "아르르「초벌 1",
        "원문과 두 후보를 대조함",
    );
    audit_quest_translation_approval_manifest(workbook, &input).unwrap()
}

#[test]
fn 첫_검토_batch를_장면_순서와_승인_상태로_만든다() {
    let workbook = workbook();
    let approvals = approvals(&workbook);
    let queue = build_quest_translation_review_queue(&workbook, &approvals, TEST_BATCH_ID).unwrap();

    assert_eq!(queue.entries.len(), 7);
    assert_eq!(
        queue
            .entries
            .iter()
            .map(|entry| entry.scene_order)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6, 7]
    );
    assert_eq!(queue.approved_entries, 1);
    assert_eq!(queue.approval_build_eligible_entries, 1);
    assert_eq!(
        queue.entries[0].selected_korean.as_deref(),
        Some("아르르「초벌 1")
    );
    assert!(queue.entries[0].waiting_reasons.is_empty());
    assert_eq!(queue.dependencies.len(), 1);
    assert_eq!(
        queue.dependencies[0].selected_value.as_deref(),
        Some("원문의 네 줄 침묵 길이를 보존한다")
    );
}

#[test]
fn 같은_초벌과_검토안은_출처_둘을_가진_후보_하나로_합친다() {
    let workbook = workbook();
    let approvals = approvals(&workbook);
    let queue = build_quest_translation_review_queue(&workbook, &approvals, TEST_BATCH_ID).unwrap();

    assert_eq!(queue.entries[0].candidates.len(), 1);
    assert_eq!(
        queue.entries[0].candidates[0].sources,
        vec![
            QuestReviewCandidateSource::Draft,
            QuestReviewCandidateSource::IndependentReview,
        ]
    );
    assert_eq!(queue.entries[1].candidates.len(), 2);
}

#[test]
fn 없는_batch는_빈_대기열이_아니라_실패한다() {
    let workbook = workbook();
    let approvals = approvals(&workbook);
    let error = build_quest_translation_review_queue(&workbook, &approvals, "missing").unwrap_err();

    assert!(error.to_string().contains("batch가 없음"));
}

#[test]
fn 전역색인은_workbook의_모든_batch를_입력순서대로_파생한다() {
    let mut workbook = workbook();
    for entry in &mut workbook.batch_decisions[0].entries {
        entry.review.decision_dependencies.clear();
    }
    workbook.dependencies.clear();
    let mut second = workbook.batch_decisions[0].clone();
    second.batch_id = "newly-added-batch".to_owned();
    for (index, entry) in second.entries.iter_mut().enumerate() {
        entry.review.batch_entry.batch_id = second.batch_id.clone();
        entry.review.batch_entry.stream_id = format!("new-stream-{index}");
    }
    workbook.entries += second.entries.len();
    workbook.batch_decisions.push(second);
    let blank = format!("{QUEST_TRANSLATION_APPROVAL_COLUMNS}\n");
    let approvals = audit_quest_translation_approval_manifest(&workbook, &blank).unwrap();

    let catalog = build_quest_translation_review_catalog(&workbook, &approvals).unwrap();

    assert_eq!(catalog.entries, 14);
    assert_eq!(catalog.approved_entries, 0);
    assert_eq!(catalog.approval_build_eligible_entries, 0);
    assert_eq!(
        catalog
            .batches
            .iter()
            .map(|batch| batch.batch_id.as_str())
            .collect::<Vec<_>>(),
        vec![TEST_BATCH_ID, "newly-added-batch"]
    );
    assert_eq!(catalog.batches[1].entries, 7);
}
