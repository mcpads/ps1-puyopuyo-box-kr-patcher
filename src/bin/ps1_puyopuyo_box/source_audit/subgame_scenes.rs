//! 서브게임 DRAC·scene glyph의 채택된 source 계약을 검증한다.

use std::collections::BTreeSet;

use sha2::Digest;

use ps1_puyopuyo_box::analysis::{
    verify_box_subgame_seq_scene_glyphs, verify_box_subgame_seq_scene_messages,
    BOX_SUBGAME_SEQ_SCENE_GLYPH_SCOPES,
};
use ps1_puyopuyo_box::compress::decompress;
use ps1_puyopuyo_box::disasm::{
    verify_box_pcomi_custom_byte_admission_producers, verify_box_pcomi_custom_byte_atlas,
    verify_box_pcomi_custom_byte_consumer, verify_box_puyo2_ending_route,
    verify_box_puyo2_master_route, verify_box_subgame_seq_drac_glyphs, BtxCellAnalysis,
    BOX_SUBGAME_SEQ_DRAC_AUXILIARY_CELL, BOX_SUBGAME_SEQ_DRAC_AUXILIARY_GP_ACCESSES,
    BOX_SUBGAME_SEQ_DRAC_AUXILIARY_NEXT_CELL, BOX_SUBGAME_SEQ_DRAC_AUXILIARY_OBJECT_SITES,
    BOX_SUBGAME_SEQ_DRAC_GLYPH_SPECS, BOX_SUBGAME_SEQ_DRAC_MAIN_GP_ACCESSES,
    BOX_SUBGAME_SEQ_DRAC_MAIN_RESOURCE_BRANCH_SITES,
};
use ps1_puyopuyo_box::graphics::drac_glyph_patch::{
    verify_box_drac_scene_glyph_assets, BOX_DRAC_SCENE_GLYPH_ASSET_SPECS,
};
use ps1_puyopuyo_box::pipeline::subgame_seq_scene_glyph_patch::verify_box_subgame_seq_scene_glyph_patches;
use ps1_puyopuyo_box::pipeline::{
    render_subgame_seq_text_coverage_tsv, verify_box_subgame_seq_scene_relinks,
    verify_box_subgame_seq_scene_translation_manifest,
    verify_box_subgame_seq_scene_translation_plan, verify_box_subgame_seq_text_coverage,
    SubgameSeqTextCoverageDisposition,
};
use ps1_puyopuyo_box::script::{
    render_subgame_seq_custom_byte_records_tsv, verify_box_subgame_seq_custom_byte_records,
};

use super::{ArchiveAuditData, SourceAuditContext, SubgameAssetAuditData, SubgameTextAuditData};

pub(super) fn audit_subgame_scenes(
    context: &SourceAuditContext<'_>,
    archives: &ArchiveAuditData,
    text: &SubgameTextAuditData,
    assets: &SubgameAssetAuditData,
    btx_cells: &BtxCellAnalysis,
) -> Result<(), String> {
    let box_executables = &context.box_executables;
    let cnx_archives = archives.archive_copies();
    let subgame_seq_sources = &text.sources;
    let subgame_seq_layouts = &text.layouts;
    let subgame_seq_text_roots = &text.text_roots;
    let subgame_seq_drac_text = &text.drac_text;
    let subgame_seq_drac_consumers = &text.drac_consumers;
    let btx_name_consumers = &assets.btx_names;
    let puyo2_exe = box_executables
        .iter()
        .find(|exe| exe.map.name == "PUYO2.EXE")
        .ok_or_else(|| "PUYO2.EXE source가 없음".to_string())?;
    let pcomi_exe = box_executables
        .iter()
        .find(|exe| exe.map.name == "PCOMI.EXE")
        .ok_or_else(|| "PCOMI.EXE source가 없음".to_string())?;
    let (_, puyo2_seq_archive) = cnx_archives
        .iter()
        .find(|(source_id, _)| *source_id == "/PUYO2/PP2_SEQ.VDD;1")
        .ok_or_else(|| "/PUYO2/PP2_SEQ.VDD;1 archive가 없음".to_string())?;
    let puyo2_master_route =
        verify_box_puyo2_master_route(puyo2_exe.map, &puyo2_exe.text, puyo2_seq_archive)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS PUYO2 canonical master route: resident root 0x{:04X}, entries/master {}/{}, \
         PP1/PP2 child records {}/{}, PP1_ALL incoming/raw pointer/address load {}/{}/{}; \
         PP1 scene subtree protected no-write",
        puyo2_master_route.resident_root_resource,
        puyo2_master_route.archive_entries,
        puyo2_master_route.master_descriptors,
        puyo2_master_route.pp1_child_records,
        puyo2_master_route.pp2_child_records,
        puyo2_master_route.pp1_all_incoming_records,
        puyo2_master_route.pp1_all_raw_pointers,
        puyo2_master_route.pp1_all_address_loads,
    );
    let puyo2_ending_route =
        verify_box_puyo2_ending_route(puyo2_seq_archive).map_err(|error| error.to_string())?;
    println!(
        "PASS PUYO2 ending source route: PP2_ALL/TTL bytes {}/{}, PP2_TTL select/call {}/{}, \
         credits export {} +0x{:04X} -> END10 resource 0x{:04X} setup +0x{:04X}; \
         final ending export {} +0x{:04X} -> END15/16 resources 0x{:04X}/0x{:04X} setup \
         +0x{:04X}/+0x{:04X}; runtime evidence separate",
        puyo2_ending_route.pp2_all_bytes,
        puyo2_ending_route.pp2_ttl_bytes,
        puyo2_ending_route.pp2_ttl_selection_records,
        puyo2_ending_route.pp2_ttl_call_records,
        puyo2_ending_route.credits_export_index,
        puyo2_ending_route.credits_export_offset,
        puyo2_ending_route.end10_resource,
        puyo2_ending_route.end10_setup_offset,
        puyo2_ending_route.final_ending_export_index,
        puyo2_ending_route.final_ending_export_offset,
        puyo2_ending_route.end15_resource,
        puyo2_ending_route.end16_resource,
        puyo2_ending_route.end15_setup_offset,
        puyo2_ending_route.end16_setup_offset,
    );
    let subgame_seq_drac_glyphs = verify_box_subgame_seq_drac_glyphs(
        box_executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_DRAC_GLYPH_SPECS
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
                BOX_SUBGAME_SEQ_DRAC_GLYPH_SPECS
                    .iter()
                    .any(|spec| spec.grp_archive == *source_id)
            })
            .map(|(source_id, archive)| (*source_id, archive)),
        subgame_seq_drac_text,
        btx_name_consumers,
        btx_cells,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS DRAC glyph assets: rows {}, repertoire slots {}, source unique pairs {}, \
         source duplicate slots {}, cells {}..={}, \
         main cell-to-resource branch sites {}, main GP accesses {}/EXE, \
         GP/main/aux globals [{}], selected/same-named cells {:?}, \
         exact cell layout 1024x256 / 16x16 8x7 at (640,0), \
         exact BTX {}, runtime CLUT/VRAM upload/draw hit unverified",
        subgame_seq_drac_glyphs.rows.len(),
        subgame_seq_drac_glyphs
            .rows
            .iter()
            .map(|row| row.repertoire_glyphs)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .next()
            .unwrap_or(0),
        subgame_seq_drac_glyphs
            .rows
            .iter()
            .map(|row| row.source_unique_pairs)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .next()
            .unwrap_or(0),
        subgame_seq_drac_glyphs
            .rows
            .iter()
            .map(|row| row.source_duplicate_slots)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .next()
            .unwrap_or(0),
        subgame_seq_drac_glyphs
            .rows
            .iter()
            .map(|row| row.cell_base)
            .min()
            .unwrap_or(0),
        subgame_seq_drac_glyphs
            .rows
            .iter()
            .map(|row| row.cell_last)
            .max()
            .unwrap_or(0),
        BOX_SUBGAME_SEQ_DRAC_MAIN_RESOURCE_BRANCH_SITES,
        BOX_SUBGAME_SEQ_DRAC_MAIN_GP_ACCESSES,
        subgame_seq_drac_glyphs
            .rows
            .iter()
            .map(|row| format!(
                "{}:0x{:08X}/0x{:08X}/0x{:08X}",
                row.scope,
                row.gp_runtime,
                row.main_resource_global_runtime,
                row.auxiliary_resource_global_runtime,
            ))
            .collect::<Vec<_>>()
            .join(","),
        subgame_seq_drac_glyphs
            .rows
            .iter()
            .map(|row| (row.scope, row.selected_same_named_equal_cells))
            .collect::<Vec<_>>(),
        subgame_seq_drac_glyphs.rows.len(),
    );
    println!(
        "PASS DRAC auxiliary consumers: rows {}, exact GP accesses {}/EXE, \
         auxiliary cell metadata {} and {}, independent resource/cell {} object sites {}/EXE, \
         main atlas pixel mutation unverified",
        subgame_seq_drac_glyphs.rows.len(),
        BOX_SUBGAME_SEQ_DRAC_AUXILIARY_GP_ACCESSES,
        BOX_SUBGAME_SEQ_DRAC_AUXILIARY_CELL,
        BOX_SUBGAME_SEQ_DRAC_AUXILIARY_NEXT_CELL,
        BOX_SUBGAME_SEQ_DRAC_AUXILIARY_CELL,
        BOX_SUBGAME_SEQ_DRAC_AUXILIARY_OBJECT_SITES,
    );
    let subgame_seq_scene_glyphs = verify_box_subgame_seq_scene_glyphs(
        box_executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_SCENE_GLYPH_SCOPES
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
                BOX_SUBGAME_SEQ_SCENE_GLYPH_SCOPES
                    .iter()
                    .any(|spec| spec.grp_archive == *source_id)
            })
            .map(|(source_id, archive)| (*source_id, archive)),
        subgame_seq_layouts,
        subgame_seq_text_roots,
        btx_name_consumers,
        (subgame_seq_drac_consumers, &subgame_seq_drac_glyphs),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS scene-local E1/E2 glyph census: rows/families {}/{}, pair slots \
         strict-SJIS/opaque {}/{}/{}, grid slots {}, copy-equal pair tables/same-stem \
         assets {}/{}, selected=same-stem rows PUYO1/2 {}/{}, atlas-compatible selected \
         rows PUYO1/2 {}/{}, equal grid cells PUYO1/2 {}/{}, canonical/protected pair \
         slots {}/{}, compound rows/messages/glyph/control/variant {}/{}/{}/{}/{}, \
         runtime pixel source unverified",
        subgame_seq_scene_glyphs.rows.len(),
        subgame_seq_scene_glyphs.families,
        subgame_seq_scene_glyphs.repertoire_slots,
        subgame_seq_scene_glyphs.strict_sjis_pairs,
        subgame_seq_scene_glyphs.opaque_pairs,
        subgame_seq_scene_glyphs.grid_slots,
        subgame_seq_scene_glyphs.repertoire_copy_matches,
        subgame_seq_scene_glyphs.same_stem_copy_matches,
        subgame_seq_scene_glyphs.puyo1_selected_same_stem_rows,
        subgame_seq_scene_glyphs.puyo2_selected_same_stem_rows,
        subgame_seq_scene_glyphs.puyo1_selected_atlas_rows,
        subgame_seq_scene_glyphs.puyo2_selected_atlas_rows,
        subgame_seq_scene_glyphs.puyo1_selected_same_stem_cells,
        subgame_seq_scene_glyphs.puyo2_selected_same_stem_cells,
        subgame_seq_scene_glyphs.canonical_glyph_slots,
        subgame_seq_scene_glyphs.protected_message_slots,
        subgame_seq_scene_glyphs.compound_rows,
        subgame_seq_scene_glyphs.compound_message_records,
        subgame_seq_scene_glyphs.compound_glyph_tokens,
        subgame_seq_scene_glyphs.compound_control_tokens,
        subgame_seq_scene_glyphs.compound_variant_records,
    );
    let subgame_seq_scene_messages = verify_box_subgame_seq_scene_messages(
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        subgame_seq_layouts,
        &subgame_seq_scene_glyphs,
        subgame_seq_drac_consumers,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS scene-local E0 message census: families {}, references/physical {}/{}, \
         glyph/control/argument/terminator tokens {}/{}/{}/{}, variants {}, \
         direct pointers {}, referenced/unique bytes {}/{}, unique payload/cross-scope pairs \
         {}/{}, controls {:02X?}, \
         runtime scene/relink unverified",
        subgame_seq_scene_messages.families,
        subgame_seq_scene_messages.rows.len(),
        subgame_seq_scene_messages.physical_messages,
        subgame_seq_scene_messages.glyph_tokens,
        subgame_seq_scene_messages.control_tokens,
        subgame_seq_scene_messages.argument_controls,
        subgame_seq_scene_messages.terminators,
        subgame_seq_scene_messages.variant_records,
        subgame_seq_scene_messages.direct_pointers,
        subgame_seq_scene_messages.referenced_message_bytes,
        subgame_seq_scene_messages.unique_message_bytes,
        subgame_seq_scene_messages.unique_payloads,
        subgame_seq_scene_messages.cross_scope_pairs,
        subgame_seq_scene_messages.control_counts,
    );
    let subgame_seq_scene_relinks = verify_box_subgame_seq_scene_relinks(
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        subgame_seq_layouts,
        &subgame_seq_scene_messages,
        &subgame_seq_scene_glyphs,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS scene-local E0 bounded relink: rows/blocks/messages {}/{}/{}, \
         capacity/original/slack bytes {}/{}/{}, zero-slack blocks {}, \
         no-op repacks exact, runtime unverified",
        subgame_seq_scene_relinks.rows.len(),
        subgame_seq_scene_relinks.blocks,
        subgame_seq_scene_relinks.messages,
        subgame_seq_scene_relinks.capacity_bytes,
        subgame_seq_scene_relinks.original_message_bytes,
        subgame_seq_scene_relinks.slack_bytes,
        subgame_seq_scene_relinks.zero_slack_blocks,
    );
    let subgame_seq_custom_byte_records = verify_box_subgame_seq_custom_byte_records(
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &text.candidates,
        subgame_seq_layouts,
    )
    .map_err(|error| error.to_string())?;
    let custom_byte_report =
        render_subgame_seq_custom_byte_records_tsv(&subgame_seq_custom_byte_records);
    let custom_byte_digest = sha2::Sha256::digest(custom_byte_report.as_bytes());
    println!(
        "PASS subgame custom byte storage owner: rows {}, fixed slots {}, \
         repertoire/used codes {}/{}, accidental SJIS rejected, \
         consumer/atlas/runtime evidence owned separately, protected no-write, \
         report SHA-256 {}",
        subgame_seq_custom_byte_records.rows.len(),
        subgame_seq_custom_byte_records
            .rows
            .iter()
            .map(|row| row.slot_count)
            .sum::<usize>(),
        subgame_seq_custom_byte_records
            .rows
            .iter()
            .map(|row| row.repertoire_codes)
            .sum::<usize>(),
        subgame_seq_custom_byte_records
            .rows
            .iter()
            .map(|row| row.used_codes)
            .sum::<usize>(),
        custom_byte_digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );
    let pcomi_custom_byte_consumer = verify_box_pcomi_custom_byte_consumer(
        pcomi_exe.map,
        pcomi_exe.text.as_slice(),
        &subgame_seq_custom_byte_records,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS PCOMI custom byte consumer: formatter 0x{:08X}, call sites {}, \
         root selector {} -> 0x{:08X}, slots {} x {}B, RNG mod {}, \
         admission callback 0x{:08X} cell {} action ({},{}) state {}/mode {}, \
         byte buffer 0x{:08X} -> u16 stream 0x{:08X} -> direct cell 0x{:08X}, \
         renderers 0x{:08X}/0x{:08X}; atlas/runtime evidence verified separately, \
         protected no-write",
        pcomi_custom_byte_consumer.formatter_runtime,
        pcomi_custom_byte_consumer.formatter_call_sites.len(),
        pcomi_custom_byte_consumer.slot_selector,
        pcomi_custom_byte_consumer.root_field_runtime,
        pcomi_custom_byte_consumer.slot_count,
        pcomi_custom_byte_consumer.slot_payload_bytes,
        pcomi_custom_byte_consumer.rng_modulus,
        pcomi_custom_byte_consumer.admission_callback_runtime,
        pcomi_custom_byte_consumer.admission_source_cell,
        pcomi_custom_byte_consumer.admission_action_column,
        pcomi_custom_byte_consumer.admission_action_row,
        pcomi_custom_byte_consumer.admission_modal_state,
        pcomi_custom_byte_consumer.admission_selector_mode,
        pcomi_custom_byte_consumer.byte_buffer_runtime,
        pcomi_custom_byte_consumer.u16_stream_runtime,
        pcomi_custom_byte_consumer.direct_cell_store_runtime,
        pcomi_custom_byte_consumer.renderer_extended_runtime,
        pcomi_custom_byte_consumer.renderer_short_runtime,
    );
    let (_, _, mcrd1_seq) = subgame_seq_sources
        .iter()
        .find(|(archive, sequence, _)| archive == "/PCOMI/PC1_SEQ.VDD;1" && sequence == "MCRD1.SEQ")
        .ok_or_else(|| "/PCOMI/PC1_SEQ.VDD;1:MCRD1.SEQ source가 없음".to_string())?;
    let (_, _, pcomi1_seq) = subgame_seq_sources
        .iter()
        .find(|(archive, sequence, _)| {
            archive == "/PCOMI/PC1_SEQ.VDD;1" && sequence == "PCOMI1.SEQ"
        })
        .ok_or_else(|| "/PCOMI/PC1_SEQ.VDD;1:PCOMI1.SEQ source가 없음".to_string())?;
    let pcomi_admission_producers = verify_box_pcomi_custom_byte_admission_producers(
        pcomi_exe.map,
        pcomi_exe.text.as_slice(),
        mcrd1_seq,
        pcomi1_seq,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS PCOMI custom byte admission producers: opcode 0x{:02X} installs 0x{:08X}, \
         callback table 0x{:08X} UI/admission 0x{:08X}/0x{:08X}, \
         MCRD1 producers {}, PCOMI1 aligned halfword candidates {} -> producers {}, \
         rejected +0x{:04X}; routes +0x{:04X} and UI +0x{:04X} -> admission +0x{:04X}; \
         runtime UX owned by separate evidence, protected no-write",
        pcomi_admission_producers.installed_opcode,
        pcomi_admission_producers.installed_handler_runtime,
        pcomi_admission_producers.callback_table_runtime,
        pcomi_admission_producers.ui_callback_runtime,
        pcomi_admission_producers.admission_callback_runtime,
        pcomi_admission_producers.mcrd1_admission_producers,
        pcomi_admission_producers.pcomi1_aligned_halfword_candidates,
        pcomi_admission_producers.producers.len(),
        pcomi_admission_producers.rejected_halfword_offset,
        pcomi_admission_producers.producers[0].sequence_offset,
        pcomi_admission_producers.producers[1]
            .preceding_ui_callback_offset
            .unwrap_or(0),
        pcomi_admission_producers.producers[1].sequence_offset,
    );
    let (_, pcomi_grp_archive) = cnx_archives
        .iter()
        .find(|(source_id, _)| *source_id == "/PCOMI/PC1_GRP.VDD;1")
        .ok_or_else(|| "/PCOMI/PC1_GRP.VDD;1 archive가 없음".to_string())?;
    let pcomi_custom_byte_atlas = verify_box_pcomi_custom_byte_atlas(
        pcomi_exe.map,
        pcomi_exe.text.as_slice(),
        pcomi1_seq,
        pcomi_grp_archive,
        btx_name_consumers,
        btx_cells,
        &subgame_seq_custom_byte_records,
        &pcomi_custom_byte_consumer,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS PCOMI custom byte atlas: resource {} -> {} descriptor 0x{:08X}, \
         CNX stored/plain {}/{}, layout {}x{}, table 0x{:08X} records/empty {}/{}, \
         repertoire/used valid nonempty-pixel cells {}/{}, intentional blank space {}/{}, \
         renderers 0x{:08X}/0x{:08X}; \
         runtime evidence owned separately, protected no-write",
        pcomi_custom_byte_atlas.resource,
        pcomi_custom_byte_atlas.asset_name,
        pcomi_custom_byte_atlas.descriptor_runtime,
        pcomi_custom_byte_atlas.stored_bytes,
        pcomi_custom_byte_atlas.plain_bytes,
        pcomi_custom_byte_atlas.atlas_width,
        pcomi_custom_byte_atlas.atlas_height,
        pcomi_custom_byte_atlas.cell_table_runtime,
        pcomi_custom_byte_atlas.cell_records,
        pcomi_custom_byte_atlas.empty_cells,
        pcomi_custom_byte_atlas.repertoire_nonempty_pixel_cells,
        pcomi_custom_byte_atlas.used_nonempty_pixel_cells,
        pcomi_custom_byte_atlas.repertoire_blank_pixel_cells,
        pcomi_custom_byte_atlas.used_blank_pixel_cells,
        pcomi_custom_byte_atlas.renderer_short_runtime,
        pcomi_custom_byte_atlas.renderer_extended_runtime,
    );
    let subgame_seq_text_coverage = verify_box_subgame_seq_text_coverage(
        &text.candidates,
        subgame_seq_text_roots,
        &assets.e6_text,
        &subgame_seq_scene_glyphs,
        &subgame_seq_scene_relinks,
        &text.min_text,
        &subgame_seq_custom_byte_records,
    )
    .map_err(|error| error.to_string())?;
    let coverage_report = render_subgame_seq_text_coverage_tsv(&subgame_seq_text_coverage);
    let coverage_digest = sha2::Sha256::digest(coverage_report.as_bytes());
    println!(
        "PASS subgame SEQ candidate coverage: rows {}, E6 {}, scene message/inside-repertoire \
         {}/{}, scene repertoire {}, protected copy {}, custom byte record {}, \
         confirmed-u16 misaligned {}, aligned unresolved {}, report SHA-256 {}",
        subgame_seq_text_coverage.rows.len(),
        subgame_seq_text_coverage.count(SubgameSeqTextCoverageDisposition::E6Text),
        subgame_seq_text_coverage.count(SubgameSeqTextCoverageDisposition::SceneMessage),
        subgame_seq_text_coverage
            .count(SubgameSeqTextCoverageDisposition::SceneMessageInsideRepertoire),
        subgame_seq_text_coverage.count(SubgameSeqTextCoverageDisposition::SceneRepertoire),
        subgame_seq_text_coverage
            .count(SubgameSeqTextCoverageDisposition::ProtectedUnreferencedCopy),
        subgame_seq_text_coverage.count(SubgameSeqTextCoverageDisposition::CustomByteRecord),
        subgame_seq_text_coverage
            .count(SubgameSeqTextCoverageDisposition::MisalignedForConfirmedU16Consumers),
        subgame_seq_text_coverage.count(SubgameSeqTextCoverageDisposition::AlignedUnresolved),
        coverage_digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );
    let subgame_seq_scene_translation_manifest =
        verify_box_subgame_seq_scene_translation_manifest(&subgame_seq_scene_messages)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS scene-local E0 source-comparable translation manifest: sequences {}, \
         logical/physical payloads {}/{}, paired/singleton {}/{}, source/KO code units {}/{}, \
         pending/draft/approved {}/{}/{}, manifest SHA-256 {}, product eligible {}",
        subgame_seq_scene_translation_manifest.sequences,
        subgame_seq_scene_translation_manifest.logical_payloads,
        subgame_seq_scene_translation_manifest.physical_messages,
        subgame_seq_scene_translation_manifest.paired_payloads,
        subgame_seq_scene_translation_manifest.singleton_payloads,
        subgame_seq_scene_translation_manifest.protected_source_code_units,
        subgame_seq_scene_translation_manifest.translated_code_units,
        subgame_seq_scene_translation_manifest.pending_human_translation,
        subgame_seq_scene_translation_manifest.draft_needs_human_review,
        subgame_seq_scene_translation_manifest.approved,
        subgame_seq_scene_translation_manifest.manifest_sha256,
        subgame_seq_scene_translation_manifest.product_eligible,
    );
    let subgame_seq_scene_translation_plan = verify_box_subgame_seq_scene_translation_plan(
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &subgame_seq_scene_translation_manifest,
        &subgame_seq_scene_messages,
        &subgame_seq_scene_glyphs,
        &subgame_seq_scene_relinks,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS scene-local E0 Unicode/pair synthetic compiler: families {}, \
         physical messages {}, family-local Hangul mappings {}, capacity/used/remaining \
         {}/{}/{}, bounded repacks {}, BTX/runtime/final translation unverified",
        subgame_seq_scene_translation_plan.families,
        subgame_seq_scene_translation_plan.physical_messages,
        subgame_seq_scene_translation_plan.glyph_mappings.len(),
        subgame_seq_scene_translation_plan.capacity_bytes,
        subgame_seq_scene_translation_plan.used_bytes,
        subgame_seq_scene_translation_plan.remaining_bytes,
        subgame_seq_scene_translation_plan.repacks.len(),
    );
    let subgame_seq_scene_glyph_patch_sources = subgame_seq_scene_glyphs
        .rows
        .iter()
        .filter(|row| row.scope == "PUYO1")
        .map(|row| {
            let archive = cnx_archives
                .iter()
                .find(|(source_id, _)| *source_id == row.grp_archive)
                .map(|(_, archive)| archive)
                .ok_or_else(|| format!("{} scene glyph GRP 누락", row.grp_archive))?;
            let entry = archive.find(row.same_stem_asset).ok_or_else(|| {
                format!(
                    "{}:{} scene glyph asset 누락",
                    row.grp_archive, row.same_stem_asset
                )
            })?;
            let plain = decompress(archive.read_file(entry)).map_err(|source| {
                format!(
                    "{}:{} scene glyph 해제 실패: {source}",
                    row.grp_archive, row.same_stem_asset
                )
            })?;
            Ok((row.grp_archive, row.same_stem_asset, plain))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let subgame_seq_scene_glyph_patches = verify_box_subgame_seq_scene_glyph_patches(
        subgame_seq_scene_glyph_patch_sources
            .iter()
            .map(|(archive, asset, plain)| (*archive, *asset, plain.as_slice())),
        subgame_seq_sources
            .iter()
            .filter(|(archive, sequence, _)| {
                subgame_seq_scene_glyphs.rows.iter().any(|row| {
                    row.scope == "PUYO1" && row.seq_archive == archive && row.sequence == sequence
                })
            })
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &subgame_seq_scene_glyphs,
        &subgame_seq_scene_translation_plan.glyph_mappings,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS PUYO1 scene-local synthetic BTX cell compile: assets/cells {}/{}, \
         source palette {:?}, changed pixels/bytes {}/{}, Maplestory Light profile, \
         non-target pixels protected; runtime/final translation unverified",
        subgame_seq_scene_glyph_patches.assets,
        subgame_seq_scene_glyph_patches.cells,
        subgame_seq_scene_glyph_patches.source_palette_indices,
        subgame_seq_scene_glyph_patches.changed_pixels,
        subgame_seq_scene_glyph_patches.changed_bytes,
    );
    let drac_scene_glyph_sources = BOX_DRAC_SCENE_GLYPH_ASSET_SPECS
        .iter()
        .map(|spec| {
            let archive = cnx_archives
                .iter()
                .find(|(source_id, _)| *source_id == spec.archive)
                .map(|(_, archive)| archive)
                .ok_or_else(|| format!("{} DRAC glyph GRP 누락", spec.archive))?;
            let entry = archive
                .find(spec.asset)
                .ok_or_else(|| format!("{}:{} DRAC glyph asset 누락", spec.archive, spec.asset))?;
            let plain = decompress(archive.read_file(entry)).map_err(|source| {
                format!(
                    "{}:{} DRAC glyph 해제 실패: {source}",
                    spec.archive, spec.asset
                )
            })?;
            Ok((spec.archive, spec.asset, plain))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let drac_scene_glyph_assets = verify_box_drac_scene_glyph_assets(
        drac_scene_glyph_sources
            .iter()
            .map(|(archive, asset, plain)| (*archive, *asset, plain.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS DRAC glyph patch sources: rows {}, binary/uniform-zero cells {:?}, \
         source palette {:?}, Maplestory Light single-glyph PoC changed pixels/bytes {:?}, \
         output SHA-256 {:?}, runtime unverified",
        drac_scene_glyph_assets.rows.len(),
        drac_scene_glyph_assets
            .rows
            .iter()
            .map(|row| (row.scope, row.binary_cells, row.uniform_zero_cells))
            .collect::<Vec<_>>(),
        drac_scene_glyph_assets
            .rows
            .iter()
            .map(|row| (row.scope, row.source_palette_indices.clone()))
            .collect::<Vec<_>>(),
        drac_scene_glyph_assets
            .rows
            .iter()
            .map(|row| {
                (
                    row.scope,
                    row.single_glyph_poc_changed_pixels,
                    row.single_glyph_poc_changed_bytes,
                )
            })
            .collect::<Vec<_>>(),
        drac_scene_glyph_assets
            .rows
            .iter()
            .map(|row| (row.scope, row.single_glyph_poc_sha256.as_str()))
            .collect::<Vec<_>>(),
    );
    Ok(())
}
