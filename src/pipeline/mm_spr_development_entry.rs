//! 셸 `MM_SPR.BTX` 검토 초안의 개발 VDD 엔트리.
//!
//! 사람 검토 전 17개 설명문의 plain BTX 합성은 graphics 모듈에 맡긴다.
//! 이 모듈은 보호 원본에 결속된 CNX replacement와 개발 상태만 소유하며,
//! 같은 `FIL_GRP.VDD`의 다른 엔트리와 합치는 일은 상위 archive 조정자가 맡는다.

use std::error::Error;
use std::fmt;

use crate::disc::VerifiedSourceImage;
use crate::graphics::mm_spr_patch::patch_mm_spr_review_translations;
use crate::graphics::mm_spr_translation::{
    verify_mm_spr_translation_manifest, MmSprTranslationReport,
};
use crate::graphics::text_catalog::MM_SPR_TEXT_ASSET;
use crate::graphics::text_patch::BakedTextPatchReport;

use super::btx_development_archive::{
    prepare_btx_development_entry, BtxDevelopmentArchiveSpec, PreparedBtxDevelopmentEntry,
};

const FIL_GRP_PATH: &str = "/FIL_GRP.VDD;1";
const MM_SPR_NAME: &str = "MM_SPR.BTX";
const MM_SPR_ENTRY_SPEC: BtxDevelopmentArchiveSpec = BtxDevelopmentArchiveSpec {
    iso_path: FIL_GRP_PATH,
    entry_name: MM_SPR_NAME,
    expected_cnx_subtype: *b"bi_",
    patch_id: "shell-mm-spr-development",
    actor: "pipeline::mm_spr_development_entry",
};

/// 검토 초안을 담은 `MM_SPR.BTX` replacement의 정적 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmSprDevelopmentEntryReport {
    pub original_cnx_bytes: usize,
    pub replacement_cnx_bytes: usize,
    pub plain_bytes: usize,
    pub cnx_roundtrip_exact: bool,
    pub final_readback_exact: bool,
    pub translation: MmSprTranslationReport,
    pub surface: BakedTextPatchReport,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// 같은 VDD의 다른 개발 엔트리와 합성할 검증 완료 `MM_SPR.BTX`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmSprDevelopmentEntryBuild {
    entry: PreparedBtxDevelopmentEntry,
    pub report: MmSprDevelopmentEntryReport,
}

impl MmSprDevelopmentEntryBuild {
    pub(super) fn entry(&self) -> &PreparedBtxDevelopmentEntry {
        &self.entry
    }

    /// 무손실 palette-index 검토 preview가 소비할 patched BTX.
    pub fn patched_plain(&self) -> &[u8] {
        self.entry.patched_plain()
    }
}

/// `MM_SPR` 번역·raster·CNX 준비 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmSprDevelopmentEntryError(String);

impl fmt::Display for MmSprDevelopmentEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for MmSprDevelopmentEntryError {}

fn error(message: impl Into<String>) -> MmSprDevelopmentEntryError {
    MmSprDevelopmentEntryError(message.into())
}

/// 지원 원본에서 현재 검토 초안 전체의 `MM_SPR.BTX` replacement를 준비한다.
pub fn prepare_mm_spr_development_entry(
    source: &VerifiedSourceImage,
) -> Result<MmSprDevelopmentEntryBuild, MmSprDevelopmentEntryError> {
    let translation =
        verify_mm_spr_translation_manifest().map_err(|source| error(source.to_string()))?;
    let (entry, surface) =
        prepare_btx_development_entry(source, MM_SPR_ENTRY_SPEC, |original_plain| {
            patch_mm_spr_review_translations(original_plain).map_err(|source| source.to_string())
        })
        .map_err(|source| error(source.to_string()))?;
    let storage = entry.storage;
    let report = MmSprDevelopmentEntryReport {
        original_cnx_bytes: storage.original_cnx_bytes,
        replacement_cnx_bytes: storage.replacement_cnx_bytes,
        plain_bytes: storage.plain_bytes,
        cnx_roundtrip_exact: storage.cnx_roundtrip_exact,
        final_readback_exact: false,
        translation,
        surface,
        runtime_verified: false,
        product_eligible: false,
    };
    let selected_translations = report
        .translation
        .japanese_targets
        .checked_sub(report.translation.pending_human_translation)
        .ok_or_else(|| error("MM_SPR 번역 상태 집계 underflow"))?;
    if report.plain_bytes
        != MM_SPR_TEXT_ASSET
            .layout
            .byte_len()
            .map_err(|source| error(source.to_string()))?
        || !report.cnx_roundtrip_exact
        || report.surface.replacement_blocks != selected_translations
        || report.surface.changed_pixels == 0
        || report.surface.changed_bytes == 0
        || report.final_readback_exact
        || report.runtime_verified
        || report.product_eligible
    {
        return Err(error("MM_SPR 개발 entry 판정 drift"));
    }
    Ok(MmSprDevelopmentEntryBuild { entry, report })
}
