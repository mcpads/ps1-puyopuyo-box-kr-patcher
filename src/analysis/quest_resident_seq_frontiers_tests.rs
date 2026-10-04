use super::*;
use crate::coords::LogicalFile;

fn map(text_len: usize) -> ExeMap {
    ExeMap {
        name: "TEST.EXE",
        load_addr: 0x8001_0000,
        text_len: u32::try_from(text_len).expect("test text len"),
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(0, u32::try_from(text_len + 0x800).expect("test file len")),
    }
}

#[test]
fn op14_global_word비교의_두_다음주소와_probe를_보존한다() {
    let text = [
        0x14, 0x41, 0, 0, 0, 0, 0, 0, 0x10, 0x00, 0x01, 0x80, 0, 0, 0, 0, 0,
    ];
    let map = map(text.len());
    let frontier = compare_frontier(&map, &text, map.load_addr).unwrap();

    assert_eq!(frontier.operation, QuestSeqBaseCompareOperation::Equal);
    assert_eq!(frontier.left_parameter, Some(0));
    assert_eq!(frontier.right_parameter, 0);
    assert_eq!(frontier.branch_target_runtime, 0x8001_0010);
    assert_eq!(frontier.fallthrough_runtime, 0x8001_000C);
    assert_eq!(
        frontier.runtime_probe_address,
        Some(QUEST_SEQ_BASE_COMPARE_STATE_BASE_RUNTIME)
    );
    assert_eq!(frontier.runtime_probe_length, Some(4));
}

#[test]
fn op80은_조건byte와_exe안_spawn_target을_요구한다() {
    let text = [0x80, 0, 0, 0, 0x08, 0x00, 0x01, 0x80, 0, 0, 0, 0];
    let map = map(text.len());
    assert_eq!(spawn_stream(&map, &text, map.load_addr), Ok(0x8001_0008));

    let mut wrong_condition = text;
    wrong_condition[1] = 1;
    assert!(spawn_stream(&map, &wrong_condition, map.load_addr).is_err());

    let mut outside = text;
    outside[4..8].copy_from_slice(&0x8002_0000u32.to_le_bytes());
    assert!(spawn_stream(&map, &outside, map.load_addr).is_err());
}
