//! DRAC consumer 분석 결과를 안정된 TSV 스키마로 직렬화한다.

use std::fmt::Write as _;

use super::{
    SubgameSeqDracConsumerAnalysis, BOX_SUBGAME_SEQ_DRAC_COMMAND_OPCODE,
    BOX_SUBGAME_SEQ_DRAC_COMMAND_TABLE_ENTRIES, SUBGAME_SEQ_DRAC_CONSUMER_SCHEMA_VERSION,
};

pub fn render_subgame_seq_drac_consumers_tsv(analysis: &SubgameSeqDracConsumerAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tscope\texe\tcommand_opcode\tcommand_records\tvariant_records\t\
         command_registration_runtime\tcommand_source_table_runtime\tcommand_copy_words_runtime\t\
         command_table_entries\t\
         command_dispatcher_runtime\tcommand_dispatch_table_runtime\tmessage_handler_runtime\t\
         current_record_load_runtime\theader_transform\t\
         header_consumer_runtime\tbody_tokenizer_runtime\ttoken_cursor_load_runtime\t\
         token_table_runtime\tdispatch_table_runtime\tdispatch_entries\t\
         dispatch_first_token\tdispatch_last_token\targument_tokens\tforbidden_state_tokens\trecords\t\
         header_bytes\tbody_bytes\tglyph_tokens\targument_controls\tstate_controls\t\
         terminators\tcontrol_counts\tvariant_state_runtime\theader_layout_matches\t\
         header_profiles\tline_break_pairs\tcolumn_advances\tend_waits\tcurrent_waits\t\
         message_object_roles\tmessage_update_runtime\tmessage_callback_field\t\
         glyph_handler_runtime\tglyph_coordinate_formula\t\
         observed_effects\tstatic_status\n",
    );
    for row in &analysis.rows {
        let control_counts = row
            .control_counts
            .iter()
            .map(|(code, count)| format!("{code:02X}:{count}"))
            .collect::<Vec<_>>()
            .join(",");
        let header_profiles = row
            .message_layouts
            .iter()
            .map(|layout| {
                format!(
                    "{}:v{}:{:02X},{:02X},{}x{}={}x{}",
                    layout.index,
                    layout.variant,
                    layout.header_x_units,
                    layout.header_y_units,
                    layout.header_columns,
                    layout.header_rows,
                    layout.derived_columns,
                    layout.derived_rows,
                )
            })
            .collect::<Vec<_>>()
            .join(";");
        let argument_tokens = row
            .argument_tokens
            .iter()
            .map(|token| format!("{token:02X}"))
            .collect::<Vec<_>>()
            .join(",");
        let forbidden_state_tokens = row
            .forbidden_state_tokens
            .iter()
            .map(|token| format!("{token:02X}:forbidden-1-byte"))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            output,
            "{schema}\t{scope}\t{exe}\t0x{opcode:02X}\t{command_records}\t{variant_records}\t\
             0x{registration:08X}\t0x{source_table:08X}\t0x{copy_words:08X}\t{table_entries}\t\
             0x{dispatcher:08X}\t0x{dispatch_table:08X}\t0x{message_handler:08X}\t\
             0x{current_record:08X}\t-1,-1,+1,+1\t0x{header_consumer:08X}\t\
             0x{tokenizer:08X}\t0x{token_cursor:08X}\t0x{token_table:08X}\t\
             0x{token_dispatch:08X}\t{dispatch_entries}\t0x{dispatch_first:02X}\t\
             0x{dispatch_last:02X}\t{argument_tokens}\t{forbidden_state_tokens}\t\
             {records}\t{header_bytes}\t\
             {body_bytes}\t{glyph_tokens}\t{argument_controls}\t{state_controls}\t\
             {terminators}\t{control_counts}\t0x{variant_state:08X}\t{layout_matches}\t\
             {header_profiles}\t{line_break_pairs}\t{column_advances}\t{end_waits}\t\
             {current_waits}\t+50=column,+54=row-step,+58=token-cursor,+98=end-wait,+A4=current-wait,+A8=glyph-wait,+AC=terminal-state\t\
             0x{message_update:08X}\t+0x18\t0x{glyph_handler:08X}\t\
             glyph+50=((header-x*8)+(column*16))<<12,glyph+54=(header-y+(row-step*3))<<15\t\
             65:terminal+end-wait,71:arg->end-wait,72x2:line-break,73:column-advance+glyph-wait,77:arg->current-wait\t\
             exact-e0-layout-and-timing-dataflow-static-runtime-ux-pending",
            schema = SUBGAME_SEQ_DRAC_CONSUMER_SCHEMA_VERSION,
            scope = row.scope,
            exe = row.exe,
            opcode = BOX_SUBGAME_SEQ_DRAC_COMMAND_OPCODE,
            command_records = row.command_records,
            variant_records = row.variant_records,
            registration = row.command_registration_runtime,
            source_table = row.command_source_table_runtime,
            copy_words = row.command_copy_words_runtime,
            table_entries = BOX_SUBGAME_SEQ_DRAC_COMMAND_TABLE_ENTRIES,
            dispatcher = row.command_dispatcher_runtime,
            dispatch_table = row.command_dispatch_table_runtime,
            message_handler = row.message_handler_runtime,
            current_record = row.current_record_load_runtime,
            header_consumer = row.header_consumer_runtime,
            tokenizer = row.body_tokenizer_runtime,
            token_cursor = row.token_cursor_load_runtime,
            token_table = row.token_table_runtime,
            token_dispatch = row.dispatch_table_runtime,
            dispatch_entries = row.dispatch_entries,
            dispatch_first = row.dispatch_first_token,
            dispatch_last = row.dispatch_last_token,
            argument_tokens = argument_tokens,
            forbidden_state_tokens = forbidden_state_tokens,
            records = row.records,
            header_bytes = row.header_bytes,
            body_bytes = row.body_bytes,
            glyph_tokens = row.glyph_tokens,
            argument_controls = row.argument_controls,
            state_controls = row.state_controls,
            terminators = row.terminators,
            control_counts = control_counts,
            variant_state = row.variant_state_runtime,
            layout_matches = row.header_layout_matches,
            header_profiles = header_profiles,
            line_break_pairs = row.line_break_pairs,
            column_advances = row.column_advances,
            end_waits = row.end_waits,
            current_waits = row.current_waits,
            message_update = row.message_update_runtime,
            glyph_handler = row.glyph_handler_runtime,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}
