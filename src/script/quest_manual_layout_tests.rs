use super::*;
use crate::script::{QuestControlMeaning, QuestSeqExtractedToken, QuestTextTerminal};

fn stream() -> QuestSeqExtractedTextStream {
    let tokens = vec![
        QuestSeqExtractedToken {
            index: 0,
            offset: 4,
            raw: vec![0x82, 0xA0],
            kind: QuestSeqExtractedTokenKind::Glyph,
            source: Some("あ".to_owned()),
            control_code: None,
            control_meaning: None,
        },
        QuestSeqExtractedToken {
            index: 1,
            offset: 6,
            raw: vec![0x82, 0xA2],
            kind: QuestSeqExtractedTokenKind::Glyph,
            source: Some("い".to_owned()),
            control_code: None,
            control_meaning: None,
        },
        QuestSeqExtractedToken {
            index: 2,
            offset: 8,
            raw: vec![0x04, 0xFF],
            kind: QuestSeqExtractedTokenKind::Control,
            source: None,
            control_code: Some(0x04),
            control_meaning: Some(QuestControlMeaning::AdvanceTextLine),
        },
        QuestSeqExtractedToken {
            index: 3,
            offset: 10,
            raw: vec![0x82, 0xA4],
            kind: QuestSeqExtractedTokenKind::Glyph,
            source: Some("う".to_owned()),
            control_code: None,
            control_meaning: None,
        },
        QuestSeqExtractedToken {
            index: 4,
            offset: 12,
            raw: vec![0x82, 0xA6],
            kind: QuestSeqExtractedTokenKind::Glyph,
            source: Some("え".to_owned()),
            control_code: None,
            control_meaning: None,
        },
        QuestSeqExtractedToken {
            index: 5,
            offset: 14,
            raw: vec![0x03, 0xFF, 0, 0, 0, 0],
            kind: QuestSeqExtractedTokenKind::Control,
            source: None,
            control_code: Some(0x03),
            control_meaning: Some(QuestControlMeaning::DeactivateInterpreterAndAlign),
        },
    ];
    QuestSeqExtractedTextStream {
        sequence: "TEST.SEQ".to_owned(),
        sequence_sha256: [0; 32],
        record_offset: 0,
        record_raw: [0x21, 0, 0, 0],
        stream_offset: 4,
        end_offset: 20,
        terminal: QuestTextTerminal::DeactivateAndAlign,
        expected_raw_sha256: [0; 32],
        tokens,
    }
}

#[test]
fn 명시적layout은_사람이_지정한run을_그대로_검증한다() {
    let plan = validate_quest_explicit_layout(&stream(), &[2, 1]).unwrap();

    assert_eq!(plan.candidate_glyph_run_lengths, [2, 1]);
    assert!(validate_quest_explicit_layout(&stream(), &[3]).is_err());
    assert!(validate_quest_explicit_layout(&stream(), &[0, 0]).is_err());
}
