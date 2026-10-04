//! E6 glyph 전략 분석의 결정적 TSV 직렬화.

use std::fmt::Write as _;

use super::{
    SubgameSeqE6GlyphAnalysis, SubgameSeqE6StorageLink, SUBGAME_SEQ_E6_GLYPH_SCHEMA_VERSION,
};

fn hex_addresses(addresses: &[u32]) -> String {
    addresses
        .iter()
        .map(|address| format!("0x{address:08X}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn hex_u16_values(values: &[u16]) -> String {
    values
        .iter()
        .map(|value| format!("0x{value:04X}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn hex_u8_values(values: &[u8]) -> String {
    values
        .iter()
        .map(|value| format!("0x{value:02X}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn e6_sequence_storage_links(values: &[SubgameSeqE6StorageLink]) -> String {
    values
        .iter()
        .map(|value| {
            format!(
                "0x{:04X}:{}:{}:{}:{}:0x{:08X}",
                value.resource,
                value.vdd_index,
                value.sequence,
                value.stored_bytes,
                value.plain_bytes,
                value.load_base
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn decimal_indices(indices: &[usize]) -> String {
    indices
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn optional_hex(value: Option<u32>) -> String {
    value.map_or_else(String::new, |value| format!("0x{value:08X}"))
}

pub fn render_subgame_seq_e6_glyphs_tsv(analysis: &SubgameSeqE6GlyphAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tscope\texe\tarchive\tglyph_resource\tglyph_asset\t\
         descriptor_runtime\tglyph_plain_bytes\tglyph_plain_sha256\tphysical_glyph_cells\t\
         e6_sequences\te6_sequence_resource_ids\te6_sequence_selection_records\t\
         e6_sequence_selected_resources\te6_sequence_selection_sources\t\
         e6_sequence_selection_flags\te6_sequence_selection_sites\t\
         e6_sequence_vdd_entries\te6_sequence_descriptor_rows\t\
         e6_sequence_cnx_payloads\te6_sequence_overlay_rows\t\
         e6_sequence_stored_bytes\te6_sequence_plain_bytes\t\
         e6_sequence_load_bases\te6_sequence_storage_links\t\
         e6_sequence_storage_verdict\t\
         glyph_tokens\tused_cell_indices\tmin_cell_index\tmax_cell_index\t\
         all_glyph_setup_records\te6_glyph_setup_records\te6_table_copies\t\
         verified_e6_cells\tverified_e6_nonempty_cells\te6_table_offset\te6_table_sha256\t\
         non_e6_glyph_setup_records\trejected_resource\trejected_asset\t\
         rejected_setup_records\trejected_table_offset\trejected_table_sha256\t\
         candidate_sequence\tcandidate_setup_offset\tcandidate_table_offset\t\
         candidate_valid_prefix\tused_indices_above_candidate\t\
         candidate_used_valid_cells\tcandidate_used_invalid_cells\t\
         candidate_used_nonempty_cells\tcandidate_used_empty_cells\t\
         candidate_invalid_used_indices\tcandidate_empty_used_indices\t\
         paired_cell_count_writers\tliteral_halfword_20_stores\t\
         descriptor_literal_halfword_20_stores\tgpu_primitive_halfword_20_stores\t\
         cache_record_halfword_20_stores\tunclassified_literal_halfword_20_stores\t\
         literal_halfword_20_verdict\tliteral_resource_006d_loads\t\
         direct_chr_object_resource_stores\tfixed_cell_chr_object_routes\t\
         variable_cell_chr_object_routes\tnon_object_literal_006d_loads\t\
         literal_resource_006d_verdict\tallocator_direct_calls\t\
         allocator_immediate_type_calls\tallocator_dynamic_type_calls\t\
         immediate_type9_calls\tdirect_chr_object_allocator_calls\t\
         inactive_type9_calls\timmediate_type9_verdict\t\
         descriptor_root_literal_combines\tdescriptor_root_resource_x36_selectors\t\
         descriptor_root_alternate_branch_combines\tdescriptor_root_registration_stores\t\
         unclassified_descriptor_root_combines\tchr_descriptor_direct_address_loads\t\
         chr_descriptor_raw_pointer_words\tdescriptor_table_records\t\
         descriptor_table_sha256\tdescriptor_table_address_loads\t\
         descriptor_table_preadjusted_address_loads\t\
         descriptor_table_raw_pointer_locations\t\
         descriptor_table_preadjusted_raw_pointers\tdescriptor_root_census_verdict\t\
         descriptor_selector_direct_callee_calls\tdescriptor_selector_local_consumers\t\
         descriptor_selector_unique_callees\tdescriptor_forwarding_calls\t\
         descriptor_forwarding_unique_callees\tdescriptor_synchronous_field_reads\t\
         descriptor_synchronous_field_writes\tdescriptor_pointer_escape_stores\t\
         descriptor_cache_link_stores\tdescriptor_fixed_cache_callbacks\t\
         descriptor_fixed_callback_writes\tdescriptor_fixed_callback_cell_reads\t\
         descriptor_fixed_callback_cell_writes\tdescriptor_async_dispatch_targets\t\
         descriptor_async_dispatch_incoming_uses\tdescriptor_singleton_callback_writes\t\
         descriptor_cache_callbacks\tdescriptor_callback_copy_loops\t\
         descriptor_completion_callback_field_writes\t\
         descriptor_completion_descriptor_writes\t\
         descriptor_completion_callback_cell_reads\t\
         descriptor_completion_callback_cell_writes\t\
         descriptor_type_cache_root_combines\t\
         descriptor_singleton_snapshot_root_combines\t\
         descriptor_type_snapshot_root_combines\tdescriptor_payload_copy_loops\t\
         descriptor_snapshot_field_writes\tdescriptor_type_snapshot_link_stores\t\
         descriptor_snapshot_raw_pointer_words\tdescriptor_alias_census_verdict\t\
         setup_source_cell_writer_runtime\t\
         cache_mirror_cell_writer_runtime\tstatic_initializer_runtime\t\
         static_initializer_cell_writer_runtime\tstatic_initializer_direct_call_sites\t\
         static_initializer_entry_reached_call_sites\tstatic_initializer_data_refs\t\
         static_initializer_raw_pointer_words\tstatic_initializer_resource\t\
         static_initializer_descriptor_runtime\tstatic_initializer_registered_assets\t\
         static_initializer_cell_table_runtime\tstatic_initializer_cell_count\t\
         static_initializer_cell_width\tstatic_initializer_cell_height\t\
         alternate_static_route_verdict\tglyph_object_allocator_runtime\t\
         glyph_object_allocator_type\tglyph_object_callback_runtime\t\
         object_pool_init_runtime\tobject_release_runtime\tobject_type_flags_runtime\t\
         object_list_runtime\trender_dispatcher_runtime\t\
         render_dispatcher_short_call_runtime\trender_dispatcher_entry_reached\t\
         renderer_route_verdict\tstatic_verdict\n",
    );
    let header_columns = output
        .lines()
        .next()
        .expect("TSV header가 있음")
        .split('\t')
        .count();
    for row in &analysis.rows {
        let row_start = output.len();
        write!(
            output,
            "{}\t{}\t{}\t{}\t0x{:04X}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
             {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t",
            SUBGAME_SEQ_E6_GLYPH_SCHEMA_VERSION,
            row.scope,
            row.exe,
            row.archive,
            row.glyph_resource,
            row.glyph_asset,
            row.descriptor_runtime,
            row.glyph_plain_bytes,
            row.glyph_plain_sha256,
            row.physical_glyph_cells,
            row.e6_sequences,
            hex_u16_values(&row.e6_sequence_resource_ids),
            row.e6_sequence_selection_records,
            row.e6_sequence_selected_resources,
            row.e6_sequence_selection_sources.join(","),
            hex_u8_values(&row.e6_sequence_selection_flags),
            row.e6_sequence_selection_sites.join(","),
            format_args!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                row.e6_sequence_vdd_entries,
                row.e6_sequence_descriptor_rows,
                row.e6_sequence_cnx_payloads,
                row.e6_sequence_overlay_rows,
                row.e6_sequence_stored_bytes,
                row.e6_sequence_plain_bytes,
                hex_addresses(&row.e6_sequence_load_bases),
                e6_sequence_storage_links(&row.e6_sequence_storage_links),
                row.e6_sequence_storage_verdict,
            ),
            row.glyph_tokens,
            row.used_cell_indices,
            row.min_cell_index,
            row.max_cell_index,
            row.all_glyph_setup_records,
            row.e6_glyph_setup_records,
            row.e6_table_copies,
            row.verified_e6_cells,
            row.verified_e6_nonempty_cells,
            row.e6_table_offset
                .map_or_else(String::new, |value| format!("0x{value:04X}")),
            row.e6_table_sha256.as_deref().unwrap_or(""),
            row.non_e6_glyph_setup_records,
            row.rejected_resource
                .map_or_else(String::new, |value| format!("0x{value:04X}")),
            row.rejected_asset.unwrap_or(""),
            row.rejected_setup_records,
            row.rejected_table_offset
                .map_or_else(String::new, |value| format!("0x{value:04X}")),
            row.rejected_table_sha256.as_deref().unwrap_or(""),
            row.candidate_sequence.unwrap_or(""),
            row.candidate_setup_offset
                .map_or_else(String::new, |value| format!("0x{value:04X}")),
            row.candidate_table_offset
                .map_or_else(String::new, |value| format!("0x{value:04X}")),
            row.candidate_valid_prefix,
            row.used_indices_above_candidate,
        )
        .expect("String write는 실패하지 않음");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            row.candidate_used_valid_cells,
            row.candidate_used_invalid_cells,
            row.candidate_used_nonempty_cells,
            row.candidate_used_empty_cells,
            decimal_indices(&row.candidate_invalid_used_indices),
            decimal_indices(&row.candidate_empty_used_indices),
            row.paired_cell_count_writers,
            hex_addresses(&row.literal_halfword_20_stores),
            hex_addresses(&row.descriptor_literal_halfword_20_stores),
            hex_addresses(&row.gpu_primitive_halfword_20_stores),
            hex_addresses(&row.cache_record_halfword_20_stores),
            row.unclassified_literal_halfword_20_stores,
            row.literal_halfword_20_verdict,
            hex_addresses(&row.literal_resource_006d_loads),
            hex_addresses(&row.direct_chr_object_resource_stores),
            row.fixed_cell_chr_object_routes,
            row.variable_cell_chr_object_routes,
            hex_addresses(&row.non_object_literal_006d_loads),
            row.literal_resource_006d_verdict,
            row.allocator_direct_calls,
            row.allocator_immediate_type_calls,
            row.allocator_dynamic_type_calls,
            hex_addresses(&row.immediate_type9_calls),
            hex_addresses(&row.direct_chr_object_allocator_calls),
            hex_addresses(&row.inactive_type9_calls),
            row.immediate_type9_verdict,
            hex_addresses(&row.descriptor_root_literal_combines),
            hex_addresses(&row.descriptor_root_resource_x36_selectors),
            hex_addresses(&row.descriptor_root_alternate_branch_combines),
            hex_addresses(&row.descriptor_root_registration_stores),
            row.unclassified_descriptor_root_combines,
            row.chr_descriptor_direct_address_loads,
            row.chr_descriptor_raw_pointer_words,
            format_args!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                row.descriptor_table_records,
                row.descriptor_table_sha256.as_deref().unwrap_or(""),
                hex_addresses(&row.descriptor_table_address_loads),
                row.descriptor_table_preadjusted_address_loads,
                hex_addresses(&row.descriptor_table_raw_pointer_locations),
                row.descriptor_table_preadjusted_raw_pointers,
                row.descriptor_root_census_verdict,
            ),
            hex_addresses(&row.descriptor_selector_direct_callee_calls),
            hex_addresses(&row.descriptor_selector_local_consumers),
            format_args!(
                "{}\t{}\t{}\t{}\t{}",
                hex_addresses(&row.descriptor_selector_unique_callees),
                hex_addresses(&row.descriptor_forwarding_calls),
                hex_addresses(&row.descriptor_forwarding_unique_callees),
                hex_addresses(&row.descriptor_synchronous_field_reads),
                hex_addresses(&row.descriptor_synchronous_field_writes),
            ),
            hex_addresses(&row.descriptor_pointer_escape_stores),
            hex_addresses(&row.descriptor_cache_link_stores),
            hex_addresses(&row.descriptor_fixed_cache_callbacks),
            hex_addresses(&row.descriptor_fixed_callback_writes),
            hex_addresses(&row.descriptor_fixed_callback_cell_reads),
            row.descriptor_fixed_callback_cell_writes,
            row.descriptor_async_dispatch_targets,
            row.descriptor_async_dispatch_incoming_uses,
            format_args!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                hex_addresses(&row.descriptor_singleton_callback_writes),
                hex_addresses(&row.descriptor_cache_callbacks),
                hex_addresses(&row.descriptor_callback_copy_loops),
                hex_addresses(&row.descriptor_completion_callback_field_writes),
                hex_addresses(&row.descriptor_completion_descriptor_writes),
                row.descriptor_completion_callback_cell_reads,
                row.descriptor_completion_callback_cell_writes,
                hex_addresses(&row.descriptor_type_cache_root_combines),
                hex_addresses(&row.descriptor_singleton_snapshot_root_combines),
                hex_addresses(&row.descriptor_type_snapshot_root_combines),
                hex_addresses(&row.descriptor_payload_copy_loops),
                hex_addresses(&row.descriptor_snapshot_field_writes),
                hex_addresses(&row.descriptor_type_snapshot_link_stores),
                row.descriptor_snapshot_raw_pointer_words,
                row.descriptor_alias_census_verdict,
            ),
            optional_hex(row.setup_source_cell_writer_runtime),
            optional_hex(row.cache_mirror_cell_writer_runtime),
            optional_hex(row.static_initializer_runtime),
            optional_hex(row.static_initializer_cell_writer_runtime),
            hex_addresses(&row.static_initializer_direct_call_sites),
            hex_addresses(&row.static_initializer_entry_reached_call_sites),
            row.static_initializer_data_refs,
            row.static_initializer_raw_pointer_words,
            row.static_initializer_resource
                .map_or_else(String::new, |value| format!("0x{value:04X}")),
            optional_hex(row.static_initializer_descriptor_runtime),
            row.static_initializer_registered_assets,
            optional_hex(row.static_initializer_cell_table_runtime),
            row.static_initializer_cell_count
                .map_or_else(String::new, |value| value.to_string()),
            row.static_initializer_cell_width
                .map_or_else(String::new, |value| value.to_string()),
            row.static_initializer_cell_height
                .map_or_else(String::new, |value| value.to_string()),
            row.alternate_static_route_verdict,
            format_args!("0x{:08X}", row.glyph_object_allocator_runtime),
            row.glyph_object_allocator_type,
            format_args!("0x{:08X}", row.glyph_object_callback_runtime),
            format_args!("0x{:08X}", row.object_pool_init_runtime),
            format_args!("0x{:08X}", row.object_release_runtime),
            format_args!("0x{:08X}", row.object_type_flags_runtime),
            format_args!("0x{:08X}", row.object_list_runtime),
            format_args!("0x{:08X}", row.render_dispatcher_runtime),
            format_args!("0x{:08X}", row.render_dispatcher_short_call_runtime),
            row.render_dispatcher_entry_reached,
            row.renderer_route_verdict,
            row.static_verdict,
        )
        .expect("String write는 실패하지 않음");
        let row_columns = output[row_start..]
            .trim_end_matches('\n')
            .split('\t')
            .count();
        assert_eq!(
            row_columns, header_columns,
            "E6 glyph TSV column 수 불일치: scope={}, header={header_columns}, row={row_columns}",
            row.scope
        );
    }
    output
}
