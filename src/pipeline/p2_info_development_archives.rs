//! `P2_INFO4/5.BTX` 수동 번역을 두 물리 VDD에 합성하는 개발 경계.
//!
//! 번역 manifest의 명시적 줄바꿈을 16×16 cell에 그대로 대응시키고, 같은
//! 두 논리 page를 PUYO1/PUYO2의 네 물리 BTX에 적용한다. 각 VDD는 실제 CNX
//! 크기에서 placement를 다시 결정하며 stress PoC의 크기·hash를 입력으로
//! 사용하지 않는다.

use std::error::Error;
use std::fmt;

use crate::disc::VerifiedSourceImage;
use crate::font::P2_INFO_PAGE_DRAFT_RASTER_PROFILE;
use crate::graphics::fixed_cell_text_patch::{
    patch_fixed_cell_text_page, FixedCellTextPatchReport,
};
use crate::graphics::p2_info_translation::{
    p2_info_translation_entries, verify_p2_info_translation_manifest, P2InfoTranslationReport,
    P2InfoTranslationStatus,
};
use crate::graphics::text_catalog::{TextAssetSpec, P2_INFO4_TEXT_ASSET, P2_INFO5_TEXT_ASSET};
use crate::graphics::text_profile::BakedFontBook;

use super::btx_development_archive::{
    prepare_btx_development_entry, BtxDevelopmentArchiveSpec, BtxDevelopmentEntryStorageReport,
};
use super::cnx_development_archive_replacement::{
    prepare_cnx_development_archive_replacement,
    verify_cnx_development_archive_replacement_readback, CnxDevelopmentArchivePlacement,
    CnxDevelopmentDiscContribution, PreparedCnxDevelopmentArchiveReplacement,
};

const ACTOR: &str = "pipeline::p2_info_development_archives";

#[derive(Debug, Clone, Copy)]
struct ArchiveSpec {
    scope: &'static str,
    iso_path: &'static str,
    patch_id: &'static str,
}

const ARCHIVES: [ArchiveSpec; 2] = [
    ArchiveSpec {
        scope: "PUYO1",
        iso_path: "/PUYO1/PP1_GRP.VDD;1",
        patch_id: "p2-info-development-puyo1",
    },
    ArchiveSpec {
        scope: "PUYO2",
        iso_path: "/PUYO2/PP2_GRP.VDD;1",
        patch_id: "p2-info-development-puyo2",
    },
];

const PAGES: [TextAssetSpec<'static>; 2] = [P2_INFO4_TEXT_ASSET, P2_INFO5_TEXT_ASSET];

/// 현재 번역에서 파생한 한 물리 BTX의 저장·조립 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2InfoDevelopmentAssetReport {
    pub scope: &'static str,
    pub archive: &'static str,
    pub asset: &'static str,
    pub translation_status: P2InfoTranslationStatus,
    pub rows: usize,
    pub used_cells: usize,
    pub empty_cells: usize,
    pub changed_cells: usize,
    pub changed_pixels: usize,
    pub changed_plain_bytes: usize,
    pub original_cnx_bytes: usize,
    pub replacement_cnx_bytes: usize,
    pub output_plain_sha256: String,
}

/// P2 INFO를 포함한 현재 classic BTX를 한 번 합성한 물리 VDD의 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2InfoDevelopmentArchiveReport {
    pub scope: &'static str,
    pub archive: &'static str,
    pub placement: &'static str,
    pub archive_entries: usize,
    pub replacement_entries: usize,
    pub moved_entry_offsets: usize,
    pub original_vdd_bytes: usize,
    pub replacement_vdd_bytes: usize,
    pub target_entries_exact: bool,
    pub non_target_entries_exact: bool,
    pub final_readback_exact: bool,
}

/// 현재 수동 번역 네 물리 사본의 누적 개발 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2InfoDevelopmentArchivesReport {
    pub translation: P2InfoTranslationReport,
    pub assets: Vec<P2InfoDevelopmentAssetReport>,
    pub archives: Vec<P2InfoDevelopmentArchiveReport>,
    pub logical_pages: usize,
    pub physical_assets: usize,
    pub physical_copies_exact: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// 두 VDD 기여와 논리 page별 preview 원천.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2InfoDevelopmentArchivesBuild {
    archives: Vec<PreparedCnxDevelopmentArchiveReplacement>,
    pub info4_surface: Vec<u8>,
    pub info5_surface: Vec<u8>,
    pub report: P2InfoDevelopmentArchivesReport,
}

/// 다른 PUYO1/2 BTX와 같은 VDD replacement로 합치기 전의 P2 INFO 엔트리 집합.
pub(super) struct PreparedP2InfoDevelopmentEntries {
    archives: Vec<PreparedP2InfoArchiveEntries>,
    translation: P2InfoTranslationReport,
    asset_reports: Vec<P2InfoDevelopmentAssetReport>,
    info4_surface: Vec<u8>,
    info5_surface: Vec<u8>,
    physical_copies_exact: bool,
}

struct PreparedP2InfoArchiveEntries {
    spec: ArchiveSpec,
    entries: Vec<super::btx_development_archive::PreparedBtxDevelopmentEntry>,
}

impl PreparedP2InfoDevelopmentEntries {
    pub(super) fn entry_refs(
        &self,
        iso_path: &str,
    ) -> Result<
        Vec<&super::btx_development_archive::PreparedBtxDevelopmentEntry>,
        P2InfoDevelopmentArchivesError,
    > {
        self.archives
            .iter()
            .find(|archive| archive.spec.iso_path.eq_ignore_ascii_case(iso_path))
            .map(|archive| archive.entries.iter().collect())
            .ok_or_else(|| error(format!("P2 INFO archive 입력 누락: {iso_path}")))
    }
}

impl P2InfoDevelopmentArchivesBuild {
    pub(super) fn disc_contributions(&self) -> Vec<CnxDevelopmentDiscContribution<'_>> {
        self.archives
            .iter()
            .map(PreparedCnxDevelopmentArchiveReplacement::disc_contribution)
            .collect()
    }
}

/// 원본·manifest·fixed-cell·VDD 합성 또는 최종 readback 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2InfoDevelopmentArchivesError(String);

impl fmt::Display for P2InfoDevelopmentArchivesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for P2InfoDevelopmentArchivesError {}

fn error(message: impl Into<String>) -> P2InfoDevelopmentArchivesError {
    P2InfoDevelopmentArchivesError(message.into())
}

fn placement_label(placement: CnxDevelopmentArchivePlacement) -> &'static str {
    match placement {
        CnxDevelopmentArchivePlacement::InPlace => "in-place",
        CnxDevelopmentArchivePlacement::RepackInContainer => "repack-in-container",
        CnxDevelopmentArchivePlacement::RelocateToReservation => "relocate-to-32m-reservation",
    }
}

fn asset_report(
    archive: ArchiveSpec,
    asset: TextAssetSpec<'static>,
    translation_status: P2InfoTranslationStatus,
    storage: BtxDevelopmentEntryStorageReport,
    patch: FixedCellTextPatchReport,
) -> P2InfoDevelopmentAssetReport {
    P2InfoDevelopmentAssetReport {
        scope: archive.scope,
        archive: archive.iso_path,
        asset: asset.name,
        translation_status,
        rows: patch.rows,
        used_cells: patch.used_cells,
        empty_cells: patch.empty_cells,
        changed_cells: patch.changed_cells,
        changed_pixels: patch.changed_pixels,
        changed_plain_bytes: patch.changed_bytes,
        original_cnx_bytes: storage.original_cnx_bytes,
        replacement_cnx_bytes: storage.replacement_cnx_bytes,
        output_plain_sha256: patch.output_sha256,
    }
}

/// 지원 원본에서 현재 두 수동 번역의 네 물리 BTX 엔트리를 준비한다.
pub(super) fn prepare_p2_info_development_entries(
    source: &VerifiedSourceImage,
) -> Result<PreparedP2InfoDevelopmentEntries, P2InfoDevelopmentArchivesError> {
    let translation =
        verify_p2_info_translation_manifest().map_err(|source| error(source.to_string()))?;
    let entries = p2_info_translation_entries().map_err(|source| error(source.to_string()))?;
    let fonts = BakedFontBook::with_bundled_maplestory_light()
        .map_err(|source| error(source.to_string()))?;
    let mut archives = Vec::new();
    let mut asset_reports = Vec::new();
    let mut info4_surface = None;
    let mut info5_surface = None;
    let mut physical_copies_exact = true;

    for archive in ARCHIVES {
        let mut prepared_entries = Vec::new();
        for asset in PAGES {
            let translation_entry = entries
                .iter()
                .find(|entry| entry.asset_name == asset.name)
                .ok_or_else(|| error(format!("{} 번역 entry 누락", asset.name)))?;
            if !translation_entry
                .archive_paths
                .split('\n')
                .any(|path| path.eq_ignore_ascii_case(archive.iso_path))
            {
                return Err(error(format!(
                    "{} 번역이 {} 물리 사본을 선언하지 않음",
                    asset.name, archive.iso_path
                )));
            }
            let spec = BtxDevelopmentArchiveSpec {
                iso_path: archive.iso_path,
                entry_name: asset.name,
                expected_cnx_subtype: *b"bi_",
                patch_id: archive.patch_id,
                actor: ACTOR,
            };
            let (prepared, patch) = prepare_btx_development_entry(source, spec, |plain| {
                patch_fixed_cell_text_page(
                    plain,
                    asset,
                    &fonts,
                    &translation_entry.ko,
                    P2_INFO_PAGE_DRAFT_RASTER_PROFILE,
                )
                .map_err(|source| source.to_string())
            })
            .map_err(|source| error(source.to_string()))?;
            let canonical = match asset.name {
                "P2_INFO4.BTX" => &mut info4_surface,
                "P2_INFO5.BTX" => &mut info5_surface,
                _ => return Err(error(format!("미등록 P2 INFO page: {}", asset.name))),
            };
            if let Some(previous) = canonical.as_ref() {
                physical_copies_exact &= previous == prepared.patched_plain();
            } else {
                *canonical = Some(prepared.patched_plain().to_vec());
            }
            asset_reports.push(asset_report(
                archive,
                asset,
                translation_entry.translation_status,
                prepared.storage,
                patch,
            ));
            prepared_entries.push(prepared);
        }
        archives.push(PreparedP2InfoArchiveEntries {
            spec: archive,
            entries: prepared_entries,
        });
    }

    let expected_physical_assets = entries
        .iter()
        .map(|entry| entry.archive_paths.split('\n').count())
        .sum::<usize>();
    if asset_reports.len() != expected_physical_assets || !physical_copies_exact {
        return Err(error(format!(
            "P2 INFO 물리 사본 불일치: expected={}, actual={}, exact={physical_copies_exact}",
            expected_physical_assets,
            asset_reports.len()
        )));
    }
    Ok(PreparedP2InfoDevelopmentEntries {
        archives,
        translation,
        asset_reports,
        info4_surface: info4_surface.ok_or_else(|| error("P2_INFO4 preview 원천 누락"))?,
        info5_surface: info5_surface.ok_or_else(|| error("P2_INFO5 preview 원천 누락"))?,
        physical_copies_exact,
    })
}

pub(super) fn finish_p2_info_development_archives(
    prepared: PreparedP2InfoDevelopmentEntries,
    archives: Vec<PreparedCnxDevelopmentArchiveReplacement>,
) -> Result<P2InfoDevelopmentArchivesBuild, P2InfoDevelopmentArchivesError> {
    let archive_reports = ARCHIVES
        .into_iter()
        .zip(&archives)
        .map(|(archive, build)| P2InfoDevelopmentArchiveReport {
            scope: archive.scope,
            archive: archive.iso_path,
            placement: placement_label(build.report.placement),
            archive_entries: build.report.archive_entries,
            replacement_entries: build.report.replacement_entries,
            moved_entry_offsets: build.report.moved_entry_offsets,
            original_vdd_bytes: build.report.original_vdd_bytes,
            replacement_vdd_bytes: build.report.vdd_bytes,
            target_entries_exact: build.report.target_entries_exact,
            non_target_entries_exact: build.report.non_target_entries_exact,
            final_readback_exact: false,
        })
        .collect::<Vec<_>>();
    let report = P2InfoDevelopmentArchivesReport {
        logical_pages: prepared.translation.entries,
        physical_assets: prepared.asset_reports.len(),
        translation: prepared.translation,
        assets: prepared.asset_reports,
        archives: archive_reports,
        physical_copies_exact: prepared.physical_copies_exact,
        runtime_verified: false,
        product_eligible: false,
    };
    if report.translation.product_eligible
        || report.logical_pages == 0
        || report.physical_assets == 0
        || report.archives.len() != ARCHIVES.len()
        || !report.physical_copies_exact
        || report.runtime_verified
        || report.product_eligible
    {
        return Err(error("P2 INFO 개발 archive 판정 drift"));
    }
    Ok(P2InfoDevelopmentArchivesBuild {
        archives,
        info4_surface: prepared.info4_surface,
        info5_surface: prepared.info5_surface,
        report,
    })
}

/// 지원 원본에서 현재 두 수동 번역을 네 물리 BTX와 두 VDD에 합성한다.
pub fn prepare_p2_info_development_archives(
    source: &VerifiedSourceImage,
) -> Result<P2InfoDevelopmentArchivesBuild, P2InfoDevelopmentArchivesError> {
    let prepared = prepare_p2_info_development_entries(source)?;
    let mut archives = Vec::new();
    for archive in ARCHIVES {
        let entries = prepared.entry_refs(archive.iso_path)?;
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
    finish_p2_info_development_archives(prepared, archives)
}

/// 누적 disc가 준비한 두 VDD 전체를 정확히 소비했는지 확인한다.
pub fn verify_p2_info_development_archives_readback(
    image: &[u8],
    build: &mut P2InfoDevelopmentArchivesBuild,
) -> Result<(), P2InfoDevelopmentArchivesError> {
    for archive in &build.archives {
        verify_cnx_development_archive_replacement_readback(image, archive)
            .map_err(|source| error(source.to_string()))?;
    }
    for report in &mut build.report.archives {
        report.final_readback_exact = true;
    }
    Ok(())
}

/// 네 물리 BTX의 조립 결과와 소속 VDD placement를 행별 TSV로 만든다.
pub fn render_p2_info_development_archives_tsv(report: &P2InfoDevelopmentArchivesReport) -> String {
    let header = [
        "scope",
        "archive",
        "asset",
        "translation_status",
        "rows",
        "used_cells",
        "empty_cells",
        "changed_cells",
        "changed_pixels",
        "changed_plain_bytes",
        "original_cnx_bytes",
        "replacement_cnx_bytes",
        "output_plain_sha256",
        "archive_placement",
        "original_vdd_bytes",
        "replacement_vdd_bytes",
        "target_entries_exact",
        "non_target_entries_exact",
        "final_readback_exact",
        "runtime_verified",
        "product_eligible",
    ]
    .join("\t");
    let rows = report
        .assets
        .iter()
        .map(|asset| {
            let archive = report
                .archives
                .iter()
                .find(|archive| archive.archive.eq_ignore_ascii_case(asset.archive))
                .expect("검증된 P2 INFO asset에는 소속 archive 판정이 있음");
            [
                asset.scope.to_string(),
                asset.archive.to_string(),
                asset.asset.to_string(),
                asset.translation_status.label().to_string(),
                asset.rows.to_string(),
                asset.used_cells.to_string(),
                asset.empty_cells.to_string(),
                asset.changed_cells.to_string(),
                asset.changed_pixels.to_string(),
                asset.changed_plain_bytes.to_string(),
                asset.original_cnx_bytes.to_string(),
                asset.replacement_cnx_bytes.to_string(),
                asset.output_plain_sha256.clone(),
                archive.placement.to_string(),
                archive.original_vdd_bytes.to_string(),
                archive.replacement_vdd_bytes.to_string(),
                archive.target_entries_exact.to_string(),
                archive.non_target_entries_exact.to_string(),
                archive.final_readback_exact.to_string(),
                report.runtime_verified.to_string(),
                report.product_eligible.to_string(),
            ]
            .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{header}\n{rows}\n")
}
