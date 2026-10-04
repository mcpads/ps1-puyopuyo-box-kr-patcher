use super::*;

#[test]
fn load_record는_flags_resource_destination을_한_단위로_보존한다() {
    assert_eq!(LOAD_RECORD[2], SUBGAME_SEQ_LOAD_FLAGS);
    assert_eq!(u16::from_le_bytes([LOAD_RECORD[4], LOAD_RECORD[5]]), 46);
    assert_eq!(
        u32::from_le_bytes(LOAD_RECORD[8..12].try_into().unwrap()),
        SUBGAME_SEQ_LOAD_DESTINATION
    );
    assert_eq!(&LOAD_RECORD[12..], &[0, 0, 0, 0]);
}

#[test]
fn flags97은_queue_callback_destination_record경로를_모두_켠다() {
    assert_ne!(SUBGAME_SEQ_LOAD_FLAGS & 0x01, 0);
    assert_ne!(SUBGAME_SEQ_LOAD_FLAGS & 0x02, 0);
    assert_ne!(SUBGAME_SEQ_LOAD_FLAGS & 0x10, 0);
    assert_eq!(SUBGAME_SEQ_LOAD_FLAGS & 0x20, 0);
    assert_ne!(SUBGAME_SEQ_LOAD_FLAGS & 0x80, 0);
}

#[test]
fn 보고서는_runtime실행과_표시를_승격하지_않는다() {
    let analysis = SubgameSeqLoadAnalysis {
        rows: vec![SubgameSeqLoadRow {
            exe: "TEST.EXE",
            archive: "/TEST/TEST.VDD;1",
            scene: "SCENE.SEQ",
            record_runtime: 1,
            destination_runtime: 2,
            selected_asset: "DRAC_SEQ.SEQ",
            asset_descriptor_runtime: 3,
            completion_callback_runtime: 4,
            streaming_init_wrapper_runtime: 5,
            streaming_continue_wrapper_runtime: 6,
            direct_wrapper_runtime: 7,
            descriptor_destination_field_runtime: 8,
            initialized_bytes: 9,
        }],
    };
    assert!(
        render_subgame_seq_loads_tsv(&analysis).contains("scene-record-to-cnx-output-overlay-only")
    );
}
