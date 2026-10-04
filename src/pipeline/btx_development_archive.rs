//! 개발 BTX 한 엔트리의 CNX 왕복과 VDD 제자리 교체 공통 경계.
//!
//! 자산별 모듈은 plain BTX 합성과 판정만 소유한다. 이 모듈은 지원 원본에서
//! 지정 엔트리를 읽고 합성 결과를 원래 CNX subtype으로 다시 압축한다.
//! 한 엔트리의 제자리 교체는 기존 offset과 비대상 payload를 보존한다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::archive::VddArchive;
use crate::compress::decompress;
use crate::disc::{DiscImage, Iso9660, VerifiedSourceImage};

use super::cnx_development_entry::prepare_cnx_development_entry;
pub(super) use super::cnx_development_entry::{
    CnxDevelopmentEntrySpec as BtxDevelopmentArchiveSpec,
    CnxDevelopmentEntryStorageReport as BtxDevelopmentEntryStorageReport,
    PreparedCnxDevelopmentEntry as PreparedBtxDevelopmentEntry,
};
use super::disc_archive_patch::DiscArchiveEntryPatch;

/// 자산별 보고서가 노출할 공통 저장·보존 판정.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BtxDevelopmentArchiveStorageReport {
    pub original_cnx_bytes: usize,
    pub replacement_cnx_bytes: usize,
    pub plain_bytes: usize,
    pub vdd_bytes: usize,
    pub cnx_roundtrip_exact: bool,
    pub entry_offset_unchanged: bool,
    pub non_target_entries_exact: bool,
}

/// 공용 Expected Write composer에 전달할 검증 완료 archive 기여.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedBtxDevelopmentArchive {
    entry: PreparedBtxDevelopmentEntry,
    replacement_vdd: Vec<u8>,
    pub storage: BtxDevelopmentArchiveStorageReport,
}

impl PreparedBtxDevelopmentArchive {
    pub fn archive_patch(&self) -> DiscArchiveEntryPatch<'_> {
        DiscArchiveEntryPatch {
            iso_path: self.entry.spec.iso_path,
            entry_name: self.entry.spec.entry_name,
            original_vdd: &self.entry.original_vdd,
            replacement_vdd: &self.replacement_vdd,
            replacement_cnx_bytes: self.storage.replacement_cnx_bytes,
            id: self.entry.spec.patch_id,
            actor: self.entry.spec.actor,
        }
    }

    pub fn patched_plain(&self) -> &[u8] {
        self.entry.patched_plain()
    }

    pub fn entry(&self) -> &PreparedBtxDevelopmentEntry {
        &self.entry
    }
}

/// 원본·CNX·VDD·readback 경계 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BtxDevelopmentArchiveError(String);

impl fmt::Display for BtxDevelopmentArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for BtxDevelopmentArchiveError {}

fn error(message: impl Into<String>) -> BtxDevelopmentArchiveError {
    BtxDevelopmentArchiveError(message.into())
}

fn verify_replacement_archive(
    spec: BtxDevelopmentArchiveSpec,
    original: &VddArchive,
    replacement_vdd: &[u8],
    expected_cnx: &[u8],
    expected_plain: &[u8],
) -> Result<(bool, bool), BtxDevelopmentArchiveError> {
    let replacement =
        VddArchive::parse(replacement_vdd).map_err(|source| error(source.to_string()))?;
    if replacement.entry_count() != original.entry_count() {
        return Err(error(format!("{} 엔트리 분모가 바뀜", spec.iso_path)));
    }
    let mut entry_offset_unchanged = true;
    let mut non_target_entries_exact = true;
    for original_entry in original.entries() {
        let final_entry = replacement
            .find(original_entry.name())
            .ok_or_else(|| error(format!("{}: 교체 VDD 엔트리 누락", original_entry.name())))?;
        entry_offset_unchanged &= final_entry.sector_offset == original_entry.sector_offset;
        if original_entry.name().eq_ignore_ascii_case(spec.entry_name) {
            if replacement.read_file(final_entry) != expected_cnx {
                return Err(error(format!(
                    "{} 교체 CNX readback 불일치",
                    spec.entry_name
                )));
            }
            let plain = decompress(replacement.read_file(final_entry))
                .map_err(|source| error(source.to_string()))?;
            if plain != expected_plain {
                return Err(error(format!(
                    "{} 교체 plain readback 불일치",
                    spec.entry_name
                )));
            }
        } else {
            non_target_entries_exact &=
                replacement.read_file(final_entry) == original.read_file(original_entry);
        }
    }
    if !entry_offset_unchanged || !non_target_entries_exact {
        return Err(error(format!(
            "{} 제자리·비대상 보존 경계 불일치",
            spec.iso_path
        )));
    }
    Ok((entry_offset_unchanged, non_target_entries_exact))
}

/// 지원 원본의 한 BTX를 자산별 함수로 합성하고 재압축 CNX를 준비한다.
pub(super) fn prepare_btx_development_entry<R>(
    source: &VerifiedSourceImage,
    spec: BtxDevelopmentArchiveSpec,
    patch_plain: impl FnOnce(&[u8]) -> Result<(Vec<u8>, R), String>,
) -> Result<(PreparedBtxDevelopmentEntry, R), BtxDevelopmentArchiveError> {
    prepare_cnx_development_entry(source, spec, patch_plain)
        .map_err(|source| error(source.to_string()))
}

/// 지원 원본의 한 BTX를 자산별 함수로 합성하고 제자리 VDD 기여를 만든다.
pub(super) fn prepare_btx_development_archive<R>(
    source: &VerifiedSourceImage,
    spec: BtxDevelopmentArchiveSpec,
    patch_plain: impl FnOnce(&[u8]) -> Result<(Vec<u8>, R), String>,
) -> Result<(PreparedBtxDevelopmentArchive, R), BtxDevelopmentArchiveError> {
    let (entry, patch_report) = prepare_btx_development_entry(source, spec, patch_plain)?;
    let archive =
        VddArchive::parse(&entry.original_vdd).map_err(|source| error(source.to_string()))?;
    let replacement_vdd = archive
        .replace_in_place(&BTreeMap::from([(
            spec.entry_name.to_string(),
            entry.replacement_cnx.clone(),
        )]))
        .map_err(|source| error(source.to_string()))?;
    let (entry_offset_unchanged, non_target_entries_exact) = verify_replacement_archive(
        spec,
        &archive,
        &replacement_vdd,
        &entry.replacement_cnx,
        &entry.patched_plain,
    )?;
    let storage = BtxDevelopmentArchiveStorageReport {
        original_cnx_bytes: entry.storage.original_cnx_bytes,
        replacement_cnx_bytes: entry.storage.replacement_cnx_bytes,
        plain_bytes: entry.storage.plain_bytes,
        vdd_bytes: replacement_vdd.len(),
        cnx_roundtrip_exact: entry.storage.cnx_roundtrip_exact,
        entry_offset_unchanged,
        non_target_entries_exact,
    };
    if storage.original_cnx_bytes == 0
        || storage.replacement_cnx_bytes == 0
        || storage.plain_bytes == 0
        || storage.vdd_bytes != entry.original_vdd.len()
        || !storage.cnx_roundtrip_exact
        || !storage.entry_offset_unchanged
        || !storage.non_target_entries_exact
    {
        return Err(error(format!(
            "{} 개발 archive 저장 판정 drift",
            spec.entry_name
        )));
    }
    Ok((
        PreparedBtxDevelopmentArchive {
            entry,
            replacement_vdd,
            storage,
        },
        patch_report,
    ))
}

/// 누적 산출물에서 교체 VDD와 BTX plain 전체를 다시 읽어 확인한다.
pub(super) fn verify_btx_development_archive_readback(
    image: &[u8],
    build: &PreparedBtxDevelopmentArchive,
) -> Result<(), BtxDevelopmentArchiveError> {
    let image =
        DiscImage::from_bytes(image.to_vec()).map_err(|source| error(source.to_string()))?;
    let iso = Iso9660::parse(&image).map_err(|source| error(source.to_string()))?;
    let final_vdd = iso
        .read_file(build.entry.spec.iso_path)
        .map_err(|source| error(source.to_string()))?;
    if final_vdd != build.replacement_vdd {
        return Err(error(format!(
            "누적 disc의 {} 최종 readback 불일치",
            build.entry.spec.iso_path
        )));
    }
    let archive = VddArchive::parse(&final_vdd).map_err(|source| error(source.to_string()))?;
    let entry = archive.find(build.entry.spec.entry_name).ok_or_else(|| {
        error(format!(
            "누적 disc에 {}가 없음",
            build.entry.spec.entry_name
        ))
    })?;
    let plain = decompress(archive.read_file(entry)).map_err(|source| error(source.to_string()))?;
    if plain != build.entry.patched_plain {
        return Err(error(format!(
            "누적 disc의 {} plain readback 불일치",
            build.entry.spec.entry_name
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "btx_development_archive_tests.rs"]
mod tests;
