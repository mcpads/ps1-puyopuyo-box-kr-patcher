//! QUEST full-disc composer에 들어가는 Expected Write와 원본 span 검증.
//!
//! verified machine code와 byte-identical copy를 논리 파일 쓰기로 바꾸고,
//! supplemental machine site와 resident text가 원본·좌표·보호 범위·relocation
//! destination을 만족하는지 적용 전에 검증한다.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use crate::coords::{ExeMap, LogicalFile, PSX_EXE_HEADER_LEN};
use crate::isa::{verify_bytes, IsaProfile, VerifiedMachineCode};
use crate::patch::{
    plan_logical_copy, plan_logical_machine_code, ImageRegion, RegionKind, WritePlan,
};

use super::super::quest_exe_relocation_control::add_region_write;
use super::super::quest_item_acquisition_text_compositions::QuestItemAcquisitionTextCompositionStatus;
use super::super::quest_item_acquisition_text_writer::QuestItemAcquisitionTextWrite;
use super::super::quest_page_carrier_plan::QUEST_EXE_ISO_PATH;
use super::super::quest_resident_text_writer::{
    QuestResidentTextCompositionStatus, QuestResidentTextWrite,
};
use super::{
    error, QuestGlyphCacheDataPatch, QuestGlyphCacheDiscError, QuestGlyphCacheMachineCodePatch,
};

pub(super) fn align_up(value: u32, alignment: u32) -> Result<u32, QuestGlyphCacheDiscError> {
    value
        .checked_add(alignment - 1)
        .map(|sum| sum / alignment * alignment)
        .ok_or_else(|| error("QUEST sparse cache EXE text sector 올림 overflow"))
}

pub(super) fn source_slice(
    raw: &[u8],
    file_offset: u64,
    len: usize,
) -> Result<&[u8], QuestGlyphCacheDiscError> {
    let start = usize::try_from(file_offset)
        .map_err(|_| error("QUEST sparse cache file offset 변환 실패"))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| error("QUEST sparse cache source slice overflow"))?;
    raw.get(start..end)
        .ok_or_else(|| error("QUEST sparse cache source slice가 파일 범위 밖임"))
}

pub(super) fn add_copy_range(
    plan: &mut WritePlan,
    baseline: &[u8],
    source_file: LogicalFile,
    destination_file: LogicalFile,
    start: u64,
    end: u64,
    id: &str,
) -> Result<usize, QuestGlyphCacheDiscError> {
    if start >= end {
        return Ok(0);
    }
    let writes = plan_logical_copy(
        baseline,
        source_file,
        start,
        destination_file,
        start,
        end - start,
        id,
        "pipeline::quest_glyph_cache_disc",
        "OP21 site를 제외한 QUEST 원본 text의 byte-identical relocation",
        QUEST_EXE_ISO_PATH,
    )
    .ok_or_else(|| error(format!("{id}: QUEST relocation copy 계획 생성 실패")))?;
    let count = writes.len();
    for write in writes {
        add_region_write(
            plan,
            write,
            "지원 원본 QUEST text의 byte-identical ISO file relocation",
        );
    }
    Ok(count)
}

pub(super) fn add_machine_code(
    plan: &mut WritePlan,
    file: LogicalFile,
    file_offset: u64,
    expected: &[u8],
    verified: &VerifiedMachineCode,
    id: &str,
    purpose: &str,
) -> Result<usize, QuestGlyphCacheDiscError> {
    let writes = plan_logical_machine_code(
        file,
        file_offset,
        expected,
        verified,
        id,
        "pipeline::quest_glyph_cache_disc",
        purpose,
    )
    .ok_or_else(|| error(format!("{id}: typed 기계어 쓰기 계획 생성 실패")))?;
    let count = writes.len();
    for write in writes {
        plan.regions.push(ImageRegion::new(
            format!("region:{}", write.id),
            write.range(),
            RegionKind::MachineCode,
            "지원 QUEST.EXE의 sparse cache typed MIPS hook",
        ));
        plan.writes.push(write);
    }
    Ok(count)
}

#[derive(Clone, Copy)]
pub(super) struct OriginalMachineSite<'a> {
    pub(super) code: &'a VerifiedMachineCode,
    pub(super) id: &'static str,
    pub(super) purpose: &'static str,
}

pub(super) struct ValidatedOriginalMachineSite<'a> {
    pub(super) site: OriginalMachineSite<'a>,
    pub(super) file_start: u64,
    pub(super) file_end: u64,
    pub(super) text_start: usize,
    pub(super) text_end: usize,
}

pub(super) fn append_supplemental_machine_sites<'a>(
    map: &ExeMap,
    original_text: &[u8],
    sites: &mut Vec<OriginalMachineSite<'a>>,
    patches: &'a [QuestGlyphCacheMachineCodePatch<'a>],
) -> Result<(), QuestGlyphCacheDiscError> {
    let mut site_ids = sites.iter().map(|site| site.id).collect::<BTreeSet<_>>();
    if site_ids.len() != sites.len() {
        return Err(error("QUEST hook 기계어 site ID가 중복됨"));
    }
    for patch in patches {
        let runtime = patch.replacement.base_addr();
        let text_start = map.text_offset(runtime).ok_or_else(|| {
            error(format!(
                "{}: supplemental 기계어 원본 text 사상 실패",
                patch.id
            ))
        })?;
        let text_end = text_start
            .checked_add(patch.original.bytes().len())
            .ok_or_else(|| error(format!("{}: supplemental 기계어 끝 overflow", patch.id)))?;
        if !site_ids.insert(patch.id)
            || patch.original.base_addr() != runtime
            || patch.original.bytes().len() != patch.replacement.bytes().len()
            || original_text.get(text_start..text_end) != Some(patch.original.bytes())
        {
            return Err(error(format!(
                "{}: supplemental 기계어 원본/replacement 결속 불일치",
                patch.id
            )));
        }
        sites.push(OriginalMachineSite {
            code: patch.replacement,
            id: patch.id,
            purpose: patch.purpose,
        });
    }
    Ok(())
}

pub(super) fn validate_original_machine_sites<'a>(
    map: &ExeMap,
    original_text_end: u64,
    sites: &[OriginalMachineSite<'a>],
) -> Result<Vec<ValidatedOriginalMachineSite<'a>>, QuestGlyphCacheDiscError> {
    let mut validated = Vec::with_capacity(sites.len());
    for &site in sites {
        let runtime = site.code.base_addr();
        let file_start = map.file_offset(runtime).ok_or_else(|| {
            error(format!(
                "{}: QUEST 기계어 site 파일 사상 실패: 0x{runtime:08X}",
                site.id
            ))
        })?;
        let file_end = file_start
            .checked_add(site.code.bytes().len() as u64)
            .ok_or_else(|| error(format!("{}: QUEST 기계어 site 끝 overflow", site.id)))?;
        let text_start = map.text_offset(runtime).ok_or_else(|| {
            error(format!(
                "{}: QUEST 기계어 site text 사상 실패: 0x{runtime:08X}",
                site.id
            ))
        })?;
        let text_end = text_start
            .checked_add(site.code.bytes().len())
            .ok_or_else(|| error(format!("{}: QUEST 기계어 site text 끝 overflow", site.id)))?;
        if !(PSX_EXE_HEADER_LEN <= file_start
            && file_end <= original_text_end
            && file_start == PSX_EXE_HEADER_LEN + text_start as u64
            && file_end == PSX_EXE_HEADER_LEN + text_end as u64)
        {
            return Err(error(format!(
                "{}: QUEST 기계어 site가 원본 text 밖임",
                site.id
            )));
        }
        validated.push(ValidatedOriginalMachineSite {
            site,
            file_start,
            file_end,
            text_start,
            text_end,
        });
    }

    validated.sort_by_key(|site| (site.text_start, site.text_end));
    for pair in validated.windows(2) {
        if pair[0].text_end > pair[1].text_start {
            return Err(error(format!(
                "{} / {}: QUEST 기계어 site span 겹침",
                pair[0].site.id, pair[1].site.id
            )));
        }
    }
    Ok(validated)
}

pub(super) fn verify_original_machine_site_readback(
    final_text: &[u8],
    site: &ValidatedOriginalMachineSite<'_>,
) -> Result<(), QuestGlyphCacheDiscError> {
    let final_site = final_text
        .get(site.text_start..site.text_end)
        .ok_or_else(|| {
            error(format!(
                "{}: QUEST 기계어 site readback 범위 이탈",
                site.site.id
            ))
        })?;
    if final_site != site.site.code.bytes() {
        return Err(error(format!(
            "{}: QUEST 기계어 site 최종 readback 불일치",
            site.site.id
        )));
    }
    verify_bytes(IsaProfile::DECLARED, site.site.code.base_addr(), final_site)
        .map_err(|source| error(source.to_string()))?;
    Ok(())
}

pub(super) fn original_text_except_ranges_exact(
    original: &[u8],
    relocated: &[u8],
    changed: &[(usize, usize)],
) -> bool {
    original.len() == relocated.len()
        && original
            .iter()
            .zip(relocated)
            .enumerate()
            .all(|(index, (left, right))| {
                left == right
                    || changed
                        .iter()
                        .any(|(start, end)| *start <= index && index < *end)
            })
}

#[derive(Debug)]
pub(super) struct ValidatedResidentTextWrite<'a> {
    pub(super) write: &'a QuestResidentTextWrite,
    pub(super) file_start: u64,
    pub(super) file_end: u64,
    pub(super) text_start: usize,
    pub(super) text_end: usize,
}

pub(super) fn sha256_hex(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn ranges_overlap(left_start: u64, left_end: u64, right_start: u64, right_end: u64) -> bool {
    left_start < right_end && right_start < left_end
}

pub(super) fn validate_resident_text_writes<'a>(
    map: &ExeMap,
    original_text: &[u8],
    protected_text_ranges: &[(usize, usize)],
    resident_text_writes: &'a [QuestResidentTextWrite],
) -> Result<Vec<ValidatedResidentTextWrite<'a>>, QuestGlyphCacheDiscError> {
    if usize::try_from(map.text_len).ok() != Some(original_text.len())
        || protected_text_ranges.is_empty()
        || protected_text_ranges
            .iter()
            .any(|&(start, end)| start >= end || end > original_text.len())
    {
        return Err(error("QUEST resident text 원본/site 경계 drift"));
    }

    let mut unit_ids = BTreeSet::new();
    let mut record_runtimes = BTreeSet::new();
    let mut validated = Vec::with_capacity(resident_text_writes.len());
    for write in resident_text_writes {
        if !unit_ids.insert(write.unit_id.as_str()) {
            return Err(error(format!(
                "{}: QUEST resident text unit ID 중복",
                write.unit_id
            )));
        }
        if !record_runtimes.insert(write.record_runtime) {
            return Err(error(format!(
                "{}: QUEST resident text record runtime 중복",
                write.unit_id
            )));
        }
        if write.status != QuestResidentTextCompositionStatus::ManualDraftNeedsGeometry
            || !write.source_exact
            || !write.controls_exact
            || !write.terminal_exact
            || write.japanese_glyphs_remaining != 0
            || write.translation_approved
            || write.layout_approved
            || write.runtime_verified
            || write.product_eligible
        {
            return Err(error(format!(
                "{}: QUEST resident text 수동 개발 상태 drift",
                write.unit_id
            )));
        }
        if write.page_id == 0
            || write.glyph_cells == 0
            || write.source.len() < 4
            || write.source.len() != write.output.len()
            || write.source.get(..4) != Some([0x21, 0, 0, 0].as_slice())
            || write.output.get(..4) != Some([0x21, write.page_id, 0, 0].as_slice())
            || write.source.get(4..) == write.output.get(4..)
        {
            return Err(error(format!(
                "{}: QUEST resident text record/glyph span 계약 drift",
                write.unit_id
            )));
        }
        if write.source_sha256 != sha256_hex(&write.source)
            || write.output_sha256 != sha256_hex(&write.output)
        {
            return Err(error(format!(
                "{}: QUEST resident text SHA-256 불일치",
                write.unit_id
            )));
        }

        let text_span = write
            .end_text_offset
            .checked_sub(write.record_text_offset)
            .ok_or_else(|| {
                error(format!(
                    "{}: QUEST resident text offset span 역전",
                    write.unit_id
                ))
            })?;
        let runtime_span = write
            .end_runtime
            .checked_sub(write.record_runtime)
            .ok_or_else(|| {
                error(format!(
                    "{}: QUEST resident text runtime span 역전",
                    write.unit_id
                ))
            })?;
        if text_span != write.source.len()
            || usize::try_from(runtime_span).ok() != Some(text_span)
            || map.text_offset(write.record_runtime) != Some(write.record_text_offset)
            || write
                .end_runtime
                .checked_sub(1)
                .and_then(|runtime| map.text_offset(runtime))
                .and_then(|offset| offset.checked_add(1))
                != Some(write.end_text_offset)
        {
            return Err(error(format!(
                "{}: QUEST resident text runtime/text 좌표 불일치",
                write.unit_id
            )));
        }
        let source = original_text
            .get(write.record_text_offset..write.end_text_offset)
            .ok_or_else(|| {
                error(format!(
                    "{}: QUEST resident text span이 원본 text 밖임",
                    write.unit_id
                ))
            })?;
        if source != write.source {
            return Err(error(format!(
                "{}: QUEST resident text 원본 byte 불일치",
                write.unit_id
            )));
        }
        if protected_text_ranges.iter().any(|&(start, end)| {
            ranges_overlap(
                write.record_text_offset as u64,
                write.end_text_offset as u64,
                start as u64,
                end as u64,
            )
        }) {
            return Err(error(format!(
                "{}: QUEST resident text가 hook site를 침범함",
                write.unit_id
            )));
        }

        let file_start = map.file_offset(write.record_runtime).ok_or_else(|| {
            error(format!(
                "{}: QUEST resident text 파일 좌표 사상 실패",
                write.unit_id
            ))
        })?;
        let file_end = file_start
            .checked_add(write.source.len() as u64)
            .ok_or_else(|| {
                error(format!(
                    "{}: QUEST resident text 파일 끝 overflow",
                    write.unit_id
                ))
            })?;
        if file_start != PSX_EXE_HEADER_LEN + write.record_text_offset as u64
            || file_end != PSX_EXE_HEADER_LEN + write.end_text_offset as u64
        {
            return Err(error(format!(
                "{}: QUEST resident text runtime/file 좌표 불일치",
                write.unit_id
            )));
        }
        validated.push(ValidatedResidentTextWrite {
            write,
            file_start,
            file_end,
            text_start: write.record_text_offset,
            text_end: write.end_text_offset,
        });
    }

    validated.sort_by_key(|span| (span.text_start, span.text_end));
    for pair in validated.windows(2) {
        if pair[0].text_end > pair[1].text_start {
            return Err(error(format!(
                "{} / {}: QUEST resident text span 겹침",
                pair[0].write.unit_id, pair[1].write.unit_id
            )));
        }
    }
    Ok(validated)
}

pub(super) fn validate_resident_relocation_destinations(
    spans: &[ValidatedResidentTextWrite<'_>],
    relocated_quest_bytes: u32,
    reservation_raw: &[u8],
    package_range: (u64, u64),
    body_range: (u64, u64),
) -> Result<(), QuestGlyphCacheDiscError> {
    for span in spans {
        if span.file_end > u64::from(relocated_quest_bytes)
            || ranges_overlap(
                span.file_start,
                span.file_end,
                package_range.0,
                package_range.1,
            )
            || ranges_overlap(span.file_start, span.file_end, body_range.0, body_range.1)
        {
            return Err(error(format!(
                "{}: QUEST resident text가 relocation payload 경계를 침범함",
                span.write.unit_id
            )));
        }
        let expected = source_slice(reservation_raw, span.file_start, span.write.output.len())?;
        if expected.iter().any(|byte| *byte != 0) {
            return Err(error(format!(
                "{}: QUEST resident text relocation destination이 0 reservation이 아님",
                span.write.unit_id
            )));
        }
    }
    Ok(())
}

#[derive(Debug)]
pub(super) struct ValidatedItemAcquisitionTextWrite<'a> {
    pub(super) write: &'a QuestItemAcquisitionTextWrite,
    pub(super) file_start: u64,
    pub(super) file_end: u64,
    pub(super) text_start: usize,
    pub(super) text_end: usize,
}

/// 고정 NUL 문자열 쓰기가 원본 text와 기계어·resident 경계를 침범하지 않는지 검증한다.
pub(super) fn validate_item_acquisition_text_writes<'a>(
    map: &ExeMap,
    original_text: &[u8],
    protected_text_ranges: &[(usize, usize)],
    writes: &'a [QuestItemAcquisitionTextWrite],
) -> Result<Vec<ValidatedItemAcquisitionTextWrite<'a>>, QuestGlyphCacheDiscError> {
    if usize::try_from(map.text_len).ok() != Some(original_text.len())
        || protected_text_ranges
            .iter()
            .any(|&(start, end)| start >= end || end > original_text.len())
    {
        return Err(error("QUEST item acquisition text 원본/보호 경계 drift"));
    }
    let mut ids = BTreeSet::new();
    let mut runtimes = BTreeSet::new();
    let mut validated = Vec::with_capacity(writes.len());
    for write in writes {
        if !ids.insert(write.unit_id.as_str())
            || !runtimes.insert(write.runtime)
            || write.page_id == 0
            || write.status
                != QuestItemAcquisitionTextCompositionStatus::ManualDraftNeedsRuntimeGeometry
            || write.source.is_empty()
            || write.source.len() != write.output.len()
            || write.source == write.output
            || write.source.last() != Some(&0)
            || write.source_sha256 != sha256_hex(&write.source)
            || write.output_sha256 != sha256_hex(&write.output)
        {
            return Err(error(format!(
                "{}: QUEST item acquisition text write 계약 drift",
                write.unit_id
            )));
        }
        let terminal_offset = write.glyph_cells * 2;
        if write.output.get(terminal_offset) != Some(&0)
            || write.output[terminal_offset..]
                .iter()
                .any(|byte| *byte != 0)
        {
            return Err(error(format!(
                "{}: QUEST item acquisition text NUL/padding 불일치",
                write.unit_id
            )));
        }
        let text_start = map.text_offset(write.runtime).ok_or_else(|| {
            error(format!(
                "{}: QUEST item acquisition text runtime 사상 실패",
                write.unit_id
            ))
        })?;
        let text_end = text_start
            .checked_add(write.source.len())
            .ok_or_else(|| error(format!("{}: text span overflow", write.unit_id)))?;
        if text_start != write.text_offset
            || original_text.get(text_start..text_end) != Some(write.source.as_slice())
            || protected_text_ranges.iter().any(|&(start, end)| {
                ranges_overlap(text_start as u64, text_end as u64, start as u64, end as u64)
            })
        {
            return Err(error(format!(
                "{}: QUEST item acquisition text 원본 byte/보호 경계 불일치",
                write.unit_id
            )));
        }
        let file_start = map.file_offset(write.runtime).ok_or_else(|| {
            error(format!(
                "{}: QUEST item acquisition text file 사상 실패",
                write.unit_id
            ))
        })?;
        let file_end = file_start
            .checked_add(write.source.len() as u64)
            .ok_or_else(|| error(format!("{}: file span overflow", write.unit_id)))?;
        if file_start != PSX_EXE_HEADER_LEN + text_start as u64
            || file_end != PSX_EXE_HEADER_LEN + text_end as u64
        {
            return Err(error(format!(
                "{}: QUEST item acquisition text runtime/file 좌표 불일치",
                write.unit_id
            )));
        }
        validated.push(ValidatedItemAcquisitionTextWrite {
            write,
            file_start,
            file_end,
            text_start,
            text_end,
        });
    }
    validated.sort_by_key(|span| (span.text_start, span.text_end));
    for pair in validated.windows(2) {
        if pair[0].text_end > pair[1].text_start {
            return Err(error(format!(
                "{} / {}: QUEST item acquisition text span 겹침",
                pair[0].write.unit_id, pair[1].write.unit_id
            )));
        }
    }
    Ok(validated)
}

pub(super) fn validate_item_acquisition_relocation_destinations(
    spans: &[ValidatedItemAcquisitionTextWrite<'_>],
    relocated_quest_bytes: u32,
    reservation_raw: &[u8],
    package_range: (u64, u64),
    body_range: (u64, u64),
) -> Result<(), QuestGlyphCacheDiscError> {
    for span in spans {
        if span.file_end > u64::from(relocated_quest_bytes)
            || ranges_overlap(
                span.file_start,
                span.file_end,
                package_range.0,
                package_range.1,
            )
            || ranges_overlap(span.file_start, span.file_end, body_range.0, body_range.1)
        {
            return Err(error(format!(
                "{}: QUEST item acquisition text가 relocation payload 경계를 침범함",
                span.write.unit_id
            )));
        }
        let expected = source_slice(reservation_raw, span.file_start, span.write.output.len())?;
        if expected.iter().any(|byte| *byte != 0) {
            return Err(error(format!(
                "{}: QUEST item acquisition text relocation destination이 0 reservation이 아님",
                span.write.unit_id
            )));
        }
    }
    Ok(())
}

#[derive(Debug)]
pub(super) struct ValidatedDataPatch<'a> {
    pub(super) patch: &'a QuestGlyphCacheDataPatch<'a>,
    pub(super) file_start: u64,
    pub(super) file_end: u64,
    pub(super) text_start: usize,
    pub(super) text_end: usize,
}

/// source-bound supplemental data가 원본 text와 선언된 변경 범위를 침범하지 않는지 검증한다.
pub(super) fn validate_data_patches<'a>(
    map: &ExeMap,
    original_text: &[u8],
    protected_text_ranges: &[(usize, usize)],
    patches: &'a [QuestGlyphCacheDataPatch<'a>],
) -> Result<Vec<ValidatedDataPatch<'a>>, QuestGlyphCacheDiscError> {
    if usize::try_from(map.text_len).ok() != Some(original_text.len())
        || protected_text_ranges
            .iter()
            .any(|&(start, end)| start >= end || end > original_text.len())
    {
        return Err(error("QUEST supplemental data 원본/보호 경계 drift"));
    }
    let mut ids = BTreeSet::new();
    let mut runtimes = BTreeSet::new();
    let mut validated = Vec::with_capacity(patches.len());
    for patch in patches {
        if !ids.insert(patch.id)
            || !runtimes.insert(patch.runtime)
            || patch.id.is_empty()
            || patch.purpose.is_empty()
            || patch.original.is_empty()
            || patch.original.len() != patch.replacement.len()
            || patch.original == patch.replacement
            || patch.original_sha256 != sha256_hex(patch.original)
            || patch.replacement_sha256 != sha256_hex(patch.replacement)
            || patch.required_page_id == Some(0)
        {
            return Err(error(format!(
                "{}: QUEST supplemental data patch 계약 drift",
                patch.id
            )));
        }
        let text_start = map.text_offset(patch.runtime).ok_or_else(|| {
            error(format!(
                "{}: QUEST supplemental data runtime 사상 실패",
                patch.id
            ))
        })?;
        let text_end = text_start
            .checked_add(patch.original.len())
            .ok_or_else(|| error(format!("{}: data text span overflow", patch.id)))?;
        if original_text.get(text_start..text_end) != Some(patch.original)
            || protected_text_ranges.iter().any(|&(start, end)| {
                ranges_overlap(text_start as u64, text_end as u64, start as u64, end as u64)
            })
        {
            return Err(error(format!(
                "{}: QUEST supplemental data 원본 byte/보호 경계 불일치",
                patch.id
            )));
        }
        let file_start = map.file_offset(patch.runtime).ok_or_else(|| {
            error(format!(
                "{}: QUEST supplemental data file 사상 실패",
                patch.id
            ))
        })?;
        let file_end = file_start
            .checked_add(patch.original.len() as u64)
            .ok_or_else(|| error(format!("{}: data file span overflow", patch.id)))?;
        if file_start != PSX_EXE_HEADER_LEN + text_start as u64
            || file_end != PSX_EXE_HEADER_LEN + text_end as u64
        {
            return Err(error(format!(
                "{}: QUEST supplemental data runtime/file 좌표 불일치",
                patch.id
            )));
        }
        validated.push(ValidatedDataPatch {
            patch,
            file_start,
            file_end,
            text_start,
            text_end,
        });
    }
    validated.sort_by_key(|span| (span.text_start, span.text_end));
    for pair in validated.windows(2) {
        if pair[0].text_end > pair[1].text_start {
            return Err(error(format!(
                "{} / {}: QUEST supplemental data span 겹침",
                pair[0].patch.id, pair[1].patch.id
            )));
        }
    }
    Ok(validated)
}

pub(super) fn validate_data_patch_relocation_destinations(
    spans: &[ValidatedDataPatch<'_>],
    relocated_quest_bytes: u32,
    reservation_raw: &[u8],
    package_range: (u64, u64),
    body_range: (u64, u64),
) -> Result<(), QuestGlyphCacheDiscError> {
    for span in spans {
        if span.file_end > u64::from(relocated_quest_bytes)
            || ranges_overlap(
                span.file_start,
                span.file_end,
                package_range.0,
                package_range.1,
            )
            || ranges_overlap(span.file_start, span.file_end, body_range.0, body_range.1)
        {
            return Err(error(format!(
                "{}: QUEST supplemental data가 relocation payload 경계를 침범함",
                span.patch.id
            )));
        }
        let expected = source_slice(
            reservation_raw,
            span.file_start,
            span.patch.replacement.len(),
        )?;
        if expected.iter().any(|byte| *byte != 0) {
            return Err(error(format!(
                "{}: QUEST supplemental data relocation destination이 0 reservation이 아님",
                span.patch.id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "write_validation_tests.rs"]
mod tests;
