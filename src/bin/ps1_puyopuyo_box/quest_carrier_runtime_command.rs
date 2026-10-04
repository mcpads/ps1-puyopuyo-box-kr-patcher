//! QUEST development carrier 추가 장면 검증 명령.

use std::fs;
use std::path::PathBuf;

use ps1_puyopuyo_box::disc::VerifiedSourceImage;
use ps1_puyopuyo_box::pipeline::{
    render_quest_development_carrier_scene_runtime_tsv,
    verify_quest_development_carrier_scene_runtime, QuestDevelopmentCarrierSceneRuntimeEvidence,
};

use super::development_disc_inputs::read_quest_development_disc_inputs;
use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn verify_quest_development_carrier_scene_runtime_command(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.collect::<Vec<_>>();
    let [expected_page, source_path, state_path, screenshot_path, descriptor_events_path, observation_path, output_path] =
        paths.as_slice()
    else {
        return Err(usage().to_string());
    };
    let expected_page = expected_page
        .to_str()
        .ok_or_else(|| "예상 page ID가 UTF-8이 아님".to_string())?
        .parse::<u8>()
        .map_err(|source| format!("예상 page ID 오류: {source}"))?;
    let source_path = PathBuf::from(source_path);
    let output_path = PathBuf::from(output_path);
    let source = VerifiedSourceImage::open(&source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let read = |path: &std::ffi::OsString| {
        let path = PathBuf::from(path);
        fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let native_state = read(state_path)?;
    let screenshot = read(screenshot_path)?;
    let descriptor_events = read(descriptor_events_path)?;
    let observation = read(observation_path)?;
    let inputs = read_quest_development_disc_inputs(&source_path, &source)?;
    let report = verify_quest_development_carrier_scene_runtime(
        &inputs.build,
        &inputs.package,
        &inputs.hook,
        expected_page,
        QuestDevelopmentCarrierSceneRuntimeEvidence {
            native_state: &native_state,
            screenshot_png: &screenshot,
            descriptor_events: &descriptor_events,
            write_observation: &observation,
        },
    )
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_development_carrier_scene_runtime_tsv(&report);
    write_report_file(
        &output_path,
        "QUEST development carrier scene runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST development carrier scene: {}, page {}, frame {}..{} ({}), \
         descriptor 0x{:08X}/object 0x{:08X}, scratch 0x{:08X}/{}B, protected {}B/{} hits, \
         bounded evidence {}, translation/layout approved {}/{}, release {}",
        report.scene_id,
        report.expected_page,
        report.start_frame,
        report.end_frame,
        report.observed_frames,
        report.descriptor_entry_runtime,
        report.object_runtime,
        report.mutable_scratch_runtime,
        report.mutable_scratch_bytes,
        report.protected_bytes,
        report.protected_write_hits,
        report.bounded_scene_evidence_verified,
        report.translation_approved,
        report.layout_approved,
        report.release_eligible,
    );
    Ok(())
}
