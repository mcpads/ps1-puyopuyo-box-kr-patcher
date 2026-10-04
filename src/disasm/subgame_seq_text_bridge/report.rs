//! 검증된 text bridge 분석 결과의 TSV 직렬화.

use std::fmt::Write as _;

use crate::coords::normalize_kseg;
use crate::isa::Reg;

use super::{
    hex, SubgameSeqTextBridgeAnalysis, SUBGAME_SEQ_TEXT_BRIDGE_MODE,
    SUBGAME_SEQ_TEXT_BRIDGE_OPCODE, SUBGAME_SEQ_TEXT_BRIDGE_SCHEMA_VERSION,
};

pub fn render_subgame_seq_text_bridges_tsv(analysis: &SubgameSeqTextBridgeAnalysis) -> String {
    let mut output = String::from(
        "schema_version\texe\tarchive\tconstructor_runtime\tsynthetic_record_runtime\t\
         synthetic_opcode\tsynthetic_mode\tdispatcher_runtime\tdispatcher_table_runtime\t\
         op21_handler_runtime\top21_mode2_handler_runtime\tobject_consumer_runtime\t\
         object_consumer_source_runtime\texe_direct_flow_refs\texe_address_load_refs\t\
         exe_raw_pointer_refs\tsequence_files_scanned\tsequence_direct_jal_refs\t\
         sequence_direct_jump_refs\tsequence_address_load_refs\tsequence_raw_pointer_refs\t\
         constructor_entry_reached\tentry_indirect_sites\tentry_jr_bios_sites\t\
         entry_jr_bios_runtime\tentry_jr_table_sites\tentry_jr_table_runtime\t\
         entry_jr_table_source_runtime\tentry_jr_table_entries\t\
         entry_jr_table_constructor_targets\texpanded_trace_iterations\t\
         expanded_table_target_seeds\texpanded_table_targets_outside_exe\t\
         expanded_reached_instructions\texpanded_indirect_sites\texpanded_jalr_sites\t\
         expanded_jr_bios_sites\texpanded_jr_table_sites\t\
         expanded_jr_unclassified_sites\texpanded_jr_unclassified_runtime\t\
         expanded_constructor_reached\texpanded_jalr_bios_vector_sites\t\
         expanded_jalr_bios_vector_runtime\texpanded_jalr_initialized_direct_sites\t\
         expanded_jalr_initialized_direct_source_runtime\t\
         expanded_jalr_initialized_direct_target\t\
         expanded_jalr_initialized_rooted_sites\t\
         expanded_jalr_initialized_rooted_source_runtime\t\
         expanded_jalr_initialized_rooted_target\t\
         expanded_jalr_initialized_indexed_sites\t\
         expanded_jalr_initialized_indexed_call_runtime\t\
         expanded_jalr_initialized_indexed_table_runtime\t\
         expanded_jalr_initialized_indexed_table_entries\t\
         expanded_jalr_initialized_indexed_unique_targets\t\
         expanded_jalr_initialized_indexed_zero_targets\t\
         expanded_jalr_initialized_indexed_outside_exe_targets\t\
         expanded_jalr_initialized_indexed_constructor_targets\t\
         expanded_jalr_initialized_indexed_sha256\t\
         expanded_jalr_outside_text_sites\texpanded_jalr_outside_text_source_runtime\t\
         expanded_jalr_outside_text_call_runtime\t\
         expanded_jalr_outside_text_direct_read_runtime\t\
         expanded_jalr_outside_text_direct_write_runtime\t\
         expanded_jalr_outside_text_address_value_runtime\t\
         expanded_jalr_computed_memory_sites\t\
         expanded_jalr_computed_memory_runtime\t\
         expanded_jalr_computed_memory_load_runtime\t\
         expanded_jalr_computed_memory_target_register\t\
         expanded_jalr_computed_memory_base_register\t\
         expanded_jalr_computed_memory_field_offset\t\
         expanded_jalr_forwarded_register_sites\t\
         expanded_jalr_forwarded_register_runtime\t\
         expanded_jalr_forwarded_capture_runtime\t\
         expanded_jalr_forwarded_function_entry_runtime\t\
         expanded_jalr_forwarded_source_argument\t\
         expanded_jalr_forwarded_direct_callers\t\
         expanded_jalr_forwarded_direct_call_runtime\t\
         expanded_jalr_forwarded_direct_target\t\
         expanded_jalr_forwarded_entry_address_load_refs\t\
         expanded_jalr_forwarded_entry_raw_pointer_refs\t\
         expanded_jalr_forwarded_constructor_targets\t\
         expanded_jalr_unclassified_sites\t\
         expanded_jalr_unclassified_runtime\texpanded_jalr_fresh_stack_arg5_sites\t\
         expanded_jalr_fresh_stack_arg5_runtime\t\
         expanded_jalr_initialized_constructor_targets\texe_stack_arg5_jalr_sites\tstack_arg5_jalr_runtime\t\
         stack_arg5_store_runtime\tindirect_table_runtime\tindirect_table_entries\t\
         indirect_table_unique_targets\tindirect_table_zero_targets\t\
         indirect_table_outside_exe_targets\tindirect_table_constructor_targets\t\
         indirect_table_sha256\tindirect_table_kind\texe_canonical_mode2_records\t\
         sequence_canonical_mode2_records\tdrac_relation\tstatic_status\tcaller_status\t\
         runtime_status\n",
    );
    for row in &analysis.rows {
        writeln!(
            output,
            "{schema}\t{exe}\t{archive}\t0x{constructor:08X}\t0x{synthetic:08X}\t0x{opcode:02X}\t{mode}\t0x{dispatcher:08X}\t0x{dispatcher_table:08X}\t0x{op21:08X}\t0x{mode2:08X}\t0x{consumer:08X}\t0x{source:08X}\t{flow}\t{address}\t{raw}\t{files}\t{jal}\t{jump}\t{sequence_address}\t{sequence_raw}\t{reached}\t{indirect}\t{jr_bios_sites}\t{jr_bios_runtime}\t{jr_table_sites}\t{jr_table_runtime}\t{jr_table_source}\t{jr_table_entries}\t{jr_table_constructor}\t{expanded_iterations}\t{expanded_seeds}\t{expanded_outside}\t{expanded_instructions}\t{expanded_indirect}\t{expanded_jalr}\t{expanded_jr_bios}\t{expanded_jr_tables}\t{expanded_jr_unclassified}\t{expanded_jr_unclassified_runtime}\t{expanded_constructor}\t{expanded_jalr_bios_sites}\t{expanded_jalr_bios_runtime}\t{expanded_jalr_direct_sites}\t{expanded_jalr_direct_sources}\t{expanded_jalr_direct_targets}\t{expanded_jalr_rooted_sites}\t{expanded_jalr_rooted_sources}\t{expanded_jalr_rooted_targets}\t{expanded_jalr_indexed_sites}\t{expanded_jalr_indexed_calls}\t{expanded_jalr_indexed_tables}\t{expanded_jalr_indexed_entries}\t{expanded_jalr_indexed_unique}\t{expanded_jalr_indexed_zero}\t{expanded_jalr_indexed_outside}\t{expanded_jalr_indexed_constructor}\t{expanded_jalr_indexed_sha}\t{expanded_jalr_outside_sites}\t{expanded_jalr_outside_sources}\t{expanded_jalr_outside_calls}\t{expanded_jalr_outside_reads}\t{expanded_jalr_outside_writes}\t{expanded_jalr_outside_address_values}\t{expanded_jalr_computed_sites}\t{expanded_jalr_computed_runtime}\t{expanded_jalr_computed_load}\t{expanded_jalr_computed_target}\t{expanded_jalr_computed_base}\t{expanded_jalr_computed_offset}\t{expanded_jalr_forwarded_sites}\t{expanded_jalr_forwarded_runtime}\t{expanded_jalr_forwarded_capture}\t{expanded_jalr_forwarded_entry}\t{expanded_jalr_forwarded_argument}\t{expanded_jalr_forwarded_callers}\t{expanded_jalr_forwarded_calls}\t{expanded_jalr_forwarded_targets}\t{expanded_jalr_forwarded_address_refs}\t{expanded_jalr_forwarded_raw_refs}\t{expanded_jalr_forwarded_constructor}\t{expanded_jalr_unclassified_sites}\t{expanded_jalr_unclassified_runtime}\t{expanded_jalr_fresh_arg5_sites}\t{expanded_jalr_fresh_arg5_runtime}\t{expanded_jalr_constructor_targets}\t{exe_arg5_sites}\t{arg5_call}\t{arg5_store}\t{indirect_table}\t{table_entries}\t{table_unique}\t{table_zero}\t{table_outside}\t{table_constructor}\t{table_sha}\t{table_kind}\t{exe_records}\t{sequence_records}\t{drac}\tfifth-arg-to-single-synthetic-op21-and-object-stream-static\tinitialized-jalr-targets-exclude-constructor-runtime-source-and-arg5-unresolved\truntime-unverified",
            schema = SUBGAME_SEQ_TEXT_BRIDGE_SCHEMA_VERSION,
            exe = row.exe,
            archive = row.archive,
            constructor = row.constructor_runtime,
            synthetic = row.synthetic_record_runtime,
            opcode = SUBGAME_SEQ_TEXT_BRIDGE_OPCODE,
            mode = SUBGAME_SEQ_TEXT_BRIDGE_MODE,
            dispatcher = row.dispatcher_runtime,
            dispatcher_table = row.dispatcher_table_runtime,
            op21 = row.op21_handler_runtime,
            mode2 = row.op21_mode2_handler_runtime,
            consumer = row.object_consumer_runtime,
            source = row.object_consumer_source_runtime,
            flow = row.exe_direct_flow_refs,
            address = row.exe_address_load_refs,
            raw = row.exe_raw_pointer_refs,
            files = row.sequence_files_scanned,
            jal = row.sequence_direct_jal_refs,
            jump = row.sequence_direct_jump_refs,
            sequence_address = row.sequence_address_load_refs,
            sequence_raw = row.sequence_raw_pointer_refs,
            reached = row.constructor_entry_reached,
            indirect = row.entry_indirect_sites,
            jr_bios_sites = row.entry_jr_bios_sites.len(),
            jr_bios_runtime = join_runtime(row.entry_jr_bios_sites.iter().copied()),
            jr_table_sites = row.entry_jr_table_sources.len(),
            jr_table_runtime = join_runtime(row.entry_jr_table_sources.iter().map(|source| source.call_runtime)),
            jr_table_source = join_runtime(row.entry_jr_table_sources.iter().map(|source| source.table_runtime)),
            jr_table_entries = join_usize(row.entry_jr_table_sources.iter().map(|source| source.table_entries)),
            jr_table_constructor = join_usize(row.entry_jr_table_sources.iter().map(|source| source.constructor_targets)),
            expanded_iterations = row.expanded_trace.iterations,
            expanded_seeds = row.expanded_trace.table_target_seeds,
            expanded_outside = row.expanded_trace.table_targets_outside_exe,
            expanded_instructions = row.expanded_trace.reached_instructions,
            expanded_indirect = row.expanded_trace.indirect_sites,
            expanded_jalr = row.expanded_trace.jalr_sites,
            expanded_jr_bios = row.expanded_trace.jr_bios_sites,
            expanded_jr_tables = row.expanded_trace.jr_table_sites,
            expanded_jr_unclassified = row.expanded_trace.jr_unclassified_sites.len(),
            expanded_jr_unclassified_runtime = join_runtime(row.expanded_trace.jr_unclassified_sites.iter().copied()),
            expanded_constructor = row.expanded_trace.constructor_reached,
            expanded_jalr_bios_sites = row.expanded_trace.jalr_bios_vector_sites.len(),
            expanded_jalr_bios_runtime = join_runtime(row.expanded_trace.jalr_bios_vector_sites.iter().copied()),
            expanded_jalr_direct_sites = row.expanded_trace.jalr_initialized_direct_sources.iter().map(|source| source.call_sites.len()).sum::<usize>(),
            expanded_jalr_direct_sources = join_runtime(row.expanded_trace.jalr_initialized_direct_sources.iter().map(|source| source.source_runtime)),
            expanded_jalr_direct_targets = join_runtime(row.expanded_trace.jalr_initialized_direct_sources.iter().map(|source| source.initialized_target)),
            expanded_jalr_rooted_sites = row.expanded_trace.jalr_initialized_rooted_sources.iter().map(|source| source.call_sites.len()).sum::<usize>(),
            expanded_jalr_rooted_sources = join_runtime(row.expanded_trace.jalr_initialized_rooted_sources.iter().map(|source| source.source_runtime)),
            expanded_jalr_rooted_targets = join_runtime(row.expanded_trace.jalr_initialized_rooted_sources.iter().map(|source| source.initialized_target)),
            expanded_jalr_indexed_sites = row.expanded_trace.jalr_initialized_indexed_sources.len(),
            expanded_jalr_indexed_calls = join_runtime(row.expanded_trace.jalr_initialized_indexed_sources.iter().map(|source| source.call_runtime)),
            expanded_jalr_indexed_tables = join_runtime(row.expanded_trace.jalr_initialized_indexed_sources.iter().map(|source| source.table_runtime)),
            expanded_jalr_indexed_entries = join_usize(row.expanded_trace.jalr_initialized_indexed_sources.iter().map(|source| source.table_entries)),
            expanded_jalr_indexed_unique = join_usize(row.expanded_trace.jalr_initialized_indexed_sources.iter().map(|source| source.unique_targets)),
            expanded_jalr_indexed_zero = join_usize(row.expanded_trace.jalr_initialized_indexed_sources.iter().map(|source| source.zero_targets)),
            expanded_jalr_indexed_outside = join_usize(row.expanded_trace.jalr_initialized_indexed_sources.iter().map(|source| source.outside_exe_targets)),
            expanded_jalr_indexed_constructor = join_usize(row.expanded_trace.jalr_initialized_indexed_sources.iter().map(|source| source.constructor_targets)),
            expanded_jalr_indexed_sha = row.expanded_trace.jalr_initialized_indexed_sources.iter().map(|source| hex(&source.sha256)).collect::<Vec<_>>().join(";"),
            expanded_jalr_outside_sites = row.expanded_trace.jalr_outside_text_sources.iter().map(|source| source.call_sites.len()).sum::<usize>(),
            expanded_jalr_outside_sources = join_runtime(row.expanded_trace.jalr_outside_text_sources.iter().map(|source| source.source_runtime)),
            expanded_jalr_outside_calls = join_runtime(row.expanded_trace.jalr_outside_text_sources.iter().flat_map(|source| source.call_sites.iter().copied())),
            expanded_jalr_outside_reads = join_runtime(row.expanded_trace.jalr_outside_text_sources.iter().flat_map(|source| source.direct_read_sites.iter().copied())),
            expanded_jalr_outside_writes = join_runtime(row.expanded_trace.jalr_outside_text_sources.iter().flat_map(|source| source.direct_write_sites.iter().copied())),
            expanded_jalr_outside_address_values = join_runtime(row.expanded_trace.jalr_outside_text_sources.iter().flat_map(|source| source.address_value_sites.iter().copied())),
            expanded_jalr_computed_sites = row.expanded_trace.jalr_computed_memory_sources.len(),
            expanded_jalr_computed_runtime = join_runtime(row.expanded_trace.jalr_computed_memory_sources.iter().map(|source| source.call_runtime)),
            expanded_jalr_computed_load = join_runtime(row.expanded_trace.jalr_computed_memory_sources.iter().map(|source| source.load_runtime)),
            expanded_jalr_computed_target = join_reg(row.expanded_trace.jalr_computed_memory_sources.iter().map(|source| source.target_reg)),
            expanded_jalr_computed_base = join_reg(row.expanded_trace.jalr_computed_memory_sources.iter().map(|source| source.base_reg)),
            expanded_jalr_computed_offset = join_i16_hex(row.expanded_trace.jalr_computed_memory_sources.iter().map(|source| source.field_offset)),
            expanded_jalr_forwarded_sites = row.expanded_trace.jalr_forwarded_register_sites.len(),
            expanded_jalr_forwarded_runtime = join_runtime(row.expanded_trace.jalr_forwarded_register_sites.iter().copied()),
            expanded_jalr_forwarded_capture = join_runtime(row.expanded_trace.jalr_forwarded_register_sources.iter().map(|source| source.capture_runtime)),
            expanded_jalr_forwarded_entry = join_runtime(row.expanded_trace.jalr_forwarded_register_sources.iter().map(|source| source.function_entry_runtime)),
            expanded_jalr_forwarded_argument = join_reg(row.expanded_trace.jalr_forwarded_register_sources.iter().map(|source| source.source_argument_reg)),
            expanded_jalr_forwarded_callers = join_usize(row.expanded_trace.jalr_forwarded_register_sources.iter().map(|source| source.direct_call_sites.len())),
            expanded_jalr_forwarded_calls = join_runtime(row.expanded_trace.jalr_forwarded_register_sources.iter().flat_map(|source| source.direct_call_sites.iter().copied())),
            expanded_jalr_forwarded_targets = join_runtime(row.expanded_trace.jalr_forwarded_register_sources.iter().flat_map(|source| source.direct_call_targets.iter().copied())),
            expanded_jalr_forwarded_address_refs = join_usize(row.expanded_trace.jalr_forwarded_register_sources.iter().map(|source| source.entry_address_load_refs)),
            expanded_jalr_forwarded_raw_refs = join_usize(row.expanded_trace.jalr_forwarded_register_sources.iter().map(|source| source.entry_raw_pointer_refs)),
            expanded_jalr_forwarded_constructor = join_usize(row.expanded_trace.jalr_forwarded_register_sources.iter().map(|source| source.constructor_targets)),
            expanded_jalr_unclassified_sites = row.expanded_trace.jalr_unclassified_sites.len(),
            expanded_jalr_unclassified_runtime = join_runtime(row.expanded_trace.jalr_unclassified_sites.iter().copied()),
            expanded_jalr_fresh_arg5_sites = row.expanded_trace.jalr_fresh_stack_arg5_sites.len(),
            expanded_jalr_fresh_arg5_runtime = join_runtime(row.expanded_trace.jalr_fresh_stack_arg5_sites.iter().copied()),
            expanded_jalr_constructor_targets = row.expanded_trace.jalr_initialized_direct_sources.iter().chain(&row.expanded_trace.jalr_initialized_rooted_sources).filter(|source| normalize_kseg(source.initialized_target) == normalize_kseg(row.constructor_runtime)).count(),
            exe_arg5_sites = row.exe_stack_arg5_jalr_sites,
            arg5_call = join_runtime(row.stack_arg5_jalr.iter().map(|candidate| candidate.call_runtime)),
            arg5_store = join_runtime(row.stack_arg5_jalr.iter().map(|candidate| candidate.argument_store_runtime)),
            indirect_table = join_runtime(row.stack_arg5_jalr.iter().map(|candidate| candidate.table_runtime)),
            table_entries = join_usize(row.stack_arg5_jalr.iter().map(|candidate| candidate.table_entries)),
            table_unique = join_usize(row.stack_arg5_jalr.iter().map(|candidate| candidate.table_unique_targets)),
            table_zero = join_usize(row.stack_arg5_jalr.iter().map(|candidate| candidate.table_zero_targets)),
            table_outside = join_usize(row.stack_arg5_jalr.iter().map(|candidate| candidate.table_outside_exe_targets)),
            table_constructor = join_usize(row.indirect_table_constructor_targets.iter().copied()),
            table_sha = row.stack_arg5_jalr.iter().map(|candidate| hex(&candidate.table_sha256)).collect::<Vec<_>>().join(";"),
            table_kind = row.stack_arg5_jalr.iter().map(|candidate| candidate.table_kind).collect::<Vec<_>>().join(";"),
            exe_records = row.exe_canonical_mode2_records,
            sequence_records = row.sequence_canonical_mode2_records,
            drac = row.drac_relation,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn join_runtime(values: impl IntoIterator<Item = u32>) -> String {
    values
        .into_iter()
        .map(|runtime| format!("0x{runtime:08X}"))
        .collect::<Vec<_>>()
        .join(";")
}

fn join_usize(values: impl IntoIterator<Item = usize>) -> String {
    values
        .into_iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(";")
}

fn join_reg(values: impl IntoIterator<Item = Reg>) -> String {
    values
        .into_iter()
        .map(|register| register.name())
        .collect::<Vec<_>>()
        .join(";")
}

fn join_i16_hex(values: impl IntoIterator<Item = i16>) -> String {
    values
        .into_iter()
        .map(|value| format!("0x{:04X}", value as u16))
        .collect::<Vec<_>>()
        .join(";")
}
