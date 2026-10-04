//! 실행 파일과 서브게임 SEQ 분석 보고서를 역할별 module로 연결한다.

use std::path::PathBuf;

use ps1_puyopuyo_box::disasm::{
    render_bios_font_analysis_tsv, render_btx_cell_consumers_tsv,
    render_common_font_draw_sites_tsv, render_exe_string_candidates_tsv,
    render_executable_trace_tsv, verify_box_bios_font_routes, verify_box_btx_cell_consumers,
    verify_box_common_font_draw_sites, verify_box_exe_string_candidates,
    verify_box_executable_traces,
};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};

use super::disc_commands::read_box_executables;
use super::translation_reports::write_report_file;
use super::usage::usage;

mod subgame_drac;
mod subgame_e6;
mod subgame_resource_routes;
mod subgame_scene;
mod subgame_text;

pub(super) use subgame_drac::{
    write_subgame_seq_drac_consumer_report, write_subgame_seq_drac_glyph_report,
    write_subgame_seq_drac_ram_report, write_subgame_seq_drac_relink_report,
    write_subgame_seq_drac_text_report,
};
pub(super) use subgame_e6::{
    write_subgame_seq_e6_glyph_report, write_subgame_seq_e6_ram_report,
    write_subgame_seq_e6_text_report,
};
pub(super) use subgame_resource_routes::{
    write_subgame_seq_descriptor_report, write_subgame_seq_layout_report,
    write_subgame_seq_load_report, write_subgame_seq_registration_report,
    write_subgame_seq_selection_report,
};
pub(super) use subgame_scene::{
    write_subgame_seq_scene_glyph_patch_report, write_subgame_seq_scene_glyph_report,
    write_subgame_seq_scene_message_report, write_subgame_seq_scene_relink_report,
    write_subgame_seq_scene_translation_plan_report,
};
pub(super) use subgame_text::{
    write_subgame_seq_control_report, write_subgame_seq_control_target_report,
    write_subgame_seq_min_text_report, write_subgame_seq_overlay_call_argument_report,
    write_subgame_seq_text_bridge_call_reports, write_subgame_seq_text_bridge_ram_report,
    write_subgame_seq_text_bridge_report, write_subgame_seq_text_candidate_report,
    write_subgame_seq_text_root_report, write_subgame_seq_text_summary_report,
    write_subgame_text_consumer_report,
};

pub(super) fn write_exe_trace_report(
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
    let summaries =
        verify_box_executable_traces(executables.iter().map(|exe| (exe.map, exe.text.as_slice())))
            .map_err(|error| error.to_string())?;
    let report = render_executable_trace_tsv(&summaries);
    write_report_file(
        output_path,
        "EXE recursive trace TSV",
        summaries.len(),
        &report,
    )
}

pub(super) fn write_bios_font_report(
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
    let analysis =
        verify_box_bios_font_routes(executables.iter().map(|exe| (exe.map, exe.text.as_slice())))
            .map_err(|error| error.to_string())?;
    let report = render_bios_font_analysis_tsv(&analysis);
    write_report_file(
        output_path,
        "BIOS font static analysis TSV",
        analysis.routes.len() + analysis.callsite_similarities.len(),
        &report,
    )
}

pub(super) fn write_btx_cell_report(
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
    let analysis =
        verify_box_btx_cell_consumers(executables.iter().map(|exe| (exe.map, exe.text.as_slice())))
            .map_err(|error| error.to_string())?;
    let report = render_btx_cell_consumers_tsv(&analysis);
    write_report_file(
        output_path,
        "BTX cell consumer static analysis TSV",
        analysis.consumers.len(),
        &report,
    )
}

pub(super) fn write_common_font_draw_site_report(
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
    let cells = verify_box_btx_cell_consumers(
        executables
            .iter()
            .map(|executable| (executable.map, executable.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let analysis = verify_box_common_font_draw_sites(
        executables
            .iter()
            .map(|executable| (executable.map, executable.text.as_slice())),
        &cells,
    )
    .map_err(|error| error.to_string())?;
    let report = render_common_font_draw_sites_tsv(&analysis);
    write_report_file(
        output_path,
        "COM_FONT draw capture sites TSV",
        analysis.sites.len(),
        &report,
    )
}

pub(super) fn write_exe_string_report(
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
    let analysis = verify_box_exe_string_candidates(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let report = render_exe_string_candidates_tsv(&analysis);
    write_report_file(
        output_path,
        "EXE string candidate TSV",
        analysis.candidates.len(),
        &report,
    )
}
