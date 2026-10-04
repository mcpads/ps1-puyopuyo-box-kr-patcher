use super::*;
use crate::font::QUEST_CODE_TABLE_COUNT;
use crate::script::{audit_quest_seq_addresses, audit_quest_seq_text_streams};

fn code_table() -> QuestCodeTable {
    let mut table = (0..QUEST_CODE_TABLE_COUNT)
        .map(|index| {
            let value = u16::try_from(index).expect("test index");
            value.to_le_bytes()
        })
        .collect::<Vec<_>>();
    table[0] = [0x82, 0xA0];
    QuestCodeTable::parse(&table.into_iter().flatten().collect::<Vec<_>>())
        .expect("test code table")
}

fn text_tail() -> [u8; 16] {
    [
        0x21, 0, 0, 0, 0x82, 0xA0, 0x82, 0xA0, 0x82, 0xA0, 0x03, 0xFF, 0, 0, 0, 0,
    ]
}

fn cross(raw: &[u8]) -> QuestSeqTextRootAnalysis {
    let table = code_table();
    let text = audit_quest_seq_text_streams([("TEST.SEQ", raw)], &table).expect("text");
    let addresses = audit_quest_seq_addresses([("TEST.SEQ", raw)]).expect("addresses");
    audit_quest_seq_text_roots(&text, &addresses).expect("cross")
}

#[test]
fn opa1_pointer가_op21_record를_가리키면_consumer_edge로_고정한다() {
    let mut raw = vec![0xA1, 0, 0, 0, 0, 0, 0, 0];
    raw.extend_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 12).to_le_bytes());
    raw.extend_from_slice(&text_tail());

    let analysis = cross(&raw);
    assert_eq!(analysis.markers.len(), 1);
    let marker = &analysis.markers[0];
    assert_eq!(
        marker.evidence,
        QuestSeqTextRootEvidence::ConsumerBackedPointerShape
    );
    assert_eq!(marker.consumer_edges.len(), 1);
    assert_eq!(marker.consumer_edges[0].opcode, 0xA1);
    assert_eq!(marker.consumer_edges[0].source_record_offset, 0);
    assert_eq!(marker.consumer_edges[0].pointer_field_offset, 8);
    assert_eq!(analysis.rooted_runs, 1);
}

#[test]
fn 일반_범위_u32는_consumer_root로_승격하지_않는다() {
    let mut raw = (TOWN00_SEQ_RUNTIME_BASE + 8).to_le_bytes().to_vec();
    raw.extend_from_slice(&[0, 0, 0, 0]);
    raw.extend_from_slice(&text_tail());

    let analysis = cross(&raw);
    let marker = &analysis.markers[0];
    assert_eq!(
        marker.evidence,
        QuestSeqTextRootEvidence::SameBaseRangeWordOnly
    );
    assert!(marker.consumer_edges.is_empty());
    assert_eq!(marker.range_word_fields, [0]);
    assert_eq!(analysis.rooted_runs, 0);
}

#[test]
fn incoming_word가_없으면_명시적으로_보존한다() {
    let analysis = cross(&text_tail());
    let marker = &analysis.markers[0];
    assert_eq!(
        marker.evidence,
        QuestSeqTextRootEvidence::NoDirectIncomingWord
    );
    assert_eq!(marker.direct_word_count(), 0);
}

#[test]
fn 파일_집합이_다른_분석은_교차하지_않는다() {
    let table = code_table();
    let raw = text_tail();
    let text = audit_quest_seq_text_streams([("TEXT.SEQ", raw.as_slice())], &table).unwrap();
    let addresses = audit_quest_seq_addresses([("OTHER.SEQ", raw.as_slice())]).unwrap();
    let result = audit_quest_seq_text_roots(&text, &addresses);
    assert!(result.unwrap_err().to_string().contains("파일 이름·길이"));
}

#[test]
fn 입력_순서와_무관하게_report가_결정적이다() {
    let table = code_table();
    let a = text_tail();
    let mut b = vec![0xA1, 0, 0, 0, 0, 0, 0, 0];
    b.extend_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 12).to_le_bytes());
    b.extend_from_slice(&text_tail());

    let left_text =
        audit_quest_seq_text_streams([("B.SEQ", b.as_slice()), ("A.SEQ", a.as_slice())], &table)
            .unwrap();
    let left_addresses =
        audit_quest_seq_addresses([("B.SEQ", b.as_slice()), ("A.SEQ", a.as_slice())]).unwrap();
    let right_text =
        audit_quest_seq_text_streams([("A.SEQ", a.as_slice()), ("B.SEQ", b.as_slice())], &table)
            .unwrap();
    let right_addresses =
        audit_quest_seq_addresses([("A.SEQ", a.as_slice()), ("B.SEQ", b.as_slice())]).unwrap();
    let left = audit_quest_seq_text_roots(&left_text, &left_addresses).unwrap();
    let right = audit_quest_seq_text_roots(&right_text, &right_addresses).unwrap();

    assert_eq!(
        render_quest_seq_text_roots_tsv(&left),
        render_quest_seq_text_roots_tsv(&right)
    );
}
