use super::*;
use crate::review::{
    QuestCompositionCandidateBasis, QuestManualComposition, QuestManualCompositionAnalysis,
    QuestManualCompositionSegment, QuestManualCompositionStatus, QuestReviewBatchSummary,
    QuestReviewCandidate, QuestReviewCandidateSource, QuestReviewCatalog, QuestReviewEntry,
    QuestReviewQueue, QuestSourceLayoutControl, QuestSourceLayoutSpan,
};
use crate::script::QuestControlMeaning;

#[test]
fn 원문표시구조는_미확정문맥과_미조판경계를_숨기지_않는다() {
    let layout = QuestSourceLayout {
        stream_id: "QUEST-SEQ-TEXT-STREAM:TEST.SEQ:0x0020".to_owned(),
        source_stream_sha256: "source".to_owned(),
        spans: vec![QuestSourceLayoutSpan {
            sequence: 1,
            protected_source: "原文".to_owned(),
            glyphs: 2,
            window_index: None,
            line_index: None,
            input_phase: 0,
            following_control: QuestSourceLayoutControl {
                token_index: 2,
                code: 0x0F,
                meaning: QuestControlMeaning::StartInputWait,
                raw_hex: "0fff0100".to_owned(),
            },
        }],
        glyphs: 2,
        controls: 1,
        unresolved_window_spans: 1,
        unresolved_line_spans: 1,
    };
    let mut html = String::new();

    render_source_layout(&mut html, &layout);

    assert!(html.contains("window 미확정 span 1개"));
    assert!(html.contains("line 미확정 span 1개"));
    assert!(html.contains("FF0F"));
    assert!(html.contains("FF0F는 input wait 경계이며 visual page clear를 뜻하지 않습니다"));
    assert!(html.contains("명시적 사람 조판 없이는"));
    assert!(!html.contains("data:image"));
}

fn review_entry(selected_korean: Option<&str>) -> QuestReviewEntry {
    QuestReviewEntry {
        batch_id: "test-batch".to_owned(),
        scene_order: 1,
        scene_title: "시험 장면".to_owned(),
        sequence_context: "시험 문맥".to_owned(),
        branch: "shared".to_owned(),
        stream_id: "QUEST-SEQ-TEXT-STREAM:TEST.SEQ:0x0020".to_owned(),
        baseline_sha256: "baseline".to_owned(),
        speaker: "화자".to_owned(),
        addressee: "청자".to_owned(),
        context_note: "상세 문맥".to_owned(),
        protected_source: "原文".to_owned(),
        control_signature: "FF04".to_owned(),
        runtime_evidence: None,
        work_notes: "초벌 근거".to_owned(),
        independent_review_status: "review-proposes-change".to_owned(),
        review_notes: "독립 검토 근거".to_owned(),
        review_action: "approve-reviewed-or-revise".to_owned(),
        candidates: vec![QuestReviewCandidate {
            korean: "자동 검출 후보".to_owned(),
            sources: vec![QuestReviewCandidateSource::IndependentReview],
        }],
        terms: Vec::new(),
        dependency_ids: Vec::new(),
        selected_korean: selected_korean.map(str::to_owned),
        approval_notes: None,
        approval_build_eligible: false,
        waiting_reasons: vec!["문장 미승인".to_owned()],
    }
}

fn source_layout() -> QuestSourceLayout {
    QuestSourceLayout {
        stream_id: "QUEST-SEQ-TEXT-STREAM:TEST.SEQ:0x0020".to_owned(),
        source_stream_sha256: "source".to_owned(),
        spans: vec![QuestSourceLayoutSpan {
            sequence: 1,
            protected_source: "原文".to_owned(),
            glyphs: 2,
            window_index: None,
            line_index: None,
            input_phase: 0,
            following_control: QuestSourceLayoutControl {
                token_index: 2,
                code: 0x04,
                meaning: QuestControlMeaning::AdvanceTextLine,
                raw_hex: "04ff".to_owned(),
            },
        }],
        glyphs: 2,
        controls: 1,
        unresolved_window_spans: 1,
        unresolved_line_spans: 1,
    }
}

fn manual_analysis() -> QuestManualCompositionAnalysis {
    QuestManualCompositionAnalysis {
        compositions: vec![QuestManualComposition {
            id: "manual-test".to_owned(),
            batch_id: "test-batch".to_owned(),
            stream_id: "QUEST-SEQ-TEXT-STREAM:TEST.SEQ:0x0020".to_owned(),
            candidate_basis: QuestCompositionCandidateBasis::ManualRevision,
            selected_korean: "수동 수정".to_owned(),
            selected_text_sha256: "selected".to_owned(),
            normalized_text_sha256: "normalized".to_owned(),
            display_korean: "수동수정".to_owned(),
            normalized_display_korean: "수동수정".to_owned(),
            source_stream_sha256: "source".to_owned(),
            geometry_profile_id: None,
            geometry_sha256: None,
            status: QuestManualCompositionStatus::ManualDraftNeedsGeometry,
            notes: "사람이 직접 작성했다.".to_owned(),
            segments: vec![
                QuestManualCompositionSegment {
                    order: 1,
                    input_phase: 1,
                    phase_line: 1,
                    selected_text_start: 0,
                    selected_text_end: 2,
                    text: "수동".to_owned(),
                    following_control_token_index: 1,
                    following_control_code: 0x0b,
                },
                QuestManualCompositionSegment {
                    order: 2,
                    input_phase: 1,
                    phase_line: 2,
                    selected_text_start: 3,
                    selected_text_end: 5,
                    text: "수정".to_owned(),
                    following_control_token_index: 2,
                    following_control_code: 0x04,
                },
            ],
            candidate_glyph_run_lengths: vec![4],
            geometry_verified: false,
            layout_approved: false,
        }],
        manually_authored: 1,
        geometry_verified: 0,
        layout_approved: 0,
        product_eligible: 0,
    }
}

fn review_catalog(queue: &QuestReviewQueue) -> QuestReviewCatalog {
    QuestReviewCatalog {
        batches: vec![QuestReviewBatchSummary {
            batch_id: queue.batch_id.clone(),
            scene_title: queue.entries[0].scene_title.clone(),
            entries: queue.entries.len(),
            approved_entries: queue.approved_entries,
            approval_build_eligible_entries: queue.approval_build_eligible_entries,
        }],
        entries: queue.entries.len(),
        approved_entries: queue.approved_entries,
        approval_build_eligible_entries: queue.approval_build_eligible_entries,
    }
}

#[test]
fn 미승인입력은_자동검출후보로_미리채우지않는다() {
    let queue = QuestReviewQueue {
        batch_id: "test-batch".to_owned(),
        entries: vec![review_entry(None)],
        dependencies: Vec::new(),
        approval_manifest_sha256: "manifest".to_owned(),
        approved_entries: 0,
        approval_build_eligible_entries: 0,
    };

    let catalog = review_catalog(&queue);
    let manual = manual_analysis();
    let html = render_quest_review_page(
        &catalog,
        &queue,
        &[source_layout()],
        &manual.compositions,
        "token",
        None,
    )
    .unwrap();

    assert!(html.contains("input phase 1 · phase line 1"));
    assert!(html.contains("input phase 1 · phase line 2"));
    assert!(!html.contains("page 1 · line"));
    assert!(html.contains("이 수동 수정안 선택"));
    assert!(html.contains("data-value=\"수동 수정\""));
    assert!(html.contains("<textarea id=\"selection-0\" name=selected_ko required></textarea>"));
    assert!(!html.contains(
        "<textarea id=\"selection-0\" name=selected_ko required>자동 검출 후보</textarea>"
    ));
}

#[test]
fn 이미사람이저장한선택만_입력칸에복원한다() {
    let queue = QuestReviewQueue {
        batch_id: "test-batch".to_owned(),
        entries: vec![review_entry(Some("사람 승인문"))],
        dependencies: Vec::new(),
        approval_manifest_sha256: "manifest".to_owned(),
        approved_entries: 1,
        approval_build_eligible_entries: 0,
    };

    let catalog = review_catalog(&queue);
    let manual = manual_analysis();
    let html = render_quest_review_page(
        &catalog,
        &queue,
        &[source_layout()],
        &manual.compositions,
        "token",
        None,
    )
    .unwrap();

    assert!(html
        .contains("<textarea id=\"selection-0\" name=selected_ko required>사람 승인문</textarea>"));
}

#[test]
fn 전역batch를_탐색하고_수동안없는항목은_자동배치하지않는다() {
    let queue = QuestReviewQueue {
        batch_id: "test-batch".to_owned(),
        entries: vec![review_entry(None)],
        dependencies: Vec::new(),
        approval_manifest_sha256: "manifest".to_owned(),
        approved_entries: 0,
        approval_build_eligible_entries: 0,
    };
    let mut catalog = review_catalog(&queue);
    catalog.batches.push(QuestReviewBatchSummary {
        batch_id: "새-batch".to_owned(),
        scene_title: "다음 장면".to_owned(),
        entries: 3,
        approved_entries: 1,
        approval_build_eligible_entries: 0,
    });
    catalog.entries = 4;
    catalog.approved_entries = 1;

    let html =
        render_quest_review_page(&catalog, &queue, &[source_layout()], &[], "token", None).unwrap();

    assert!(html.contains("총 2 batch · 4문장"));
    assert!(html.contains("/?batch=%EC%83%88-batch"));
    assert!(html.contains("아직 사람이 직접 작성한 문장·input phase·phase line 조판이 없습니다"));
    assert!(!html.contains("이 수동 수정안 선택"));
    assert!(html.contains("name=batch_id value=\"test-batch\""));
}
