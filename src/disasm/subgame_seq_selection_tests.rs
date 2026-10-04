use super::*;

#[test]
fn resource46_record는_4바이트_정렬된_exact_8바이트만_고른다() {
    let mut raw = vec![0u8; 24];
    raw[4..12].copy_from_slice(&SELECTION_RECORD);
    raw[13..21].copy_from_slice(&SELECTION_RECORD);
    assert_eq!(find_selection_records(&raw), vec![4]);
}

#[test]
fn direct_pointer_field는_initialized_범위만_센다() {
    let target = 0x8019_EC58u32;
    let mut raw = vec![0u8; 16];
    raw[4..8].copy_from_slice(&target.to_le_bytes());
    raw[12..16].copy_from_slice(&target.to_le_bytes());
    assert_eq!(direct_pointer_field_offsets(&raw, 12, target), vec![4]);
}

#[test]
fn opcode10_slot은_32word_table_안의_index16이다() {
    assert_eq!(usize::from(SUBGAME_SEQ_SELECTION_OPCODE), 16);
    assert!(usize::from(SUBGAME_SEQ_SELECTION_OPCODE) < SUBGAME_SEQ_OPCODE_TABLE_ENTRIES);
}

#[test]
fn 보고서는_runtime_실행이나_표시를_주장하지_않는다() {
    let analysis = SubgameSeqSelectionAnalysis {
        rows: vec![SubgameSeqSelectionRow {
            exe: "TEST.EXE",
            archive: "/TEST/TEST.VDD;1",
            scene: "SCENE.SEQ",
            load_base: 1,
            record_offset: 2,
            record_runtime: 3,
            direct_pointer_field_runtimes: vec![4],
            dispatcher_runtime: 5,
            opcode_table_runtime: 6,
            selector_slot_runtime: 7,
            selector_runtime: 8,
            selected_asset: "DRAC_SEQ.SEQ",
            asset_descriptor_runtime: 9,
        }],
    };
    let report = render_subgame_seq_selections_tsv(&analysis);
    assert!(report.contains("scene-record-to-drac-descriptor-selection-only"));
    assert!(report.contains("0x00000004"));
}
