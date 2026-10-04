//! QUEST glyph cache secondary bank runtime 검증 명령.

use std::fs;
use std::path::PathBuf;

use ps1_puyopuyo_box::disc::VerifiedSourceImage;
use ps1_puyopuyo_box::pipeline::{
    render_quest_glyph_cache_bank_runtime_tsv, verify_quest_glyph_cache_bank_runtime,
    QuestGlyphCacheBankRuntimeEvidence,
};

use super::development_disc_inputs::read_current_development_disc_inputs;
use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn verify_quest_glyph_cache_bank_runtime_command(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.collect::<Vec<_>>();
    let [stream_id, primary_page, source_path, evidence_dir, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let stream_id = stream_id
        .to_str()
        .ok_or_else(|| "QUEST bank stream ID가 UTF-8이 아님".to_string())?;
    let primary_page = primary_page
        .to_str()
        .ok_or_else(|| "QUEST bank primary page가 UTF-8이 아님".to_string())?
        .parse::<u8>()
        .map_err(|source| format!("QUEST bank primary page 오류: {source}"))?;
    let source_path = PathBuf::from(source_path);
    let evidence_dir = PathBuf::from(evidence_dir);
    let output_path = PathBuf::from(output_path);
    let source = VerifiedSourceImage::open(&source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let read = |name: &str| {
        let path = evidence_dir.join(name);
        fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let before_commit_state = read("before-secondary-commit.state")?;
    let commit_cpu_state = read("secondary-commit-state.json")?;
    let after_visible_state = read("after-secondary-visible.state")?;
    let after_visible_screenshot = read("after-secondary-visible.png")?;
    let draw_events = read("secondary-draw-events.json")?;
    let inputs = read_current_development_disc_inputs(&source_path, &source)?;
    let report = verify_quest_glyph_cache_bank_runtime(
        &inputs.build,
        &inputs.package,
        &inputs.hook,
        &inputs.routing,
        stream_id,
        primary_page,
        QuestGlyphCacheBankRuntimeEvidence {
            before_commit_state: &before_commit_state,
            commit_cpu_state: &commit_cpu_state,
            after_visible_state: &after_visible_state,
            after_visible_screenshot: &after_visible_screenshot,
            draw_events: &draw_events,
        },
    )
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_glyph_cache_bank_runtime_tsv(&report);
    write_report_file(
        &output_path,
        "QUEST glyph cache bank runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST glyph cache secondary bank: {} page {}→{} marker {}/{} at 0x{:08X}, \
         objects {}→{}, draws {}/{}, dropped {}, downstream {}, cold {}, all-pinned {}, release {}",
        report.stream_id,
        report.primary_page,
        report.secondary_page,
        report.primary_marker,
        report.secondary_marker,
        report.secondary_commit_runtime,
        report.before_live_objects,
        report.after_live_objects,
        report.primary_draws,
        report.secondary_draws,
        report.dropped_events,
        report.downstream_input_transition_verified,
        report.cold_route_verified,
        report.all_banks_pinned_runtime_verified,
        report.release_eligible,
    );
    Ok(())
}
