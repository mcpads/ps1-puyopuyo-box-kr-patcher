//! 서브게임 SEQ layout·descriptor·registration·selection·load 보고서.

use std::path::PathBuf;

use ps1_puyopuyo_box::disasm::{
    render_subgame_seq_descriptors_tsv, render_subgame_seq_loads_tsv,
    render_subgame_seq_registrations_tsv, render_subgame_seq_selections_tsv,
};
use ps1_puyopuyo_box::script::render_subgame_seq_layouts_tsv;

use crate::cli::analysis_inputs::{
    read_subgame_seq_descriptor_analysis, read_subgame_seq_layout_analysis,
    read_subgame_seq_load_analysis, read_subgame_seq_registration_analysis,
    read_subgame_seq_selection_analysis,
};
use crate::cli::translation_reports::write_report_file;
use crate::cli::usage::usage;

pub(in crate::cli) fn write_subgame_seq_layout_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_layout_analysis(source_path)?;
    let report = render_subgame_seq_layouts_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame SEQ overlay/raw layout TSV",
        analysis.rows.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_descriptor_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_descriptor_analysis(source_path)?;
    let report = render_subgame_seq_descriptors_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame SEQ descriptor static analysis TSV",
        analysis.rows.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_registration_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_registration_analysis(source_path)?;
    let report = render_subgame_seq_registrations_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame SEQ registration static analysis TSV",
        analysis.rows.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_selection_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_selection_analysis(source_path)?;
    let report = render_subgame_seq_selections_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame SEQ scene selection static analysis TSV",
        analysis.rows.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_load_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_load_analysis(source_path)?;
    let report = render_subgame_seq_loads_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame SEQ CNX load static analysis TSV",
        analysis.rows.len(),
        &report,
    )
}
