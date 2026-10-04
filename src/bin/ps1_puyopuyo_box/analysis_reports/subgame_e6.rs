//! 서브게임 E6 text·glyph 정적 보고서와 PUYO2 RAM 관측.

use std::fs;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::compress::decompress;
use ps1_puyopuyo_box::disasm::{render_subgame_seq_e6_glyphs_tsv, render_subgame_seq_e6_text_tsv};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::runtime::{
    inspect_puyo2_e6_ram, render_puyo2_e6_ram_tsv, Puyo2E6RuntimeSource,
};

use crate::cli::analysis_inputs::{
    read_subgame_seq_e6_glyph_analysis, read_subgame_seq_e6_text_analysis,
};
use crate::cli::translation_reports::write_report_file;
use crate::cli::usage::usage;

pub(in crate::cli) fn write_subgame_seq_e6_text_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_e6_text_analysis(source_path)?;
    let report = render_subgame_seq_e6_text_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame E6 direct text and consumer TSV",
        analysis.rows.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_e6_glyph_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_e6_glyph_analysis(source_path)?;
    let report = render_subgame_seq_e6_glyphs_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame E6 glyph cell strategy TSV",
        analysis.rows.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_e6_ram_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, ram_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_e6_glyph_analysis(source_path)?;
    let p2 = analysis
        .rows
        .iter()
        .find(|row| row.scope == "PUYO2")
        .ok_or_else(|| "PUYO2 E6 glyph 분석 행이 없음".to_string())?;

    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let vdd_raw = iso
        .read_file(p2.archive)
        .map_err(|error| error.to_string())?;
    let archive = VddArchive::parse(&vdd_raw)
        .map_err(|error| format!("{} VDD 파싱 실패: {error}", p2.archive))?;
    let payloads = p2
        .e6_sequence_storage_links
        .iter()
        .map(|link| {
            let entry = archive.entries().get(link.vdd_index).ok_or_else(|| {
                format!("{} E6 VDD index 범위 이탈: {}", p2.archive, link.vdd_index)
            })?;
            if entry.name() != link.sequence || entry.size as usize != link.stored_bytes {
                return Err(format!(
                    "{}:{} E6 VDD 원본 계약 불일치",
                    p2.archive, link.sequence
                ));
            }
            let raw = decompress(archive.read_file(entry)).map_err(|error| {
                format!("{}:{} CNX 해제 실패: {error}", p2.archive, link.sequence)
            })?;
            if raw.len() != link.plain_bytes
                || <[u8; 32]>::from(Sha256::digest(&raw)) != link.sequence_sha256
            {
                return Err(format!(
                    "{}:{} E6 해제 원본 계약 불일치",
                    p2.archive, link.sequence
                ));
            }
            Ok((link, raw))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let ram =
        fs::read(ram_path).map_err(|error| format!("{} 읽기 실패: {error}", ram_path.display()))?;
    let observation = inspect_puyo2_e6_ram(
        &ram,
        payloads.iter().map(|(link, raw)| Puyo2E6RuntimeSource {
            resource: link.resource,
            sequence: &link.sequence,
            raw,
            sequence_sha256: link.sequence_sha256,
            load_base: link.load_base,
            initialized_bytes: link.initialized_bytes,
            bss_bytes: link.bss_bytes,
        }),
        &analysis.common_used_cell_index_values,
    )
    .map_err(|error| error.to_string())?;
    let report = render_puyo2_e6_ram_tsv(&observation);
    write_report_file(
        output_path,
        "PUYO2 E6 overlay/descriptor/glyph raw RAM observation TSV",
        1,
        &report,
    )?;
    println!(
        "  source {}, best match {}, exact overlays {}, callback/exact glyph {}/{}, \
         current records {}, {}",
        observation.source_sequences,
        observation.best_matching_bytes,
        observation.exact_overlay_matches.len(),
        observation.pool_callback_candidates,
        observation.exact_glyph_candidates,
        observation.current_record_candidates.len(),
        observation.observation_status(),
    );
    Ok(())
}
