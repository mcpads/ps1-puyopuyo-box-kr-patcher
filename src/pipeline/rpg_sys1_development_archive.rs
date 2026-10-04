//! QUEST level-up `RPG_SYS1.BTX` 개발 surface의 `RPG_GRP.VDD` 기여.
//!
//! plain BTX 합성은 graphics 모듈에 맡기고, 이 모듈은 공통 archive 경계를
//! 통해 제자리 기여와 누적 disc 최종 readback만 소유한다.

use std::error::Error;
use std::fmt;

use crate::disc::VerifiedSourceImage;
use crate::graphics::rpg_sys1_level_up_surface::{
    patch_rpg_sys1_level_up_development_surface, RpgSys1LevelUpSurfaceReport,
};
use crate::graphics::text_catalog::RPG_SYS1_TEXT_ASSET;

use super::btx_development_archive::{
    prepare_btx_development_archive, verify_btx_development_archive_readback,
    BtxDevelopmentArchiveSpec, PreparedBtxDevelopmentArchive,
};
use super::disc_archive_patch::DiscArchiveEntryPatch;

const RPG_GRP_PATH: &str = "/QUEST/RPG_GRP.VDD;1";
const RPG_SYS1_NAME: &str = "RPG_SYS1.BTX";
const RPG_SYS1_ARCHIVE_SPEC: BtxDevelopmentArchiveSpec = BtxDevelopmentArchiveSpec {
    iso_path: RPG_GRP_PATH,
    entry_name: RPG_SYS1_NAME,
    expected_cnx_subtype: *b"bi_",
    patch_id: "quest-rpg-sys1-level-up-development",
    actor: "pipeline::rpg_sys1_development_archive",
};

/// level-up surface를 담은 제자리 VDD 교체의 정적 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgSys1DevelopmentArchiveReport {
    pub original_cnx_bytes: usize,
    pub replacement_cnx_bytes: usize,
    pub plain_bytes: usize,
    pub vdd_bytes: usize,
    pub cnx_roundtrip_exact: bool,
    pub entry_offset_unchanged: bool,
    pub non_target_entries_exact: bool,
    pub final_readback_exact: bool,
    pub surface: RpgSys1LevelUpSurfaceReport,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// 누적 disc composer에 넘길 검증 완료 `RPG_GRP.VDD` 기여.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgSys1DevelopmentArchiveBuild {
    archive: PreparedBtxDevelopmentArchive,
    pub report: RpgSys1DevelopmentArchiveReport,
}

impl RpgSys1DevelopmentArchiveBuild {
    pub(super) fn entry(&self) -> &super::cnx_development_entry::PreparedCnxDevelopmentEntry {
        self.archive.entry()
    }

    pub fn archive_patch(&self) -> DiscArchiveEntryPatch<'_> {
        self.archive.archive_patch()
    }

    /// 무손실 palette-index 검토 preview가 소비할 patched BTX.
    pub fn patched_plain(&self) -> &[u8] {
        self.archive.patched_plain()
    }
}

/// level-up archive 입력·압축·readback 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgSys1DevelopmentArchiveError(String);

impl fmt::Display for RpgSys1DevelopmentArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for RpgSys1DevelopmentArchiveError {}

fn error(message: impl Into<String>) -> RpgSys1DevelopmentArchiveError {
    RpgSys1DevelopmentArchiveError(message.into())
}

/// 지원 원본에서 level-up 개발 surface의 제자리 VDD 기여를 준비한다.
pub fn prepare_rpg_sys1_development_archive(
    source: &VerifiedSourceImage,
) -> Result<RpgSys1DevelopmentArchiveBuild, RpgSys1DevelopmentArchiveError> {
    let (archive, surface) =
        prepare_btx_development_archive(source, RPG_SYS1_ARCHIVE_SPEC, |original_plain| {
            patch_rpg_sys1_level_up_development_surface(original_plain)
                .map_err(|source| source.to_string())
        })
        .map_err(|source| error(source.to_string()))?;
    let storage = archive.storage;
    let report = RpgSys1DevelopmentArchiveReport {
        original_cnx_bytes: storage.original_cnx_bytes,
        replacement_cnx_bytes: storage.replacement_cnx_bytes,
        plain_bytes: storage.plain_bytes,
        vdd_bytes: storage.vdd_bytes,
        cnx_roundtrip_exact: storage.cnx_roundtrip_exact,
        entry_offset_unchanged: storage.entry_offset_unchanged,
        non_target_entries_exact: storage.non_target_entries_exact,
        final_readback_exact: false,
        surface,
        runtime_verified: false,
        product_eligible: false,
    };
    if report.plain_bytes
        != RPG_SYS1_TEXT_ASSET
            .layout
            .byte_len()
            .map_err(|source| error(source.to_string()))?
        || !report.cnx_roundtrip_exact
        || !report.entry_offset_unchanged
        || !report.non_target_entries_exact
        || report.final_readback_exact
        || report.surface.replacement_blocks != 1
        || report.surface.translation_approved
        || report.surface.profile_approved
        || report.surface.runtime_verified
        || report.surface.product_eligible
        || report.runtime_verified
        || report.product_eligible
    {
        return Err(error("RPG_SYS1 개발 archive 판정 drift"));
    }
    Ok(RpgSys1DevelopmentArchiveBuild { archive, report })
}

/// 누적 산출물에서 `RPG_GRP:RPG_SYS1.BTX` 전체를 다시 읽어 확인한다.
pub fn verify_rpg_sys1_development_archive_readback(
    image: &[u8],
    build: &RpgSys1DevelopmentArchiveBuild,
) -> Result<(), RpgSys1DevelopmentArchiveError> {
    verify_btx_development_archive_readback(image, &build.archive)
        .map_err(|source| error(source.to_string()))
}
