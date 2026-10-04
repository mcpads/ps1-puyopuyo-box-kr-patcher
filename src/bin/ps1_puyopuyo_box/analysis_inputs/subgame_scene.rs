//! scene E0 message·glyph·relink 분석 입력을 조립한다.

use std::path::Path;

use ps1_puyopuyo_box::analysis::{
    verify_box_subgame_seq_scene_glyphs, verify_box_subgame_seq_scene_messages,
    SubgameSeqSceneGlyphAnalysis, SubgameSeqSceneMessageAnalysis,
    BOX_SUBGAME_SEQ_SCENE_GLYPH_SCOPES,
};
use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::disasm::{verify_box_btx_name_consumers, BOX_BTX_NAME_CONSUMERS};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::pipeline::{
    verify_box_subgame_seq_scene_relinks, SubgameSeqSceneRelinkAnalysis,
};
use ps1_puyopuyo_box::script::{
    verify_box_subgame_seq_layouts, verify_box_subgame_seq_text_candidates,
    verify_box_subgame_seq_text_roots, BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS,
};

use crate::cli::disc_commands::read_box_executables;

use super::source_files::read_subgame_seq_sources;
use super::subgame_drac::{
    read_subgame_seq_drac_consumer_analysis, read_subgame_seq_drac_glyph_analysis,
};

pub(in crate::cli) fn read_subgame_seq_scene_glyph_analysis(
    source_path: &Path,
) -> Result<SubgameSeqSceneGlyphAnalysis, String> {
    let drac_glyphs = read_subgame_seq_drac_glyph_analysis(source_path)?;
    let drac_consumers = read_subgame_seq_drac_consumer_analysis(source_path)?;
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
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
    let text = verify_box_subgame_seq_text_candidates(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let layouts = verify_box_subgame_seq_layouts(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &text,
    )
    .map_err(|error| error.to_string())?;
    let roots = verify_box_subgame_seq_text_roots(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &text,
        &layouts,
    )
    .map_err(|error| error.to_string())?;
    let grp_archives = BOX_BTX_NAME_CONSUMERS
        .iter()
        .map(|spec| {
            let raw = iso
                .read_file(spec.archive)
                .map_err(|error| error.to_string())?;
            let archive = VddArchive::parse(&raw)
                .map_err(|error| format!("{} VDD 파싱 실패: {error}", spec.archive))?;
            Ok((spec.archive, archive))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let btx_names = verify_box_btx_name_consumers(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        grp_archives
            .iter()
            .map(|(archive_id, archive)| (*archive_id, archive)),
    )
    .map_err(|error| error.to_string())?;
    verify_box_subgame_seq_scene_glyphs(
        executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_SCENE_GLYPH_SCOPES
                    .iter()
                    .any(|spec| spec.exe == exe.map.name)
            })
            .map(|exe| (exe.map, exe.text.as_slice())),
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        grp_archives
            .iter()
            .filter(|(archive_id, _)| {
                BOX_SUBGAME_SEQ_SCENE_GLYPH_SCOPES
                    .iter()
                    .any(|spec| spec.grp_archive == *archive_id)
            })
            .map(|(archive_id, archive)| (*archive_id, archive)),
        &layouts,
        &roots,
        &btx_names,
        (&drac_consumers, &drac_glyphs),
    )
    .map_err(|error| error.to_string())
}

pub(in crate::cli) fn read_subgame_seq_scene_message_analysis(
    source_path: &Path,
) -> Result<SubgameSeqSceneMessageAnalysis, String> {
    let glyphs = read_subgame_seq_scene_glyph_analysis(source_path)?;
    let consumers = read_subgame_seq_drac_consumer_analysis(source_path)?;
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
    let text = verify_box_subgame_seq_text_candidates(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let layouts = verify_box_subgame_seq_layouts(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &text,
    )
    .map_err(|error| error.to_string())?;
    verify_box_subgame_seq_scene_messages(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &layouts,
        &glyphs,
        &consumers,
    )
    .map_err(|error| error.to_string())
}

pub(in crate::cli) fn read_subgame_seq_scene_relink_analysis(
    source_path: &Path,
) -> Result<SubgameSeqSceneRelinkAnalysis, String> {
    let messages = read_subgame_seq_scene_message_analysis(source_path)?;
    let glyphs = read_subgame_seq_scene_glyph_analysis(source_path)?;
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
    let text = verify_box_subgame_seq_text_candidates(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let layouts = verify_box_subgame_seq_layouts(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &text,
    )
    .map_err(|error| error.to_string())?;
    verify_box_subgame_seq_scene_relinks(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &layouts,
        &messages,
        &glyphs,
    )
    .map_err(|error| error.to_string())
}
