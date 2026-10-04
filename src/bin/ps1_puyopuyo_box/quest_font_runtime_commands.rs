//! QUEST font 활성 lifetime과 자연 도달 증거를 검증하는 명령.

use std::fs;
use std::path::PathBuf;

use ps1_puyopuyo_box::pipeline::{
    render_quest_font_active_lifetime_runtime_tsv, render_quest_font_active_natural_runtime_tsv,
    verify_quest_font_active_lifetime_runtime as verify_quest_font_active_lifetime_runtime_state,
    verify_quest_font_active_natural_runtime as verify_quest_font_active_natural_runtime_state,
    QuestFontActiveNaturalRuntimeInputs,
};

use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn verify_quest_font_active_lifetime_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [image_path, before_ram_path, after_ram_path, settled_ram_path, events_path, output_path] =
        paths.as_slice()
    else {
        return Err(usage().to_string());
    };
    let read =
        |path: &PathBuf| fs::read(path).map_err(|error| format!("{}: {error}", path.display()));
    let report = verify_quest_font_active_lifetime_runtime_state(
        &read(image_path)?,
        &read(before_ram_path)?,
        &read(after_ram_path)?,
        &read(settled_ram_path)?,
        &read(events_path)?,
    )
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_font_active_lifetime_runtime_tsv(&report);
    write_report_file(
        output_path,
        "QUEST font active lifetime runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST font active lifetime: state {}, active 0x{:08X}, producer/clear \
         0x{:08X}/0x{:08X}, diff {}B, events {}, dropped {}, 자연 도달 {}, release {}",
        report.active_state_index,
        report.active_runtime,
        report.producer_runtime,
        report.clear_runtime,
        report.producer_diff_bytes,
        report.event_count,
        report.dropped_events,
        report.route_naturally_reached,
        report.release_eligible,
    );
    Ok(())
}

pub(super) fn verify_quest_font_active_natural_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [image_path, checkpoint_path, before_ram_path, after_ram_path, settled_ram_path, events_path, route_evidence_path, message_png_path, choice_png_path, output_path] =
        paths.as_slice()
    else {
        return Err(usage().to_string());
    };
    let read =
        |path: &PathBuf| fs::read(path).map_err(|error| format!("{}: {error}", path.display()));
    let image = read(image_path)?;
    let checkpoint_state = read(checkpoint_path)?;
    let before_ram = read(before_ram_path)?;
    let after_ram = read(after_ram_path)?;
    let settled_ram = read(settled_ram_path)?;
    let events_raw = read(events_path)?;
    let route_evidence_raw = read(route_evidence_path)?;
    let message_png = read(message_png_path)?;
    let choice_png = read(choice_png_path)?;
    let report =
        verify_quest_font_active_natural_runtime_state(QuestFontActiveNaturalRuntimeInputs {
            image: &image,
            checkpoint_state: &checkpoint_state,
            before_ram: &before_ram,
            after_ram: &after_ram,
            settled_ram: &settled_ram,
            events_raw: &events_raw,
            route_evidence_raw: &route_evidence_raw,
            message_png: &message_png,
            choice_png: &choice_png,
        })
        .map_err(|error| error.to_string())?;
    let rendered = render_quest_font_active_natural_runtime_tsv(&report);
    write_report_file(
        output_path,
        "QUEST font active natural runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST font active natural: state {}, active 0x{:08X}, producer/clear \
         0x{:08X}/0x{:08X}, samples {}, frame {}, writes {}, cold boot {}, release {}",
        report.transition.active_state_index,
        report.transition.active_runtime,
        report.transition.producer_runtime,
        report.transition.clear_runtime,
        report.producer_samples,
        report.producer_frame,
        report.memory_write_count_after_checkpoint,
        report.cold_boot_route_reached,
        report.transition.release_eligible,
    );
    Ok(())
}
