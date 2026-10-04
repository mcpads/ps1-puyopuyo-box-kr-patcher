//! 적용된 QUEST glyph cache disc의 구조와 payload readback.
//!
//! 최종 Mode2 이미지에서 ISO와 재배치 EXE를 다시 읽어 package, typed code,
//! 수동 text, 보호 범위와 원본 extent를 검증한다. Expected Write를 만들거나
//! 이미지를 변경하는 일은 이 module이 맡지 않는다.

use sha2::{Digest, Sha256};

use crate::coords::exe_map;
use crate::disc::{DiscImage, Iso9660, PsxExe, VerifiedSourceImage, BOX_32M_PATH};
use crate::isa::{verify_bytes, IsaProfile};

use super::super::disc_archive_patch::verify_disc_archive_patch_readback;
use super::super::quest_exe_relocation_control::{
    count_extent_overlaps, extent_intervals, verify_header_identity,
};
use super::super::quest_glyph_cache_hook_plan::QuestGlyphCacheHookPlan;
use super::super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::super::quest_page_carrier_plan::QUEST_EXE_ISO_PATH;
use super::relocation_write_plan::QuestGlyphCacheDiscWritePlan;
use super::write_validation::{
    original_text_except_ranges_exact, sha256_hex, verify_original_machine_site_readback,
};
use super::{error, QuestGlyphCacheDiscError, QuestGlyphCacheDiscSupplementalWrites};

/// 최종 이미지에서 다시 확인한 QUEST payload와 무변경 경계.
pub(super) struct QuestGlyphCacheDiscReadback {
    pub(super) image: Vec<u8>,
    pub(super) package_exact: bool,
    pub(super) site_exact: bool,
    pub(super) original_font_guard_site_exact: bool,
    pub(super) font_object_marker_site_exact: bool,
    pub(super) font_draw_descriptor_site_exact: bool,
    pub(super) font_draw_texture_page_site_exact: bool,
    pub(super) item_name_residency_site_exact: bool,
    pub(super) item_text_mode_residency_site_exact: bool,
    pub(super) item_description_residency_sites_exact: bool,
    pub(super) body_exact: bool,
    pub(super) supplemental_machine_code_exact: bool,
    pub(super) resident_text_exact: bool,
    pub(super) item_acquisition_text_exact: bool,
    pub(super) supplemental_data_exact: bool,
    pub(super) original_text_except_site_exact: bool,
    pub(super) original_text_except_declared_writes_exact: bool,
    pub(super) extension_outside_payload_zero: bool,
    pub(super) original_quest_extent_unchanged: bool,
    pub(super) extent_overlaps: usize,
    pub(super) image_sha256: String,
}

/// 적용 결과를 원본과 plan에 대조하고 검증된 image byte를 반환한다.
pub(super) fn verify_quest_glyph_cache_disc_readback(
    source: &VerifiedSourceImage,
    package: &QuestGlyphCachePackage,
    hook: &QuestGlyphCacheHookPlan,
    supplemental_writes: QuestGlyphCacheDiscSupplementalWrites<'_>,
    plan: &QuestGlyphCacheDiscWritePlan<'_>,
    output: Vec<u8>,
) -> Result<QuestGlyphCacheDiscReadback, QuestGlyphCacheDiscError> {
    let final_image = DiscImage::from_bytes(output).map_err(|source| error(source.to_string()))?;
    let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
    verify_disc_archive_patch_readback(
        &final_iso,
        supplemental_writes.archive_patches,
        supplemental_writes.archive_repacks,
    )
    .map_err(|source| error(source.to_string()))?;
    for file_placement in &plan.relocated_files {
        let file_patch = file_placement.patch;
        let final_raw = final_iso
            .read_file_at_extent(file_patch.iso_path, file_placement.extent)
            .map_err(|source| error(source.to_string()))?;
        if final_raw != file_patch.replacement {
            return Err(error(format!(
                "{}: relocation 파일 최종 readback 불일치",
                file_patch.id
            )));
        }
    }
    let final_quest_raw = final_iso
        .read_file_at_extent(QUEST_EXE_ISO_PATH, plan.relocated_quest)
        .map_err(|source| error(source.to_string()))?;
    let extent_overlaps = count_extent_overlaps(
        &extent_intervals(&final_iso).map_err(|source| error(source.to_string()))?,
    )
    .map_err(|source| error(source.to_string()))?;

    let map = exe_map("QUEST.EXE").ok_or_else(|| error("QUEST.EXE 좌표 사상이 없음"))?;
    let original_exe =
        PsxExe::parse(&plan.original_quest_raw).map_err(|source| error(source.to_string()))?;
    let final_exe = PsxExe::parse(&final_quest_raw).map_err(|source| error(source.to_string()))?;
    verify_header_identity(
        &original_exe,
        &final_exe,
        plan.relocated_text_bytes as usize,
    )
    .map_err(|source| error(source.to_string()))?;

    for site in &plan.machine_sites {
        verify_original_machine_site_readback(final_exe.text(), site)?;
    }
    let site_exact = true;
    let original_font_guard_site_exact = true;
    let font_object_marker_site_exact = true;
    let font_draw_descriptor_site_exact = true;
    let font_draw_texture_page_site_exact = true;
    let item_name_residency_site_exact = true;
    let item_text_mode_residency_site_exact = true;
    let item_description_residency_sites_exact = true;
    let supplemental_machine_code_exact = true;

    let body_text_offset = map
        .loaded_text_offset(hook.body_runtime, plan.relocated_text_bytes)
        .ok_or_else(|| error("QUEST sparse cache body text readback 사상 실패"))?;
    let final_body = final_exe
        .text()
        .get(body_text_offset..body_text_offset + hook.body.bytes().len())
        .ok_or_else(|| error("QUEST sparse cache body readback 범위 이탈"))?;
    let body_exact = final_body == hook.body.bytes();
    if !body_exact {
        return Err(error("QUEST sparse cache body 최종 readback 불일치"));
    }
    verify_bytes(IsaProfile::DECLARED, hook.body_runtime, final_body)
        .map_err(|source| error(source.to_string()))?;

    let package_text_offset = map
        .loaded_text_offset(
            crate::disasm::QUEST_PAGE_CARRIER_START_RUNTIME,
            plan.relocated_text_bytes,
        )
        .ok_or_else(|| error("QUEST sparse cache package text readback 사상 실패"))?;
    let final_package = final_exe
        .text()
        .get(package_text_offset..package_text_offset + package.bytes.len())
        .ok_or_else(|| error("QUEST sparse cache package readback 범위 이탈"))?;
    let package_exact = final_package == package.bytes
        && format!("{:x}", Sha256::digest(final_package)) == package.sha256;
    if !package_exact {
        return Err(error("QUEST sparse cache package 최종 readback 불일치"));
    }

    let resident_text_exact = plan.resident_text_writes.iter().all(|span| {
        final_exe
            .text()
            .get(span.text_start..span.text_end)
            .is_some_and(|final_span| {
                final_span == span.write.output
                    && sha256_hex(final_span) == span.write.output_sha256
            })
    });
    if !resident_text_exact {
        return Err(error("QUEST resident text 최종 readback 불일치"));
    }
    let item_acquisition_text_exact = plan.item_acquisition_text_writes.iter().all(|span| {
        final_exe
            .text()
            .get(span.text_start..span.text_end)
            .is_some_and(|final_span| {
                final_span == span.write.output
                    && sha256_hex(final_span) == span.write.output_sha256
            })
    });
    if !item_acquisition_text_exact {
        return Err(error("QUEST item acquisition text 최종 readback 불일치"));
    }
    let supplemental_data_exact = plan.data_patches.iter().all(|span| {
        final_exe
            .text()
            .get(span.text_start..span.text_end)
            .is_some_and(|final_span| {
                final_span == span.patch.replacement
                    && sha256_hex(final_span) == span.patch.replacement_sha256
            })
    });
    if !supplemental_data_exact {
        return Err(error("QUEST supplemental data 최종 readback 불일치"));
    }
    let original_text_except_site_exact = original_text_except_ranges_exact(
        original_exe.text(),
        &final_exe.text()[..original_exe.text().len()],
        &plan.protected_machine_text_ranges,
    );
    let mut declared_text_changes = Vec::with_capacity(
        plan.resident_text_writes.len()
            + plan.item_acquisition_text_writes.len()
            + plan.data_patches.len()
            + plan.protected_machine_text_ranges.len(),
    );
    declared_text_changes.extend(plan.protected_machine_text_ranges.iter().copied());
    declared_text_changes.extend(
        plan.resident_text_writes
            .iter()
            .map(|span| (span.text_start, span.text_end)),
    );
    declared_text_changes.extend(
        plan.item_acquisition_text_writes
            .iter()
            .map(|span| (span.text_start, span.text_end)),
    );
    declared_text_changes.extend(
        plan.data_patches
            .iter()
            .map(|span| (span.text_start, span.text_end)),
    );
    let original_text_except_declared_writes_exact = original_text_except_ranges_exact(
        original_exe.text(),
        &final_exe.text()[..original_exe.text().len()],
        &declared_text_changes,
    );
    if !original_text_except_declared_writes_exact {
        return Err(error(
            "QUEST sparse cache 선언된 site/resident span 밖 원본 text가 달라짐",
        ));
    }

    let extension = &final_exe.text()[original_exe.text().len()..];
    let package_extension_start = package_text_offset
        .checked_sub(original_exe.text().len())
        .ok_or_else(|| error("QUEST sparse cache package가 원본 text와 겹침"))?;
    let package_extension_end = package_extension_start + package.bytes.len();
    let body_extension_start = body_text_offset
        .checked_sub(original_exe.text().len())
        .ok_or_else(|| error("QUEST sparse cache body가 원본 text와 겹침"))?;
    let body_extension_end = body_extension_start + hook.body.bytes().len();
    if package_extension_end != body_extension_start {
        return Err(error("QUEST sparse cache package와 body가 연속하지 않음"));
    }
    let extension_outside_payload_zero = extension[..package_extension_start]
        .iter()
        .chain(extension[body_extension_end..].iter())
        .all(|byte| *byte == 0);
    if !extension_outside_payload_zero {
        return Err(error(
            "QUEST sparse cache payload 밖 extension이 전량 0이 아님",
        ));
    }

    let final_32m_raw = final_iso
        .read_file_at_extent(BOX_32M_PATH, plan.relocated_32m)
        .map_err(|source| error(source.to_string()))?;
    if final_32m_raw.len() != plan.relocated_32m.len as usize
        || final_32m_raw.iter().any(|byte| *byte != 0)
    {
        return Err(error("QUEST sparse cache 분할 뒤 32M.BIN zero 계약 불일치"));
    }
    let original_quest_extent_unchanged = source.image().raw_bytes()
        [plan.original_extent_start as usize..plan.original_extent_end as usize]
        == final_image.raw_bytes()
            [plan.original_extent_start as usize..plan.original_extent_end as usize];
    if !original_quest_extent_unchanged {
        return Err(error(
            "QUEST sparse cache 원본 EXE extent 무변경 경계 불일치",
        ));
    }

    let image = final_image.into_bytes();
    let image_sha256 = format!("{:x}", Sha256::digest(&image));
    Ok(QuestGlyphCacheDiscReadback {
        image,
        package_exact,
        site_exact,
        original_font_guard_site_exact,
        font_object_marker_site_exact,
        font_draw_descriptor_site_exact,
        font_draw_texture_page_site_exact,
        item_name_residency_site_exact,
        item_text_mode_residency_site_exact,
        item_description_residency_sites_exact,
        body_exact,
        supplemental_machine_code_exact,
        resident_text_exact,
        item_acquisition_text_exact,
        supplemental_data_exact,
        original_text_except_site_exact,
        original_text_except_declared_writes_exact,
        extension_outside_payload_zero,
        original_quest_extent_unchanged,
        extent_overlaps,
        image_sha256,
    })
}
