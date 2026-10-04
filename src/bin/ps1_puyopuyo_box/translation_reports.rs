//! 공통 writer와 비-QUEST 번역 모집단 보고서.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use ps1_puyopuyo_box::graphics::mm_spr_translation::render_mm_spr_review_tsv;
use ps1_puyopuyo_box::graphics::text_candidates::{
    render_pending_text_review_tsv, verify_pending_text_catalog_specs, BOX_PENDING_TEXT_ASSETS,
};
use ps1_puyopuyo_box::graphics::translation_coverage::{
    render_translation_review_tsv, verify_translation_coverage,
};
use ps1_puyopuyo_box::pipeline::{
    render_box_puyo2_e6_translation_manifest_tsv,
    render_box_subgame_seq_scene_translation_manifest_tsv,
    verify_box_puyo2_e6_translation_manifest, verify_box_subgame_seq_scene_translation_manifest,
};
use ps1_puyopuyo_box::script::{
    render_box_menu06_translation_manifest_tsv, render_menu06_text_review_tsv,
    render_quest_dialogue_review_tsv, verify_box_menu06_translation_manifest,
};

use super::analysis_inputs::{
    read_menu06_text_analysis, read_subgame_seq_e6_text_analysis,
    read_subgame_seq_scene_message_analysis, read_town00_source,
};
use super::output_writers::write_new_file;
use super::usage::usage;

pub(super) fn report_translation_coverage(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    if args.count() != 0 {
        return Err(usage().to_string());
    }
    let report = verify_translation_coverage().map_err(|error| error.to_string())?;
    println!(
        "PASS BTX translation coverage: catalog Japanese {}, managed {} \
         (PoC-approved {}, approved {}, draft {}, pending {}), unmanaged {}",
        report.catalog_japanese_targets,
        report.managed_targets,
        report.poc_approved,
        report.approved,
        report.draft_human_review_required,
        report.pending_human_translation,
        report.unmanaged_targets
    );
    Ok(())
}

pub(super) fn write_report_file(
    output_path: &Path,
    label: &str,
    rows: usize,
    report: &str,
) -> Result<(), String> {
    if output_path.exists() {
        return Err(format!(
            "기존 report를 덮어쓰지 않음: {}",
            output_path.display()
        ));
    }
    let parent = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| format!("{} 생성 실패: {error}", parent.display()))?;

    let temporary_path = output_path.with_extension(format!("tsv.tmp-{}", std::process::id()));
    write_new_file(&temporary_path, report.as_bytes())?;
    if let Err(error) = fs::rename(&temporary_path, output_path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(format!("{} 확정 실패: {error}", output_path.display()));
    }
    println!(
        "PASS {label}: rows {}, SHA-256 {:x}, output {}",
        rows,
        Sha256::digest(report.as_bytes()),
        output_path.display()
    );
    Ok(())
}

pub(super) fn write_translation_review(
    args: impl Iterator<Item = std::ffi::OsString>,
    label: &str,
    rows: usize,
    report: String,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    write_report_file(output_path, label, rows, &report)
}

pub(super) fn write_mm_spr_review(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let report = render_mm_spr_review_tsv().map_err(|error| error.to_string())?;
    let rows = report.lines().skip(1).count();
    write_translation_review(args, "MM_SPR review TSV", rows, report)
}

pub(super) fn write_btx_translation_review(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let report = render_translation_review_tsv().map_err(|error| error.to_string())?;
    let coverage = verify_translation_coverage().map_err(|error| error.to_string())?;
    write_translation_review(
        args,
        "BTX translation review TSV",
        coverage.catalog_japanese_targets,
        report,
    )
}

pub(super) fn write_btx_pending_review(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let report = render_pending_text_review_tsv().map_err(|error| error.to_string())?;
    let summary = verify_pending_text_catalog_specs(&BOX_PENDING_TEXT_ASSETS)
        .map_err(|error| error.to_string())?;
    write_translation_review(
        args,
        "BTX pending extraction review TSV",
        summary.assets,
        report,
    )
}

pub(super) fn write_menu06_text_review(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_menu06_text_analysis(source_path)?;
    let report = render_menu06_text_review_tsv(&analysis);
    write_report_file(
        output_path,
        "MENU06 code-to-cell text review TSV",
        analysis.segments.len(),
        &report,
    )
}

pub(super) fn write_menu06_translation_manifest(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let text = read_menu06_text_analysis(source_path)?;
    let analysis =
        verify_box_menu06_translation_manifest(&text).map_err(|error| error.to_string())?;
    let report = render_box_menu06_translation_manifest_tsv(&analysis);
    write_report_file(
        output_path,
        "MENU06 source-comparable translation manifest TSV",
        analysis.entries.len(),
        &report,
    )?;
    println!(
        "  source/KO/padding cells {}/{}/{}, approved/draft {}/{}, \
         visible/used/remaining glyph slots {}/{}/{}, product eligible {}",
        analysis.protected_source_code_units,
        analysis.translated_code_units,
        analysis.padding_cells,
        analysis.approved,
        analysis.draft_needs_human_review,
        analysis.unique_translation_glyphs,
        analysis.plan.used_slots,
        analysis.plan.remaining_slots,
        analysis.product_eligible,
    );
    Ok(())
}

pub(super) fn write_quest_dialogue_review(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let town00 = read_town00_source(source_path)?;
    let report = render_quest_dialogue_review_tsv(&town00).map_err(|error| error.to_string())?;
    let rows = report.lines().skip(1).count();
    write_report_file(output_path, "QUEST dialogue review TSV", rows, &report)
}

pub(super) fn write_puyo2_e6_translation_review(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let text = read_subgame_seq_e6_text_analysis(source_path)?;
    let analysis =
        verify_box_puyo2_e6_translation_manifest(&text).map_err(|error| error.to_string())?;
    let report = render_box_puyo2_e6_translation_manifest_tsv(&analysis);
    write_report_file(
        output_path,
        "PUYO2 E6 source-comparable translation review TSV",
        analysis.spans,
        &report,
    )?;
    println!(
        "  sequences/streams {}/{}, source/KO code units {}/{}, approved/draft {}/{}, \
         product eligible {}",
        analysis.sequences,
        analysis.streams,
        analysis.protected_source_code_units,
        analysis.translated_code_units,
        analysis.approved,
        analysis.draft_needs_human_review,
        analysis.product_eligible,
    );
    Ok(())
}

pub(super) fn write_subgame_seq_scene_translation_review(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let messages = read_subgame_seq_scene_message_analysis(source_path)?;
    let analysis = verify_box_subgame_seq_scene_translation_manifest(&messages)
        .map_err(|error| error.to_string())?;
    let report = render_box_subgame_seq_scene_translation_manifest_tsv(&analysis);
    write_report_file(
        output_path,
        "scene-local E0 source-comparable translation review TSV",
        analysis.logical_payloads,
        &report,
    )?;
    println!(
        "  sequences {}, logical/physical {}/{}, paired/singleton {}/{}, \
         source/KO code units {}/{}, pending/draft/approved {}/{}/{}, product eligible {}",
        analysis.sequences,
        analysis.logical_payloads,
        analysis.physical_messages,
        analysis.paired_payloads,
        analysis.singleton_payloads,
        analysis.protected_source_code_units,
        analysis.translated_code_units,
        analysis.pending_human_translation,
        analysis.draft_needs_human_review,
        analysis.approved,
        analysis.product_eligible,
    );
    Ok(())
}
