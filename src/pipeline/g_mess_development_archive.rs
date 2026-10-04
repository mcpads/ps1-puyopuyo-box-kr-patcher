//! GOCHA1 결과 문장 `G_MESS.BTX`의 개발 archive 기여.
//!
//! 사람 승인 전 초안의 plain BTX 합성은 graphics 모듈에 맡긴다. 이 모듈은
//! CNX 왕복, `TUU_GRP.VDD` 제자리 교체와 누적 disc 최종 readback만 소유한다.

use std::error::Error;
use std::fmt;

use crate::disc::VerifiedSourceImage;
use crate::graphics::g_mess_translation::{
    patch_gocha_result_message_drafts, verify_gocha_result_message_manifest,
    BtxRasterTranslationReport,
};
use crate::graphics::text_catalog::G_MESS_TEXT_ASSET;
use crate::graphics::text_patch::BakedTextPatchReport;

use super::btx_development_archive::{
    prepare_btx_development_archive, verify_btx_development_archive_readback,
    BtxDevelopmentArchiveSpec, PreparedBtxDevelopmentArchive, PreparedBtxDevelopmentEntry,
};
use super::disc_archive_patch::DiscArchiveEntryPatch;

const TUU_GRP_PATH: &str = "/GOCHA1/TUU_GRP.VDD;1";
const G_MESS_NAME: &str = "G_MESS.BTX";
const G_MESS_ARCHIVE_SPEC: BtxDevelopmentArchiveSpec = BtxDevelopmentArchiveSpec {
    iso_path: TUU_GRP_PATH,
    entry_name: G_MESS_NAME,
    expected_cnx_subtype: *b"bi_",
    patch_id: "gocha1-g-mess-development",
    actor: "pipeline::g_mess_development_archive",
};

/// 결과 문장 초안을 담은 제자리 VDD 교체의 정적 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GMessDevelopmentArchiveReport {
    pub original_cnx_bytes: usize,
    pub replacement_cnx_bytes: usize,
    pub plain_bytes: usize,
    pub vdd_bytes: usize,
    pub cnx_roundtrip_exact: bool,
    pub entry_offset_unchanged: bool,
    pub non_target_entries_exact: bool,
    pub final_readback_exact: bool,
    pub translation: BtxRasterTranslationReport,
    pub surface: BakedTextPatchReport,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// 누적 disc composer에 넘길 검증 완료 `TUU_GRP.VDD` 기여.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GMessDevelopmentArchiveBuild {
    archive: PreparedBtxDevelopmentArchive,
    pub report: GMessDevelopmentArchiveReport,
}

impl GMessDevelopmentArchiveBuild {
    pub fn archive_patch(&self) -> DiscArchiveEntryPatch<'_> {
        self.archive.archive_patch()
    }

    /// 무손실 palette-index 검토 preview가 소비할 patched BTX.
    pub fn patched_plain(&self) -> &[u8] {
        self.archive.patched_plain()
    }

    /// 같은 `TUU_GRP.VDD`의 다른 BTX와 한 replacement map으로 합칠 엔트리.
    pub(super) fn entry(&self) -> &PreparedBtxDevelopmentEntry {
        self.archive.entry()
    }
}

/// 결과 문장 archive 입력·압축·readback 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GMessDevelopmentArchiveError(String);

impl fmt::Display for GMessDevelopmentArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for GMessDevelopmentArchiveError {}

fn error(message: impl Into<String>) -> GMessDevelopmentArchiveError {
    GMessDevelopmentArchiveError(message.into())
}

/// 지원 원본에서 결과 문장 개발 surface의 제자리 VDD 기여를 준비한다.
pub fn prepare_g_mess_development_archive(
    source: &VerifiedSourceImage,
) -> Result<GMessDevelopmentArchiveBuild, GMessDevelopmentArchiveError> {
    let translation =
        verify_gocha_result_message_manifest().map_err(|source| error(source.to_string()))?;
    let (archive, surface) =
        prepare_btx_development_archive(source, G_MESS_ARCHIVE_SPEC, |original_plain| {
            patch_gocha_result_message_drafts(original_plain).map_err(|source| source.to_string())
        })
        .map_err(|source| error(source.to_string()))?;
    let storage = archive.storage;
    let report = GMessDevelopmentArchiveReport {
        original_cnx_bytes: storage.original_cnx_bytes,
        replacement_cnx_bytes: storage.replacement_cnx_bytes,
        plain_bytes: storage.plain_bytes,
        vdd_bytes: storage.vdd_bytes,
        cnx_roundtrip_exact: storage.cnx_roundtrip_exact,
        entry_offset_unchanged: storage.entry_offset_unchanged,
        non_target_entries_exact: storage.non_target_entries_exact,
        final_readback_exact: false,
        translation,
        surface,
        runtime_verified: false,
        product_eligible: false,
    };
    if report.plain_bytes
        != G_MESS_TEXT_ASSET
            .layout
            .byte_len()
            .map_err(|source| error(source.to_string()))?
        || !report.cnx_roundtrip_exact
        || !report.entry_offset_unchanged
        || !report.non_target_entries_exact
        || report.final_readback_exact
        || report.translation.entries == 0
        || report.translation.entries != report.surface.replacement_blocks
        || report.surface.changed_pixels == 0
        || report.surface.changed_bytes == 0
        || report.runtime_verified
        || report.product_eligible
    {
        return Err(error("G_MESS 개발 archive 판정 drift"));
    }
    Ok(GMessDevelopmentArchiveBuild { archive, report })
}

/// 누적 산출물에서 `TUU_GRP:G_MESS.BTX` 전체를 다시 읽어 확인한다.
pub fn verify_g_mess_development_archive_readback(
    image: &[u8],
    build: &GMessDevelopmentArchiveBuild,
) -> Result<(), GMessDevelopmentArchiveError> {
    verify_btx_development_archive_readback(image, &build.archive)
        .map_err(|source| error(source.to_string()))
}
