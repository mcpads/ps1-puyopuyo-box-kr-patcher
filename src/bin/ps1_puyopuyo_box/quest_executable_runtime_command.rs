//! QUEST executable relocation의 runtime 적재 증거를 검증하는 명령.

use std::fs;
use std::path::PathBuf;

use ps1_puyopuyo_box::disc::VerifiedSourceImage;
use ps1_puyopuyo_box::pipeline::{
    render_quest_exe_relocation_runtime_tsv, verify_quest_exe_relocation_control_ram,
};

use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn verify_quest_exe_relocation_control_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, ram_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let ram = fs::read(ram_path).map_err(|error| format!("{}: {error}", ram_path.display()))?;
    let report = verify_quest_exe_relocation_control_ram(&source, &ram)
        .map_err(|error| error.to_string())?;
    let rendered = render_quest_exe_relocation_runtime_tsv(&report);
    write_report_file(
        output_path,
        "QUEST EXE relocation runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST EXE relocation runtime: load 0x{:08X}, original {}B + \
         extension {}B = {}B, text exact {}, extension zero {}, text SHA-256 {}, \
         RAM SHA-256 {}, image SHA-256 {}",
        report.load_addr,
        report.original_text_bytes,
        report.extension_bytes,
        report.loaded_text_bytes,
        report.loaded_text_exact,
        report.extension_zero,
        report.loaded_text_sha256,
        report.ram_sha256,
        report.image_sha256,
    );
    Ok(())
}
