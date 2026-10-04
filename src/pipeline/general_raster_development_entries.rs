//! 평탄한 배경으로 판정한 일반 raster BTX의 물리 엔트리 준비.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::btx::Layout;
use crate::disc::VerifiedSourceImage;
use crate::graphics::boxopt_development_surface::patch_boxopt_development_surface;
use crate::graphics::general_raster_development_surface::{
    patch_flat_background_general_raster_surface, patch_p2_end16_development_surface,
    FlatBackgroundGeneralRasterSurfaceSpec, GeneralRasterDevelopmentSurfaceReport,
};
use crate::graphics::mcrd01_development_surface::patch_mcrd01_development_surface;
use crate::graphics::rank_s_development_surface::patch_rank_s_development_surface;
use crate::graphics::text_catalog::{
    TextAssetSpec, BATAN2_TEXT_ASSET, BOXOPT_TEXT_ASSET, MCRD01_TEXT_ASSET, MENU01_TEXT_ASSET,
    MENU201_TEXT_ASSET, MENU203_TEXT_ASSET, MOPT_TEXT_ASSET, P1_OVER3_TEXT_ASSET,
    P2RULED1_TEXT_ASSET, P2_END10_TEXT_ASSET, P2_END15_TEXT_ASSET, P2_END16_TEXT_ASSET,
    P2_INFO1_TEXT_ASSET, P2_INFO3_TEXT_ASSET, P2_OP13_TEXT_ASSET, P2_OP1_TEXT_ASSET,
    RANK_S_TEXT_ASSET, TITLEA_TEXT_ASSET,
};

use super::btx_development_archive::{
    prepare_btx_development_entry, BtxDevelopmentArchiveSpec, BtxDevelopmentEntryStorageReport,
    PreparedBtxDevelopmentEntry,
};

const ACTOR: &str = "pipeline::general_raster_development_entries";

#[derive(Debug, Clone, Copy)]
struct ArchiveSpec {
    scope: &'static str,
    iso_path: &'static str,
    patch_id: &'static str,
}

/// Catalog가 선언한 물리 사본을 Expected Write owner에 연결하는 경로 registry.
///
/// 자산별 archive 목록을 여기서 다시 복제하지 않는다. 각 자산은 자기 catalog의
/// `archive_paths`를 사용하고, 이 표는 해당 경로의 scope와 진단 ID만 제공한다.
const GENERAL_RASTER_ARCHIVES: [ArchiveSpec; 10] = [
    ArchiveSpec {
        scope: "QUEST",
        iso_path: "/QUEST/RPG_GRP.VDD;1",
        patch_id: "general-raster-entry-quest",
    },
    ArchiveSpec {
        scope: "KOUSIKI",
        iso_path: "/KOUSIKI/KOU_GRP.VDD;1",
        patch_id: "general-raster-entry-kousiki",
    },
    ArchiveSpec {
        scope: "SHELL",
        iso_path: "/FIL_GRP.VDD;1",
        patch_id: "general-raster-entry-shell",
    },
    ArchiveSpec {
        scope: "GOCHA1",
        iso_path: "/GOCHA1/TUU_GRP.VDD;1",
        patch_id: "general-raster-entry-gocha1",
    },
    ArchiveSpec {
        scope: "GOCHA2",
        iso_path: "/GOCHA2/GOTY_GRP.VDD;1",
        patch_id: "general-raster-entry-gocha2",
    },
    ArchiveSpec {
        scope: "HORI2",
        iso_path: "/HORI2/HORI2GRP.VDD;1",
        patch_id: "general-raster-entry-hori2",
    },
    ArchiveSpec {
        scope: "HORI4",
        iso_path: "/HORI4/HORI4GRP.VDD;1",
        patch_id: "general-raster-entry-hori4",
    },
    ArchiveSpec {
        scope: "MINPUYO",
        iso_path: "/MINPUYO/MIN_GRP.VDD;1",
        patch_id: "general-raster-entry-minpuyo",
    },
    ArchiveSpec {
        scope: "PUYO1",
        iso_path: "/PUYO1/PP1_GRP.VDD;1",
        patch_id: "general-raster-entry-puyo1",
    },
    ArchiveSpec {
        scope: "PUYO2",
        iso_path: "/PUYO2/PP2_GRP.VDD;1",
        patch_id: "general-raster-entry-puyo2",
    },
];

#[derive(Debug, Clone, Copy)]
struct FlatBackgroundDevelopmentAsset {
    asset: TextAssetSpec<'static>,
    preview_stem: &'static str,
    expected_cnx_subtype: [u8; 3],
}

/// 사람이 작성한 번역·profile까지 준비된 whole-bbox 자산 묶음.
const FLAT_BACKGROUND_DEVELOPMENT_ASSETS: [FlatBackgroundDevelopmentAsset; 14] = [
    FlatBackgroundDevelopmentAsset {
        asset: P2RULED1_TEXT_ASSET,
        preview_stem: "p2-ruled1",
        expected_cnx_subtype: *b"bi_",
    },
    FlatBackgroundDevelopmentAsset {
        asset: MENU01_TEXT_ASSET,
        preview_stem: "classic-puyo-mode-menu",
        expected_cnx_subtype: *b"bi_",
    },
    FlatBackgroundDevelopmentAsset {
        asset: MOPT_TEXT_ASSET,
        preview_stem: "mopt",
        expected_cnx_subtype: *b"bi_",
    },
    FlatBackgroundDevelopmentAsset {
        asset: BATAN2_TEXT_ASSET,
        preview_stem: "batan2",
        expected_cnx_subtype: *b"bi_",
    },
    FlatBackgroundDevelopmentAsset {
        asset: TITLEA_TEXT_ASSET,
        preview_stem: "titlea",
        expected_cnx_subtype: *b"bit",
    },
    FlatBackgroundDevelopmentAsset {
        asset: MENU201_TEXT_ASSET,
        preview_stem: "menu201",
        expected_cnx_subtype: *b"bi_",
    },
    FlatBackgroundDevelopmentAsset {
        asset: MENU203_TEXT_ASSET,
        preview_stem: "menu203",
        expected_cnx_subtype: *b"bi_",
    },
    FlatBackgroundDevelopmentAsset {
        asset: P1_OVER3_TEXT_ASSET,
        preview_stem: "p1-over3",
        expected_cnx_subtype: *b"bi_",
    },
    FlatBackgroundDevelopmentAsset {
        asset: P2_END10_TEXT_ASSET,
        preview_stem: "p2-end10",
        expected_cnx_subtype: *b"bi_",
    },
    FlatBackgroundDevelopmentAsset {
        asset: P2_END15_TEXT_ASSET,
        preview_stem: "p2-end15",
        expected_cnx_subtype: *b"bi_",
    },
    FlatBackgroundDevelopmentAsset {
        asset: P2_INFO1_TEXT_ASSET,
        preview_stem: "p2-info1",
        expected_cnx_subtype: *b"bi_",
    },
    FlatBackgroundDevelopmentAsset {
        asset: P2_INFO3_TEXT_ASSET,
        preview_stem: "p2-info3",
        expected_cnx_subtype: *b"bi_",
    },
    FlatBackgroundDevelopmentAsset {
        asset: P2_OP1_TEXT_ASSET,
        preview_stem: "p2-op1",
        expected_cnx_subtype: *b"bi_",
    },
    FlatBackgroundDevelopmentAsset {
        asset: P2_OP13_TEXT_ASSET,
        preview_stem: "p2-op13",
        expected_cnx_subtype: *b"bi_",
    },
];

/// 일반 raster 한 물리 사본의 합성·저장 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralRasterDevelopmentAssetReport {
    pub scope: &'static str,
    pub archive: &'static str,
    pub asset: &'static str,
    pub replacement_blocks: usize,
    pub changed_pixels: usize,
    pub changed_plain_bytes: usize,
    pub original_cnx_bytes: usize,
    pub replacement_cnx_bytes: usize,
    pub output_plain_sha256: String,
    pub translation_approved: bool,
    pub profile_approved: bool,
    pub final_readback_exact: bool,
}

/// 현재 연결한 일반 raster 논리·물리 사본의 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralRasterDevelopmentEntriesReport {
    pub logical_assets: usize,
    pub physical_assets: usize,
    pub assets: Vec<GeneralRasterDevelopmentAssetReport>,
    pub physical_copies_exact: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// 일반 raster batch에서 출력기로 넘기는 논리 surface preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralRasterDevelopmentSurface {
    pub asset: &'static str,
    pub preview_stem: &'static str,
    pub layout: Layout,
    pub bytes: Vec<u8>,
}

pub(super) struct PreparedGeneralRasterDevelopmentEntries {
    asset: TextAssetSpec<'static>,
    preview_stem: &'static str,
    archives: Vec<PreparedGeneralRasterArchiveEntry>,
    pub surface: Vec<u8>,
    pub report: GeneralRasterDevelopmentEntriesReport,
}

/// 여러 논리 surface를 물리 VDD별로 꺼내 쓰는 현재 개발 batch.
///
/// 개별 자산은 자기 원본·합성·CNX 왕복을 계속 소유하고, 이 구조는 같은
/// VDD에 들어갈 엔트리와 readback만 한꺼번에 조정한다.
pub(super) struct PreparedGeneralRasterDevelopmentBatch {
    assets: Vec<PreparedGeneralRasterDevelopmentEntries>,
}

struct PreparedGeneralRasterArchiveEntry {
    archive: ArchiveSpec,
    entry: PreparedBtxDevelopmentEntry,
}

impl PreparedGeneralRasterDevelopmentEntries {
    pub(super) fn record_verified_archive_readback(
        &mut self,
        iso_path: &str,
    ) -> Result<(), GeneralRasterDevelopmentEntriesError> {
        let prepared = self
            .archives
            .iter()
            .filter(|archive| archive.archive.iso_path.eq_ignore_ascii_case(iso_path))
            .count();
        let matching_reports = self
            .report
            .assets
            .iter_mut()
            .filter(|report| report.archive.eq_ignore_ascii_case(iso_path))
            .collect::<Vec<_>>();
        if prepared != 1 || matching_reports.len() != 1 {
            return Err(error(format!(
                "일반 raster 검증 archive가 준비 집합과 일치하지 않음: {iso_path}"
            )));
        }
        let report = matching_reports
            .into_iter()
            .next()
            .ok_or_else(|| error(format!("일반 raster 검증 보고서 누락: {iso_path}")))?;
        if report.final_readback_exact {
            return Err(error(format!(
                "일반 raster archive readback 중복 기록: {iso_path}"
            )));
        }
        report.final_readback_exact = true;
        Ok(())
    }
}

impl PreparedGeneralRasterDevelopmentBatch {
    pub(super) fn entry_refs(
        &self,
        iso_path: &str,
    ) -> Result<Vec<&PreparedBtxDevelopmentEntry>, GeneralRasterDevelopmentEntriesError> {
        let entries = self
            .assets
            .iter()
            .flat_map(|asset| &asset.archives)
            .filter(|archive| archive.archive.iso_path.eq_ignore_ascii_case(iso_path))
            .map(|archive| &archive.entry)
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return Err(error(format!(
                "일반 raster batch archive 입력 누락: {iso_path}"
            )));
        }
        let mut names = BTreeSet::new();
        if entries
            .iter()
            .any(|entry| !names.insert(entry.spec.entry_name.to_ascii_uppercase()))
        {
            return Err(error(format!(
                "일반 raster batch archive 엔트리 중복: {iso_path}"
            )));
        }
        Ok(entries)
    }

    pub(super) fn record_verified_archive_readback(
        &mut self,
        iso_path: &str,
    ) -> Result<(), GeneralRasterDevelopmentEntriesError> {
        let mut matched = 0usize;
        for asset in &mut self.assets {
            if asset
                .archives
                .iter()
                .any(|archive| archive.archive.iso_path.eq_ignore_ascii_case(iso_path))
            {
                asset.record_verified_archive_readback(iso_path)?;
                matched += 1;
            }
        }
        if matched == 0 {
            return Err(error(format!(
                "일반 raster batch readback 대상 누락: {iso_path}"
            )));
        }
        Ok(())
    }

    pub(super) fn report(
        &self,
    ) -> Result<GeneralRasterDevelopmentEntriesReport, GeneralRasterDevelopmentEntriesError> {
        merge_general_raster_development_reports(
            &self
                .assets
                .iter()
                .map(|asset| &asset.report)
                .collect::<Vec<_>>(),
        )
    }

    /// 동명·서로 다른 digest의 atlas를 archive별 실제 기여로 구분한다.
    pub(super) fn report_for_archives(
        &self,
        paths: &[&str],
    ) -> Result<GeneralRasterDevelopmentEntriesReport, GeneralRasterDevelopmentEntriesError> {
        let reports = self
            .assets
            .iter()
            .filter_map(|entry| {
                let mut report = entry.report.clone();
                report.assets.retain(|asset| paths.contains(&asset.archive));
                report.physical_assets = report.assets.len();
                (!report.assets.is_empty()).then_some(report)
            })
            .collect::<Vec<_>>();
        merge_general_raster_development_reports(&reports.iter().collect::<Vec<_>>())
    }

    pub(super) fn report_for_assets(
        &self,
        asset_names: &[(&str, &str)],
    ) -> Result<GeneralRasterDevelopmentEntriesReport, GeneralRasterDevelopmentEntriesError> {
        let expected = asset_names
            .iter()
            .map(|(name, digest)| (name.to_ascii_uppercase(), *digest))
            .collect::<BTreeSet<_>>();
        if expected.len() != asset_names.len() {
            return Err(error("일반 raster batch 보고서 요청 자산 중복"));
        }
        let reports = self
            .assets
            .iter()
            .filter(|asset| {
                expected.contains(&(
                    asset.asset.name.to_ascii_uppercase(),
                    asset.asset.expected_decompressed_sha256,
                ))
            })
            .map(|asset| &asset.report)
            .collect::<Vec<_>>();
        if reports.len() != expected.len() {
            return Err(error("일반 raster batch 보고서 요청 자산 누락"));
        }
        merge_general_raster_development_reports(&reports)
    }

    pub(super) fn into_surfaces(
        self,
    ) -> Result<Vec<GeneralRasterDevelopmentSurface>, GeneralRasterDevelopmentEntriesError> {
        let mut asset_names = BTreeSet::new();
        let mut preview_stems = BTreeSet::new();
        let mut surfaces = Vec::with_capacity(self.assets.len());
        for asset in self.assets {
            if !asset_names.insert((
                asset.asset.name.to_ascii_uppercase(),
                asset.asset.expected_decompressed_sha256,
            )) {
                return Err(error(format!(
                    "일반 raster batch 논리 자산 중복: {}",
                    asset.asset.name
                )));
            }
            if asset.preview_stem.is_empty()
                || !preview_stems.insert(asset.preview_stem.to_ascii_lowercase())
            {
                return Err(error(format!(
                    "일반 raster batch preview stem 누락 또는 중복: {}",
                    asset.preview_stem
                )));
            }
            if asset.surface.len()
                != asset
                    .asset
                    .layout
                    .byte_len()
                    .map_err(|source| error(source.to_string()))?
            {
                return Err(error(format!(
                    "일반 raster batch preview 크기 불일치: {}",
                    asset.asset.name
                )));
            }
            surfaces.push(GeneralRasterDevelopmentSurface {
                asset: asset.asset.name,
                preview_stem: asset.preview_stem,
                layout: asset.asset.layout,
                bytes: asset.surface,
            });
        }
        Ok(surfaces)
    }
}

/// 일반 raster 원본·합성·CNX 준비 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralRasterDevelopmentEntriesError(String);

impl fmt::Display for GeneralRasterDevelopmentEntriesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for GeneralRasterDevelopmentEntriesError {}

fn error(message: impl Into<String>) -> GeneralRasterDevelopmentEntriesError {
    GeneralRasterDevelopmentEntriesError(message.into())
}

fn asset_report(
    archive: ArchiveSpec,
    storage: BtxDevelopmentEntryStorageReport,
    surface: GeneralRasterDevelopmentSurfaceReport,
) -> GeneralRasterDevelopmentAssetReport {
    GeneralRasterDevelopmentAssetReport {
        scope: archive.scope,
        archive: archive.iso_path,
        asset: surface.asset,
        replacement_blocks: surface.replacement_blocks,
        changed_pixels: surface.changed_pixels,
        changed_plain_bytes: surface.changed_bytes,
        original_cnx_bytes: storage.original_cnx_bytes,
        replacement_cnx_bytes: storage.replacement_cnx_bytes,
        output_plain_sha256: surface.output_sha256,
        translation_approved: surface.translation_approved,
        profile_approved: surface.profile_approved,
        final_readback_exact: false,
    }
}

fn archive_spec_for_path(
    iso_path: &str,
) -> Result<ArchiveSpec, GeneralRasterDevelopmentEntriesError> {
    let matches = GENERAL_RASTER_ARCHIVES
        .iter()
        .copied()
        .filter(|archive| archive.iso_path.eq_ignore_ascii_case(iso_path))
        .collect::<Vec<_>>();
    let [archive] = matches.as_slice() else {
        return Err(error(format!(
            "일반 raster archive 경로 registry 결속 수 불일치: {iso_path}"
        )));
    };
    Ok(*archive)
}

fn archive_specs_for_asset(
    asset: TextAssetSpec<'static>,
) -> Result<Vec<ArchiveSpec>, GeneralRasterDevelopmentEntriesError> {
    let mut archive_specs = Vec::with_capacity(asset.archive_paths.len());
    for &iso_path in asset.archive_paths {
        archive_specs.push(archive_spec_for_path(iso_path).map_err(|source| {
            error(format!("{}의 archive 경로 결속 실패: {source}", asset.name))
        })?);
    }
    let unique_paths = archive_specs
        .iter()
        .map(|archive| archive.iso_path.to_ascii_uppercase())
        .collect::<BTreeSet<_>>();
    if unique_paths.len() != archive_specs.len() {
        return Err(error(format!(
            "{}의 catalog archive 경로가 중복됨",
            asset.name
        )));
    }
    Ok(archive_specs)
}

fn prepare_general_raster_development_entries<E: ToString>(
    source: &VerifiedSourceImage,
    asset: TextAssetSpec<'static>,
    preview_stem: &'static str,
    expected_cnx_subtype: [u8; 3],
    patch_surface: impl Fn(&[u8]) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), E>,
) -> Result<PreparedGeneralRasterDevelopmentEntries, GeneralRasterDevelopmentEntriesError> {
    let archive_specs = archive_specs_for_asset(asset)?;
    let declared_archives = asset
        .archive_paths
        .iter()
        .map(|path| path.to_ascii_uppercase())
        .collect::<BTreeSet<_>>();
    let prepared_archives = archive_specs
        .iter()
        .map(|archive| archive.iso_path.to_ascii_uppercase())
        .collect::<BTreeSet<_>>();
    if declared_archives != prepared_archives || prepared_archives.len() != archive_specs.len() {
        return Err(error(format!(
            "{} catalog와 물리 archive 준비 집합 불일치",
            asset.name
        )));
    }
    let mut archives = Vec::new();
    let mut reports = Vec::new();
    let mut canonical_surface = None;
    let mut physical_copies_exact = true;
    for &archive in &archive_specs {
        let spec = BtxDevelopmentArchiveSpec {
            iso_path: archive.iso_path,
            entry_name: asset.name,
            expected_cnx_subtype,
            patch_id: archive.patch_id,
            actor: ACTOR,
        };
        let (entry, surface) = prepare_btx_development_entry(source, spec, |plain| {
            patch_surface(plain).map_err(|source| source.to_string())
        })
        .map_err(|source| error(source.to_string()))?;
        if let Some(previous) = canonical_surface.as_ref() {
            physical_copies_exact &= previous == entry.patched_plain();
        } else {
            canonical_surface = Some(entry.patched_plain().to_vec());
        }
        reports.push(asset_report(archive, entry.storage, surface));
        archives.push(PreparedGeneralRasterArchiveEntry { archive, entry });
    }
    let report = GeneralRasterDevelopmentEntriesReport {
        logical_assets: usize::from(canonical_surface.is_some()),
        physical_assets: reports.len(),
        assets: reports,
        physical_copies_exact,
        runtime_verified: false,
        product_eligible: false,
    };
    if report.logical_assets == 0
        || report.physical_assets == 0
        || !report.physical_copies_exact
        || report.assets.iter().any(|asset| {
            asset.replacement_blocks == 0
                || asset.changed_pixels == 0
                || asset.changed_plain_bytes == 0
                || asset.final_readback_exact
        })
        || report.runtime_verified
        || report.product_eligible
    {
        return Err(error("일반 raster 개발 entry 판정 drift"));
    }
    Ok(PreparedGeneralRasterDevelopmentEntries {
        asset,
        preview_stem,
        archives,
        surface: canonical_surface.ok_or_else(|| error("일반 raster preview 원천 누락"))?,
        report,
    })
}

fn prepare_flat_background_general_raster_development_entries(
    source: &VerifiedSourceImage,
    development_asset: FlatBackgroundDevelopmentAsset,
) -> Result<PreparedGeneralRasterDevelopmentEntries, GeneralRasterDevelopmentEntriesError> {
    let surface_spec = FlatBackgroundGeneralRasterSurfaceSpec {
        asset: development_asset.asset,
    };
    prepare_general_raster_development_entries(
        source,
        surface_spec.asset,
        development_asset.preview_stem,
        development_asset.expected_cnx_subtype,
        |plain| patch_flat_background_general_raster_surface(plain, surface_spec),
    )
}

/// 지원 원본의 P2 ending banner 두 사본을 같은 보호형 surface에서 준비한다.
fn prepare_p2_end16_development_entries(
    source: &VerifiedSourceImage,
) -> Result<PreparedGeneralRasterDevelopmentEntries, GeneralRasterDevelopmentEntriesError> {
    prepare_general_raster_development_entries(
        source,
        P2_END16_TEXT_ASSET,
        "p2-ending-banner",
        *b"bi_",
        patch_p2_end16_development_surface,
    )
}

/// 지원 원본의 셸 ranking label atlas 한 사본을 보호형 surface로 준비한다.
fn prepare_rank_s_development_entries(
    source: &VerifiedSourceImage,
) -> Result<PreparedGeneralRasterDevelopmentEntries, GeneralRasterDevelopmentEntriesError> {
    prepare_general_raster_development_entries(
        source,
        RANK_S_TEXT_ASSET,
        "rank-s",
        *b"bi_",
        patch_rank_s_development_surface,
    )
}

/// 지원 원본의 memory-card button label 한 사본을 보호형 surface로 준비한다.
fn prepare_mcrd01_development_entries(
    source: &VerifiedSourceImage,
) -> Result<PreparedGeneralRasterDevelopmentEntries, GeneralRasterDevelopmentEntriesError> {
    prepare_general_raster_development_entries(
        source,
        MCRD01_TEXT_ASSET,
        "mcrd01",
        *b"bi_",
        patch_mcrd01_development_surface,
    )
}

/// 다섯 서브게임의 공용 sparse selector atlas를 같은 논리 surface로 준비한다.
fn prepare_boxopt_development_entries(
    source: &VerifiedSourceImage,
) -> Result<PreparedGeneralRasterDevelopmentEntries, GeneralRasterDevelopmentEntriesError> {
    prepare_general_raster_development_entries(
        source,
        BOXOPT_TEXT_ASSET,
        "boxopt",
        *b"bi_",
        patch_boxopt_development_surface,
    )
}

/// 현재 연결된 일반 raster를 한 번 준비해 VDD별 합성 입력으로 제공한다.
pub(super) fn prepare_current_general_raster_development_batch(
    source: &VerifiedSourceImage,
) -> Result<PreparedGeneralRasterDevelopmentBatch, GeneralRasterDevelopmentEntriesError> {
    let mut assets = FLAT_BACKGROUND_DEVELOPMENT_ASSETS
        .iter()
        .copied()
        .map(|asset| prepare_flat_background_general_raster_development_entries(source, asset))
        .collect::<Result<Vec<_>, _>>()?;
    for (asset, stem) in [
        (
            crate::graphics::style_selection_surface::LV_MENU2_ASSET,
            "lv-menu2",
        ),
        (
            crate::graphics::style_selection_surface::LV_MENU_ASSET,
            "lv-menu",
        ),
    ] {
        assets.push(prepare_general_raster_development_entries(
            source,
            asset,
            stem,
            *b"bi_",
            |plain| {
                crate::graphics::style_selection_surface::patch_style_selection_surface(
                    plain, asset,
                )
            },
        )?);
    }
    assets.extend([
        prepare_general_raster_development_entries(
            source,
            crate::graphics::official_name_commands::NAME_COMMAND_ASSET,
            "official-name-commands",
            *b"bi_",
            crate::graphics::official_name_commands::patch_name_commands,
        )?,
        prepare_general_raster_development_entries(
            source,
            crate::graphics::official_menu_surface::OFFICIAL_MENU_ASSET,
            "official-menu",
            *b"bi_",
            crate::graphics::official_menu_surface::patch_official_menu_surface,
        )?,
        prepare_general_raster_development_entries(
            source,
            crate::graphics::special_skill_surface::SPECIAL_SKILL_ASSET,
            "special-skills",
            *b"bi_",
            crate::graphics::special_skill_surface::patch_special_skill_surface,
        )?,
        prepare_general_raster_development_entries(
            source,
            crate::graphics::course_map_surface::COURSE_MAP_ASSET,
            "course-map",
            *b"bi_",
            crate::graphics::course_map_surface::patch_course_map_surface,
        )?,
        prepare_general_raster_development_entries(
            source,
            crate::graphics::course_result_surface::COURSE_RESULT_ASSET,
            "course-result",
            *b"bi_",
            crate::graphics::course_result_surface::patch_course_result_surface,
        )?,
        prepare_p2_end16_development_entries(source)?,
        prepare_rank_s_development_entries(source)?,
        prepare_mcrd01_development_entries(source)?,
        prepare_boxopt_development_entries(source)?,
        prepare_general_raster_development_entries(
            source,
            crate::graphics::text_catalog::MENU202_TEXT_ASSET,
            "menu202",
            *b"bi_",
            crate::graphics::generated_menu_surface::patch_menu202_surface,
        )?,
        prepare_general_raster_development_entries(
            source,
            crate::graphics::text_catalog::KANBAN_TEXT_ASSET,
            "kanban",
            *b"bit",
            crate::graphics::generated_menu_surface::patch_kanban_surface,
        )?,
    ]);
    for (asset, stem) in [
        (
            crate::graphics::classic_graphics_surface::CONTINUE_ASSET,
            "classic-continue",
        ),
        (
            crate::graphics::classic_graphics_surface::RANKING_ASSET,
            "classic-ranking-captions",
        ),
        (
            crate::graphics::classic_graphics_surface::DIFFICULTY_ASSET,
            "classic-difficulty",
        ),
        (
            crate::graphics::classic_graphics_surface::OPPONENT_ASSET,
            "classic-opponent-prompt",
        ),
        (
            crate::graphics::classic_graphics_surface::OPTION_ASSET,
            "classic-option-labels",
        ),
    ] {
        assets.push(prepare_general_raster_development_entries(
            source,
            asset,
            stem,
            *b"bi_",
            |plain| crate::graphics::classic_graphics_surface::patch_labels(plain, asset),
        )?);
    }
    assets.push(prepare_general_raster_development_entries(
        source,
        crate::graphics::classic_graphics_surface::LOGO_ASSET,
        "classic-puyo1-logo",
        *b"bi_",
        crate::graphics::classic_graphics_surface::patch_logo,
    )?);
    crate::graphics::small_battle_badge_surface::verify_consumers(source.image()).map_err(error)?;
    let translated_badges = assets
        .iter()
        .find(|entry| entry.asset == BATAN2_TEXT_ASSET)
        .ok_or_else(|| error("작은 배지의 채택 번역 surface 누락"))?
        .surface
        .clone();
    for (asset, stem) in crate::graphics::classic_graphics_surface::BADGE_ASSETS {
        assets.push(prepare_general_raster_development_entries(
            source,
            asset,
            stem,
            *b"bi_",
            |plain| {
                crate::graphics::classic_graphics_surface::patch_badges(
                    plain,
                    asset,
                    &translated_badges,
                )
            },
        )?);
    }
    for (asset, stem) in crate::graphics::small_battle_badge_surface::SMALL_BADGE_ASSETS {
        assets.push(prepare_general_raster_development_entries(
            source,
            asset,
            stem,
            *b"bi_",
            |plain| {
                crate::graphics::small_battle_badge_surface::patch_small_badges(
                    plain,
                    asset,
                    &translated_badges,
                )
            },
        )?);
    }
    for (asset, stem) in crate::graphics::common_chain_surface::CHAIN_ASSETS {
        assets.push(prepare_general_raster_development_entries(
            source,
            asset,
            stem,
            *b"bi_",
            |plain| crate::graphics::common_chain_surface::patch_chain_surface(plain, asset),
        )?);
    }
    let expected_logical_assets = assets.len();
    let expected_physical_assets = assets
        .iter()
        .map(|asset| asset.report.physical_assets)
        .sum::<usize>();
    let batch = PreparedGeneralRasterDevelopmentBatch { assets };
    let report = batch.report()?;
    if report.logical_assets != expected_logical_assets
        || report.physical_assets != expected_physical_assets
    {
        return Err(error("현재 일반 raster batch 분모 drift"));
    }
    Ok(batch)
}

/// 서로 다른 논리 자산 보고서를 현재 일반 raster 전체 판정으로 합친다.
pub(super) fn merge_general_raster_development_reports(
    reports: &[&GeneralRasterDevelopmentEntriesReport],
) -> Result<GeneralRasterDevelopmentEntriesReport, GeneralRasterDevelopmentEntriesError> {
    if reports.is_empty() {
        return Err(error("일반 raster 집계 입력이 비어 있음"));
    }
    let mut assets = Vec::new();
    let mut physical_keys = BTreeSet::new();
    for report in reports {
        for asset in &report.assets {
            if !physical_keys.insert((asset.archive, asset.asset)) {
                return Err(error(format!(
                    "일반 raster 물리 자산 중복: {}:{}",
                    asset.archive, asset.asset
                )));
            }
            assets.push(asset.clone());
        }
    }
    let report = GeneralRasterDevelopmentEntriesReport {
        logical_assets: reports.iter().map(|report| report.logical_assets).sum(),
        physical_assets: reports.iter().map(|report| report.physical_assets).sum(),
        assets,
        physical_copies_exact: reports.iter().all(|report| report.physical_copies_exact),
        runtime_verified: reports.iter().all(|report| report.runtime_verified),
        product_eligible: reports.iter().all(|report| report.product_eligible),
    };
    if report.logical_assets == 0
        || report.physical_assets != report.assets.len()
        || !report.physical_copies_exact
    {
        return Err(error("일반 raster 집계 판정 drift"));
    }
    Ok(report)
}

/// 일반 raster 물리 사본 보고서를 행별 TSV로 만든다.
pub fn render_general_raster_development_entries_tsv(
    report: &GeneralRasterDevelopmentEntriesReport,
) -> String {
    let header = [
        "scope",
        "archive",
        "asset",
        "replacement_blocks",
        "changed_pixels",
        "changed_plain_bytes",
        "original_cnx_bytes",
        "replacement_cnx_bytes",
        "output_plain_sha256",
        "translation_approved",
        "profile_approved",
        "final_readback_exact",
        "runtime_verified",
        "product_eligible",
    ]
    .join("\t");
    let rows = report
        .assets
        .iter()
        .map(|asset| {
            [
                asset.scope.to_string(),
                asset.archive.to_string(),
                asset.asset.to_string(),
                asset.replacement_blocks.to_string(),
                asset.changed_pixels.to_string(),
                asset.changed_plain_bytes.to_string(),
                asset.original_cnx_bytes.to_string(),
                asset.replacement_cnx_bytes.to_string(),
                asset.output_plain_sha256.clone(),
                asset.translation_approved.to_string(),
                asset.profile_approved.to_string(),
                asset.final_readback_exact.to_string(),
                report.runtime_verified.to_string(),
                report.product_eligible.to_string(),
            ]
            .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{header}\n{rows}\n")
}

#[cfg(test)]
#[path = "general_raster_development_entries_tests.rs"]
mod tests;
