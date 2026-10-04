//! P2 INFO page별 CNX 합성과 VDD 재조립 준비.

use std::collections::BTreeMap;

use crate::archive::{VddArchive, VddError};
use crate::compress::{compress, decompress, parse_header};
use crate::disc::Iso9660;
use crate::font::P2_INFO_PAGE_DRAFT_RASTER_PROFILE;
use crate::graphics::fixed_cell_text_patch::patch_fixed_cell_text_page;
use crate::graphics::text_profile::BakedFontBook;

use super::{
    digest, error, text_asset, P2InfoPageStressAssetReport, P2InfoPageStressDiscError,
    P2InfoPageStressVddReport, P2InfoPageVddRewriteMethod, TargetSpec, TARGETS,
};

#[derive(Debug)]
pub(super) struct PreparedTarget {
    pub(super) spec: TargetSpec,
    pub(super) replacement_cnx: Vec<u8>,
    pub(super) replacement_plain: Vec<u8>,
}

#[derive(Debug)]
pub(super) struct PreparedVdd {
    pub(super) scope: &'static str,
    pub(super) archive: &'static str,
    pub(super) original_vdd: Vec<u8>,
    pub(super) original_archive: VddArchive,
    pub(super) replacement_vdd: Vec<u8>,
    pub(super) method: P2InfoPageVddRewriteMethod,
    pub(super) targets: Vec<PreparedTarget>,
}

pub(super) fn verify_rebuilt_vdd(
    prepared: &PreparedVdd,
    rebuilt: &[u8],
) -> Result<(), P2InfoPageStressDiscError> {
    let length_is_valid = match prepared.method {
        P2InfoPageVddRewriteMethod::InPlace | P2InfoPageVddRewriteMethod::RepackInContainer => {
            rebuilt.len() == prepared.original_vdd.len()
        }
        P2InfoPageVddRewriteMethod::RelocateToReservation => {
            rebuilt.len() > prepared.original_vdd.len()
        }
    };
    if !length_is_valid {
        return Err(error(format!(
            "{} VDD placement 길이 전제 불일치: original={}, replacement={}, method={:?}",
            prepared.archive,
            prepared.original_vdd.len(),
            rebuilt.len(),
            prepared.method,
        )));
    }
    let archive = VddArchive::parse(rebuilt)
        .map_err(|source| error(format!("{} VDD readback 실패: {source}", prepared.archive)))?;
    if archive.entry_count() != prepared.original_archive.entry_count() {
        return Err(error(format!("{} VDD 엔트리 수 변경", prepared.archive)));
    }
    for original_entry in prepared.original_archive.entries() {
        let final_entry = archive.find(original_entry.name()).ok_or_else(|| {
            error(format!(
                "{}:{} 최종 엔트리 누락",
                prepared.archive,
                original_entry.name()
            ))
        })?;
        if let Some(target) = prepared.targets.iter().find(|target| {
            original_entry
                .name()
                .eq_ignore_ascii_case(target.spec.asset)
        }) {
            let cnx = archive.read_file(final_entry);
            if cnx != target.replacement_cnx {
                return Err(error(format!(
                    "{}:{} 최종 CNX readback 불일치",
                    prepared.archive, target.spec.asset
                )));
            }
            let plain = decompress(cnx).map_err(|source| {
                error(format!(
                    "{}:{} 최종 plain 해제 실패: {source}",
                    prepared.archive, target.spec.asset
                ))
            })?;
            if plain != target.replacement_plain {
                return Err(error(format!(
                    "{}:{} 최종 plain readback 불일치",
                    prepared.archive, target.spec.asset
                )));
            }
        } else if archive.read_file(final_entry)
            != prepared.original_archive.read_file(original_entry)
        {
            return Err(error(format!(
                "{}:{} 비대상 엔트리 변경",
                prepared.archive,
                original_entry.name()
            )));
        }
    }
    Ok(())
}

pub(super) fn prepare_vdd(
    iso: &Iso9660<'_>,
    scope: &'static str,
    archive_path: &'static str,
    fonts: &BakedFontBook,
    stress_text: &str,
) -> Result<
    (
        PreparedVdd,
        Vec<P2InfoPageStressAssetReport>,
        P2InfoPageStressVddReport,
    ),
    P2InfoPageStressDiscError,
> {
    let original_vdd = iso
        .read_file(archive_path)
        .map_err(|source| error(source.to_string()))?;
    let original_archive = VddArchive::parse(&original_vdd)
        .map_err(|source| error(format!("{archive_path} VDD 파싱 실패: {source}")))?;
    let mut replacements = BTreeMap::new();
    let mut targets = Vec::new();
    let mut asset_reports = Vec::new();
    for spec in TARGETS
        .iter()
        .copied()
        .filter(|target| target.archive == archive_path)
    {
        let entry = original_archive
            .find(spec.asset)
            .ok_or_else(|| error(format!("{}:{} 원본 엔트리 누락", spec.archive, spec.asset)))?;
        let original_cnx = original_archive.read_file(entry);
        let header = parse_header(original_cnx)
            .map_err(|source| error(format!("{}:{}: {source}", spec.archive, spec.asset)))?;
        if header.subtype != *b"bi_" {
            return Err(error(format!(
                "{}:{} CNX subtype 불일치: {:?}",
                spec.archive, spec.asset, header.subtype
            )));
        }
        let original_plain = decompress(original_cnx)
            .map_err(|source| error(format!("{}:{}: {source}", spec.archive, spec.asset)))?;
        let (replacement_plain, patch) = patch_fixed_cell_text_page(
            &original_plain,
            text_asset(spec.asset)?,
            fonts,
            stress_text,
            P2_INFO_PAGE_DRAFT_RASTER_PROFILE,
        )
        .map_err(|source| error(source.to_string()))?;
        let replacement_cnx = compress(&replacement_plain, header.subtype)
            .map_err(|source| error(format!("{}:{}: {source}", spec.archive, spec.asset)))?;
        if decompress(&replacement_cnx).map_err(|source| error(source.to_string()))?
            != replacement_plain
        {
            return Err(error(format!(
                "{}:{} 재압축 자기 readback 불일치",
                spec.archive, spec.asset
            )));
        }
        if replacements
            .insert(spec.asset.to_string(), replacement_cnx.clone())
            .is_some()
        {
            return Err(error(format!(
                "{}:{} replacement 중복",
                spec.archive, spec.asset
            )));
        }
        asset_reports.push(P2InfoPageStressAssetReport {
            scope: spec.scope,
            archive: spec.archive,
            asset: spec.asset,
            cnx_before: original_cnx.len(),
            cnx_after: replacement_cnx.len(),
            changed_cells: patch.changed_cells,
            changed_pixels: patch.changed_pixels,
            changed_plain_bytes: patch.changed_bytes,
            output_plain_sha256: patch.output_sha256,
        });
        targets.push(PreparedTarget {
            spec,
            replacement_cnx,
            replacement_plain,
        });
    }
    if targets.len() != 2 {
        return Err(error(format!(
            "{archive_path} P2 INFO target 분모 불일치: {}",
            targets.len()
        )));
    }
    let (replacement_vdd, method) = match original_archive.replace_in_place(&replacements) {
        Ok(rebuilt) => (rebuilt, P2InfoPageVddRewriteMethod::InPlace),
        Err(VddError::ReplacementDoesNotFit { .. }) => {
            match original_archive.repack_in_container(&replacements) {
                Ok(rebuilt) => (rebuilt, P2InfoPageVddRewriteMethod::RepackInContainer),
                Err(VddError::RepackedArchiveDoesNotFit { .. }) => (
                    original_archive.rebuild(&replacements).map_err(|source| {
                        error(format!("{archive_path} VDD 성장 재조립 실패: {source}"))
                    })?,
                    P2InfoPageVddRewriteMethod::RelocateToReservation,
                ),
                Err(source) => {
                    return Err(error(format!("{archive_path} VDD 재패킹 실패: {source}")))
                }
            }
        }
        Err(source) => {
            return Err(error(format!(
                "{archive_path} VDD 제자리 교체 실패: {source}"
            )))
        }
    };
    let prepared = PreparedVdd {
        scope,
        archive: archive_path,
        original_vdd,
        original_archive,
        replacement_vdd,
        method,
        targets,
    };
    verify_rebuilt_vdd(&prepared, &prepared.replacement_vdd)?;
    let vdd_report = P2InfoPageStressVddReport {
        scope,
        archive: archive_path,
        method,
        changed_vdd_bytes: prepared
            .original_vdd
            .iter()
            .zip(&prepared.replacement_vdd)
            .filter(|(before, after)| before != after)
            .count()
            + prepared
                .replacement_vdd
                .len()
                .saturating_sub(prepared.original_vdd.len()),
        output_vdd_sha256: digest(&prepared.replacement_vdd),
    };
    Ok((prepared, asset_reports, vdd_report))
}
