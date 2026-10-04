//! 수동 조판 QUEST stream·sparse glyph cache·typed hook의 full-disc 개발 빌드.
//!
//! 12개 대상 SEQ를 각각 재압축한 뒤 `RPG_SEQ.VDD`를 한 번만 재패킹하고,
//! package와 hook을 같은 불변 원본의 `src/patch` plan에 합성한다. 명시적
//! 수동 조판이 없는 stream은 원문을 유지한다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::compress::decompress;
use crate::disc::{DiscImage, Iso9660, VerifiedSourceImage};
use crate::font::QuestCodeTable;
use crate::script::QUEST_ITEM_TEXT_SEQUENCE;

use super::disc_archive_patch::{
    expected_archive_after_changes, DiscArchiveEntryPatch, DiscArchiveRepack,
    DiscFileContributions, DiscRelocatedFilePatch,
};
use super::quest_battle_name_writer::{
    verify_quest_battle_name_writer_code_supply, QuestBattleNameWrite,
    QuestBattleNameWriterAnalysis,
};
use super::quest_glyph_cache_disc::{
    build_quest_glyph_cache_disc_with_resident_text, QuestGlyphCacheDataPatch,
    QuestGlyphCacheDiscReport, QuestGlyphCacheDiscSupplementalWrites,
    QuestGlyphCacheMachineCodePatch,
};
use super::quest_glyph_cache_hook_plan::QuestGlyphCacheHookPlan;
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_item_acquisition_text_compositions::QuestItemAcquisitionTextCompositionStatus;
use super::quest_item_acquisition_text_writer::{
    verify_quest_item_acquisition_text_writer_code_supply, QuestItemAcquisitionTextWrite,
    QuestItemAcquisitionTextWriterAnalysis,
};
use super::quest_item_name_writer::QuestItemNameWrite;
use super::quest_item_text_writer::{
    verify_quest_item_text_writer_code_supply, QuestItemDescriptionWrite,
    QuestItemTextWriterAnalysis,
};
use super::quest_manual_stream_writer::QuestManualStreamWriterAnalysis;
use super::quest_resident_text_writer::{
    verify_quest_resident_text_writer_code_supply, QuestResidentTextCompositionStatus,
    QuestResidentTextWrite, QuestResidentTextWriterAnalysis,
};
use super::quest_translation_encoding::QuestTranslationEncodingPlan;
use super::quest_ui_literal_compositions::QuestUiLiteralCompositionStatus;
use super::quest_ui_literal_writer::{
    verify_quest_ui_literal_writer_code_supply, QuestUiLiteralWrite, QuestUiLiteralWriterAnalysis,
};

mod report;
mod sequence_archive;
mod text_reports;

pub use report::render_quest_development_disc_tsv;
use sequence_archive::prepare_development_seq_archive;
use text_reports::{
    prepare_battle_name_report, prepare_item_acquisition_text_report,
    prepare_item_description_report, prepare_item_name_report, prepare_resident_text_report,
    prepare_ui_literal_report,
};

pub const QUEST_DEVELOPMENT_DISC_SCHEMA_VERSION: &str = "quest-development-disc-v2";

const RPG_SEQ_PATH: &str = "/QUEST/RPG_SEQ.VDD;1";
const RPG_GRP_PATH: &str = "/QUEST/RPG_GRP.VDD;1";

/// disc에 합성한 resident text 개발 입력과 최종 readback 경계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentDiscResidentTextReport {
    pub writes: Vec<QuestResidentTextWrite>,
    pub units: usize,
    pub glyph_cells: usize,
    pub source_bytes: usize,
    pub output_bytes: usize,
    pub status: QuestResidentTextCompositionStatus,
    pub source_exact: bool,
    pub controls_exact: bool,
    pub terminal_exact: bool,
    pub final_exact: bool,
    pub japanese_glyphs_remaining: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// disc에 합성한 자연 조우 전투 이름과 최종 EXE readback 경계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentDiscBattleNameReport {
    pub writes: Vec<QuestBattleNameWrite>,
    pub page_id: u8,
    pub physical_names: usize,
    pub unique_sources: usize,
    pub glyph_cells: usize,
    pub source_bytes: usize,
    pub output_bytes: usize,
    pub selected_drafts: usize,
    pub needs_human_choice: usize,
    pub source_exact: bool,
    pub nul_terminal_exact: bool,
    pub final_exact: bool,
    pub japanese_glyphs_remaining: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// disc에 합성한 `ITEM_DAT` 수동 이름과 최종 VDD/EXE readback 경계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentDiscItemNameReport {
    pub writes: Vec<QuestItemNameWrite>,
    pub page_id: u8,
    pub units: usize,
    pub glyph_cells: usize,
    pub source_name_bytes: usize,
    pub output_name_bytes: usize,
    pub source_sequence_bytes: usize,
    pub output_sequence_bytes: usize,
    pub source_sequence_sha256: String,
    pub output_sequence_sha256: String,
    pub active_window_remaining: usize,
    pub draft_needs_independent_review: usize,
    pub draft_needs_human_choice: usize,
    pub source_exact: bool,
    pub pointer_readback_exact: bool,
    pub nul_terminal_exact: bool,
    pub combined_relink_exact: bool,
    pub final_exact: bool,
    pub japanese_glyphs_remaining: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// disc에 합성한 `ITEM_DAT` 수동 설명과 최종 VDD/EXE readback 경계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentDiscItemDescriptionReport {
    pub writes: Vec<QuestItemDescriptionWrite>,
    pub page_ids: Vec<u8>,
    pub units: usize,
    pub glyph_cells: usize,
    pub controls: usize,
    pub source_description_bytes: usize,
    pub output_description_bytes: usize,
    pub active_window_remaining: usize,
    pub draft_needs_independent_review: usize,
    pub draft_needs_human_choice: usize,
    pub source_exact: bool,
    pub pointer_readback_exact: bool,
    pub controls_exact: bool,
    pub terminal_exact: bool,
    pub final_exact: bool,
    pub japanese_glyphs_remaining: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// disc에 합성한 아이템 획득 고정 문자열과 최종 EXE readback 경계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentDiscItemAcquisitionTextReport {
    pub writes: Vec<QuestItemAcquisitionTextWrite>,
    pub page_id: u8,
    pub units: usize,
    pub glyph_cells: usize,
    pub source_bytes: usize,
    pub output_bytes: usize,
    pub status: QuestItemAcquisitionTextCompositionStatus,
    pub source_exact: bool,
    pub nul_terminal_exact: bool,
    pub final_exact: bool,
    pub japanese_glyphs_remaining: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// disc에 합성한 world-map 지명·공용 선택지와 최종 EXE readback 경계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentDiscUiLiteralReport {
    pub writes: Vec<QuestUiLiteralWrite>,
    pub page_id: u8,
    pub units: usize,
    pub world_map_labels: usize,
    pub binary_choices: usize,
    pub glyph_cells: usize,
    pub source_bytes: usize,
    pub output_bytes: usize,
    pub status: QuestUiLiteralCompositionStatus,
    pub source_exact: bool,
    pub nul_terminal_exact: bool,
    pub final_exact: bool,
    pub japanese_glyphs_remaining: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// 수동 조판 번역과 ITEM_DAT 텍스트를 결합한 개발 disc의 정적 검산 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentDiscReport {
    pub schema_version: &'static str,
    pub sequence_entries: usize,
    pub target_sequence_entries: usize,
    pub non_target_sequence_entries: usize,
    pub selected_routes: usize,
    pub retained_source_routes: usize,
    pub inline_routes: usize,
    pub relocated_routes: usize,
    pub source_plain_bytes: usize,
    pub output_plain_bytes: usize,
    pub source_target_cnx_bytes: usize,
    pub output_target_cnx_bytes: usize,
    pub seq_vdd_lba: u32,
    pub seq_vdd_bytes: usize,
    pub seq_vdd_sectors: u32,
    pub seq_vdd_sha256: String,
    pub seq_vdd_repacked: bool,
    pub seq_vdd_relocated: bool,
    pub seq_vdd_exact: bool,
    pub target_plain_exact: bool,
    pub target_cnx_roundtrip_exact: bool,
    pub non_target_plain_exact: bool,
    pub non_target_cnx_exact: bool,
    pub original_font_archive_unchanged: bool,
    pub explicit_layout_routes: usize,
    pub translation_approved_routes: usize,
    pub layout_approved_routes: usize,
    pub runtime_verified: bool,
    pub product_eligible: bool,
    pub resident_text: QuestDevelopmentDiscResidentTextReport,
    pub battle_names: QuestDevelopmentDiscBattleNameReport,
    pub item_names: QuestDevelopmentDiscItemNameReport,
    pub item_descriptions: QuestDevelopmentDiscItemDescriptionReport,
    pub item_acquisition_text: QuestDevelopmentDiscItemAcquisitionTextReport,
    pub ui_literals: QuestDevelopmentDiscUiLiteralReport,
    pub disc: QuestGlyphCacheDiscReport,
}

/// 검증된 QUEST 번역 writer를 한 exact development disc로 만드는 입력.
pub struct QuestDevelopmentDiscInput<'a> {
    pub source: &'a VerifiedSourceImage,
    pub manual_writer: &'a QuestManualStreamWriterAnalysis,
    pub package: &'a QuestGlyphCachePackage,
    pub hook: &'a QuestGlyphCacheHookPlan,
    pub resident_text_writer: &'a QuestResidentTextWriterAnalysis,
    pub battle_name_writer: &'a QuestBattleNameWriterAnalysis,
    pub item_text_writer: &'a QuestItemTextWriterAnalysis,
    pub item_acquisition_text_writer: &'a QuestItemAcquisitionTextWriterAnalysis,
    pub ui_literal_writer: &'a QuestUiLiteralWriterAnalysis,
    pub encoding: &'a QuestTranslationEncodingPlan,
    pub code_table: &'a QuestCodeTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentDiscBuild {
    pub image: Vec<u8>,
    pub report: QuestDevelopmentDiscReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentDiscError(String);

impl fmt::Display for QuestDevelopmentDiscError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestDevelopmentDiscError {}

fn error(message: impl Into<String>) -> QuestDevelopmentDiscError {
    QuestDevelopmentDiscError(message.into())
}

fn sha256(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

/// 검증된 QUEST resident·고정 slot·SEQ 번역을 같은 불변 원본 disc에 합성한다.
pub fn build_quest_development_disc(
    input: QuestDevelopmentDiscInput<'_>,
) -> Result<QuestDevelopmentDiscBuild, QuestDevelopmentDiscError> {
    build_quest_development_disc_with_file_contributions(input, DiscFileContributions::default())
}

/// 최상위 누적 조정자가 준비한 파일 기여를 QUEST 쓰기와 함께 합성한다.
pub(super) fn build_quest_development_disc_with_file_contributions(
    input: QuestDevelopmentDiscInput<'_>,
    file_contributions: DiscFileContributions<'_>,
) -> Result<QuestDevelopmentDiscBuild, QuestDevelopmentDiscError> {
    let DiscFileContributions {
        archive_patches,
        archive_repacks,
        relocated_files,
        subgame_help,
        classic_scene_spacing,
    } = file_contributions;
    let QuestDevelopmentDiscInput {
        source,
        manual_writer,
        package,
        hook,
        resident_text_writer,
        battle_name_writer,
        item_text_writer,
        item_acquisition_text_writer,
        ui_literal_writer,
        encoding,
        code_table,
    } = input;
    verify_quest_resident_text_writer_code_supply(
        resident_text_writer,
        encoding,
        package,
        code_table,
    )
    .map_err(|source| error(source.to_string()))?;
    verify_quest_battle_name_writer_code_supply(battle_name_writer, encoding, package, code_table)
        .map_err(|source| error(source.to_string()))?;
    verify_quest_item_text_writer_code_supply(item_text_writer, encoding, package, code_table)
        .map_err(|source| error(source.to_string()))?;
    verify_quest_item_acquisition_text_writer_code_supply(
        item_acquisition_text_writer,
        encoding,
        package,
        code_table,
    )
    .map_err(|source| error(source.to_string()))?;
    verify_quest_ui_literal_writer_code_supply(ui_literal_writer, encoding, package, code_table)
        .map_err(|source| error(source.to_string()))?;
    if hook.item_name_page_id != Some(item_text_writer.name.page_id)
        || hook
            .item_text_page_ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            != item_text_writer
                .description_page_ids
                .iter()
                .copied()
                .chain([item_text_writer.name.page_id])
                .collect()
        || package.menu_font_page != Some(item_text_writer.name.page_id)
        || item_acquisition_text_writer.page_id != item_text_writer.name.page_id
        || ui_literal_writer.page_id != item_text_writer.name.page_id
        || usize::from(battle_name_writer.page_id) != package.plan.pages
        || hook
            .item_text_page_ids
            .contains(&battle_name_writer.page_id)
        || hook.item_acquisition_whole_string_sites.len()
            != crate::disasm::QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES.len()
        || !hook.scopes_item_acquisition_fixed_message_renderer_route
        || hook.ui_literal_choice_direct_glyph_sites.len()
            != crate::disasm::QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES.len()
        || !hook.menu_literal_sites.iter().map(|s| s.base_addr()).eq(
            crate::disasm::QUEST_MENU_GLYPH_CALLS
                .iter()
                .map(|c| c.runtime),
        )
        || hook.world_map_label_whole_string_site.is_none()
        || !hook.routes_ui_literal_choices_through_item_page
        || !hook.routes_world_map_labels_through_item_page
    {
        return Err(error(
            "QUEST item/acquisition/UI/battle-name writer와 hook page 결속 불일치",
        ));
    }
    let resident_text = prepare_resident_text_report(resident_text_writer)?;
    let battle_names = prepare_battle_name_report(battle_name_writer)?;
    let item_acquisition_text = prepare_item_acquisition_text_report(item_acquisition_text_writer)?;
    let ui_literals = prepare_ui_literal_report(ui_literal_writer)?;
    build_quest_development_disc_inner(QuestDevelopmentDiscAssemblyInput {
        source,
        writer: manual_writer,
        package,
        hook,
        resident_text,
        battle_name_writer,
        battle_names,
        item_text_writer,
        item_acquisition_text,
        ui_literals,
        archive_patches,
        archive_repacks,
        relocated_files,
        subgame_help,
        classic_scene_spacing,
    })
}

struct QuestDevelopmentDiscAssemblyInput<'a, 'archive_slice, 'archive_asset> {
    subgame_help: Option<&'archive_slice super::subgame_help_development::PreparedSubgameHelp>,
    classic_scene_spacing:
        Option<&'archive_slice super::classic_scene_spacing::PreparedClassicSceneSpacing>,
    source: &'a VerifiedSourceImage,
    writer: &'a QuestManualStreamWriterAnalysis,
    package: &'a QuestGlyphCachePackage,
    hook: &'a QuestGlyphCacheHookPlan,
    resident_text: QuestDevelopmentDiscResidentTextReport,
    battle_name_writer: &'a QuestBattleNameWriterAnalysis,
    battle_names: QuestDevelopmentDiscBattleNameReport,
    item_text_writer: &'a QuestItemTextWriterAnalysis,
    item_acquisition_text: QuestDevelopmentDiscItemAcquisitionTextReport,
    ui_literals: QuestDevelopmentDiscUiLiteralReport,
    archive_patches: &'archive_slice [DiscArchiveEntryPatch<'archive_asset>],
    archive_repacks: &'archive_slice [DiscArchiveRepack<'archive_asset>],
    relocated_files: &'archive_slice [DiscRelocatedFilePatch<'archive_asset>],
}

fn build_quest_development_disc_inner(
    input: QuestDevelopmentDiscAssemblyInput<'_, '_, '_>,
) -> Result<QuestDevelopmentDiscBuild, QuestDevelopmentDiscError> {
    let QuestDevelopmentDiscAssemblyInput {
        source,
        writer,
        package,
        hook,
        mut resident_text,
        battle_name_writer,
        mut battle_names,
        item_text_writer,
        mut item_acquisition_text,
        mut ui_literals,
        archive_patches,
        archive_repacks,
        relocated_files,
        subgame_help,
        classic_scene_spacing,
    } = input;
    let mut item_names = prepare_item_name_report(item_text_writer)?;
    let mut item_descriptions = prepare_item_description_report(item_text_writer)?;
    if writer.product_eligible
        || writer.runtime_verified
        || writer.explicit_layout_routes != writer.selected_routes
        || writer.translation_approved_routes != 0
        || writer.layout_approved_routes != 0
        || package.runtime_verified
        || package.build_eligible
        || hook.runtime_verified
        || hook.build_eligible
        || hook.item_name_page_id != Some(item_text_writer.name.page_id)
        || item_acquisition_text.page_id != item_text_writer.name.page_id
        || ui_literals.page_id != item_text_writer.name.page_id
        || battle_names.page_id != battle_name_writer.page_id
        || usize::from(battle_names.page_id) != package.plan.pages
        || resident_text
            .writes
            .iter()
            .filter(|write| write.page_id == battle_names.page_id)
            .count()
            != 1
    {
        return Err(error("QUEST development disc 입력 계약 drift"));
    }
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let original_seq_vdd = iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|source| error(source.to_string()))?;
    let original_font_vdd = iso
        .read_file(RPG_GRP_PATH)
        .map_err(|source| error(source.to_string()))?;
    let prepared =
        prepare_development_seq_archive(&original_seq_vdd, writer, Some(item_text_writer))?;
    let resident_text_writes = resident_text.writes.as_slice();
    let item_acquisition_text_writes = item_acquisition_text.writes.as_slice();
    let data_patches = battle_names
        .writes
        .iter()
        .map(|write| QuestGlyphCacheDataPatch {
            runtime: write.runtime,
            original: &write.source,
            replacement: &write.output,
            original_sha256: &write.source_sha256,
            replacement_sha256: &write.output_sha256,
            required_page_id: Some(write.page_id),
            id: &write.unit_id,
            purpose: "검증된 QUEST 자연 조우 전투 이름 고정 slot 삽입",
        })
        .chain(
            ui_literals
                .writes
                .iter()
                .map(|write| QuestGlyphCacheDataPatch {
                    runtime: write.runtime,
                    original: &write.source,
                    replacement: &write.output,
                    original_sha256: &write.source_sha256,
                    replacement_sha256: &write.output_sha256,
                    required_page_id: Some(write.page_id),
                    id: &write.unit_id,
                    purpose: "검증된 QUEST world-map 지명 또는 공용 선택지 literal 삽입",
                }),
        )
        .collect::<Vec<_>>();
    let machine_code_patches: &[QuestGlyphCacheMachineCodePatch<'_>] = &[];
    let disc = if prepared.seq_vdd_relocated {
        let relocated_file = DiscRelocatedFilePatch {
            iso_path: RPG_SEQ_PATH,
            original: &original_seq_vdd,
            replacement: &prepared.replacement_vdd,
            id: "quest-development-seq-vdd",
            actor: "pipeline::quest_development_disc",
        };
        let mut combined_relocated_files = relocated_files.to_vec();
        combined_relocated_files.push(relocated_file);
        build_quest_glyph_cache_disc_with_resident_text(
            source,
            package,
            hook,
            QUEST_DEVELOPMENT_DISC_SCHEMA_VERSION,
            QuestGlyphCacheDiscSupplementalWrites {
                archive_patches,
                archive_repacks,
                relocated_files: &combined_relocated_files,
                subgame_help,
                classic_scene_spacing,
                resident_text_writes,
                item_acquisition_text_writes,
                data_patches: &data_patches,
                machine_code_patches,
            },
        )
    } else {
        let archive_repack = DiscArchiveRepack {
            iso_path: RPG_SEQ_PATH,
            original_vdd: &original_seq_vdd,
            replacement_vdd: &prepared.replacement_vdd,
            id: "quest-development-seq-vdd",
            actor: "pipeline::quest_development_disc",
        };
        let mut combined_archive_repacks = archive_repacks.to_vec();
        combined_archive_repacks.push(archive_repack);
        build_quest_glyph_cache_disc_with_resident_text(
            source,
            package,
            hook,
            QUEST_DEVELOPMENT_DISC_SCHEMA_VERSION,
            QuestGlyphCacheDiscSupplementalWrites {
                archive_patches,
                archive_repacks: &combined_archive_repacks,
                relocated_files,
                subgame_help,
                classic_scene_spacing,
                resident_text_writes,
                item_acquisition_text_writes,
                data_patches: &data_patches,
                machine_code_patches,
            },
        )
    }
    .map_err(|source| error(source.to_string()))?;

    let final_image =
        DiscImage::from_bytes(disc.image.clone()).map_err(|source| error(source.to_string()))?;
    let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
    let final_seq_vdd = final_iso
        .read_file(RPG_SEQ_PATH)
        .map_err(|source| error(source.to_string()))?;
    let final_seq_entry = final_iso
        .find(RPG_SEQ_PATH)
        .map_err(|source| error(source.to_string()))?;
    let [final_seq_extent] = final_seq_entry.extents() else {
        return Err(error(
            "QUEST development 최종 RPG_SEQ.VDD가 단일 extent가 아님",
        ));
    };
    let final_font_vdd = final_iso
        .read_file(RPG_GRP_PATH)
        .map_err(|source| error(source.to_string()))?;
    let seq_vdd_exact = final_seq_vdd == prepared.replacement_vdd;
    let original_font_archive_unchanged = final_font_vdd == original_font_vdd;
    let has_font_archive_change = archive_patches
        .iter()
        .any(|patch| patch.iso_path.eq_ignore_ascii_case(RPG_GRP_PATH))
        || archive_repacks
            .iter()
            .any(|repack| repack.iso_path.eq_ignore_ascii_case(RPG_GRP_PATH));
    let font_archive_should_be_unchanged = !has_font_archive_change;
    let expected_font_vdd = expected_archive_after_changes(
        RPG_GRP_PATH,
        &original_font_vdd,
        archive_patches,
        archive_repacks,
    )
    .map_err(|source| error(source.to_string()))?;
    let font_archive_expected_state_exact = final_font_vdd == expected_font_vdd;
    if !seq_vdd_exact || !font_archive_expected_state_exact {
        return Err(error(
            "QUEST development disc 최종 VDD/font readback 불일치",
        ));
    }
    let final_archive =
        VddArchive::parse(&final_seq_vdd).map_err(|source| error(source.to_string()))?;
    let entry = final_archive
        .find(QUEST_ITEM_TEXT_SEQUENCE)
        .ok_or_else(|| error("QUEST item text 최종 ITEM_DAT 엔트리 누락"))?;
    let final_plain =
        decompress(final_archive.read_file(entry)).map_err(|source| error(source.to_string()))?;
    if final_plain != item_text_writer.relink.control.output
        || sha256(&final_plain) != item_names.output_sequence_sha256
        || disc.report.item_name_residency_site_runtime
            != hook
                .item_name_residency_site
                .as_ref()
                .map(crate::isa::VerifiedMachineCode::base_addr)
        || !disc.report.item_name_residency_site_exact
        || disc.report.item_text_mode_residency_site_runtime
            != hook
                .item_text_mode_residency_site
                .as_ref()
                .map(crate::isa::VerifiedMachineCode::base_addr)
        || !disc.report.item_text_mode_residency_site_exact
        || disc.report.item_description_residency_site_runtimes
            != hook
                .item_description_residency_sites
                .iter()
                .map(crate::isa::VerifiedMachineCode::base_addr)
                .collect::<Vec<_>>()
        || !disc.report.item_description_residency_sites_exact
        || disc.report.supplemental_machine_code_sites != 0
        || !disc.report.supplemental_machine_code_exact
        || disc.report.archive_entry_patches != archive_patches.len()
        || disc.report.archive_repacks
            != archive_repacks.len() + usize::from(!prepared.seq_vdd_relocated)
        || disc.report.relocated_files
            != relocated_files.len() + usize::from(prepared.seq_vdd_relocated)
    {
        return Err(error("QUEST item text disc 최종 VDD/EXE readback 불일치"));
    }
    item_names.final_exact = true;
    item_descriptions.final_exact = true;
    if disc.report.resident_text_writes != resident_text.units
        || disc.report.resident_text_bytes != resident_text.output_bytes
        || !disc.report.resident_text_exact
        || disc.report.original_text_except_site_exact
        || !disc.report.original_text_except_declared_writes_exact
    {
        return Err(error(
            "QUEST development resident disc readback 분모 불일치",
        ));
    }
    resident_text.final_exact = true;
    if disc.report.item_acquisition_text_writes != item_acquisition_text.units
        || disc.report.item_acquisition_text_bytes != item_acquisition_text.output_bytes
        || !disc.report.item_acquisition_text_exact
    {
        return Err(error(
            "QUEST development item acquisition text readback 분모 불일치",
        ));
    }
    item_acquisition_text.final_exact = true;
    let expected_data_patches = battle_names
        .physical_names
        .checked_add(ui_literals.units)
        .ok_or_else(|| error("QUEST development supplemental data 분모 overflow"))?;
    let expected_data_bytes = battle_names
        .output_bytes
        .checked_add(ui_literals.output_bytes)
        .ok_or_else(|| error("QUEST development supplemental data byte 합계 overflow"))?;
    if data_patches.len() != expected_data_patches
        || disc.report.supplemental_data_patches != expected_data_patches
        || disc.report.supplemental_data_bytes != expected_data_bytes
        || !disc.report.supplemental_data_exact
    {
        return Err(error(
            "QUEST development battle-name/UI literal disc readback 분모 불일치",
        ));
    }
    battle_names.final_exact = true;
    ui_literals.final_exact = true;

    let sequence_entries = writer.sequences.len();
    let output_plain_bytes = writer
        .output_bytes
        .checked_sub(item_names.source_sequence_bytes)
        .and_then(|value| value.checked_add(item_names.output_sequence_bytes))
        .ok_or_else(|| error("QUEST item name output plain byte 합계 overflow"))?;
    let report = QuestDevelopmentDiscReport {
        schema_version: QUEST_DEVELOPMENT_DISC_SCHEMA_VERSION,
        sequence_entries,
        target_sequence_entries: prepared.target_sequence_entries,
        non_target_sequence_entries: prepared.non_target_sequence_entries,
        selected_routes: writer.selected_routes,
        retained_source_routes: writer.retained_source_routes,
        inline_routes: writer.inline_routes,
        relocated_routes: writer.relocated_routes,
        source_plain_bytes: writer.source_bytes,
        output_plain_bytes,
        source_target_cnx_bytes: prepared.source_target_cnx_bytes,
        output_target_cnx_bytes: prepared.output_target_cnx_bytes,
        seq_vdd_lba: final_seq_extent.lba,
        seq_vdd_bytes: final_seq_vdd.len(),
        seq_vdd_sectors: final_seq_extent
            .len
            .div_ceil(crate::source::FORM1_DATA_LEN as u32),
        seq_vdd_sha256: sha256(&final_seq_vdd),
        seq_vdd_repacked: prepared.seq_vdd_repacked,
        seq_vdd_relocated: prepared.seq_vdd_relocated,
        seq_vdd_exact,
        target_plain_exact: prepared.target_plain_exact,
        target_cnx_roundtrip_exact: prepared.target_cnx_roundtrip_exact,
        non_target_plain_exact: prepared.non_target_plain_exact,
        non_target_cnx_exact: prepared.non_target_cnx_exact,
        original_font_archive_unchanged,
        explicit_layout_routes: writer.explicit_layout_routes,
        translation_approved_routes: writer.translation_approved_routes,
        layout_approved_routes: writer.layout_approved_routes,
        runtime_verified: false,
        product_eligible: false,
        resident_text,
        battle_names,
        item_names,
        item_descriptions,
        item_acquisition_text,
        ui_literals,
        disc: disc.report,
    };
    if report.sequence_entries
        != report.target_sequence_entries + report.non_target_sequence_entries
        || report.target_sequence_entries == 0
        || report.selected_routes == 0
        || report.seq_vdd_sha256.len() != 64
        || report.disc.relocated_files
            != relocated_files.len() + usize::from(report.seq_vdd_relocated)
        || !report.seq_vdd_exact
        || !report.target_plain_exact
        || !report.target_cnx_roundtrip_exact
        || !report.non_target_plain_exact
        || !report.non_target_cnx_exact
        || report.original_font_archive_unchanged != font_archive_should_be_unchanged
        || report.explicit_layout_routes != writer.explicit_layout_routes
        || report.explicit_layout_routes != report.selected_routes
        || report.translation_approved_routes != 0
        || report.layout_approved_routes != 0
        || report.runtime_verified
        || report.product_eligible
        || {
            let resident = &report.resident_text;
            resident.units != resident.writes.len()
                || resident.source_bytes != resident.output_bytes
                || resident.status != QuestResidentTextCompositionStatus::ManualDraftNeedsGeometry
                || !resident.source_exact
                || !resident.controls_exact
                || !resident.terminal_exact
                || !resident.final_exact
                || resident.japanese_glyphs_remaining != 0
                || resident.translation_approved
                || resident.layout_approved
                || resident.runtime_verified
                || resident.product_eligible
        }
        || {
            let item = &report.item_names;
            item.page_id == 0
                || item.units != item.writes.len()
                || item.draft_needs_independent_review + item.draft_needs_human_choice != item.units
                || item.source_sequence_sha256.len() != 64
                || item.output_sequence_sha256.len() != 64
                || !item.source_exact
                || !item.pointer_readback_exact
                || !item.nul_terminal_exact
                || !item.combined_relink_exact
                || !item.final_exact
                || item.japanese_glyphs_remaining != 0
                || item.translation_approved
                || item.layout_approved
                || item.runtime_verified
                || item.product_eligible
        }
        || {
            let names = &report.battle_names;
            names.page_id == 0
                || names.physical_names != names.writes.len()
                || names.unique_sources == 0
                || names.unique_sources > names.physical_names
                || names.selected_drafts + names.needs_human_choice != names.physical_names
                || names.source_bytes != names.output_bytes
                || !names.source_exact
                || !names.nul_terminal_exact
                || !names.final_exact
                || names.japanese_glyphs_remaining != 0
                || names.translation_approved
                || names.layout_approved
                || names.runtime_verified
                || names.product_eligible
        }
        || {
            let item = &report.item_descriptions;
            item.page_ids.is_empty()
                || item.page_ids.contains(&0)
                || item.units != item.writes.len()
                || item.draft_needs_independent_review + item.draft_needs_human_choice != item.units
                || !item.source_exact
                || !item.pointer_readback_exact
                || !item.controls_exact
                || !item.terminal_exact
                || !item.final_exact
                || item.japanese_glyphs_remaining != 0
                || item.translation_approved
                || item.layout_approved
                || item.runtime_verified
                || item.product_eligible
        }
        || {
            let item = &report.item_acquisition_text;
            item.page_id == 0
                || item.units != item.writes.len()
                || item.source_bytes != item.output_bytes
                || item.status
                    != QuestItemAcquisitionTextCompositionStatus::ManualDraftNeedsRuntimeGeometry
                || !item.source_exact
                || !item.nul_terminal_exact
                || !item.final_exact
                || item.japanese_glyphs_remaining != 0
                || item.translation_approved
                || item.layout_approved
                || item.runtime_verified
                || item.product_eligible
        }
        || report.disc.runtime_verified
        || report.disc.build_eligible
    {
        return Err(error("QUEST development disc 최종 판정 drift"));
    }

    Ok(QuestDevelopmentDiscBuild {
        image: disc.image,
        report,
    })
}

#[cfg(test)]
#[path = "quest_development_disc_tests.rs"]
mod tests;
