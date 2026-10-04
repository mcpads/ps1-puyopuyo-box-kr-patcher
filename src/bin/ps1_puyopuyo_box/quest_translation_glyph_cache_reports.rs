//! QUEST 선택 번역의 glyph cache plan·package·hook report 명령.

use std::path::PathBuf;

use ps1_puyopuyo_box::disc::{Iso9660, VerifiedSourceImage};
use ps1_puyopuyo_box::pipeline::quest_translation_glyph_cache_hook_plan::{
    render_quest_translation_glyph_cache_hook_plan_tsv,
    verify_box_quest_translation_glyph_cache_hook_plan,
};
use ps1_puyopuyo_box::pipeline::quest_translation_glyph_cache_package::{
    render_quest_translation_glyph_cache_package_tsv,
    verify_box_quest_translation_glyph_cache_package,
};
use ps1_puyopuyo_box::pipeline::quest_translation_glyph_cache_plan::{
    render_quest_translation_glyph_cache_plan_tsv, verify_box_quest_translation_glyph_cache_plan,
};

use super::disc_commands::read_box_executables;
use super::quest_analysis_inputs::read_quest_translation_selection_inputs;
use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn write_quest_translation_glyph_cache_plan(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let inputs = read_quest_translation_selection_inputs(source_path)?;
    let analysis =
        verify_box_quest_translation_glyph_cache_plan(&source, &inputs.selection, &inputs.routing)
            .map_err(|error| error.to_string())?;
    let report = render_quest_translation_glyph_cache_plan_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST translation glyph cache plan TSV",
        analysis.pages.len(),
        &report,
    )?;
    println!(
        "  glyphs {}, directory/selected pages {}/{}, max page {}, assignments {}/{}B, \
         dictionary rows {}, catalog/original-source/package {}/{}/{}B, carrier remaining {}B, \
         product-eligible {}",
        analysis.plan.glyphs,
        analysis.plan.pages,
        analysis.selected_pages,
        analysis.plan.max_page_glyphs,
        analysis.plan.page_assignments,
        analysis.plan.page_assignment_bytes,
        analysis.plan.catalog.dictionary_rows,
        analysis.plan.catalog.stored_bytes,
        analysis.plan.original_atlas_source.stored_bytes,
        analysis.plan.package_bytes,
        analysis.plan.carrier_remaining_bytes,
        analysis.product_eligible_routes,
    );
    Ok(())
}

pub(super) fn write_quest_translation_glyph_cache_package(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let inputs = read_quest_translation_selection_inputs(source_path)?;
    let package = verify_box_quest_translation_glyph_cache_package(
        &source,
        &inputs.selection,
        &inputs.routing,
    )
    .map_err(|error| error.to_string())?;
    let report = render_quest_translation_glyph_cache_package_tsv(&package);
    write_report_file(
        output_path,
        "QUEST translation glyph cache package TSV",
        1,
        &report,
    )?;
    println!(
        "  package {}B, SHA-256 {}, scratch offset/bytes {}/{}, sections exact {}, \
         runtime {}, build-eligible {}",
        package.bytes.len(),
        package.sha256,
        package.layout.scratch_offset,
        package.layout.scratch_bytes,
        package.sections_exact,
        package.runtime_verified,
        package.build_eligible,
    );
    Ok(())
}

pub(super) fn write_quest_translation_glyph_cache_hook_plan(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let iso = Iso9660::parse(source.image()).map_err(|error| error.to_string())?;
    let executables = read_box_executables(&iso)?;
    let quest = executables
        .iter()
        .find(|exe| exe.map.name == "QUEST.EXE")
        .ok_or_else(|| "QUEST.EXE 검증 입력이 없음".to_string())?;
    let inputs = read_quest_translation_selection_inputs(source_path)?;
    let package = verify_box_quest_translation_glyph_cache_package(
        &source,
        &inputs.selection,
        &inputs.routing,
    )
    .map_err(|error| error.to_string())?;
    let plan = verify_box_quest_translation_glyph_cache_hook_plan(
        quest.map,
        &quest.text,
        &package,
        &inputs.routing,
    )
    .map_err(|error| error.to_string())?;
    let report = render_quest_translation_glyph_cache_hook_plan_tsv(&package, &plan);
    write_report_file(
        output_path,
        "QUEST translation glyph cache hook plan TSV",
        1,
        &report,
    )?;
    println!(
        "  package/body {}/{}B, body runtime 0x{:08X}, carrier used/remaining {}/{}B, \
         max uploads {}, relocated {}, runtime {}, build-eligible {}",
        plan.package_bytes,
        plan.body_bytes,
        plan.body_runtime,
        plan.carrier_used_bytes,
        plan.carrier_remaining_bytes,
        plan.max_page_uploads,
        plan.relocated_routes,
        plan.runtime_verified,
        plan.build_eligible,
    );
    Ok(())
}
