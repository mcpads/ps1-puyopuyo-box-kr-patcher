use super::*;
use crate::script::{QuestSeqExtractedToken, QuestTextTerminal};

fn glyph(index: usize, source: &str) -> QuestSeqExtractedToken {
    QuestSeqExtractedToken {
        index,
        offset: index * 2,
        raw: vec![0x82, 0xA0],
        kind: QuestSeqExtractedTokenKind::Glyph,
        source: Some(source.to_owned()),
        control_code: None,
        control_meaning: None,
    }
}

fn control(
    index: usize,
    code: u8,
    raw: &[u8],
    meaning: QuestControlMeaning,
) -> QuestSeqExtractedToken {
    QuestSeqExtractedToken {
        index,
        offset: index * 2,
        raw: raw.to_vec(),
        kind: QuestSeqExtractedTokenKind::Control,
        source: None,
        control_code: Some(code),
        control_meaning: Some(meaning),
    }
}

fn stream(tokens: Vec<QuestSeqExtractedToken>) -> QuestSeqExtractedTextStream {
    QuestSeqExtractedTextStream {
        sequence: "TEST.SEQ".to_owned(),
        sequence_sha256: [0; 32],
        record_offset: 0x20,
        record_raw: [0x21, 0, 0, 0],
        stream_offset: 0x24,
        end_offset: 0x80,
        terminal: QuestTextTerminal::DeactivateAndAlign,
        expected_raw_sha256: [0xAB; 32],
        tokens,
    }
}

#[test]
fn 원문_control순서와_확정가능한_window_line만_기록한다() {
    let source = stream(vec![
        glyph(0, "앞"),
        control(
            1,
            0x06,
            &[0x06, 0xFF, 2, 0],
            QuestControlMeaning::SelectTextWindow,
        ),
        control(2, 0x15, &[0x15, 0xFF], QuestControlMeaning::OpenTextWindow),
        glyph(3, "가"),
        glyph(4, "나"),
        control(5, 0x04, &[0x04, 0xFF], QuestControlMeaning::AdvanceTextLine),
        glyph(6, "다"),
        control(
            7,
            0x0F,
            &[0x0F, 0xFF, 1, 0],
            QuestControlMeaning::StartInputWait,
        ),
        glyph(8, "라"),
        control(
            9,
            0x03,
            &[0x03, 0xFF, 0, 0, 0, 0],
            QuestControlMeaning::DeactivateInterpreterAndAlign,
        ),
    ]);

    let layout = build_quest_source_layout(&source).expect("원문 표시 구조");

    assert_eq!(layout.glyphs, 5);
    assert_eq!(layout.controls, 5);
    assert_eq!(layout.spans.len(), 5);
    assert_eq!(layout.spans[0].protected_source, "앞");
    assert_eq!(layout.spans[0].window_index, None);
    assert_eq!(layout.spans[2].protected_source, "가나");
    assert_eq!(layout.spans[2].window_index, Some(2));
    assert_eq!(layout.spans[2].line_index, Some(0));
    assert_eq!(layout.spans[3].protected_source, "다");
    assert_eq!(layout.spans[3].line_index, Some(1));
    assert_eq!(layout.spans[4].protected_source, "라");
    assert_eq!(layout.spans[4].input_phase, 1);
    assert_eq!(layout.unresolved_window_spans, 1);
    assert_eq!(layout.unresolved_line_spans, 1);
}

#[test]
fn 잘못된_ff06과_비canonical종단은_부분구조를_반환하지_않는다() {
    let invalid_select = stream(vec![
        control(
            0,
            0x06,
            &[0x06, 0xFF, 2, 1],
            QuestControlMeaning::SelectTextWindow,
        ),
        control(
            1,
            0x03,
            &[0x03, 0xFF, 0, 0, 0, 0],
            QuestControlMeaning::DeactivateInterpreterAndAlign,
        ),
    ]);
    assert!(build_quest_source_layout(&invalid_select).is_err());

    let wrong_terminal = stream(vec![control(
        0,
        0x05,
        &[0x05, 0xFF],
        QuestControlMeaning::CloseTextWindow,
    )]);
    assert!(build_quest_source_layout(&wrong_terminal).is_err());
}
