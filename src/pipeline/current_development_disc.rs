//! 현재 활성 한글화 기여를 한 불변 원본 disc에 누적하는 최상위 조정자.
//!
//! 각 기능 모듈이 자체 replacement와 판정을 만들고, 이 모듈은 기여를
//! 공용 Expected Write composer에 전달한 뒤 기능별 최종 readback만 묶는다.
//! QUEST·MMENU·MM_SPR·RPG_SYS1·G_MESS·classic PUYO BTX의 내부 합성 규칙을
//! 소유하지 않는다.

use std::error::Error;
use std::fmt;

use crate::graphics::text_catalog::{
    G_MESS_TEXT_ASSET, MMENU_TEXT_ASSET, MM_SPR_TEXT_ASSET, RPG_SYS1_TEXT_ASSET,
};

use super::cnx_development_archive_replacement::{
    repack_cnx_development_entries, verify_cnx_development_archive_replacement_readback,
    CnxDevelopmentDiscContribution,
};

use super::classic_code_glyph_development_entries::{
    prepare_classic_code_glyph_development_entries, ClassicCodeGlyphDevelopmentEntriesReport,
};
use super::classic_puyo_development_archives::{
    prepare_classic_puyo_development_archives, verify_classic_puyo_development_archives_readback,
};
use super::classic_puyo_sequence_development_archives::{
    prepare_classic_puyo_sequence_development_archives,
    verify_classic_puyo_sequence_development_archives_readback,
};

use super::g_mess_development_archive::{
    prepare_g_mess_development_archive, GMessDevelopmentArchiveReport,
};
use super::general_raster_development_entries::{
    prepare_current_general_raster_development_batch, GeneralRasterDevelopmentEntriesReport,
    GeneralRasterDevelopmentSurface,
};

use super::disc_archive_patch::DiscFileContributions;
use super::mm_spr_development_entry::{
    prepare_mm_spr_development_entry, MmSprDevelopmentEntryReport,
};
use super::mmenu_development_archive::{
    prepare_mmenu_development_entry, MmenuDevelopmentEntryReport,
};
use super::p2_info_development_archives::P2InfoDevelopmentArchivesReport;
use super::quest_development_disc::{
    build_quest_development_disc_with_file_contributions, QuestDevelopmentDiscInput,
    QuestDevelopmentDiscReport,
};
use super::rpg_sys1_development_archive::{
    prepare_rpg_sys1_development_archive, RpgSys1DevelopmentArchiveReport,
};
use super::subgame_raster_development_archives::{
    prepare_subgame_raster_development_archives,
    verify_subgame_raster_development_archives_readback,
};

/// 현재 누적 개발 disc의 기능별 정적 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentDevelopmentDiscReport {
    pub quest: QuestDevelopmentDiscReport,
    pub shell_archive: ShellDevelopmentArchiveReport,
    pub mmenu: MmenuDevelopmentEntryReport,
    pub mm_spr: MmSprDevelopmentEntryReport,
    pub rpg_sys1: RpgSys1DevelopmentArchiveReport,
    pub g_mess: GMessDevelopmentArchiveReport,
    pub p2_info: P2InfoDevelopmentArchivesReport,
    pub general_raster: GeneralRasterDevelopmentEntriesReport,
    pub classic_code_glyph: ClassicCodeGlyphDevelopmentEntriesReport,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// MMENU와 MM_SPR를 한 번 재패킹한 `FIL_GRP.VDD`의 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellDevelopmentArchiveReport {
    pub archive_entries: usize,
    pub replacement_entries: usize,
    pub moved_entry_offsets: usize,
    pub vdd_bytes: usize,
    pub target_entries_exact: bool,
    pub non_target_entries_exact: bool,
    pub final_readback_exact: bool,
}

/// 현재 누적 개발 disc와 baked surface 무손실 preview 원천.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentDevelopmentDiscBuild {
    pub image: Vec<u8>,
    pub mmenu_surface: Vec<u8>,
    pub mm_spr_surface: Vec<u8>,
    pub rpg_sys1_surface: Vec<u8>,
    pub g_mess_surface: Vec<u8>,
    pub p2_info4_surface: Vec<u8>,
    pub p2_info5_surface: Vec<u8>,
    pub general_raster_surfaces: Vec<GeneralRasterDevelopmentSurface>,
    pub menu06_surface: Vec<u8>,
    pub puyo2_e6_surface: Vec<u8>,
    pub puyo1_scene_surfaces: Vec<CurrentDevelopmentSceneSurface>,
    pub report: CurrentDevelopmentDiscReport,
}

/// 작성 완료된 PUYO1 scene atlas의 동적 preview 출력.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentDevelopmentSceneSurface {
    pub preview_stem: String,
    pub bytes: Vec<u8>,
}

/// 기능 기여 조정·최종 readback 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentDevelopmentDiscError(String);

impl fmt::Display for CurrentDevelopmentDiscError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for CurrentDevelopmentDiscError {}

fn error(message: impl Into<String>) -> CurrentDevelopmentDiscError {
    CurrentDevelopmentDiscError(message.into())
}

/// 현재 기능 집합을 한 번의 누적 Expected Write로 합성한다.
pub fn build_current_development_disc(
    quest_input: QuestDevelopmentDiscInput<'_>,
) -> Result<CurrentDevelopmentDiscBuild, CurrentDevelopmentDiscError> {
    let classic_scene_spacing =
        super::classic_scene_spacing::prepare(quest_input.source).map_err(error)?;
    let subgame_help =
        super::subgame_help_development::prepare_subgame_help(quest_input.source).map_err(error)?;
    let mut mmenu = prepare_mmenu_development_entry(quest_input.source)
        .map_err(|source| error(source.to_string()))?;
    let mut mm_spr = prepare_mm_spr_development_entry(quest_input.source)
        .map_err(|source| error(source.to_string()))?;
    let mut general_raster = prepare_current_general_raster_development_batch(quest_input.source)
        .map_err(|source| error(source.to_string()))?;
    let mut classic_code_glyph = prepare_classic_code_glyph_development_entries(quest_input.source)
        .map_err(|source| error(source.to_string()))?;
    let boxparts =
        super::boxparts_development::prepare_boxparts(quest_input.source).map_err(error)?;
    let boxcube = super::boxcube_development::prepare_boxcube(quest_input.source).map_err(error)?;
    let mut shell_entries = vec![mmenu.entry(), mm_spr.entry(), &boxparts, &boxcube];
    shell_entries.extend(
        general_raster
            .entry_refs("/FIL_GRP.VDD;1")
            .map_err(|source| error(source.to_string()))?,
    );
    let shell_entry_count = shell_entries.len();
    let mut shell_archive = repack_cnx_development_entries(
        "/FIL_GRP.VDD;1",
        "shell-btx-development",
        "pipeline::current_development_disc",
        &shell_entries,
    )
    .map_err(|source| error(source.to_string()))?;
    let mut rpg_sys1 = prepare_rpg_sys1_development_archive(quest_input.source)
        .map_err(|source| error(source.to_string()))?;
    let mut g_mess = prepare_g_mess_development_archive(quest_input.source)
        .map_err(|source| error(source.to_string()))?;
    let mut classic_puyo = prepare_classic_puyo_development_archives(
        quest_input.source,
        &general_raster,
        &classic_code_glyph,
    )
    .map_err(|source| error(source.to_string()))?;
    let classic_sequences = prepare_classic_puyo_sequence_development_archives(&classic_code_glyph)
        .map_err(|source| error(source.to_string()))?;
    let official_card =
        super::official_card_development::prepare(quest_input.source).map_err(error)?;
    let subgame_raster =
        prepare_subgame_raster_development_archives(&general_raster, &g_mess, &official_card.atlas)
            .map_err(|source| error(source.to_string()))?;
    let feast_banner =
        super::quest_feast_banner::prepare_quest_feast_banner(quest_input.source).map_err(error)?;
    let battle_badges =
        super::quest_battle_badges::prepare(quest_input.source, &general_raster).map_err(error)?;
    let font_symbols = super::quest_font_symbols::prepare(quest_input.source).map_err(error)?;
    let mut quest_entries = vec![
        rpg_sys1.entry(),
        &feast_banner,
        &battle_badges,
        &font_symbols,
    ];
    quest_entries.extend(
        general_raster
            .entry_refs("/QUEST/RPG_GRP.VDD;1")
            .map_err(|e| error(e.to_string()))?,
    );
    let quest_graphics = repack_cnx_development_entries(
        "/QUEST/RPG_GRP.VDD;1",
        "quest-graphics-development",
        "pipeline::current_development_disc",
        &quest_entries,
    )
    .map_err(|source| error(source.to_string()))?;
    let archive_patches = [];
    let shell_archive_repack = match shell_archive.disc_contribution() {
        CnxDevelopmentDiscContribution::ArchiveRepack(repack) => repack,
        CnxDevelopmentDiscContribution::RelocatedFile(_) => {
            return Err(error("FIL_GRP.VDD 고정 container 재패킹 계약 drift"))
        }
    };
    let mut archive_repacks = vec![shell_archive_repack];
    match quest_graphics.disc_contribution() {
        CnxDevelopmentDiscContribution::ArchiveRepack(repack) => archive_repacks.push(repack),
        CnxDevelopmentDiscContribution::RelocatedFile(_) => {
            return Err(error("QUEST graphics 고정 container 용량 초과"))
        }
    }
    let mut relocated_files = Vec::new();
    for contribution in classic_puyo.disc_contributions() {
        match contribution {
            CnxDevelopmentDiscContribution::ArchiveRepack(repack) => archive_repacks.push(repack),
            CnxDevelopmentDiscContribution::RelocatedFile(relocated) => {
                relocated_files.push(relocated)
            }
        }
    }
    for contribution in classic_sequences.disc_contributions() {
        match contribution {
            CnxDevelopmentDiscContribution::ArchiveRepack(repack) => archive_repacks.push(repack),
            CnxDevelopmentDiscContribution::RelocatedFile(relocated) => {
                relocated_files.push(relocated)
            }
        }
    }
    for contribution in subgame_raster.disc_contributions() {
        match contribution {
            CnxDevelopmentDiscContribution::ArchiveRepack(repack) => archive_repacks.push(repack),
            CnxDevelopmentDiscContribution::RelocatedFile(relocated) => {
                relocated_files.push(relocated)
            }
        }
    }
    match official_card.sequence.disc_contribution() {
        CnxDevelopmentDiscContribution::ArchiveRepack(repack) => archive_repacks.push(repack),
        CnxDevelopmentDiscContribution::RelocatedFile(relocated) => relocated_files.push(relocated),
    }
    let archive_patch_count = archive_patches.len();
    let archive_repack_count = archive_repacks.len();
    let relocated_file_count = relocated_files.len();
    let quest = build_quest_development_disc_with_file_contributions(
        quest_input,
        DiscFileContributions {
            subgame_help: Some(&subgame_help),
            classic_scene_spacing: Some(&classic_scene_spacing),
            archive_patches: &archive_patches,
            archive_repacks: &archive_repacks,
            relocated_files: &relocated_files,
        },
    )
    .map_err(|source| error(source.to_string()))?;
    subgame_help.verify_readback(&quest.image).map_err(error)?;
    classic_scene_spacing
        .verify_readback(&quest.image)
        .map_err(error)?;
    verify_cnx_development_archive_replacement_readback(&quest.image, &official_card.sequence)
        .map_err(|e| error(e.to_string()))?;
    verify_cnx_development_archive_replacement_readback(&quest.image, &shell_archive)
        .map_err(|source| error(source.to_string()))?;
    general_raster
        .record_verified_archive_readback("/FIL_GRP.VDD;1")
        .map_err(|source| error(source.to_string()))?;
    verify_cnx_development_archive_replacement_readback(&quest.image, &quest_graphics)
        .map_err(|source| error(source.to_string()))?;
    general_raster
        .record_verified_archive_readback("/QUEST/RPG_GRP.VDD;1")
        .map_err(|e| error(e.to_string()))?;
    super::quest_feast_banner::verify_feast_palette(&quest.image).map_err(error)?;
    super::quest_battle_badges::verify_consumer(
        &crate::disc::DiscImage::from_bytes(quest.image.clone())
            .map_err(|e| error(e.to_string()))?,
    )
    .map_err(error)?;
    crate::graphics::small_battle_badge_surface::verify_consumers(
        &crate::disc::DiscImage::from_bytes(quest.image.clone())
            .map_err(|e| error(e.to_string()))?,
    )
    .map_err(error)?;
    super::mmenu_development_archive::verify_mmenu_palette(&quest.image).map_err(error)?;
    super::boxparts_development::verify_boxparts_palette(&quest.image).map_err(error)?;
    super::boxcube_development::verify_boxcube_palette(&quest.image).map_err(error)?;
    verify_subgame_raster_development_archives_readback(
        &quest.image,
        &subgame_raster,
        &mut general_raster,
    )
    .map_err(|source| error(source.to_string()))?;
    drop(archive_repacks);
    drop(relocated_files);
    verify_classic_puyo_development_archives_readback(
        &quest.image,
        &mut classic_puyo,
        &mut general_raster,
        &mut classic_code_glyph,
    )
    .map_err(|source| error(source.to_string()))?;
    verify_classic_puyo_sequence_development_archives_readback(
        &quest.image,
        &classic_sequences,
        &mut classic_code_glyph,
    )
    .map_err(|source| error(source.to_string()))?;
    mmenu.report.final_readback_exact = true;
    mm_spr.report.final_readback_exact = true;
    shell_archive.report.final_readback_exact = true;
    rpg_sys1.report.final_readback_exact = true;
    g_mess.report.final_readback_exact = true;
    let general_raster_report = general_raster
        .report()
        .map_err(|source| error(source.to_string()))?;
    let report = CurrentDevelopmentDiscReport {
        quest: quest.report,
        shell_archive: ShellDevelopmentArchiveReport {
            archive_entries: shell_archive.report.archive_entries,
            replacement_entries: shell_archive.report.replacement_entries,
            moved_entry_offsets: shell_archive.report.moved_entry_offsets,
            vdd_bytes: shell_archive.report.vdd_bytes,
            target_entries_exact: shell_archive.report.target_entries_exact,
            non_target_entries_exact: shell_archive.report.non_target_entries_exact,
            final_readback_exact: shell_archive.report.final_readback_exact,
        },
        mmenu: mmenu.report.clone(),
        mm_spr: mm_spr.report.clone(),
        rpg_sys1: rpg_sys1.report.clone(),
        g_mess: g_mess.report.clone(),
        p2_info: classic_puyo.report.p2_info.clone(),
        general_raster: general_raster_report,
        classic_code_glyph: classic_code_glyph.report.clone(),
        runtime_verified: false,
        product_eligible: false,
    };
    if report.quest.disc.archive_entry_patches != archive_patch_count
        || report.quest.disc.archive_repacks
            != archive_repack_count + usize::from(!report.quest.seq_vdd_relocated)
        || report.quest.disc.relocated_files
            != relocated_file_count + usize::from(report.quest.seq_vdd_relocated)
        || report.quest.original_font_archive_unchanged
        || report.shell_archive.replacement_entries != shell_entry_count
        || !report.shell_archive.target_entries_exact
        || !report.shell_archive.non_target_entries_exact
        || !report.shell_archive.final_readback_exact
        || !report.mmenu.final_readback_exact
        || report.mmenu.surface.labels != 6
        || report.mmenu.surface.physical_regions
            != crate::graphics::mmenu_surface::MMENU_PHYSICAL_REGIONS
        || report.mmenu.plain_bytes
            != MMENU_TEXT_ASSET
                .layout
                .byte_len()
                .map_err(|source| error(source.to_string()))?
        || !report.mm_spr.final_readback_exact
        || report.mm_spr.translation.pending_human_translation
            > report.mm_spr.translation.japanese_targets
        || report.mm_spr.surface.replacement_blocks
            != report
                .mm_spr
                .translation
                .japanese_targets
                .saturating_sub(report.mm_spr.translation.pending_human_translation)
        || report.mm_spr.plain_bytes
            != MM_SPR_TEXT_ASSET
                .layout
                .byte_len()
                .map_err(|source| error(source.to_string()))?
        || report.mm_spr.runtime_verified
        || report.mm_spr.product_eligible
        || !report.rpg_sys1.final_readback_exact
        || report.rpg_sys1.surface.replacement_blocks != 1
        || report.rpg_sys1.plain_bytes
            != RPG_SYS1_TEXT_ASSET
                .layout
                .byte_len()
                .map_err(|source| error(source.to_string()))?
        || report.rpg_sys1.surface.translation_approved
        || report.rpg_sys1.surface.profile_approved
        || report.rpg_sys1.runtime_verified
        || report.rpg_sys1.product_eligible
        || !report.g_mess.final_readback_exact
        || report.g_mess.translation.entries != report.g_mess.surface.replacement_blocks
        || report.g_mess.plain_bytes
            != G_MESS_TEXT_ASSET
                .layout
                .byte_len()
                .map_err(|source| error(source.to_string()))?
        || report.g_mess.runtime_verified
        || report.g_mess.product_eligible
        || report.p2_info.logical_pages != report.p2_info.translation.entries
        || report.p2_info.physical_assets != report.p2_info.assets.len()
        || report.p2_info.archives.iter().any(|archive| {
            !archive.target_entries_exact
                || !archive.non_target_entries_exact
                || !archive.final_readback_exact
        })
        || !report.p2_info.physical_copies_exact
        || report.p2_info.translation.product_eligible
        || report.p2_info.runtime_verified
        || report.p2_info.product_eligible
        || report.general_raster.logical_assets == 0
        || report.general_raster.physical_assets == 0
        || !report.general_raster.physical_copies_exact
        || report
            .general_raster
            .assets
            .iter()
            .any(|asset| !asset.final_readback_exact)
        || report.general_raster.runtime_verified
        || report.general_raster.product_eligible
        || !report.classic_code_glyph.menu06.final_readback_exact
        || !report.classic_code_glyph.puyo2_e6.final_readback_exact
        || !report.classic_code_glyph.puyo1_scene.final_readback_exact
        || report.classic_code_glyph.runtime_verified
        || report.classic_code_glyph.product_eligible
        || report.runtime_verified
        || report.product_eligible
    {
        return Err(error("current development disc 판정 drift"));
    }
    let general_raster_surfaces = general_raster
        .into_surfaces()
        .map_err(|source| error(source.to_string()))?;
    let puyo1_scene_surfaces = classic_code_glyph
        .puyo1_scene_surfaces
        .into_iter()
        .map(|surface| CurrentDevelopmentSceneSurface {
            preview_stem: surface.preview_stem,
            bytes: surface.bytes,
        })
        .collect();
    Ok(CurrentDevelopmentDiscBuild {
        image: quest.image,
        mmenu_surface: mmenu.patched_plain().to_vec(),
        mm_spr_surface: mm_spr.patched_plain().to_vec(),
        rpg_sys1_surface: rpg_sys1.patched_plain().to_vec(),
        g_mess_surface: g_mess.patched_plain().to_vec(),
        p2_info4_surface: classic_puyo.p2_info4_surface().to_vec(),
        p2_info5_surface: classic_puyo.p2_info5_surface().to_vec(),
        general_raster_surfaces,
        menu06_surface: classic_code_glyph.menu06_surface,
        puyo2_e6_surface: classic_code_glyph.puyo2_e6_surface,
        puyo1_scene_surfaces,
        report,
    })
}

/// 현재 누적 기능 판정을 한 행 TSV로 직렬화한다.
pub fn render_current_development_disc_tsv(report: &CurrentDevelopmentDiscReport) -> String {
    let header = [
        "image_sha256",
        "final_expected_writes",
        "archive_entry_patches",
        "archive_repacks",
        "relocated_files",
        "shell_archive_entries",
        "shell_replacement_entries",
        "shell_moved_entry_offsets",
        "shell_final_readback_exact",
        "mmenu_labels",
        "mmenu_physical_regions",
        "mmenu_changed_pixels",
        "mmenu_plain_sha256",
        "mmenu_final_readback_exact",
        "rpg_sys1_blocks",
        "rpg_sys1_changed_pixels",
        "rpg_sys1_plain_sha256",
        "rpg_sys1_final_readback_exact",
        "rpg_sys1_translation_approved",
        "rpg_sys1_profile_approved",
        "mmenu_translation_approved",
        "mmenu_profile_approved",
        "mm_spr_targets",
        "mm_spr_poc_approved",
        "mm_spr_approved",
        "mm_spr_drafts",
        "mm_spr_changed_pixels",
        "mm_spr_plain_sha256",
        "mm_spr_final_readback_exact",
        "g_mess_entries",
        "g_mess_drafts",
        "g_mess_approved",
        "g_mess_changed_pixels",
        "g_mess_plain_sha256",
        "g_mess_final_readback_exact",
        "g_mess_translation_product_eligible",
        "p2_info_pages",
        "p2_info_physical_assets",
        "p2_info_drafts",
        "p2_info_profile_approved",
        "p2_info_archive_results",
        "p2_info_placements",
        "p2_info_final_readback_exact",
        "general_raster_logical_assets",
        "general_raster_physical_assets",
        "general_raster_blocks",
        "general_raster_changed_pixels",
        "general_raster_final_readback_exact",
        "runtime_verified",
        "product_eligible",
    ];
    let placements = report
        .p2_info
        .archives
        .iter()
        .map(|archive| archive.placement)
        .collect::<Vec<_>>()
        .join(",");
    let values = [
        report.quest.disc.image_sha256.clone(),
        report.quest.disc.final_expected_writes.to_string(),
        report.quest.disc.archive_entry_patches.to_string(),
        report.quest.disc.archive_repacks.to_string(),
        report.quest.disc.relocated_files.to_string(),
        report.shell_archive.archive_entries.to_string(),
        report.shell_archive.replacement_entries.to_string(),
        report.shell_archive.moved_entry_offsets.to_string(),
        report.shell_archive.final_readback_exact.to_string(),
        report.mmenu.surface.labels.to_string(),
        report.mmenu.surface.physical_regions.to_string(),
        report.mmenu.surface.changed_pixels.to_string(),
        report.mmenu.surface.output_sha256.clone(),
        report.mmenu.final_readback_exact.to_string(),
        report.rpg_sys1.surface.replacement_blocks.to_string(),
        report.rpg_sys1.surface.changed_pixels.to_string(),
        report.rpg_sys1.surface.output_sha256.clone(),
        report.rpg_sys1.final_readback_exact.to_string(),
        report.rpg_sys1.surface.translation_approved.to_string(),
        report.rpg_sys1.surface.profile_approved.to_string(),
        report.mmenu.surface.translation_approved.to_string(),
        report.mmenu.surface.profile_approved.to_string(),
        report.mm_spr.translation.japanese_targets.to_string(),
        report.mm_spr.translation.poc_approved.to_string(),
        report.mm_spr.translation.approved.to_string(),
        report
            .mm_spr
            .translation
            .draft_human_review_required
            .to_string(),
        report.mm_spr.surface.changed_pixels.to_string(),
        report.mm_spr.surface.output_sha256.clone(),
        report.mm_spr.final_readback_exact.to_string(),
        report.g_mess.translation.entries.to_string(),
        report.g_mess.translation.drafts.to_string(),
        report.g_mess.translation.approved.to_string(),
        report.g_mess.surface.changed_pixels.to_string(),
        report.g_mess.surface.output_sha256.clone(),
        report.g_mess.final_readback_exact.to_string(),
        report.g_mess.translation.product_eligible.to_string(),
        report.p2_info.logical_pages.to_string(),
        report.p2_info.physical_assets.to_string(),
        report.p2_info.translation.draft_translations.to_string(),
        report.p2_info.translation.profile_approved.to_string(),
        report.p2_info.archives.len().to_string(),
        placements,
        report
            .p2_info
            .archives
            .iter()
            .all(|archive| archive.final_readback_exact)
            .to_string(),
        report.general_raster.logical_assets.to_string(),
        report.general_raster.physical_assets.to_string(),
        report
            .general_raster
            .assets
            .iter()
            .map(|asset| asset.replacement_blocks)
            .sum::<usize>()
            .to_string(),
        report
            .general_raster
            .assets
            .iter()
            .map(|asset| asset.changed_pixels)
            .sum::<usize>()
            .to_string(),
        report
            .general_raster
            .assets
            .iter()
            .all(|asset| asset.final_readback_exact)
            .to_string(),
        report.runtime_verified.to_string(),
        report.product_eligible.to_string(),
    ];
    format!("{}\n{}\n", header.join("\t"), values.join("\t"))
}
