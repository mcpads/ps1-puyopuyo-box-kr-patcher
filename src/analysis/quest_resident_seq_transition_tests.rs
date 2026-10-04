use super::*;
use crate::coords::{LogicalFile, PSX_EXE_HEADER_LEN};

fn map() -> ExeMap {
    ExeMap {
        name: "QUEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x0008_0000,
        entry_pc: 0x8001_0000,
        file: LogicalFile::new(0, 0x0008_0000 + PSX_EXE_HEADER_LEN as u32),
    }
}

fn source_text() -> Vec<u8> {
    let map = map();
    let mut text = vec![0; map.text_len as usize];
    let branch_offset = map
        .text_offset(QUEST_START_MENU_BRANCH_RUNTIME)
        .expect("branch offset");
    text[branch_offset..branch_offset + 4].copy_from_slice(&[
        0x09,
        QUEST_START_MENU_SELECTION_FIELD_OFFSET,
        QUEST_START_MENU_SELECTION_COUNT as u8,
        0,
    ]);
    for (index, target) in QUEST_START_MENU_BRANCH_TARGETS.iter().enumerate() {
        let offset = branch_offset + 4 + index * 4;
        text[offset..offset + 4].copy_from_slice(&target.to_le_bytes());
    }
    let record_offset = map
        .text_offset(QUEST_RESIDENT_OP8A_RUNTIME)
        .expect("record offset");
    text[record_offset..record_offset + QUEST_RESIDENT_OP8A_RECORD_BYTES]
        .copy_from_slice(&QUEST_RESIDENT_OP8A_RAW);
    text
}

fn layout() -> QuestSeqResourceCatalogLayout {
    QuestSeqResourceCatalogLayout {
        descriptor_root_runtime: 0x8008_F9D0,
        descriptor_stride: 36,
        prefix_descriptor_rows: 1,
    }
}

#[test]
fn 시작메뉴의_새게임분기와_상주_op8a를_결속한다() {
    let summary = verify_box_quest_resident_seq_transition(
        &map(),
        &source_text(),
        ["ALLMAP.SEQ", "SUKE.SEQ", "TOWN00.SEQ"],
        layout(),
    )
    .unwrap();

    assert_eq!(summary.new_game_selection_index, 1);
    assert_eq!(summary.new_game_stream_runtime, 0x8008_96DC);
    assert_eq!(summary.record_text_offset, 0x0007_9798);
    assert_eq!(summary.encoded_resource_id, 3);
    assert_eq!(summary.descriptor_runtime, 0x8008_FA3C);
    assert_eq!(summary.vdd_entry_index, 2);
    assert_eq!(summary.target_sequence, "TOWN00.SEQ");
}

#[test]
fn 상주_op8a_record_drift를_거부한다() {
    let map = map();
    let mut text = source_text();
    let offset = map
        .text_offset(QUEST_RESIDENT_OP8A_RUNTIME)
        .expect("record offset");
    text[offset + 2] = 4;

    assert!(verify_box_quest_resident_seq_transition(
        &map,
        &text,
        ["ALLMAP.SEQ", "SUKE.SEQ", "TOWN00.SEQ", "TOWN10.SEQ"],
        layout(),
    )
    .is_err());
}

#[test]
fn 시작메뉴_selection_table_drift를_거부한다() {
    let map = map();
    let mut text = source_text();
    let offset = map
        .text_offset(QUEST_START_MENU_BRANCH_RUNTIME)
        .expect("branch offset");
    text[offset + 8] ^= 1;

    assert!(verify_box_quest_resident_seq_transition(
        &map,
        &text,
        ["ALLMAP.SEQ", "SUKE.SEQ", "TOWN00.SEQ"],
        layout(),
    )
    .is_err());
}
