use super::ledger::{
    parse_quest_manual_composition_ledger, parse_quest_manual_composition_rows, COMPOSITION_HEADER,
    SEGMENT_HEADER,
};
use super::model::sha256_text;
use super::source_binding::audit_quest_manual_composition_source_bindings;
use super::*;
use crate::review::{build_quest_source_layout, QuestReviewEntry};
use crate::script::{
    QuestControlMeaning, QuestSeqExtractedTextStream, QuestSeqExtractedToken,
    QuestSeqExtractedTokenKind, QuestTextTerminal,
};

fn audit_inputs(
    queues: &[QuestReviewQueue],
    source_layouts: &[QuestSourceLayout],
    streams: &[QuestSeqExtractedTextStream],
    compositions_input: &str,
    segments_input: &str,
    require_complete_catalog: bool,
) -> Result<QuestManualCompositionAnalysis, QuestManualCompositionError> {
    let ledger = parse_quest_manual_composition_ledger(compositions_input, segments_input)?;
    audit_quest_manual_composition_source_bindings(
        queues,
        source_layouts,
        streams,
        ledger,
        require_complete_catalog,
    )
}

#[test]
fn 자동작성으로_표시된후보는_거부한다() {
    let (_, _, _, compositions, _) = inputs();
    let automatic = compositions.replacen("manual-revision", "automatic-wrap", 1);
    assert!(parse_quest_manual_composition_rows(&automatic).is_err());
}

fn stream() -> QuestSeqExtractedTextStream {
    QuestSeqExtractedTextStream {
        sequence: "TEST.SEQ".to_owned(),
        sequence_sha256: [0; 32],
        record_offset: 0x20,
        record_raw: [0x21, 0, 0, 0],
        stream_offset: 0x24,
        end_offset: 0x30,
        terminal: QuestTextTerminal::DeactivateAndAlign,
        expected_raw_sha256: [0; 32],
        tokens: vec![
            QuestSeqExtractedToken {
                index: 0,
                offset: 0,
                raw: vec![0x82, 0xA0],
                kind: QuestSeqExtractedTokenKind::Glyph,
                source: Some("あ".to_owned()),
                control_code: None,
                control_meaning: None,
            },
            QuestSeqExtractedToken {
                index: 1,
                offset: 2,
                raw: vec![0x04, 0xFF],
                kind: QuestSeqExtractedTokenKind::Control,
                source: None,
                control_code: Some(0x04),
                control_meaning: Some(QuestControlMeaning::AdvanceTextLine),
            },
            QuestSeqExtractedToken {
                index: 2,
                offset: 4,
                raw: vec![0x82, 0xA2],
                kind: QuestSeqExtractedTokenKind::Glyph,
                source: Some("い".to_owned()),
                control_code: None,
                control_meaning: None,
            },
            QuestSeqExtractedToken {
                index: 3,
                offset: 6,
                raw: vec![0x03, 0xFF, 0, 0, 0, 0],
                kind: QuestSeqExtractedTokenKind::Control,
                source: None,
                control_code: Some(0x03),
                control_meaning: Some(QuestControlMeaning::DeactivateInterpreterAndAlign),
            },
        ],
    }
}

fn queue_for_batch(batch_id: &str, stream_id: &str) -> QuestReviewQueue {
    QuestReviewQueue {
        batch_id: batch_id.to_owned(),
        entries: vec![QuestReviewEntry {
            batch_id: batch_id.to_owned(),
            scene_order: 1,
            scene_title: "시험".to_owned(),
            sequence_context: "시험".to_owned(),
            branch: "shared".to_owned(),
            stream_id: stream_id.to_owned(),
            baseline_sha256: "baseline".to_owned(),
            speaker: "화자".to_owned(),
            addressee: "청자".to_owned(),
            context_note: "문맥".to_owned(),
            protected_source: "あい".to_owned(),
            control_signature: "FF04,FF03".to_owned(),
            runtime_evidence: None,
            work_notes: "초벌".to_owned(),
            independent_review_status: "review-agrees".to_owned(),
            review_notes: "검토".to_owned(),
            review_action: "approve-reviewed-or-revise".to_owned(),
            candidates: Vec::new(),
            terms: Vec::new(),
            dependency_ids: Vec::new(),
            selected_korean: None,
            approval_notes: None,
            approval_build_eligible: false,
            waiting_reasons: vec!["문장 미승인".to_owned()],
        }],
        dependencies: Vec::new(),
        approval_manifest_sha256: "approval".to_owned(),
        approved_entries: 0,
        approval_build_eligible_entries: 0,
    }
}

fn queue(stream_id: &str) -> QuestReviewQueue {
    queue_for_batch("test-batch", stream_id)
}

fn inputs() -> (
    QuestReviewQueue,
    Vec<QuestSourceLayout>,
    Vec<QuestSeqExtractedTextStream>,
    String,
    String,
) {
    let stream = stream();
    let stream_id = stream.id();
    let source_layout = build_quest_source_layout(&stream).unwrap();
    let selected = "가 나";
    let compositions = format!(
        "{COMPOSITION_HEADER}\nquest-manual-composition\ttest-composition\ttest-batch\t{stream_id}\tmanual-revision\t{selected}\t{}\t{}\t\t\tmanual-draft-needs-geometry\t사람이 작성함\n",
        sha256_text(selected), source_layout.source_stream_sha256
    );
    let segments = format!(
        "{SEGMENT_HEADER}\nquest-manual-composition-segment\ttest-composition\t1\t1\t1\t0\t1\t가\t1\tFF04\nquest-manual-composition-segment\ttest-composition\t2\t1\t2\t2\t3\t나\t3\tFF03\n"
    );
    (
        queue(&stream_id),
        vec![source_layout],
        vec![stream],
        compositions,
        segments,
    )
}

#[test]
fn 사람이쓴segment만_그대로배치하고_geometry승격은_하지않는다() {
    let (queue, layouts, streams, compositions, segments) = inputs();
    let analysis = audit_inputs(
        std::slice::from_ref(&queue),
        &layouts,
        &streams,
        &compositions,
        &segments,
        true,
    )
    .unwrap();

    assert_eq!(analysis.manually_authored, 1);
    assert_eq!(analysis.compositions[0].candidate_glyph_run_lengths, [1, 1]);
    assert_eq!(analysis.compositions[0].selected_korean, "가 나");
    assert_eq!(analysis.compositions[0].display_korean, "가나");
    assert_eq!(analysis.geometry_verified, 0);
    assert_eq!(analysis.layout_approved, 0);
    assert_eq!(analysis.product_eligible, 0);
}

fn audit_punctuation(
    source: &str,
    selected: &str,
) -> Result<QuestManualCompositionAnalysis, QuestManualCompositionError> {
    let mut stream = stream();
    let glyph = stream.tokens[0].clone();
    let mut terminal = stream.tokens[3].clone();
    stream.tokens = source
        .chars()
        .enumerate()
        .map(|(index, character)| QuestSeqExtractedToken {
            index,
            offset: index * 2,
            source: Some(character.to_string()),
            ..glyph.clone()
        })
        .collect();
    let anchor = stream.tokens.len();
    stream.tokens.push(QuestSeqExtractedToken {
        index: anchor,
        offset: anchor * 2,
        raw: vec![0x0f, 0xff],
        kind: QuestSeqExtractedTokenKind::Control,
        source: None,
        control_code: Some(0x0f),
        control_meaning: Some(QuestControlMeaning::StartInputWait),
    });
    terminal.index = anchor + 1;
    terminal.offset = (anchor + 1) * 2;
    stream.tokens.push(terminal);
    let source_layout = build_quest_source_layout(&stream).unwrap();
    let stream_id = stream.id();
    let compositions = format!(
        "{COMPOSITION_HEADER}\nquest-manual-composition\tpunctuation\ttest-batch\t{stream_id}\tmanual-revision\t{selected}\t{}\t{}\t\t\tmanual-draft-needs-geometry\t원문 기호 보존\n",
        sha256_text(selected), source_layout.source_stream_sha256
    );
    let segments = format!(
        "{SEGMENT_HEADER}\nquest-manual-composition-segment\tpunctuation\t1\t1\t1\t0\t{}\t{selected}\t{anchor}\tFF0F\n",
        selected.chars().count()
    );
    audit_inputs(
        &[queue(&stream_id)],
        &[source_layout],
        &[stream],
        &compositions,
        &segments,
        true,
    )
}

#[test]
fn 원문_말줄임표만_있는_대사는_한글을_덧붙이지_않고_보존한다() {
    let analysis = audit_punctuation("・・・", "…").unwrap();
    assert_eq!(analysis.compositions[0].display_korean, "…");
    assert_eq!(analysis.compositions[0].candidate_glyph_run_lengths, [1, 0]);
    assert_eq!(analysis.product_eligible, 0);
}

#[test]
fn 원문_대사나_다른_기호를_말줄임표로_대체하면_거부한다() {
    for source in ["あいう", "・・", "！！！", "TODO"] {
        assert!(audit_punctuation(source, "…").is_err(), "{source}");
    }
    assert!(audit_punctuation("TODO", "TODO").is_err());
}

#[test]
fn 같은행_control_사이의_선택문공백은_표시glyph로_보존한다() {
    let mut stream = stream();
    stream.tokens[1] = QuestSeqExtractedToken {
        index: 1,
        offset: 2,
        raw: vec![0x18, 0xFF, 0, 0],
        kind: QuestSeqExtractedTokenKind::Control,
        source: None,
        control_code: Some(0x18),
        control_meaning: Some(QuestControlMeaning::SetGlyphEffectMode),
    };
    let stream_id = stream.id();
    let source_layout = build_quest_source_layout(&stream).unwrap();
    let selected = "가 나";
    let compositions = format!(
        "{COMPOSITION_HEADER}\nquest-manual-composition\ttest-inline-space\ttest-batch\t{stream_id}\tmanual-revision\t{selected}\t{}\t{}\t\t\tmanual-draft-needs-geometry\t같은 행의 실제 공백을 사람이 작성함\n",
        sha256_text(selected),
        source_layout.source_stream_sha256,
    );
    let segments = format!(
        "{SEGMENT_HEADER}\nquest-manual-composition-segment\ttest-inline-space\t1\t1\t1\t0\t1\t가\t1\tFF18\nquest-manual-composition-segment\ttest-inline-space\t2\t1\t1\t2\t3\t나\t3\tFF03\n"
    );

    let analysis = audit_inputs(
        &[queue(&stream_id)],
        std::slice::from_ref(&source_layout),
        std::slice::from_ref(&stream),
        &compositions,
        &segments,
        true,
    )
    .unwrap();

    assert_eq!(analysis.compositions[0].display_korean, "가 나");
    assert_eq!(analysis.compositions[0].normalized_display_korean, "가　나");
    assert_eq!(analysis.compositions[0].candidate_glyph_run_lengths, [1, 2]);
}

#[test]
fn segment범위누락과_control_anchor변조를_자동검출한다() {
    let (queue, layouts, streams, compositions, segments) = inputs();
    let wrong_range = segments.replacen("\t0\t1\t가", "\t0\t2\t가", 1);
    assert!(audit_inputs(
        std::slice::from_ref(&queue),
        &layouts,
        &streams,
        &compositions,
        &wrong_range,
        true,
    )
    .is_err());

    let missing_anchor = segments.replacen("\t1\tFF04", "\t99\tFF04", 1);
    assert!(audit_inputs(
        std::slice::from_ref(&queue),
        &layouts,
        &streams,
        &compositions,
        &missing_anchor,
        true,
    )
    .is_err());

    let ignored_line_advance = segments.replacen("\t2\t1\t2\t2\t3\t나", "\t2\t1\t1\t2\t3\t나", 1);
    assert!(audit_inputs(
        std::slice::from_ref(&queue),
        &layouts,
        &streams,
        &compositions,
        &ignored_line_advance,
        true,
    )
    .is_err());
}

fn input_wait_stream() -> QuestSeqExtractedTextStream {
    QuestSeqExtractedTextStream {
        sequence: "INPUT_WAIT.SEQ".to_owned(),
        sequence_sha256: [0; 32],
        record_offset: 0x20,
        record_raw: [0x21, 0, 0, 0],
        stream_offset: 0x24,
        end_offset: 0x32,
        terminal: QuestTextTerminal::DeactivateAndAlign,
        expected_raw_sha256: [0; 32],
        tokens: vec![
            QuestSeqExtractedToken {
                index: 0,
                offset: 0,
                raw: vec![0x82, 0xA0],
                kind: QuestSeqExtractedTokenKind::Glyph,
                source: Some("あ".to_owned()),
                control_code: None,
                control_meaning: None,
            },
            QuestSeqExtractedToken {
                index: 1,
                offset: 2,
                raw: vec![0x0F, 0xFF, 1, 0],
                kind: QuestSeqExtractedTokenKind::Control,
                source: None,
                control_code: Some(0x0F),
                control_meaning: Some(QuestControlMeaning::StartInputWait),
            },
            QuestSeqExtractedToken {
                index: 2,
                offset: 6,
                raw: vec![0x82, 0xA2],
                kind: QuestSeqExtractedTokenKind::Glyph,
                source: Some("い".to_owned()),
                control_code: None,
                control_meaning: None,
            },
            QuestSeqExtractedToken {
                index: 3,
                offset: 8,
                raw: vec![0x03, 0xFF, 0, 0, 0, 0],
                kind: QuestSeqExtractedTokenKind::Control,
                source: None,
                control_code: Some(0x03),
                control_meaning: Some(QuestControlMeaning::DeactivateInterpreterAndAlign),
            },
        ],
    }
}

#[test]
fn ff0f는_input_phase만_나누고_visual_page_clear로_해석하지않는다() {
    let stream = input_wait_stream();
    let stream_id = stream.id();
    let source_layout = build_quest_source_layout(&stream).unwrap();
    let selected = "가나";
    let compositions = format!(
        "{COMPOSITION_HEADER}\nquest-manual-composition\ttest-input-phase\ttest-batch\t{stream_id}\tmanual-revision\t{selected}\t{}\t{}\t\t\tmanual-draft-needs-geometry\t입력 단계를 사람이 작성함\n",
        sha256_text(selected),
        source_layout.source_stream_sha256,
    );
    let segments = format!(
        "{SEGMENT_HEADER}\nquest-manual-composition-segment\ttest-input-phase\t1\t1\t1\t0\t1\t가\t1\tFF0F\nquest-manual-composition-segment\ttest-input-phase\t2\t2\t1\t1\t2\t나\t3\tFF03\n"
    );

    let analysis = audit_inputs(
        &[queue(&stream_id)],
        std::slice::from_ref(&source_layout),
        std::slice::from_ref(&stream),
        &compositions,
        &segments,
        true,
    )
    .unwrap();
    assert_eq!(analysis.compositions[0].segments[1].input_phase, 2);
    assert_eq!(analysis.compositions[0].segments[1].phase_line, 1);

    let ignored_input_wait = segments.replacen(
        "\t2\t2\t1\t1\t2\t나\t3\tFF03",
        "\t2\t1\t1\t1\t2\t나\t3\tFF03",
        1,
    );
    let error = audit_inputs(
        &[queue(&stream_id)],
        &[source_layout],
        &[stream],
        &compositions,
        &ignored_input_wait,
        true,
    )
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("같은 input phase/phase line이 원문 입력 대기"));
}

fn two_batch_inputs() -> (
    Vec<QuestReviewQueue>,
    Vec<QuestSourceLayout>,
    Vec<QuestSeqExtractedTextStream>,
    String,
    String,
) {
    let first = stream();
    let mut second = stream();
    second.record_offset = 0x40;
    second.stream_offset = 0x44;
    second.end_offset = 0x50;
    let first_id = first.id();
    let second_id = second.id();
    let first_layout = build_quest_source_layout(&first).unwrap();
    let second_layout = build_quest_source_layout(&second).unwrap();
    let first_selected = "가 나";
    let second_selected = "다 라";
    let compositions = format!(
        "{COMPOSITION_HEADER}\nquest-manual-composition\tfirst-composition\tfirst-batch\t{first_id}\tmanual-revision\t{first_selected}\t{}\t{}\t\t\tmanual-draft-needs-geometry\t첫 장면을 사람이 작성함\nquest-manual-composition\tsecond-composition\tsecond-batch\t{second_id}\tmanual-revision\t{second_selected}\t{}\t{}\t\t\tmanual-draft-needs-geometry\t둘째 장면을 사람이 작성함\n",
        sha256_text(first_selected),
        first_layout.source_stream_sha256,
        sha256_text(second_selected),
        second_layout.source_stream_sha256,
    );
    let segments = format!(
        "{SEGMENT_HEADER}\nquest-manual-composition-segment\tfirst-composition\t1\t1\t1\t0\t1\t가\t1\tFF04\nquest-manual-composition-segment\tfirst-composition\t2\t1\t2\t2\t3\t나\t3\tFF03\nquest-manual-composition-segment\tsecond-composition\t1\t1\t1\t0\t1\t다\t1\tFF04\nquest-manual-composition-segment\tsecond-composition\t2\t1\t2\t2\t3\t라\t3\tFF03\n"
    );
    (
        vec![
            queue_for_batch("first-batch", &first_id),
            queue_for_batch("second-batch", &second_id),
        ],
        vec![first_layout, second_layout],
        vec![first, second],
        compositions,
        segments,
    )
}

#[test]
fn 여러_batch의_수동composition을_하나의개발입력으로_검증한다() {
    let (queues, layouts, streams, compositions, segments) = two_batch_inputs();
    let analysis =
        audit_inputs(&queues, &layouts, &streams, &compositions, &segments, true).unwrap();

    assert_eq!(
        analysis
            .compositions
            .iter()
            .map(|composition| composition.stream_id.as_str())
            .collect::<Vec<_>>(),
        queues
            .iter()
            .flat_map(|queue| queue.entries.iter().map(|entry| entry.stream_id.as_str()))
            .collect::<Vec<_>>()
    );
    assert!(analysis
        .compositions
        .iter()
        .any(|composition| composition.batch_id == "first-batch"));
    assert!(analysis
        .compositions
        .iter()
        .any(|composition| composition.batch_id == "second-batch"));
}

#[test]
fn 한_batch에서_사람이확정한stream만_부분조판할수있다() {
    let (mut queues, layouts, streams, compositions, segments) = two_batch_inputs();
    let second_entry = queues.remove(1).entries.remove(0);
    let batch_id = queues[0].batch_id.clone();
    queues[0].entries.push(QuestReviewEntry {
        batch_id,
        ..second_entry
    });
    let one_composition = compositions.lines().take(2).collect::<Vec<_>>().join("\n") + "\n";
    let one_segment_set = segments.lines().take(3).collect::<Vec<_>>().join("\n") + "\n";

    let analysis = audit_inputs(
        &queues,
        &layouts,
        &streams,
        &one_composition,
        &one_segment_set,
        true,
    )
    .unwrap();

    assert_eq!(queues[0].entries.len(), 2);
    assert_eq!(analysis.manually_authored, 1);
    assert_eq!(
        analysis.compositions[0].stream_id,
        queues[0].entries[0].stream_id
    );
}

#[test]
fn 단일batch_ui는_다른batch_수동안을_자동으로섞지않는다() {
    let (queues, layouts, streams, compositions, segments) = two_batch_inputs();
    let analysis = audit_inputs(
        std::slice::from_ref(&queues[0]),
        &layouts[..1],
        &streams,
        &compositions,
        &segments,
        false,
    )
    .unwrap();

    assert_eq!(analysis.compositions.len(), 1);
    assert_eq!(analysis.compositions[0].batch_id, "first-batch");
}

#[test]
fn 전체원장의_batch가_검토scope에_없으면_실패한다() {
    let (queues, layouts, streams, compositions, segments) = two_batch_inputs();
    let unknown_batch = compositions.replacen("second-batch", "unknown-batch", 1);
    let result = audit_inputs(&queues, &layouts, &streams, &unknown_batch, &segments, true);

    assert!(result
        .unwrap_err()
        .to_string()
        .contains("검토 batch 모집단 불일치"));
}
