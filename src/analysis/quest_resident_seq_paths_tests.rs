use super::*;
use crate::coords::LogicalFile;
use crate::font::QUEST_CODE_TABLE_COUNT;

fn map(text_len: usize) -> ExeMap {
    map_at(0x8001_0000, text_len)
}

fn map_at(load_addr: u32, text_len: usize) -> ExeMap {
    ExeMap {
        name: "TEST.EXE",
        load_addr,
        text_len: u32::try_from(text_len).expect("test text len"),
        entry_pc: load_addr,
        file: LogicalFile::new(0, u32::try_from(text_len + 0x800).expect("test file len")),
    }
}

fn code_table_with(codes: &[[u8; 2]]) -> QuestCodeTable {
    let mut table = (0..QUEST_CODE_TABLE_COUNT)
        .map(|index| u16::try_from(index).expect("test code").to_le_bytes())
        .collect::<Vec<_>>();
    for (index, code) in codes.iter().copied().enumerate() {
        table[index] = code;
    }
    let raw = table.into_iter().flatten().collect::<Vec<_>>();
    QuestCodeTable::parse(&raw).expect("test code table")
}

#[test]
fn ff03_text뒤의_record까지_계속_걷는다() {
    let code_table = code_table_with(&[[0x82, 0xA0]]);
    let text = [
        0x21, 0, 0, 0, 0x82, 0xA0, 0x03, 0xFF, 0, 0, 0, 0, 0x21, 0, 0, 0, 0x00, 0xFF, 0, 0, 0, 0,
    ];
    let map = map(text.len());
    let path = walk_resident_stream(&map, &text, &code_table, map.load_addr, 1).unwrap();

    assert_eq!(path.nodes.len(), 2);
    assert_eq!(
        path.nodes[0].transition,
        QuestResidentSeqPathTransition::TextDeactivateAndAlign
    );
    assert_eq!(path.nodes[0].next_runtime, Some(0x8001_000C));
    assert_eq!(
        path.nodes[1].transition,
        QuestResidentSeqPathTransition::TextRelease
    );
    assert_eq!(path.status, QuestResidentSeqPathStatus::ReleasedInterpreter);
}

#[test]
fn runtime_compare에서_후보를_고르지_않는다() {
    let code_table = code_table_with(&[]);
    let text = [0x14, 0x41, 0, 0, 0, 0, 0, 0];
    let map = map(text.len());
    let path = walk_resident_stream(&map, &text, &code_table, map.load_addr, 1).unwrap();

    assert_eq!(path.nodes.len(), 1);
    assert_eq!(
        path.nodes[0].transition,
        QuestResidentSeqPathTransition::RuntimeFrontier
    );
    assert_eq!(
        path.frontier_requirement,
        Some(QuestResidentSeqFrontierRequirement::CompareState)
    );
}

#[test]
fn 비활성_c1은_saved_stream을_읽지_않고_직접_전진한다() {
    let code_table = code_table_with(&[]);
    let text = [
        0xC1, 0, 0, 0, 0, 0, 0, 0, 0x21, 0, 0, 0, 0x00, 0xFF, 0, 0, 0, 0,
    ];
    let map = map(text.len());
    let path = walk_resident_stream(&map, &text, &code_table, map.load_addr, 1).unwrap();

    assert_eq!(path.nodes.len(), 2);
    assert_eq!(
        path.nodes[0].transition,
        QuestResidentSeqPathTransition::DirectAdvance
    );
    assert_eq!(path.nodes[0].next_runtime, Some(0x8001_0008));
    assert_eq!(path.nodes[0].side_stream_runtime, None);
    assert_eq!(path.nodes[0].child_stream_runtime, None);
    assert_eq!(path.status, QuestResidentSeqPathStatus::ReleasedInterpreter);
}

#[test]
fn 활성_c1은_deferred_stream과현재전진을_함께_보존한다() {
    let code_table = code_table_with(&[]);
    let text = [
        0xC1, 1, 0, 0, 0x0C, 0x00, 0x01, 0x80, 0x14, 0x41, 0, 0, 0x01, 0, 0, 0,
    ];
    let map = map(text.len());
    let path = walk_resident_stream(&map, &text, &code_table, map.load_addr, 1).unwrap();

    assert_eq!(path.nodes.len(), 2);
    assert_eq!(
        path.nodes[0].transition,
        QuestResidentSeqPathTransition::SavedStreamAndAdvance
    );
    assert_eq!(path.nodes[0].next_runtime, Some(0x8001_0008));
    assert_eq!(path.nodes[0].side_stream_runtime, Some(0x8001_000C));
    assert_eq!(path.nodes[0].child_stream_runtime, None);
    assert_eq!(
        path.frontier_requirement,
        Some(QuestResidentSeqFrontierRequirement::CompareState)
    );
}

#[test]
fn op1c_800896dc는_parent와_child_stream을_별도_edge로_보존한다() {
    let code_table = code_table_with(&[]);
    let text = [
        0x1C, 0x00, 0x00, 0x00, 0xE8, 0x96, 0x08, 0x80, // 0x800896DC
        0x00, 0x00, 0x00, 0x00, // 0x800896E4 parent release
        0xC1, 0x01, 0x00, 0x00, // 0x800896E8 child stream
    ];
    let map = map_at(0x8008_96DC, text.len());
    let parent = walk_resident_stream(&map, &text, &code_table, map.load_addr, 1).unwrap();

    assert_eq!(parent.nodes.len(), 2);
    assert_eq!(
        parent.nodes[0].transition,
        QuestResidentSeqPathTransition::ChildStreamAndAdvance
    );
    assert_eq!(parent.nodes[0].next_runtime, Some(0x8008_96E4));
    assert_eq!(parent.nodes[0].side_stream_runtime, None);
    assert_eq!(parent.nodes[0].child_stream_runtime, Some(0x8008_96E8));
    assert_eq!(
        parent.nodes[1].transition,
        QuestResidentSeqPathTransition::ReleaseInterpreter
    );
    assert_eq!(
        parent.status,
        QuestResidentSeqPathStatus::ReleasedInterpreter
    );

    assert_eq!(parent.nodes[1].runtime, 0x8008_96E4);
}

#[test]
fn op1c_child_stream이_text_밖이면_실패한다() {
    let code_table = code_table_with(&[]);
    let text = [0x1C, 0, 0, 0, 0x0C, 0x00, 0x01, 0x80, 0x00, 0, 0, 0];
    let map = map(text.len());
    let error = walk_resident_stream(&map, &text, &code_table, map.load_addr, 1).unwrap_err();

    assert!(error
        .to_string()
        .contains("OP1C child stream이 EXE text 밖임"));
}

#[test]
fn deferred_800898d0은_op00에서_종료하고_cell_table을_읽지_않는다() {
    let code_table = code_table_with(&[]);
    let text = [
        0xC1, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 0x800898D0
        0x8C, 0x00, 0xFF, 0x00, // 0x800898D8
        0xC2, 0x00, 0x00, 0x00, // 0x800898DC
        0x02, 0x00, 0x08, 0x00, // 0x800898E0
        0x3A, 0x16, 0x00, 0x00, // 0x800898E4
        0x3A, 0x08, 0x00, 0x00, // 0x800898E8
        0x02, 0x00, 0x08, 0x00, // 0x800898EC
        0xBC, 0x00, 0x00, 0x00, // 0x800898F0
        0xBC, 0x01, 0x00, 0x00, // 0x800898F4
        0x00, 0x00, 0x00, 0x00, // 0x800898F8
        0x00, 0x01, 0x00, 0x01, // 0x800898FC cell table
    ];
    let map = map_at(0x8008_98D0, text.len());
    let path = walk_resident_stream(&map, &text, &code_table, map.load_addr, 0).unwrap();

    assert_eq!(
        path.nodes
            .iter()
            .map(|node| node.opcode)
            .collect::<Vec<_>>(),
        [0xC1, 0x8C, 0xC2, 0x02, 0x3A, 0x3A, 0x02, 0xBC, 0xBC, 0x00]
    );
    assert_eq!(
        path.nodes.last().map(|node| node.runtime),
        Some(0x8008_98F8)
    );
    assert!(path.nodes.iter().all(|node| node.runtime != 0x8008_98FC));
    assert_eq!(path.status, QuestResidentSeqPathStatus::ReleasedInterpreter);
}

#[test]
fn op1c_child_80089868은_80089884_op00에서_종료한다() {
    let code_table = code_table_with(&[]);
    let text = [
        0x3A, 0x16, 0x00, 0x00, // 0x80089868
        0x3A, 0x08, 0x00, 0x00, // 0x8008986C
        0x10, 0x80, 0xA2, 0x00, 0x00, 0x00, 0x00, 0x00, // 0x80089870
        0xBC, 0x00, 0x00, 0x00, // 0x80089878
        0x11, 0x40, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, // 0x8008987C
        0x00, 0x00, 0x00, 0x00, // 0x80089884
    ];
    let map = map_at(0x8008_9868, text.len());
    let path = walk_resident_stream(&map, &text, &code_table, map.load_addr, 0).unwrap();

    assert_eq!(
        path.nodes
            .iter()
            .map(|node| node.opcode)
            .collect::<Vec<_>>(),
        [0x3A, 0x3A, 0x10, 0xBC, 0x11, 0x00]
    );
    assert_eq!(
        path.nodes.last().map(|node| node.runtime),
        Some(0x8008_9884)
    );
    assert_eq!(path.status, QuestResidentSeqPathStatus::ReleasedInterpreter);
}
