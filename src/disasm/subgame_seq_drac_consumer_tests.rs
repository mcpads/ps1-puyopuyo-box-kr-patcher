use super::*;

#[test]
fn message_record는_header_glyph_control_종결을_빈틈없이_나눈다() {
    let raw = [
        0x12, 0x09, 0x09, 0x03, 0x82, 0xA8, 0x71, 0x41, 0x72, 0x73, 0x77, 0x16, 0x65, 0x65,
    ];
    let stats = parse_message_record(&raw).unwrap();
    assert_eq!(stats.header_bytes, 4);
    assert_eq!(stats.body_bytes, 10);
    assert_eq!(stats.glyph_tokens, 1);
    assert_eq!(stats.argument_controls, 2);
    assert_eq!(stats.state_controls, 2);
    assert_eq!(stats.terminators, 1);
    assert_eq!(
        stats.control_counts,
        [(0x65, 1), (0x71, 1), (0x72, 1), (0x73, 1), (0x77, 1)]
            .into_iter()
            .collect()
    );
}

#[test]
fn 미지원_control과_종결뒤_trailing_byte를_거부한다() {
    assert!(parse_message_record(&[1, 2, 3, 4, 0x70, 0x65, 0x65]).is_err());
    assert!(parse_message_record(&[1, 2, 3, 4, 0x65, 0x65, 0]).is_err());
}

#[test]
fn message_pointer앞_e0_command와_variant만_허용한다() {
    assert_eq!(
        parse_message_command_variant(&[0xE0, 0, 0, 0, 1, 2, 3, 4], 4).unwrap(),
        0
    );
    assert_eq!(
        parse_message_command_variant(&[0xE0, 1, 0, 0, 1, 2, 3, 4], 4).unwrap(),
        1
    );
    assert!(parse_message_command_variant(&[0xE1, 0, 0, 0], 4).is_err());
    assert!(parse_message_command_variant(&[0xE0, 2, 0, 0], 4).is_err());
    assert!(parse_message_command_variant(&[0xE0, 0, 1, 0], 4).is_err());
}

#[test]
fn header_열행과_control_layout을_대조한다() {
    let raw = [
        0x10, 0x08, 0x03, 0x02, 0x82, 0xA8, 0x73, 0x91, 0x4F, 0x72, 0x72, 0x82, 0xC1, 0x81, 0x49,
        0x82, 0xA8, 0x71, 0x20, 0x77, 0x0C, 0x65, 0x65,
    ];
    let layout = analyze_message_layout(&raw, 3, 1).unwrap();
    assert_eq!(layout.header_columns, 3);
    assert_eq!(layout.header_rows, 2);
    assert_eq!(layout.derived_columns, 3);
    assert_eq!(layout.derived_rows, 2);
    assert_eq!(layout.line_break_pairs, 1);
    assert_eq!(layout.column_advances, 1);
    assert_eq!(layout.end_waits, 1);
    assert_eq!(layout.current_waits, 1);
    assert!(analyze_message_layout(&[0, 0, 1, 2, 0x72, 0x65, 0x65], 1, 0).is_err());
}

#[test]
fn report는_정적소비와_runtime표시를_분리한다() {
    let message_layout = SubgameSeqDracMessageLayout {
        index: 1,
        variant: 0,
        header_x_units: 0x1A,
        header_y_units: 0x0F,
        header_columns: 4,
        header_rows: 1,
        derived_columns: 4,
        derived_rows: 1,
        line_break_pairs: 0,
        column_advances: 0,
        end_waits: 0,
        current_waits: 0,
    };
    let row = SubgameSeqDracConsumerRow {
        scope: "PUYO1",
        exe: "PUYO1.EXE",
        current_record_load_runtime: 1,
        header_consumer_runtime: 2,
        body_tokenizer_runtime: 3,
        token_cursor_load_runtime: 4,
        token_table_runtime: 5,
        dispatch_table_runtime: 6,
        dispatch_entries: 22,
        dispatch_first_token: 0x63,
        dispatch_last_token: 0x78,
        argument_tokens: BOX_SUBGAME_SEQ_DRAC_ARGUMENT_TOKENS,
        forbidden_state_tokens: vec![0x63, 0x78],
        records: 5,
        header_bytes: 20,
        body_bytes: 220,
        glyph_tokens: 89,
        argument_controls: 9,
        state_controls: 14,
        terminators: 5,
        control_counts: OBSERVED_CONTROL_COUNTS.into_iter().collect(),
        command_registration_runtime: 7,
        command_source_table_runtime: 8,
        command_copy_words_runtime: 9,
        command_dispatcher_runtime: 10,
        command_dispatch_table_runtime: 11,
        message_handler_runtime: 12,
        command_records: 5,
        variant_records: 1,
        variant_state_runtime: 13,
        header_layout_matches: 5,
        line_break_pairs: 6,
        column_advances: 2,
        end_waits: 2,
        current_waits: 7,
        message_layouts: vec![message_layout],
        glyph_handler_runtime: 14,
        message_update_runtime: 15,
    };
    let report =
        render_subgame_seq_drac_consumers_tsv(&SubgameSeqDracConsumerAnalysis { rows: vec![row] });
    assert!(report.contains("-1,-1,+1,+1"));
    assert!(report.contains("71:arg->end-wait"));
    assert!(report.contains("72x2:line-break"));
    assert!(report.contains("glyph+50=((header-x*8)+(column*16))<<12"));
    assert!(report.contains("message_update_runtime\tmessage_callback_field"));
    assert!(report.contains("63:forbidden-1-byte,78:forbidden-1-byte"));
    assert!(report.contains("exact-e0-layout-and-timing-dataflow-static-runtime-ux-pending"));
}
