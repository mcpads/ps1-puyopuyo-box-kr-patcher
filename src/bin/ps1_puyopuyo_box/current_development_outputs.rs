//! 현재 누적 development disc와 기능별 검산·preview의 원자적 출력.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ps1_puyopuyo_box::font::DEMO_FONT_LAYOUT;
use ps1_puyopuyo_box::graphics::menu06_glyph_patch::MENU06_GLYPH_ATLAS_LAYOUT;
use ps1_puyopuyo_box::graphics::text_catalog::{
    G_MESS_TEXT_ASSET, MMENU_TEXT_ASSET, MM_SPR_TEXT_ASSET, P2_INFO4_TEXT_ASSET,
    P2_INFO5_TEXT_ASSET, RPG_SYS1_TEXT_ASSET,
};
use ps1_puyopuyo_box::pipeline::{
    render_classic_code_glyph_development_entries_tsv, render_current_development_disc_tsv,
    render_general_raster_development_entries_tsv, render_p2_info_development_archives_tsv,
    render_quest_development_disc_tsv, render_quest_translation_selection_routing_tsv,
    CurrentDevelopmentDiscBuild, QuestTranslationSelectionRoutingAnalysis,
    SCENE_GLYPH_ATLAS_LAYOUT,
};
use ps1_puyopuyo_box::review::render_btx_family_preview_bmp;

use super::output_writers::{write_image_outputs, write_new_file};

fn remove_outputs<'a>(paths: impl IntoIterator<Item = &'a Path>) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

struct StagedOutput {
    final_path: PathBuf,
    temporary_path: PathBuf,
    bytes: Vec<u8>,
}

fn staged_output(output_dir: &Path, filename: String, pid: u32, bytes: Vec<u8>) -> StagedOutput {
    StagedOutput {
        final_path: output_dir.join(&filename),
        temporary_path: output_dir.join(format!(".{filename}.tmp-{pid}")),
        bytes,
    }
}

pub(super) fn write_current_development_disc_outputs(
    output_dir: &Path,
    build: CurrentDevelopmentDiscBuild,
    routing: QuestTranslationSelectionRoutingAnalysis,
) -> Result<(), String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("{} 생성 실패: {error}", output_dir.display()))?;
    let base = "current-development";
    let pid = std::process::id();
    let mut staged_outputs = vec![
        staged_output(
            output_dir,
            format!("{base}.tsv"),
            pid,
            render_current_development_disc_tsv(&build.report).into_bytes(),
        ),
        staged_output(
            output_dir,
            format!("{base}-quest.tsv"),
            pid,
            render_quest_development_disc_tsv(&build.report.quest).into_bytes(),
        ),
        staged_output(
            output_dir,
            format!("{base}-routing.tsv"),
            pid,
            render_quest_translation_selection_routing_tsv(&routing).into_bytes(),
        ),
        staged_output(
            output_dir,
            format!("{base}-p2-info.tsv"),
            pid,
            render_p2_info_development_archives_tsv(&build.report.p2_info).into_bytes(),
        ),
        staged_output(
            output_dir,
            format!("{base}-general-raster.tsv"),
            pid,
            render_general_raster_development_entries_tsv(&build.report.general_raster)
                .into_bytes(),
        ),
        staged_output(
            output_dir,
            format!("{base}-classic-code-glyph.tsv"),
            pid,
            render_classic_code_glyph_development_entries_tsv(&build.report.classic_code_glyph)
                .into_bytes(),
        ),
        staged_output(
            output_dir,
            format!("{base}-mmenu.bmp"),
            pid,
            render_btx_family_preview_bmp(&build.mmenu_surface, MMENU_TEXT_ASSET.layout)
                .map_err(|error| error.to_string())?,
        ),
        staged_output(
            output_dir,
            format!("{base}-mm-spr.bmp"),
            pid,
            render_btx_family_preview_bmp(&build.mm_spr_surface, MM_SPR_TEXT_ASSET.layout)
                .map_err(|error| error.to_string())?,
        ),
        staged_output(
            output_dir,
            format!("{base}-rpg-sys1.bmp"),
            pid,
            render_btx_family_preview_bmp(&build.rpg_sys1_surface, RPG_SYS1_TEXT_ASSET.layout)
                .map_err(|error| error.to_string())?,
        ),
        staged_output(
            output_dir,
            format!("{base}-g-mess.bmp"),
            pid,
            render_btx_family_preview_bmp(&build.g_mess_surface, G_MESS_TEXT_ASSET.layout)
                .map_err(|error| error.to_string())?,
        ),
        staged_output(
            output_dir,
            format!("{base}-p2-info4.bmp"),
            pid,
            render_btx_family_preview_bmp(&build.p2_info4_surface, P2_INFO4_TEXT_ASSET.layout)
                .map_err(|error| error.to_string())?,
        ),
        staged_output(
            output_dir,
            format!("{base}-p2-info5.bmp"),
            pid,
            render_btx_family_preview_bmp(&build.p2_info5_surface, P2_INFO5_TEXT_ASSET.layout)
                .map_err(|error| error.to_string())?,
        ),
        staged_output(
            output_dir,
            format!("{base}-menu06.bmp"),
            pid,
            render_btx_family_preview_bmp(&build.menu06_surface, MENU06_GLYPH_ATLAS_LAYOUT)
                .map_err(|error| error.to_string())?,
        ),
        staged_output(
            output_dir,
            format!("{base}-puyo2-e6.bmp"),
            pid,
            render_btx_family_preview_bmp(&build.puyo2_e6_surface, DEMO_FONT_LAYOUT)
                .map_err(|error| error.to_string())?,
        ),
    ];
    for surface in &build.puyo1_scene_surfaces {
        staged_outputs.push(staged_output(
            output_dir,
            format!("{base}-{}.bmp", surface.preview_stem),
            pid,
            render_btx_family_preview_bmp(&surface.bytes, SCENE_GLYPH_ATLAS_LAYOUT)
                .map_err(|error| error.to_string())?,
        ));
    }
    for surface in &build.general_raster_surfaces {
        staged_outputs.push(staged_output(
            output_dir,
            format!("{base}-{}.bmp", surface.preview_stem),
            pid,
            render_btx_family_preview_bmp(&surface.bytes, surface.layout)
                .map_err(|error| error.to_string())?,
        ));
    }
    let bin_path = output_dir.join(format!("{base}.bin"));
    let cue_path = output_dir.join(format!("{base}.cue"));
    let mut unique_final_paths = BTreeSet::new();
    if staged_outputs
        .iter()
        .any(|output| !unique_final_paths.insert(output.final_path.clone()))
    {
        return Err("current development 산출물 경로 중복".to_string());
    }
    if let Some(existing) = staged_outputs
        .iter()
        .map(|output| output.final_path.as_path())
        .chain([bin_path.as_path(), cue_path.as_path()])
        .find(|path| path.exists())
    {
        return Err(format!(
            "기존 산출물을 덮어쓰지 않음: {}",
            existing.display()
        ));
    }
    for output in &staged_outputs {
        if let Err(error) = write_new_file(&output.temporary_path, &output.bytes) {
            remove_outputs(
                staged_outputs
                    .iter()
                    .map(|output| output.temporary_path.as_path()),
            );
            return Err(error);
        }
    }
    if let Err(error) = write_image_outputs(output_dir, base, &build.image) {
        remove_outputs(
            staged_outputs
                .iter()
                .map(|output| output.temporary_path.as_path()),
        );
        return Err(error);
    }
    for output in &staged_outputs {
        if let Err(error) = fs::rename(&output.temporary_path, &output.final_path) {
            remove_outputs(
                staged_outputs
                    .iter()
                    .map(|output| output.temporary_path.as_path()),
            );
            remove_outputs(
                staged_outputs
                    .iter()
                    .map(|output| output.final_path.as_path())
                    .chain([bin_path.as_path(), cue_path.as_path()]),
            );
            return Err(format!(
                "{} 확정 실패: {error}",
                output.final_path.display()
            ));
        }
    }

    let report = &build.report;
    let p2_info_placements = report
        .p2_info
        .archives
        .iter()
        .map(|archive| archive.placement)
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "PASS current cumulative development disc: QUEST selected/retained {}/{}, \
         resident/battle/UI writes {}/{}/{}, \
         MMENU labels/regions {}/{}, pixels {}, CNX {}→{}B, \
         MM_SPR targets/drafts/approved {}/{}/{}, pixels {}, CNX {}→{}B, \
         RPG_SYS1 blocks {}, pixels {}, CNX {}→{}B, archive contributions {}, \
         archive repacks {}, shell entries/replacements/moved {}/{}/{}, \
         G_MESS blocks/drafts/approved {}/{}/{}, pixels {}, CNX {}→{}B, \
         P2 INFO pages/physical/archives {}/{}/{}, placements {}, \
         general raster logical/physical {}/{}, \
         relocated files {}, \
         Expected Writes {}, runtime {}, product-eligible {}, image SHA-256 {}",
        report.quest.selected_routes,
        report.quest.retained_source_routes,
        report.quest.resident_text.units,
        report.quest.battle_names.physical_names,
        report.quest.ui_literals.units,
        report.mmenu.surface.labels,
        report.mmenu.surface.physical_regions,
        report.mmenu.surface.changed_pixels,
        report.mmenu.original_cnx_bytes,
        report.mmenu.replacement_cnx_bytes,
        report.mm_spr.translation.japanese_targets,
        report.mm_spr.translation.draft_human_review_required,
        report.mm_spr.translation.approved,
        report.mm_spr.surface.changed_pixels,
        report.mm_spr.original_cnx_bytes,
        report.mm_spr.replacement_cnx_bytes,
        report.rpg_sys1.surface.replacement_blocks,
        report.rpg_sys1.surface.changed_pixels,
        report.rpg_sys1.original_cnx_bytes,
        report.rpg_sys1.replacement_cnx_bytes,
        report.quest.disc.archive_entry_patches,
        report.quest.disc.archive_repacks,
        report.shell_archive.archive_entries,
        report.shell_archive.replacement_entries,
        report.shell_archive.moved_entry_offsets,
        report.g_mess.translation.entries,
        report.g_mess.translation.drafts,
        report.g_mess.translation.approved,
        report.g_mess.surface.changed_pixels,
        report.g_mess.original_cnx_bytes,
        report.g_mess.replacement_cnx_bytes,
        report.p2_info.logical_pages,
        report.p2_info.physical_assets,
        report.p2_info.archives.len(),
        p2_info_placements,
        report.general_raster.logical_assets,
        report.general_raster.physical_assets,
        report.quest.disc.relocated_files,
        report.quest.disc.final_expected_writes,
        report.runtime_verified,
        report.product_eligible,
        report.quest.disc.image_sha256,
    );
    for path in staged_outputs
        .iter()
        .map(|output| output.final_path.as_path())
        .chain([bin_path.as_path(), cue_path.as_path()])
    {
        println!("  {}", path.display());
    }
    Ok(())
}
