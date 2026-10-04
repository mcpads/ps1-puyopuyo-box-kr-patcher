use super::*;
use crate::script::{
    QuestSeqTranslationUnitPolicy, QuestTerminologyCandidate, QuestTerminologyCategory,
    QuestTerminologyEvidence, QuestTerminologyEvidenceState, QuestTranslationBatchEntry,
    QuestTranslationBatchRole, QuestTranslationContextConfidence, QuestTranslationDecisionAnalysis,
    QuestTranslationDecisionDependency, QuestTranslationDecisionEntry,
    QuestTranslationIndependentReviewStatus, QuestTranslationReviewAction,
    QuestTranslationReviewEntry, QuestTranslationReviewRequirement, QuestTranslationWorkStatus,
};

fn term(decision: QuestTerminologyDecision) -> QuestTerminologyCandidate {
    QuestTerminologyCandidate {
        id: "quest.character.arle".to_owned(),
        category: QuestTerminologyCategory::Character,
        protected_source: "アルル".to_owned(),
        source_occurrences: 1,
        matched_units: 1,
        candidate_ko: vec!["아르르".to_owned()],
        approved_ko: (decision == QuestTerminologyDecision::HumanApproved)
            .then(|| "아르르".to_owned()),
        decision,
        evidence: vec![QuestTerminologyEvidence {
            project: "sibling".to_owned(),
            path: "terms.tsv".to_owned(),
            state: QuestTerminologyEvidenceState::SeriesApproved,
            candidate_ko: "아르르".to_owned(),
            notes: "계열 표기".to_owned(),
        }],
    }
}

fn workbook(term_decision: QuestTerminologyDecision) -> QuestTranslationDecisionWorkbook {
    let term = term(term_decision);
    let batch_entry = QuestTranslationBatchEntry {
        batch_id: "scene-a".to_owned(),
        batch_context_sha256: "batch-context".to_owned(),
        scene_id: "scene".to_owned(),
        scene_title: "장면".to_owned(),
        sequence_context: "장면 문맥".to_owned(),
        source_evidence: "원본 근거".to_owned(),
        scene_order: 1,
        branch: "shared".to_owned(),
        stream_id: "stream-a".to_owned(),
        role: QuestTranslationBatchRole::Translate,
        speaker_id: "arle".to_owned(),
        speaker: "아르르".to_owned(),
        addressee_id: "other".to_owned(),
        addressee: "상대".to_owned(),
        confidence: QuestTranslationContextConfidence::Confirmed,
        context_note: "확정 문맥".to_owned(),
        protected_raw_hex: "0000".to_owned(),
        protected_source: "アルル「やった！".to_owned(),
        translation_policy: QuestSeqTranslationUnitPolicy::JapaneseTranslationCandidate,
        control_signature: "end".to_owned(),
        runtime_evidence: None,
        matched_term_ids: vec![term.id.clone()],
        matched_term_decisions: vec![term.decision.label()],
        korean_draft: "아르르「해냈다!".to_owned(),
        work_status: QuestTranslationWorkStatus::DraftNeedsIndependentReview,
        work_notes: "초벌".to_owned(),
    };
    let review = QuestTranslationReviewEntry {
        batch_entry,
        requirement: QuestTranslationReviewRequirement::HumanContextDecision,
        independent_review_status: Some(
            QuestTranslationIndependentReviewStatus::NeedsHumanDecision,
        ),
        reviewed_korean: "아르르「해냈다!".to_owned(),
        decision_dependencies: vec!["quest.style.arle".to_owned()],
        review_notes: "검토 동의".to_owned(),
        review_asset_sha256: Some("review-asset".to_owned()),
    };
    let batch_decision = QuestTranslationDecisionAnalysis {
        batch_id: "scene-a".to_owned(),
        review_asset_sha256: "review-asset".to_owned(),
        translation_batch_report_sha256: "translation-batch".to_owned(),
        terminology_report_sha256: "terminology".to_owned(),
        entries: vec![QuestTranslationDecisionEntry {
            review,
            terms: vec![term.clone()],
            review_action: QuestTranslationReviewAction::ResolveDependenciesAndApprove,
        }],
        affected_terms: 1,
        review_agreements: 0,
        review_proposals: 0,
        review_human_decisions: 1,
        pending_term_decisions: usize::from(
            term_decision != QuestTerminologyDecision::HumanApproved,
        ),
        conflicting_term_decisions: 0,
        approved_term_decisions: usize::from(
            term_decision == QuestTerminologyDecision::HumanApproved,
        ),
        pending_human_decisions: 1,
        build_eligible_entries: 0,
    };
    QuestTranslationDecisionWorkbook {
        batch_decisions: vec![batch_decision],
        terms: vec![term],
        dependencies: vec![QuestTranslationDecisionDependency {
            id: "quest.style.arle".to_owned(),
            batch_ids: vec!["scene-a".to_owned()],
            stream_ids: vec!["stream-a".to_owned()],
            occurrences: 1,
        }],
        entries: 1,
        review_agreements: 0,
        review_proposals: 0,
        review_human_decisions: 1,
        pending_term_decisions: usize::from(
            term_decision != QuestTerminologyDecision::HumanApproved,
        ),
        conflicting_term_decisions: 0,
        approved_term_decisions: usize::from(
            term_decision == QuestTerminologyDecision::HumanApproved,
        ),
        dependency_occurrences: 1,
        build_eligible_entries: 0,
    }
}

fn approval_input(
    workbook: &QuestTranslationDecisionWorkbook,
    include_dependency: bool,
    selected_translation: &str,
) -> String {
    let baselines = approval_baselines(workbook).unwrap();
    let mut output = format!("{QUEST_TRANSLATION_APPROVAL_COLUMNS}\n");
    for baseline in baselines {
        if baseline.baseline.kind == QuestTranslationApprovalKind::SharedDependency
            && !include_dependency
        {
            continue;
        }
        let selected = if baseline.baseline.kind == QuestTranslationApprovalKind::SharedDependency {
            "활달한 반말을 유지한다"
        } else {
            selected_translation
        };
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t사람이 원문과 검토안을 대조함\n",
            baseline.baseline.kind.label(),
            baseline.baseline.decision_id,
            baseline.baseline.baseline_sha256,
            selected,
        ));
    }
    output
}

#[test]
fn 빈_승인입력은_후보를_승인으로_추론하지_않는다() {
    let workbook = workbook(QuestTerminologyDecision::PendingHumanApproval);
    let analysis = audit_quest_translation_approval_manifest(
        &workbook,
        &format!("{QUEST_TRANSLATION_APPROVAL_COLUMNS}\n"),
    )
    .unwrap();

    assert_eq!(analysis.baselines.len(), 2);
    assert!(analysis.approvals.is_empty());
    assert_eq!(analysis.build_eligible_entries, 0);
}

#[test]
fn 승인번역은_용어와_공통의존성이_모두_승인돼야_build_eligible이다() {
    let pending_term = workbook(QuestTerminologyDecision::PendingHumanApproval);
    let pending_analysis = audit_quest_translation_approval_manifest(
        &pending_term,
        &approval_input(&pending_term, true, "아르르「해냈다!"),
    )
    .unwrap();
    assert_eq!(pending_analysis.entry_approvals_waiting_for_terms, 1);
    assert_eq!(pending_analysis.build_eligible_entries, 0);

    let approved_term = workbook(QuestTerminologyDecision::HumanApproved);
    let missing_dependency = audit_quest_translation_approval_manifest(
        &approved_term,
        &approval_input(&approved_term, false, "아르르「해냈다!"),
    )
    .unwrap();
    assert_eq!(
        missing_dependency.entry_approvals_waiting_for_dependencies,
        1
    );
    assert_eq!(missing_dependency.build_eligible_entries, 0);

    let ready = audit_quest_translation_approval_manifest(
        &approved_term,
        &approval_input(&approved_term, true, "아르르「해냈다!"),
    )
    .unwrap();
    assert_eq!(ready.build_eligible_entries, 1);
}

#[test]
fn 현재_결정자료와_다른_기준선은_승인을_거부한다() {
    let workbook = workbook(QuestTerminologyDecision::HumanApproved);
    let input = approval_input(&workbook, true, "아르르「해냈다!").replacen(
        &approval_baselines(&workbook).unwrap()[0]
            .baseline
            .baseline_sha256,
        &"0".repeat(64),
        1,
    );
    let error = audit_quest_translation_approval_manifest(&workbook, &input).unwrap_err();

    assert!(error.to_string().contains("기준선 drift"));
}

#[test]
fn 일본어가_남은_승인번역은_거부한다() {
    let workbook = workbook(QuestTerminologyDecision::HumanApproved);
    let input = approval_input(&workbook, true, "アルル「해냈다!");
    let error = audit_quest_translation_approval_manifest(&workbook, &input).unwrap_err();

    assert!(error.to_string().contains("선택값에 일본어가 남음"));
}

#[test]
fn 미해결_placeholder가_남은_승인번역은_거부한다() {
    let workbook = workbook(QuestTerminologyDecision::HumanApproved);
    let input = approval_input(&workbook, true, "{quest.character.arle}「해냈다!");
    let error = audit_quest_translation_approval_manifest(&workbook, &input).unwrap_err();

    assert!(error.to_string().contains("미해결 placeholder가 남음"));
}
