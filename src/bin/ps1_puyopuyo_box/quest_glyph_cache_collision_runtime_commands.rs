//! QUEST glyph cache 충돌과 yield/retry 복구 증거를 검증하는 명령.

use std::fs;
use std::path::PathBuf;

use ps1_puyopuyo_box::disc::VerifiedSourceImage;
use ps1_puyopuyo_box::pipeline::{
    render_quest_glyph_cache_collision_runtime_tsv, verify_box_quest_glyph_cache_transition_poc,
    verify_box_quest_glyph_cache_upload_poc,
    verify_quest_glyph_cache_collision_runtime as verify_quest_glyph_cache_collision_runtime_state,
    verify_quest_glyph_cache_transition_collision_runtime as verify_quest_glyph_cache_transition_collision_runtime_state,
    QuestGlyphCacheCollisionRuntimeReport,
};

use super::quest_analysis_inputs::read_quest_glyph_cache_control_inputs;
use super::translation_reports::write_report_file;
use super::usage::usage;

struct QuestGlyphCacheCollisionRuntimeFiles {
    source: VerifiedSourceImage,
    pristine_ram: Vec<u8>,
    forced_ram: Vec<u8>,
    yielded_ram: Vec<u8>,
    retry_ram: Vec<u8>,
    entry_events: Vec<u8>,
    yield_events: Vec<u8>,
    retry_events: Vec<u8>,
    output_path: PathBuf,
}

fn read_quest_glyph_cache_collision_runtime_files(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<QuestGlyphCacheCollisionRuntimeFiles, String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, pristine_ram_path, forced_ram_path, yielded_ram_path, retry_ram_path, entry_events_path, yield_events_path, retry_events_path, output_path] =
        paths.as_slice()
    else {
        return Err(usage().to_string());
    };
    let read =
        |path: &PathBuf| fs::read(path).map_err(|error| format!("{}: {error}", path.display()));
    Ok(QuestGlyphCacheCollisionRuntimeFiles {
        source: VerifiedSourceImage::open(source_path)
            .map_err(|error| format!("{}: {error}", source_path.display()))?,
        pristine_ram: read(pristine_ram_path)?,
        forced_ram: read(forced_ram_path)?,
        yielded_ram: read(yielded_ram_path)?,
        retry_ram: read(retry_ram_path)?,
        entry_events: read(entry_events_path)?,
        yield_events: read(yield_events_path)?,
        retry_events: read(retry_events_path)?,
        output_path: output_path.clone(),
    })
}

fn write_quest_glyph_cache_collision_runtime_report(
    files: &QuestGlyphCacheCollisionRuntimeFiles,
    report: &QuestGlyphCacheCollisionRuntimeReport,
    label: &str,
) -> Result<(), String> {
    let rendered = render_quest_glyph_cache_collision_runtime_tsv(report);
    write_report_file(&files.output_path, label, 1, &rendered)?;
    let result_label = label.strip_suffix(" TSV").unwrap_or(label);
    println!(
        "PASS {result_label}: body 0x{:08X}/{}B, object 0x{:08X}, stream 0x{:08X}, \
         first active/yield/page 0x{:08X}/0x{:08X}/0x{:08X}, active states {:?}, \
         forced/collision/stack diff {}/{}/{}, events {}, dropped {}, \
         retry exact {}, image SHA-256 {}",
        report.body_runtime,
        report.body_bytes,
        report.object_runtime,
        report.stream_runtime,
        report.first_active_runtime,
        report.yield_runtime,
        report.page_state_runtime,
        report.active_state_indices,
        report.forced_diff_bytes,
        report.collision_diff_bytes,
        report.collision_stack_diff_bytes,
        report.event_count,
        report.dropped_events,
        report.active_collision_policy_verified,
        report.image_sha256,
    );
    Ok(())
}

pub(super) fn verify_quest_glyph_cache_collision_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let files = read_quest_glyph_cache_collision_runtime_files(args)?;
    let inputs = read_quest_glyph_cache_control_inputs(&files.source)?;
    let build = verify_box_quest_glyph_cache_upload_poc(
        &files.source,
        &inputs.batches,
        &inputs.routing,
        &inputs.package,
        &inputs.hook,
    )
    .map_err(|error| error.to_string())?;
    let report = verify_quest_glyph_cache_collision_runtime_state(
        &build,
        &inputs.package,
        &inputs.hook,
        &files.pristine_ram,
        &files.forced_ram,
        &files.yielded_ram,
        &files.retry_ram,
        &files.entry_events,
        &files.yield_events,
        &files.retry_events,
    )
    .map_err(|error| error.to_string())?;
    write_quest_glyph_cache_collision_runtime_report(
        &files,
        &report,
        "QUEST sparse glyph cache collision runtime TSV",
    )
}

pub(super) fn verify_quest_glyph_cache_transition_collision_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let files = read_quest_glyph_cache_collision_runtime_files(args)?;
    let inputs = read_quest_glyph_cache_control_inputs(&files.source)?;
    let build = verify_box_quest_glyph_cache_transition_poc(
        &files.source,
        &inputs.batches,
        &inputs.routing,
        &inputs.package,
        &inputs.hook,
    )
    .map_err(|error| error.to_string())?;
    let report = verify_quest_glyph_cache_transition_collision_runtime_state(
        &build,
        &inputs.package,
        &inputs.hook,
        &files.pristine_ram,
        &files.forced_ram,
        &files.yielded_ram,
        &files.retry_ram,
        &files.entry_events,
        &files.yield_events,
        &files.retry_events,
    )
    .map_err(|error| error.to_string())?;
    write_quest_glyph_cache_collision_runtime_report(
        &files,
        &report,
        "QUEST sparse glyph cache transition collision runtime TSV",
    )
}
