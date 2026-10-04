//! 서브게임 VDD의 일반 raster 엔트리를 VDD별 한 결과로 합성한다.
//!
//! 자산 합성·CNX 검증은 일반 raster batch가 소유한다. 이 모듈은 공통 VDD
//! 경로별 엔트리를 모아 공용 disc composer에 한 번씩 넘기고 최종 readback을
//! 다시 일반 raster 보고서에 귀속한다.

use crate::graphics::text_catalog::{
    TextAssetSpec, BATAN2_TEXT_ASSET, BOXOPT_TEXT_ASSET, KANBAN_TEXT_ASSET,
};
use std::error::Error;
use std::fmt;

use super::cnx_development_archive_replacement::{
    prepare_cnx_development_archive_replacement,
    verify_cnx_development_archive_replacement_readback, CnxDevelopmentDiscContribution,
    PreparedCnxDevelopmentArchiveReplacement,
};
use super::g_mess_development_archive::GMessDevelopmentArchiveBuild;
use super::general_raster_development_entries::PreparedGeneralRasterDevelopmentBatch;

const ACTOR: &str = "pipeline::subgame_raster_development_archives";
const ASSETS: [TextAssetSpec<'static>; 10] = [
    crate::graphics::official_menu_surface::OFFICIAL_MENU_ASSET,
    crate::graphics::official_name_commands::NAME_COMMAND_ASSET,
    crate::graphics::style_selection_surface::LV_MENU2_ASSET,
    crate::graphics::style_selection_surface::LV_MENU_ASSET,
    crate::graphics::special_skill_surface::SPECIAL_SKILL_ASSET,
    BATAN2_TEXT_ASSET,
    BOXOPT_TEXT_ASSET,
    KANBAN_TEXT_ASSET,
    crate::graphics::course_result_surface::COURSE_RESULT_ASSET,
    crate::graphics::course_map_surface::COURSE_MAP_ASSET,
];

#[derive(Debug, Clone, Copy)]
struct ArchiveSpec {
    iso_path: &'static str,
    patch_id: &'static str,
}

const ARCHIVES: [ArchiveSpec; 6] = [
    ArchiveSpec {
        iso_path: "/KOUSIKI/KOU_GRP.VDD;1",
        patch_id: "subgame-raster-development-kousiki",
    },
    ArchiveSpec {
        iso_path: "/GOCHA1/TUU_GRP.VDD;1",
        patch_id: "subgame-raster-development-gocha1",
    },
    ArchiveSpec {
        iso_path: "/GOCHA2/GOTY_GRP.VDD;1",
        patch_id: "subgame-raster-development-gocha2",
    },
    ArchiveSpec {
        iso_path: "/HORI2/HORI2GRP.VDD;1",
        patch_id: "subgame-raster-development-hori2",
    },
    ArchiveSpec {
        iso_path: "/HORI4/HORI4GRP.VDD;1",
        patch_id: "subgame-raster-development-hori4",
    },
    ArchiveSpec {
        iso_path: "/MINPUYO/MIN_GRP.VDD;1",
        patch_id: "subgame-raster-development-minpuyo",
    },
];

// 사본 수는 catalog가 소유한다. archive마다 별도 숫자를 복제하지 않는다.
fn general_entry_count(iso_path: &str) -> usize {
    ASSETS
        .iter()
        .filter(|asset| {
            asset
                .archive_paths
                .iter()
                .any(|path| path.eq_ignore_ascii_case(iso_path))
        })
        .count()
}

pub(super) struct PreparedSubgameRasterDevelopmentArchives {
    archives: Vec<PreparedCnxDevelopmentArchiveReplacement>,
}

impl PreparedSubgameRasterDevelopmentArchives {
    pub(super) fn disc_contributions(&self) -> Vec<CnxDevelopmentDiscContribution<'_>> {
        self.archives
            .iter()
            .map(PreparedCnxDevelopmentArchiveReplacement::disc_contribution)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SubgameRasterDevelopmentArchivesError(String);

impl fmt::Display for SubgameRasterDevelopmentArchivesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for SubgameRasterDevelopmentArchivesError {}

fn error(message: impl Into<String>) -> SubgameRasterDevelopmentArchivesError {
    SubgameRasterDevelopmentArchivesError(message.into())
}

pub(super) fn prepare_subgame_raster_development_archives(
    general_raster: &PreparedGeneralRasterDevelopmentBatch,
    g_mess: &GMessDevelopmentArchiveBuild,
    official_card: &super::cnx_development_entry::PreparedCnxDevelopmentEntry,
) -> Result<PreparedSubgameRasterDevelopmentArchives, SubgameRasterDevelopmentArchivesError> {
    let mut archives = Vec::with_capacity(ARCHIVES.len());
    for archive in ARCHIVES {
        let mut entries = general_raster
            .entry_refs(archive.iso_path)
            .map_err(|source| error(source.to_string()))?;
        if archive
            .iso_path
            .eq_ignore_ascii_case("/GOCHA1/TUU_GRP.VDD;1")
        {
            entries.push(g_mess.entry());
        }
        let is_official = archive.iso_path == "/KOUSIKI/KOU_GRP.VDD;1";
        if is_official {
            entries.push(official_card);
        }
        let expected_entries = general_entry_count(archive.iso_path)
            + crate::graphics::common_chain_surface::CHAIN_ASSETS
                .iter()
                .filter(|(asset, _)| asset.archive_paths.contains(&archive.iso_path))
                .count()
            + crate::graphics::small_battle_badge_surface::SMALL_BADGE_ASSETS
                .iter()
                .filter(|(asset, _)| asset.archive_paths.contains(&archive.iso_path))
                .count()
            + usize::from(is_official)
            + usize::from(
                archive
                    .iso_path
                    .eq_ignore_ascii_case("/GOCHA1/TUU_GRP.VDD;1"),
            );
        if entries.len() != expected_entries {
            return Err(error(format!(
                "{} 서브게임 raster 엔트리 분모 불일치: {}",
                archive.iso_path,
                entries.len()
            )));
        }
        archives.push(
            prepare_cnx_development_archive_replacement(
                archive.iso_path,
                archive.patch_id,
                ACTOR,
                &entries,
            )
            .map_err(|source| error(source.to_string()))?,
        );
    }
    if archives.len() != ARCHIVES.len() {
        return Err(error("서브게임 raster archive 준비 분모 drift"));
    }
    Ok(PreparedSubgameRasterDevelopmentArchives { archives })
}

pub(super) fn verify_subgame_raster_development_archives_readback(
    image: &[u8],
    build: &PreparedSubgameRasterDevelopmentArchives,
    general_raster: &mut PreparedGeneralRasterDevelopmentBatch,
) -> Result<(), SubgameRasterDevelopmentArchivesError> {
    if build.archives.len() != ARCHIVES.len() {
        return Err(error("서브게임 raster archive readback 분모 drift"));
    }
    for (archive, spec) in build.archives.iter().zip(ARCHIVES) {
        verify_cnx_development_archive_replacement_readback(image, archive)
            .map_err(|source| error(source.to_string()))?;
        general_raster
            .record_verified_archive_readback(spec.iso_path)
            .map_err(|source| error(source.to_string()))?;
    }
    let report = general_raster
        .report_for_assets(&ASSETS.map(|asset| (asset.name, asset.expected_decompressed_sha256)))
        .map_err(|source| error(source.to_string()))?;
    let expected_physical_assets = ARCHIVES
        .iter()
        .map(|archive| general_entry_count(archive.iso_path))
        .sum::<usize>();
    if report.logical_assets != ASSETS.len()
        || report.physical_assets != expected_physical_assets
        || report
            .assets
            .iter()
            .any(|asset| !asset.final_readback_exact)
    {
        return Err(error("서브게임 raster 최종 readback 판정 누락"));
    }
    Ok(())
}
