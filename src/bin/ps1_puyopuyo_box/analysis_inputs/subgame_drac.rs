//! DRAC text·relink·consumer·glyph 분석 입력을 조립한다.

use std::path::Path;

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::disasm::{
    verify_box_btx_cell_consumers, verify_box_btx_name_consumers,
    verify_box_subgame_seq_drac_consumers, verify_box_subgame_seq_drac_glyphs,
    SubgameSeqDracConsumerAnalysis, SubgameSeqDracGlyphAnalysis, BOX_BTX_NAME_CONSUMERS,
    BOX_SUBGAME_SEQ_DRAC_CONSUMER_SPECS, BOX_SUBGAME_SEQ_DRAC_GLYPH_SPECS,
};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::script::{
    verify_box_subgame_seq_drac_relinks, verify_box_subgame_seq_drac_text,
    verify_box_subgame_seq_layouts, verify_box_subgame_seq_text_candidates,
    SubgameSeqDracRelinkAnalysis, SubgameSeqDracTextAnalysis, BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS,
};

use crate::cli::disc_commands::read_box_executables;

use super::source_files::read_subgame_seq_sources;

pub(in crate::cli) fn read_subgame_seq_drac_text_analysis(
    source_path: &Path,
) -> Result<SubgameSeqDracTextAnalysis, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let archives = BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS
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
    let sources = read_subgame_seq_sources(&archives)?;
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
    verify_box_subgame_seq_drac_text(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &text,
        &layouts,
    )
    .map_err(|error| error.to_string())
}

pub(in crate::cli) fn read_subgame_seq_drac_relink_analysis(
    source_path: &Path,
) -> Result<SubgameSeqDracRelinkAnalysis, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let archives = BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS
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
    let sources = read_subgame_seq_sources(&archives)?;
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
    let drac_text = verify_box_subgame_seq_drac_text(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &text,
        &layouts,
    )
    .map_err(|error| error.to_string())?;
    verify_box_subgame_seq_drac_relinks(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &drac_text,
        &layouts,
    )
    .map_err(|error| error.to_string())
}

pub(in crate::cli) fn read_subgame_seq_drac_consumer_analysis(
    source_path: &Path,
) -> Result<SubgameSeqDracConsumerAnalysis, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let archives = BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS
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
    let sources = read_subgame_seq_sources(&archives)?;
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
    let drac_text = verify_box_subgame_seq_drac_text(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &text,
        &layouts,
    )
    .map_err(|error| error.to_string())?;
    verify_box_subgame_seq_drac_consumers(
        executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_DRAC_CONSUMER_SPECS
                    .iter()
                    .any(|spec| spec.exe == exe.map.name)
            })
            .map(|exe| (exe.map, exe.text.as_slice())),
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &drac_text,
    )
    .map_err(|error| error.to_string())
}

pub(in crate::cli) fn read_subgame_seq_drac_glyph_analysis(
    source_path: &Path,
) -> Result<SubgameSeqDracGlyphAnalysis, String> {
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
    let drac_text = verify_box_subgame_seq_drac_text(
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
    let btx_cells =
        verify_box_btx_cell_consumers(executables.iter().map(|exe| (exe.map, exe.text.as_slice())))
            .map_err(|error| error.to_string())?;
    verify_box_subgame_seq_drac_glyphs(
        executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_DRAC_GLYPH_SPECS
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
                BOX_SUBGAME_SEQ_DRAC_GLYPH_SPECS
                    .iter()
                    .any(|spec| spec.grp_archive == *archive_id)
            })
            .map(|(archive_id, archive)| (*archive_id, archive)),
        &drac_text,
        &btx_names,
        &btx_cells,
    )
    .map_err(|error| error.to_string())
}
