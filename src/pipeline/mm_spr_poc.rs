//! 셸 `MM_SPR.BTX` 첫 일본어 설명문 한글 PoC.
//!
//! 원본 또는 한글 BTX를 CNX로 다시 압축하고 `FIL_GRP.VDD`를 고정
//! 컨테이너 안에서 재패킹한 뒤, Mode2/2352 Expected Write와 최종
//! readback까지 한 번에 검증한다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::compress::{compress, decompress, parse_header};
use crate::disc::{apply_form1_write_plan, DiscImage, Iso9660, VerifiedSourceImage};
use crate::graphics::mm_spr_patch::{
    patch_mm_spr_approved_poc_translations, patch_mm_spr_first_description,
};
use crate::graphics::mm_spr_translation::{
    approved_poc_translation, approved_poc_translations, MM_SPR_FIRST_DESCRIPTION_BLOCK_ID,
};
use crate::graphics::text_catalog::{verify_text_asset, MM_SPR_TEXT_ASSET};
use crate::patch::WritePlan;

use super::vdd_plan::{add_vdd_repack, logical_file};

const FIL_GRP_PATH: &str = "/FIL_GRP.VDD;1";
const MM_SPR_NAME: &str = "MM_SPR.BTX";

/// 원문 재압축 control 또는 한글 한 문장 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmSprPocMode {
    Control,
    Korean,
}

impl MmSprPocMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Korean => "ko",
        }
    }
}

/// 첫 block 재현 또는 manifest 승인 집합을 고르는 PoC 범위.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmSprPocScope {
    FirstDescription,
    ApprovedDescriptions,
}

impl MmSprPocScope {
    pub const fn label(self) -> &'static str {
        match self {
            Self::FirstDescription => "shell-first-description",
            Self::ApprovedDescriptions => "shell-approved-descriptions",
        }
    }
}

/// 셸 첫 설명문 빌드의 정적 검증 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmSprPocReport {
    pub scope: MmSprPocScope,
    pub mode: MmSprPocMode,
    pub selected_translation_blocks: usize,
    pub asset_cnx_before: usize,
    pub asset_cnx_after: usize,
    pub replacement_blocks: usize,
    pub changed_pixels: usize,
    pub changed_plain_bytes: usize,
    pub output_plain_sha256: String,
    pub data_writes: usize,
    pub final_expected_writes: usize,
    pub touched_sectors: usize,
    pub image_sha256: String,
}

/// 완전히 검증된 셸 첫 설명문 raw BIN.
#[derive(Debug)]
pub struct MmSprPocBuild {
    pub image: Vec<u8>,
    pub report: MmSprPocReport,
}

/// 셸 첫 설명문 파이프라인 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmSprPocError(String);

impl fmt::Display for MmSprPocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for MmSprPocError {}

fn error(message: impl Into<String>) -> MmSprPocError {
    MmSprPocError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn verify_final_asset(
    iso: &Iso9660<'_>,
    original_archive: &VddArchive,
    expected_vdd: &[u8],
    expected_plain: &[u8],
    expected_subtype: [u8; 3],
) -> Result<(), MmSprPocError> {
    let final_vdd = iso
        .read_file(FIL_GRP_PATH)
        .map_err(|source| error(source.to_string()))?;
    if final_vdd != expected_vdd {
        return Err(error(format!("{FIL_GRP_PATH}: 최종 VDD readback 불일치")));
    }
    let archive = VddArchive::parse(&final_vdd).map_err(|source| error(source.to_string()))?;
    if archive.entry_count() != original_archive.entry_count() {
        return Err(error(format!("{FIL_GRP_PATH}: 최종 VDD 엔트리 수 불일치")));
    }
    for original_entry in original_archive.entries() {
        if original_entry.name().eq_ignore_ascii_case(MM_SPR_NAME) {
            continue;
        }
        let final_entry = archive.find(original_entry.name()).ok_or_else(|| {
            error(format!(
                "{FIL_GRP_PATH}:{} 최종 readback 없음",
                original_entry.name()
            ))
        })?;
        if archive.read_file(final_entry) != original_archive.read_file(original_entry) {
            return Err(error(format!(
                "{FIL_GRP_PATH}:{} 비대상 엔트리 내용이 바뀜",
                original_entry.name()
            )));
        }
    }
    let entry = archive
        .find(MM_SPR_NAME)
        .ok_or_else(|| error(format!("{FIL_GRP_PATH}:{MM_SPR_NAME} 최종 readback 없음")))?;
    let final_cnx = archive.read_file(entry);
    let header = parse_header(final_cnx).map_err(|source| error(source.to_string()))?;
    if header.subtype != expected_subtype {
        return Err(error(format!(
            "{FIL_GRP_PATH}:{MM_SPR_NAME} 최종 CNX subtype 불일치"
        )));
    }
    let final_plain = decompress(final_cnx).map_err(|source| error(source.to_string()))?;
    if final_plain != expected_plain {
        return Err(error(format!(
            "{FIL_GRP_PATH}:{MM_SPR_NAME} 최종 해제 readback 불일치"
        )));
    }
    Ok(())
}

/// 지원 원본에서 control 또는 한글 셸 첫 설명문 raw BIN을 만든다.
pub fn build_mm_spr_first_description(
    source: &VerifiedSourceImage,
    mode: MmSprPocMode,
) -> Result<MmSprPocBuild, MmSprPocError> {
    build_mm_spr(source, mode, MmSprPocScope::FirstDescription)
}

/// 지원 원본에서 manifest의 PoC 승인 설명문을 모두 반영한 raw BIN을 만든다.
pub fn build_mm_spr_approved_descriptions(
    source: &VerifiedSourceImage,
    mode: MmSprPocMode,
) -> Result<MmSprPocBuild, MmSprPocError> {
    build_mm_spr(source, mode, MmSprPocScope::ApprovedDescriptions)
}

fn build_mm_spr(
    source: &VerifiedSourceImage,
    mode: MmSprPocMode,
    scope: MmSprPocScope,
) -> Result<MmSprPocBuild, MmSprPocError> {
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let iso_entry = iso
        .find(FIL_GRP_PATH)
        .map_err(|source| error(source.to_string()))?
        .clone();
    let file = logical_file(&iso_entry).map_err(error)?;
    let original_vdd = iso
        .read_file(FIL_GRP_PATH)
        .map_err(|source| error(source.to_string()))?;
    let archive = VddArchive::parse(&original_vdd).map_err(|source| error(source.to_string()))?;
    let entry = archive
        .find(MM_SPR_NAME)
        .ok_or_else(|| error(format!("{FIL_GRP_PATH}에 {MM_SPR_NAME}이 없음")))?;
    let original_cnx = archive.read_file(entry);
    let header = parse_header(original_cnx).map_err(|source| error(source.to_string()))?;
    if header.subtype != *b"bi_" {
        return Err(error(format!(
            "{FIL_GRP_PATH}:{MM_SPR_NAME} CNX subtype이 bi_가 아님"
        )));
    }
    let original_plain = decompress(original_cnx).map_err(|source| error(source.to_string()))?;
    verify_text_asset(&original_plain, MM_SPR_TEXT_ASSET)
        .map_err(|source| error(source.to_string()))?;
    let selected_translation_blocks = match scope {
        MmSprPocScope::FirstDescription => {
            approved_poc_translation(MM_SPR_FIRST_DESCRIPTION_BLOCK_ID)
                .map_err(|source| error(source.to_string()))?;
            1
        }
        MmSprPocScope::ApprovedDescriptions => approved_poc_translations()
            .map_err(|source| error(source.to_string()))?
            .len(),
    };

    let (new_plain, replacement_blocks, changed_pixels, changed_plain_bytes, output_plain_sha256) =
        match mode {
            MmSprPocMode::Control => (original_plain.clone(), 0, 0, 0, digest(&original_plain)),
            MmSprPocMode::Korean => {
                let (patched, report) = match scope {
                    MmSprPocScope::FirstDescription => {
                        patch_mm_spr_first_description(&original_plain)
                    }
                    MmSprPocScope::ApprovedDescriptions => {
                        patch_mm_spr_approved_poc_translations(&original_plain)
                    }
                }
                .map_err(|source| error(source.to_string()))?;
                (
                    patched,
                    report.replacement_blocks,
                    report.changed_pixels,
                    report.changed_bytes,
                    report.output_sha256,
                )
            }
        };
    let new_cnx =
        compress(&new_plain, header.subtype).map_err(|source| error(source.to_string()))?;
    if decompress(&new_cnx).map_err(|source| error(source.to_string()))? != new_plain {
        return Err(error(format!(
            "{FIL_GRP_PATH}:{MM_SPR_NAME} 재압축 자기 readback 불일치"
        )));
    }

    let new_vdd = archive
        .repack_in_container(&BTreeMap::from([(
            MM_SPR_NAME.to_string(),
            new_cnx.clone(),
        )]))
        .map_err(|source| error(source.to_string()))?;
    let mut data_plan = WritePlan::new();
    add_vdd_repack(
        &mut data_plan,
        file,
        &original_vdd,
        &new_vdd,
        "shell-mm-spr",
        "pipeline::mm_spr_poc",
    )
    .map_err(error)?;
    let data_writes = data_plan.writes.len();
    let touched_sectors = data_plan.touched_sectors().len();
    let final_expected_writes = data_writes + touched_sectors;
    let output = apply_form1_write_plan(source.image(), &data_plan)
        .map_err(|source| error(source.to_string()))?;

    let final_image = DiscImage::from_bytes(output).map_err(|source| error(source.to_string()))?;
    let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
    verify_final_asset(&final_iso, &archive, &new_vdd, &new_plain, header.subtype)?;
    let output = final_image.into_bytes();
    let image_sha256 = digest(&output);

    Ok(MmSprPocBuild {
        image: output,
        report: MmSprPocReport {
            scope,
            mode,
            selected_translation_blocks,
            asset_cnx_before: original_cnx.len(),
            asset_cnx_after: new_cnx.len(),
            replacement_blocks,
            changed_pixels,
            changed_plain_bytes,
            output_plain_sha256,
            data_writes,
            final_expected_writes,
            touched_sectors,
            image_sha256,
        },
    })
}

#[cfg(test)]
#[path = "mm_spr_poc_tests.rs"]
mod tests;
