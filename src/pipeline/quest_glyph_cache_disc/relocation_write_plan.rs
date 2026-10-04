//! QUEST glyph cache disc의 relocation 배치와 Expected Write 계획.
//!
//! 불변 원본에서 QUEST.EXE, 선택 자산과 32M reservation의 최종 extent를
//! 정하고 모든 code/data/metadata 변경을 하나의 [`WritePlan`]로 만든다.
//! 계획을 적용하거나 최종 이미지를 판정하는 일은 이 module이 맡지 않는다.

use std::collections::BTreeSet;

use crate::coords::{exe_map, LogicalFile, PSX_EXE_HEADER_LEN};
use crate::disc::{
    add_single_extent_record_update, partition_extent_with_remainder, verify_box_32m_reservation,
    Extent, Iso9660, PsxExe, VerifiedSourceImage, BOX_32M_PATH,
};
use crate::patch::{RegionKind, UnchangedBoundary, WritePlan};
use crate::source::{FORM1_DATA_LEN, SECTOR_LEN};

use super::super::disc_archive_patch::add_disc_archive_patch_writes;
use super::super::quest_glyph_cache_hook_plan::verify_quest_glyph_cache_installation;
use super::super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::super::quest_page_carrier_plan::QUEST_EXE_ISO_PATH;
use super::super::vdd_plan::{add_logical_span, logical_file};
use super::write_validation::{
    add_copy_range, add_machine_code, align_up, append_supplemental_machine_sites, source_slice,
    validate_data_patch_relocation_destinations, validate_data_patches,
    validate_item_acquisition_relocation_destinations, validate_item_acquisition_text_writes,
    validate_original_machine_sites, validate_resident_relocation_destinations,
    validate_resident_text_writes, OriginalMachineSite, ValidatedDataPatch,
    ValidatedItemAcquisitionTextWrite, ValidatedOriginalMachineSite, ValidatedResidentTextWrite,
};
use super::{
    error, QuestGlyphCacheDiscError, QuestGlyphCacheDiscSupplementalWrites, QuestGlyphCacheHookPlan,
};
use crate::pipeline::disc_archive_patch::DiscRelocatedFilePatch;

pub(super) struct QuestGlyphCacheRelocatedFilePlacement<'a> {
    pub(super) patch: &'a DiscRelocatedFilePatch<'a>,
    pub(super) extent: Extent,
}

/// 적용 전 검증을 마친 단일 QUEST disc Expected Write 계획과 readback 입력.
pub(super) struct QuestGlyphCacheDiscWritePlan<'a> {
    pub(super) writes: WritePlan,
    pub(super) original_quest_raw: Vec<u8>,
    pub(super) relocated_text_bytes: u32,
    pub(super) relocated_quest: Extent,
    pub(super) relocated_quest_sectors: u32,
    pub(super) relocated_32m: Extent,
    pub(super) relocated_files: Vec<QuestGlyphCacheRelocatedFilePlacement<'a>>,
    pub(super) relocated_file_bytes: usize,
    pub(super) relocated_file_sectors: u32,
    pub(super) original_extent_start: u64,
    pub(super) original_extent_end: u64,
    pub(super) protected_machine_text_ranges: Vec<(usize, usize)>,
    pub(super) machine_sites: Vec<ValidatedOriginalMachineSite<'a>>,
    pub(super) resident_text_writes: Vec<ValidatedResidentTextWrite<'a>>,
    pub(super) resident_text_bytes: usize,
    pub(super) item_acquisition_text_writes: Vec<ValidatedItemAcquisitionTextWrite<'a>>,
    pub(super) item_acquisition_text_bytes: usize,
    pub(super) data_patches: Vec<ValidatedDataPatch<'a>>,
    pub(super) data_patch_bytes: usize,
    pub(super) machine_code_writes: usize,
    pub(super) data_writes: usize,
    pub(super) metadata_writes: usize,
    pub(super) relocation_copy_writes: usize,
    pub(super) touched_sectors: usize,
    pub(super) final_expected_writes: usize,
}

/// QUEST relocation의 전체 쓰기 집합을 불변 원본 기준으로 계획한다.
pub(super) fn plan_quest_glyph_cache_disc_relocation<'a>(
    source: &VerifiedSourceImage,
    package: &QuestGlyphCachePackage,
    hook: &'a QuestGlyphCacheHookPlan,
    supplemental_writes: QuestGlyphCacheDiscSupplementalWrites<'a>,
) -> Result<QuestGlyphCacheDiscWritePlan<'a>, QuestGlyphCacheDiscError> {
    let QuestGlyphCacheDiscSupplementalWrites {
        subgame_help,
        classic_scene_spacing,
        archive_patches,
        archive_repacks,
        relocated_files,
        resident_text_writes,
        item_acquisition_text_writes,
        data_patches,
        machine_code_patches,
    } = supplemental_writes;
    let installation = verify_quest_glyph_cache_installation(package, hook)
        .map_err(|source| error(source.to_string()))?;
    let body_end_runtime = installation.body_end_runtime;

    let map = exe_map("QUEST.EXE").ok_or_else(|| error("QUEST.EXE 좌표 사상이 없음"))?;
    let required_text_bytes = body_end_runtime
        .checked_sub(map.load_addr)
        .ok_or_else(|| error("QUEST sparse cache hook이 load 주소보다 앞섬"))?;
    let relocated_text_bytes = align_up(required_text_bytes, FORM1_DATA_LEN as u32)?;
    let relocated_quest_bytes = relocated_text_bytes
        .checked_add(
            u32::try_from(PSX_EXE_HEADER_LEN)
                .map_err(|_| error("PS-X EXE header 길이 변환 실패"))?,
        )
        .ok_or_else(|| error("QUEST sparse cache EXE 파일 길이 overflow"))?;
    let relocated_quest_sectors = relocated_quest_bytes.div_ceil(FORM1_DATA_LEN as u32);
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let quest_entry = iso
        .find(QUEST_EXE_ISO_PATH)
        .map_err(|source| error(source.to_string()))?;
    let reservation_entry = iso
        .find(BOX_32M_PATH)
        .map_err(|source| error(source.to_string()))?;
    let original_quest_file = logical_file(quest_entry).map_err(error)?;
    let original_quest_raw = iso
        .read_file(QUEST_EXE_ISO_PATH)
        .map_err(|source| error(source.to_string()))?;
    let original_exe =
        PsxExe::parse(&original_quest_raw).map_err(|source| error(source.to_string()))?;
    original_exe
        .verify_map(map)
        .map_err(|source| error(source.to_string()))?;
    let reservation =
        verify_box_32m_reservation(&iso, source.image(), [("QUEST.EXE", original_exe.text())])
            .map_err(|source| error(source.to_string()))?;
    if reservation_entry.len() != reservation.bytes {
        return Err(error("32M reservation entry와 검증 요약 길이 불일치"));
    }

    let reservation_extent = Extent {
        lba: reservation.lba,
        len: u32::try_from(reservation.bytes)
            .map_err(|_| error("32M reservation 길이 변환 실패"))?,
    };
    let mut relocated_paths = BTreeSet::new();
    let mut allocation_lengths = Vec::with_capacity(relocated_files.len() + 1);
    allocation_lengths.push(relocated_quest_bytes);
    let mut relocated_file_bytes = 0usize;
    let mut relocated_file_sectors = 0u32;
    for file_patch in relocated_files {
        if file_patch.iso_path == QUEST_EXE_ISO_PATH
            || file_patch.iso_path == BOX_32M_PATH
            || !relocated_paths.insert(file_patch.iso_path)
        {
            return Err(error(format!(
                "{}: relocation ISO 경로가 중복되거나 예약 경계임",
                file_patch.id
            )));
        }
        let bytes = u32::try_from(file_patch.replacement.len()).map_err(|_| {
            error(format!(
                "{}: relocation 파일 길이가 u32를 넘음",
                file_patch.id
            ))
        })?;
        let sectors = bytes.div_ceil(FORM1_DATA_LEN as u32);
        allocation_lengths.push(bytes);
        relocated_file_bytes = relocated_file_bytes
            .checked_add(file_patch.replacement.len())
            .ok_or_else(|| error("QUEST relocation 파일 길이 합계 overflow"))?;
        relocated_file_sectors = relocated_file_sectors
            .checked_add(sectors)
            .ok_or_else(|| error("QUEST relocation 파일 sector 합계 overflow"))?;
    }
    let partition = partition_extent_with_remainder(reservation_extent, &allocation_lengths)
        .map_err(|source| error(source.to_string()))?;
    if partition.allocations().len() != relocated_files.len() + 1 {
        return Err(error("QUEST relocation extent 분모 불일치"));
    }
    let relocated_quest = partition.allocations()[0];
    let relocated_file_layouts = relocated_files
        .iter()
        .zip(&partition.allocations()[1..])
        .map(|(patch, &extent)| QuestGlyphCacheRelocatedFilePlacement { patch, extent })
        .collect::<Vec<_>>();
    let relocated_32m = partition.remainder();
    if relocated_quest_sectors
        + relocated_file_sectors
        + relocated_32m.len.div_ceil(FORM1_DATA_LEN as u32)
        != reservation.form1_sectors
    {
        return Err(error("QUEST sparse cache EXE/32M sector 합 불일치"));
    }

    let destination_file = LogicalFile::new(relocated_quest.lba, relocated_quest.len);
    let reservation_raw = iso
        .read_file(BOX_32M_PATH)
        .map_err(|source| error(source.to_string()))?;
    let header_len =
        usize::try_from(PSX_EXE_HEADER_LEN).map_err(|_| error("PS-X EXE header 길이 변환 실패"))?;
    let mut relocated_header = original_quest_raw[..header_len].to_vec();
    relocated_header[0x1C..0x20].copy_from_slice(&relocated_text_bytes.to_le_bytes());

    let mut plan = WritePlan::new();
    add_logical_span(
        &mut plan,
        destination_file,
        0,
        &reservation_raw[..header_len],
        &relocated_header,
        "quest-glyph-cache-header",
        "pipeline::quest_glyph_cache_disc",
        "sparse package와 hook을 포함하도록 확장한 PS-X EXE header",
        crate::patch::WriteIntent::Metadata,
    )
    .map_err(error)?;

    let original_text_end = PSX_EXE_HEADER_LEN
        .checked_add(u64::from(map.text_len))
        .ok_or_else(|| error("QUEST 원본 text 파일 끝 overflow"))?;
    let mut original_machine_sites = installation
        .machine_sites
        .iter()
        .map(|site| OriginalMachineSite {
            code: site.code,
            id: site.id,
            purpose: site.purpose,
        })
        .collect::<Vec<_>>();
    append_supplemental_machine_sites(
        map,
        original_exe.text(),
        &mut original_machine_sites,
        machine_code_patches,
    )?;
    let validated_machine_sites =
        validate_original_machine_sites(map, original_text_end, &original_machine_sites)?;
    let protected_machine_text_ranges = validated_machine_sites
        .iter()
        .map(|site| (site.text_start, site.text_end))
        .collect::<Vec<_>>();
    let validated_resident_text_writes = validate_resident_text_writes(
        map,
        original_exe.text(),
        &protected_machine_text_ranges,
        resident_text_writes,
    )?;
    if let Some(span) = validated_resident_text_writes
        .iter()
        .find(|span| usize::from(span.write.page_id) > package.plan.pages)
    {
        return Err(error(format!(
            "{}: QUEST resident text page가 package 범위 밖임: page={}, pages={}",
            span.write.unit_id, span.write.page_id, package.plan.pages
        )));
    }
    let resident_text_bytes =
        validated_resident_text_writes
            .iter()
            .try_fold(0usize, |total, span| {
                total
                    .checked_add(span.write.output.len())
                    .ok_or_else(|| error("QUEST resident text byte 합계 overflow"))
            })?;
    let mut protected_item_acquisition_ranges = protected_machine_text_ranges.clone();
    protected_item_acquisition_ranges.extend(
        validated_resident_text_writes
            .iter()
            .map(|span| (span.text_start, span.text_end)),
    );
    let validated_item_acquisition_text_writes = validate_item_acquisition_text_writes(
        map,
        original_exe.text(),
        &protected_item_acquisition_ranges,
        item_acquisition_text_writes,
    )?;
    if let Some(span) = validated_item_acquisition_text_writes
        .iter()
        .find(|span| usize::from(span.write.page_id) > package.plan.pages)
    {
        return Err(error(format!(
            "{}: QUEST item acquisition text page가 package 범위 밖임: page={}, pages={}",
            span.write.unit_id, span.write.page_id, package.plan.pages
        )));
    }
    let item_acquisition_text_bytes =
        validated_item_acquisition_text_writes
            .iter()
            .try_fold(0usize, |total, span| {
                total
                    .checked_add(span.write.output.len())
                    .ok_or_else(|| error("QUEST item acquisition text byte 합계 overflow"))
            })?;
    let mut protected_data_ranges = protected_item_acquisition_ranges;
    protected_data_ranges.extend(
        validated_item_acquisition_text_writes
            .iter()
            .map(|span| (span.text_start, span.text_end)),
    );
    let validated_data_patches = validate_data_patches(
        map,
        original_exe.text(),
        &protected_data_ranges,
        data_patches,
    )?;
    if let Some(span) = validated_data_patches.iter().find(|span| {
        span.patch
            .required_page_id
            .is_some_and(|page_id| usize::from(page_id) > package.plan.pages)
    }) {
        return Err(error(format!(
            "{}: QUEST supplemental data page가 package 범위 밖임",
            span.patch.id
        )));
    }
    let data_patch_bytes = validated_data_patches
        .iter()
        .try_fold(0usize, |total, span| {
            total
                .checked_add(span.patch.replacement.len())
                .ok_or_else(|| error("QUEST supplemental data byte 합계 overflow"))
        })?;

    let package_file_offset = map
        .loaded_file_offset(
            crate::disasm::QUEST_PAGE_CARRIER_START_RUNTIME,
            relocated_text_bytes,
        )
        .ok_or_else(|| error("QUEST sparse cache package 확장 파일 사상 실패"))?;
    let package_file_end = package_file_offset
        .checked_add(package.bytes.len() as u64)
        .ok_or_else(|| error("QUEST sparse cache package 파일 끝 overflow"))?;
    let body_file_offset = map
        .loaded_file_offset(hook.body_runtime, relocated_text_bytes)
        .ok_or_else(|| error("QUEST sparse cache body 확장 파일 사상 실패"))?;
    let body_file_end = body_file_offset
        .checked_add(hook.body.bytes().len() as u64)
        .ok_or_else(|| error("QUEST sparse cache body 파일 끝 overflow"))?;
    if package_file_offset < original_text_end
        || package_file_end != body_file_offset
        || body_file_end > u64::from(relocated_quest.len)
    {
        return Err(error("QUEST sparse cache relocation payload 경계 drift"));
    }
    validate_resident_relocation_destinations(
        &validated_resident_text_writes,
        relocated_quest.len,
        &reservation_raw,
        (package_file_offset, package_file_end),
        (body_file_offset, body_file_end),
    )?;
    validate_item_acquisition_relocation_destinations(
        &validated_item_acquisition_text_writes,
        relocated_quest.len,
        &reservation_raw,
        (package_file_offset, package_file_end),
        (body_file_offset, body_file_end),
    )?;
    validate_data_patch_relocation_destinations(
        &validated_data_patches,
        relocated_quest.len,
        &reservation_raw,
        (package_file_offset, package_file_end),
        (body_file_offset, body_file_end),
    )?;

    let mut copy_exclusions = Vec::with_capacity(
        validated_resident_text_writes.len()
            + validated_item_acquisition_text_writes.len()
            + validated_data_patches.len()
            + validated_machine_sites.len(),
    );
    copy_exclusions.extend(
        validated_machine_sites
            .iter()
            .map(|site| (site.file_start, site.file_end)),
    );
    copy_exclusions.extend(
        validated_resident_text_writes
            .iter()
            .map(|span| (span.file_start, span.file_end)),
    );
    copy_exclusions.extend(
        validated_item_acquisition_text_writes
            .iter()
            .map(|span| (span.file_start, span.file_end)),
    );
    copy_exclusions.extend(
        validated_data_patches
            .iter()
            .map(|span| (span.file_start, span.file_end)),
    );
    copy_exclusions.sort_unstable();
    let mut relocation_copy_writes = 0usize;
    let mut copy_start = PSX_EXE_HEADER_LEN;
    for (index, (excluded_start, excluded_end)) in copy_exclusions.iter().copied().enumerate() {
        if excluded_start < copy_start || excluded_end > original_text_end {
            return Err(error(
                "QUEST relocation 제외 span이 겹치거나 원본 text 밖임",
            ));
        }
        relocation_copy_writes += add_copy_range(
            &mut plan,
            source.image().raw_bytes(),
            original_quest_file,
            destination_file,
            copy_start,
            excluded_start,
            &format!("quest-glyph-cache-copy-{index}"),
        )?;
        copy_start = excluded_end;
    }
    relocation_copy_writes += add_copy_range(
        &mut plan,
        source.image().raw_bytes(),
        original_quest_file,
        destination_file,
        copy_start,
        original_text_end,
        &format!("quest-glyph-cache-copy-{}", copy_exclusions.len()),
    )?;

    for (index, span) in validated_resident_text_writes.iter().enumerate() {
        add_logical_span(
            &mut plan,
            destination_file,
            usize::try_from(span.file_start).map_err(|_| {
                error(format!(
                    "{}: QUEST resident text destination offset 변환 실패",
                    span.write.unit_id
                ))
            })?,
            source_slice(&reservation_raw, span.file_start, span.write.output.len())?,
            &span.write.output,
            &format!("quest-resident-text-{index}"),
            "pipeline::quest_glyph_cache_disc",
            "검증된 QUEST resident OP21 record와 glyph pair를 재배치 EXE에 삽입",
            crate::patch::WriteIntent::Data,
        )
        .map_err(error)?;
    }

    for (index, span) in validated_item_acquisition_text_writes.iter().enumerate() {
        add_logical_span(
            &mut plan,
            destination_file,
            usize::try_from(span.file_start).map_err(|_| {
                error(format!(
                    "{}: QUEST item acquisition text destination offset 변환 실패",
                    span.write.unit_id
                ))
            })?,
            source_slice(&reservation_raw, span.file_start, span.write.output.len())?,
            &span.write.output,
            &format!("quest-item-acquisition-text-{index}"),
            "pipeline::quest_glyph_cache_disc",
            "검증된 QUEST item acquisition 고정 문자열을 재배치 EXE에 삽입",
            crate::patch::WriteIntent::Data,
        )
        .map_err(error)?;
    }

    for span in &validated_data_patches {
        add_logical_span(
            &mut plan,
            destination_file,
            usize::try_from(span.file_start).map_err(|_| {
                error(format!(
                    "{}: QUEST supplemental data destination offset 변환 실패",
                    span.patch.id
                ))
            })?,
            source_slice(
                &reservation_raw,
                span.file_start,
                span.patch.replacement.len(),
            )?,
            span.patch.replacement,
            span.patch.id,
            "pipeline::quest_glyph_cache_disc",
            span.patch.purpose,
            crate::patch::WriteIntent::Data,
        )
        .map_err(error)?;
    }

    let mut machine_code_writes = 0usize;
    for site in &validated_machine_sites {
        machine_code_writes += add_machine_code(
            &mut plan,
            destination_file,
            site.file_start,
            source_slice(
                &reservation_raw,
                site.file_start,
                site.site.code.bytes().len(),
            )?,
            site.site.code,
            site.site.id,
            site.site.purpose,
        )?;
    }

    add_logical_span(
        &mut plan,
        destination_file,
        usize::try_from(package_file_offset)
            .map_err(|_| error("QUEST sparse cache package offset 변환 실패"))?,
        source_slice(&reservation_raw, package_file_offset, package.bytes.len())?,
        &package.bytes,
        "quest-glyph-cache-package",
        "pipeline::quest_glyph_cache_disc",
        "재배치 QUEST.EXE에 검산된 sparse glyph cache package 삽입",
        crate::patch::WriteIntent::Data,
    )
    .map_err(error)?;

    machine_code_writes += add_machine_code(
        &mut plan,
        destination_file,
        body_file_offset,
        source_slice(&reservation_raw, body_file_offset, hook.body.bytes().len())?,
        &hook.body,
        "quest-glyph-cache-body",
        "route, page, live object scan, decode, upload, object atlas 선택을 수행하는 sparse cache body",
    )?;

    add_disc_archive_patch_writes(&iso, &mut plan, archive_patches, archive_repacks)
        .map_err(|source| error(source.to_string()))?;

    for file_placement in &relocated_file_layouts {
        let file_patch = file_placement.patch;
        let extent = file_placement.extent;
        let entry = iso
            .find(file_patch.iso_path)
            .map_err(|source| error(source.to_string()))?;
        let original = iso
            .read_file(file_patch.iso_path)
            .map_err(|source| error(source.to_string()))?;
        if original != file_patch.original {
            return Err(error(format!(
                "{}: relocation 파일 원본 불일치",
                file_patch.id
            )));
        }
        let destination = LogicalFile::new(extent.lba, extent.len);
        let reservation_offset =
            usize::try_from(u64::from(extent.lba - reservation.lba) * FORM1_DATA_LEN as u64)
                .map_err(|_| error(format!("{}: reservation offset 변환 실패", file_patch.id)))?;
        let reservation_end = reservation_offset
            .checked_add(file_patch.replacement.len())
            .ok_or_else(|| error(format!("{}: reservation 범위 overflow", file_patch.id)))?;
        let expected = reservation_raw
            .get(reservation_offset..reservation_end)
            .ok_or_else(|| error(format!("{}: reservation 범위 이탈", file_patch.id)))?;
        add_logical_span(
            &mut plan,
            destination,
            0,
            expected,
            file_patch.replacement,
            file_patch.id,
            file_patch.actor,
            "확장된 QUEST 자산을 검증된 32M reservation으로 재배치",
            crate::patch::WriteIntent::Data,
        )
        .map_err(error)?;

        let [original_extent] = entry.extents() else {
            return Err(error(format!(
                "{}: 원본 ISO 파일이 단일 extent가 아님",
                file_patch.id
            )));
        };
        let original_start = u64::from(original_extent.lba) * SECTOR_LEN as u64;
        let original_end = original_start
            .checked_add(
                u64::from(original_extent.len.div_ceil(FORM1_DATA_LEN as u32)) * SECTOR_LEN as u64,
            )
            .ok_or_else(|| error(format!("{}: 원본 extent 경계 overflow", file_patch.id)))?;
        plan.unchanged.push(UnchangedBoundary::new(
            format!("{}:original-extent", file_patch.id),
            original_start..original_end,
            "재배치 뒤 원본 ISO 파일 raw extent byte-identical 보존",
        ));
    }

    for (entry, replacement, id, purpose) in [
        (
            quest_entry,
            relocated_quest,
            "quest-glyph-cache-directory-record",
            "QUEST.EXE를 sparse cache와 함께 32M reservation으로 재배치",
        ),
        (
            reservation_entry,
            relocated_32m,
            "quest-glyph-cache-32m-record",
            "sparse cache QUEST.EXE 뒤에서 시작하도록 32M.BIN 축소",
        ),
    ] {
        add_single_extent_record_update(
            &mut plan,
            source.image(),
            entry,
            replacement,
            id,
            "pipeline::quest_glyph_cache_disc",
            purpose,
        )
        .map_err(|source| error(source.to_string()))?;
    }
    for file_placement in &relocated_file_layouts {
        let file_patch = file_placement.patch;
        let entry = iso
            .find(file_patch.iso_path)
            .map_err(|source| error(source.to_string()))?;
        add_single_extent_record_update(
            &mut plan,
            source.image(),
            entry,
            file_placement.extent,
            &format!("{}:directory-record", file_patch.id),
            file_patch.actor,
            "확장된 QUEST 자산의 relocation LBA와 길이 갱신",
        )
        .map_err(|source| error(source.to_string()))?;
    }

    let original_extent_start = u64::from(original_quest_file.lba) * SECTOR_LEN as u64;
    let original_extent_end = original_extent_start
        .checked_add(u64::from(original_quest_file.sector_count()) * SECTOR_LEN as u64)
        .ok_or_else(|| error("QUEST 원본 raw extent 경계 overflow"))?;
    plan.unchanged.push(UnchangedBoundary::new(
        "quest-glyph-cache-original-extent",
        original_extent_start..original_extent_end,
        "재배치 뒤에도 원본 QUEST raw extent는 byte-identical 보존",
    ));

    if let Some(help) = subgame_help {
        help.add_writes(&iso, &mut plan).map_err(error)?;
    }

    if let Some(spacing) = classic_scene_spacing {
        machine_code_writes += spacing.add_writes(&iso, &mut plan).map_err(error)?;
    }

    let data_writes = plan
        .writes
        .iter()
        .filter(|write| write.intent.region_kind() == RegionKind::Data)
        .count();
    let metadata_writes = plan
        .writes
        .iter()
        .filter(|write| write.intent.region_kind() == RegionKind::Metadata)
        .count();
    let touched_sectors = plan.touched_sectors().len();
    let final_expected_writes = plan.writes.len() + touched_sectors;

    Ok(QuestGlyphCacheDiscWritePlan {
        writes: plan,
        original_quest_raw,
        relocated_text_bytes,
        relocated_quest,
        relocated_quest_sectors,
        relocated_32m,
        relocated_files: relocated_file_layouts,
        relocated_file_bytes,
        relocated_file_sectors,
        original_extent_start,
        original_extent_end,
        protected_machine_text_ranges,
        machine_sites: validated_machine_sites,
        resident_text_writes: validated_resident_text_writes,
        resident_text_bytes,
        item_acquisition_text_writes: validated_item_acquisition_text_writes,
        item_acquisition_text_bytes,
        data_patches: validated_data_patches,
        data_patch_bytes,
        machine_code_writes,
        data_writes,
        metadata_writes,
        relocation_copy_writes,
        touched_sectors,
        final_expected_writes,
    })
}
