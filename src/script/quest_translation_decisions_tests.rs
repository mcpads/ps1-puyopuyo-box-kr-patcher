use super::*;
use crate::script::{
    QuestSeqTranslationUnitPolicy, QuestTerminologyCategory, QuestTerminologyEvidence,
    QuestTerminologyEvidenceState, QuestTranslationBatchEntry, QuestTranslationContextConfidence,
    QuestTranslationReviewRequirement, QuestTranslationWorkStatus,
};

fn batch_entry(
    batch_id: &str,
    scene_order: usize,
    term: Option<&QuestTerminologyCandidate>,
) -> QuestTranslationBatchEntry {
    let (matched_term_ids, matched_term_decisions) = term.map_or_else(
        || (Vec::new(), Vec::new()),
        |term| (vec![term.id.clone()], vec![term.decision.label()]),
    );
    QuestTranslationBatchEntry {
        batch_id: batch_id.to_string(),
        batch_context_sha256: "context".to_string(),
        scene_id: "first-scene".to_string(),
        scene_title: "첫 장면".to_string(),
        sequence_context: "장면 문맥".to_string(),
        source_evidence: "원본 근거".to_string(),
        scene_order,
        branch: "shared".to_string(),
        stream_id: format!("{batch_id}-stream-{scene_order}"),
        role: QuestTranslationBatchRole::Translate,
        speaker_id: "speaker".to_string(),
        speaker: "화자".to_string(),
        addressee_id: "listener".to_string(),
        addressee: "청자".to_string(),
        confidence: QuestTranslationContextConfidence::Confirmed,
        context_note: "확정 문맥".to_string(),
        protected_raw_hex: "0000".to_string(),
        protected_source: "原文".to_string(),
        translation_policy: QuestSeqTranslationUnitPolicy::JapaneseTranslationCandidate,
        control_signature: "end".to_string(),
        runtime_evidence: None,
        matched_term_ids,
        matched_term_decisions,
        korean_draft: "초안".to_string(),
        work_status: QuestTranslationWorkStatus::DraftNeedsIndependentReview,
        work_notes: "초안 메모".to_string(),
    }
}

fn review_entry(
    batch_id: &str,
    scene_order: usize,
    status: Option<QuestTranslationIndependentReviewStatus>,
    term: Option<&QuestTerminologyCandidate>,
) -> QuestTranslationReviewEntry {
    QuestTranslationReviewEntry {
        batch_entry: batch_entry(batch_id, scene_order, term),
        requirement: QuestTranslationReviewRequirement::IndependentReview,
        independent_review_status: status,
        reviewed_korean: status.map(|_| "검토안".to_string()).unwrap_or_default(),
        decision_dependencies: if status
            == Some(QuestTranslationIndependentReviewStatus::NeedsHumanDecision)
        {
            vec!["speaker-decision".to_string()]
        } else {
            Vec::new()
        },
        review_notes: status.map(|_| "검토 메모".to_string()).unwrap_or_default(),
        review_asset_sha256: status.map(|_| format!("review-{batch_id}")),
    }
}

fn review_analysis(entries: Vec<QuestTranslationReviewEntry>) -> QuestTranslationReviewAnalysis {
    QuestTranslationReviewAnalysis {
        entries,
        review_assets: 0,
        protected_context_entries: 0,
        human_term_decision_entries: 0,
        human_context_decision_entries: 0,
        runtime_sample_entries: 0,
        independent_review_queue_entries: 0,
        reviewed_entries: 0,
        review_agreements: 0,
        review_proposals: 0,
        review_human_decision_rows: 0,
        unreviewed_drafts: 0,
        build_eligible_entries: 0,
        translation_batch_report_sha256: "translation-batch".to_owned(),
    }
}

fn terminology_candidate(
    id: &str,
    decision: QuestTerminologyDecision,
) -> QuestTerminologyCandidate {
    QuestTerminologyCandidate {
        id: id.to_string(),
        category: QuestTerminologyCategory::Character,
        protected_source: "原語".to_string(),
        source_occurrences: 1,
        matched_units: 1,
        candidate_ko: vec!["후보".to_string()],
        approved_ko: (decision == QuestTerminologyDecision::HumanApproved)
            .then(|| "후보".to_string()),
        decision,
        evidence: vec![QuestTerminologyEvidence {
            project: "sibling".to_string(),
            path: "terms.tsv".to_string(),
            state: QuestTerminologyEvidenceState::SeriesApproved,
            candidate_ko: "후보".to_string(),
            notes: "근거".to_string(),
        }],
    }
}

fn terminology_analysis(
    terms: Vec<QuestTerminologyCandidate>,
) -> QuestTerminologyCandidateAnalysis {
    QuestTerminologyCandidateAnalysis {
        evidence_rows: terms.iter().map(|term| term.evidence.len()).sum(),
        pending_terms: terms
            .iter()
            .filter(|term| term.decision == QuestTerminologyDecision::PendingHumanApproval)
            .count(),
        conflicting_terms: terms
            .iter()
            .filter(|term| term.decision == QuestTerminologyDecision::ConflictingSiblingCandidates)
            .count(),
        approved_terms: terms
            .iter()
            .filter(|term| term.decision == QuestTerminologyDecision::HumanApproved)
            .count(),
        terms,
    }
}

#[test]
fn 독립검토_상태마다_사람이_선택할_범위를_분리한다() {
    let reviews = review_analysis(vec![
        review_entry(
            "scene-a",
            1,
            Some(QuestTranslationIndependentReviewStatus::ReviewAgrees),
            None,
        ),
        review_entry(
            "scene-a",
            2,
            Some(QuestTranslationIndependentReviewStatus::ReviewProposesChange),
            None,
        ),
        review_entry(
            "scene-a",
            3,
            Some(QuestTranslationIndependentReviewStatus::NeedsHumanDecision),
            None,
        ),
    ]);
    let analysis = audit_quest_translation_batch_decisions(
        &reviews,
        &terminology_analysis(Vec::new()),
        "scene-a",
    )
    .unwrap();

    assert_eq!(
        analysis
            .entries
            .iter()
            .map(|entry| entry.review_action)
            .collect::<Vec<_>>(),
        vec![
            QuestTranslationReviewAction::ApproveReviewedTextOrRevise,
            QuestTranslationReviewAction::ChooseDraftReviewedTextOrRevise,
            QuestTranslationReviewAction::ResolveDependenciesAndApprove,
        ]
    );
    assert_eq!(analysis.pending_human_decisions, 3);
    assert_eq!(analysis.build_eligible_entries, 0);
}

#[test]
fn 독립검토가_빠진_batch는_결정자료를_만들지_않는다() {
    let reviews = review_analysis(vec![review_entry("scene-a", 1, None, None)]);
    let error = audit_quest_translation_batch_decisions(
        &reviews,
        &terminology_analysis(Vec::new()),
        "scene-a",
    )
    .unwrap_err();

    assert!(error.to_string().contains("독립 검토가 완료되지 않음"));
}

#[test]
fn 다른_batch의_검토추가는_선택한_batch_결정자료를_바꾸지_않는다() {
    let selected = review_entry(
        "scene-a",
        1,
        Some(QuestTranslationIndependentReviewStatus::ReviewAgrees),
        None,
    );
    let unrelated = review_entry(
        "scene-b",
        1,
        Some(QuestTranslationIndependentReviewStatus::ReviewProposesChange),
        None,
    );
    let terminology = terminology_analysis(Vec::new());
    let before = audit_quest_translation_batch_decisions(
        &review_analysis(vec![selected.clone()]),
        &terminology,
        "scene-a",
    )
    .unwrap();
    let after = audit_quest_translation_batch_decisions(
        &review_analysis(vec![selected, unrelated]),
        &terminology,
        "scene-a",
    )
    .unwrap();

    assert_eq!(
        render_quest_translation_batch_decisions_tsv(&before),
        render_quest_translation_batch_decisions_tsv(&after)
    );
}

#[test]
fn 없는_batch는_결정자료_요청을_거부한다() {
    let reviews = review_analysis(vec![review_entry(
        "scene-a",
        1,
        Some(QuestTranslationIndependentReviewStatus::ReviewAgrees),
        None,
    )]);
    let error = audit_quest_translation_batch_decisions(
        &reviews,
        &terminology_analysis(Vec::new()),
        "missing-scene",
    )
    .unwrap_err();

    assert!(error.to_string().contains("결정 대상 batch가 없음"));
}

#[test]
fn 검토자산이_있는_모든_batch를_결정자료로_결합한다() {
    let mut reviews = review_analysis(vec![
        review_entry(
            "scene-a",
            1,
            Some(QuestTranslationIndependentReviewStatus::ReviewAgrees),
            None,
        ),
        review_entry(
            "scene-b",
            1,
            Some(QuestTranslationIndependentReviewStatus::ReviewProposesChange),
            None,
        ),
    ]);
    reviews.review_assets = 2;

    let analyses =
        audit_quest_reviewed_batch_decisions(&reviews, &terminology_analysis(Vec::new())).unwrap();

    assert_eq!(
        analyses
            .iter()
            .map(|analysis| analysis.batch_id.as_str())
            .collect::<Vec<_>>(),
        vec!["scene-a", "scene-b"]
    );
}

#[test]
fn 원문에_연결된_용어_후보와_근거를_같은_행에_보존한다() {
    let term = terminology_candidate(
        "quest.character.sample",
        QuestTerminologyDecision::PendingHumanApproval,
    );
    let reviews = review_analysis(vec![review_entry(
        "scene-a",
        1,
        Some(QuestTranslationIndependentReviewStatus::ReviewAgrees),
        Some(&term),
    )]);
    let analysis = audit_quest_translation_batch_decisions(
        &reviews,
        &terminology_analysis(vec![term.clone()]),
        "scene-a",
    )
    .unwrap();
    let report = render_quest_translation_batch_decisions_tsv(&analysis);

    assert!(report.contains("quest.character.sample[原語]=>후보[pending-human-approval]"));
    assert!(report.contains("quest.character.sample=>sibling:terms.tsv[series-approved]=>후보"));
    assert!(report.contains("pending-human-approval\tno"));
}

#[test]
fn 번역배치가_가리킨_용어가_원장에_없으면_거부한다() {
    let missing = terminology_candidate(
        "quest.character.missing",
        QuestTerminologyDecision::PendingHumanApproval,
    );
    let reviews = review_analysis(vec![review_entry(
        "scene-a",
        1,
        Some(QuestTranslationIndependentReviewStatus::ReviewAgrees),
        Some(&missing),
    )]);
    let error = audit_quest_translation_batch_decisions(
        &reviews,
        &terminology_analysis(Vec::new()),
        "scene-a",
    )
    .unwrap_err();

    assert!(error.to_string().contains("결정 자료에 필요한 용어가 없음"));
}

#[test]
fn 전역_결정자료는_용어와_반복의존성과_장면별_대조를_한번에_보존한다() {
    let term = terminology_candidate(
        "quest.character.sample",
        QuestTerminologyDecision::PendingHumanApproval,
    );
    let mut first = review_entry(
        "scene-a",
        1,
        Some(QuestTranslationIndependentReviewStatus::NeedsHumanDecision),
        Some(&term),
    );
    first.decision_dependencies = vec!["quest.style.shared".to_string()];
    let mut second = review_entry(
        "scene-b",
        1,
        Some(QuestTranslationIndependentReviewStatus::NeedsHumanDecision),
        Some(&term),
    );
    second.decision_dependencies = vec!["quest.style.shared".to_string()];
    let mut reviews = review_analysis(vec![first, second]);
    reviews.review_assets = 2;
    reviews.reviewed_entries = 2;

    let workbook =
        audit_quest_translation_decision_workbook(&reviews, &terminology_analysis(vec![term]))
            .unwrap();
    let report = render_quest_translation_decision_workbook_tsv(&workbook);

    assert_eq!(workbook.batch_decisions.len(), 2);
    assert_eq!(workbook.entries, 2);
    assert_eq!(workbook.dependencies.len(), 1);
    assert_eq!(workbook.dependencies[0].occurrences, 2);
    assert_eq!(
        workbook.dependencies[0].batch_ids,
        vec!["scene-a", "scene-b"]
    );
    assert!(report.contains("terminology-decision\tquest.character.sample"));
    assert!(report.contains("shared-dependency-decision\tquest.style.shared"));
    assert!(report.contains("translation-entry-decision\tscene-a-stream-1"));
    assert!(report.contains("protected_source\tcandidate_ko"));
    assert!(report.contains("draft_ko\tindependent_review_status\treview_ko"));
    assert!(report.contains("human_selected_ko\thuman_decision_notes\tbuild_eligible"));
}

#[test]
fn 전역_결정자료는_서로_다른_batch의_같은_stream_id를_거부한다() {
    let first = review_entry(
        "scene-a",
        1,
        Some(QuestTranslationIndependentReviewStatus::ReviewAgrees),
        None,
    );
    let mut second = review_entry(
        "scene-b",
        1,
        Some(QuestTranslationIndependentReviewStatus::ReviewAgrees),
        None,
    );
    second.batch_entry.stream_id = first.batch_entry.stream_id.clone();
    let mut reviews = review_analysis(vec![first, second]);
    reviews.review_assets = 2;
    reviews.reviewed_entries = 2;

    let error =
        audit_quest_translation_decision_workbook(&reviews, &terminology_analysis(Vec::new()))
            .unwrap_err();

    assert!(error
        .to_string()
        .contains("전역 결정 자료의 stream ID가 중복됨"));
}

#[test]
fn 전역_결정자료는_한행의_중복_의존성을_거부한다() {
    let mut review = review_entry(
        "scene-a",
        1,
        Some(QuestTranslationIndependentReviewStatus::NeedsHumanDecision),
        None,
    );
    review.decision_dependencies = vec![
        "quest.style.shared".to_string(),
        "quest.style.shared".to_string(),
    ];
    let mut reviews = review_analysis(vec![review]);
    reviews.review_assets = 1;
    reviews.reviewed_entries = 1;

    let error =
        audit_quest_translation_decision_workbook(&reviews, &terminology_analysis(Vec::new()))
            .unwrap_err();

    assert!(error.to_string().contains("사람 결정 의존성이 중복됨"));
}
