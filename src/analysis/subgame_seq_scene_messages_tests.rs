use super::*;

#[test]
fn message는_glyph와_인자없는_forbidden_token을_빈틈없이_나눈다() {
    let raw = [1, 2, 3, 4, 0x82, 0xA0, 0x63, 0x71, 0x20, 0x78, 0x65, 0x65];
    let parsed = parse_message(
        &raw,
        0,
        &[[0x82, 0xA0]],
        0x63,
        0x78,
        &BOX_SUBGAME_SEQ_DRAC_ARGUMENT_TOKENS,
    )
    .unwrap();
    assert_eq!(parsed.end_offset, raw.len());
    assert_eq!(parsed.glyph_tokens, 1);
    assert_eq!(parsed.control_tokens, 3);
    assert_eq!(parsed.argument_controls, 1);
    assert_eq!(parsed.source_text, "あ<63><71:20><78><6565>");
}

#[test]
fn message는_범위밖_token과_repertoire밖_glyph를_거부한다() {
    assert!(parse_message(&[1, 2, 3, 4, 0x62, 0x65, 0x65], 0, &[], 0x63, 0x78, &[]).is_err());
    assert!(parse_message(
        &[1, 2, 3, 4, 0x82, 0xA0, 0x65, 0x65],
        0,
        &[[0x82, 0xA2]],
        0x63,
        0x78,
        &[],
    )
    .is_err());
}

#[test]
fn report는_정적소비와_runtime_재배치를_분리한다() {
    let row = SubgameSeqSceneMessageRow {
        scope: "PUYO1",
        exe: "PUYO1.EXE",
        archive: "/PUYO1/PP1_SEQ.VDD;1",
        sequence: "MIN_SEQ.SEQ",
        sequence_sha256: [1; 32],
        index: 1,
        command_offset: 4,
        pointer_field_offset: 8,
        pointer_field_runtime: 9,
        record_offset: 12,
        record_runtime: 13,
        record_end_offset: 20,
        variant: 0,
        header: [1, 2, 3, 4],
        raw: vec![1, 2, 3, 4, 0x63, 0x65, 0x65],
        raw_sha256: [2; 32],
        payload_copy_count: 1,
        counterpart_scope: None,
        counterpart_index: None,
        glyph_tokens: 0,
        control_tokens: 1,
        argument_controls: 0,
        unique_glyph_cells: 0,
        maximum_glyph_cell: 0,
        control_counts: [(0x63, 1)].into_iter().collect(),
        source_text: "<63><6565>".to_string(),
    };
    let report = render_subgame_seq_scene_messages_tsv(&SubgameSeqSceneMessageAnalysis {
        rows: vec![row],
        families: 1,
        physical_messages: 1,
        glyph_tokens: 0,
        control_tokens: 1,
        argument_controls: 0,
        terminators: 1,
        variant_records: 0,
        direct_pointers: 1,
        referenced_message_bytes: 7,
        unique_message_bytes: 7,
        unique_payloads: 1,
        cross_scope_pairs: 0,
        control_counts: [(0x63, 1)].into_iter().collect(),
    });
    assert!(report.contains("63/78=forbidden-1-byte"));
    assert!(report.contains("runtime_verdict\tpatch_strategy"));
    assert!(report.contains("preserve-no-write-until-relink-layout-and-runtime-contract"));
}
