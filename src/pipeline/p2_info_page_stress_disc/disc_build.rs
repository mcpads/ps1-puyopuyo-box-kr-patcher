//! 준비한 P2 INFO VDD의 32M reservation 배치와 전체 디스크 검산.

use std::collections::BTreeMap;

use crate::compress::cnx_output_size_upper_bound;
use crate::coords::{exe_map, LogicalFile};
use crate::disc::{
    add_single_extent_record_update, apply_form1_write_plan, partition_extent_with_remainder,
    verify_box_32m_reservation, DiscImage, Extent, Iso9660, PsxExe, VerifiedSourceImage,
    BOX_32M_PATH,
};
use crate::graphics::fixed_cell_text_patch::build_p2_info_page_capacity_stress_text;
use crate::graphics::text_profile::BakedFontBook;
use crate::patch::{UnchangedBoundary, WriteIntent, WritePlan};
use crate::source::{BOX_EXE_ISO_PATHS, FORM1_DATA_LEN, SECTOR_LEN};

use crate::pipeline::quest_exe_relocation_control::{count_extent_overlaps, extent_intervals};
use crate::pipeline::vdd_plan::{
    add_logical_span, add_vdd_entry_replacement, add_vdd_repack, logical_file,
};

use super::asset_preparation::{prepare_vdd, verify_rebuilt_vdd};
use super::{
    digest, error, P2InfoPageStressDiscBuild, P2InfoPageStressDiscError,
    P2InfoPageStressDiscReport, P2InfoPageStressRelocationReport, P2InfoPageVddRewriteMethod,
    BOX_P2_INFO_PAGE_STRESS_CNX_UPPER_BOUND_BYTES, BOX_P2_INFO_PAGE_STRESS_DISC_ASSETS,
    BOX_P2_INFO_PAGE_STRESS_DISC_DATA_WRITES, BOX_P2_INFO_PAGE_STRESS_DISC_FINAL_EXPECTED_WRITES,
    BOX_P2_INFO_PAGE_STRESS_DISC_IMAGE_SHA256, BOX_P2_INFO_PAGE_STRESS_DISC_METADATA_WRITES,
    BOX_P2_INFO_PAGE_STRESS_DISC_PLANNED_WRITES, BOX_P2_INFO_PAGE_STRESS_DISC_TOUCHED_SECTORS,
    BOX_P2_INFO_PAGE_STRESS_DISC_VDDS, BOX_P2_INFO_PAGE_STRESS_PUYO1_RELOCATED_BYTES,
    BOX_P2_INFO_PAGE_STRESS_PUYO1_RELOCATED_LBA, BOX_P2_INFO_PAGE_STRESS_PUYO2_RELOCATED_BYTES,
    BOX_P2_INFO_PAGE_STRESS_PUYO2_RELOCATED_LBA, BOX_P2_INFO_PAGE_STRESS_RELOCATED_32M_BYTES,
    BOX_P2_INFO_PAGE_STRESS_RELOCATED_32M_LBA, BOX_P2_INFO_PAGE_STRESS_RESERVATION_HEADROOM_BYTES,
    BOX_P2_INFO_PAGE_STRESS_VDD_UPPER_BOUND_BYTES, EXPECTED_ASSET_ROWS, EXPECTED_VDD_ROWS,
    P2_INFO_PAGE_STRESS_DISC_SCHEMA_VERSION,
};

fn read_box_executable_texts(
    iso: &Iso9660<'_>,
) -> Result<Vec<(String, Vec<u8>)>, P2InfoPageStressDiscError> {
    let mut executables = Vec::with_capacity(BOX_EXE_ISO_PATHS.len());
    for (path, name) in BOX_EXE_ISO_PATHS {
        let raw = iso
            .read_file(path)
            .map_err(|source| error(source.to_string()))?;
        let executable =
            PsxExe::parse(&raw).map_err(|source| error(format!("{path}: {source}")))?;
        let map = exe_map(name).ok_or_else(|| error(format!("{name}: EXE 사상 누락")))?;
        executable
            .verify_map(map)
            .map_err(|source| error(format!("{path}: {source}")))?;
        executables.push((name.to_string(), executable.text().to_vec()));
    }
    Ok(executables)
}

pub(super) fn build(
    source: &VerifiedSourceImage,
) -> Result<P2InfoPageStressDiscBuild, P2InfoPageStressDiscError> {
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let fonts = BakedFontBook::with_bundled_maplestory_light()
        .map_err(|source| error(source.to_string()))?;
    let stress = build_p2_info_page_capacity_stress_text(&fonts)
        .map_err(|source| error(source.to_string()))?;
    let mut prepared = Vec::new();
    let mut assets = Vec::new();
    let mut vdds = Vec::new();
    for (scope, archive) in [
        ("PUYO1", "/PUYO1/PP1_GRP.VDD;1"),
        ("PUYO2", "/PUYO2/PP2_GRP.VDD;1"),
    ] {
        let (vdd, mut asset_rows, vdd_row) =
            prepare_vdd(&iso, scope, archive, &fonts, &stress.text)?;
        prepared.push(vdd);
        assets.append(&mut asset_rows);
        vdds.push(vdd_row);
    }
    if assets.len() != BOX_P2_INFO_PAGE_STRESS_DISC_ASSETS
        || vdds.len() != BOX_P2_INFO_PAGE_STRESS_DISC_VDDS
    {
        return Err(error(format!(
            "P2 INFO stress 분모 불일치: assets/vdds={}/{}",
            assets.len(),
            vdds.len()
        )));
    }
    let actual_assets = assets
        .iter()
        .map(|row| {
            (
                row.scope,
                row.asset,
                row.cnx_before,
                row.cnx_after,
                row.changed_pixels,
                row.changed_plain_bytes,
                row.output_plain_sha256.as_str(),
            )
        })
        .collect::<Vec<_>>();
    if actual_assets != EXPECTED_ASSET_ROWS {
        return Err(error(format!(
            "P2 INFO stress asset 기준선 불일치: {actual_assets:?}"
        )));
    }
    let actual_vdds = vdds
        .iter()
        .map(|row| {
            (
                row.scope,
                row.method,
                row.changed_vdd_bytes,
                row.output_vdd_sha256.as_str(),
            )
        })
        .collect::<Vec<_>>();
    if actual_vdds != EXPECTED_VDD_ROWS {
        return Err(error(format!(
            "P2 INFO stress VDD 기준선 불일치: {actual_vdds:?}"
        )));
    }

    let relocation_required = prepared
        .iter()
        .any(|vdd| vdd.method == P2InfoPageVddRewriteMethod::RelocateToReservation);
    let executables = relocation_required
        .then(|| read_box_executable_texts(&iso))
        .transpose()?
        .unwrap_or_default();
    let reservation = relocation_required
        .then(|| {
            verify_box_32m_reservation(
                &iso,
                source.image(),
                executables
                    .iter()
                    .map(|(name, text)| (name.as_str(), text.as_slice())),
            )
            .map_err(|source| error(source.to_string()))
        })
        .transpose()?;
    let conservative_cnx_upper_bound_bytes = prepared
        .iter()
        .flat_map(|vdd| &vdd.targets)
        .map(|target| {
            cnx_output_size_upper_bound(target.replacement_plain.len())
                .ok_or_else(|| error("P2 INFO CNX 보수 상한 overflow"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if conservative_cnx_upper_bound_bytes
        .iter()
        .any(|bound| *bound != BOX_P2_INFO_PAGE_STRESS_CNX_UPPER_BOUND_BYTES)
    {
        return Err(error(format!(
            "P2 INFO CNX 보수 상한 기준선 불일치: {conservative_cnx_upper_bound_bytes:?}"
        )));
    }
    let conservative_vdd_upper_bound_bytes = prepared
        .iter()
        .map(|vdd| {
            let target_bound = vdd
                .targets
                .iter()
                .map(|target| {
                    cnx_output_size_upper_bound(target.replacement_plain.len())
                        .ok_or_else(|| error("P2 INFO target CNX 보수 상한 overflow"))
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .try_fold(0u64, |total, bound| {
                    total
                        .checked_add(bound as u64)
                        .ok_or_else(|| error("P2 INFO target CNX 보수 상한 합계 overflow"))
                })?;
            (vdd.original_vdd.len() as u64)
                .checked_add(target_bound)
                .ok_or_else(|| error("P2 INFO VDD 보수 상한 overflow"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .try_fold(0u64, |total, bound| {
            total
                .checked_add(bound)
                .ok_or_else(|| error("P2 INFO VDD 보수 상한 합계 overflow"))
        })?;
    let reservation_capacity = reservation.as_ref().map_or(0, |summary| summary.bytes);
    let reservation_headroom_bytes = reservation_capacity
        .checked_sub(conservative_vdd_upper_bound_bytes)
        .ok_or_else(|| {
            error(format!(
                "P2 INFO 보수 VDD 상한이 reservation을 넘음: required={conservative_vdd_upper_bound_bytes}, capacity={reservation_capacity}"
            ))
        })?;
    if (
        conservative_vdd_upper_bound_bytes,
        reservation_headroom_bytes,
    ) != (
        BOX_P2_INFO_PAGE_STRESS_VDD_UPPER_BOUND_BYTES,
        BOX_P2_INFO_PAGE_STRESS_RESERVATION_HEADROOM_BYTES,
    ) {
        return Err(error(format!(
            "P2 INFO reservation 보수 상한 기준선 불일치: VDD/headroom={conservative_vdd_upper_bound_bytes}/{reservation_headroom_bytes}"
        )));
    }
    let reservation_entry = iso
        .find(BOX_32M_PATH)
        .map_err(|source| error(source.to_string()))?;
    let original_reservation_extent = reservation_entry
        .extents()
        .first()
        .copied()
        .ok_or_else(|| error("32M reservation extent 누락"))?;
    let mut reservation_remainder = match reservation.as_ref() {
        Some(summary) => Extent {
            lba: summary.lba,
            len: u32::try_from(summary.bytes)
                .map_err(|_| error("P2 INFO 32M reservation 길이 overflow"))?,
        },
        None => original_reservation_extent,
    };
    let mut relocations = Vec::new();
    let mut final_extents = BTreeMap::new();
    let mut plan = WritePlan::new();
    for vdd in &prepared {
        let entry = iso
            .find(vdd.archive)
            .map_err(|source| error(source.to_string()))?;
        match vdd.method {
            P2InfoPageVddRewriteMethod::InPlace => {
                let file = logical_file(entry).map_err(error)?;
                for target in &vdd.targets {
                    add_vdd_entry_replacement(
                        &mut plan,
                        file,
                        &vdd.original_archive,
                        &vdd.original_vdd,
                        &vdd.replacement_vdd,
                        target.spec.asset,
                        target.replacement_cnx.len(),
                        &format!("p2-info-page-stress:{}:{}", vdd.scope, target.spec.asset),
                        "pipeline::p2_info_page_stress_disc",
                    )
                    .map_err(error)?;
                }
                let [extent] = entry.extents() else {
                    return Err(error(format!("{} 원본 extent 분모 불일치", vdd.archive)));
                };
                final_extents.insert(vdd.archive, *extent);
            }
            P2InfoPageVddRewriteMethod::RepackInContainer => {
                add_vdd_repack(
                    &mut plan,
                    logical_file(entry).map_err(error)?,
                    &vdd.original_vdd,
                    &vdd.replacement_vdd,
                    &format!("p2-info-page-stress:{}", vdd.scope),
                    "pipeline::p2_info_page_stress_disc",
                )
                .map_err(error)?;
                let [extent] = entry.extents() else {
                    return Err(error(format!("{} 원본 extent 분모 불일치", vdd.archive)));
                };
                final_extents.insert(vdd.archive, *extent);
            }
            P2InfoPageVddRewriteMethod::RelocateToReservation => {
                let [original_extent] = entry.extents() else {
                    return Err(error(format!("{} 원본 extent 분모 불일치", vdd.archive)));
                };
                let relocated_bytes = u32::try_from(vdd.replacement_vdd.len())
                    .map_err(|_| error(format!("{} 성장 VDD 길이 overflow", vdd.archive)))?;
                if !relocated_bytes.is_multiple_of(FORM1_DATA_LEN as u32) {
                    return Err(error(format!(
                        "{} 성장 VDD가 Form1 sector 정렬이 아님: {relocated_bytes}",
                        vdd.archive
                    )));
                }
                let relocated_sectors = relocated_bytes / FORM1_DATA_LEN as u32;
                let partition =
                    partition_extent_with_remainder(reservation_remainder, &[relocated_bytes])
                        .map_err(|source| error(format!("{}: {source}", vdd.archive)))?;
                let [relocated] = partition.allocations() else {
                    return Err(error(format!(
                        "{} relocation extent 분모 불일치",
                        vdd.archive
                    )));
                };
                let relocated = *relocated;
                let expected_zero = vec![0; vdd.replacement_vdd.len()];
                add_logical_span(
                    &mut plan,
                    LogicalFile::new(relocated.lba, relocated.len),
                    0,
                    &expected_zero,
                    &vdd.replacement_vdd,
                    &format!("p2-info-page-stress:{}:relocated-vdd", vdd.scope),
                    "pipeline::p2_info_page_stress_disc",
                    "32M reservation에 성장한 P2 INFO GRP VDD 배치",
                    WriteIntent::Data,
                )
                .map_err(error)?;
                add_single_extent_record_update(
                    &mut plan,
                    source.image(),
                    entry,
                    relocated,
                    &format!("p2-info-page-stress:{}:iso-record", vdd.scope),
                    "pipeline::p2_info_page_stress_disc",
                    "성장한 P2 INFO GRP VDD를 32M reservation으로 재배치",
                )
                .map_err(|source| error(source.to_string()))?;
                let original_start = u64::from(original_extent.lba) * SECTOR_LEN as u64;
                let original_end = original_start
                    .checked_add(
                        u64::from(original_extent.len.div_ceil(FORM1_DATA_LEN as u32))
                            * SECTOR_LEN as u64,
                    )
                    .ok_or_else(|| error(format!("{} 원본 extent 끝 overflow", vdd.archive)))?;
                plan.unchanged.push(UnchangedBoundary::new(
                    format!("p2-info-page-stress:{}:original-extent", vdd.scope),
                    original_start..original_end,
                    "재배치 뒤 원본 GRP raw extent byte-identical 보존",
                ));
                relocations.push(P2InfoPageStressRelocationReport {
                    scope: vdd.scope,
                    archive: vdd.archive,
                    original_lba: original_extent.lba,
                    original_bytes: original_extent.len,
                    relocated_lba: relocated.lba,
                    relocated_bytes: relocated.len,
                    relocated_sectors,
                });
                final_extents.insert(vdd.archive, relocated);
                reservation_remainder = partition.remainder();
            }
        }
    }
    let relocated_32m = reservation_remainder;
    if relocation_required {
        add_single_extent_record_update(
            &mut plan,
            source.image(),
            reservation_entry,
            relocated_32m,
            "p2-info-page-stress:32m-record",
            "pipeline::p2_info_page_stress_disc",
            "재배치 GRP VDD 뒤에서 시작하도록 32M.BIN 축소",
        )
        .map_err(|source| error(source.to_string()))?;
    }
    let planned_writes = plan.writes.len();
    let metadata_writes = plan
        .writes
        .iter()
        .filter(|write| matches!(write.intent, WriteIntent::Metadata))
        .count();
    let data_writes = plan
        .writes
        .iter()
        .filter(|write| matches!(write.intent, WriteIntent::Data))
        .count();
    let touched_sectors = plan.touched_sectors().len();
    let final_expected_writes = planned_writes + touched_sectors;
    let output = apply_form1_write_plan(source.image(), &plan)
        .map_err(|source| error(source.to_string()))?;
    let final_image = DiscImage::from_bytes(output).map_err(|source| error(source.to_string()))?;
    let final_iso = Iso9660::parse(&final_image).map_err(|source| error(source.to_string()))?;
    for vdd in &prepared {
        let expected_extent = *final_extents
            .get(vdd.archive)
            .ok_or_else(|| error(format!("{} 최종 extent 기대값 누락", vdd.archive)))?;
        let final_vdd = final_iso
            .read_file_at_extent(vdd.archive, expected_extent)
            .map_err(|source| error(source.to_string()))?;
        if final_vdd != vdd.replacement_vdd {
            return Err(error(format!("{} 최종 VDD readback 불일치", vdd.archive)));
        }
        verify_rebuilt_vdd(vdd, &final_vdd)?;
    }
    final_iso
        .verify_single_extent_file(BOX_32M_PATH, relocated_32m)
        .map_err(|source| error(source.to_string()))?;
    let extent_overlaps = count_extent_overlaps(
        &extent_intervals(&final_iso).map_err(|source| error(source.to_string()))?,
    )
    .map_err(|source| error(source.to_string()))?;
    let placement = (
        relocations
            .first()
            .map(|row| (row.scope, row.relocated_lba, row.relocated_bytes)),
        relocations
            .get(1)
            .map(|row| (row.scope, row.relocated_lba, row.relocated_bytes)),
        relocated_32m.lba,
        relocated_32m.len,
        extent_overlaps,
    );
    let expected_placement = (
        Some((
            "PUYO1",
            BOX_P2_INFO_PAGE_STRESS_PUYO1_RELOCATED_LBA,
            BOX_P2_INFO_PAGE_STRESS_PUYO1_RELOCATED_BYTES,
        )),
        Some((
            "PUYO2",
            BOX_P2_INFO_PAGE_STRESS_PUYO2_RELOCATED_LBA,
            BOX_P2_INFO_PAGE_STRESS_PUYO2_RELOCATED_BYTES,
        )),
        BOX_P2_INFO_PAGE_STRESS_RELOCATED_32M_LBA,
        BOX_P2_INFO_PAGE_STRESS_RELOCATED_32M_BYTES,
        0,
    );
    if placement != expected_placement {
        return Err(error(format!(
            "P2 INFO stress relocation 기준선 불일치: expected={expected_placement:?}, actual={placement:?}"
        )));
    }
    let output = final_image.into_bytes();
    let image_sha256 = digest(&output);
    let decision = (
        planned_writes,
        metadata_writes,
        data_writes,
        touched_sectors,
        final_expected_writes,
        image_sha256.as_str(),
    );
    let expected = (
        BOX_P2_INFO_PAGE_STRESS_DISC_PLANNED_WRITES,
        BOX_P2_INFO_PAGE_STRESS_DISC_METADATA_WRITES,
        BOX_P2_INFO_PAGE_STRESS_DISC_DATA_WRITES,
        BOX_P2_INFO_PAGE_STRESS_DISC_TOUCHED_SECTORS,
        BOX_P2_INFO_PAGE_STRESS_DISC_FINAL_EXPECTED_WRITES,
        BOX_P2_INFO_PAGE_STRESS_DISC_IMAGE_SHA256,
    );
    if decision != expected {
        return Err(error(format!(
            "P2 INFO stress disc 결정값 불일치: expected={expected:?}, actual={decision:?}, \
             assets={assets:?}, vdds={vdds:?}, relocations={relocations:?}, \
             32m={relocated_32m:?}, extent_overlaps={extent_overlaps}"
        )));
    }
    Ok(P2InfoPageStressDiscBuild {
        image: output,
        report: P2InfoPageStressDiscReport {
            schema_version: P2_INFO_PAGE_STRESS_DISC_SCHEMA_VERSION,
            scanned_hangul: stress.scanned_hangul,
            skipped_hangul: stress.skipped_hangul,
            renderable_hangul: stress.renderable_hangul,
            distinct_hangul: stress.selected_hangul,
            assets,
            vdds,
            relocations,
            relocated_32m_lba: relocated_32m.lba,
            relocated_32m_bytes: relocated_32m.len,
            conservative_cnx_upper_bound_bytes: BOX_P2_INFO_PAGE_STRESS_CNX_UPPER_BOUND_BYTES,
            conservative_vdd_upper_bound_bytes,
            reservation_headroom_bytes,
            extent_overlaps,
            planned_writes,
            metadata_writes,
            data_writes,
            touched_sectors,
            final_expected_writes,
            image_sha256,
            product_eligible: false,
            runtime_verified: false,
        },
    })
}
