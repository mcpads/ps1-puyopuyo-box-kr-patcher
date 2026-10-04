//! 같은 VDD의 여러 개발 CNX 엔트리를 한 번 합성하는 조정자.
//!
//! 각 엔트리는 자기 plain 합성과 CNX 왕복을 먼저 끝낸다. 이 모듈은 같은
//! 불변 원본에 결속된 서로 다른 엔트리만 받아 replacement map을 합성한다.
//! 기존 container에 들어가면 고정 크기로 두고, 넘으면 성장 VDD를 만들되
//! 실제 32M 배치는 공용 disc composer에 맡긴다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::archive::{VddArchive, VddError};
use crate::compress::decompress;
use crate::disc::{DiscImage, Iso9660};

use super::cnx_development_entry::PreparedCnxDevelopmentEntry;
use super::disc_archive_patch::{DiscArchiveRepack, DiscRelocatedFilePatch};

/// 복수 CNX 엔트리를 합성한 VDD의 최종 배치 방식.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CnxDevelopmentArchivePlacement {
    InPlace,
    RepackInContainer,
    RelocateToReservation,
}

/// 공용 disc composer에 전달할 한 VDD의 배타적 기여.
#[derive(Clone, Copy)]
pub(super) enum CnxDevelopmentDiscContribution<'a> {
    ArchiveRepack(DiscArchiveRepack<'a>),
    RelocatedFile(DiscRelocatedFilePatch<'a>),
}

/// 여러 CNX replacement를 담은 VDD 합성 판정.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CnxDevelopmentArchiveReplacementReport {
    pub archive_entries: usize,
    pub replacement_entries: usize,
    pub moved_entry_offsets: usize,
    pub placement: CnxDevelopmentArchivePlacement,
    pub original_vdd_bytes: usize,
    pub vdd_bytes: usize,
    pub target_entries_exact: bool,
    pub non_target_entries_exact: bool,
    pub final_readback_exact: bool,
}

/// 여러 CNX 엔트리를 불변 원본에서 한 번 합성한 VDD replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedCnxDevelopmentArchiveReplacement {
    iso_path: &'static str,
    id: &'static str,
    actor: &'static str,
    original_vdd: Vec<u8>,
    replacement_vdd: Vec<u8>,
    pub report: CnxDevelopmentArchiveReplacementReport,
}

impl PreparedCnxDevelopmentArchiveReplacement {
    pub fn disc_contribution(&self) -> CnxDevelopmentDiscContribution<'_> {
        match self.report.placement {
            CnxDevelopmentArchivePlacement::InPlace
            | CnxDevelopmentArchivePlacement::RepackInContainer => {
                CnxDevelopmentDiscContribution::ArchiveRepack(DiscArchiveRepack {
                    iso_path: self.iso_path,
                    original_vdd: &self.original_vdd,
                    replacement_vdd: &self.replacement_vdd,
                    id: self.id,
                    actor: self.actor,
                })
            }
            CnxDevelopmentArchivePlacement::RelocateToReservation => {
                CnxDevelopmentDiscContribution::RelocatedFile(DiscRelocatedFilePatch {
                    iso_path: self.iso_path,
                    original: &self.original_vdd,
                    replacement: &self.replacement_vdd,
                    id: self.id,
                    actor: self.actor,
                })
            }
        }
    }
}

/// 복수 CNX VDD 합성·readback 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CnxDevelopmentArchiveReplacementError(String);

impl fmt::Display for CnxDevelopmentArchiveReplacementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for CnxDevelopmentArchiveReplacementError {}

fn error(message: impl Into<String>) -> CnxDevelopmentArchiveReplacementError {
    CnxDevelopmentArchiveReplacementError(message.into())
}

fn verify_repacked_archive(
    original: &VddArchive,
    replacement_vdd: &[u8],
    entries: &[&PreparedCnxDevelopmentEntry],
    placement: CnxDevelopmentArchivePlacement,
) -> Result<CnxDevelopmentArchiveReplacementReport, CnxDevelopmentArchiveReplacementError> {
    let replacement =
        VddArchive::parse(replacement_vdd).map_err(|source| error(source.to_string()))?;
    if replacement.entry_count() != original.entry_count() {
        return Err(error("재패킹 VDD 엔트리 분모가 바뀜"));
    }
    let mut moved_entry_offsets = 0usize;
    let mut target_entries_exact = true;
    let mut non_target_entries_exact = true;
    for original_entry in original.entries() {
        let final_entry = replacement
            .find(original_entry.name())
            .ok_or_else(|| error(format!("{}: 재패킹 VDD 엔트리 누락", original_entry.name())))?;
        moved_entry_offsets +=
            usize::from(final_entry.sector_offset != original_entry.sector_offset);
        if let Some(entry) = entries.iter().find(|entry| {
            original_entry
                .name()
                .eq_ignore_ascii_case(entry.spec.entry_name)
        }) {
            target_entries_exact &= replacement.read_file(final_entry) == entry.replacement_cnx;
            let plain = decompress(replacement.read_file(final_entry))
                .map_err(|source| error(source.to_string()))?;
            target_entries_exact &= plain == entry.patched_plain;
        } else {
            non_target_entries_exact &=
                replacement.read_file(final_entry) == original.read_file(original_entry);
        }
    }
    let report = CnxDevelopmentArchiveReplacementReport {
        archive_entries: original.entry_count(),
        replacement_entries: entries.len(),
        moved_entry_offsets,
        placement,
        original_vdd_bytes: original.serialize_lossless().len(),
        vdd_bytes: replacement_vdd.len(),
        target_entries_exact,
        non_target_entries_exact,
        final_readback_exact: false,
    };
    if report.archive_entries == 0
        || report.replacement_entries == 0
        || report.vdd_bytes == 0
        || match report.placement {
            CnxDevelopmentArchivePlacement::InPlace
            | CnxDevelopmentArchivePlacement::RepackInContainer => {
                report.vdd_bytes != report.original_vdd_bytes
            }
            CnxDevelopmentArchivePlacement::RelocateToReservation => {
                report.vdd_bytes <= report.original_vdd_bytes
            }
        }
        || !report.target_entries_exact
        || !report.non_target_entries_exact
        || report.final_readback_exact
    {
        return Err(error("CNX 개발 VDD 재패킹 판정 drift"));
    }
    Ok(report)
}

fn prepare_cnx_development_archive_replacement_with_mode(
    iso_path: &'static str,
    id: &'static str,
    actor: &'static str,
    entries: &[&PreparedCnxDevelopmentEntry],
    prefer_in_place: bool,
) -> Result<PreparedCnxDevelopmentArchiveReplacement, CnxDevelopmentArchiveReplacementError> {
    let first = entries
        .first()
        .ok_or_else(|| error("CNX 개발 VDD 재패킹 입력이 비어 있음"))?;
    let mut names = BTreeSet::new();
    let mut replacements = BTreeMap::new();
    for entry in entries {
        if !entry.spec.iso_path.eq_ignore_ascii_case(iso_path) {
            return Err(error(format!(
                "CNX 개발 entry의 VDD 경로 불일치: expected={iso_path}, actual={}",
                entry.spec.iso_path
            )));
        }
        if entry.original_vdd != first.original_vdd {
            return Err(error(format!(
                "{iso_path}: CNX 개발 entry의 불변 원본 VDD 불일치"
            )));
        }
        if !names.insert(entry.spec.entry_name.to_ascii_uppercase()) {
            return Err(error(format!(
                "{iso_path}: CNX 개발 entry 중복: {}",
                entry.spec.entry_name
            )));
        }
        replacements.insert(
            entry.spec.entry_name.to_string(),
            entry.replacement_cnx.clone(),
        );
    }
    let original =
        VddArchive::parse(&first.original_vdd).map_err(|source| error(source.to_string()))?;
    let in_place = prefer_in_place
        .then(|| original.replace_in_place(&replacements))
        .transpose();
    let (replacement_vdd, placement) = match in_place {
        Ok(Some(replacement)) => (replacement, CnxDevelopmentArchivePlacement::InPlace),
        Ok(None) | Err(VddError::ReplacementDoesNotFit { .. }) => {
            match original.repack_in_container(&replacements) {
                Ok(replacement) => (
                    replacement,
                    CnxDevelopmentArchivePlacement::RepackInContainer,
                ),
                Err(VddError::RepackedArchiveDoesNotFit { .. }) => (
                    original
                        .rebuild(&replacements)
                        .map_err(|source| error(source.to_string()))?,
                    CnxDevelopmentArchivePlacement::RelocateToReservation,
                ),
                Err(source) => return Err(error(source.to_string())),
            }
        }
        Err(source) => return Err(error(source.to_string())),
    };
    let report = verify_repacked_archive(&original, &replacement_vdd, entries, placement)?;
    Ok(PreparedCnxDevelopmentArchiveReplacement {
        iso_path,
        id,
        actor,
        original_vdd: first.original_vdd.clone(),
        replacement_vdd,
        report,
    })
}

/// 여러 CNX replacement를 현재 크기에 맞는 VDD 하나로 합성한다.
pub(super) fn prepare_cnx_development_archive_replacement(
    iso_path: &'static str,
    id: &'static str,
    actor: &'static str,
    entries: &[&PreparedCnxDevelopmentEntry],
) -> Result<PreparedCnxDevelopmentArchiveReplacement, CnxDevelopmentArchiveReplacementError> {
    prepare_cnx_development_archive_replacement_with_mode(iso_path, id, actor, entries, true)
}

/// 고정 container가 필수인 archive의 여러 CNX를 한 번 재패킹한다.
pub(super) fn repack_cnx_development_entries(
    iso_path: &'static str,
    id: &'static str,
    actor: &'static str,
    entries: &[&PreparedCnxDevelopmentEntry],
) -> Result<PreparedCnxDevelopmentArchiveReplacement, CnxDevelopmentArchiveReplacementError> {
    let build =
        prepare_cnx_development_archive_replacement_with_mode(iso_path, id, actor, entries, false)?;
    if build.report.placement != CnxDevelopmentArchivePlacement::RepackInContainer {
        return Err(error(format!(
            "{iso_path}: 고정 container 재패킹 대신 {:?}가 선택됨",
            build.report.placement
        )));
    }
    Ok(build)
}

/// 누적 산출물에서 여러 CNX를 합친 VDD 전체를 다시 읽어 확인한다.
pub(super) fn verify_cnx_development_archive_replacement_readback(
    image: &[u8],
    build: &PreparedCnxDevelopmentArchiveReplacement,
) -> Result<(), CnxDevelopmentArchiveReplacementError> {
    let image =
        DiscImage::from_bytes(image.to_vec()).map_err(|source| error(source.to_string()))?;
    let iso = Iso9660::parse(&image).map_err(|source| error(source.to_string()))?;
    let final_vdd = iso
        .read_file(build.iso_path)
        .map_err(|source| error(source.to_string()))?;
    if final_vdd != build.replacement_vdd {
        return Err(error(format!(
            "누적 disc의 {} 재패킹 최종 readback 불일치",
            build.iso_path
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "cnx_development_archive_replacement_tests.rs"]
mod tests;
