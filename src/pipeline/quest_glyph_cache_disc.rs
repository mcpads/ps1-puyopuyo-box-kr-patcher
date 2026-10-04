//! QUEST sparse glyph cache package와 typed hook의 full-disc control build.
//!
//! 지원 원본 QUEST.EXE를 `32M.BIN` 선두로 확장 재배치하고, package와
//! sector 분할 typed body, OP21 site를 한 불변 원본의 `src/patch` plan에
//! 합성한다. 번역 stream은 바꾸지 않으므로 실행 기구 검증용 control이며
//! runtime·번역 승인·release 증거가 아니다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::{exe_map, PSX_EXE_HEADER_LEN};
use crate::disasm::{
    QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME, QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME,
    QUEST_FONT_HOOK_SITE_RUNTIME, QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME,
    QUEST_GLYPH_RENDERER_RUNTIME,
};
use crate::disc::{apply_form1_write_plan, VerifiedSourceImage};
use crate::isa::VerifiedMachineCode;
use crate::source::FORM1_DATA_LEN;

use super::disc_archive_patch::{DiscArchiveEntryPatch, DiscArchiveRepack, DiscRelocatedFilePatch};
use super::quest_glyph_cache_hook_plan::QuestGlyphCacheHookPlan;
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_item_acquisition_text_writer::QuestItemAcquisitionTextWrite;
use super::quest_resident_text_writer::QuestResidentTextWrite;

mod readback;
mod relocation_write_plan;
mod write_validation;

use readback::verify_quest_glyph_cache_disc_readback;
use relocation_write_plan::plan_quest_glyph_cache_disc_relocation;
use write_validation::align_up;

pub const QUEST_GLYPH_CACHE_DISC_SCHEMA_VERSION: &str = "quest-glyph-cache-disc-control-v3";

/// sparse package와 hook이 설치된 결정적 control build의 검산 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheDiscReport {
    pub schema_version: &'static str,
    pub package_sha256: String,
    pub package_bytes: usize,
    pub site_runtime: u32,
    pub original_font_guard_site_runtime: u32,
    pub font_object_marker_site_runtime: u32,
    pub font_draw_descriptor_site_runtime: u32,
    pub font_draw_texture_page_site_runtime: u32,
    pub item_name_residency_site_runtime: Option<u32>,
    pub item_text_mode_residency_site_runtime: Option<u32>,
    pub item_description_residency_site_runtimes: Vec<u32>,
    pub body_runtime: u32,
    pub body_bytes: usize,
    pub relocated_quest_lba: u32,
    pub relocated_quest_bytes: u32,
    pub relocated_quest_sectors: u32,
    pub relocated_32m_lba: u32,
    pub relocated_32m_bytes: u32,
    pub relocated_files: usize,
    pub relocated_file_bytes: usize,
    pub relocated_file_sectors: u32,
    pub archive_entry_patches: usize,
    pub archive_repacks: usize,
    pub machine_code_writes: usize,
    pub data_writes: usize,
    pub metadata_writes: usize,
    pub relocation_copy_writes: usize,
    pub touched_sectors: usize,
    pub final_expected_writes: usize,
    pub package_exact: bool,
    pub site_exact: bool,
    pub original_font_guard_site_exact: bool,
    pub font_object_marker_site_exact: bool,
    pub font_draw_descriptor_site_exact: bool,
    pub font_draw_texture_page_site_exact: bool,
    pub item_name_residency_site_exact: bool,
    pub item_text_mode_residency_site_exact: bool,
    pub item_description_residency_sites_exact: bool,
    pub body_exact: bool,
    pub supplemental_machine_code_sites: usize,
    pub supplemental_machine_code_exact: bool,
    pub supplemental_data_patches: usize,
    pub supplemental_data_bytes: usize,
    pub supplemental_data_exact: bool,
    pub resident_text_writes: usize,
    pub resident_text_bytes: usize,
    pub resident_text_exact: bool,
    pub item_acquisition_text_writes: usize,
    pub item_acquisition_text_bytes: usize,
    pub item_acquisition_text_exact: bool,
    pub original_text_except_site_exact: bool,
    pub original_text_except_declared_writes_exact: bool,
    pub extension_outside_payload_zero: bool,
    pub original_quest_extent_unchanged: bool,
    pub extent_overlaps: usize,
    pub runtime_verified: bool,
    pub build_eligible: bool,
    pub image_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheDiscBuild {
    pub image: Vec<u8>,
    pub report: QuestGlyphCacheDiscReport,
}

/// 공통 hook 밖의 원본 EXE typed 기계어 한 곳을 같은 relocation에 합성한다.
pub(super) struct QuestGlyphCacheMachineCodePatch<'a> {
    pub original: &'a VerifiedMachineCode,
    pub replacement: &'a VerifiedMachineCode,
    pub id: &'static str,
    pub purpose: &'static str,
}

/// 원본 EXE의 같은 길이 data span을 source-bound replacement로 바꾼다.
#[derive(Debug)]
pub(super) struct QuestGlyphCacheDataPatch<'a> {
    pub runtime: u32,
    pub original: &'a [u8],
    pub replacement: &'a [u8],
    pub original_sha256: &'a str,
    pub replacement_sha256: &'a str,
    pub required_page_id: Option<u8>,
    pub id: &'a str,
    pub purpose: &'static str,
}

/// 공통 disc composer에 선택적으로 더할 검증된 쓰기 묶음.
#[derive(Clone, Copy, Default)]
pub(super) struct QuestGlyphCacheDiscSupplementalWrites<'a> {
    pub subgame_help: Option<&'a super::subgame_help_development::PreparedSubgameHelp>,
    pub classic_scene_spacing:
        Option<&'a super::classic_scene_spacing::PreparedClassicSceneSpacing>,
    pub archive_patches: &'a [DiscArchiveEntryPatch<'a>],
    pub archive_repacks: &'a [DiscArchiveRepack<'a>],
    pub relocated_files: &'a [DiscRelocatedFilePatch<'a>],
    pub resident_text_writes: &'a [QuestResidentTextWrite],
    pub item_acquisition_text_writes: &'a [QuestItemAcquisitionTextWrite],
    pub data_patches: &'a [QuestGlyphCacheDataPatch<'a>],
    pub machine_code_patches: &'a [QuestGlyphCacheMachineCodePatch<'a>],
}

/// sparse cache disc control의 원본·좌표·Expected Write·readback 불일치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGlyphCacheDiscError(String);

impl fmt::Display for QuestGlyphCacheDiscError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestGlyphCacheDiscError {}

fn error(message: impl Into<String>) -> QuestGlyphCacheDiscError {
    QuestGlyphCacheDiscError(message.into())
}

/// 검산된 package/site/body와 선택 VDD patch를 한 Expected Write plan으로 합성한다.
pub(super) fn build_quest_glyph_cache_disc(
    source: &VerifiedSourceImage,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    schema_version: &'static str,
    archive_patches: &[DiscArchiveEntryPatch<'_>],
    archive_repacks: &[DiscArchiveRepack<'_>],
    relocated_files: &[DiscRelocatedFilePatch<'_>],
) -> Result<QuestGlyphCacheDiscBuild, QuestGlyphCacheDiscError> {
    build_quest_glyph_cache_disc_with_resident_text(
        source,
        package,
        hook,
        schema_version,
        QuestGlyphCacheDiscSupplementalWrites {
            subgame_help: None,
            classic_scene_spacing: None,
            archive_patches,
            archive_repacks,
            relocated_files,
            resident_text_writes: &[],
            item_acquisition_text_writes: &[],
            data_patches: &[],
            machine_code_patches: &[],
        },
    )
}

/// 검산된 resident text span까지 같은 불변 원본 Expected Write plan에 합성한다.
pub(super) fn build_quest_glyph_cache_disc_with_resident_text<'a>(
    source: &VerifiedSourceImage,
    package: &QuestGlyphCachePackage,
    hook: &'a QuestGlyphCacheHookPlan,
    schema_version: &'static str,
    supplemental_writes: QuestGlyphCacheDiscSupplementalWrites<'a>,
) -> Result<QuestGlyphCacheDiscBuild, QuestGlyphCacheDiscError> {
    let plan = plan_quest_glyph_cache_disc_relocation(source, package, hook, supplemental_writes)?;
    let output = apply_form1_write_plan(source.image(), &plan.writes)
        .map_err(|source| error(source.to_string()))?;
    let readback = verify_quest_glyph_cache_disc_readback(
        source,
        package,
        hook,
        supplemental_writes,
        &plan,
        output,
    )?;

    Ok(QuestGlyphCacheDiscBuild {
        image: readback.image,
        report: QuestGlyphCacheDiscReport {
            schema_version,
            package_sha256: package.sha256.clone(),
            package_bytes: package.bytes.len(),
            site_runtime: QUEST_FONT_HOOK_SITE_RUNTIME,
            original_font_guard_site_runtime: QUEST_GLYPH_RENDERER_RUNTIME,
            font_object_marker_site_runtime: QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME,
            font_draw_descriptor_site_runtime: QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME,
            font_draw_texture_page_site_runtime: QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME,
            item_name_residency_site_runtime: hook
                .item_name_residency_site
                .as_ref()
                .map(VerifiedMachineCode::base_addr),
            item_text_mode_residency_site_runtime: hook
                .item_text_mode_residency_site
                .as_ref()
                .map(VerifiedMachineCode::base_addr),
            item_description_residency_site_runtimes: hook
                .item_description_residency_sites
                .iter()
                .map(VerifiedMachineCode::base_addr)
                .collect(),
            body_runtime: hook.body_runtime,
            body_bytes: hook.body_bytes,
            relocated_quest_lba: plan.relocated_quest.lba,
            relocated_quest_bytes: plan.relocated_quest.len,
            relocated_quest_sectors: plan.relocated_quest_sectors,
            relocated_32m_lba: plan.relocated_32m.lba,
            relocated_32m_bytes: plan.relocated_32m.len,
            relocated_files: plan.relocated_files.len(),
            relocated_file_bytes: plan.relocated_file_bytes,
            relocated_file_sectors: plan.relocated_file_sectors,
            archive_entry_patches: supplemental_writes.archive_patches.len(),
            archive_repacks: supplemental_writes.archive_repacks.len(),
            machine_code_writes: plan.machine_code_writes,
            data_writes: plan.data_writes,
            metadata_writes: plan.metadata_writes,
            relocation_copy_writes: plan.relocation_copy_writes,
            touched_sectors: plan.touched_sectors,
            final_expected_writes: plan.final_expected_writes,
            package_exact: readback.package_exact,
            site_exact: readback.site_exact,
            original_font_guard_site_exact: readback.original_font_guard_site_exact,
            font_object_marker_site_exact: readback.font_object_marker_site_exact,
            font_draw_descriptor_site_exact: readback.font_draw_descriptor_site_exact,
            font_draw_texture_page_site_exact: readback.font_draw_texture_page_site_exact,
            item_name_residency_site_exact: readback.item_name_residency_site_exact,
            item_text_mode_residency_site_exact: readback.item_text_mode_residency_site_exact,
            item_description_residency_sites_exact: readback.item_description_residency_sites_exact,
            body_exact: readback.body_exact,
            supplemental_machine_code_sites: supplemental_writes.machine_code_patches.len(),
            supplemental_machine_code_exact: readback.supplemental_machine_code_exact,
            supplemental_data_patches: plan.data_patches.len(),
            supplemental_data_bytes: plan.data_patch_bytes,
            supplemental_data_exact: readback.supplemental_data_exact,
            resident_text_writes: plan.resident_text_writes.len(),
            resident_text_bytes: plan.resident_text_bytes,
            resident_text_exact: readback.resident_text_exact,
            item_acquisition_text_writes: plan.item_acquisition_text_writes.len(),
            item_acquisition_text_bytes: plan.item_acquisition_text_bytes,
            item_acquisition_text_exact: readback.item_acquisition_text_exact,
            original_text_except_site_exact: readback.original_text_except_site_exact,
            original_text_except_declared_writes_exact: readback
                .original_text_except_declared_writes_exact,
            extension_outside_payload_zero: readback.extension_outside_payload_zero,
            original_quest_extent_unchanged: readback.original_quest_extent_unchanged,
            extent_overlaps: readback.extent_overlaps,
            runtime_verified: false,
            build_eligible: false,
            image_sha256: readback.image_sha256,
        },
    })
}

/// package/site/body만 설치하는 exact page-0 control image를 만든다.
pub fn build_quest_glyph_cache_disc_control(
    source: &VerifiedSourceImage,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
) -> Result<QuestGlyphCacheDiscBuild, QuestGlyphCacheDiscError> {
    build_quest_glyph_cache_disc(
        source,
        package,
        hook,
        QUEST_GLYPH_CACHE_DISC_SCHEMA_VERSION,
        &[],
        &[],
        &[],
    )
}

/// 지원 원본과 현재 package/hook에서 파생한 control image를 검증한다.
pub fn verify_box_quest_glyph_cache_disc_control(
    source: &VerifiedSourceImage,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
) -> Result<QuestGlyphCacheDiscBuild, QuestGlyphCacheDiscError> {
    let build = build_quest_glyph_cache_disc_control(source, package, hook)?;
    let report = &build.report;
    let map = exe_map("QUEST.EXE").ok_or_else(|| error("QUEST.EXE 좌표 사상이 없음"))?;
    let body_end_runtime = hook
        .body_runtime
        .checked_add(
            u32::try_from(hook.body_bytes)
                .map_err(|_| error("QUEST sparse cache hook body 길이 변환 실패"))?,
        )
        .ok_or_else(|| error("QUEST sparse cache hook body runtime 끝 overflow"))?;
    let expected_text_bytes = align_up(
        body_end_runtime
            .checked_sub(map.load_addr)
            .ok_or_else(|| error("QUEST sparse cache hook이 load 주소보다 앞섬"))?,
        FORM1_DATA_LEN as u32,
    )?;
    let expected_file_bytes = expected_text_bytes
        .checked_add(
            u32::try_from(PSX_EXE_HEADER_LEN)
                .map_err(|_| error("PS-X EXE header 길이 변환 실패"))?,
        )
        .ok_or_else(|| error("QUEST sparse cache EXE 파일 길이 overflow"))?;
    let expected_final_writes = report
        .machine_code_writes
        .checked_add(report.data_writes)
        .and_then(|count| count.checked_add(report.metadata_writes))
        .and_then(|count| count.checked_add(report.relocation_copy_writes))
        .and_then(|count| count.checked_add(report.touched_sectors))
        .ok_or_else(|| error("QUEST sparse cache Expected Write 수 overflow"))?;
    if report.relocated_quest_bytes != expected_file_bytes
        || report.relocated_quest_sectors != expected_file_bytes.div_ceil(FORM1_DATA_LEN as u32)
        || report.relocated_files != 0
        || report.relocated_file_bytes != 0
        || report.relocated_file_sectors != 0
        || report.archive_entry_patches != 0
        || report.archive_repacks != 0
        || report.resident_text_writes != 0
        || report.resident_text_bytes != 0
        || report.item_acquisition_text_writes != 0
        || report.item_acquisition_text_bytes != 0
        || report.supplemental_data_patches != 0
        || report.supplemental_data_bytes != 0
        || report.package_sha256 != package.sha256
        || report.package_bytes != package.bytes.len()
        || report.site_runtime != hook.site.base_addr()
        || report.original_font_guard_site_runtime != hook.original_font_guard_site.base_addr()
        || report.font_object_marker_site_runtime != hook.font_object_marker_site.base_addr()
        || report.font_draw_descriptor_site_runtime != hook.font_draw_descriptor_site.base_addr()
        || report.font_draw_texture_page_site_runtime
            != hook.font_draw_texture_page_site.base_addr()
        || report.body_runtime != hook.body_runtime
        || report.body_bytes != hook.body.bytes().len()
        || report.final_expected_writes != expected_final_writes
        || report.image_sha256 != format!("{:x}", Sha256::digest(&build.image))
    {
        return Err(error(format!(
            "QUEST sparse cache disc control 현재 입력 파생값 불일치: bytes/sectors {}/{}",
            report.relocated_quest_bytes, report.relocated_quest_sectors
        )));
    }
    if !report.package_exact
        || !report.site_exact
        || !report.original_font_guard_site_exact
        || !report.font_object_marker_site_exact
        || !report.font_draw_descriptor_site_exact
        || !report.font_draw_texture_page_site_exact
        || !report.body_exact
        || !report.resident_text_exact
        || !report.item_acquisition_text_exact
        || !report.supplemental_data_exact
        || !report.original_text_except_site_exact
        || !report.original_text_except_declared_writes_exact
        || !report.extension_outside_payload_zero
        || !report.original_quest_extent_unchanged
        || report.extent_overlaps != 0
        || report.runtime_verified
        || report.build_eligible
    {
        return Err(error("QUEST sparse cache disc control 판정 drift"));
    }
    Ok(build)
}
