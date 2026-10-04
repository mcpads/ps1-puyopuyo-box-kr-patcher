use super::*;
use crate::font::{
    QUEST_CODE_TABLE_COUNT, QUEST_DOUBLE_EXCLAMATION_CODE, QUEST_DOUBLE_QUESTION_CODE,
};

fn code_table_with(codes: &[[u8; 2]]) -> QuestCodeTable {
    let mut table = (0..QUEST_CODE_TABLE_COUNT)
        .map(|index| {
            let value = u16::try_from(index).expect("test index");
            value.to_le_bytes()
        })
        .collect::<Vec<_>>();
    for (index, code) in codes.iter().copied().enumerate() {
        table[index] = code;
    }
    let raw = table.into_iter().flatten().collect::<Vec<_>>();
    QuestCodeTable::parse(&raw).expect("test code table")
}

#[test]
fn op21_record와_glyph_control을_원본_동일하게_왕복한다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let raw = [
        0x21, 0, 0, 0, 0x82, 0xA0, 0x82, 0xA0, 0x04, 0xFF, 0x82, 0xA0, 0x03, 0xFF, 0, 0, 0, 0, 0, 0,
    ];
    let analysis =
        audit_quest_seq_text_extraction([("TEST.SEQ", raw.as_slice())], &code_table).unwrap();
    assert_eq!(analysis.streams.len(), 1);
    assert_eq!(
        (
            analysis.token_occurrences,
            analysis.glyph_occurrences,
            analysis.control_occurrences,
            analysis.roundtrip_bytes,
        ),
        (5, 3, 2, raw.len())
    );
    let stream = &analysis.streams[0];
    assert_eq!(stream.tokens[0].source.as_deref(), Some("あ"));
    assert_eq!(
        stream.tokens[2].control_meaning,
        Some(QuestControlMeaning::AdvanceTextLine)
    );
    assert_eq!(
        serialize_quest_seq_extracted_text_stream(stream),
        Ok(raw.to_vec())
    );
}

#[test]
fn 전용_두_문자_cell은_단일_의미문자로_추출하고_raw를_왕복한다() {
    let code_table = code_table_with(&[
        [0x82, 0xA0],
        QUEST_DOUBLE_EXCLAMATION_CODE,
        QUEST_DOUBLE_QUESTION_CODE,
    ]);
    let raw = [
        0x21, 0, 0, 0, 0x82, 0xA0, 0x21, 0x21, 0x3F, 0x3F, 0x03, 0xFF, 0, 0, 0, 0,
    ];
    let analysis =
        audit_quest_seq_text_extraction([("TEST.SEQ", raw.as_slice())], &code_table).unwrap();
    let stream = &analysis.streams[0];

    assert_eq!(stream.tokens[1].source.as_deref(), Some("‼"));
    assert_eq!(stream.tokens[2].source.as_deref(), Some("⁇"));
    assert_eq!(stream.tokens[1].raw, QUEST_DOUBLE_EXCLAMATION_CODE);
    assert_eq!(stream.tokens[2].raw, QUEST_DOUBLE_QUESTION_CODE);
    assert_eq!(
        serialize_quest_seq_extracted_text_stream(stream),
        Ok(raw.to_vec())
    );
}

#[test]
fn 보호_raw나_offset이_바뀐_stream은_부분_출력하지_않는다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let raw = [0x21, 0, 0, 0, 0x82, 0xA0, 0x03, 0xFF, 0, 0, 0, 0];
    let analysis =
        audit_quest_seq_text_extraction([("TEST.SEQ", raw.as_slice())], &code_table).unwrap();

    let mut changed_raw = analysis.streams[0].clone();
    changed_raw.tokens[0].raw[0] ^= 1;
    assert!(serialize_quest_seq_extracted_text_stream(&changed_raw).is_err());

    let mut changed_offset = analysis.streams[0].clone();
    changed_offset.tokens[1].offset += 2;
    assert!(serialize_quest_seq_extracted_text_stream(&changed_offset).is_err());

    let mut reversed_range = analysis.streams[0].clone();
    reversed_range.record_offset = 1;
    reversed_range.stream_offset = 5;
    reversed_range.end_offset = 0;
    assert!(serialize_quest_seq_extracted_text_stream(&reversed_range).is_err());
}

#[test]
fn 입력_순서와_무관하게_token_ledger가_결정적이다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let a = [0x21, 0, 0, 0, 0x00, 0xFF, 0, 0, 0, 0];
    let b = [0x21, 0, 0, 0, 0x82, 0xA0, 0x1A, 0xFF, 0, 0, 0, 0];
    let left = audit_quest_seq_text_extraction(
        [("B.SEQ", b.as_slice()), ("A.SEQ", a.as_slice())],
        &code_table,
    )
    .unwrap();
    let right = audit_quest_seq_text_extraction(
        [("A.SEQ", a.as_slice()), ("B.SEQ", b.as_slice())],
        &code_table,
    )
    .unwrap();
    assert_eq!(
        render_quest_seq_text_token_ledger_tsv(&left),
        render_quest_seq_text_token_ledger_tsv(&right)
    );
}

#[test]
fn 중복_입력_이름은_거부한다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let raw = [0u8; 4];
    let result = audit_quest_seq_text_extraction(
        [("TEST.SEQ", raw.as_slice()), ("TEST.SEQ", raw.as_slice())],
        &code_table,
    );
    assert!(result.unwrap_err().to_string().contains("입력 중복"));
}
