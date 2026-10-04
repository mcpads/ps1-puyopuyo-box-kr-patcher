use super::*;

#[test]
fn resource_id는_archive_descriptor에서_36바이트_stride로_환산한다() {
    let archive = 0x8002_0000u32;
    let resource = 46u16;
    assert_eq!(
        archive + u32::from(resource) * SUBGAME_SEQ_DESCRIPTOR_STRIDE,
        0x8002_0678
    );
}

#[test]
fn 보고서는_실제_장면_선택_미판정을_표시한다() {
    let analysis = SubgameSeqRegistrationAnalysis {
        rows: vec![SubgameSeqRegistrationRow {
            exe: "TEST.EXE",
            archive: "TEST_SEQ.VDD",
            asset: "TEST.SEQ",
            vdd_index: 1,
            registration_record_runtime: 1,
            registration_handler_runtime: 2,
            loader_runtime: 3,
            table_root_runtime: 4,
            archive_descriptor_runtime: 5,
            archive_name_runtime: 6,
            asset_descriptor_runtime: 7,
            resource_id: 8,
            prefix_descriptor_rows: 9,
        }],
    };
    let report = render_subgame_seq_registrations_tsv(&analysis);
    assert!(report.contains("type1-registration-and-selectable-resource-only"));
    assert!(report.contains("0x00000005"));
}
