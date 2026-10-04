use super::*;
use crate::analysis::audit_quest_seq_text_paths;
use crate::font::{QuestCodeTable, QUEST_CODE_TABLE_COUNT};

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

fn pointer_record(target_offset: u32) -> Vec<u8> {
    let mut raw = vec![0xA1, 0, 0, 0, 0, 0, 0, 0];
    raw.extend_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + target_offset).to_le_bytes());
    raw
}

#[test]
fn op0d_frontier는_전역_word_probe주소를_계산한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x0D, 0x40, 0x04, 0x00]);
    let paths = audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let analysis = audit_quest_seq_text_frontiers([("TEST.SEQ", raw.as_slice())], &paths).unwrap();

    assert_eq!(analysis.frontiers.len(), 1);
    let frontier = &analysis.frontiers[0];
    assert_eq!(frontier.terminal_offset, 12);
    assert_eq!(
        frontier.requirement,
        QuestSeqTextFrontierRequirement::GlobalStateWord
    );
    assert_eq!(
        frontier.parameter,
        Some(QuestSeqTextFrontierParameter::GlobalStateWordIndex { index: 0x40 })
    );
    assert_eq!(frontier.runtime_probe_address, Some(0x800D_B7C0));
    assert_eq!(frontier.runtime_probe_length, Some(4));
}

#[test]
fn op8a_frontier는_resource_id를_보존한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x8A, 0, 0x34, 0x12]);
    let paths = audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let analysis = audit_quest_seq_text_frontiers([("TEST.SEQ", raw.as_slice())], &paths).unwrap();

    assert_eq!(
        analysis.frontiers[0].parameter,
        Some(QuestSeqTextFrontierParameter::ResourceId { id: 0x1234 })
    );
    assert_eq!(analysis.frontiers[0].runtime_probe_address, None);
}

#[test]
fn op13은_object_halfword와_두_next를_보존한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x13, 0x80, 0xC2, 0, 0, 0, 0, 0]);
    raw.extend_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 0x20).to_le_bytes());
    raw.extend_from_slice(&[0; 16]);
    let paths = audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let analysis = audit_quest_seq_text_frontiers([("TEST.SEQ", raw.as_slice())], &paths).unwrap();

    let frontier = &analysis.frontiers[0];
    assert_eq!(
        frontier.requirement,
        QuestSeqTextFrontierRequirement::CompareObjectState
    );
    let compare = frontier.compare.expect("OP13 비교 record");
    assert_eq!(compare.operation, QuestSeqBaseCompareOperation::NotEqual);
    assert_eq!(compare.value_bytes, 2);
    assert_eq!(
        compare.left_source,
        QuestSeqBaseCompareOperandSource::ObjectField
    );
    assert_eq!(compare.left_parameter, Some(0xC2));
    assert_eq!(
        compare.right_source,
        QuestSeqBaseCompareOperandSource::Inline
    );
    assert_eq!(compare.right_parameter, 0);
    assert_eq!(compare.branch_target_offset, Some(0x20));
    assert_eq!(compare.fallthrough_offset, 0x18);
}

#[test]
fn op14는_handler_argument와_inline_word를_구분한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x14, 0x00, 0x20, 0, 0x48, 0, 0, 0]);
    raw.extend_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 0x20).to_le_bytes());
    raw.extend_from_slice(&[0; 16]);
    let paths = audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let analysis = audit_quest_seq_text_frontiers([("TEST.SEQ", raw.as_slice())], &paths).unwrap();

    let frontier = &analysis.frontiers[0];
    assert_eq!(
        frontier.requirement,
        QuestSeqTextFrontierRequirement::CompareHandlerArgumentState
    );
    let compare = frontier.compare.expect("OP14 비교 record");
    assert_eq!(
        compare.left_source,
        QuestSeqBaseCompareOperandSource::HandlerArgumentA2
    );
    assert_eq!(compare.left_parameter, None);
    assert_eq!(compare.right_parameter, 0x48);
    assert_eq!(compare.branch_target_offset, Some(0x20));
    assert_eq!(compare.fallthrough_offset, 0x18);
}

#[test]
fn 도달한_op21은_frontier에서_제외한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[
        0x21, 0, 0, 0, 0x82, 0xA0, 0x82, 0xA0, 0x82, 0xA0, 0x03, 0xFF, 0, 0, 0, 0,
    ]);
    let paths = audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let analysis = audit_quest_seq_text_frontiers([("TEST.SEQ", raw.as_slice())], &paths).unwrap();
    assert!(analysis.frontiers.is_empty());
    assert_eq!(analysis.path_occurrences, 0);
}

#[test]
fn op00_release는_runtime_frontier에서_제외한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x00, 0, 0, 0]);
    raw.extend_from_slice(&[
        0x21, 0, 0, 0, 0x82, 0xA0, 0x82, 0xA0, 0x82, 0xA0, 0x03, 0xFF, 0, 0, 0, 0,
    ]);
    let paths = audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    assert_eq!(
        paths.paths[0].status,
        QuestSeqTextPathStatus::ReleasedInterpreter
    );

    let analysis = audit_quest_seq_text_frontiers([("TEST.SEQ", raw.as_slice())], &paths).unwrap();
    assert!(analysis.frontiers.is_empty());
    assert_eq!(analysis.path_occurrences, 0);
}

#[test]
fn 입력_순서와_무관하게_report가_결정적이다() {
    let mut a = pointer_record(12);
    a.extend_from_slice(&[0x0D, 1, 0, 0]);
    let mut b = pointer_record(12);
    b.extend_from_slice(&[0x8A, 0, 2, 0]);
    let table = code_table();
    let left_paths =
        audit_quest_seq_text_paths([("B.SEQ", b.as_slice()), ("A.SEQ", a.as_slice())], &table)
            .unwrap();
    let right_paths =
        audit_quest_seq_text_paths([("A.SEQ", a.as_slice()), ("B.SEQ", b.as_slice())], &table)
            .unwrap();
    let left = audit_quest_seq_text_frontiers(
        [("B.SEQ", b.as_slice()), ("A.SEQ", a.as_slice())],
        &left_paths,
    )
    .unwrap();
    let right = audit_quest_seq_text_frontiers(
        [("A.SEQ", a.as_slice()), ("B.SEQ", b.as_slice())],
        &right_paths,
    )
    .unwrap();
    assert_eq!(
        render_quest_seq_text_frontiers_tsv(&left),
        render_quest_seq_text_frontiers_tsv(&right)
    );
}

#[test]
fn 빈입력_중복과_path원본누락을_거부한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x0D, 1, 0, 0]);
    let paths = audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();

    assert!(audit_quest_seq_text_frontiers([], &paths).is_err());
    assert!(audit_quest_seq_text_frontiers(
        [("TEST.SEQ", raw.as_slice()), ("TEST.SEQ", raw.as_slice())],
        &paths,
    )
    .is_err());
    assert!(audit_quest_seq_text_frontiers([("OTHER.SEQ", raw.as_slice())], &paths).is_err());
}
