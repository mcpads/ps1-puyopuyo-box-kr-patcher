//! scene-local E0/E1/E2 분석·합성 보고서.

use std::path::PathBuf;

use ps1_puyopuyo_box::analysis::{
    render_subgame_seq_scene_glyphs_tsv, render_subgame_seq_scene_messages_tsv,
};
use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::compress::decompress;
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::pipeline::subgame_seq_scene_glyph_patch::{
    render_subgame_seq_scene_glyph_patches_tsv, verify_box_subgame_seq_scene_glyph_patches,
};
use ps1_puyopuyo_box::pipeline::{
    render_subgame_seq_scene_relinks_tsv, render_subgame_seq_scene_translation_plan_tsv,
    verify_box_subgame_seq_scene_translation_manifest,
    verify_box_subgame_seq_scene_translation_plan,
};
use ps1_puyopuyo_box::script::BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS;

use crate::cli::analysis_inputs::{
    read_subgame_seq_scene_glyph_analysis, read_subgame_seq_scene_message_analysis,
    read_subgame_seq_scene_relink_analysis, read_subgame_seq_sources,
};
use crate::cli::translation_reports::write_report_file;
use crate::cli::usage::usage;

pub(in crate::cli) fn write_subgame_seq_scene_glyph_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_scene_glyph_analysis(source_path)?;
    let report = render_subgame_seq_scene_glyphs_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame scene-local E1/E2 glyph asset static analysis TSV",
        analysis.rows.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_scene_message_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_scene_message_analysis(source_path)?;
    let report = render_subgame_seq_scene_messages_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame scene-local E0 message static analysis TSV",
        analysis.rows.len(),
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_scene_relink_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_subgame_seq_scene_relink_analysis(source_path)?;
    let report = render_subgame_seq_scene_relinks_tsv(&analysis);
    write_report_file(
        output_path,
        "subgame scene-local E0 bounded relink static analysis TSV",
        analysis.blocks,
        &report,
    )
}

pub(in crate::cli) fn write_subgame_seq_scene_translation_plan_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let messages = read_subgame_seq_scene_message_analysis(source_path)?;
    let glyphs = read_subgame_seq_scene_glyph_analysis(source_path)?;
    let relinks = read_subgame_seq_scene_relink_analysis(source_path)?;
    let manifest = verify_box_subgame_seq_scene_translation_manifest(&messages)
        .map_err(|error| error.to_string())?;
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let seq_archives = BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS
        .iter()
        .map(|expectation| {
            let raw = iso
                .read_file(expectation.archive)
                .map_err(|error| error.to_string())?;
            let archive = VddArchive::parse(&raw)
                .map_err(|error| format!("{} VDD 파싱 실패: {error}", expectation.archive))?;
            Ok((expectation.archive, archive))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let sources = read_subgame_seq_sources(&seq_archives)?;
    let plan = verify_box_subgame_seq_scene_translation_plan(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &manifest,
        &messages,
        &glyphs,
        &relinks,
    )
    .map_err(|error| error.to_string())?;
    let report = render_subgame_seq_scene_translation_plan_tsv(&plan);
    write_report_file(
        output_path,
        "scene-local E0 Unicode/pair synthetic compiler TSV",
        plan.rows.len(),
        &report,
    )?;
    println!(
        "  families {}, physical messages {}, mappings {}, capacity/used/remaining {}/{}/{}, \
         BTX/runtime/final translation unverified",
        plan.families,
        plan.physical_messages,
        plan.glyph_mappings.len(),
        plan.capacity_bytes,
        plan.used_bytes,
        plan.remaining_bytes,
    );
    Ok(())
}

pub(in crate::cli) fn write_subgame_seq_scene_glyph_patch_report(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let messages = read_subgame_seq_scene_message_analysis(source_path)?;
    let glyphs = read_subgame_seq_scene_glyph_analysis(source_path)?;
    let relinks = read_subgame_seq_scene_relink_analysis(source_path)?;
    let manifest = verify_box_subgame_seq_scene_translation_manifest(&messages)
        .map_err(|error| error.to_string())?;
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let seq_archives = BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS
        .iter()
        .map(|expectation| {
            let raw = iso
                .read_file(expectation.archive)
                .map_err(|error| error.to_string())?;
            let archive = VddArchive::parse(&raw)
                .map_err(|error| format!("{} VDD 파싱 실패: {error}", expectation.archive))?;
            Ok((expectation.archive, archive))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let all_seq_sources = read_subgame_seq_sources(&seq_archives)?;
    let seq_sources = all_seq_sources
        .iter()
        .filter(|(archive, sequence, _)| {
            glyphs.rows.iter().any(|row| {
                row.scope == "PUYO1" && row.seq_archive == archive && row.sequence == sequence
            })
        })
        .map(|(archive, sequence, raw)| (archive.clone(), sequence.clone(), raw.clone()))
        .collect::<Vec<_>>();
    let plan = verify_box_subgame_seq_scene_translation_plan(
        all_seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &manifest,
        &messages,
        &glyphs,
        &relinks,
    )
    .map_err(|error| error.to_string())?;
    let grp_archive_path = "/PUYO1/PP1_GRP.VDD;1";
    let grp_archive_raw = iso
        .read_file(grp_archive_path)
        .map_err(|error| error.to_string())?;
    let grp_archive = VddArchive::parse(&grp_archive_raw)
        .map_err(|error| format!("{grp_archive_path} VDD 파싱 실패: {error}"))?;
    let assets = glyphs
        .rows
        .iter()
        .filter(|row| row.scope == "PUYO1")
        .map(|row| {
            let entry = grp_archive.find(row.same_stem_asset).ok_or_else(|| {
                format!(
                    "{}:{} scene glyph asset 누락",
                    row.grp_archive, row.same_stem_asset
                )
            })?;
            let plain = decompress(grp_archive.read_file(entry)).map_err(|error| {
                format!(
                    "{}:{} scene glyph 해제 실패: {error}",
                    row.grp_archive, row.same_stem_asset
                )
            })?;
            Ok((row.grp_archive, row.same_stem_asset, plain))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let analysis = verify_box_subgame_seq_scene_glyph_patches(
        assets
            .iter()
            .map(|(archive, asset, plain)| (*archive, *asset, plain.as_slice())),
        seq_sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &glyphs,
        &plan.glyph_mappings,
    )
    .map_err(|error| error.to_string())?;
    let report = render_subgame_seq_scene_glyph_patches_tsv(&analysis);
    write_report_file(
        output_path,
        "PUYO1 scene-local synthetic BTX cell compile TSV",
        analysis.rows.len(),
        &report,
    )?;
    println!(
        "  assets/cells {}/{}, source palette {:?}, changed pixels/bytes {}/{}, \
         runtime/final translation unverified",
        analysis.assets,
        analysis.cells,
        analysis.source_palette_indices,
        analysis.changed_pixels,
        analysis.changed_bytes,
    );
    Ok(())
}
