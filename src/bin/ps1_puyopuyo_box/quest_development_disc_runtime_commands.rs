//! QUEST development disc의 residency와 route 경계를 검증하는 명령.

use std::fs;
use std::path::PathBuf;

use ps1_puyopuyo_box::disc::VerifiedSourceImage;
use ps1_puyopuyo_box::pipeline::{
    render_quest_development_max_page_runtime_tsv, render_quest_development_route_runtime_tsv,
    render_quest_glyph_cache_residency_tsv,
    verify_quest_development_disc_initial_residency as verify_quest_development_disc_initial_residency_state,
    verify_quest_development_disc_residency as verify_quest_development_disc_residency_state,
    verify_quest_development_max_page_runtime, verify_quest_development_synthetic_route_runtime,
    QuestDevelopmentMaxPageRuntimeEvidence, QuestDevelopmentRouteRuntimeEvidence,
};

use super::development_disc_inputs::read_quest_development_disc_inputs;
use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn verify_quest_development_disc_residency_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let mut expect_initial_scratch = false;
    let mut values = Vec::new();
    for value in args {
        if value == std::ffi::OsStr::new("--expect-initial-scratch") {
            if expect_initial_scratch {
                return Err("--expect-initial-scratch가 중복됨".to_string());
            }
            expect_initial_scratch = true;
        } else {
            values.push(value);
        }
    }
    let [expected_page, source_path, state_path, screenshot_path, output_path] = values.as_slice()
    else {
        return Err(usage().to_string());
    };
    let expected_page = expected_page
        .to_str()
        .ok_or_else(|| "예상 page ID가 UTF-8이 아님".to_string())?
        .parse::<u8>()
        .map_err(|source| format!("예상 page ID 오류: {source}"))?;
    let source_path = PathBuf::from(source_path);
    let state_path = PathBuf::from(state_path);
    let screenshot_path = PathBuf::from(screenshot_path);
    let output_path = PathBuf::from(output_path);

    let source = VerifiedSourceImage::open(&source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let native_state =
        fs::read(&state_path).map_err(|error| format!("{}: {error}", state_path.display()))?;
    let screenshot = fs::read(&screenshot_path)
        .map_err(|error| format!("{}: {error}", screenshot_path.display()))?;
    let inputs = read_quest_development_disc_inputs(&source_path, &source)?;
    let report = if expect_initial_scratch {
        verify_quest_development_disc_initial_residency_state(
            &inputs.build,
            &inputs.package,
            &inputs.hook,
            expected_page,
            &native_state,
            &screenshot,
        )
    } else {
        verify_quest_development_disc_residency_state(
            &inputs.build,
            &inputs.package,
            &inputs.hook,
            expected_page,
            &native_state,
            &screenshot,
        )
    }
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_glyph_cache_residency_tsv(&report);
    write_report_file(
        &output_path,
        "QUEST development full-disc residency TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST development full-disc residency: page {}, scratch {}, package/site/guard/body exact \
         {}/{}/{}/{}, carrier suffix {}B zero, active font states {}, screenshot {}x{}, \
         state SHA-256 {}, image SHA-256 {}, translation/layout approved {}/{}, product {}",
        report.page_state,
        if expect_initial_scratch {
            "initial-zero"
        } else {
            "uploaded-page"
        },
        report.resident_package_exact,
        report.site_exact,
        report.original_font_guard_site_exact,
        report.body_exact,
        report.carrier_suffix_bytes,
        report.active_font_state_indices.len(),
        report.screenshot_width,
        report.screenshot_height,
        report.state_sha256,
        report.image_sha256,
        report.translation_approved,
        report.layout_approved,
        report.release_eligible,
    );
    Ok(())
}

pub(super) fn verify_quest_development_disc_synthetic_route_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let values = args.collect::<Vec<_>>();
    let [stream_id, source_path, state_path, ram_path, screenshot_path, output_path] =
        values.as_slice()
    else {
        return Err(usage().to_string());
    };
    let stream_id = stream_id
        .to_str()
        .ok_or_else(|| "stream ID가 UTF-8이 아님".to_string())?;
    let source_path = PathBuf::from(source_path);
    let state_path = PathBuf::from(state_path);
    let ram_path = PathBuf::from(ram_path);
    let screenshot_path = PathBuf::from(screenshot_path);
    let output_path = PathBuf::from(output_path);

    let source = VerifiedSourceImage::open(&source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let entry_state =
        fs::read(&state_path).map_err(|error| format!("{}: {error}", state_path.display()))?;
    let entry_ram =
        fs::read(&ram_path).map_err(|error| format!("{}: {error}", ram_path.display()))?;
    let screenshot = fs::read(&screenshot_path)
        .map_err(|error| format!("{}: {error}", screenshot_path.display()))?;
    let inputs = read_quest_development_disc_inputs(&source_path, &source)?;
    let evidence = QuestDevelopmentRouteRuntimeEvidence {
        stream_id,
        entry_state_json: &entry_state,
        entry_ram: &entry_ram,
        visible_png: &screenshot,
    };
    let report = verify_quest_development_synthetic_route_runtime(
        &inputs.build,
        &inputs.writer,
        &inputs.routing,
        &inputs.hook,
        evidence,
    )
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_development_route_runtime_tsv(&report);
    write_report_file(
        &output_path,
        "QUEST synthetic route runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST development synthetic route: {} {}, SEQ {}B exact {}, record/body \
         0x{:08X}/0x{:08X}, object 0x{:08X} pointer exact {}, visible {}x{}, \
         pointer intervention {}, natural reach {}, product {}",
        report.stream_id,
        report.placement,
        report.sequence_bytes,
        report.sequence_loaded_exact,
        report.record_runtime,
        report.body_runtime,
        report.object_runtime,
        report.object_stream_exact,
        report.visible_png_width,
        report.visible_png_height,
        report.synthetic_pointer_intervention,
        report.natural_reach_verified,
        report.product_eligible,
    );
    Ok(())
}

pub(super) fn verify_quest_development_disc_max_page_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let values = args.collect::<Vec<_>>();
    let [stream_id, source_path, evidence_dir, output_path] = values.as_slice() else {
        return Err(usage().to_string());
    };
    let stream_id = stream_id
        .to_str()
        .ok_or_else(|| "stream ID가 UTF-8이 아님".to_string())?;
    let source_path = PathBuf::from(source_path);
    let evidence_dir = PathBuf::from(evidence_dir);
    let output_path = PathBuf::from(output_path);
    let read = |relative: &str| {
        let path = evidence_dir.join(relative);
        fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))
    };

    let source = VerifiedSourceImage::open(&source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let entry_state = read("entry-memory/state.json")?;
    let entry_ram = read("entry-memory/ram.bin")?;
    let return_state = read("return-memory/state.json")?;
    let return_ram = read("return-memory/ram.bin")?;
    let return_gpu = read("return-memory/gpu.bin")?;
    let lifecycle_events = read("lifecycle-events.json")?;
    let visible_png = read("visible.png")?;
    let inputs = read_quest_development_disc_inputs(&source_path, &source)?;
    let report = verify_quest_development_max_page_runtime(
        &inputs.build,
        &inputs.writer,
        &inputs.routing,
        &inputs.package,
        &inputs.hook,
        QuestDevelopmentMaxPageRuntimeEvidence {
            stream_id,
            entry_state_json: &entry_state,
            entry_ram: &entry_ram,
            return_state_json: &return_state,
            return_ram: &return_ram,
            return_gpu: &return_gpu,
            lifecycle_events_json: &lifecycle_events,
            visible_png: &visible_png,
        },
    )
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_development_max_page_runtime_tsv(&report);
    write_report_file(
        &output_path,
        "QUEST development maximum-page runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST development maximum page: {} {}, page {} -> {} / {} glyphs, \
         LoadImage/DrawSync {}/{}, events {}/dropped {}, RAM/GPU/cells exact, \
         visible {}x{}, synthetic stream/page {}/{}, natural reach {}, product {}",
        report.stream_id,
        report.placement,
        report.previous_page_id,
        report.target_page_id,
        report.max_page_glyphs,
        report.load_image_calls,
        report.draw_sync_calls,
        report.event_count,
        report.dropped_events,
        report.visible_png_width,
        report.visible_png_height,
        report.synthetic_stream_intervention,
        report.synthetic_previous_page_intervention,
        report.natural_reach_verified,
        report.product_eligible,
    );
    Ok(())
}
