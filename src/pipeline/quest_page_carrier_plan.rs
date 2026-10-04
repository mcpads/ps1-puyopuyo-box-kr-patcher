//! QUEST page CNX resident carrier의 EXE/ISO 비용 판정.
//!
//! post-BSS 후보에 최대 batch page CNX를 둔다고 가정해 PS-X EXE text를
//! 2,048바이트 단위로 얼마나 늘려야 하는지 계산한다. 원본 QUEST extent
//! 바로 뒤가 다른 파일이므로 제자리 성장은 승인하지 않는다. `32M.BIN`
//! 안에 재배치할 물리 용량이 있다는 사실도 ISO record 갱신과 runtime
//! 소유권을 대신하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::{ExeMap, PSX_EXE_HEADER_LEN};
use crate::disasm::{
    verify_box_quest_page_carrier, QuestPageCarrierSummary, QUEST_PAGE_CARRIER_START_RUNTIME,
};
use crate::disc::{Box32mSummary, Iso9660};
use crate::source::FORM1_DATA_LEN;

use super::QuestFontAlternativesAnalysis;

pub const QUEST_PAGE_CARRIER_PLAN_SCHEMA_VERSION: &str = "quest-page-carrier-plan-v1";
pub const QUEST_EXE_ISO_PATH: &str = "/QUEST/QUEST.EXE;1";
pub const QUEST_RELOCATION_CONTROL_TEXT_BYTES: usize = 0x000F_B800;
pub const QUEST_RELOCATION_CONTROL_FILE_BYTES: usize = 1_032_192;
pub const QUEST_RELOCATION_CONTROL_SECTORS: u32 = 504;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestPageCarrierPlan {
    pub schema_version: &'static str,
    pub carrier: QuestPageCarrierSummary,
    pub page_cnx_min_bytes: usize,
    pub page_cnx_max_bytes: usize,
    pub original_file_bytes: usize,
    pub original_sectors: u32,
    pub required_text_bytes: usize,
    pub required_file_bytes: usize,
    pub required_sectors: u32,
    pub growth_bytes: usize,
    pub growth_sectors: u32,
    pub original_end_lba: u32,
    pub next_extent_lba: u32,
    pub in_place_growth_possible: bool,
    pub volume_tail_sectors: u32,
    pub reservation_sectors: u32,
    pub reservation_fit: bool,
    pub requires_iso_relocation: bool,
    pub runtime_ownership_proven: bool,
    pub iso_relocation_implemented: bool,
    pub build_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPageCarrierPlanError(String);

impl fmt::Display for QuestPageCarrierPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestPageCarrierPlanError {}

fn error(message: impl Into<String>) -> QuestPageCarrierPlanError {
    QuestPageCarrierPlanError(message.into())
}

fn align_up(value: usize, alignment: usize) -> Result<usize, QuestPageCarrierPlanError> {
    value
        .checked_add(alignment - 1)
        .map(|sum| sum / alignment * alignment)
        .ok_or_else(|| error("QUEST page carrier 크기 올림 overflow"))
}

fn required_sizes(
    map: &ExeMap,
    page_cnx_bytes: usize,
) -> Result<(usize, usize, u32), QuestPageCarrierPlanError> {
    let carrier_offset = QUEST_PAGE_CARRIER_START_RUNTIME
        .checked_sub(map.load_addr)
        .ok_or_else(|| error("QUEST page carrier가 EXE load 주소보다 앞섬"))?;
    let unaligned_text = usize::try_from(carrier_offset)
        .map_err(|_| error("QUEST page carrier text offset 변환 overflow"))?
        .checked_add(page_cnx_bytes)
        .ok_or_else(|| error("QUEST page carrier text 크기 overflow"))?;
    let text_bytes = align_up(unaligned_text, FORM1_DATA_LEN)?;
    let file_bytes = usize::try_from(PSX_EXE_HEADER_LEN)
        .expect("PS-X EXE header 길이는 usize에 들어감")
        .checked_add(text_bytes)
        .ok_or_else(|| error("QUEST page carrier 파일 크기 overflow"))?;
    let sectors = u32::try_from(file_bytes / FORM1_DATA_LEN)
        .map_err(|_| error("QUEST page carrier sector 수 변환 overflow"))?;
    Ok((text_bytes, file_bytes, sectors))
}

/// 지원 원본 topology와 provisional 최대 page를 결합해 carrier 비용을 고정한다.
pub fn verify_quest_page_carrier_plan(
    iso: &Iso9660<'_>,
    reserved: &Box32mSummary,
    map: &ExeMap,
    text: &[u8],
    alternatives: &QuestFontAlternativesAnalysis,
) -> Result<QuestPageCarrierPlan, QuestPageCarrierPlanError> {
    let carrier =
        verify_box_quest_page_carrier(map, text).map_err(|source| error(source.to_string()))?;
    if alternatives.batch_page_cnx_min == 0
        || alternatives.batch_page_cnx_max < alternatives.batch_page_cnx_min
    {
        return Err(error("QUEST page CNX min/max가 유효하지 않음"));
    }
    if alternatives.batch_page_cnx_max > carrier.capacity_bytes {
        return Err(error(format!(
            "QUEST 최대 page CNX가 carrier를 넘음: page={}B, carrier={}B",
            alternatives.batch_page_cnx_max, carrier.capacity_bytes
        )));
    }

    let entry = iso
        .find(QUEST_EXE_ISO_PATH)
        .map_err(|source| error(source.to_string()))?;
    let [extent] = entry.extents() else {
        return Err(error(format!(
            "{QUEST_EXE_ISO_PATH} extent 수 불일치: {}",
            entry.extents().len()
        )));
    };
    if extent.lba != map.file.lba || extent.len != map.file.len {
        return Err(error(format!(
            "{QUEST_EXE_ISO_PATH} 사상 불일치: ISO=({},{}), map=({},{})",
            extent.lba, extent.len, map.file.lba, map.file.len
        )));
    }

    let original_sectors = map.file.sector_count();
    let original_end_lba = map
        .file
        .lba
        .checked_add(original_sectors)
        .ok_or_else(|| error("QUEST 원본 extent 끝 LBA overflow"))?;
    let next_extent_lba = iso
        .entries()
        .iter()
        .flat_map(|other| other.extents())
        .filter(|other| other.lba >= original_end_lba)
        .map(|other| other.lba)
        .min()
        .ok_or_else(|| error("QUEST 뒤 ISO extent가 없음"))?;
    if next_extent_lba != original_end_lba {
        return Err(error(format!(
            "QUEST 뒤 extent 경계 drift: expected={}, actual={next_extent_lba}",
            original_end_lba
        )));
    }

    let (required_text_bytes, required_file_bytes, required_sectors) =
        required_sizes(map, alternatives.batch_page_cnx_max)?;
    if required_text_bytes != QUEST_RELOCATION_CONTROL_TEXT_BYTES
        || required_file_bytes != QUEST_RELOCATION_CONTROL_FILE_BYTES
        || required_sectors != QUEST_RELOCATION_CONTROL_SECTORS
    {
        return Err(error(format!(
            "QUEST relocation control 크기 drift: text={required_text_bytes}, \
             file={required_file_bytes}, sectors={required_sectors}"
        )));
    }
    let original_file_bytes =
        usize::try_from(map.file.len).map_err(|_| error("QUEST 원본 파일 크기 변환 overflow"))?;
    let growth_bytes = required_file_bytes
        .checked_sub(original_file_bytes)
        .ok_or_else(|| error("QUEST page carrier가 원본 파일보다 작게 계산됨"))?;
    let growth_sectors = required_sectors
        .checked_sub(original_sectors)
        .ok_or_else(|| error("QUEST page carrier sector 증가량 underflow"))?;
    let reservation_fit = required_sectors <= reserved.form1_sectors;
    if !reservation_fit {
        return Err(error(format!(
            "QUEST 확장 EXE가 32M reservation을 넘음: required={}, available={}",
            required_sectors, reserved.form1_sectors
        )));
    }

    Ok(QuestPageCarrierPlan {
        schema_version: QUEST_PAGE_CARRIER_PLAN_SCHEMA_VERSION,
        carrier,
        page_cnx_min_bytes: alternatives.batch_page_cnx_min,
        page_cnx_max_bytes: alternatives.batch_page_cnx_max,
        original_file_bytes,
        original_sectors,
        required_text_bytes,
        required_file_bytes,
        required_sectors,
        growth_bytes,
        growth_sectors,
        original_end_lba,
        next_extent_lba,
        in_place_growth_possible: false,
        volume_tail_sectors: reserved.volume_tail_sectors,
        reservation_sectors: reserved.form1_sectors,
        reservation_fit,
        requires_iso_relocation: true,
        runtime_ownership_proven: false,
        iso_relocation_implemented: true,
        build_eligible: false,
    })
}

#[cfg(test)]
#[path = "quest_page_carrier_plan_tests.rs"]
mod tests;
