//! QUEST page 공급과 dialogue 전환의 runtime 증거를 검증하는 명령.

use std::fs;
use std::path::PathBuf;

use ps1_puyopuyo_box::disc::VerifiedSourceImage;
use ps1_puyopuyo_box::pipeline::{
    render_quest_page_first_dialogue_runtime_tsv, render_quest_page_transition_runtime_tsv,
    render_quest_page_two_dialogues_runtime_tsv,
    verify_quest_page_first_dialogue_runtime as verify_quest_page_first_dialogue_runtime_state,
    verify_quest_page_transition_runtime as verify_quest_page_transition_runtime_state,
    verify_quest_page_two_dialogues_runtime as verify_quest_page_two_dialogues_runtime_state,
    QuestPageTransitionPhase, QuestPageTwoDialoguesRuntimeCheckpoint,
    QuestPageTwoDialoguesRuntimeEvidence,
};

use super::quest_font_page_reports::read_verified_quest_second_dialogue_assets;
use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn verify_quest_page_first_dialogue_runtime(
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
    let report = verify_quest_page_first_dialogue_runtime_state(&source, &ram, &gpu)
        .map_err(|error| error.to_string())?;
    let rendered = render_quest_page_first_dialogue_runtime_tsv(&report);
    write_report_file(
        output_path,
        "QUEST page first dialogue runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST page first dialogue runtime: target 0x{:08X}, raw source 0x{:08X} {}B, \
         state {}, hook/site/resident/upload exact, RAM SHA-256 {}, GPU SHA-256 {}, \
         image SHA-256 {}",
        report.target_record_runtime,
        report.page_source_runtime,
        report.page_bytes,
        report.page_state,
        report.ram_sha256,
        report.gpu_sha256,
        report.image_sha256,
    );
    Ok(())
}

pub(super) fn verify_quest_page_transition_runtime(
    mut args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let phase_arg = args.next().ok_or_else(|| usage().to_string())?;
    let phase = match phase_arg.to_str() {
        Some("korean") => QuestPageTransitionPhase::Korean,
        Some("restored") => QuestPageTransitionPhase::Restored,
        _ => return Err("QUEST page transition phase는 korean 또는 restored여야 함".to_string()),
    };
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, ram_path, gpu_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let ram = fs::read(ram_path).map_err(|error| format!("{}: {error}", ram_path.display()))?;
    let gpu = fs::read(gpu_path).map_err(|error| format!("{}: {error}", gpu_path.display()))?;
    let report = verify_quest_page_transition_runtime_state(&source, &ram, &gpu, phase)
        .map_err(|error| error.to_string())?;
    let rendered = render_quest_page_transition_runtime_tsv(&report);
    write_report_file(
        output_path,
        "QUEST page transition runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST page transition runtime: phase {}, source \
         0x{:08X}/{}B, state {}, hook/site/pages/upload exact, RAM SHA-256 {}, \
         GPU SHA-256 {}, image SHA-256 {}",
        report.phase.label(),
        report.expected_page_source_runtime,
        report.page_bytes,
        report.page_state,
        report.ram_sha256,
        report.gpu_sha256,
        report.image_sha256,
    );
    Ok(())
}

pub(super) fn verify_quest_page_two_dialogues_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, evidence_dir, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let second = read_verified_quest_second_dialogue_assets(source_path)?;
    let read = |relative: &str| {
        let path = evidence_dir.join(relative);
        fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let first_ram = read("first-return-dump/ram.bin")?;
    let first_gpu = read("first-return-dump/gpu.bin")?;
    let first_entry_events = read("first-hook-entry-events.json")?;
    let first_lifecycle_events = read("first-lifecycle-events.json")?;
    let second_ram = read("second-return-dump/ram.bin")?;
    let second_gpu = read("second-return-dump/gpu.bin")?;
    let second_entry_events = read("second-hook-entry-events.json")?;
    let second_lifecycle_events = read("second-lifecycle-events.json")?;
    let restored_ram = read("restored-return-dump/ram.bin")?;
    let restored_gpu = read("restored-return-dump/gpu.bin")?;
    let restored_entry_events = read("restored-hook-entry-events.json")?;
    let restored_lifecycle_events = read("restored-lifecycle-events.json")?;
    let first_visible = read("first-visible.png")?;
    let second_visible = read("second-visible.png")?;
    let second_visible_page2 = read("second-visible-page2.png")?;
    let restored_visible = read("restored-visible.png")?;

    let report = verify_quest_page_two_dialogues_runtime_state(
        &source,
        &second,
        QuestPageTwoDialoguesRuntimeEvidence {
            first: QuestPageTwoDialoguesRuntimeCheckpoint {
                ram: &first_ram,
                gpu: &first_gpu,
                entry_events: &first_entry_events,
                lifecycle_events: &first_lifecycle_events,
            },
            second: QuestPageTwoDialoguesRuntimeCheckpoint {
                ram: &second_ram,
                gpu: &second_gpu,
                entry_events: &second_entry_events,
                lifecycle_events: &second_lifecycle_events,
            },
            restored: QuestPageTwoDialoguesRuntimeCheckpoint {
                ram: &restored_ram,
                gpu: &restored_gpu,
                entry_events: &restored_entry_events,
                lifecycle_events: &restored_lifecycle_events,
            },
            first_visible_png: &first_visible,
            second_visible_png: &second_visible,
            second_visible_page2_png: &second_visible_page2,
            restored_visible_png: &restored_visible,
        },
    )
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_page_two_dialogues_runtime_tsv(&report);
    write_report_file(
        output_path,
        "QUEST two-dialogue page runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST two-dialogue page runtime: records 0x{:08X}/0x{:08X}/0x{:08X}, \
         objects 0x{:08X}/0x{:08X}/0x{:08X}, body 0x{:08X}/{}B, page states \
         {}/{}/{}, upload {}/{}/{}, sync {}/{}/{}, events {}, dropped {}, \
         visible PNG shape {}, runtime {}, layout approved {}, release {}, image SHA-256 {}",
        report.first_record_runtime,
        report.second_record_runtime,
        report.restored_record_runtime,
        report.first_object_runtime,
        report.second_object_runtime,
        report.restored_object_runtime,
        report.body_runtime,
        report.body_bytes,
        report.first_page_state,
        report.second_page_state,
        report.restored_page_state,
        report.first_upload_calls,
        report.second_upload_calls,
        report.restored_upload_calls,
        report.first_sync_calls,
        report.second_sync_calls,
        report.restored_sync_calls,
        report.event_count,
        report.dropped_events,
        report.visible_capture_shapes_exact,
        report.runtime_verified,
        report.layout_approved,
        report.release_eligible,
        report.image_sha256,
    );
    Ok(())
}
