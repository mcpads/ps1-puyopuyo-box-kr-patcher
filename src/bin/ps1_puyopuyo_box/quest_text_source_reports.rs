//! QUEST text source와 실행 경로의 정적 report 명령.

use std::collections::BTreeSet;
use std::path::PathBuf;

use ps1_puyopuyo_box::analysis::quest_seq_flag_branches::{
    render_quest_seq_flag_branches_tsv, BOX_QUEST_SUKE_CONFRONTATION_FLAG_RECORD_OFFSET,
};
use ps1_puyopuyo_box::analysis::quest_seq_resource_transitions::render_quest_seq_resource_transitions_tsv;
use ps1_puyopuyo_box::analysis::quest_seq_text_frontiers::render_quest_seq_text_frontiers_tsv;
use ps1_puyopuyo_box::analysis::quest_seq_text_paths::{
    render_quest_seq_text_paths_tsv, QuestSeqTextPathTransition,
};
use ps1_puyopuyo_box::analysis::{
    render_quest_battle_name_table_tsv, verify_box_quest_battle_name_table,
};
use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::compress::{decompress, is_cnx};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::script::quest_item_text_units::render_quest_item_text_units_tsv;
use ps1_puyopuyo_box::script::quest_seq_text_candidates::{
    render_quest_seq_text_candidates_tsv, render_quest_seq_text_summary_tsv,
};
use ps1_puyopuyo_box::script::quest_seq_text_document::render_quest_seq_text_document_summary_tsv;
use ps1_puyopuyo_box::script::quest_seq_text_extraction::render_quest_seq_text_token_ledger_tsv;
use ps1_puyopuyo_box::script::quest_seq_text_roots::render_quest_seq_text_roots_tsv;
use ps1_puyopuyo_box::script::quest_seq_text_streams::{
    render_quest_seq_rooted_text_runs_tsv, render_quest_seq_text_streams_tsv,
};
use ps1_puyopuyo_box::script::quest_text_candidates::{
    render_town00_text_candidates_tsv, verify_box_town00_text_candidates,
};

use super::disc_commands::read_box_executables;
use super::quest_analysis_inputs::{
    read_quest_item_text_analysis, read_quest_seq_flag_branch_analysis,
    read_quest_seq_resource_transition_analysis, read_quest_seq_text_analysis,
    read_quest_seq_text_document_analysis, read_quest_seq_text_extraction_analysis,
    read_quest_seq_text_frontier_analysis, read_quest_seq_text_path_analysis,
    read_quest_seq_text_root_analysis, read_quest_seq_text_stream_analysis,
};
use super::translation_reports::write_report_file;
use super::usage::usage;
use super::{quest_code_table_from_executable, RPG_SEQ_PATH, TOWN00_NAME};

pub(super) fn write_quest_battle_name_source_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let code_table = quest_code_table_from_executable(quest.map, &quest.text)?;
    let analysis = verify_box_quest_battle_name_table(quest.map, &quest.text, &code_table)
        .map_err(|error| error.to_string())?;
    let report = render_quest_battle_name_table_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST battle name source TSV",
        analysis.entries.len(),
        &report,
    )?;
    println!(
        "  physical entries {}, unique sources {}, glyphs {}, protected slot bytes {}",
        analysis.entries.len(),
        analysis.unique_sources,
        analysis.glyph_occurrences,
        analysis.slot_bytes,
    );
    Ok(())
}

pub(super) fn write_quest_item_text_source_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_quest_item_text_analysis(source_path)?;
    let report = render_quest_item_text_units_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST item text source TSV",
        analysis.units.len(),
        &report,
    )?;
    println!(
        "  records {}, names {}, descriptions {}, Japanese units {}, runtime-observed {}",
        analysis.unique_records,
        analysis.name_units,
        analysis.description_units,
        analysis.japanese_units,
        analysis.runtime_observed_units,
    );
    Ok(())
}

pub(super) fn write_town00_text_candidate_review(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest_executable = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let code_table =
        quest_code_table_from_executable(quest_executable.map, &quest_executable.text)?;

    let vdd_raw = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&vdd_raw)
        .map_err(|error| format!("{RPG_SEQ_PATH} VDD 파싱 실패: {error}"))?;
    let entry = archive
        .find(TOWN00_NAME)
        .ok_or_else(|| format!("{RPG_SEQ_PATH}:{TOWN00_NAME} 엔트리가 없음"))?;
    let cnx = archive.read_file(entry);
    if !is_cnx(cnx) {
        return Err(format!("{RPG_SEQ_PATH}:{TOWN00_NAME}가 CNX v2가 아님"));
    }
    let town00 =
        decompress(cnx).map_err(|error| format!("{RPG_SEQ_PATH}:{TOWN00_NAME}: {error}"))?;
    let analysis = verify_box_town00_text_candidates(&town00, &code_table)
        .map_err(|error| error.to_string())?;
    let report = render_town00_text_candidates_tsv(&analysis);
    write_report_file(
        output_path,
        "TOWN00 text candidate TSV",
        analysis.candidates.len(),
        &report,
    )?;
    println!(
        "  runtime-observed spans {}, static-only {}, source code units {}, \
         unique source characters {}",
        analysis.runtime_observed_spans,
        analysis.static_only_candidates,
        analysis.source_code_units,
        analysis.unique_source_characters,
    );
    Ok(())
}

pub(super) fn write_quest_seq_text_candidate_review(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_quest_seq_text_analysis(source_path)?;
    let report = render_quest_seq_text_candidates_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST SEQ text candidate TSV",
        analysis.candidates.len(),
        &report,
    )?;
    println!(
        "  sequences {} / with candidates {}, runtime-observed spans {}, static-only {}, \
         source code units {}, unique source characters {}",
        analysis.summaries.len(),
        analysis.sequences_with_candidates,
        analysis.runtime_observed_spans,
        analysis.static_only_candidates,
        analysis.source_code_units,
        analysis.unique_source_characters,
    );
    Ok(())
}

pub(super) fn write_quest_seq_text_summary(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_quest_seq_text_analysis(source_path)?;
    let report = render_quest_seq_text_summary_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST SEQ text summary TSV",
        analysis.summaries.len(),
        &report,
    )
}

pub(super) fn write_quest_seq_text_stream_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_quest_seq_text_stream_analysis(source_path)?;
    let report = render_quest_seq_text_streams_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST SEQ OP21 text stream TSV",
        analysis.markers.len(),
        &report,
    )?;
    println!(
        "  sequences {}, OP21 shapes {}, parseable {}, rejected {}, rooted runs {} / \
         code-table candidates {}, rooted code units {}, unique source characters {}",
        analysis.summaries.len(),
        analysis.markers.len(),
        analysis
            .markers
            .iter()
            .filter(|marker| marker.is_parseable())
            .count(),
        analysis
            .markers
            .iter()
            .filter(|marker| !marker.is_parseable())
            .count(),
        analysis.rooted_runs.len(),
        analysis.code_table_run_candidates,
        analysis.rooted_code_units,
        analysis.unique_source_characters,
    );
    Ok(())
}

pub(super) fn write_quest_seq_text_root_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_quest_seq_text_root_analysis(source_path)?;
    let report = render_quest_seq_text_roots_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST SEQ OP21 direct root TSV",
        analysis.markers.len(),
        &report,
    )?;
    println!(
        "  consumer-backed markers {}, edges {}, range-word-only {}, \
         no direct word {}, rooted runs {}, rooted code units {}",
        analysis
            .markers
            .iter()
            .filter(|marker| !marker.consumer_edges.is_empty())
            .count(),
        analysis
            .markers
            .iter()
            .map(|marker| marker.consumer_edges.len())
            .sum::<usize>(),
        analysis
            .summaries
            .iter()
            .map(|summary| summary.range_only_markers)
            .sum::<usize>(),
        analysis
            .summaries
            .iter()
            .map(|summary| summary.no_direct_word_markers)
            .sum::<usize>(),
        analysis.rooted_runs,
        analysis.rooted_code_units,
    );
    Ok(())
}

pub(super) fn write_quest_seq_text_path_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_quest_seq_text_path_analysis(source_path)?;
    let report = render_quest_seq_text_paths_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST SEQ OP21 static path TSV",
        analysis.paths.len(),
        &report,
    )?;
    let record_mode_advances =
        analysis.transition_count(QuestSeqTextPathTransition::RecordModeAdvance);
    let subdispatch_advances =
        analysis.transition_count(QuestSeqTextPathTransition::SubdispatchAdvance);
    let fallback_advances = analysis.transition_count(QuestSeqTextPathTransition::FallbackAdvance);
    println!(
        "  direct {}, sequential {}, OP03 branch-reached {}, unique edges {} / occurrences {}, stopped {}, \
         path nodes {}, record mode {} / subdispatch {} / fallback {}, \
         rooted runs {}, rooted code units {}",
        analysis
            .summaries
            .iter()
            .map(|summary| summary.direct_markers)
            .sum::<usize>(),
        analysis
            .summaries
            .iter()
            .map(|summary| summary.sequential_markers)
            .sum::<usize>(),
        analysis
            .summaries
            .iter()
            .map(|summary| summary.direct_branch_markers)
            .sum::<usize>(),
        analysis.unique_direct_branch_edges,
        analysis.direct_branch_edge_occurrences,
        analysis
            .summaries
            .iter()
            .map(|summary| summary.stopped)
            .sum::<usize>(),
        analysis
            .summaries
            .iter()
            .map(|summary| summary.path_node_occurrences)
            .sum::<usize>(),
        record_mode_advances,
        subdispatch_advances,
        fallback_advances,
        analysis.rooted_runs,
        analysis.rooted_code_units,
    );
    Ok(())
}

pub(super) fn write_quest_seq_text_frontier_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_quest_seq_text_frontier_analysis(source_path)?;
    let report = render_quest_seq_text_frontiers_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST SEQ text frontier TSV",
        analysis.frontiers.len(),
        &report,
    )?;
    let op0d_frontiers = analysis
        .frontiers
        .iter()
        .filter(|frontier| frontier.runtime_probe_address.is_some())
        .count();
    let probe_addresses = analysis
        .frontiers
        .iter()
        .filter_map(|frontier| frontier.runtime_probe_address)
        .collect::<BTreeSet<_>>();
    println!(
        "  path occurrences {}, OP0D frontiers {}, unique probe addresses {}, \
         compare records {}, runtime requirements {}",
        analysis.path_occurrences,
        op0d_frontiers,
        probe_addresses.len(),
        analysis
            .frontiers
            .iter()
            .filter(|frontier| frontier.compare.is_some())
            .count(),
        analysis
            .frontiers
            .iter()
            .map(|frontier| frontier.requirement.label())
            .collect::<BTreeSet<_>>()
            .len(),
    );
    Ok(())
}

pub(super) fn write_quest_seq_flag_branch_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_quest_seq_flag_branch_analysis(source_path)?;
    let report = render_quest_seq_flag_branches_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST SEQ OPA1 flag branch TSV",
        analysis.branches.len(),
        &report,
    )?;
    let probe_addresses = analysis
        .branches
        .iter()
        .map(|branch| branch.probe.runtime_address)
        .collect::<BTreeSet<_>>();
    let suke = analysis
        .branches
        .iter()
        .find(|branch| {
            branch.sequence == "SUKE.SEQ"
                && branch.record_offset == BOX_QUEST_SUKE_CONFRONTATION_FLAG_RECORD_OFFSET
        })
        .expect("지원 원본 검증이 SUKE 첫 대화 OPA1을 보장함");
    println!(
        "  unique probe addresses {}, SUKE clear/set targets 0x{:04X}/0x{:04X}",
        probe_addresses.len(),
        suke.selected_offset(0),
        suke.selected_offset(suke.probe.mask),
    );
    Ok(())
}

pub(super) fn write_quest_seq_resource_transition_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_quest_seq_resource_transition_analysis(source_path)?;
    let report = render_quest_seq_resource_transitions_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST SEQ resource transition TSV",
        analysis.transitions.len(),
        &report,
    )?;
    println!(
        "  catalog {}, transitions {} / path occurrences {}, direct unique targets {}, \
         runtime-default {}, prefix descriptor rows {}",
        analysis.resource_catalog_entries,
        analysis.transitions.len(),
        analysis.path_occurrences,
        analysis.unique_target_resources,
        analysis.runtime_default_transitions,
        analysis.layout.prefix_descriptor_rows,
    );
    Ok(())
}

pub(super) fn write_quest_seq_rooted_text_run_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_quest_seq_text_stream_analysis(source_path)?;
    let report = render_quest_seq_rooted_text_runs_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST SEQ OP21-rooted text run TSV",
        analysis.rooted_runs.len(),
        &report,
    )?;
    println!(
        "  code-table candidates {}, candidates containing rooted runs {}, \
         exact matches {}, rooted runs {}, rooted code units {}, unique source characters {}",
        analysis.code_table_run_candidates,
        analysis.code_table_candidates_with_rooted_run,
        analysis.exact_code_table_candidate_matches,
        analysis.rooted_runs.len(),
        analysis.rooted_code_units,
        analysis.unique_source_characters,
    );
    Ok(())
}

pub(super) fn write_quest_seq_text_token_ledger(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_quest_seq_text_extraction_analysis(source_path)?;
    let report = render_quest_seq_text_token_ledger_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST SEQ token-preserving text ledger TSV",
        analysis.token_occurrences,
        &report,
    )?;
    println!(
        "  streams {}, glyph tokens {}, control tokens {}, roundtrip bytes {}",
        analysis.streams.len(),
        analysis.glyph_occurrences,
        analysis.control_occurrences,
        analysis.roundtrip_bytes,
    );
    Ok(())
}

pub(super) fn write_quest_seq_text_document_summary(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_quest_seq_text_document_analysis(source_path)?;
    let report = render_quest_seq_text_document_summary_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST SEQ lossless text document summary TSV",
        analysis.documents.len(),
        &report,
    )?;
    println!(
        "  chunks {}, text streams {} / {}B, opaque {} / {}B, full roundtrip {}B",
        analysis.chunks,
        analysis.text_stream_chunks,
        analysis.text_stream_bytes,
        analysis.opaque_chunks,
        analysis.opaque_bytes,
        analysis.source_bytes,
    );
    Ok(())
}
