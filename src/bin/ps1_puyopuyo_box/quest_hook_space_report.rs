//! QUEST hook 설치 공간 report 명령.

use std::path::PathBuf;

use ps1_puyopuyo_box::disasm::quest_hook_candidates::{
    render_quest_hook_space_candidates_tsv, verify_box_quest_hook_space_candidates,
};
use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};

use super::disc_commands::read_box_executables;
use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn write_quest_hook_space_report(
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
    let analysis = verify_box_quest_hook_space_candidates(quest.map, &quest.text)
        .map_err(|error| error.to_string())?;
    let report = render_quest_hook_space_candidates_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST hook zero-run candidate TSV",
        analysis.candidates.len(),
        &report,
    )?;
    println!(
        "  zero bytes {}, entry-reached {}, referenced {}, unreferenced runtime-pending {}, \
         open indirect sites {}, analysis SHA-256 {}",
        analysis.total_zero_bytes,
        analysis.entry_reached_candidates,
        analysis.referenced_candidates,
        analysis.unreferenced_candidates,
        analysis.indirect_sites,
        analysis
            .analysis_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );
    Ok(())
}
