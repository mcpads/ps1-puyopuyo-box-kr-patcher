//! 서브게임 SEQ text, layout, DRAC message 소비를 검증한다.

use std::collections::BTreeSet;

use ps1_puyopuyo_box::analysis::verify_box_subgame_seq_overlay_call_arguments;
use ps1_puyopuyo_box::disasm::{
    verify_box_pcomi_glyph_codes, verify_box_pcomi_seq_load_route,
    verify_box_subgame_seq_control_targets, verify_box_subgame_seq_drac_consumers,
    verify_box_subgame_seq_text_bridges, SubgameControlAnalysis, SubgameTextConsumerAnalysis,
    BOX_SUBGAME_SEQ_DRAC_CONSUMER_SPECS,
};
use ps1_puyopuyo_box::script::{
    plan_box_subgame_seq_drac_translation, repack_box_subgame_seq_drac_messages,
    verify_box_subgame_seq_drac_relinks, verify_box_subgame_seq_drac_text,
    verify_box_subgame_seq_layouts, verify_box_subgame_seq_min_text,
    verify_box_subgame_seq_text_candidates, verify_box_subgame_seq_text_roots,
    SubgameSeqDracTranslation,
};

use crate::cli::analysis_inputs::read_subgame_seq_sources;

use super::{ArchiveAuditData, SourceAuditContext, SubgameTextAuditData};

pub(super) fn audit_subgame_text(
    context: &SourceAuditContext<'_>,
    archives: &ArchiveAuditData,
    subgame_text_consumers: &SubgameTextConsumerAnalysis,
    subgame_controls: &SubgameControlAnalysis,
) -> Result<SubgameTextAuditData, String> {
    let box_executables = &context.box_executables;
    let cnx_archives = archives.archive_copies();
    let pcomi = box_executables
        .iter()
        .find(|exe| exe.map.name == "PCOMI.EXE")
        .ok_or_else(|| "PCOMI.EXE 원본 입력 누락".to_string())?;
    let pcomi_glyph_codes =
        verify_box_pcomi_glyph_codes(pcomi.map, pcomi.text.as_slice(), subgame_text_consumers)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS PCOMI glyph code routes: consumers {}, direct <0x1000 +0x96, \
         suffix tables {} × 96, nonzero {}/{}, unique {}/{}, BIOS fallback 0x8001BA4C; \
         suffix writer high 0x20..0x7F; custom-byte direct-cell consumer is separate",
        pcomi_glyph_codes.consumers.len(),
        pcomi_glyph_codes.tables.len(),
        pcomi_glyph_codes.tables[0].nonzero_entries,
        pcomi_glyph_codes.tables[1].nonzero_entries,
        pcomi_glyph_codes.tables[0].unique_entries,
        pcomi_glyph_codes.tables[1].unique_entries,
    );
    let subgame_seq_sources = read_subgame_seq_sources(&cnx_archives)?;
    let pcomi1 = subgame_seq_sources
        .iter()
        .find(|(archive, sequence, _)| {
            archive == "/PCOMI/PC1_SEQ.VDD;1" && sequence == "PCOMI1.SEQ"
        })
        .ok_or_else(|| "PCOMI1.SEQ 원본 입력 누락".to_string())?;
    let pcomi_load_route =
        verify_box_pcomi_seq_load_route(pcomi.map, pcomi.text.as_slice(), pcomi1.2.as_slice())
            .map_err(|error| error.to_string())?;
    println!(
        "PASS PCOMI1 load route: record 0x{:08X}, type/resource {}/{}, flags 0x{:02X}, \
         descriptor 0x{:08X} -> destination 0x{:08X}, completion 0x{:08X}, \
         master call 0x{:08X} -> overlay entry/start 0x{:08X}/0x{:08X}; \
         custom-byte reader/atlas verified separately",
        pcomi_load_route.load_record_runtime,
        pcomi_load_route.resource_type,
        pcomi_load_route.resource_id,
        pcomi_load_route.flags,
        pcomi_load_route.descriptor_runtime,
        pcomi_load_route.destination_runtime,
        pcomi_load_route.completion_callback_runtime,
        pcomi_load_route.master_call_runtime,
        pcomi_load_route.overlay_entry_runtime,
        pcomi_load_route.overlay_start_runtime,
    );
    let subgame_control_targets = verify_box_subgame_seq_control_targets(
        box_executables
            .iter()
            .filter(|exe| matches!(exe.map.name, "PCOMI.EXE" | "PUYO1.EXE" | "PUYO2.EXE"))
            .map(|exe| (exe.map, exe.text.as_slice())),
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        subgame_controls,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame SEQ control targets: handlers {}, files {}, aligned candidates SEQ/EXE {}/{}, internal pointer records {}, FF12 generated 22B, FF15 admitted <=768B",
        subgame_control_targets.rows.len(),
        subgame_control_targets.sequence_files,
        subgame_control_targets.sequence_candidates,
        subgame_control_targets.exe_candidates,
        subgame_control_targets.internal_pointers,
    );
    let subgame_seq_text_bridges = verify_box_subgame_seq_text_bridges(
        box_executables
            .iter()
            .map(|exe| (exe.map, exe.text.as_slice())),
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        subgame_text_consumers,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame SEQ text constructor bridges: EXE {}, canonical OP21/mode2 EXE/SEQ {}/{}, \
         object +0x8C consumers {}, SEQ direct-reference scan {} files × {}, \
         direct refs 0, entry reached 0, indirect sites PCOMI/PUYO1/PUYO2 79/67/67, \
         entry jr BIOS/table/unclassified {}/{}/0, expanded seeds/instructions {}/{}, \
         expanded indirect/jalr {}/{}, jr BIOS/table/unclassified {}/{}/0, \
         expanded jalr target BIOS/initialized/outside-text/runtime-field/forwarded/unclassified \
         {}/{}/{}/{}/{}/0, outside direct read/write/address {}/{}/0, \
         forwarded direct callers {}, fresh stack-arg5 {}, \
         standard-frame per-call stack-arg5 jalr EXE {}, constructor targets 0, \
         runtime source and arg5 unresolved",
        subgame_seq_text_bridges.rows.len(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .map(|row| row.exe_canonical_mode2_records)
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .first()
            .map_or(0, |row| row.sequence_canonical_mode2_records),
        subgame_seq_text_bridges.rows.len(),
        subgame_seq_text_bridges.sequence_files,
        subgame_seq_text_bridges.rows.len(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .map(|row| row.entry_jr_bios_sites.len())
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .map(|row| row.entry_jr_table_sources.len())
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .map(|row| row.expanded_trace.table_target_seeds)
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .map(|row| row.expanded_trace.reached_instructions)
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .map(|row| row.expanded_trace.indirect_sites)
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .map(|row| row.expanded_trace.jalr_sites)
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .map(|row| row.expanded_trace.jr_bios_sites)
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .map(|row| row.expanded_trace.jr_table_sites)
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .map(|row| row.expanded_trace.jalr_bios_vector_sites.len())
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .flat_map(|row| {
                row.expanded_trace
                    .jalr_initialized_direct_sources
                    .iter()
                    .chain(&row.expanded_trace.jalr_initialized_rooted_sources)
            })
            .map(|source| source.call_sites.len())
            .sum::<usize>()
            + subgame_seq_text_bridges
                .rows
                .iter()
                .map(|row| row.expanded_trace.jalr_initialized_indexed_sources.len())
                .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .flat_map(|row| &row.expanded_trace.jalr_outside_text_sources)
            .map(|source| source.call_sites.len())
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .map(|row| row.expanded_trace.jalr_computed_memory_sources.len())
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .map(|row| row.expanded_trace.jalr_forwarded_register_sites.len())
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .flat_map(|row| &row.expanded_trace.jalr_outside_text_sources)
            .map(|source| source.direct_read_sites.len())
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .flat_map(|row| &row.expanded_trace.jalr_outside_text_sources)
            .map(|source| source.direct_write_sites.len())
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .flat_map(|row| &row.expanded_trace.jalr_forwarded_register_sources)
            .map(|source| source.direct_call_sites.len())
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .map(|row| row.expanded_trace.jalr_fresh_stack_arg5_sites.len())
            .sum::<usize>(),
        subgame_seq_text_bridges
            .rows
            .iter()
            .map(|row| row.exe_stack_arg5_jalr_sites)
            .sum::<usize>(),
    );
    let overlay_call_arguments = verify_box_subgame_seq_overlay_call_arguments(
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame SEQ overlay outgoing stack arguments: files {}, overlays {}, rows {}, \
         current-call {}, previously-consumed {}; linear stack-slot flow only, callee and \
         argument meaning unresolved",
        overlay_call_arguments.sequence_files,
        overlay_call_arguments.overlay_files,
        overlay_call_arguments.rows.len(),
        overlay_call_arguments.current_call_rows(),
        overlay_call_arguments.previously_consumed_rows(),
    );
    let subgame_seq_text = verify_box_subgame_seq_text_candidates(
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame SEQ text static lower bound: archives {}, sequences {}, bytes {}, \
         candidate files {}, candidates {}, code units {}, strict {}, heuristic-only {}, \
         unique decoded characters {}, analysis SHA-256 {}",
        subgame_seq_text.archives,
        subgame_seq_text.summaries.len(),
        subgame_seq_text.total_bytes,
        subgame_seq_text.files_with_candidates,
        subgame_seq_text.candidates.len(),
        subgame_seq_text.code_units,
        subgame_seq_text.strict_candidates,
        subgame_seq_text.heuristic_only_candidates,
        subgame_seq_text.unique_decoded_characters,
        subgame_seq_text
            .analysis_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );
    let subgame_seq_layouts = verify_box_subgame_seq_layouts(
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &subgame_seq_text,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame SEQ layouts: files {}, overlays {}, raw-opaque {}, \
         overlay candidates/code units {}/{}, raw {}/{}, load bases {}, DRAC roots {}",
        subgame_seq_layouts.rows.len(),
        subgame_seq_layouts.overlays,
        subgame_seq_layouts.raw_opaque,
        subgame_seq_layouts.overlay_candidates,
        subgame_seq_layouts.overlay_code_units,
        subgame_seq_layouts.raw_candidates,
        subgame_seq_layouts.raw_code_units,
        subgame_seq_layouts.load_base_counts.len(),
        subgame_seq_layouts.roots,
    );
    let subgame_seq_text_roots = verify_box_subgame_seq_text_roots(
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &subgame_seq_text,
        &subgame_seq_layouts,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame SEQ text pointer roots: candidates {}, exact {}, preceding-header {}, \
         none {}, relations exact/header {}/{}; all remain no-write",
        subgame_seq_text_roots.rows.len(),
        subgame_seq_text_roots.exact_candidates,
        subgame_seq_text_roots.header_candidates,
        subgame_seq_text_roots.none_candidates,
        subgame_seq_text_roots.exact_relations,
        subgame_seq_text_roots.header_relations,
    );
    let subgame_seq_min_text = verify_box_subgame_seq_min_text(
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &subgame_seq_text,
        &subgame_seq_layouts,
        &subgame_seq_text_roots,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS MIN scene text: copies {}, repertoire {}/{} code units, messages {}/{} spans/{} \
         terminators/{} code units, protected {}B, direct pointers {}, zero reserve {}B, runtime \
         unverified",
        subgame_seq_min_text.copies,
        subgame_seq_min_text.repertoires,
        subgame_seq_min_text.repertoire_code_units,
        subgame_seq_min_text.message_records,
        subgame_seq_min_text.message_spans,
        subgame_seq_min_text.message_terminators,
        subgame_seq_min_text.message_code_units,
        subgame_seq_min_text.message_protected_bytes,
        subgame_seq_min_text.direct_pointers,
        subgame_seq_min_text.zero_reserve_bytes,
    );
    let subgame_seq_drac_text = verify_box_subgame_seq_drac_text(
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &subgame_seq_text,
        &subgame_seq_layouts,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame DRAC text records: repertoire {} / {} code units, \
         messages {} / spans {} / {} code units, protected bytes {}, \
         kana misses {}",
        subgame_seq_drac_text.repertoires,
        subgame_seq_drac_text.repertoire_code_units,
        subgame_seq_drac_text.message_records,
        subgame_seq_drac_text.message_spans,
        subgame_seq_drac_text.message_code_units,
        subgame_seq_drac_text.message_protected_bytes,
        subgame_seq_drac_text.missed_message_spans,
    );
    let subgame_seq_drac_relinks = verify_box_subgame_seq_drac_relinks(
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &subgame_seq_drac_text,
        &subgame_seq_layouts,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame DRAC message relinks: copies {}, direct pointers {} each, \
         original {}B, common in-place capacity/slack {}B/{}B, \
         extra two-byte glyphs {}, bounded repacker no-op exact, protected tails exact, \
         runtime unverified",
        subgame_seq_drac_relinks.rows.len(),
        subgame_seq_drac_relinks
            .rows
            .iter()
            .map(|row| row.pointers.len())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .next()
            .unwrap_or(0),
        subgame_seq_drac_relinks
            .rows
            .iter()
            .map(|row| row.original_message_bytes)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .next()
            .unwrap_or(0),
        subgame_seq_drac_relinks.common_capacity_bytes,
        subgame_seq_drac_relinks.common_slack_bytes,
        subgame_seq_drac_relinks.common_extra_two_byte_glyphs,
    );
    let one_span = ["가"];
    let three_spans = ["가", "가", "가"];
    let synthetic_drac_translations = [
        SubgameSeqDracTranslation {
            index: 1,
            spans: &one_span,
        },
        SubgameSeqDracTranslation {
            index: 2,
            spans: &three_spans,
        },
        SubgameSeqDracTranslation {
            index: 3,
            spans: &three_spans,
        },
        SubgameSeqDracTranslation {
            index: 4,
            spans: &three_spans,
        },
        SubgameSeqDracTranslation {
            index: 5,
            spans: &three_spans,
        },
    ];
    let synthetic_drac_plan =
        plan_box_subgame_seq_drac_translation(&synthetic_drac_translations, &subgame_seq_drac_text)
            .map_err(|error| error.to_string())?;
    let mut synthetic_repacked_copies = 0usize;
    for row in &subgame_seq_drac_relinks.rows {
        let raw = subgame_seq_sources
            .iter()
            .find(|(archive, sequence, _)| archive == row.archive && sequence == row.sequence)
            .map(|(_, _, raw)| raw.as_slice())
            .ok_or_else(|| format!("{} DRAC synthetic repack source 누락", row.scope))?;
        let repack = repack_box_subgame_seq_drac_messages(
            row.scope,
            raw,
            &synthetic_drac_plan.replacement_records,
            &subgame_seq_drac_text,
            &subgame_seq_drac_relinks,
        )
        .map_err(|error| error.to_string())?;
        if repack.used_bytes != synthetic_drac_plan.used_bytes
            || repack.remaining_bytes != synthetic_drac_plan.remaining_bytes
        {
            return Err(format!(
                "{} DRAC synthetic compiler/repacker 용량 불일치",
                row.scope
            ));
        }
        synthetic_repacked_copies += 1;
    }
    println!(
        "PASS subgame DRAC Unicode compiler: synthetic one-glyph source gate, \
         records {}, mappings {}, repacked copies {}, unique/duplicate repertoire pairs {}/{}, \
         used/remaining {}/{}, \
         final translation unapproved",
        synthetic_drac_plan.replacement_records.len(),
        synthetic_drac_plan.glyph_mappings.len(),
        synthetic_repacked_copies,
        synthetic_drac_plan.source_unique_pairs,
        synthetic_drac_plan.source_duplicate_slots,
        synthetic_drac_plan.used_bytes,
        synthetic_drac_plan.remaining_bytes,
    );
    let subgame_seq_drac_consumers = verify_box_subgame_seq_drac_consumers(
        box_executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_DRAC_CONSUMER_SPECS
                    .iter()
                    .any(|spec| spec.exe == exe.map.name)
            })
            .map(|exe| (exe.map, exe.text.as_slice())),
        subgame_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &subgame_seq_drac_text,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS subgame DRAC record consumers: EXE {}, records {}, E0 commands/variants {}/{}, \
         glyph tokens {}, argument/state controls {}/{}, terminators {}, \
         header/layout matches {}, line-break pairs {}, column advances {}, end/current waits {}/{}, \
         message update callbacks {}",
        subgame_seq_drac_consumers.rows.len(),
        subgame_seq_drac_consumers
            .rows
            .iter()
            .map(|row| row.records)
            .sum::<usize>(),
        subgame_seq_drac_consumers
            .rows
            .iter()
            .map(|row| row.command_records)
            .sum::<usize>(),
        subgame_seq_drac_consumers
            .rows
            .iter()
            .map(|row| row.variant_records)
            .sum::<usize>(),
        subgame_seq_drac_consumers
            .rows
            .iter()
            .map(|row| row.glyph_tokens)
            .sum::<usize>(),
        subgame_seq_drac_consumers
            .rows
            .iter()
            .map(|row| row.argument_controls)
            .sum::<usize>(),
        subgame_seq_drac_consumers
            .rows
            .iter()
            .map(|row| row.state_controls)
            .sum::<usize>(),
        subgame_seq_drac_consumers
            .rows
            .iter()
            .map(|row| row.terminators)
            .sum::<usize>(),
        subgame_seq_drac_consumers
            .rows
            .iter()
            .map(|row| row.header_layout_matches)
            .sum::<usize>(),
        subgame_seq_drac_consumers
            .rows
            .iter()
            .map(|row| row.line_break_pairs)
            .sum::<usize>(),
        subgame_seq_drac_consumers
            .rows
            .iter()
            .map(|row| row.column_advances)
            .sum::<usize>(),
        subgame_seq_drac_consumers
            .rows
            .iter()
            .map(|row| row.end_waits)
            .sum::<usize>(),
        subgame_seq_drac_consumers
            .rows
            .iter()
            .map(|row| row.current_waits)
            .sum::<usize>(),
        subgame_seq_drac_consumers.rows.len(),
    );

    Ok(SubgameTextAuditData {
        sources: subgame_seq_sources,
        candidates: subgame_seq_text,
        layouts: subgame_seq_layouts,
        text_roots: subgame_seq_text_roots,
        min_text: subgame_seq_min_text,
        drac_text: subgame_seq_drac_text,
        drac_consumers: subgame_seq_drac_consumers,
    })
}
