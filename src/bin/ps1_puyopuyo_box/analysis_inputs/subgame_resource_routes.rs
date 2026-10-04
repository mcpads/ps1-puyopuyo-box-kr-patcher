//! 서브게임 SEQ descriptor·registration·selection·load 경로 입력을 조립한다.

use std::path::Path;

use ps1_puyopuyo_box::archive::VddArchive;
use ps1_puyopuyo_box::disasm::{
    verify_box_subgame_seq_descriptors, verify_box_subgame_seq_loads,
    verify_box_subgame_seq_registrations, verify_box_subgame_seq_selections,
    SubgameSeqDescriptorAnalysis, SubgameSeqLoadAnalysis, SubgameSeqRegistrationAnalysis,
    SubgameSeqSelectionAnalysis, BOX_SUBGAME_SEQ_DESCRIPTOR_SPECS, BOX_SUBGAME_SEQ_LOAD_SPECS,
    BOX_SUBGAME_SEQ_REGISTRATION_SPECS, BOX_SUBGAME_SEQ_SELECTION_DISPATCHERS,
};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::script::BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS;

use crate::cli::disc_commands::read_box_executables;

use super::source_files::read_subgame_seq_sources;
use super::subgame_text::read_subgame_seq_layout_analysis;

pub(in crate::cli) fn read_subgame_seq_descriptor_analysis(
    source_path: &Path,
) -> Result<SubgameSeqDescriptorAnalysis, String> {
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let archives = BOX_SUBGAME_SEQ_DESCRIPTOR_SPECS
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
    verify_box_subgame_seq_descriptors(
        executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_DESCRIPTOR_SPECS
                    .iter()
                    .any(|spec| spec.exe == exe.map.name)
            })
            .map(|exe| (exe.map, exe.text.as_slice())),
        archives.iter().map(|(archive, vdd)| (*archive, vdd)),
    )
    .map_err(|error| error.to_string())
}

pub(in crate::cli) fn read_subgame_seq_registration_analysis(
    source_path: &Path,
) -> Result<SubgameSeqRegistrationAnalysis, String> {
    let descriptors = read_subgame_seq_descriptor_analysis(source_path)?;
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    verify_box_subgame_seq_registrations(
        executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_REGISTRATION_SPECS
                    .iter()
                    .any(|spec| spec.exe == exe.map.name)
            })
            .map(|exe| (exe.map, exe.text.as_slice())),
        &descriptors,
    )
    .map_err(|error| error.to_string())
}

pub(in crate::cli) fn read_subgame_seq_selection_analysis(
    source_path: &Path,
) -> Result<SubgameSeqSelectionAnalysis, String> {
    let registrations = read_subgame_seq_registration_analysis(source_path)?;
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
    verify_box_subgame_seq_selections(
        executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_SELECTION_DISPATCHERS
                    .iter()
                    .any(|spec| spec.exe == exe.map.name)
            })
            .map(|exe| (exe.map, exe.text.as_slice())),
        sources
            .iter()
            .map(|(archive, scene, raw)| (archive.as_str(), scene.as_str(), raw.as_slice())),
        &registrations,
    )
    .map_err(|error| error.to_string())
}

pub(in crate::cli) fn read_subgame_seq_load_analysis(
    source_path: &Path,
) -> Result<SubgameSeqLoadAnalysis, String> {
    let selections = read_subgame_seq_selection_analysis(source_path)?;
    let layouts = read_subgame_seq_layout_analysis(source_path)?;
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
    verify_box_subgame_seq_loads(
        executables
            .iter()
            .filter(|exe| {
                BOX_SUBGAME_SEQ_LOAD_SPECS
                    .iter()
                    .any(|spec| spec.exe == exe.map.name)
            })
            .map(|exe| (exe.map, exe.text.as_slice())),
        sources
            .iter()
            .map(|(archive, scene, raw)| (archive.as_str(), scene.as_str(), raw.as_slice())),
        &selections,
        &layouts,
    )
    .map_err(|error| error.to_string())
}
