//! `QUEST.EXE` 상주 시작 메뉴 script의 resource 전환 기준점.
//!
//! `RPG_SEQ.VDD` 정적 path 원장과 달리 이 모듈은 EXE text 안에 상주하는
//! 시작 메뉴 선택표와 OP8A record 하나만 검증한다. record의 실행 도달은
//! source gate가 아니라 별도의 runtime 관측이 소유한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;

use super::quest_seq_resource_transitions::{
    collect_quest_seq_resource_catalog, resolve_quest_seq_direct_resource,
    QuestSeqResourceCatalogLayout,
};

pub const QUEST_START_MENU_BRANCH_RUNTIME: u32 = 0x8008_96CC;
pub const QUEST_START_MENU_SELECTION_FIELD_OFFSET: u8 = 0x34;
pub const QUEST_START_MENU_SELECTION_COUNT: u16 = 3;
pub const QUEST_START_MENU_NEW_GAME_INDEX: usize = 1;
pub const QUEST_START_MENU_NEW_GAME_STREAM_RUNTIME: u32 = 0x8008_96DC;
pub const QUEST_RESIDENT_OP8A_RUNTIME: u32 = 0x8008_9798;
pub const QUEST_RESIDENT_OP8A_RESOURCE_ID: u16 = 3;

const QUEST_START_MENU_BRANCH_BYTES: usize = 4 + QUEST_START_MENU_SELECTION_COUNT as usize * 4;
const QUEST_RESIDENT_OP8A_RECORD_BYTES: usize = 4;
const QUEST_START_MENU_BRANCH_TARGETS: [u32; QUEST_START_MENU_SELECTION_COUNT as usize] = [
    0x8008_97A0,
    QUEST_START_MENU_NEW_GAME_STREAM_RUNTIME,
    0x8008_97DC,
];
const QUEST_RESIDENT_OP8A_RAW: [u8; QUEST_RESIDENT_OP8A_RECORD_BYTES] = [0x8A, 0, 3, 0];

/// 지원 원본의 EXE 상주 시작 메뉴 전환 기준점.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqTransitionSummary {
    pub menu_branch_runtime: u32,
    pub menu_selection_field_offset: u8,
    pub menu_selection_count: u16,
    pub new_game_selection_index: usize,
    pub new_game_stream_runtime: u32,
    pub record_runtime: u32,
    pub record_text_offset: usize,
    pub encoded_resource_id: u16,
    pub descriptor_runtime: u32,
    pub vdd_entry_index: usize,
    pub target_sequence: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqTransitionError(String);

impl fmt::Display for QuestResidentSeqTransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentSeqTransitionError {}

fn error(message: impl Into<String>) -> QuestResidentSeqTransitionError {
    QuestResidentSeqTransitionError(message.into())
}

fn text_slice<'a>(
    map: &ExeMap,
    text: &'a [u8],
    runtime: u32,
    length: usize,
) -> Result<&'a [u8], QuestResidentSeqTransitionError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "QUEST resident SEQ 주소가 EXE text 밖임: 0x{runtime:08X}"
        ))
    })?;
    let end = offset
        .checked_add(length)
        .ok_or_else(|| error("QUEST resident SEQ 범위 계산 overflow"))?;
    text.get(offset..end).ok_or_else(|| {
        error(format!(
            "QUEST resident SEQ 원본이 잘림: 0x{runtime:08X}+0x{length:X}"
        ))
    })
}

fn read_u32(raw: &[u8], offset: usize) -> Result<u32, QuestResidentSeqTransitionError> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| error("QUEST resident SEQ u32 범위 계산 overflow"))?;
    let bytes = raw
        .get(offset..end)
        .ok_or_else(|| error("QUEST resident SEQ u32가 잘림"))?;
    Ok(u32::from_le_bytes(
        bytes
            .try_into()
            .map_err(|_| error("QUEST resident SEQ u32 폭 불일치"))?,
    ))
}

/// 지원 `QUEST.EXE`의 시작 메뉴 선택표와 상주 OP8A record를 검증한다.
pub fn verify_box_quest_resident_seq_transition<'a>(
    map: &ExeMap,
    text: &[u8],
    resource_catalog: impl IntoIterator<Item = &'a str>,
    layout: QuestSeqResourceCatalogLayout,
) -> Result<QuestResidentSeqTransitionSummary, QuestResidentSeqTransitionError> {
    if map.name != "QUEST.EXE" {
        return Err(error(format!(
            "QUEST resident SEQ 사상 이름 불일치: {}",
            map.name
        )));
    }

    let branch = text_slice(
        map,
        text,
        QUEST_START_MENU_BRANCH_RUNTIME,
        QUEST_START_MENU_BRANCH_BYTES,
    )?;
    if branch[..4]
        != [
            0x09,
            QUEST_START_MENU_SELECTION_FIELD_OFFSET,
            QUEST_START_MENU_SELECTION_COUNT as u8,
            0,
        ]
    {
        return Err(error(format!(
            "QUEST 시작 메뉴 OP09 header 불일치: {:02X?}",
            &branch[..4]
        )));
    }
    let branch_targets = (0..usize::from(QUEST_START_MENU_SELECTION_COUNT))
        .map(|index| read_u32(branch, 4 + index * 4))
        .collect::<Result<Vec<_>, _>>()?;
    if branch_targets != QUEST_START_MENU_BRANCH_TARGETS {
        return Err(error(format!(
            "QUEST 시작 메뉴 OP09 target 불일치: expected={QUEST_START_MENU_BRANCH_TARGETS:08X?}, actual={branch_targets:08X?}"
        )));
    }
    for target in &branch_targets {
        if !map.contains(*target) {
            return Err(error(format!(
                "QUEST 시작 메뉴 OP09 target이 EXE text 밖임: 0x{target:08X}"
            )));
        }
    }

    let record = text_slice(
        map,
        text,
        QUEST_RESIDENT_OP8A_RUNTIME,
        QUEST_RESIDENT_OP8A_RECORD_BYTES,
    )?;
    if record != QUEST_RESIDENT_OP8A_RAW {
        return Err(error(format!(
            "QUEST resident OP8A record 불일치: expected={QUEST_RESIDENT_OP8A_RAW:02X?}, actual={record:02X?}"
        )));
    }
    let encoded_resource_id = u16::from_le_bytes(
        record[2..4]
            .try_into()
            .map_err(|_| error("QUEST resident OP8A resource ID 폭 불일치"))?,
    );
    if encoded_resource_id != QUEST_RESIDENT_OP8A_RESOURCE_ID {
        return Err(error(format!(
            "QUEST resident OP8A resource ID 불일치: expected={QUEST_RESIDENT_OP8A_RESOURCE_ID}, actual={encoded_resource_id}"
        )));
    }

    let catalog = collect_quest_seq_resource_catalog(resource_catalog)
        .map_err(|cause| error(cause.to_string()))?;
    let resolved = resolve_quest_seq_direct_resource(
        &catalog,
        layout,
        encoded_resource_id,
        "QUEST.EXE resident OP8A 0x80089798",
    )
    .map_err(|cause| error(cause.to_string()))?;
    let record_text_offset = map
        .text_offset(QUEST_RESIDENT_OP8A_RUNTIME)
        .ok_or_else(|| error("QUEST resident OP8A text offset 환산 실패"))?;

    Ok(QuestResidentSeqTransitionSummary {
        menu_branch_runtime: QUEST_START_MENU_BRANCH_RUNTIME,
        menu_selection_field_offset: QUEST_START_MENU_SELECTION_FIELD_OFFSET,
        menu_selection_count: QUEST_START_MENU_SELECTION_COUNT,
        new_game_selection_index: QUEST_START_MENU_NEW_GAME_INDEX,
        new_game_stream_runtime: branch_targets[QUEST_START_MENU_NEW_GAME_INDEX],
        record_runtime: QUEST_RESIDENT_OP8A_RUNTIME,
        record_text_offset,
        encoded_resource_id,
        descriptor_runtime: resolved.descriptor_runtime,
        vdd_entry_index: resolved.vdd_entry_index,
        target_sequence: resolved.target_sequence,
    })
}

#[cfg(test)]
#[path = "quest_resident_seq_transition_tests.rs"]
mod tests;
