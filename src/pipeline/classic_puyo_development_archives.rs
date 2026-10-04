//! PUYO1·PUYO2의 현재 BTX 엔트리를 VDD별 한 replacement로 합성한다.

use std::error::Error;
use std::fmt;

use crate::disc::VerifiedSourceImage;

use super::classic_code_glyph_development_entries::PreparedClassicCodeGlyphDevelopmentEntries;
use super::cnx_development_archive_replacement::{
    prepare_cnx_development_archive_replacement, CnxDevelopmentDiscContribution,
};
use super::general_raster_development_entries::{
    GeneralRasterDevelopmentEntriesReport, PreparedGeneralRasterDevelopmentBatch,
};
use super::p2_info_development_archives::{
    finish_p2_info_development_archives, prepare_p2_info_development_entries,
    verify_p2_info_development_archives_readback, P2InfoDevelopmentArchivesBuild,
    P2InfoDevelopmentArchivesReport,
};

const ACTOR: &str = "pipeline::classic_puyo_development_archives";

#[derive(Debug, Clone, Copy)]
struct ArchiveSpec {
    iso_path: &'static str,
    patch_id: &'static str,
}

const ARCHIVES: [ArchiveSpec; 2] = [
    ArchiveSpec {
        iso_path: "/PUYO1/PP1_GRP.VDD;1",
        patch_id: "classic-puyo-development-puyo1",
    },
    ArchiveSpec {
        iso_path: "/PUYO2/PP2_GRP.VDD;1",
        patch_id: "classic-puyo-development-puyo2",
    },
];

/// 두 classic VDD의 P2 INFO와 일반 raster 누적 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicPuyoDevelopmentArchivesReport {
    pub p2_info: P2InfoDevelopmentArchivesReport,
    pub general_raster: GeneralRasterDevelopmentEntriesReport,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// 두 classic VDD 기여와 P2 INFO preview 원천.
pub struct ClassicPuyoDevelopmentArchivesBuild {
    p2_info: P2InfoDevelopmentArchivesBuild,
    pub report: ClassicPuyoDevelopmentArchivesReport,
}

impl ClassicPuyoDevelopmentArchivesBuild {
    pub(super) fn disc_contributions(&self) -> Vec<CnxDevelopmentDiscContribution<'_>> {
        self.p2_info.disc_contributions()
    }

    pub fn p2_info4_surface(&self) -> &[u8] {
        &self.p2_info.info4_surface
    }

    pub fn p2_info5_surface(&self) -> &[u8] {
        &self.p2_info.info5_surface
    }
}

/// classic VDD 엔트리 합성·저장·최종 readback 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicPuyoDevelopmentArchivesError(String);

impl fmt::Display for ClassicPuyoDevelopmentArchivesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ClassicPuyoDevelopmentArchivesError {}

fn error(message: impl Into<String>) -> ClassicPuyoDevelopmentArchivesError {
    ClassicPuyoDevelopmentArchivesError(message.into())
}

/// P2 INFO 두 page와 일반 raster batch를 VDD별로 합성한다.
pub(super) fn prepare_classic_puyo_development_archives(
    source: &VerifiedSourceImage,
    general_raster: &PreparedGeneralRasterDevelopmentBatch,
    code_glyph: &PreparedClassicCodeGlyphDevelopmentEntries,
) -> Result<ClassicPuyoDevelopmentArchivesBuild, ClassicPuyoDevelopmentArchivesError> {
    let p2_entries =
        prepare_p2_info_development_entries(source).map_err(|source| error(source.to_string()))?;
    let mut archives = Vec::new();
    let mut expected_replacement_entries = Vec::new();
    for archive in ARCHIVES {
        let mut entries = p2_entries
            .entry_refs(archive.iso_path)
            .map_err(|source| error(source.to_string()))?;
        entries.extend(
            general_raster
                .entry_refs(archive.iso_path)
                .map_err(|source| error(source.to_string()))?,
        );
        entries.extend(
            code_glyph
                .entry_refs(archive.iso_path)
                .map_err(|source| error(source.to_string()))?,
        );
        expected_replacement_entries.push(entries.len());
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
    let p2_info = finish_p2_info_development_archives(p2_entries, archives)
        .map_err(|source| error(source.to_string()))?;
    let general_raster = general_raster
        .report_for_archives(&ARCHIVES.map(|archive| archive.iso_path))
        .map_err(|source| error(source.to_string()))?;
    let report = ClassicPuyoDevelopmentArchivesReport {
        p2_info: p2_info.report.clone(),
        general_raster,
        runtime_verified: false,
        product_eligible: false,
    };
    if report.p2_info.archives.len() != ARCHIVES.len()
        || report
            .p2_info
            .archives
            .iter()
            .zip(expected_replacement_entries)
            .any(|(archive, expected)| archive.replacement_entries != expected)
        || report.general_raster.logical_assets == 0
        || report.general_raster.physical_assets == 0
        || !report.general_raster.physical_copies_exact
        || report.runtime_verified
        || report.product_eligible
    {
        return Err(error("classic PUYO 개발 archive 판정 drift"));
    }
    Ok(ClassicPuyoDevelopmentArchivesBuild { p2_info, report })
}

/// 누적 disc에서 두 VDD 전체와 일반 raster의 소비를 결속한다.
pub(super) fn verify_classic_puyo_development_archives_readback(
    image: &[u8],
    build: &mut ClassicPuyoDevelopmentArchivesBuild,
    general_raster: &mut PreparedGeneralRasterDevelopmentBatch,
    code_glyph: &mut PreparedClassicCodeGlyphDevelopmentEntries,
) -> Result<(), ClassicPuyoDevelopmentArchivesError> {
    verify_p2_info_development_archives_readback(image, &mut build.p2_info)
        .map_err(|source| error(source.to_string()))?;
    for archive in ARCHIVES {
        general_raster
            .record_verified_archive_readback(archive.iso_path)
            .map_err(|source| error(source.to_string()))?;
        code_glyph
            .record_verified_archive_readback(archive.iso_path)
            .map_err(|source| error(source.to_string()))?;
    }
    build.report.p2_info = build.p2_info.report.clone();
    build.report.general_raster = general_raster
        .report_for_archives(&ARCHIVES.map(|archive| archive.iso_path))
        .map_err(|source| error(source.to_string()))?;
    if build
        .report
        .general_raster
        .assets
        .iter()
        .any(|asset| !asset.final_readback_exact)
    {
        return Err(error("일반 raster 최종 readback 판정 누락"));
    }
    Ok(())
}
