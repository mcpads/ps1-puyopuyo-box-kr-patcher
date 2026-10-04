use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::font::QUEST_CODE_TABLE_COUNT;
use crate::pipeline::quest_glyph_cache_plan::render_quest_glyph_cache_repertoire_cells;
use crate::script::{
    QuestControlMeaning, QuestSeqExtractedTextStream, QuestSeqExtractedToken,
    QuestSeqExtractedTokenKind, QuestTextTerminal,
};

fn stream_with_line_control() -> QuestSeqExtractedTextStream {
    let tokens = vec![
        QuestSeqExtractedToken {
            index: 0,
            offset: 4,
            raw: vec![0x82, 0xA0],
            kind: QuestSeqExtractedTokenKind::Glyph,
            source: Some("あ".to_string()),
            control_code: None,
            control_meaning: None,
        },
        QuestSeqExtractedToken {
            index: 1,
            offset: 6,
            raw: vec![0x82, 0xA2],
            kind: QuestSeqExtractedTokenKind::Glyph,
            source: Some("い".to_string()),
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
            source: Some("う".to_string()),
            control_code: None,
            control_meaning: None,
        },
        QuestSeqExtractedToken {
            index: 4,
            offset: 12,
            raw: vec![0x82, 0xA6],
            kind: QuestSeqExtractedTokenKind::Glyph,
            source: Some("え".to_string()),
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
        sequence: "TEST.SEQ".to_string(),
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
fn 명시적layout은_사람이_쓴_run경계를_그대로_직렬화한다() {
    let body = serialize_explicit_body(
        &stream_with_line_control(),
        4,
        &[[0x82, 0xA0], [0x82, 0xA2], [0x82, 0xA4]],
        &[2, 1],
    )
    .unwrap();
    assert_eq!(&body[..6], &[0x82, 0xA0, 0x82, 0xA2, 0x04, 0xFF]);
    assert_eq!(&body[6..8], &[0x82, 0xA4]);
}

#[test]
fn 명시적layout의_glyph합계가_선택문과_다르면_실패한다() {
    assert!(serialize_explicit_body(
        &stream_with_line_control(),
        4,
        &[[0x82, 0xA0], [0x82, 0xA2], [0x82, 0xA4]],
        &[1, 1],
    )
    .is_err());
}

#[test]
fn tail복귀record는_같은창의_원본종단을_가리킨다() {
    assert_eq!(
        return_record(0x8019_BA90).unwrap(),
        [0x21, 0xFF, 0x08, 0x08]
    );
}

#[test]
fn expected_write는_원본불일치와_겹침에서_부분쓰기없이_실패한다() {
    let mut output = vec![1, 2, 3, 4];
    let mut claimed = vec![false; 4];
    assert!(write_exact(&mut output, &mut claimed, 1, &[9], &[7], "test").is_err());
    assert_eq!(output, [1, 2, 3, 4]);
    write_exact(&mut output, &mut claimed, 1, &[2], &[7], "test").unwrap();
    assert!(write_exact(&mut output, &mut claimed, 1, &[7], &[8], "test").is_err());
    assert_eq!(output, [1, 7, 3, 4]);
}

#[test]
fn 원문run에_드는_수동본문은_control_offset과_전체span을_보존한다() {
    let stream = stream_with_line_control();
    let body = serialize_explicit_inline_body_preserving_source_offsets(
        &stream,
        stream.stream_offset,
        &[[0x82, 0xA0], [0x82, 0xA4], [0x82, 0xA6]],
        &[1, 2],
        [0x81, 0x40],
    )
    .unwrap();

    assert_eq!(body.len(), stream.end_offset - stream.stream_offset);
    assert_eq!(&body[2..4], &[0x81, 0x40]);
    assert_eq!(&body[4..6], &[0x04, 0xFF]);
    assert_eq!(&body[10..], &[0x03, 0xFF, 0, 0, 0, 0]);
}

#[test]
fn 전체glyph수가_맞아도_한_run이_원문을_넘으면_inline을_거부한다() {
    let stream = stream_with_line_control();
    assert!(serialize_explicit_inline_body_preserving_source_offsets(
        &stream,
        stream.stream_offset,
        &[[0x82, 0xA0], [0x82, 0xA2], [0x82, 0xA4], [0x82, 0xA6]],
        &[3, 1],
        [0x81, 0x40],
    )
    .is_err());
}

#[test]
fn 같은분모의_다른unicode_repertoire는_package_raster에_결속되지않는다() {
    let original = ['가', '나'].into_iter().collect::<BTreeSet<_>>();
    let actual_cells = render_quest_glyph_cache_repertoire_cells(&original).unwrap();

    verify_quest_repertoire_cells("가나", &actual_cells).unwrap();
    let error = verify_quest_repertoire_cells("가다", &actual_cells).unwrap_err();
    assert!(error.to_string().contains("package raster 불일치"));
}

#[test]
fn 독립bank의_비공백glyph는_source_code로_fallback하지않는다() {
    let mut codes = (0..QUEST_CODE_TABLE_COUNT)
        .map(|index| u16::try_from(index).unwrap().to_le_bytes())
        .collect::<Vec<_>>();
    codes[0] = [0x81, 0x40];
    codes[1] = [0x81, 0x75];
    let raw = codes.into_iter().flatten().collect::<Vec<_>>();
    let code_table = QuestCodeTable::parse(&raw).unwrap();
    let page_codes = BTreeMap::from([(1, BTreeMap::new())]);

    let error = encode_quest_page_text("「", 1, &page_codes, &code_table).unwrap_err();
    assert!(error.to_string().contains("독립 bank 공급 누락"));
    assert_eq!(
        encode_quest_page_text("　", 1, &page_codes, &code_table).unwrap(),
        vec![[0x81, 0x40]]
    );
}

#[test]
fn source_atlas의_빈cell은_전각공백에만_허용한다() {
    verify_source_atlas_cell('\u{3000}', &[0; 8]).unwrap();
    assert!(verify_source_atlas_cell('「', &[0; 8]).is_err());
    assert!(verify_source_atlas_cell('\u{3000}', &[0, 1]).is_err());
    verify_source_atlas_cell('「', &[0, 1]).unwrap();
}
