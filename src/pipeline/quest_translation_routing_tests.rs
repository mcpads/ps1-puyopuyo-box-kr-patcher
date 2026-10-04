use super::*;
use crate::script::{QuestControlMeaning, QuestSeqExtractedToken};

fn ff03_stream(source_glyphs: usize) -> QuestSeqExtractedTextStream {
    let mut tokens = Vec::new();
    let mut offset = 4usize;
    for index in 0..source_glyphs {
        tokens.push(QuestSeqExtractedToken {
            index,
            offset,
            raw: vec![0x82, 0xA0],
            kind: QuestSeqExtractedTokenKind::Glyph,
            source: Some("あ".to_string()),
            control_code: None,
            control_meaning: None,
        });
        offset += 2;
    }
    let terminal_len = quest_control_spec(0x03)
        .unwrap()
        .layout
        .encoded_len(u32::try_from(offset).unwrap())
        .unwrap();
    tokens.push(QuestSeqExtractedToken {
        index: source_glyphs,
        offset,
        raw: {
            let mut raw = vec![0x03, 0xFF, 0, 0, 0, 0];
            raw.resize(terminal_len, 0);
            raw
        },
        kind: QuestSeqExtractedTokenKind::Control,
        source: None,
        control_code: Some(0x03),
        control_meaning: Some(QuestControlMeaning::DeactivateInterpreterAndAlign),
    });
    QuestSeqExtractedTextStream {
        sequence: "TEST.SEQ".to_string(),
        sequence_sha256: [1; 32],
        record_offset: 0,
        record_raw: [0x21, 0, 0, 0],
        stream_offset: 4,
        end_offset: offset + terminal_len,
        terminal: QuestTextTerminal::DeactivateAndAlign,
        expected_raw_sha256: [2; 32],
        tokens,
    }
}

#[test]
fn op21_route는_opcode를_보존하고_page와_target_word를_분리한다() {
    let stream = ff03_stream(3);
    assert_eq!(routed_record(&stream, 7, None), Ok([0x21, 7, 0, 0]));
    assert_eq!(
        routed_record(&stream, 7, Some(0x1234 * 4)),
        Ok([0x21, 7, 0x34, 0x12])
    );
    assert!(routed_record(&stream, 0, None).is_err());
    assert!(routed_record(&stream, 1, Some(2)).is_err());
    assert!(routed_record(&stream, 1, Some((usize::from(u16::MAX) + 1) * 4)).is_err());
}

#[test]
fn placeholder가_남은_draft는_status와_별개로_사람_결정을_요구한다() {
    assert!(draft_needs_human_decision(
        QuestTranslationWorkStatus::DraftNeedsIndependentReview,
        "{용어}를 받았다"
    ));
    assert!(draft_needs_human_decision(
        QuestTranslationWorkStatus::NeedsHumanTermDecision,
        "후보"
    ));
    assert!(!draft_needs_human_decision(
        QuestTranslationWorkStatus::DraftNeedsIndependentReview,
        "결정된 초안"
    ));
}

#[test]
fn projected_stream은_글리프수와_ff03정렬을_실제_byte로_센다() {
    let stream = ff03_stream(3);
    assert_eq!(projected_stream_bytes(&stream, 0, 3), Ok(stream.end_offset));
    assert_eq!(
        projected_stream_body_bytes(&stream, 4, 3),
        Ok(stream.end_offset - QUEST_SEQ_TEXT_MODE_RECORD_BYTES)
    );
    assert_eq!(
        projected_stream_bytes(&stream, 0, 4),
        Ok(stream.end_offset + 4)
    );
    assert!(projected_stream_bytes(&stream, 2, 3).is_err());
    assert!(projected_stream_body_bytes(&stream, 2, 3).is_err());
}

#[test]
fn 네바이트정렬은_이미정렬된값과_overflow를_구분한다() {
    assert_eq!(align4(0), Ok(0));
    assert_eq!(align4(1), Ok(4));
    assert_eq!(align4(7), Ok(8));
    assert!(align4(usize::MAX).is_err());
}
