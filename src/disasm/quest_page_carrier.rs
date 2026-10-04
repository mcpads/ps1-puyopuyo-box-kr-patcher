//! QUEST page CNX를 resident로 둘 수 있는 post-BSS 후보의 정적 경계.
//!
//! startup zero-clear 끝을 2,048바이트로 올림한 주소부터 원본 loader
//! arena 직전까지를 먼저 훑는다. 첫 정적 pointer가 나타나는 주소에서
//! 범위를 잘라 그 앞쪽만 resident 후보로 남긴다. 정적 미참조는 runtime
//! 소유권 증명이 아니므로 이 모듈만으로 patch 설치를 승인하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::{normalize_kseg, ExeMap};

use super::linear;
use super::quest_hook_space::{
    verify_box_quest_hook_space, QuestHookSpaceError, QUEST_HOOK_TEXT_END_RUNTIME,
    QUEST_HOOK_ZERO_CLEAR_END_RUNTIME,
};
use super::quest_page_source::{
    verify_box_quest_page_source, QuestPageSourceError, QUEST_LOADER_ARENA_START_RUNTIME,
};
use super::xref::{find_address_loads, DEFAULT_WINDOW};

pub const QUEST_PAGE_CARRIER_ALIGNMENT: u32 = 2_048;
pub const QUEST_PAGE_CARRIER_START_RUNTIME: u32 = 0x8010_8800;
pub const QUEST_PAGE_CARRIER_END_RUNTIME: u32 = 0x8011_9008;
pub const QUEST_PAGE_CARRIER_SEARCH_END_RUNTIME: u32 = QUEST_LOADER_ARENA_START_RUNTIME;
pub const QUEST_PAGE_CARRIER_CAPACITY_BYTES: usize =
    (QUEST_PAGE_CARRIER_END_RUNTIME - QUEST_PAGE_CARRIER_START_RUNTIME) as usize;

/// post-BSS resident 후보의 정적 참조 하한.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestPageCarrierSummary {
    pub start_runtime: u32,
    pub end_runtime: u32,
    pub capacity_bytes: usize,
    pub search_end_runtime: u32,
    pub search_capacity_bytes: usize,
    pub zero_clear_end_runtime: u32,
    pub alignment_gap_bytes: usize,
    pub extension_padding_bytes: usize,
    pub address_loads: usize,
    pub aligned_pointer_words: usize,
    pub suffix_address_loads: usize,
    pub suffix_aligned_pointer_words: usize,
    pub first_suffix_reference_runtime: u32,
    pub static_unreferenced: bool,
    pub runtime_ownership_proven: bool,
    pub build_eligible: bool,
}

#[derive(Debug)]
pub enum QuestPageCarrierError {
    HookSpace(QuestHookSpaceError),
    PageSource(QuestPageSourceError),
    UnalignedText(usize),
    CoordinateOverflow,
    BoundaryMismatch(&'static str),
    ReferenceBoundaryMismatch {
        address_loads: usize,
        aligned_pointer_words: usize,
        first_reference_runtime: Option<u32>,
    },
}

impl fmt::Display for QuestPageCarrierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HookSpace(source) => write!(f, "QUEST page carrier BSS 경계 실패: {source}"),
            Self::PageSource(source) => {
                write!(f, "QUEST page carrier loader 경계 실패: {source}")
            }
            Self::UnalignedText(actual) => write!(
                f,
                "QUEST page carrier text 길이가 4바이트 정렬이 아님: {actual}"
            ),
            Self::CoordinateOverflow => f.write_str("QUEST page carrier 좌표 overflow"),
            Self::BoundaryMismatch(message) => {
                write!(f, "QUEST page carrier 경계 불일치: {message}")
            }
            Self::ReferenceBoundaryMismatch {
                address_loads,
                aligned_pointer_words,
                first_reference_runtime,
            } => write!(
                f,
                "QUEST page carrier 정적 참조 경계 drift: address_loads={address_loads}, \
                 aligned_pointer_words={aligned_pointer_words}, first={}",
                first_reference_runtime
                    .map(|address| format!("0x{address:08X}"))
                    .unwrap_or_else(|| "없음".to_string())
            ),
        }
    }
}

impl Error for QuestPageCarrierError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::HookSpace(source) => Some(source),
            Self::PageSource(source) => Some(source),
            _ => None,
        }
    }
}

impl From<QuestHookSpaceError> for QuestPageCarrierError {
    fn from(value: QuestHookSpaceError) -> Self {
        Self::HookSpace(value)
    }
}

impl From<QuestPageSourceError> for QuestPageCarrierError {
    fn from(value: QuestPageSourceError) -> Self {
        Self::PageSource(value)
    }
}

fn in_range(value: u32, start: u32, end: u32) -> bool {
    let normalized = normalize_kseg(value);
    (start..end).contains(&normalized)
}

fn count_static_references(
    map: &ExeMap,
    text: &[u8],
    start: u32,
    end: u32,
) -> Result<(usize, usize, Option<u32>), QuestPageCarrierError> {
    let instructions =
        linear::sweep(map, text).ok_or(QuestPageCarrierError::UnalignedText(text.len()))?;
    let matching_loads = find_address_loads(&instructions, DEFAULT_WINDOW)
        .iter()
        .filter(|load| in_range(load.value, start, end))
        .map(|load| normalize_kseg(load.value))
        .collect::<Vec<_>>();
    let matching_pointers = text
        .chunks_exact(4)
        .filter_map(|raw| {
            let value =
                u32::from_le_bytes((*raw).try_into().expect("4바이트 정렬 chunk 범위를 확인함"));
            in_range(value, start, end).then_some(normalize_kseg(value))
        })
        .collect::<Vec<_>>();
    let first_reference_runtime = matching_loads
        .iter()
        .chain(&matching_pointers)
        .copied()
        .min();
    Ok((
        matching_loads.len(),
        matching_pointers.len(),
        first_reference_runtime,
    ))
}

/// 지원 원본의 post-BSS resident 후보 경계와 정적 미참조 하한을 검증한다.
pub fn verify_box_quest_page_carrier(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestPageCarrierSummary, QuestPageCarrierError> {
    verify_box_quest_hook_space(map, text)?;
    verify_box_quest_page_source(map, text)?;

    let expected_start = QUEST_HOOK_ZERO_CLEAR_END_RUNTIME
        .checked_add(QUEST_PAGE_CARRIER_ALIGNMENT - 1)
        .ok_or(QuestPageCarrierError::CoordinateOverflow)?
        / QUEST_PAGE_CARRIER_ALIGNMENT
        * QUEST_PAGE_CARRIER_ALIGNMENT;
    if expected_start != QUEST_PAGE_CARRIER_START_RUNTIME {
        return Err(QuestPageCarrierError::BoundaryMismatch(
            "startup clear 끝의 sector 올림 주소",
        ));
    }
    if QUEST_PAGE_CARRIER_SEARCH_END_RUNTIME != QUEST_LOADER_ARENA_START_RUNTIME
        || QUEST_PAGE_CARRIER_START_RUNTIME >= QUEST_PAGE_CARRIER_END_RUNTIME
        || QUEST_PAGE_CARRIER_END_RUNTIME >= QUEST_PAGE_CARRIER_SEARCH_END_RUNTIME
    {
        return Err(QuestPageCarrierError::BoundaryMismatch(
            "carrier와 정적 search와 loader arena 순서",
        ));
    }
    if QUEST_PAGE_CARRIER_CAPACITY_BYTES != 67_592 {
        return Err(QuestPageCarrierError::BoundaryMismatch("carrier 용량 산술"));
    }

    let (address_loads, aligned_pointer_words, first_candidate_reference) =
        count_static_references(
            map,
            text,
            QUEST_PAGE_CARRIER_START_RUNTIME,
            QUEST_PAGE_CARRIER_END_RUNTIME,
        )?;
    if address_loads != 0 || aligned_pointer_words != 0 {
        return Err(QuestPageCarrierError::ReferenceBoundaryMismatch {
            address_loads,
            aligned_pointer_words,
            first_reference_runtime: first_candidate_reference,
        });
    }
    let (suffix_address_loads, suffix_aligned_pointer_words, first_suffix_reference_runtime) =
        count_static_references(
            map,
            text,
            QUEST_PAGE_CARRIER_END_RUNTIME,
            QUEST_PAGE_CARRIER_SEARCH_END_RUNTIME,
        )?;
    if suffix_address_loads != 39
        || suffix_aligned_pointer_words != 4
        || first_suffix_reference_runtime != Some(QUEST_PAGE_CARRIER_END_RUNTIME)
    {
        return Err(QuestPageCarrierError::ReferenceBoundaryMismatch {
            address_loads: suffix_address_loads,
            aligned_pointer_words: suffix_aligned_pointer_words,
            first_reference_runtime: first_suffix_reference_runtime,
        });
    }
    let alignment_gap_bytes = usize::try_from(
        QUEST_PAGE_CARRIER_START_RUNTIME
            .checked_sub(QUEST_HOOK_ZERO_CLEAR_END_RUNTIME)
            .ok_or(QuestPageCarrierError::CoordinateOverflow)?,
    )
    .map_err(|_| QuestPageCarrierError::CoordinateOverflow)?;
    let extension_padding_bytes = usize::try_from(
        QUEST_PAGE_CARRIER_START_RUNTIME
            .checked_sub(QUEST_HOOK_TEXT_END_RUNTIME)
            .ok_or(QuestPageCarrierError::CoordinateOverflow)?,
    )
    .map_err(|_| QuestPageCarrierError::CoordinateOverflow)?;

    Ok(QuestPageCarrierSummary {
        start_runtime: QUEST_PAGE_CARRIER_START_RUNTIME,
        end_runtime: QUEST_PAGE_CARRIER_END_RUNTIME,
        capacity_bytes: QUEST_PAGE_CARRIER_CAPACITY_BYTES,
        search_end_runtime: QUEST_PAGE_CARRIER_SEARCH_END_RUNTIME,
        search_capacity_bytes: usize::try_from(
            QUEST_PAGE_CARRIER_SEARCH_END_RUNTIME - QUEST_PAGE_CARRIER_START_RUNTIME,
        )
        .map_err(|_| QuestPageCarrierError::CoordinateOverflow)?,
        zero_clear_end_runtime: QUEST_HOOK_ZERO_CLEAR_END_RUNTIME,
        alignment_gap_bytes,
        extension_padding_bytes,
        address_loads,
        aligned_pointer_words,
        suffix_address_loads,
        suffix_aligned_pointer_words,
        first_suffix_reference_runtime: first_suffix_reference_runtime
            .expect("검증한 suffix 첫 참조가 있음"),
        static_unreferenced: true,
        runtime_ownership_proven: false,
        build_eligible: false,
    })
}

#[cfg(test)]
#[path = "quest_page_carrier_tests.rs"]
mod tests;
