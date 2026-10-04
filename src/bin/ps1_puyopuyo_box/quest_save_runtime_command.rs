//! QUEST memory card 저장 결과를 검증하는 명령.

use std::fs;
use std::path::PathBuf;

use ps1_puyopuyo_box::pipeline::{
    render_quest_save_card_runtime_tsv,
    verify_quest_save_card_runtime as verify_quest_save_card_runtime_state,
};

use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn verify_quest_save_card_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [image_path, blank_card_path, saved_card_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let image =
        fs::read(image_path).map_err(|error| format!("{}: {error}", image_path.display()))?;
    let blank_card = fs::read(blank_card_path)
        .map_err(|error| format!("{}: {error}", blank_card_path.display()))?;
    let saved_card = fs::read(saved_card_path)
        .map_err(|error| format!("{}: {error}", saved_card_path.display()))?;
    let report = verify_quest_save_card_runtime_state(&image, &blank_card, &saved_card)
        .map_err(|error| error.to_string())?;
    let rendered = render_quest_save_card_runtime_tsv(&report);
    write_report_file(output_path, "QUEST save card runtime TSV", 1, &rendered)?;
    println!(
        "PASS QUEST persistent save card: block {}, status 0x{:02X}, {}B, {}, \
         data 0x{:X}, card SHA-256 {}, image SHA-256 {}; cold load는 별도 runtime 증거",
        report.directory_block,
        report.directory_status,
        report.declared_save_bytes,
        report.filename,
        report.data_block_offset,
        report.saved_card_sha256,
        report.image_sha256,
    );
    Ok(())
}
