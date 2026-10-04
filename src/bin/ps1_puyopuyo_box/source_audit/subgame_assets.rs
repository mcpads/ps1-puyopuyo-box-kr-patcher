//! 서브게임 descriptor, load, E6 글리프와 번역 계획을 검증한다.

use ps1_puyopuyo_box::disasm::{
    verify_box_btx_descriptor_selections, verify_box_btx_name_consumers,
    verify_box_common_font_cell_sources, verify_box_common_font_fixed_draws,
    verify_box_subgame_seq_descriptors, verify_box_subgame_seq_e6_glyphs,
    verify_box_subgame_seq_e6_text, verify_box_subgame_seq_loads,
    verify_box_subgame_seq_registrations, verify_box_subgame_seq_selections, BtxCellAnalysis,
    CommonFontDrawSiteAnalysis, BOX_SUBGAME_SEQ_DESCRIPTOR_SPECS, BOX_SUBGAME_SEQ_E6_GLYPH_SPECS,
    BOX_SUBGAME_SEQ_E6_TEXT_SPECS, BOX_SUBGAME_SEQ_LOAD_SPECS, BOX_SUBGAME_SEQ_REGISTRATION_SPECS,
    BOX_SUBGAME_SEQ_SELECTION_DISPATCHERS, DEMO_FONT_CODE_TABLE_ENTRIES,
    DEMO_FONT_CODE_TABLE_RUNTIME,
};
use ps1_puyopuyo_box::font::{verify_box_common_font_text_surfaces, verify_box_common_fonts};
use ps1_puyopuyo_box::graphics::e6_demo_font_patch::audit_box_puyo2_e6_demo_font_geometry;
use ps1_puyopuyo_box::pipeline::{
    plan_box_puyo2_subgame_seq_e6_translation, review_box_puyo2_e6_translations,
    verify_box_puyo2_e6_translation_manifest, verify_box_puyo2_subgame_seq_e6_translation_plan,
};

use super::{ArchiveAuditData, SourceAuditContext, SubgameAssetAuditData, SubgameTextAuditData};

pub(super) fn audit_subgame_assets(
    context: &SourceAuditContext<'_>,
    archives: &ArchiveAuditData,
    text: &SubgameTextAuditData,
    btx_cells: &BtxCellAnalysis,
    common_font_draw_sites: &CommonFontDrawSiteAnalysis,
) -> Result<SubgameAssetAuditData, String> {
    let box_executables = &context.box_executables;
    let cnx_archives = archives.archive_copies();
    let subgame_seq_sources = &text.sources;
    let subgame_seq_layouts = &text.layouts;
    let subgame_seq_text_roots = &text.text_roots;
    let subgame_seq_descriptors = verify_box_subgame_seq_descriptors(
        box_executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_DESCRIPTOR_SPECS
                    .iter()
                    .any(|spec| spec.exe == exe.map.name)
            })
            .map(|exe| (exe.map, exe.text.as_slice())),
        cnx_archives
            .iter()
            .filter(|(archive, _)| {
                BOX_SUBGAME_SEQ_DESCRIPTOR_SPECS
                    .iter()
                    .any(|spec| spec.archive == *archive)
            })
            .map(|(archive, vdd)| (*archive, vdd)),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame SEQ descriptors: selectors {}, representative source rows {}, \
         descriptor stride 36B / cache field +0x14",
        subgame_seq_descriptors.rows.len(),
        subgame_seq_descriptors.rows.len(),
    );
    let subgame_seq_registrations = verify_box_subgame_seq_registrations(
        box_executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_REGISTRATION_SPECS
                    .iter()
                    .any(|spec| spec.exe == exe.map.name)
            })
            .map(|exe| (exe.map, exe.text.as_slice())),
        &subgame_seq_descriptors,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame SEQ registrations: type 1 records {}, VDD loaders {}, \
         representative resource IDs {:?}",
        subgame_seq_registrations.rows.len(),
        subgame_seq_registrations.rows.len(),
        subgame_seq_registrations
            .rows
            .iter()
            .map(|row| row.resource_id)
            .collect::<Vec<_>>(),
    );
    let subgame_seq_selections = verify_box_subgame_seq_selections(
        box_executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_SELECTION_DISPATCHERS
                    .iter()
                    .any(|spec| spec.exe == exe.map.name)
            })
            .map(|exe| (exe.map, exe.text.as_slice())),
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &subgame_seq_registrations,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame SEQ selections: scene records {}, opcode tables {} × 32, \
         resource 46 -> DRAC descriptors {}",
        subgame_seq_selections.rows.len(),
        BOX_SUBGAME_SEQ_SELECTION_DISPATCHERS.len(),
        subgame_seq_selections.rows.len(),
    );
    let subgame_seq_loads = verify_box_subgame_seq_loads(
        box_executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_LOAD_SPECS
                    .iter()
                    .any(|spec| spec.exe == exe.map.name)
            })
            .map(|exe| (exe.map, exe.text.as_slice())),
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &subgame_seq_selections,
        subgame_seq_layouts,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame SEQ loads: records {}, flags 0x97, CNX output/overlay 0x801B5270, \
         descriptor destinations {}",
        subgame_seq_loads.rows.len(),
        subgame_seq_loads.rows.len(),
    );
    let common_fonts = verify_box_common_fonts(
        cnx_archives
            .iter()
            .filter(|(source_id, _)| source_id.ends_with("GRP.VDD;1"))
            .map(|(source_id, archive)| (*source_id, archive)),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS COM_FONT physical variants: copies {}, unique contents {}, \
         256x80 {}, 256x128 {}, total nonzero pixels {}",
        common_fonts.variants,
        common_fonts.unique_contents,
        common_fonts.short_variants,
        common_fonts.tall_variants,
        common_fonts.total_nonzero_pixels,
    );
    let common_font_text = verify_box_common_font_text_surfaces(
        cnx_archives
            .iter()
            .filter(|(source_id, _)| source_id.ends_with("GRP.VDD;1"))
            .map(|(source_id, archive)| (*source_id, archive)),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS COM_FONT Japanese text surfaces: physical {}, archives {}, source texts {}, \
         pixel surfaces {}, representative runtime cell candidates {}, runtime draw verified false",
        common_font_text.physical_surfaces,
        common_font_text.physical_archives,
        common_font_text.unique_source_texts,
        common_font_text.unique_pixel_surfaces,
        common_font_text.representative_runtime_cell_bindings,
    );
    let btx_name_consumers = verify_box_btx_name_consumers(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        cnx_archives
            .iter()
            .filter(|(source_id, _)| source_id.ends_with("GRP.VDD;1"))
            .map(|(source_id, archive)| (*source_id, archive)),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS BTX name consumers: EXE/GRP pairs {}, physical BTX {}, literal names {}, \
         exact static refs {}, exact entry refs {}, table refs {}, table entry refs {}, \
         descriptor pointers {}, descriptor refs {}, descriptor entry refs {}, analysis SHA-256 {}",
        btx_name_consumers.summaries.len(),
        btx_name_consumers.links.len(),
        btx_name_consumers
            .summaries
            .iter()
            .map(|summary| summary.literal_names)
            .sum::<usize>(),
        btx_name_consumers
            .summaries
            .iter()
            .map(|summary| summary.statically_referenced_assets)
            .sum::<usize>(),
        btx_name_consumers
            .summaries
            .iter()
            .map(|summary| summary.entry_referenced_assets)
            .sum::<usize>(),
        btx_name_consumers
            .summaries
            .iter()
            .map(|summary| summary.table_static_ref_sites.len())
            .sum::<usize>(),
        btx_name_consumers
            .summaries
            .iter()
            .map(|summary| summary.table_entry_reached_ref_sites.len())
            .sum::<usize>(),
        btx_name_consumers
            .summaries
            .iter()
            .map(|summary| summary.descriptor_pointers)
            .sum::<usize>(),
        btx_name_consumers
            .summaries
            .iter()
            .map(|summary| summary.descriptor_static_ref_sites.len())
            .sum::<usize>(),
        btx_name_consumers
            .summaries
            .iter()
            .map(|summary| summary.descriptor_entry_reached_ref_sites.len())
            .sum::<usize>(),
        btx_name_consumers
            .analysis_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );
    let btx_descriptor_selections = verify_box_btx_descriptor_selections(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let common_font_fixed_draws = verify_box_common_font_fixed_draws(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        &btx_name_consumers,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS fixed COM_FONT direct draws: scanned EXE {}, COM_FONT EXE {}, sites {}",
        common_font_fixed_draws.scanned_executables,
        common_font_fixed_draws.common_font_executables,
        common_font_fixed_draws.sites.len(),
    );
    let common_font_cell_sources = verify_box_common_font_cell_sources(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        &btx_name_consumers,
        common_font_draw_sites,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS COM_FONT Japanese cell sources: resolved/unresolved surfaces {}/{}, \
         literal writers {}, entry-reached {}, runtime draw verified false",
        common_font_cell_sources.resolved_surfaces,
        common_font_cell_sources.unresolved_surfaces,
        common_font_cell_sources.literal_writer_sites,
        common_font_cell_sources.entry_reached_writer_sites,
    );
    let subgame_seq_e6_text = verify_box_subgame_seq_e6_text(
        box_executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_E6_TEXT_SPECS
                    .iter()
                    .any(|spec| spec.exe == exe.map.name)
            })
            .map(|exe| (exe.map, exe.text.as_slice())),
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        subgame_seq_layouts,
        subgame_seq_text_roots,
        &btx_name_consumers,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame E6 direct text: consumers {}, streams {}, physical/logical sequences {}/{}, \
         spans/code units {}/{}, exact E6/E1 roots {}/{}, controls 70/72/77/65 \
         {}/{}/{}/{}, counterpart equal/different {}/{}, resources CHR.BTX/DEMOFONT.BTX, \
         runtime incomplete",
        subgame_seq_e6_text.consumers,
        subgame_seq_e6_text.rows.len(),
        subgame_seq_e6_text.physical_sequences,
        subgame_seq_e6_text.logical_sequences,
        subgame_seq_e6_text.spans,
        subgame_seq_e6_text.code_units,
        subgame_seq_e6_text.exact_e6_roots,
        subgame_seq_e6_text.exact_e1_roots,
        subgame_seq_e6_text.control_70,
        subgame_seq_e6_text.control_72,
        subgame_seq_e6_text.control_77,
        subgame_seq_e6_text.terminators,
        subgame_seq_e6_text.counterpart_equal,
        subgame_seq_e6_text.counterpart_different,
    );
    let puyo2_e6_executable = box_executables
        .iter()
        .find(|executable| executable.map.name == "PUYO2.EXE")
        .ok_or_else(|| "PUYO2 E6 translation plan EXE 누락".to_string())?;
    let code_table_offset = puyo2_e6_executable
        .map
        .text_offset(DEMO_FONT_CODE_TABLE_RUNTIME)
        .ok_or_else(|| "PUYO2 E6 code table이 EXE 밖임".to_string())?;
    let code_table_end = code_table_offset
        .checked_add(DEMO_FONT_CODE_TABLE_ENTRIES * 2)
        .ok_or_else(|| "PUYO2 E6 code table 끝 overflow".to_string())?;
    let code_table = puyo2_e6_executable
        .text
        .get(code_table_offset..code_table_end)
        .ok_or_else(|| "PUYO2 E6 code table 범위 이탈".to_string())?;
    let e6_translation_plan = verify_box_puyo2_subgame_seq_e6_translation_plan(
        code_table,
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &subgame_seq_e6_text,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS PUYO2 E6 synthetic translation plan: overlays {}, streams/pointers {}, \
         code slots reachable/retained/hangul/remaining {}/{}/{}/{}, \
         block capacity/used/remaining {}/{}/{}, report SHA-256 {}, runtime unverified",
        e6_translation_plan.rows.len(),
        e6_translation_plan.pointer_relations,
        e6_translation_plan.plan.reachable_slots,
        e6_translation_plan.plan.retained_source_characters,
        e6_translation_plan.plan.glyph_mappings.len(),
        e6_translation_plan.plan.remaining_slots,
        e6_translation_plan.plan.capacity_bytes,
        e6_translation_plan.plan.used_bytes,
        e6_translation_plan.plan.remaining_bytes,
        ps1_puyopuyo_box::pipeline::BOX_PUYO2_E6_TRANSLATION_PLAN_REPORT_SHA256,
    );
    let e6_translation_manifest = verify_box_puyo2_e6_translation_manifest(&subgame_seq_e6_text)
        .map_err(|error| error.to_string())?;
    let e6_review_translations = review_box_puyo2_e6_translations(&subgame_seq_e6_text)
        .map_err(|error| error.to_string())?;
    let e6_review_plan = plan_box_puyo2_subgame_seq_e6_translation(
        &e6_review_translations,
        code_table,
        &subgame_seq_e6_text,
    )
    .map_err(|error| error.to_string())?;
    let e6_font_geometry = audit_box_puyo2_e6_demo_font_geometry(&e6_review_plan.glyph_mappings)
        .map_err(|error| error.to_string())?;
    if e6_font_geometry.glyphs != e6_review_plan.glyph_mappings.len() {
        return Err(format!(
            "PUYO2 E6 DEMOFONT geometry glyph 분모 불일치: expected={}, actual={}",
            e6_review_plan.glyph_mappings.len(),
            e6_font_geometry.glyphs
        ));
    }
    println!(
        "PASS PUYO2 E6 DEMOFONT draft geometry: glyphs {}, normalized wide Hangul {}, \
         squat after normalization {}, foreground width {}..{} / height {}..{}; \
         translation/profile HITL unapproved",
        e6_font_geometry.glyphs,
        e6_font_geometry.normalized_wide_hangul,
        e6_font_geometry.squat_foreground_glyphs,
        e6_font_geometry.minimum_foreground_width,
        e6_font_geometry.maximum_foreground_width,
        e6_font_geometry.minimum_foreground_height,
        e6_font_geometry.maximum_foreground_height,
    );
    println!(
        "PASS PUYO2 E6 translation manifest and draft compile: sequences/streams/spans \
         {}/{}/{}, source/KO code units {}/{}, approved/draft {}/{}, \
         code slots retained/hangul/remaining {}/{}/{}, block capacity/used/remaining \
         {}/{}/{}, manifest SHA-256 {}, product eligible {}",
        e6_translation_manifest.sequences,
        e6_translation_manifest.streams,
        e6_translation_manifest.spans,
        e6_translation_manifest.protected_source_code_units,
        e6_translation_manifest.translated_code_units,
        e6_translation_manifest.approved,
        e6_translation_manifest.draft_needs_human_review,
        e6_review_plan.retained_source_characters,
        e6_review_plan.glyph_mappings.len(),
        e6_review_plan.remaining_slots,
        e6_review_plan.capacity_bytes,
        e6_review_plan.used_bytes,
        e6_review_plan.remaining_bytes,
        e6_translation_manifest.manifest_sha256,
        e6_translation_manifest.product_eligible,
    );
    let subgame_seq_e6_glyphs = verify_box_subgame_seq_e6_glyphs(
        box_executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_E6_GLYPH_SPECS
                    .iter()
                    .any(|spec| spec.exe == exe.map.name)
            })
            .map(|exe| (exe.map, exe.text.as_slice())),
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        cnx_archives
            .iter()
            .filter(|(source_id, _)| {
                BOX_SUBGAME_SEQ_E6_GLYPH_SPECS
                    .iter()
                    .any(|spec| spec.seq_archive == *source_id || spec.grp_archive == *source_id)
            })
            .map(|(source_id, archive)| (*source_id, archive)),
        subgame_seq_layouts,
        &subgame_seq_e6_text,
        &btx_name_consumers,
        btx_cells,
    )
    .map_err(|error| error.to_string())?;
    let p1_e6_glyphs = subgame_seq_e6_glyphs
        .rows
        .iter()
        .find(|row| row.scope == "PUYO1")
        .ok_or_else(|| "PUYO1 E6 glyph 전략 행이 없음".to_string())?;
    let p2_e6_glyphs = subgame_seq_e6_glyphs
        .rows
        .iter()
        .find(|row| row.scope == "PUYO2")
        .ok_or_else(|| "PUYO2 E6 glyph 전략 행이 없음".to_string())?;
    println!(
        "PASS subgame E6 glyph strategy: common indices {} ({}..={}), \
         sequence resources PUYO1/2 {}/{}, canonical selections {}/{}, \
         selected resources {}/{}, selection sources {}/{}, \
         PUYO2 {}/{} mapped/physical cells / {} exact overlay setups / static-ready, \
         PUYO1 candidate prefix {} / used above {}, used cells valid/invalid/nonempty/empty \
         {}/{}/{}/{}, paired writers {}, \
         literal +0x20 stores descriptor/GPU/cache/unclassified {}/{}/{}/{}, \
         literal resource 006D loads/object/fixed/variable/non-object {}/{}/{}/{}/{}, \
         allocator direct/immediate/dynamic/type9/CHR/inactive {}/{}/{}/{}/{}/{}, \
         descriptor root literal/x36/alternate/register/unclassified {}/{}/{}/{}/{}, \
         CHR descriptor direct/raw refs {}/{}, \
         descriptor span records/address/pre-address/raw/pre-raw {}/{}/{}/{}/{}, \
         descriptor selector callee/local/unique {}/{}/{}, \
         descriptor forwarding calls/targets/sync reads/writes {}/{}/{}/{}, \
         pointer escapes/cache links/fixed callbacks/writes/cell reads/cell writes \
         {}/{}/{}/{}/{}/{}, async states/incoming uses {}/{}, \
         callback singleton writes/values/copies/completion field writes/descriptor writes/\
         cell reads/cell writes/type roots {}/{}/{}/{}/{}/{}/{}/{}, \
         descriptor clone singleton/type roots/copies/mutations/links/raw {}/{}/{}/{}/{}/{}, \
         static initializer direct/entry calls {}/{}, resource {:?}, registered assets {}, \
         cell count {:?}, object short-renderer routes {}/{} / \
         direct route rejected / indirect unresolved / no-write",
        subgame_seq_e6_glyphs.common_used_cell_indices,
        subgame_seq_e6_glyphs.common_min_cell_index,
        subgame_seq_e6_glyphs.common_max_cell_index,
        p1_e6_glyphs.e6_sequence_resource_ids.len(),
        p2_e6_glyphs.e6_sequence_resource_ids.len(),
        p1_e6_glyphs.e6_sequence_selection_records,
        p2_e6_glyphs.e6_sequence_selection_records,
        p1_e6_glyphs.e6_sequence_selected_resources,
        p2_e6_glyphs.e6_sequence_selected_resources,
        p1_e6_glyphs.e6_sequence_selection_sources.len(),
        p2_e6_glyphs.e6_sequence_selection_sources.len(),
        p2_e6_glyphs.verified_e6_cells,
        p2_e6_glyphs.physical_glyph_cells,
        p2_e6_glyphs.e6_glyph_setup_records,
        p1_e6_glyphs.candidate_valid_prefix,
        p1_e6_glyphs.used_indices_above_candidate,
        p1_e6_glyphs.candidate_used_valid_cells,
        p1_e6_glyphs.candidate_used_invalid_cells,
        p1_e6_glyphs.candidate_used_nonempty_cells,
        p1_e6_glyphs.candidate_used_empty_cells,
        p1_e6_glyphs.paired_cell_count_writers,
        p1_e6_glyphs.descriptor_literal_halfword_20_stores.len(),
        p1_e6_glyphs.gpu_primitive_halfword_20_stores.len(),
        p1_e6_glyphs.cache_record_halfword_20_stores.len(),
        p1_e6_glyphs.unclassified_literal_halfword_20_stores,
        p1_e6_glyphs.literal_resource_006d_loads.len(),
        p1_e6_glyphs.direct_chr_object_resource_stores.len(),
        p1_e6_glyphs.fixed_cell_chr_object_routes,
        p1_e6_glyphs.variable_cell_chr_object_routes,
        p1_e6_glyphs.non_object_literal_006d_loads.len(),
        p1_e6_glyphs.allocator_direct_calls,
        p1_e6_glyphs.allocator_immediate_type_calls,
        p1_e6_glyphs.allocator_dynamic_type_calls,
        p1_e6_glyphs.immediate_type9_calls.len(),
        p1_e6_glyphs.direct_chr_object_allocator_calls.len(),
        p1_e6_glyphs.inactive_type9_calls.len(),
        p1_e6_glyphs.descriptor_root_literal_combines.len(),
        p1_e6_glyphs.descriptor_root_resource_x36_selectors.len(),
        p1_e6_glyphs.descriptor_root_alternate_branch_combines.len(),
        p1_e6_glyphs.descriptor_root_registration_stores.len(),
        p1_e6_glyphs.unclassified_descriptor_root_combines,
        p1_e6_glyphs.chr_descriptor_direct_address_loads,
        p1_e6_glyphs.chr_descriptor_raw_pointer_words,
        p1_e6_glyphs.descriptor_table_records,
        p1_e6_glyphs.descriptor_table_address_loads.len(),
        p1_e6_glyphs.descriptor_table_preadjusted_address_loads,
        p1_e6_glyphs.descriptor_table_raw_pointer_locations.len(),
        p1_e6_glyphs.descriptor_table_preadjusted_raw_pointers,
        p1_e6_glyphs.descriptor_selector_direct_callee_calls.len(),
        p1_e6_glyphs.descriptor_selector_local_consumers.len(),
        p1_e6_glyphs.descriptor_selector_unique_callees.len(),
        p1_e6_glyphs.descriptor_forwarding_calls.len(),
        p1_e6_glyphs.descriptor_forwarding_unique_callees.len(),
        p1_e6_glyphs.descriptor_synchronous_field_reads.len(),
        p1_e6_glyphs.descriptor_synchronous_field_writes.len(),
        p1_e6_glyphs.descriptor_pointer_escape_stores.len(),
        p1_e6_glyphs.descriptor_cache_link_stores.len(),
        p1_e6_glyphs.descriptor_fixed_cache_callbacks.len(),
        p1_e6_glyphs.descriptor_fixed_callback_writes.len(),
        p1_e6_glyphs.descriptor_fixed_callback_cell_reads.len(),
        p1_e6_glyphs.descriptor_fixed_callback_cell_writes,
        p1_e6_glyphs.descriptor_async_dispatch_targets,
        p1_e6_glyphs.descriptor_async_dispatch_incoming_uses,
        p1_e6_glyphs.descriptor_singleton_callback_writes.len(),
        p1_e6_glyphs.descriptor_cache_callbacks.len(),
        p1_e6_glyphs.descriptor_callback_copy_loops.len(),
        p1_e6_glyphs
            .descriptor_completion_callback_field_writes
            .len(),
        p1_e6_glyphs.descriptor_completion_descriptor_writes.len(),
        p1_e6_glyphs.descriptor_completion_callback_cell_reads,
        p1_e6_glyphs.descriptor_completion_callback_cell_writes,
        p1_e6_glyphs.descriptor_type_cache_root_combines.len(),
        p1_e6_glyphs
            .descriptor_singleton_snapshot_root_combines
            .len(),
        p1_e6_glyphs.descriptor_type_snapshot_root_combines.len(),
        p1_e6_glyphs.descriptor_payload_copy_loops.len(),
        p1_e6_glyphs.descriptor_snapshot_field_writes.len(),
        p1_e6_glyphs.descriptor_type_snapshot_link_stores.len(),
        p1_e6_glyphs.descriptor_snapshot_raw_pointer_words,
        p1_e6_glyphs.static_initializer_direct_call_sites.len(),
        p1_e6_glyphs
            .static_initializer_entry_reached_call_sites
            .len(),
        p1_e6_glyphs.static_initializer_resource,
        p1_e6_glyphs.static_initializer_registered_assets,
        p1_e6_glyphs.static_initializer_cell_count,
        subgame_seq_e6_glyphs.renderer_route_rows,
        subgame_seq_e6_glyphs.rows.len(),
    );
    println!(
        "PASS subgame E6 sequence storage: PUYO1/2 VDD-descriptor-CNX-overlay \
         {}/{}/{}/{} and {}/{}/{}/{}, stored/plain bytes {}/{} and {}/{}, \
         load bases {:08X?}/{:08X?}; PUYO1 {}, PUYO2 {}",
        p1_e6_glyphs.e6_sequence_vdd_entries,
        p1_e6_glyphs.e6_sequence_descriptor_rows,
        p1_e6_glyphs.e6_sequence_cnx_payloads,
        p1_e6_glyphs.e6_sequence_overlay_rows,
        p2_e6_glyphs.e6_sequence_vdd_entries,
        p2_e6_glyphs.e6_sequence_descriptor_rows,
        p2_e6_glyphs.e6_sequence_cnx_payloads,
        p2_e6_glyphs.e6_sequence_overlay_rows,
        p1_e6_glyphs.e6_sequence_stored_bytes,
        p1_e6_glyphs.e6_sequence_plain_bytes,
        p2_e6_glyphs.e6_sequence_stored_bytes,
        p2_e6_glyphs.e6_sequence_plain_bytes,
        p1_e6_glyphs.e6_sequence_load_bases,
        p2_e6_glyphs.e6_sequence_load_bases,
        p1_e6_glyphs.e6_sequence_storage_verdict,
        p2_e6_glyphs.e6_sequence_storage_verdict,
    );

    Ok(SubgameAssetAuditData {
        btx_names: btx_name_consumers,
        btx_descriptor_selections,
        e6_text: subgame_seq_e6_text,
        e6_translation: e6_translation_plan,
    })
}
