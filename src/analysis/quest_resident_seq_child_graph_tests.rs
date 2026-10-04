use super::*;
use crate::coords::LogicalFile;
use crate::font::QUEST_CODE_TABLE_COUNT;

fn map(text_len: usize) -> ExeMap {
    ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_0000,
        text_len: u32::try_from(text_len).expect("test text len"),
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(0, u32::try_from(text_len + 0x800).expect("test file len")),
    }
}

fn code_table() -> QuestCodeTable {
    let raw = (0..QUEST_CODE_TABLE_COUNT)
        .flat_map(|index| u16::try_from(index).expect("test code").to_le_bytes())
        .collect::<Vec<_>>();
    QuestCodeTable::parse(&raw).expect("test code table")
}

fn parent_summary(path: QuestResidentSeqPath) -> QuestResidentSeqPathSummary {
    QuestResidentSeqPathSummary {
        paths: vec![path],
        install_sites: 1,
        nodes: 2,
        op21_records: 0,
        text_glyphs: 0,
        text_controls: 0,
        released_paths: 1,
        runtime_frontiers: 0,
    }
}

#[test]
fn op1c_child는_parent와_별도_path에서_해제까지_걷는다() {
    let text = [
        0x1C, 0, 0, 0, 0x0C, 0x00, 0x01, 0x80, // parent OP1C
        0x00, 0, 0, 0, // parent release
        0x00, 0, 0, 0, // child release
    ];
    let map = map(text.len());
    let table = code_table();
    let parent = walk_resident_stream(&map, &text, &table, map.load_addr, 1).unwrap();

    let graph =
        audit_quest_resident_seq_child_graph(&map, &text, &table, &parent_summary(parent)).unwrap();

    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.paths.len(), 1);
    assert_eq!(graph.paths[0].root_runtime, 0x8001_000C);
    assert_eq!(
        graph.paths[0].status,
        QuestResidentSeqPathStatus::ReleasedInterpreter
    );
    assert_eq!(graph.released_paths, 1);
}

#[test]
fn child_path의_op1c도_별도_grandchild_seed로_확장한다() {
    let text = [
        0x1C, 0, 0, 0, 0x0C, 0x00, 0x01, 0x80, // parent OP1C
        0x00, 0, 0, 0, // parent release
        0x1C, 0, 0, 0, 0x18, 0x00, 0x01, 0x80, // child OP1C
        0x00, 0, 0, 0, // child release
        0x00, 0, 0, 0, // grandchild release
    ];
    let map = map(text.len());
    let table = code_table();
    let parent = walk_resident_stream(&map, &text, &table, map.load_addr, 1).unwrap();

    let graph =
        audit_quest_resident_seq_child_graph(&map, &text, &table, &parent_summary(parent)).unwrap();

    assert_eq!(graph.edges.len(), 2);
    assert_eq!(graph.paths.len(), 2);
    assert_eq!(graph.paths[0].root_runtime, 0x8001_000C);
    assert_eq!(graph.paths[1].root_runtime, 0x8001_0018);
    assert!(graph
        .paths
        .iter()
        .all(|path| path.status == QuestResidentSeqPathStatus::ReleasedInterpreter));
}
