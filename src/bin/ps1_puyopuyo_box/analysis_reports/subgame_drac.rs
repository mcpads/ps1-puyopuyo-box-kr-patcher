//! DRAC text·consumer·glyph·relink 정적 보고서와 PUYO2 RAM 관측.

use std::fs;
use std::path::PathBuf;

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::compress::decompress;
use ps1_puyopuyo_box::disasm::{
    render_subgame_seq_drac_consumers_tsv, render_subgame_seq_drac_glyphs_tsv,
};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::runtime::{inspect_puyo2_drac_ram, render_puyo2_drac_ram_tsv};
use ps1_puyopuyo_box::script::{
    render_subgame_seq_drac_relinks_tsv, render_subgame_seq_drac_text_tsv,
};

use crate::cli::analysis_inputs::{
    read_subgame_seq_drac_consumer_analysis, read_subgame_seq_drac_glyph_analysis,
    read_subgame_seq_drac_relink_analysis, read_subgame_seq_drac_text_analysis,
};
use crate::cli::translation_reports::write_report_file;
use crate::cli::usage::usage;

pub(in crate::cli) fn write_subgame_seq_drac_text_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_drac_text_analysis(source_path)?;
    let report = render_subgame_seq_drac_text_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame DRAC repertoire/message boundary TSV",
        analysis.rows.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_drac_consumer_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_drac_consumer_analysis(source_path)?;
    let report = render_subgame_seq_drac_consumers_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame DRAC record consumer static analysis TSV",
        analysis.rows.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_drac_ram_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, ram_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let _consumer_analysis = read_subgame_seq_drac_consumer_analysis(source_path)?;
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let vdd_raw = iso
        .read_file("/PUYO2/PP2_SEQ.VDD;1")
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&vdd_raw)
        .map_err(|error| format!("/PUYO2/PP2_SEQ.VDD;1 VDD 파싱 실패: {error}"))?;
    let entry = archive
        .find("DRAC_SEQ.SEQ")
        .ok_or_else(|| "/PUYO2/PP2_SEQ.VDD;1:DRAC_SEQ.SEQ가 없음".to_string())?;
    let sequence = decompress(archive.read_file(entry))
        .map_err(|error| format!("PUYO2 DRAC_SEQ.SEQ 해제 실패: {error}"))?;
    let ram =
        fs::read(ram_path).map_err(|error| format!("{} 읽기 실패: {error}", ram_path.display()))?;
    let observation = inspect_puyo2_drac_ram(&ram, &sequence).map_err(|error| error.to_string())?;
    let report = render_puyo2_drac_ram_tsv(&observation);
    write_report_file(
        output_path,
        "PUYO2 DRAC overlay/message/glyph raw RAM observation TSV",
        1,
        &report,
    )?;
    println!(
        "  overlay exact {}, matching {}/{}, message callback/candidates {}/{}, \
         glyph callback/direct/same-named {}/{}/{}, globals main/aux {}/{}, \
         exact E0 candidates {}, {}",
        observation.overlay_initialized_exact,
        observation.overlay_matching_bytes,
        ps1_puyopuyo_box::runtime::PUYO2_DRAC_INITIALIZED_BYTES,
        observation.callback_occurrences,
        observation.message_candidates.len(),
        observation.glyph_callback_occurrences,
        observation.exact_main_glyph_candidates,
        observation.exact_same_named_glyph_candidates,
        observation.main_resource_global,
        observation.auxiliary_resource_global,
        observation.exact_record_candidates,
        observation.observation_status(),
    );
    Ok(())
}

pub(in crate::cli) fn write_subgame_seq_drac_glyph_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_drac_glyph_analysis(source_path)?;
    let report = render_subgame_seq_drac_glyphs_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame DRAC glyph asset static analysis TSV",
        analysis.rows.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_drac_relink_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_drac_relink_analysis(source_path)?;
    let report = render_subgame_seq_drac_relinks_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame DRAC message relink static analysis TSV",
        analysis.rows.len(),
        &report,
    )
}
