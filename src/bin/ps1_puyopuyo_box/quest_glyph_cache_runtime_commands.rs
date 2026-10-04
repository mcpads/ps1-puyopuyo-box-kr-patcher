//! QUEST glyph cache의 적재, 전환, residency 증거를 검증하는 명령.

use std::fs;
use std::path::PathBuf;

use ps1_puyopuyo_box::disc::VerifiedSourceImage;
use ps1_puyopuyo_box::pipeline::{
    render_quest_glyph_cache_residency_tsv, render_quest_glyph_cache_runtime_tsv,
    render_quest_glyph_cache_transition_runtime_tsv, render_quest_glyph_cache_upload_runtime_tsv,
    render_quest_resident_text_runtime_tsv, verify_box_quest_glyph_cache_disc_control,
    verify_box_quest_glyph_cache_transition_poc, verify_box_quest_glyph_cache_upload_poc,
    verify_quest_glyph_cache_control_ram,
    verify_quest_glyph_cache_residency as verify_quest_glyph_cache_residency_state,
    verify_quest_glyph_cache_transition_runtime as verify_quest_glyph_cache_transition_runtime_state,
    verify_quest_glyph_cache_upload_runtime as verify_quest_glyph_cache_upload_runtime_state,
    verify_quest_resident_text_runtime as verify_quest_resident_text_runtime_state,
    QuestGlyphCacheTransitionRuntimeCheckpoint, QuestGlyphCacheTransitionRuntimeEvidence,
    QuestResidentTextRuntimeEvidence,
};

use super::development_disc_inputs::read_quest_development_disc_inputs;
use super::quest_analysis_inputs::read_quest_glyph_cache_control_inputs;
use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn verify_quest_glyph_cache_control_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, ram_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let ram = fs::read(ram_path).map_err(|error| format!("{}: {error}", ram_path.display()))?;
    let inputs = read_quest_glyph_cache_control_inputs(&source)?;
    let build = verify_box_quest_glyph_cache_disc_control(&source, &inputs.package, &inputs.hook)
        .map_err(|error| error.to_string())?;
    let report = verify_quest_glyph_cache_control_ram(&build, &inputs.package, &inputs.hook, &ram)
        .map_err(|error| error.to_string())?;
    let rendered = render_quest_glyph_cache_runtime_tsv(&report);
    write_report_file(
        output_path,
        "QUEST sparse glyph cache runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST sparse glyph cache runtime installation: package 0x{:08X}/{}B, \
         site 0x{:08X}/{}B, body 0x{:08X}/{}B exact, page state 0x{:08X}={}, \
         RAM SHA-256 {}, image SHA-256 {}",
        report.package_runtime,
        report.package_bytes,
        report.site_runtime,
        report.site_bytes,
        report.body_runtime,
        report.body_bytes,
        report.page_state_runtime,
        report.page_state,
        report.ram_sha256,
        report.image_sha256,
    );
    Ok(())
}

pub(super) fn verify_quest_glyph_cache_upload_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, ram_path, gpu_path, events_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let ram = fs::read(ram_path).map_err(|error| format!("{}: {error}", ram_path.display()))?;
    let gpu = fs::read(gpu_path).map_err(|error| format!("{}: {error}", gpu_path.display()))?;
    let events =
        fs::read(events_path).map_err(|error| format!("{}: {error}", events_path.display()))?;
    let inputs = read_quest_glyph_cache_control_inputs(&source)?;
    let build = verify_box_quest_glyph_cache_upload_poc(
        &source,
        &inputs.batches,
        &inputs.routing,
        &inputs.package,
        &inputs.hook,
    )
    .map_err(|error| error.to_string())?;
    let report = verify_quest_glyph_cache_upload_runtime_state(
        &build,
        &inputs.package,
        &inputs.hook,
        &ram,
        &gpu,
        &events,
    )
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_glyph_cache_upload_runtime_tsv(&report);
    write_report_file(
        output_path,
        "QUEST sparse glyph cache active runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST sparse glyph cache active runtime: package 0x{:08X}/{}B, \
         site 0x{:08X}/{}B, body 0x{:08X}/{}B exact, page state 0x{:08X}={}, \
         upload/sync/commit {}/{}/{}, uploaded cells {}/{} exact, RAM SHA-256 {}, \
         GPU SHA-256 {}, events SHA-256 {}, image SHA-256 {}",
        report.package_runtime,
        report.package_bytes,
        report.site_runtime,
        report.site_bytes,
        report.body_runtime,
        report.body_bytes,
        report.page_state_runtime,
        report.page_state,
        report.load_image_calls,
        report.draw_sync_calls,
        report.page_commit_events,
        report.page_glyphs,
        report.page_glyphs,
        report.ram_sha256,
        report.gpu_sha256,
        report.events_sha256,
        report.image_sha256,
    );
    Ok(())
}

pub(super) fn verify_quest_resident_text_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, evidence_dir, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let inputs = read_quest_development_disc_inputs(source_path, &source)?;
    let read = |relative: &str| {
        let path = evidence_dir.join(relative);
        fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let start_menu_ram = read("start-menu-dump/ram.bin")?;
    let start_menu_gpu = read("start-menu-dump/gpu.bin")?;
    let start_menu_events = read("start-menu-events.json")?;
    let start_menu_screenshot = read("start-menu.png")?;
    let farewell_ram = read("farewell-dump/ram.bin")?;
    let farewell_gpu = read("farewell-dump/gpu.bin")?;
    let farewell_events = read("farewell-reuse-events.json")?;
    let farewell_screenshot = read("farewell.png")?;
    let report = verify_quest_resident_text_runtime_state(
        &inputs.build,
        &inputs.package,
        &inputs.hook,
        QuestResidentTextRuntimeEvidence {
            start_menu_ram: &start_menu_ram,
            start_menu_gpu: &start_menu_gpu,
            start_menu_events: &start_menu_events,
            start_menu_screenshot: &start_menu_screenshot,
            farewell_ram: &farewell_ram,
            farewell_gpu: &farewell_gpu,
            farewell_events: &farewell_events,
            farewell_screenshot: &farewell_screenshot,
        },
    )
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_resident_text_runtime_tsv(&report);
    write_report_file(output_path, "QUEST resident text runtime TSV", 2, &rendered)?;
    let [start, farewell] = report.screens.as_slice() else {
        return Err(format!(
            "QUEST resident runtime 화면 분모 불일치: expected=2, actual={}",
            report.screens.len()
        ));
    };
    println!(
        "PASS QUEST resident text runtime: image {}, page {}, start upload/sync/commit \
         {}/{}/{}, farewell resident/upload {}/{}, start window ({},{}) {}x{}, \
         farewell window ({},{}) {}x{}, 사람 translation/layout/display 승인 false",
        report.image_sha256,
        start.page_id,
        start.load_image_calls,
        start.draw_sync_calls,
        start.primary_commit_events,
        farewell.primary_resident_events,
        farewell.load_image_calls,
        start.window.geometry.parent_x_px,
        start.window.geometry.parent_y_px,
        start.window.geometry.parent_width_px,
        start.window.geometry.parent_height_px,
        farewell.window.geometry.parent_x_px,
        farewell.window.geometry.parent_y_px,
        farewell.window.geometry.parent_width_px,
        farewell.window.geometry.parent_height_px,
    );
    Ok(())
}

pub(super) fn verify_quest_glyph_cache_transition_runtime(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, evidence_dir, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let inputs = read_quest_glyph_cache_control_inputs(&source)?;
    let build = verify_box_quest_glyph_cache_transition_poc(
        &source,
        &inputs.batches,
        &inputs.routing,
        &inputs.package,
        &inputs.hook,
    )
    .map_err(|error| error.to_string())?;
    let read = |relative: &str| {
        let path = evidence_dir.join(relative);
        fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))
    };

    let first_entry_state = read("first-entry-dump/state.json")?;
    let first_entry_ram = read("first-entry-dump/ram.bin")?;
    let first_return_ram = read("first-return-dump/ram.bin")?;
    let first_return_gpu = read("first-return-dump/gpu.bin")?;
    let first_lifecycle = read("first-lifecycle-events.json")?;
    let second_entry_state = read("second-entry-dump/state.json")?;
    let second_entry_ram = read("second-entry-dump/ram.bin")?;
    let second_return_ram = read("second-return-dump/ram.bin")?;
    let second_return_gpu = read("second-return-dump/gpu.bin")?;
    let second_lifecycle = read("second-lifecycle-events.json")?;
    let third_entry_state = read("third-entry-dump/state.json")?;
    let third_entry_ram = read("third-entry-dump/ram.bin")?;
    let third_return_ram = read("third-return-dump/ram.bin")?;
    let third_return_gpu = read("third-return-dump/gpu.bin")?;
    let third_lifecycle = read("third-lifecycle-events.json")?;
    let restored_entry_state = read("restored-entry-dump/state.json")?;
    let restored_entry_ram = read("restored-entry-dump/ram.bin")?;
    let restored_return_ram = read("restored-return-dump/ram.bin")?;
    let restored_return_gpu = read("restored-return-dump/gpu.bin")?;
    let restored_lifecycle = read("restored-lifecycle-events.json")?;
    let first_visible = read("first-visible.png")?;
    let first_visible_page2 = read("first-visible-page2.png")?;
    let first_visible_page3 = read("first-visible-page3.png")?;
    let second_visible = read("second-visible.png")?;
    let second_visible_page2 = read("second-visible-page2.png")?;
    let third_visible = read("third-visible.png")?;
    let third_visible_page2 = read("third-visible-page2.png")?;
    let restored_visible = read("restored-visible.png")?;

    let report = verify_quest_glyph_cache_transition_runtime_state(
        &build,
        &inputs.package,
        &inputs.hook,
        QuestGlyphCacheTransitionRuntimeEvidence {
            first: QuestGlyphCacheTransitionRuntimeCheckpoint {
                entry_state: &first_entry_state,
                entry_ram: &first_entry_ram,
                return_ram: &first_return_ram,
                return_gpu: &first_return_gpu,
                lifecycle_events: &first_lifecycle,
            },
            second: QuestGlyphCacheTransitionRuntimeCheckpoint {
                entry_state: &second_entry_state,
                entry_ram: &second_entry_ram,
                return_ram: &second_return_ram,
                return_gpu: &second_return_gpu,
                lifecycle_events: &second_lifecycle,
            },
            third: QuestGlyphCacheTransitionRuntimeCheckpoint {
                entry_state: &third_entry_state,
                entry_ram: &third_entry_ram,
                return_ram: &third_return_ram,
                return_gpu: &third_return_gpu,
                lifecycle_events: &third_lifecycle,
            },
            restored: QuestGlyphCacheTransitionRuntimeCheckpoint {
                entry_state: &restored_entry_state,
                entry_ram: &restored_entry_ram,
                return_ram: &restored_return_ram,
                return_gpu: &restored_return_gpu,
                lifecycle_events: &restored_lifecycle,
            },
            first_visible_png: &first_visible,
            first_visible_page2_png: &first_visible_page2,
            first_visible_page3_png: &first_visible_page3,
            second_visible_png: &second_visible,
            second_visible_page2_png: &second_visible_page2,
            third_visible_png: &third_visible,
            third_visible_page2_png: &third_visible_page2,
            restored_visible_png: &restored_visible,
        },
    )
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_glyph_cache_transition_runtime_tsv(&report);
    write_report_file(
        output_path,
        "QUEST sparse glyph cache transition runtime TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST sparse glyph cache transition runtime: records {}, objects {}, \
         pages {}, upload {}, sync {}, events {}, dropped {}, same-page skip {}, \
         different-page exact {}, original restored {}, visible PNG shape {}, \
         runtime {}, translation/layout approved {}/{}, release {}, image SHA-256 {}",
        report
            .record_runtimes
            .iter()
            .map(|runtime| format!("0x{runtime:08X}"))
            .collect::<Vec<_>>()
            .join("/"),
        report
            .object_runtimes
            .iter()
            .map(|runtime| format!("0x{runtime:08X}"))
            .collect::<Vec<_>>()
            .join("/"),
        report
            .page_states
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join("/"),
        report
            .load_image_calls
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join("/"),
        report
            .draw_sync_calls
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join("/"),
        report.event_count,
        report.dropped_events,
        report.same_page_upload_skipped,
        report.different_page_upload_exact,
        report.original_page_restored,
        report.visible_capture_shapes_exact,
        report.runtime_verified,
        report.translation_approved,
        report.layout_approved,
        report.release_eligible,
        report.image_sha256,
    );
    Ok(())
}

pub(super) fn verify_quest_glyph_cache_residency(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let values = args.collect::<Vec<_>>();
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
    let inputs = read_quest_glyph_cache_control_inputs(&source)?;
    let build = verify_box_quest_glyph_cache_transition_poc(
        &source,
        &inputs.batches,
        &inputs.routing,
        &inputs.package,
        &inputs.hook,
    )
    .map_err(|error| error.to_string())?;
    let report = verify_quest_glyph_cache_residency_state(
        &build,
        &inputs.package,
        &inputs.hook,
        expected_page,
        &native_state,
        &screenshot,
    )
    .map_err(|error| error.to_string())?;
    let rendered = render_quest_glyph_cache_residency_tsv(&report);
    write_report_file(
        &output_path,
        "QUEST sparse glyph cache residency TSV",
        1,
        &rendered,
    )?;
    println!(
        "PASS QUEST sparse glyph cache residency: page {}, package/site/body exact \
         {}/{}/{}, carrier suffix {}B zero, active font states {}, screenshot {}x{}, \
         state SHA-256 {}, image SHA-256 {}, translation/layout approved {}/{}, release {}",
        report.page_state,
        report.resident_package_exact,
        report.site_exact,
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
