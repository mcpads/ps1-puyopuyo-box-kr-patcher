use super::*;
use crate::font::QUEST_CODE_TABLE_COUNT;

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
fn op21부터_ff03까지_token과_일본어_run을_고정한다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let raw = [
        0x21, 0, 0, 0, 0x82, 0xA0, 0x82, 0xA0, 0x82, 0xA0, 0x03, 0xFF, 0, 0, 0, 0,
    ];
    let analysis =
        audit_quest_seq_text_streams([("TEST.SEQ", raw.as_slice())], &code_table).unwrap();

    assert_eq!(analysis.markers.len(), 1);
    assert_eq!(analysis.rooted_runs.len(), 1);
    let marker = &analysis.markers[0];
    assert_eq!(marker.record_raw, [0x21, 0, 0, 0]);
    assert_eq!(marker.stream_offset, 4);
    assert_eq!(marker.end_offset, Some(16));
    assert_eq!(marker.terminal, Some(QuestTextTerminal::DeactivateAndAlign));
    assert_eq!(
        (marker.token_count, marker.glyph_count, marker.control_count),
        (4, 3, 1)
    );
    let run = &analysis.rooted_runs[0];
    assert_eq!(run.offset, 4);
    assert_eq!(run.source, "あああ");
    assert_eq!(run.owner_record_offsets, [0]);
}

#[test]
fn nonzero_미소비_byte를_실행_record로_승격하지_않는다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let raw = [
        0x21, 0x21, 0x81, 0x40, 0x82, 0xA0, 0x82, 0xA0, 0x82, 0xA0, 0x03, 0xFF, 0, 0, 0, 0,
    ];
    let analysis =
        audit_quest_seq_text_streams([("TEST.SEQ", raw.as_slice())], &code_table).unwrap();

    assert!(!analysis.markers[0].is_parseable());
    assert!(analysis.markers[0]
        .error
        .as_deref()
        .unwrap()
        .contains("canonical zero"));
    assert!(analysis.rooted_runs.is_empty());
}

#[test]
fn 코드표에_없는_glyph가_나오면_op21_shape를_기각한다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let raw = [0x21, 0, 0, 0, 0x82, 0xA2, 0x03, 0xFF, 0, 0, 0, 0];
    let analysis =
        audit_quest_seq_text_streams([("TEST.SEQ", raw.as_slice())], &code_table).unwrap();

    assert_eq!(analysis.markers.len(), 1);
    assert!(!analysis.markers[0].is_parseable());
    assert!(analysis.markers[0]
        .error
        .as_deref()
        .unwrap()
        .contains("코드표에 없음"));
    assert!(analysis.rooted_runs.is_empty());
}

#[test]
fn lifecycle_종단이_없으면_부분_stream을_반환하지_않는다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let raw = [0x21, 0, 0, 0, 0x82, 0xA0, 0x82, 0xA0, 0x82, 0xA0];
    let analysis =
        audit_quest_seq_text_streams([("TEST.SEQ", raw.as_slice())], &code_table).unwrap();

    assert_eq!(analysis.markers.len(), 1);
    assert!(!analysis.markers[0].is_parseable());
    assert!(analysis.markers[0]
        .error
        .as_deref()
        .unwrap()
        .contains("lifecycle 종단"));
    assert!(analysis.rooted_runs.is_empty());
}

#[test]
fn 입력_순서와_무관하게_report가_결정적이다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let a = [0x21, 0, 0, 0, 0x00, 0xFF, 0, 0, 0, 0];
    let b = [
        0x21, 0, 0, 0, 0x82, 0xA0, 0x82, 0xA0, 0x82, 0xA0, 0x1A, 0xFF, 0, 0, 0, 0,
    ];
    let left = audit_quest_seq_text_streams(
        [("B.SEQ", b.as_slice()), ("A.SEQ", a.as_slice())],
        &code_table,
    )
    .unwrap();
    let right = audit_quest_seq_text_streams(
        [("A.SEQ", a.as_slice()), ("B.SEQ", b.as_slice())],
        &code_table,
    )
    .unwrap();

    assert_eq!(
        render_quest_seq_text_streams_tsv(&left),
        render_quest_seq_text_streams_tsv(&right)
    );
    assert_eq!(
        render_quest_seq_rooted_text_runs_tsv(&left),
        render_quest_seq_rooted_text_runs_tsv(&right)
    );
}

#[test]
fn 중복_입력_이름은_거부한다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let raw = [0u8; 4];
    let result = audit_quest_seq_text_streams(
        [("TEST.SEQ", raw.as_slice()), ("TEST.SEQ", raw.as_slice())],
        &code_table,
    );
    assert!(result.unwrap_err().to_string().contains("입력 중복"));
}
