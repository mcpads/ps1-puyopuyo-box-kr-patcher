//! 공통 서브게임 SEQ text·layout·root 분석 입력을 조립한다.

use std::path::Path;

use ps1_puyopuyo_box::analysis::{
    verify_box_subgame_seq_overlay_call_arguments, SubgameSeqOverlayCallArgumentAnalysis,
};
use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::compress::{decompress, is_cnx};
use ps1_puyopuyo_box::disasm::{
    verify_box_subgame_seq_control_targets, verify_box_subgame_seq_controls,
    verify_box_subgame_seq_text_bridges, verify_box_subgame_text_consumers,
    SubgameControlTargetAnalysis, SubgameSeqTextBridgeAnalysis,
};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::script::{
    verify_box_menu06_text, verify_box_subgame_seq_layouts, verify_box_subgame_seq_min_text,
    verify_box_subgame_seq_text_candidates, verify_box_subgame_seq_text_roots, Menu06TextAnalysis,
    SubgameSeqLayoutAnalysis, SubgameSeqSceneTextAnalysis, SubgameSeqTextCandidateAnalysis,
    SubgameSeqTextRootAnalysis, BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS,
};

use crate::cli::disc_commands::read_box_executables;
use crate::cli::BoxExecutable;

use super::source_files::read_subgame_seq_sources;

pub(in crate::cli) fn read_menu06_text_analysis(
    source_path: &Path,
) -> Result<Menu06TextAnalysis, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let mut sources = Vec::new();
    for archive_path in ["/PUYO1/PP1_SEQ.VDD;1", "/PUYO2/PP2_SEQ.VDD;1"] {
        let raw = iso
            .read_file(archive_path)
            .map_err(|error| error.to_string())?;
        let archive = VddArchive::parse(&raw)
            .map_err(|error| format!("{archive_path} VDD 파싱 실패: {error}"))?;
        let entry = archive
            .find("MENU1.SEQ")
            .ok_or_else(|| format!("{archive_path}:MENU1.SEQ가 없음"))?;
        let cnx = archive.read_file(entry);
        if !is_cnx(cnx) {
            return Err(format!("{archive_path}:MENU1.SEQ가 CNX v2가 아님"));
        }
        let plain =
            decompress(cnx).map_err(|error| format!("{archive_path}:MENU1.SEQ: {error}"))?;
        sources.push((archive_path, "MENU1.SEQ", plain));
    }
    verify_box_menu06_text(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (*archive, *sequence, raw.as_slice())),
    )
    .map_err(|error| error.to_string())
}

pub(in crate::cli) struct SubgameSeqTextBridgeSource {
    pub analysis: SubgameSeqTextBridgeAnalysis,
    pub executables: Vec<BoxExecutable>,
}

pub(in crate::cli) fn read_subgame_seq_text_bridge_source(
    source_path: &Path,
) -> Result<SubgameSeqTextBridgeSource, String> {
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
    let consumers = verify_box_subgame_text_consumers(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let analysis = verify_box_subgame_seq_text_bridges(
        executables.iter().map(|exe| (exe.map, exe.text.as_slice())),
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &consumers,
    )
    .map_err(|error| error.to_string())?;
    Ok(SubgameSeqTextBridgeSource {
        analysis,
        executables,
    })
}

pub(in crate::cli) fn read_subgame_seq_text_bridge_analysis(
    source_path: &Path,
) -> Result<SubgameSeqTextBridgeAnalysis, String> {
    Ok(read_subgame_seq_text_bridge_source(source_path)?.analysis)
}

pub(in crate::cli) fn read_subgame_seq_overlay_call_argument_analysis(
    source_path: &Path,
) -> Result<SubgameSeqOverlayCallArgumentAnalysis, String> {
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
    verify_box_subgame_seq_overlay_call_arguments(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())
}

pub(in crate::cli) fn read_subgame_seq_control_target_analysis(
    source_path: &Path,
) -> Result<SubgameControlTargetAnalysis, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let selected = executables
        .iter()
        .filter(|exe| matches!(exe.map.name, "PCOMI.EXE" | "PUYO1.EXE" | "PUYO2.EXE"))
        .collect::<Vec<_>>();
    let consumers = verify_box_subgame_text_consumers(
        selected.iter().map(|exe| (exe.map, exe.text.as_slice())),
    )
    .map_err(|error| error.to_string())?;
    let controls = verify_box_subgame_seq_controls(
        selected.iter().map(|exe| (exe.map, exe.text.as_slice())),
        &consumers,
    )
    .map_err(|error| error.to_string())?;
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
    verify_box_subgame_seq_control_targets(
        selected.iter().map(|exe| (exe.map, exe.text.as_slice())),
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &controls,
    )
    .map_err(|error| error.to_string())
}

pub(in crate::cli) fn read_subgame_seq_text_analysis(
    source_path: &Path,
) -> Result<SubgameSeqTextCandidateAnalysis, String> {
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
    verify_box_subgame_seq_text_candidates(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
    )
    .map_err(|error| error.to_string())
}

pub(in crate::cli) fn read_subgame_seq_layout_analysis(
    source_path: &Path,
) -> Result<SubgameSeqLayoutAnalysis, String> {
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
    verify_box_subgame_seq_layouts(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &text,
    )
    .map_err(|error| error.to_string())
}

pub(in crate::cli) fn read_subgame_seq_text_root_analysis(
    source_path: &Path,
) -> Result<SubgameSeqTextRootAnalysis, String> {
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
    verify_box_subgame_seq_text_roots(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &text,
        &layouts,
    )
    .map_err(|error| error.to_string())
}

pub(in crate::cli) fn read_subgame_seq_min_text_analysis(
    source_path: &Path,
) -> Result<SubgameSeqSceneTextAnalysis, String> {
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
    let roots = verify_box_subgame_seq_text_roots(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &text,
        &layouts,
    )
    .map_err(|error| error.to_string())?;
    verify_box_subgame_seq_min_text(
        sources
            .iter()
            .map(|(archive, sequence, raw)| (archive.as_str(), sequence.as_str(), raw.as_slice())),
        &text,
        &layouts,
        &roots,
    )
    .map_err(|error| error.to_string())
}
