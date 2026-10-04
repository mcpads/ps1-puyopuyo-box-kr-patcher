//! 첫 셸 `MMENU.BTX` 개발 surface의 `FIL_GRP.VDD` 기여.
//!
//! surface 합성, CNX 왕복, 제자리 VDD 교체와 비대상 엔트리 보존까지만
//! 소유한다. disc Expected Write 적용은 공용 누적 composer가 소유한다.

use std::error::Error;
use std::fmt;

use crate::disc::VerifiedSourceImage;
use crate::graphics::mmenu_surface::{patch_mmenu_development_surface, MmenuSurfaceReport};

use super::btx_development_archive::{
    prepare_btx_development_archive, prepare_btx_development_entry,
    verify_btx_development_archive_readback, BtxDevelopmentArchiveSpec,
    BtxDevelopmentEntryStorageReport, PreparedBtxDevelopmentArchive, PreparedBtxDevelopmentEntry,
};
use super::disc_archive_patch::DiscArchiveEntryPatch;

const FIL_GRP_PATH: &str = "/FIL_GRP.VDD;1";
const MMENU_NAME: &str = "MMENU.BTX";

/// 글자 index가 참조한 원본 EXE의 팔레트를 최종 이미지에서도 보존한다.
pub(super) fn verify_mmenu_palette(image: &[u8]) -> Result<(), String> {
    use crate::disc::{DiscImage, Iso9660};
    use sha2::{Digest, Sha256};
    let disc = DiscImage::from_bytes(image.to_vec()).map_err(|e| e.to_string())?;
    let iso = Iso9660::parse(&disc).map_err(|e| e.to_string())?;
    let exe = iso.read_file("/SLPS_031.14;1").map_err(|e| e.to_string())?;
    let palette = exe.get(0x1aff8..0x1b1f8).ok_or("MMENU 팔레트 범위 누락")?;
    if format!("{:x}", Sha256::digest(palette))
        != "a9d90c99c24d7659942d140fc0c8b7c11725b80c3b37dee4a08341478b789b31"
    {
        return Err("MMENU 최종 팔레트 불일치".into());
    }
    Ok(())
}
const MMENU_ARCHIVE_SPEC: BtxDevelopmentArchiveSpec = BtxDevelopmentArchiveSpec {
    iso_path: FIL_GRP_PATH,
    entry_name: MMENU_NAME,
    expected_cnx_subtype: *b"bit",
    patch_id: "shell-mmenu-development",
    actor: "pipeline::mmenu_development_archive",
};

/// 다른 `FIL_GRP.VDD` 엔트리와 합성할 MMENU replacement의 정적 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmenuDevelopmentEntryReport {
    pub original_cnx_bytes: usize,
    pub replacement_cnx_bytes: usize,
    pub plain_bytes: usize,
    pub cnx_roundtrip_exact: bool,
    pub final_readback_exact: bool,
    pub surface: MmenuSurfaceReport,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// 같은 VDD의 다른 개발 엔트리와 합성할 검증 완료 `MMENU.BTX`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmenuDevelopmentEntryBuild {
    entry: PreparedBtxDevelopmentEntry,
    pub report: MmenuDevelopmentEntryReport,
}

impl MmenuDevelopmentEntryBuild {
    pub(super) fn entry(&self) -> &PreparedBtxDevelopmentEntry {
        &self.entry
    }

    /// 무손실 palette-index 검토 preview가 소비할 patched BTX.
    pub fn patched_plain(&self) -> &[u8] {
        self.entry.patched_plain()
    }
}

/// 첫 메뉴 surface를 담은 독립 제자리 VDD 교체의 정적 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmenuDevelopmentArchiveReport {
    pub entry: MmenuDevelopmentEntryReport,
    pub vdd_bytes: usize,
    pub entry_offset_unchanged: bool,
    pub non_target_entries_exact: bool,
    pub final_readback_exact: bool,
}

/// 누적 disc composer에 넘길 검증 완료 `FIL_GRP.VDD` 기여.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmenuDevelopmentArchiveBuild {
    archive: PreparedBtxDevelopmentArchive,
    pub report: MmenuDevelopmentArchiveReport,
}

impl MmenuDevelopmentArchiveBuild {
    /// 공용 Expected Write composer가 소비할 제자리 archive entry 변경.
    pub fn archive_patch(&self) -> DiscArchiveEntryPatch<'_> {
        self.archive.archive_patch()
    }

    /// 무손실 palette-index 검토 preview가 소비할 patched BTX.
    pub fn patched_plain(&self) -> &[u8] {
        self.archive.patched_plain()
    }
}

/// 첫 메뉴 archive 입력·압축·readback 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmenuDevelopmentArchiveError(String);

impl fmt::Display for MmenuDevelopmentArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for MmenuDevelopmentArchiveError {}

fn error(message: impl Into<String>) -> MmenuDevelopmentArchiveError {
    MmenuDevelopmentArchiveError(message.into())
}

fn build_entry_report(
    storage: BtxDevelopmentEntryStorageReport,
    surface: MmenuSurfaceReport,
) -> Result<MmenuDevelopmentEntryReport, MmenuDevelopmentArchiveError> {
    let report = MmenuDevelopmentEntryReport {
        original_cnx_bytes: storage.original_cnx_bytes,
        replacement_cnx_bytes: storage.replacement_cnx_bytes,
        plain_bytes: storage.plain_bytes,
        cnx_roundtrip_exact: storage.cnx_roundtrip_exact,
        final_readback_exact: false,
        runtime_verified: false,
        product_eligible: false,
        surface,
    };
    if report.original_cnx_bytes == 0
        || report.replacement_cnx_bytes == 0
        || report.plain_bytes == 0
        || !report.cnx_roundtrip_exact
        || report.final_readback_exact
        || report.surface.translation_approved
        || report.surface.profile_approved
        || report.surface.runtime_verified
        || report.surface.product_eligible
        || report.runtime_verified
        || report.product_eligible
    {
        return Err(error("MMENU 개발 entry 판정 drift"));
    }
    Ok(report)
}

/// 지원 원본에서 첫 메뉴 개발 surface의 CNX replacement를 준비한다.
pub fn prepare_mmenu_development_entry(
    source: &VerifiedSourceImage,
) -> Result<MmenuDevelopmentEntryBuild, MmenuDevelopmentArchiveError> {
    let (entry, surface) =
        prepare_btx_development_entry(source, MMENU_ARCHIVE_SPEC, |original_plain| {
            patch_mmenu_development_surface(original_plain).map_err(|source| source.to_string())
        })
        .map_err(|source| error(source.to_string()))?;
    let report = build_entry_report(entry.storage, surface)?;
    Ok(MmenuDevelopmentEntryBuild { entry, report })
}

/// 지원 원본에서 첫 메뉴 개발 surface의 제자리 VDD 기여를 준비한다.
pub fn prepare_mmenu_development_archive(
    source: &VerifiedSourceImage,
) -> Result<MmenuDevelopmentArchiveBuild, MmenuDevelopmentArchiveError> {
    let (archive, surface) =
        prepare_btx_development_archive(source, MMENU_ARCHIVE_SPEC, |original_plain| {
            patch_mmenu_development_surface(original_plain).map_err(|source| source.to_string())
        })
        .map_err(|source| error(source.to_string()))?;
    let storage = archive.storage;
    let entry = build_entry_report(archive.entry().storage, surface)?;
    let report = MmenuDevelopmentArchiveReport {
        entry,
        vdd_bytes: storage.vdd_bytes,
        entry_offset_unchanged: storage.entry_offset_unchanged,
        non_target_entries_exact: storage.non_target_entries_exact,
        final_readback_exact: false,
    };
    if !report.entry_offset_unchanged
        || !report.non_target_entries_exact
        || report.final_readback_exact
    {
        return Err(error("MMENU 개발 archive 판정 drift"));
    }
    Ok(MmenuDevelopmentArchiveBuild { archive, report })
}

/// 누적 산출물에서 `FIL_GRP:MMENU.BTX` 전체를 다시 읽어 확인한다.
pub fn verify_mmenu_development_archive_readback(
    image: &[u8],
    build: &MmenuDevelopmentArchiveBuild,
) -> Result<(), MmenuDevelopmentArchiveError> {
    verify_btx_development_archive_readback(image, &build.archive)
        .map_err(|source| error(source.to_string()))
}
