//! 빌드 결과 파일의 원자적 생성과 readback 출력.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use ps1_puyopuyo_box::pipeline::{
    render_quest_exe_relocation_control_tsv, render_quest_page_first_dialogue_tsv,
    render_quest_page_transition_tsv, render_quest_page_two_dialogues_tsv, AllCnxControlBuild,
    DracSingleGlyphPocBuild, Menu06DraftDiscBuild, MmSprPocBuild, P2InfoPageStressDiscBuild,
    Puyo2E6ApprovedTranslationBuild, Puyo2E6SingleGlyphPocBuild, Puyo2FullGlyphPopulationPocBuild,
    Puyo2SceneGlyphPopulationPocBuild, QuestExeRelocationControlBuild, QuestHookNoopBuild,
    QuestPageFirstDialogueBuild, QuestPageTransitionBuild, QuestPageTwoDialoguesBuild,
    QuestPocBuild,
};

pub(super) fn write_new_file(path: &Path, data: &[u8]) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("{} 임시 파일 생성 실패: {error}", path.display()))?;
    if let Err(error) = file.write_all(data) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("{} 임시 파일 쓰기 실패: {error}", path.display()));
    }
    if let Err(error) = file.sync_all() {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("{} 임시 파일 동기화 실패: {error}", path.display()));
    }
    Ok(())
}

pub(super) fn write_image_outputs(
    output_dir: &Path,
    base: &str,
    image: &[u8],
) -> Result<(PathBuf, PathBuf), String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("{} 생성 실패: {error}", output_dir.display()))?;
    let bin_path = output_dir.join(format!("{base}.bin"));
    let cue_path = output_dir.join(format!("{base}.cue"));
    if bin_path.exists() || cue_path.exists() {
        return Err(format!(
            "기존 산출물을 덮어쓰지 않음: {} / {}",
            bin_path.display(),
            cue_path.display()
        ));
    }
    let pid = std::process::id();
    let bin_temp = output_dir.join(format!(".{base}.bin.tmp-{pid}"));
    let cue_temp = output_dir.join(format!(".{base}.cue.tmp-{pid}"));
    let cue =
        format!("FILE \"{base}.bin\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\n");

    write_new_file(&bin_temp, image)?;
    if let Err(error) = write_new_file(&cue_temp, cue.as_bytes()) {
        let _ = fs::remove_file(&bin_temp);
        return Err(error);
    }
    if let Err(error) = fs::rename(&bin_temp, &bin_path) {
        let _ = fs::remove_file(&bin_temp);
        let _ = fs::remove_file(&cue_temp);
        return Err(format!("{} 확정 실패: {error}", bin_path.display()));
    }
    if let Err(error) = fs::rename(&cue_temp, &cue_path) {
        let _ = fs::remove_file(&bin_path);
        let _ = fs::remove_file(&cue_temp);
        return Err(format!("{} 확정 실패: {error}", cue_path.display()));
    }

    Ok((bin_path, cue_path))
}

pub(super) fn write_poc_outputs(output_dir: &Path, build: QuestPocBuild) -> Result<(), String> {
    let base = format!("quest-first-dialogue-{}", build.report.mode.label());
    let (bin_path, cue_path) = write_image_outputs(output_dir, &base, &build.image)?;
    println!(
        "PASS QUEST first dialogue {}: TOWN00 CNX {}→{}B, font CNX {}→{}B, \
         font plan assigned {} / provisional wide {} (remaining {}), \
         final Expected Writes {} (data {}, EDC/ECC {}), image SHA-256 {}",
        build.report.mode.label(),
        build.report.town00_cnx_before,
        build.report.town00_cnx_after,
        build.report.font_cnx_before,
        build.report.font_cnx_after,
        build.report.font_plan.assigned_glyphs,
        build.report.font_plan.provisional_wide_slots,
        build.report.font_plan.provisional_remaining,
        build.report.final_expected_writes,
        build.report.data_writes,
        build.report.touched_sectors,
        build.report.image_sha256
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    Ok(())
}

pub(super) fn write_mm_spr_poc_outputs(
    output_dir: &Path,
    build: MmSprPocBuild,
) -> Result<(), String> {
    let base = format!(
        "{}-{}",
        build.report.scope.label(),
        build.report.mode.label()
    );
    let (bin_path, cue_path) = write_image_outputs(output_dir, &base, &build.image)?;
    println!(
        "PASS {} {}: MM_SPR CNX {}→{}B, selected {}, replaced {}, \
         changed pixels {}, plain bytes {}, plain SHA-256 {}, \
         final Expected Writes {} (data {}, EDC/ECC {}), image SHA-256 {}",
        build.report.scope.label(),
        build.report.mode.label(),
        build.report.asset_cnx_before,
        build.report.asset_cnx_after,
        build.report.selected_translation_blocks,
        build.report.replacement_blocks,
        build.report.changed_pixels,
        build.report.changed_plain_bytes,
        build.report.output_plain_sha256,
        build.report.final_expected_writes,
        build.report.data_writes,
        build.report.touched_sectors,
        build.report.image_sha256
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    Ok(())
}

pub(super) fn write_drac_single_glyph_poc_outputs(
    output_dir: &Path,
    build: DracSingleGlyphPocBuild,
) -> Result<(), String> {
    let (bin_path, cue_path) =
        write_image_outputs(output_dir, "drac-single-glyph-poc", &build.image)?;
    println!(
        "PASS DRAC synthetic single-glyph PoC: glyph {}, messages {}, mappings {}, \
         SEQ {:?}, BTX {:?}, final Expected Writes {} (data {}, EDC/ECC {}), \
         image SHA-256 {}; translation/runtime unapproved",
        build.report.synthetic_glyph,
        build.report.translation_messages,
        build.report.glyph_mappings,
        build
            .report
            .sequence_assets
            .iter()
            .map(|row| (
                row.scope,
                row.cnx_before,
                row.cnx_after,
                row.used_bytes,
                row.remaining_bytes,
                row.output_plain_sha256.as_str(),
            ))
            .collect::<Vec<_>>(),
        build
            .report
            .glyph_assets
            .iter()
            .map(|row| (
                row.scope,
                row.cnx_before,
                row.cnx_after,
                row.changed_pixels,
                row.changed_plain_bytes,
                row.output_plain_sha256.as_str(),
            ))
            .collect::<Vec<_>>(),
        build.report.final_expected_writes,
        build.report.data_writes,
        build.report.touched_sectors,
        build.report.image_sha256,
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    Ok(())
}

pub(super) fn write_puyo2_scene_glyph_population_poc_outputs(
    output_dir: &Path,
    build: Puyo2SceneGlyphPopulationPocBuild,
) -> Result<(), String> {
    let (bin_path, cue_path) =
        write_image_outputs(output_dir, "puyo2-scene-glyph-population-poc", &build.image)?;
    let report = &build.report;
    println!(
        "PASS PUYO2 scene glyph population PoC: glyph {}, assets {}, candidate links {}, \
         incompatible selected {:?}, cells {}, changed pixels/bytes {}/{}, CNX {:?}, \
         final Expected Writes {} (data {}, EDC/ECC {}), image SHA-256 {}; \
         product eligible {}, translation/runtime unapproved",
        report.synthetic_glyph,
        report.assets.len(),
        report.candidate_links,
        report.incompatible_selected_assets,
        report.changed_cells,
        report.changed_pixels,
        report.changed_plain_bytes,
        report
            .assets
            .iter()
            .map(|row| (row.asset, row.cnx_before, row.cnx_after))
            .collect::<Vec<_>>(),
        report.final_expected_writes,
        report.data_writes,
        report.touched_sectors,
        report.image_sha256,
        report.product_eligible,
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    Ok(())
}

pub(super) fn write_puyo2_full_glyph_population_poc_outputs(
    output_dir: &Path,
    build: Puyo2FullGlyphPopulationPocBuild,
) -> Result<(), String> {
    let (bin_path, cue_path) =
        write_image_outputs(output_dir, "puyo2-full-glyph-population-poc", &build.image)?;
    let report = &build.report;
    let disc = &report.disc;
    println!(
        "PASS PUYO2 E6+scene full glyph population PoC: glyph {}, E6 SEQ assets {}, \
         DEMOFONT CNX {}→{}B, scene assets/links/incompatible {}/{}/{}, \
         scene cells/pixels/bytes {}/{}/{}, scene CNX {:?}, \
         GRP relocated {}, target LBA {} / {}B / {} sectors, 32M extent LBA {} / {}B, \
         Expected Writes {} (data {}, metadata {}, EDC/ECC {}), extent overlaps {}, \
         image SHA-256 {}; product eligible {}, runtime unapproved",
        report.synthetic_glyph,
        disc.sequence_assets.len(),
        disc.font_asset.cnx_before,
        disc.font_asset.cnx_after,
        report.scene_assets.len(),
        report.scene_candidate_links,
        report.scene_incompatible_selected_assets.len(),
        report.scene_changed_cells,
        report.scene_changed_pixels,
        report.scene_changed_plain_bytes,
        report
            .scene_assets
            .iter()
            .map(|row| (row.asset, row.cnx_before, row.cnx_after))
            .collect::<Vec<_>>(),
        disc.grp_relocated,
        disc.relocated_grp_lba,
        disc.relocated_grp_bytes,
        disc.relocated_grp_sectors,
        disc.relocated_32m_lba,
        disc.relocated_32m_bytes,
        disc.final_expected_writes,
        disc.data_writes,
        disc.metadata_writes,
        disc.touched_sectors,
        disc.extent_overlaps,
        disc.image_sha256,
        report.product_eligible,
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    Ok(())
}

pub(super) fn write_menu06_draft_outputs(
    output_dir: &Path,
    build: Menu06DraftDiscBuild,
) -> Result<(), String> {
    let (bin_path, cue_path) =
        write_image_outputs(output_dir, "menu06-korean-draft-poc", &build.image)?;
    let report = &build.report;
    println!(
        "PASS MENU06 Korean draft PoC: VDD targets {}, in-place/repacked {}/{}, \
         Expected Writes {} (payload {}, VDD metadata {}, EDC/ECC {}), touched sectors {}, \
         image SHA-256 {}; product eligible {}, runtime verified {}",
        report.reinsertion.rows.len(),
        report.reinsertion.in_place,
        report.reinsertion.repacked,
        report.final_expected_writes,
        report.payload_writes,
        report.vdd_metadata_writes,
        report.touched_sectors,
        report.touched_sectors,
        report.image_sha256,
        report.product_eligible,
        report.runtime_verified,
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    Ok(())
}

pub(super) fn write_p2_info_page_stress_outputs(
    output_dir: &Path,
    build: P2InfoPageStressDiscBuild,
) -> Result<(), String> {
    let (bin_path, cue_path) = write_image_outputs(
        output_dir,
        "p2-info-page-distinct-hangul-stress",
        &build.image,
    )?;
    let report = &build.report;
    println!(
        "PASS P2 INFO distinct-Hangul full-disc stress: scanned/skipped/renderable/used \
         {}/{}/{}/{}, assets/VDDs {}/{}, methods {:?}, relocations {:?}, \
         remaining 32M LBA/bytes {}/{}, conservative CNX/VDD/headroom bytes {}/{}/{}, \
         Expected Writes {} \
         (data {}, metadata {}, EDC/ECC {}), touched sectors {}, image SHA-256 {}; \
         product eligible {}, runtime verified {}",
        report.scanned_hangul,
        report.skipped_hangul,
        report.renderable_hangul,
        report.distinct_hangul,
        report.assets.len(),
        report.vdds.len(),
        report
            .vdds
            .iter()
            .map(|vdd| (vdd.scope, vdd.method.label()))
            .collect::<Vec<_>>(),
        report
            .relocations
            .iter()
            .map(|row| (row.scope, row.relocated_lba, row.relocated_bytes))
            .collect::<Vec<_>>(),
        report.relocated_32m_lba,
        report.relocated_32m_bytes,
        report.conservative_cnx_upper_bound_bytes,
        report.conservative_vdd_upper_bound_bytes,
        report.reservation_headroom_bytes,
        report.final_expected_writes,
        report.data_writes,
        report.metadata_writes,
        report.touched_sectors,
        report.touched_sectors,
        report.image_sha256,
        report.product_eligible,
        report.runtime_verified,
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    Ok(())
}

pub(super) fn write_puyo2_e6_single_glyph_poc_outputs(
    output_dir: &Path,
    build: Puyo2E6SingleGlyphPocBuild,
) -> Result<(), String> {
    let (bin_path, cue_path) =
        write_image_outputs(output_dir, "puyo2-e6-single-glyph-poc", &build.image)?;
    let report = &build.report;
    println!(
        "PASS PUYO2 E6 synthetic single-glyph PoC: glyph {}, SEQ assets {}, \
         SEQ CNX {}→{}B, DEMOFONT CNX {}→{}B, changed pixels/bytes {}/{}, \
         GRP relocation LBA {} / {}B / {} sectors, 32M remainder LBA {} / {}B, \
         Expected Writes {} (data {}, metadata {}, EDC/ECC {}), extent overlaps {}, \
         image SHA-256 {}; translation/runtime unapproved",
        report.synthetic_glyph,
        report.sequence_assets.len(),
        report.sequence_cnx_before,
        report.sequence_cnx_after,
        report.font_asset.cnx_before,
        report.font_asset.cnx_after,
        report.font_asset.changed_pixels,
        report.font_asset.changed_plain_bytes,
        report.relocated_grp_lba,
        report.relocated_grp_bytes,
        report.relocated_grp_sectors,
        report.relocated_32m_lba,
        report.relocated_32m_bytes,
        report.final_expected_writes,
        report.data_writes,
        report.metadata_writes,
        report.touched_sectors,
        report.extent_overlaps,
        report.image_sha256,
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    Ok(())
}

pub(super) fn write_puyo2_e6_approved_translation_outputs(
    output_dir: &Path,
    build: Puyo2E6ApprovedTranslationBuild,
) -> Result<(), String> {
    let (bin_path, cue_path) = write_image_outputs(output_dir, "puyo2-e6-korean", &build.image)?;
    let report = &build.report;
    let disc = &report.disc;
    println!(
        "PASS PUYO2 E6 approved translation: manifest {}, glyphs retained/Hangul/remaining \
         {}/{}/{}, SEQ assets {}, SEQ CNX {}→{}B, DEMOFONT CNX {}→{}B, \
         GRP relocation LBA {} / {}B / {} sectors, Expected Writes {} \
         (data {}, metadata {}, EDC/ECC {}), extent overlaps {}, image SHA-256 {}; \
         runtime unverified",
        report.manifest_sha256,
        report.retained_source_characters,
        report.hangul_glyphs,
        report.remaining_slots,
        disc.sequence_assets.len(),
        disc.sequence_cnx_before,
        disc.sequence_cnx_after,
        disc.font_asset.cnx_before,
        disc.font_asset.cnx_after,
        disc.relocated_grp_lba,
        disc.relocated_grp_bytes,
        disc.relocated_grp_sectors,
        disc.final_expected_writes,
        disc.data_writes,
        disc.metadata_writes,
        disc.touched_sectors,
        disc.extent_overlaps,
        disc.image_sha256,
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    Ok(())
}

pub(super) fn write_cnx_control_outputs(
    output_dir: &Path,
    build: AllCnxControlBuild,
) -> Result<(), String> {
    let (bin_path, cue_path) = write_image_outputs(output_dir, "cnx-all-control", &build.image)?;
    let report = &build.report;
    println!(
        "PASS all-CNX control: VDD {}, entries {}, CNX {} \
         (bin {}, bi_ {}, bit {}, cel {}), raw {}, CNX {}→{}B, byte-identical {}/{}, \
         final Expected Writes {} (data {}, EDC/ECC {}), image SHA-256 {}",
        report.inventory.archives,
        report.inventory.entries,
        report.inventory.cnx,
        report.inventory.bin,
        report.inventory.bi_,
        report.inventory.bit,
        report.inventory.cel,
        report.inventory.raw,
        report.original_cnx_bytes,
        report.recompressed_cnx_bytes,
        report.byte_identical_cnx,
        report.inventory.cnx,
        report.final_expected_writes,
        report.data_writes,
        report.touched_sectors,
        report.image_sha256
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    Ok(())
}

pub(super) fn write_quest_hook_noop_outputs(
    output_dir: &Path,
    build: QuestHookNoopBuild,
) -> Result<(), String> {
    let (bin_path, cue_path) = write_image_outputs(output_dir, "quest-hook-noop", &build.image)?;
    let report = &build.report;
    println!(
        "PASS QUEST no-op hook: site 0x{:08X} / {} word, body 0x{:08X} / {} words, \
         continuation 0x{:08X}, reserved {}B, final Expected Writes {} \
         (machine code {}, EDC/ECC {}), image SHA-256 {}",
        report.site_runtime,
        report.site_words,
        report.body_runtime,
        report.body_words,
        report.continuation_runtime,
        report.reserved_bytes,
        report.final_expected_writes,
        report.machine_code_writes,
        report.touched_sectors,
        report.image_sha256,
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    Ok(())
}

pub(super) fn write_quest_exe_relocation_control_outputs(
    output_dir: &Path,
    build: QuestExeRelocationControlBuild,
) -> Result<(), String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("{} 생성 실패: {error}", output_dir.display()))?;
    let report_path = output_dir.join("quest-exe-relocation-control.tsv");
    if report_path.exists() {
        return Err(format!(
            "기존 산출물을 덮어쓰지 않음: {}",
            report_path.display()
        ));
    }
    let pid = std::process::id();
    let report_temp = output_dir.join(format!(".quest-exe-relocation-control.tsv.tmp-{pid}"));
    let report_raw = render_quest_exe_relocation_control_tsv(&build.report);
    write_new_file(&report_temp, report_raw.as_bytes())?;
    let (bin_path, cue_path) =
        match write_image_outputs(output_dir, "quest-exe-relocation-control", &build.image) {
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

    let report = &build.report;
    println!(
        "PASS QUEST EXE relocation control: QUEST LBA {} / {}B / {} sectors → \
         LBA {} / {}B / {} sectors, 32M → LBA {} / {}B / {} sectors, \
         Expected Writes {} (metadata {}, exact-copy {}, EDC/ECC {}), \
         text exact {}, extension zero {}, old extent unchanged {}, overlaps {}, \
         runtime verified {}, image SHA-256 {}",
        report.original_quest_lba,
        report.original_quest_bytes,
        report.original_quest_sectors,
        report.relocated_quest_lba,
        report.relocated_quest_bytes,
        report.relocated_quest_sectors,
        report.relocated_32m_lba,
        report.relocated_32m_bytes,
        report.relocated_32m_sectors,
        report.final_expected_writes,
        report.metadata_writes,
        report.relocation_copy_writes,
        report.touched_sectors,
        report.original_text_exact,
        report.extension_zero,
        report.original_extent_unchanged,
        report.extent_overlaps,
        report.runtime_verified,
        report.image_sha256,
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    println!("  TSV {}", report_path.display());
    Ok(())
}

pub(super) fn write_quest_page_first_dialogue_outputs(
    output_dir: &Path,
    build: QuestPageFirstDialogueBuild,
) -> Result<(), String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("{} 생성 실패: {error}", output_dir.display()))?;
    let report_path = output_dir.join("quest-page-first-dialogue.tsv");
    if report_path.exists() {
        return Err(format!(
            "기존 산출물을 덮어쓰지 않음: {}",
            report_path.display()
        ));
    }
    let pid = std::process::id();
    let report_temp = output_dir.join(format!(".quest-page-first-dialogue.tsv.tmp-{pid}"));
    let report_raw = render_quest_page_first_dialogue_tsv(&build.report);
    write_new_file(&report_temp, report_raw.as_bytes())?;
    let (bin_path, cue_path) =
        match write_image_outputs(output_dir, "quest-page-first-dialogue", &build.image) {
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

    let report = &build.report;
    println!(
        "PASS QUEST first-dialogue resident page PoC: record 0x{:08X}, body \
         0x{:08X}/{}B (slack {}B), raw page 0x{:08X}/{}B, QUEST LBA {}/{}B/{} \
         sectors, 32M LBA {}/{}, Expected Writes {} (machine {}, data {}, metadata {}, \
         exact-copy {}, EDC/ECC {}), text exact {}, page exact {}, extension zero {}, \
         original font {}, old extent {}, overlaps {}, runtime {}, release {}, SHA-256 {}",
        report.target_record_runtime,
        report.body_runtime,
        report.body_bytes,
        report.body_slack_bytes,
        report.page_source_runtime,
        report.page_bytes,
        report.relocated_quest_lba,
        report.relocated_quest_bytes,
        report.relocated_quest_sectors,
        report.relocated_32m_lba,
        report.relocated_32m_bytes,
        report.final_expected_writes,
        report.machine_code_writes,
        report.data_writes,
        report.metadata_writes,
        report.relocation_copy_writes,
        report.touched_sectors,
        report.original_text_except_hook_exact,
        report.resident_page_exact,
        report.extension_outside_page_zero,
        report.original_font_archive_unchanged,
        report.original_quest_extent_unchanged,
        report.extent_overlaps,
        report.runtime_verified,
        report.release_eligible,
        report.image_sha256,
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    println!("  TSV {}", report_path.display());
    Ok(())
}

pub(super) fn write_quest_page_transition_outputs(
    output_dir: &Path,
    build: QuestPageTransitionBuild,
) -> Result<(), String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("{} 생성 실패: {error}", output_dir.display()))?;
    let report_path = output_dir.join("quest-page-transition.tsv");
    if report_path.exists() {
        return Err(format!(
            "기존 산출물을 덮어쓰지 않음: {}",
            report_path.display()
        ));
    }
    let pid = std::process::id();
    let report_temp = output_dir.join(format!(".quest-page-transition.tsv.tmp-{pid}"));
    let report_raw = render_quest_page_transition_tsv(&build.report);
    write_new_file(&report_temp, report_raw.as_bytes())?;
    let (bin_path, cue_path) =
        match write_image_outputs(output_dir, "quest-page-transition", &build.image) {
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

    let report = &build.report;
    println!(
        "PASS QUEST two-page transition PoC: Korean record 0x{:08X}, all other OP21 use original page, body \
         0x{:08X}/{}B (slack {}B), pages {} x {}B at 0x{:08X}/0x{:08X}, \
         QUEST LBA {}/{}B/{} sectors, 32M LBA {}/{}, Expected Writes {} \
         (machine {}, data {}, metadata {}, exact-copy {}, EDC/ECC {}), text exact {}, \
         pages exact {}/{}, differ {}, extension zero {}, original font {}, old extent {}, \
         overlaps {}, runtime {}, release {}, SHA-256 {}",
        report.first_record_runtime,
        report.body_runtime,
        report.body_bytes,
        report.body_slack_bytes,
        report.page_count,
        report.page_bytes,
        report.korean_page_source_runtime,
        report.original_page_source_runtime,
        report.relocated_quest_lba,
        report.relocated_quest_bytes,
        report.relocated_quest_sectors,
        report.relocated_32m_lba,
        report.relocated_32m_bytes,
        report.final_expected_writes,
        report.machine_code_writes,
        report.data_writes,
        report.metadata_writes,
        report.relocation_copy_writes,
        report.touched_sectors,
        report.original_text_except_hook_exact,
        report.korean_page_exact,
        report.original_page_exact,
        report.pages_differ,
        report.extension_outside_pages_zero,
        report.original_font_archive_unchanged,
        report.original_quest_extent_unchanged,
        report.extent_overlaps,
        report.runtime_verified,
        report.release_eligible,
        report.image_sha256,
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    println!("  TSV {}", report_path.display());
    Ok(())
}

pub(super) fn write_quest_page_two_dialogues_outputs(
    output_dir: &Path,
    build: QuestPageTwoDialoguesBuild,
) -> Result<(), String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("{} 생성 실패: {error}", output_dir.display()))?;
    let report_path = output_dir.join("quest-page-two-dialogues.tsv");
    if report_path.exists() {
        return Err(format!(
            "기존 산출물을 덮어쓰지 않음: {}",
            report_path.display()
        ));
    }
    let pid = std::process::id();
    let report_temp = output_dir.join(format!(".quest-page-two-dialogues.tsv.tmp-{pid}"));
    let report_raw = render_quest_page_two_dialogues_tsv(&build.report);
    write_new_file(&report_temp, report_raw.as_bytes())?;
    let (bin_path, cue_path) =
        match write_image_outputs(output_dir, "quest-page-two-dialogues", &build.image) {
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

    let report = &build.report;
    println!(
        "PASS QUEST two-dialogue page PoC: records 0x{:08X}/0x{:08X}, body {}B \
         (slack {}B), pages {} x {}B, mappings second/batch {}/{}, stream {}, page {}, QUEST \
         LBA {}/{}B/{} sectors, Expected Writes {} (machine {}, data {}, metadata {}, \
         exact-copy {}, EDC/ECC {}), text exact {}, pages exact {}/{}, extension zero {}, \
         overlaps {}, approved {}/{}, runtime {}, release {}, SHA-256 {}",
        report.first_record_runtime,
        report.second_record_runtime,
        report.body_bytes,
        report.body_slack_bytes,
        report.page_count,
        report.page_bytes,
        report.second_glyph_mappings,
        report.korean_page_glyph_mappings,
        report.second_stream_sha256,
        report.korean_page_sha256,
        report.relocated_quest_lba,
        report.relocated_quest_bytes,
        report.relocated_quest_sectors,
        report.final_expected_writes,
        report.machine_code_writes,
        report.data_writes,
        report.metadata_writes,
        report.relocation_copy_writes,
        report.touched_sectors,
        report.original_text_except_hook_exact,
        report.korean_page_exact,
        report.original_page_exact,
        report.extension_outside_pages_zero,
        report.extent_overlaps,
        report.translation_approved,
        report.layout_approved,
        report.runtime_verified,
        report.release_eligible,
        report.image_sha256,
    );
    println!("  BIN {}", bin_path.display());
    println!("  CUE {}", cue_path.display());
    println!("  TSV {}", report_path.display());
    Ok(())
}
