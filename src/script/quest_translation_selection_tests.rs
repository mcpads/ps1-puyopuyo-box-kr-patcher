use super::*;
use crate::script::{
    QuestSeqTranslationUnitPolicy, QuestTranslationBatchEntry, QuestTranslationBatchRole,
    QuestTranslationContextConfidence, QuestTranslationReviewEntry,
    QuestTranslationReviewRequirement, QuestTranslationWorkStatus,
};

fn selection_entry(
    stream_id: &str,
    batch_id: &str,
    review_status: QuestTranslationIndependentReviewStatus,
    draft_korean: &str,
    selected_korean: Option<&str>,
) -> QuestTranslationSelectionEntry {
    QuestTranslationSelectionEntry {
        stream_id: stream_id.to_owned(),
        batch_id: batch_id.to_owned(),
        review_status,
        disposition: if selected_korean.is_some() {
            QuestTranslationSelectionDisposition::SelectReviewedKorean
        } else {
            QuestTranslationSelectionDisposition::RetainSource
        },
        protected_source: "アルル「げんぶん".to_owned(),
        draft_korean: draft_korean.to_owned(),
        reviewed_korean: selected_korean.unwrap_or_default().to_owned(),
        selected_korean: selected_korean.map(str::to_owned),
        selected_hangul: "stale".to_owned(),
        decision_dependencies: if selected_korean.is_some() {
            Vec::new()
        } else {
            vec!["사람 결정".to_owned()]
        },
        translation_approved: false,
        product_eligible: false,
    }
}

fn base() -> QuestTranslationSelectionAnalysis {
    let mut analysis = summarize_entries(vec![
        selection_entry(
            "stream-1",
            "batch-1",
            QuestTranslationIndependentReviewStatus::ReviewAgrees,
            "가나",
            Some("가나"),
        ),
        selection_entry(
            "stream-2",
            "batch-2",
            QuestTranslationIndependentReviewStatus::NeedsHumanDecision,
            "나다",
            None,
        ),
    ]);
    analysis.entries[1].reviewed_korean = "다라".to_owned();
    analysis
}

fn manual(
    stream_id: &str,
    batch_id: &str,
    selected_korean: &str,
) -> QuestManualCompositionSelection {
    QuestManualCompositionSelection {
        stream_id: stream_id.to_owned(),
        batch_id: batch_id.to_owned(),
        selected_korean: selected_korean.to_owned(),
    }
}

#[test]
fn 검토동의와수정제안만_검토안을_기술선택으로고른다() {
    assert_eq!(
        disposition(QuestTranslationIndependentReviewStatus::ReviewAgrees),
        QuestTranslationSelectionDisposition::SelectReviewedKorean
    );
    assert_eq!(
        disposition(QuestTranslationIndependentReviewStatus::ReviewProposesChange),
        QuestTranslationSelectionDisposition::SelectReviewedKorean
    );
    assert_eq!(
        disposition(QuestTranslationIndependentReviewStatus::NeedsHumanDecision),
        QuestTranslationSelectionDisposition::RetainSource
    );
}

#[test]
fn 수동composition은_독립된선택역할을가진다() {
    assert_eq!(
        QuestTranslationSelectionDisposition::SelectManualComposition.label(),
        "select-manual-composition"
    );
}

#[test]
fn development는_수동composition만고르고_나머지는원문을유지한다() {
    let base = base();
    let result = select_quest_manual_compositions_for_development(
        &base,
        &[manual("stream-2", "batch-2", "바가")],
    )
    .expect("수동 composition development 선택");

    assert_eq!(result.selected_entries, 1);
    assert_eq!(result.retained_source_entries, 1);
    assert_eq!(result.selected_manual_compositions, 1);
    assert_eq!(result.entries[0].selected_korean, None);
    assert_eq!(
        result.entries[0].disposition,
        QuestTranslationSelectionDisposition::RetainSource
    );
    assert_eq!(result.entries[1].selected_korean.as_deref(), Some("바가"));
    assert_eq!(
        result.entries[1].disposition,
        QuestTranslationSelectionDisposition::SelectManualComposition
    );
    assert_eq!(result.translation_approved_entries, 0);
    assert_eq!(result.product_eligible_entries, 0);
    assert_eq!(base.selected_entries, 1);
    assert_eq!(base.entries[0].selected_korean.as_deref(), Some("가나"));
}

#[test]
fn 빈수동composition인_development는_모든stream의원문을유지한다() {
    let result = select_quest_manual_compositions_for_development(&base(), &[])
        .expect("빈 수동 composition development 선택");

    assert_eq!(result.selected_entries, 0);
    assert_eq!(result.retained_source_entries, 2);
    assert_eq!(result.selected_manual_compositions, 0);
    assert!(result.entries.iter().all(|entry| entry.disposition
        == QuestTranslationSelectionDisposition::RetainSource
        && entry.selected_korean.is_none()));
}

#[test]
fn 한글레퍼토리는_완성형음절만_중복없이센다() {
    assert_eq!(
        hangul_repertoire("아르르 ABC 아!")
            .into_iter()
            .collect::<String>(),
        "르아"
    );
}

#[test]
fn 수동composition을_적용하고_모든파생집계를다시계산한다() {
    let base = base();
    let result = apply_quest_manual_composition_selections(
        &base,
        &[
            manual("stream-1", "batch-1", "라마"),
            manual("stream-2", "batch-2", "바가"),
        ],
    )
    .expect("수동 composition 선택");

    assert_eq!(result.selected_entries, 2);
    assert_eq!(result.retained_source_entries, 0);
    assert_eq!(result.selected_entries_with_review_agreement, 1);
    assert_eq!(result.selected_entries_with_review_proposal, 0);
    assert_eq!(result.selected_manual_compositions, 2);
    assert_eq!(result.selected_batches, 2);
    assert_eq!(result.selected_hangul_repertoire, "가라마바");
    assert_eq!(result.draft_hangul_repertoire, "가나다");
    assert_eq!(result.new_cache_glyphs, "라마바");
    assert_eq!(result.draft_only_glyphs, "나다");
    assert_eq!(result.translation_approved_entries, 0);
    assert_eq!(result.product_eligible_entries, 0);
    assert_eq!(
        result.entries[0].disposition,
        QuestTranslationSelectionDisposition::SelectManualComposition
    );
    assert_eq!(result.entries[0].selected_hangul, "라마");
    assert_eq!(result.entries[1].selected_hangul, "가바");
    assert_eq!(base.selected_manual_compositions, 0);
    assert_eq!(base.entries[0].selected_korean.as_deref(), Some("가나"));
}

#[test]
fn 빈선택도_entry에서_aggregate와레퍼토리를다시계산한다() {
    let mut base = base();
    base.selected_entries = 99;
    base.retained_source_entries = 99;
    base.selected_entries_with_review_agreement = 99;
    base.selected_batches = 99;
    base.selected_hangul_repertoire = "오래된값".to_owned();
    base.draft_hangul_repertoire = "오래된값".to_owned();
    base.entries[0].selected_hangul = "오래된값".to_owned();

    let result =
        apply_quest_manual_composition_selections(&base, &[]).expect("빈 수동 선택의 재집계");

    assert_eq!(result.selected_entries, 1);
    assert_eq!(result.retained_source_entries, 1);
    assert_eq!(result.selected_entries_with_review_agreement, 1);
    assert_eq!(result.selected_manual_compositions, 0);
    assert_eq!(result.selected_batches, 1);
    assert_eq!(result.selected_hangul_repertoire, "가나");
    assert_eq!(result.draft_hangul_repertoire, "가나다");
    assert_eq!(result.entries[0].selected_hangul, "가나");
}

#[test]
fn 대상중복과batch불일치와불완전한국어를_실패폐쇄한다() {
    let base = base();

    assert!(apply_quest_manual_composition_selections(
        &base,
        &[manual("missing", "batch-1", "수동 문장")]
    )
    .is_err());
    assert!(apply_quest_manual_composition_selections(
        &base,
        &[
            manual("stream-1", "batch-1", "첫 문장"),
            manual("stream-1", "batch-1", "둘째 문장"),
        ]
    )
    .is_err());
    assert!(apply_quest_manual_composition_selections(
        &base,
        &[manual("stream-1", "wrong-batch", "수동 문장")]
    )
    .is_err());
    assert!(apply_quest_manual_composition_selections(
        &base,
        &[manual("stream-1", "batch-1", "アルル")]
    )
    .is_err());
    assert!(apply_quest_manual_composition_selections(
        &base,
        &[manual("stream-1", "batch-1", "수동 {name}")]
    )
    .is_err());
}

#[test]
fn 중복된기준stream은_선택전에실패한다() {
    let mut base = base();
    base.entries.push(base.entries[0].clone());

    assert!(apply_quest_manual_composition_selections(
        &base,
        &[manual("stream-1", "batch-1", "수동 문장")]
    )
    .is_err());
}

fn reviewed_batch_entry() -> QuestTranslationBatchEntry {
    QuestTranslationBatchEntry {
        batch_id: "batch-1".to_owned(),
        batch_context_sha256: "context".to_owned(),
        scene_id: "scene".to_owned(),
        scene_title: "장면".to_owned(),
        sequence_context: "문맥".to_owned(),
        source_evidence: "근거".to_owned(),
        scene_order: 1,
        branch: "shared".to_owned(),
        stream_id: "stream-1".to_owned(),
        role: QuestTranslationBatchRole::Translate,
        speaker_id: "speaker".to_owned(),
        speaker: "화자".to_owned(),
        addressee_id: "listener".to_owned(),
        addressee: "청자".to_owned(),
        confidence: QuestTranslationContextConfidence::Confirmed,
        context_note: "확정".to_owned(),
        protected_raw_hex: "0000".to_owned(),
        protected_source: "アルル".to_owned(),
        translation_policy: QuestSeqTranslationUnitPolicy::JapaneseTranslationCandidate,
        control_signature: "FF03".to_owned(),
        runtime_evidence: None,
        matched_term_ids: Vec::new(),
        matched_term_decisions: Vec::new(),
        korean_draft: "초벌".to_owned(),
        work_status: QuestTranslationWorkStatus::DraftNeedsIndependentReview,
        work_notes: "초벌 메모".to_owned(),
    }
}

#[test]
fn 기본audit은_수동composition을선택하지않는다() {
    let reviews = QuestTranslationReviewAnalysis {
        entries: vec![QuestTranslationReviewEntry {
            batch_entry: reviewed_batch_entry(),
            requirement: QuestTranslationReviewRequirement::IndependentReview,
            independent_review_status: Some(QuestTranslationIndependentReviewStatus::ReviewAgrees),
            reviewed_korean: "아르르".to_owned(),
            decision_dependencies: Vec::new(),
            review_notes: "검토".to_owned(),
            review_asset_sha256: Some("review".to_owned()),
        }],
        review_assets: 1,
        protected_context_entries: 0,
        human_term_decision_entries: 0,
        human_context_decision_entries: 0,
        runtime_sample_entries: 0,
        independent_review_queue_entries: 1,
        reviewed_entries: 1,
        review_agreements: 1,
        review_proposals: 0,
        review_human_decision_rows: 0,
        unreviewed_drafts: 0,
        build_eligible_entries: 0,
        translation_batch_report_sha256: "batch".to_owned(),
    };

    let result = audit_quest_translation_selection(&reviews).expect("기본 검토안 audit");

    assert_eq!(result.selected_entries, 1);
    assert_eq!(result.selected_entries_with_review_agreement, 1);
    assert_eq!(result.selected_manual_compositions, 0);
}
