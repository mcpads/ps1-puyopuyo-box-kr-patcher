//! QUEST sparse glyph cache build 산출물의 원자적 파일 생성.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use ps1_puyopuyo_box::pipeline::{
    render_quest_development_disc_tsv, render_quest_glyph_cache_transition_poc_tsv,
    render_quest_glyph_cache_upload_poc_tsv, render_quest_translation_selection_routing_tsv,
    QuestDevelopmentDiscBuild, QuestGlyphCacheDiscBuild, QuestGlyphCacheTransitionPocBuild,
    QuestGlyphCacheUploadPocBuild, QuestTranslationSelectionRoutingAnalysis,
};

use super::output_writers::{write_image_outputs, write_new_file};

pub(super) fn write_quest_glyph_cache_control_outputs(
    output_dir: &Path,
    build: QuestGlyphCacheDiscBuild,
) -> Result<(), String> {
    let (bin_path, cue_path) =
        write_image_outputs(output_dir, "quest-glyph-cache-control", &build.image)?;
    let report = &build.report;
    println!(
        "PASS QUEST sparse glyph cache control output: package {}B/{}, body \
         0x{:08X}/{}B, QUEST LBA/bytes/sectors {}/{}/{}, Expected Writes {} \
         (machine {}, data {}, metadata {}, copy {}, EDC/ECC {}), exact \
         package/site/body/original/extension {}/{}/{}/{}/{}, runtime {}, \
         build-eligible {}, image SHA-256 {}",
        report.package_bytes,
        report.package_sha256,
        report.body_runtime,
        report.body_bytes,
        report.relocated_quest_lba,
        report.relocated_quest_bytes,
        report.relocated_quest_sectors,
        report.final_expected_writes,
        report.machine_code_writes,
        report.data_writes,
        report.metadata_writes,
        report.relocation_copy_writes,
        report.touched_sectors,
        report.package_exact,
        report.site_exact,
        report.body_exact,
        report.original_text_except_site_exact,
        report.extension_outside_payload_zero,
        report.runtime_verified,
        report.build_eligible,
        report.image_sha256,
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    Ok(())
}

fn write_quest_translation_disc_outputs(
    output_dir: &Path,
    base: &str,
    label: &str,
    build: QuestDevelopmentDiscBuild,
    routing: &QuestTranslationSelectionRoutingAnalysis,
) -> Result<(), String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("{} 생성 실패: {error}", output_dir.display()))?;
    let report_path = output_dir.join(format!("{base}.tsv"));
    let routing_path = output_dir.join(format!("{base}-routing.tsv"));
    if report_path.exists() || routing_path.exists() {
        return Err(format!(
            "기존 산출물을 덮어쓰지 않음: {} / {}",
            report_path.display(),
            routing_path.display(),
        ));
    }
    let pid = std::process::id();
    let report_temp = output_dir.join(format!(".{base}.tsv.tmp-{pid}"));
    let routing_temp = output_dir.join(format!(".{base}-routing.tsv.tmp-{pid}"));
    let report_raw = render_quest_development_disc_tsv(&build.report);
    let routing_raw = render_quest_translation_selection_routing_tsv(routing);
    write_new_file(&report_temp, report_raw.as_bytes())?;
    if let Err(error) = write_new_file(&routing_temp, routing_raw.as_bytes()) {
        let _ = fs::remove_file(&report_temp);
        return Err(error);
    }
    let (bin_path, cue_path) = match write_image_outputs(output_dir, base, &build.image) {
        Ok(paths) => paths,
        Err(error) => {
            let _ = fs::remove_file(&report_temp);
            let _ = fs::remove_file(&routing_temp);
            return Err(error);
        }
    };
    if let Err(error) = fs::rename(&report_temp, &report_path) {
        let _ = fs::remove_file(&report_temp);
        let _ = fs::remove_file(&routing_temp);
        let _ = fs::remove_file(&bin_path);
        let _ = fs::remove_file(&cue_path);
        return Err(format!("{} 확정 실패: {error}", report_path.display()));
    }
    if let Err(error) = fs::rename(&routing_temp, &routing_path) {
        let _ = fs::remove_file(&report_path);
        let _ = fs::remove_file(&routing_temp);
        let _ = fs::remove_file(&bin_path);
        let _ = fs::remove_file(&cue_path);
        return Err(format!("{} 확정 실패: {error}", routing_path.display()));
    }

    let report = &build.report;
    println!(
        "PASS {label}: SEQ target/non-target {}/{}, selected/retained \
         {}/{}, inline/relocated {}/{}, plain {}→{}B, target CNX {}→{}B, \
         VDD LBA/bytes/sectors {}/{}/{}/{}/repacked {}/relocated {}, \
         package/body {}/{}B, Expected Writes {}, exact VDD/target/non-target/font \
         {}/{}/{}/{}, manual layout {}, runtime {}, \
         product-eligible {}, image SHA-256 {}",
        report.target_sequence_entries,
        report.non_target_sequence_entries,
        report.selected_routes,
        report.retained_source_routes,
        report.inline_routes,
        report.relocated_routes,
        report.source_plain_bytes,
        report.output_plain_bytes,
        report.source_target_cnx_bytes,
        report.output_target_cnx_bytes,
        report.seq_vdd_lba,
        report.seq_vdd_bytes,
        report.seq_vdd_sectors,
        report.seq_vdd_sha256,
        report.seq_vdd_repacked,
        report.seq_vdd_relocated,
        report.disc.package_bytes,
        report.disc.body_bytes,
        report.disc.final_expected_writes,
        report.seq_vdd_exact,
        report.target_plain_exact && report.target_cnx_roundtrip_exact,
        report.non_target_plain_exact && report.non_target_cnx_exact,
        report.original_font_archive_unchanged,
        report.explicit_layout_routes,
        report.runtime_verified,
        report.product_eligible,
        report.disc.image_sha256,
    );
    {
        let resident = &report.resident_text;
        let page_ids = resident
            .writes
            .iter()
            .map(|write| write.page_id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|page_id| page_id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let record_runtimes = resident
            .writes
            .iter()
            .map(|write| format!("0x{:08X}", write.record_runtime))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "  resident units/cells/bytes {}/{}/{}→{}B, pages [{}], records [{}], \
             status {}, exact source/control/terminal/final {}/{}/{}/{}, Japanese {}, \
             approved translation/layout/runtime {}/{}/{}, product-eligible {}",
            resident.units,
            resident.glyph_cells,
            resident.source_bytes,
            resident.output_bytes,
            page_ids,
            record_runtimes,
            resident.status.label(),
            resident.source_exact,
            resident.controls_exact,
            resident.terminal_exact,
            resident.final_exact,
            resident.japanese_glyphs_remaining,
            resident.translation_approved,
            resident.layout_approved,
            resident.runtime_verified,
            resident.product_eligible,
        );
    }
    {
        let names = &report.battle_names;
        println!(
            "  battle names/page/physical/unique/writes/cells {}/{}/{}/{}/{}, slot bytes {}→{}, \
             review independent/choice {}/{}, exact source/NUL/final {}/{}/{}, Japanese {}, \
             approved translation/layout/runtime {}/{}/{}, product-eligible {}",
            names.page_id,
            names.physical_names,
            names.unique_sources,
            names.writes.len(),
            names.glyph_cells,
            names.source_bytes,
            names.output_bytes,
            names.selected_drafts,
            names.needs_human_choice,
            names.source_exact,
            names.nul_terminal_exact,
            names.final_exact,
            names.japanese_glyphs_remaining,
            names.translation_approved,
            names.layout_approved,
            names.runtime_verified,
            names.product_eligible,
        );
    }
    {
        let item = &report.item_names;
        println!(
            "  item names/page/cells {}/{}/{}, name bytes {}→{}, sequence {}→{}B, \
             pointer/NUL/combined/final exact {}/{}/{}/{}, remaining {}B, \
             review independent/choice {}/{}, approved translation/layout/runtime \
             {}/{}/{}, product-eligible {}",
            item.units,
            item.page_id,
            item.glyph_cells,
            item.source_name_bytes,
            item.output_name_bytes,
            item.source_sequence_bytes,
            item.output_sequence_bytes,
            item.pointer_readback_exact,
            item.nul_terminal_exact,
            item.combined_relink_exact,
            item.final_exact,
            item.active_window_remaining,
            item.draft_needs_independent_review,
            item.draft_needs_human_choice,
            item.translation_approved,
            item.layout_approved,
            item.runtime_verified,
            item.product_eligible,
        );
    }
    {
        let item = &report.item_descriptions;
        println!(
            "  item descriptions/pages/cells/controls {}/{:?}/{}/{}, description bytes {}→{}, \
             pointer/control/terminal/final exact {}/{}/{}/{}, remaining {}B, \
             review independent/choice {}/{}, approved translation/layout/runtime \
             {}/{}/{}, product-eligible {}",
            item.units,
            item.page_ids,
            item.glyph_cells,
            item.controls,
            item.source_description_bytes,
            item.output_description_bytes,
            item.pointer_readback_exact,
            item.controls_exact,
            item.terminal_exact,
            item.final_exact,
            item.active_window_remaining,
            item.draft_needs_independent_review,
            item.draft_needs_human_choice,
            item.translation_approved,
            item.layout_approved,
            item.runtime_verified,
            item.product_eligible,
        );
    }
    {
        let item = &report.item_acquisition_text;
        let runtimes = item
            .writes
            .iter()
            .map(|write| format!("0x{:08X}", write.runtime))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "  item acquisition/page/cells/bytes {}/{}/{} / {}→{}, calls [{}], \
             status {}, exact source/NUL/final {}/{}/{}, Japanese {}, \
             approved translation/layout/runtime {}/{}/{}, product-eligible {}",
            item.units,
            item.page_id,
            item.glyph_cells,
            item.source_bytes,
            item.output_bytes,
            runtimes,
            item.status.label(),
            item.source_exact,
            item.nul_terminal_exact,
            item.final_exact,
            item.japanese_glyphs_remaining,
            item.translation_approved,
            item.layout_approved,
            item.runtime_verified,
            item.product_eligible,
        );
    }
    {
        let item = &report.ui_literals;
        println!(
            "  UI literals/page/units/map/choice/writes/cells {}/{}/{}/{}/{}/{}, bytes {}→{}, \
             exact source/NUL/final {}/{}/{}, Japanese {}, \
             approved translation/layout/runtime {}/{}/{}, product-eligible {}",
            item.page_id,
            item.units,
            item.world_map_labels,
            item.binary_choices,
            item.writes.len(),
            item.glyph_cells,
            item.source_bytes,
            item.output_bytes,
            item.source_exact,
            item.nul_terminal_exact,
            item.final_exact,
            item.japanese_glyphs_remaining,
            item.translation_approved,
            item.layout_approved,
            item.runtime_verified,
            item.product_eligible,
        );
    }
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    println!("  TSV {}", report_path.display());
    println!("  ROUTING {}", routing_path.display());
    Ok(())
}

pub(super) fn write_quest_development_disc_outputs(
    output_dir: &Path,
    build: QuestDevelopmentDiscBuild,
    routing: QuestTranslationSelectionRoutingAnalysis,
) -> Result<(), String> {
    write_quest_translation_disc_outputs(
        output_dir,
        "quest-development",
        "QUEST authored-layout text development full-disc",
        build,
        &routing,
    )
}

pub(super) fn write_quest_glyph_cache_upload_outputs(
    output_dir: &Path,
    build: QuestGlyphCacheUploadPocBuild,
) -> Result<(), String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("{} 생성 실패: {error}", output_dir.display()))?;
    let report_path = output_dir.join("quest-glyph-cache-upload.tsv");
    if report_path.exists() {
        return Err(format!(
            "기존 산출물을 덮어쓰지 않음: {}",
            report_path.display()
        ));
    }
    let pid = std::process::id();
    let report_temp = output_dir.join(format!(".quest-glyph-cache-upload.tsv.tmp-{pid}"));
    let report_raw = render_quest_glyph_cache_upload_poc_tsv(&build.report);
    write_new_file(&report_temp, report_raw.as_bytes())?;
    let (bin_path, cue_path) =
        match write_image_outputs(output_dir, "quest-glyph-cache-upload", &build.image) {
            Ok(paths) => paths,
            Err(error) => {
                let _ = fs::remove_file(&report_temp);
                return Err(error);
            }
        };
    if let Err(error) = fs::rename(&report_temp, &report_path) {
        let _ = fs::remove_file(&report_temp);
        let _ = fs::remove_file(&bin_path);
        let _ = fs::remove_file(&cue_path);
        return Err(format!("{} 확정 실패: {error}", report_path.display()));
    }

    let input = &build.report.input;
    let disc = &build.report.disc;
    println!(
        "PASS QUEST sparse glyph cache active-upload PoC: {} / batch {} / page {}, \
         glyphs page/PoC {}/{}, slots {}..{}, record {:02X?}, stream {}B {}→{}, \
         TOWN00 CNX {}B, Expected Writes {} (machine {}, data {}, metadata {}, copy {}, \
         EDC/ECC {}), VDD/TOWN00/font {}/{}/{}, approved {}, runtime {}, release {}, \
         image SHA-256 {}",
        input.stream_id,
        input.batch_id,
        input.page_id,
        input.page_glyphs,
        input.poc_glyphs,
        input.first_slot_index,
        input.last_slot_index,
        input.routed_record,
        input.source_stream_bytes,
        input.source_stream_sha256,
        input.replacement_stream_sha256,
        input.town00_cnx_bytes,
        disc.final_expected_writes,
        disc.machine_code_writes,
        disc.data_writes,
        disc.metadata_writes,
        disc.relocation_copy_writes,
        disc.touched_sectors,
        build.report.seq_vdd_exact,
        build.report.town00_plain_exact,
        build.report.original_font_archive_unchanged,
        input.poc_translation_approved,
        build.report.runtime_verified,
        build.report.release_eligible,
        disc.image_sha256,
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    println!("  TSV {}", report_path.display());
    Ok(())
}

pub(super) fn write_quest_glyph_cache_transition_outputs(
    output_dir: &Path,
    build: QuestGlyphCacheTransitionPocBuild,
) -> Result<(), String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("{} 생성 실패: {error}", output_dir.display()))?;
    let report_path = output_dir.join("quest-glyph-cache-transition.tsv");
    if report_path.exists() {
        return Err(format!(
            "기존 산출물을 덮어쓰지 않음: {}",
            report_path.display()
        ));
    }
    let pid = std::process::id();
    let report_temp = output_dir.join(format!(".quest-glyph-cache-transition.tsv.tmp-{pid}"));
    let report_raw = render_quest_glyph_cache_transition_poc_tsv(&build.report);
    write_new_file(&report_temp, report_raw.as_bytes())?;
    let (bin_path, cue_path) =
        match write_image_outputs(output_dir, "quest-glyph-cache-transition", &build.image) {
            Ok(paths) => paths,
            Err(error) => {
                let _ = fs::remove_file(&report_temp);
                return Err(error);
            }
        };
    if let Err(error) = fs::rename(&report_temp, &report_path) {
        let _ = fs::remove_file(&report_temp);
        let _ = fs::remove_file(&bin_path);
        let _ = fs::remove_file(&cue_path);
        return Err(format!("{} 확정 실패: {error}", report_path.display()));
    }

    println!(
        "PASS QUEST sparse glyph cache transition PoC: streams {}, \
         Expected Writes {}, VDD/TOWN00/font {}/{}/{}, runtime {}, release {}, image SHA-256 {}",
        build
            .report
            .streams
            .iter()
            .map(|plan| format!("{}:page{}", plan.stream_id, plan.page_id))
            .collect::<Vec<_>>()
            .join(","),
        build.report.disc.final_expected_writes,
        build.report.seq_vdd_exact,
        build.report.town00_plain_exact,
        build.report.original_font_archive_unchanged,
        build.report.runtime_verified,
        build.report.release_eligible,
        build.report.disc.image_sha256,
    );
    for plan in &build.report.streams {
        println!(
            "  {} batch {} page {} lines {}/{}/{} stream {}B {}→{} approved {}/{}",
            plan.stream_id,
            plan.batch_id,
            plan.page_id,
            plan.line_capacities
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(","),
            plan.line_used
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(","),
            plan.line_padding
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(","),
            plan.source_stream_bytes,
            plan.source_stream_sha256,
            plan.replacement_stream_sha256,
            plan.translation_approved,
            plan.layout_approved,
        );
    }
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    println!("  TSV {}", report_path.display());
    Ok(())
}
