use super::*;
use crate::font::QUEST_CODE_TABLE_COUNT;

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

fn pointer_record(target_offset: u32) -> Vec<u8> {
    let mut raw = vec![0xA1, 0, 0, 0, 0, 0, 0, 0];
    raw.extend_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + target_offset).to_le_bytes());
    raw
}

#[test]
fn 유일한_직접증가폭을_거쳐_op21에_도달한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x02, 0, 0, 0]);
    raw.extend_from_slice(&text_tail());

    let analysis =
        audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    assert_eq!(analysis.paths.len(), 1);
    let path = &analysis.paths[0];
    assert_eq!(path.status, QuestSeqTextPathStatus::ReachedSequentialOp21);
    assert_eq!(path.reached_op21_offset, Some(16));
    assert_eq!(
        path.nodes,
        [
            QuestSeqTextPathNode {
                offset: 12,
                opcode: 0x02,
                transition: QuestSeqTextPathTransition::DirectAdvance,
                next_offset: Some(16),
                child_stream_offset: None,
                direct_advance_bytes: Some(4),
            },
            QuestSeqTextPathNode {
                offset: 16,
                opcode: 0x21,
                transition: QuestSeqTextPathTransition::End,
                next_offset: None,
                child_stream_offset: None,
                direct_advance_bytes: None,
            },
        ]
    );
}

#[test]
fn op03_direct_branch_target을_따른다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x03, 0, 0, 0]);
    raw.extend_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 20).to_le_bytes());
    raw.extend_from_slice(&text_tail());

    let analysis =
        audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let path = &analysis.paths[0];
    assert_eq!(path.status, QuestSeqTextPathStatus::ReachedSequentialOp21);
    assert_eq!(path.reached_op21_offset, Some(20));
    assert_eq!(
        path.nodes,
        [
            QuestSeqTextPathNode {
                offset: 12,
                opcode: 0x03,
                transition: QuestSeqTextPathTransition::DirectBranch,
                next_offset: Some(20),
                child_stream_offset: None,
                direct_advance_bytes: None,
            },
            QuestSeqTextPathNode {
                offset: 20,
                opcode: 0x21,
                transition: QuestSeqTextPathTransition::End,
                next_offset: None,
                child_stream_offset: None,
                direct_advance_bytes: None,
            },
        ]
    );
    assert_eq!(analysis.summaries[0].direct_branch_markers, 1);
    assert_eq!(analysis.summaries[0].direct_branch_edge_occurrences, 1);
    assert_eq!(analysis.direct_branch_edge_occurrences, 1);
    assert_eq!(analysis.unique_direct_branch_edges, 1);
}

#[test]
fn op03의_파일밖_target은_중단으로_보존한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x03, 0, 0, 0]);
    raw.extend_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 0x1000).to_le_bytes());

    let analysis =
        audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    assert_eq!(
        analysis.paths[0].status,
        QuestSeqTextPathStatus::StoppedAtInvalidDirectBranchTarget
    );
}

#[test]
fn op03이_방문_node로_되돌아오면_cycle로_보존한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x03, 0, 0, 0]);
    raw.extend_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 12).to_le_bytes());

    let analysis =
        audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let path = &analysis.paths[0];
    assert_eq!(path.status, QuestSeqTextPathStatus::StoppedAtCycle);
    assert_eq!(path.nodes.len(), 1);
    assert_eq!(path.nodes[0].next_offset, Some(12));
}

#[test]
fn op00은_뒤의_op21을_읽지_않고_interpreter를_해제한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x00, 0, 0, 0]);
    raw.extend_from_slice(&text_tail());

    let analysis =
        audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let path = &analysis.paths[0];
    assert_eq!(path.status, QuestSeqTextPathStatus::ReleasedInterpreter);
    assert_eq!(path.reached_op21_offset, None);
    assert_eq!(path.nodes.len(), 1);
    assert_eq!(path.nodes[0].offset, 12);
    assert_eq!(
        path.nodes[0].transition,
        QuestSeqTextPathTransition::ReleaseInterpreter
    );
    assert_eq!(path.nodes[0].next_offset, None);
}

#[test]
fn op1c는_parent_next와_child_stream을_한_graph_edge로_보존한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x1C, 0, 0, 0]);
    raw.extend_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 36).to_le_bytes());
    raw.extend_from_slice(&text_tail());
    raw.extend_from_slice(&text_tail());

    let analysis =
        audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let path = &analysis.paths[0];
    assert_eq!(path.status, QuestSeqTextPathStatus::ReachedSequentialOp21);
    assert_eq!(path.reached_op21_offset, Some(20));
    assert_eq!(path.nodes[0].opcode, 0x1C);
    assert_eq!(
        path.nodes[0].transition,
        QuestSeqTextPathTransition::SpawnChildAndAdvance
    );
    assert_eq!(path.nodes[0].next_offset, Some(20));
    assert_eq!(path.nodes[0].child_stream_offset, Some(36));
    assert_eq!(path.nodes[0].direct_advance_bytes, Some(8));
    assert!(render_quest_seq_text_paths_tsv(&analysis).contains("0x0024"));
}

#[test]
fn op1c_child_stream이_파일_밖이면_실패한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x1C, 0, 0, 0]);
    raw.extend_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 0x1000).to_le_bytes());
    raw.extend_from_slice(&text_tail());

    assert!(audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).is_err());
}

#[test]
fn op82는_child_stream과_parent_next를_함께_보존한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x82, 2, 0, 0, 0, 0, 0, 0]);
    raw.extend_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 40).to_le_bytes());
    raw.extend_from_slice(&text_tail());
    raw.extend_from_slice(&text_tail());

    let analysis =
        audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let path = analysis
        .paths
        .iter()
        .find(|path| path.seed_offset == 12)
        .expect("OPA1이 만든 parent seed");
    assert_eq!(path.status, QuestSeqTextPathStatus::ReachedSequentialOp21);
    assert_eq!(path.reached_op21_offset, Some(24));
    assert_eq!(path.nodes[0].opcode, 0x82);
    assert_eq!(
        path.nodes[0].transition,
        QuestSeqTextPathTransition::SpawnChildAndAdvance
    );
    assert_eq!(path.nodes[0].next_offset, Some(24));
    assert_eq!(path.nodes[0].child_stream_offset, Some(40));
    assert_eq!(path.nodes[0].direct_advance_bytes, Some(12));
}

#[test]
fn op82의_nonlocal_child_pointer는_parent진행을_막지_않는다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x82, 2, 0, 0, 0, 0, 0, 0]);
    raw.extend_from_slice(&0x8000_1000u32.to_le_bytes());
    raw.extend_from_slice(&text_tail());

    let analysis =
        audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let path = analysis
        .paths
        .iter()
        .find(|path| path.seed_offset == 12)
        .expect("OPA1이 만든 parent seed");
    assert_eq!(path.status, QuestSeqTextPathStatus::ReachedSequentialOp21);
    assert_eq!(path.reached_op21_offset, Some(24));
    assert_eq!(path.nodes[0].next_offset, Some(24));
    assert_eq!(path.nodes[0].child_stream_offset, None);
}

#[test]
fn op28_sub00의_advance뒤_조건0은_옮겨진_stream을_따른다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x28, 0, 0, 0, 0, 0, 0, 0]);
    raw.extend_from_slice(&text_tail());

    let analysis =
        audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let path = &analysis.paths[0];
    assert_eq!(path.status, QuestSeqTextPathStatus::ReachedSequentialOp21);
    assert_eq!(
        path.nodes[0],
        QuestSeqTextPathNode {
            offset: 12,
            opcode: 0x28,
            transition: QuestSeqTextPathTransition::Op28AdvanceYield,
            next_offset: Some(20),
            child_stream_offset: None,
            direct_advance_bytes: Some(8),
        }
    );
}

#[test]
fn op28_sub00의_advance뒤_조건nonzero는_pointer_target을_따른다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x28, 0, 0, 0, 0, 0, 0, 0]);
    raw.extend_from_slice(&[0, 0, 0, 1]);
    raw.extend_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 28).to_le_bytes());
    raw.extend_from_slice(&text_tail());

    let analysis =
        audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let path = &analysis.paths[0];
    assert_eq!(path.status, QuestSeqTextPathStatus::ReachedSequentialOp21);
    assert_eq!(
        path.nodes[0],
        QuestSeqTextPathNode {
            offset: 12,
            opcode: 0x28,
            transition: QuestSeqTextPathTransition::Op28Branch,
            next_offset: Some(28),
            child_stream_offset: None,
            direct_advance_bytes: Some(8),
        }
    );
}

#[test]
fn op28의_미등록_subopcode는_중단으로_보존한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x28, 0x20, 0, 0]);

    let analysis =
        audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    assert_eq!(
        analysis.paths[0].status,
        QuestSeqTextPathStatus::StoppedAtInvalidOp28Flow
    );
}

#[test]
fn op21을_직접_가리키는_seed는_한_node로_보존한다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&text_tail());

    let analysis =
        audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let path = &analysis.paths[0];
    assert_eq!(path.status, QuestSeqTextPathStatus::ReachedDirectOp21);
    assert_eq!(path.nodes.len(), 1);
    assert_eq!(analysis.rooted_runs, 1);
}

#[test]
fn 미분류_복수_직접증가폭_opcode에서는_멈춘다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0x08, 0, 0, 0]);
    raw.extend_from_slice(&text_tail());

    let analysis =
        audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    let path = &analysis.paths[0];
    assert_eq!(
        path.status,
        QuestSeqTextPathStatus::StoppedWithoutUniqueDirectWidth
    );
    assert_eq!(path.nodes.len(), 1);
    assert_eq!(path.reached_op21_offset, None);
}

#[test]
fn op10_11은_record_mode로_4_8바이트를_고른다() {
    for opcode in [0x10, 0x11] {
        let mut wide = pointer_record(12);
        wide.extend_from_slice(&[opcode, 0, 0, 0, 0, 0, 0, 0]);
        wide.extend_from_slice(&text_tail());
        let analysis =
            audit_quest_seq_text_paths([("TEST.SEQ", wide.as_slice())], &code_table()).unwrap();
        assert_eq!(
            analysis.paths[0].nodes[0].transition,
            QuestSeqTextPathTransition::RecordModeAdvance
        );
        assert_eq!(
            analysis.transition_count(QuestSeqTextPathTransition::RecordModeAdvance),
            1
        );
        assert_eq!(analysis.paths[0].nodes[0].direct_advance_bytes, Some(8));
        assert_eq!(analysis.paths[0].reached_op21_offset, Some(20));

        let mut short = pointer_record(12);
        short.extend_from_slice(&[opcode, 0x10, 0, 0]);
        short.extend_from_slice(&text_tail());
        let analysis =
            audit_quest_seq_text_paths([("TEST.SEQ", short.as_slice())], &code_table()).unwrap();
        assert_eq!(
            analysis.paths[0].nodes[0].transition,
            QuestSeqTextPathTransition::RecordModeAdvance
        );
        assert_eq!(analysis.paths[0].nodes[0].direct_advance_bytes, Some(4));
        assert_eq!(analysis.paths[0].reached_op21_offset, Some(16));
    }
}

#[test]
fn op3a_정적_하위dispatch와_fallback은_4바이트를_따른다() {
    for (opcode, byte1, transition) in [
        (0x3A, 0x02, QuestSeqTextPathTransition::SubdispatchAdvance),
        (0x3A, 0x08, QuestSeqTextPathTransition::SubdispatchAdvance),
        (0x3A, 0x0A, QuestSeqTextPathTransition::SubdispatchAdvance),
        (0x3A, 0x16, QuestSeqTextPathTransition::SubdispatchAdvance),
        (0x30, 0x00, QuestSeqTextPathTransition::FallbackAdvance),
        (0x40, 0x00, QuestSeqTextPathTransition::FallbackAdvance),
    ] {
        let mut raw = pointer_record(12);
        raw.extend_from_slice(&[opcode, byte1, 0, 0]);
        raw.extend_from_slice(&text_tail());
        let analysis =
            audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
        assert_eq!(analysis.paths[0].nodes[0].transition, transition);
        assert_eq!(analysis.transition_count(transition), 1);
        assert_eq!(analysis.paths[0].nodes[0].direct_advance_bytes, Some(4));
        assert_eq!(analysis.paths[0].reached_op21_offset, Some(16));
    }
}

#[test]
fn 다른_stream_pointer_opcode를_순차경로로_넘지_않는다() {
    let mut raw = pointer_record(12);
    raw.extend_from_slice(&[0xA8, 0, 0, 0, 0, 0, 0, 0]);
    raw.extend_from_slice(&text_tail());

    let analysis =
        audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table()).unwrap();
    assert_eq!(
        analysis.paths[0].status,
        QuestSeqTextPathStatus::StoppedAtStreamPointerOpcode
    );
}

#[test]
fn 입력_순서와_무관하게_report가_결정적이다() {
    let mut a = pointer_record(12);
    a.extend_from_slice(&text_tail());
    let mut b = pointer_record(12);
    b.extend_from_slice(&[0x02, 0, 0, 0]);
    b.extend_from_slice(&text_tail());
    let table = code_table();
    let left =
        audit_quest_seq_text_paths([("B.SEQ", b.as_slice()), ("A.SEQ", a.as_slice())], &table)
            .unwrap();
    let right =
        audit_quest_seq_text_paths([("A.SEQ", a.as_slice()), ("B.SEQ", b.as_slice())], &table)
            .unwrap();

    assert_eq!(
        render_quest_seq_text_paths_tsv(&left),
        render_quest_seq_text_paths_tsv(&right)
    );
}

#[test]
fn 빈이름_중복과_빈입력을_거부한다() {
    let raw = text_tail();
    let table = code_table();
    assert!(audit_quest_seq_text_paths([], &table).is_err());
    assert!(audit_quest_seq_text_paths([("", raw.as_slice())], &table).is_err());
    assert!(audit_quest_seq_text_paths(
        [("TEST.SEQ", raw.as_slice()), ("TEST.SEQ", raw.as_slice()),],
        &table,
    )
    .is_err());
}
