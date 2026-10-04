//! QUEST atlas bank bounded VRAM 점유 검증 명령.

use std::fs;
use std::path::PathBuf;

use ps1_puyopuyo_box::disc::VerifiedSourceImage;
use ps1_puyopuyo_box::pipeline::{
    render_quest_vram_bank_ownership_runtime_tsv, verify_quest_vram_bank_ownership_runtime,
};

use super::development_disc_inputs::read_current_development_disc_inputs;
use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn verify_quest_vram_bank_ownership_runtime_command(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, ram_path, gpu_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let ram = fs::read(ram_path).map_err(|error| format!("{}: {error}", ram_path.display()))?;
    let gpu = fs::read(gpu_path).map_err(|error| format!("{}: {error}", gpu_path.display()))?;
    let inputs = read_current_development_disc_inputs(source_path, &source)?;
    let report = verify_quest_vram_bank_ownership_runtime(
        &inputs.build,
        &inputs.package,
        &inputs.hook,
        &ram,
        &gpu,
    )
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_vram_bank_ownership_runtime_tsv(&report);
    write_report_file(
        output_path,
        "QUEST VRAM bank ownership runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST bounded VRAM bank occupancy: state {:02X?}, primary {}/{} nonzero words, \
         secondary {}/{} nonzero words, exact installation {}, resident cells {}, ownership {}, \
         draw {}, release {}",
        report.page_state,
        report.primary.nonzero_words,
        report.primary.width_words * report.primary.height_rows,
        report.secondary.nonzero_words,
        report.secondary.width_words * report.secondary.height_rows,
        report.resident_installation_exact,
        report.resident_cells_exact,
        report.ownership_verified,
        report.draw_verified,
        report.release_eligible,
    );
    Ok(())
}
