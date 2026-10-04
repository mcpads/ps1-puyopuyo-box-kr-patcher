//! 누적 disc build에 합성할 VDD archive 변경.
//!
//! 기능별 pipeline은 교체할 archive와 불변 원본을 선언하고, 이 모듈은 실제
//! ISO 파일을 다시 읽어 원본을 대조한 뒤 `WritePlan`에 VDD 변경을 등록한다.
//! 적용 뒤에는 같은 ISO 경로에서 replacement 전체를 다시 읽어 검증한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::archive::VddArchive;
use crate::disc::Iso9660;
use crate::patch::WritePlan;

use super::vdd_plan::{add_vdd_entry_replacement, add_vdd_repack, logical_file};

/// 한 VDD entry를 제자리에서 교체하는 disc patch 입력.
pub struct DiscArchiveEntryPatch<'a> {
    pub iso_path: &'static str,
    pub entry_name: &'static str,
    pub original_vdd: &'a [u8],
    pub replacement_vdd: &'a [u8],
    pub replacement_cnx_bytes: usize,
    pub id: &'static str,
    pub actor: &'static str,
}

/// 고정된 VDD container 전체를 다시 배치하는 disc patch 입력.
#[derive(Clone, Copy)]
pub(super) struct DiscArchiveRepack<'a> {
    pub iso_path: &'static str,
    pub original_vdd: &'a [u8],
    pub replacement_vdd: &'a [u8],
    pub id: &'static str,
    pub actor: &'static str,
}

/// 검증된 32M reservation으로 함께 옮길 성장 ISO 파일.
#[derive(Clone, Copy)]
pub(super) struct DiscRelocatedFilePatch<'a> {
    pub iso_path: &'static str,
    pub original: &'a [u8],
    pub replacement: &'a [u8],
    pub id: &'static str,
    pub actor: &'static str,
}

/// 누적 disc build가 한 불변 원본에 합성할 파일 단위 기여.
#[derive(Clone, Copy, Default)]
pub(super) struct DiscFileContributions<'a> {
    pub subgame_help: Option<&'a super::subgame_help_development::PreparedSubgameHelp>,
    pub classic_scene_spacing:
        Option<&'a super::classic_scene_spacing::PreparedClassicSceneSpacing>,
    pub archive_patches: &'a [DiscArchiveEntryPatch<'a>],
    pub archive_repacks: &'a [DiscArchiveRepack<'a>],
    pub relocated_files: &'a [DiscRelocatedFilePatch<'a>],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DiscArchivePatchError(String);

impl fmt::Display for DiscArchivePatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for DiscArchivePatchError {}

fn error(message: impl Into<String>) -> DiscArchivePatchError {
    DiscArchivePatchError(message.into())
}

fn verify_archive_change_paths(
    entry_patches: &[DiscArchiveEntryPatch<'_>],
    repacks: &[DiscArchiveRepack<'_>],
) -> Result<(), DiscArchivePatchError> {
    let mut changed_archives = BTreeSet::new();
    for patch in entry_patches {
        if !changed_archives.insert(patch.iso_path.to_ascii_uppercase()) {
            return Err(error(format!(
                "같은 VDD의 entry patch가 둘 이상임: {}",
                patch.iso_path
            )));
        }
    }
    for repack in repacks {
        if !changed_archives.insert(repack.iso_path.to_ascii_uppercase()) {
            return Err(error(format!(
                "같은 VDD에 독립 archive 변경이 둘 이상임: {}",
                repack.iso_path
            )));
        }
    }
    Ok(())
}

/// 특정 VDD에 변경이 없으면 원본, 하나면 그 replacement를 반환한다.
///
/// entry patch와 repack은 모두 VDD 전체 replacement를 소유하므로 같은 VDD의
/// 변경을 순서대로 적용하는 방식은 허용하지 않는다.
pub(super) fn expected_archive_after_changes<'a>(
    iso_path: &str,
    original: &'a [u8],
    entry_patches: &'a [DiscArchiveEntryPatch<'a>],
    repacks: &'a [DiscArchiveRepack<'a>],
) -> Result<&'a [u8], DiscArchivePatchError> {
    let mut expected = None;
    for patch in entry_patches
        .iter()
        .filter(|patch| patch.iso_path.eq_ignore_ascii_case(iso_path))
    {
        if expected.replace(patch.replacement_vdd).is_some() {
            return Err(error(format!(
                "같은 VDD의 entry patch가 둘 이상임: {iso_path}"
            )));
        }
    }
    for repack in repacks
        .iter()
        .filter(|repack| repack.iso_path.eq_ignore_ascii_case(iso_path))
    {
        if expected.replace(repack.replacement_vdd).is_some() {
            return Err(error(format!(
                "같은 VDD에 독립 archive 변경이 둘 이상임: {iso_path}"
            )));
        }
    }
    Ok(expected.unwrap_or(original))
}

/// 선언한 VDD entry 교체와 repack을 불변 원본 기준 Expected Write로 등록한다.
pub(super) fn add_disc_archive_patch_writes(
    iso: &Iso9660<'_>,
    plan: &mut WritePlan,
    entry_patches: &[DiscArchiveEntryPatch<'_>],
    repacks: &[DiscArchiveRepack<'_>],
) -> Result<(), DiscArchivePatchError> {
    verify_archive_change_paths(entry_patches, repacks)?;
    for patch in entry_patches {
        let entry = iso
            .find(patch.iso_path)
            .map_err(|source| error(source.to_string()))?;
        let file = logical_file(entry).map_err(error)?;
        let source_vdd = iso
            .read_file(patch.iso_path)
            .map_err(|source| error(source.to_string()))?;
        if source_vdd != patch.original_vdd {
            return Err(error(format!("{}: VDD entry patch 원본 불일치", patch.id)));
        }
        let archive =
            VddArchive::parse(patch.original_vdd).map_err(|source| error(source.to_string()))?;
        add_vdd_entry_replacement(
            plan,
            file,
            &archive,
            patch.original_vdd,
            patch.replacement_vdd,
            patch.entry_name,
            patch.replacement_cnx_bytes,
            patch.id,
            patch.actor,
        )
        .map_err(error)?;
    }
    for repack in repacks {
        let entry = iso
            .find(repack.iso_path)
            .map_err(|source| error(source.to_string()))?;
        let file = logical_file(entry).map_err(error)?;
        let source_vdd = iso
            .read_file(repack.iso_path)
            .map_err(|source| error(source.to_string()))?;
        if source_vdd != repack.original_vdd {
            return Err(error(format!("{}: VDD repack 원본 불일치", repack.id)));
        }
        add_vdd_repack(
            plan,
            file,
            repack.original_vdd,
            repack.replacement_vdd,
            repack.id,
            repack.actor,
        )
        .map_err(error)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "disc_archive_patch_tests.rs"]
mod tests;

/// 최종 ISO에서 선언한 VDD replacement 전체가 그대로 소비되는지 확인한다.
pub(super) fn verify_disc_archive_patch_readback(
    iso: &Iso9660<'_>,
    entry_patches: &[DiscArchiveEntryPatch<'_>],
    repacks: &[DiscArchiveRepack<'_>],
) -> Result<(), DiscArchivePatchError> {
    for patch in entry_patches {
        let final_vdd = iso
            .read_file(patch.iso_path)
            .map_err(|source| error(source.to_string()))?;
        if final_vdd != patch.replacement_vdd {
            return Err(error(format!(
                "{}: VDD entry patch 최종 readback 불일치",
                patch.id
            )));
        }
    }
    for repack in repacks {
        let final_vdd = iso
            .read_file(repack.iso_path)
            .map_err(|source| error(source.to_string()))?;
        if final_vdd != repack.replacement_vdd {
            return Err(error(format!(
                "{}: VDD repack 최종 readback 불일치",
                repack.id
            )));
        }
    }
    Ok(())
}
