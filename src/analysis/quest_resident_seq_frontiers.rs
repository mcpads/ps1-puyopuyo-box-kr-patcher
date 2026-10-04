//! `QUEST.EXE` 상주 literal path의 runtime frontier 입력.
//!
//! [`super::quest_resident_seq_paths`]가 멈춘 record를 다시 임의 분기하지 않고,
//! OP13/14 비교 operand·두 다음 주소와 OP80 spawn stream target을 원본에서
//! 구조화한다. probe 주소와 target이 확정돼도 조건값, 객체 생성 성공과
//! 정상 실행 도달은 runtime 관측 대상으로 남는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::disasm::{
    quest_seq_base_compare_record_layout, QuestSeqBaseCompareOperandSource,
    QuestSeqBaseCompareOperation, QUEST_SEQ_BASE_COMPARE_STATE_BASE_RUNTIME,
};

use super::quest_resident_seq_paths::{QuestResidentSeqPathStatus, QuestResidentSeqPathSummary};

const BOX_QUEST_RESIDENT_FRONTIERS: usize = 4;
const BOX_QUEST_RESIDENT_COMPARE_FRONTIERS: usize = 3;
const BOX_QUEST_RESIDENT_SPAWN_FRONTIERS: usize = 1;

/// frontier를 결정하는 runtime 입력 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestResidentSeqRuntimeRequirement {
    GlobalStateWord,
    SpawnObjectState,
}

impl QuestResidentSeqRuntimeRequirement {
    pub const fn label(self) -> &'static str {
        match self {
            Self::GlobalStateWord => "global-state-word",
            Self::SpawnObjectState => "spawn-object-state",
        }
    }
}

/// OP13/14 frontier의 두 정적 다음 주소와 비교 입력.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestResidentSeqCompareFrontier {
    pub operation: QuestSeqBaseCompareOperation,
    pub value_bytes: usize,
    pub left_source: QuestSeqBaseCompareOperandSource,
    pub left_parameter: Option<u32>,
    pub right_source: QuestSeqBaseCompareOperandSource,
    pub right_parameter: u32,
    pub branch_target_runtime: u32,
    pub fallthrough_runtime: u32,
    pub runtime_probe_address: Option<u32>,
    pub runtime_probe_length: Option<usize>,
}

/// literal path 하나의 정적 종단과 다음 runtime 관측점.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestResidentSeqFrontier {
    pub root_runtime: u32,
    pub record_runtime: u32,
    pub opcode: u8,
    pub requirement: QuestResidentSeqRuntimeRequirement,
    pub compare: Option<QuestResidentSeqCompareFrontier>,
    pub spawn_stream_runtime: Option<u32>,
}

/// 지원 원본의 literal path frontier 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqFrontierSummary {
    pub frontiers: Vec<QuestResidentSeqFrontier>,
    pub compare_frontiers: usize,
    pub spawn_frontiers: usize,
    pub unique_probe_addresses: usize,
    pub unique_spawn_streams: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqFrontierError(String);

impl fmt::Display for QuestResidentSeqFrontierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentSeqFrontierError {}

fn error(message: impl Into<String>) -> QuestResidentSeqFrontierError {
    QuestResidentSeqFrontierError(message.into())
}

fn read_u32(text: &[u8], offset: usize, role: &str) -> Result<u32, QuestResidentSeqFrontierError> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| error(format!("QUEST resident {role} 범위 overflow")))?;
    let raw = text
        .get(offset..end)
        .ok_or_else(|| error(format!("QUEST resident {role}가 잘림")))?;
    Ok(u32::from_le_bytes(
        raw.try_into().expect("4바이트 범위를 확인함"),
    ))
}

fn compare_value(
    text: &[u8],
    offset: usize,
    value_bytes: usize,
) -> Result<u32, QuestResidentSeqFrontierError> {
    let end = offset
        .checked_add(value_bytes)
        .ok_or_else(|| error("QUEST resident compare value 범위 overflow"))?;
    let raw = text
        .get(offset..end)
        .ok_or_else(|| error("QUEST resident compare value가 잘림"))?;
    match value_bytes {
        2 => Ok(u32::from(u16::from_le_bytes(
            raw.try_into().expect("2바이트 범위를 확인함"),
        ))),
        4 => Ok(u32::from_le_bytes(
            raw.try_into().expect("4바이트 범위를 확인함"),
        )),
        _ => Err(error(format!(
            "QUEST resident compare value 폭이 지원 범위 밖임: {value_bytes}"
        ))),
    }
}

pub(super) fn compare_frontier(
    map: &ExeMap,
    text: &[u8],
    record_runtime: u32,
) -> Result<QuestResidentSeqCompareFrontier, QuestResidentSeqFrontierError> {
    let offset = map.text_offset(record_runtime).ok_or_else(|| {
        error(format!(
            "QUEST resident compare record가 EXE text 밖임: 0x{record_runtime:08X}"
        ))
    })?;
    let opcode = *text
        .get(offset)
        .ok_or_else(|| error("QUEST resident compare opcode가 잘림"))?;
    let selector = *text
        .get(offset + 1)
        .ok_or_else(|| error("QUEST resident compare selector가 잘림"))?;
    let layout = quest_seq_base_compare_record_layout(opcode, selector).ok_or_else(|| {
        error(format!(
            "QUEST resident compare layout이 없음: OP{opcode:02X}/0x{selector:02X}"
        ))
    })?;
    let left_parameter = layout
        .left_parameter_offset
        .map(|relative| {
            offset
                .checked_add(relative)
                .and_then(|at| text.get(at))
                .copied()
                .map(u32::from)
                .ok_or_else(|| error("QUEST resident compare left parameter가 잘림"))
        })
        .transpose()?;
    let right_offset = offset
        .checked_add(layout.right_parameter_offset)
        .ok_or_else(|| error("QUEST resident compare right parameter offset overflow"))?;
    let right_parameter = if layout.right_source == QuestSeqBaseCompareOperandSource::Inline {
        compare_value(text, right_offset, layout.value_bytes)?
    } else {
        u32::from(
            *text
                .get(right_offset)
                .ok_or_else(|| error("QUEST resident compare right parameter가 잘림"))?,
        )
    };
    let pointer_offset = offset
        .checked_add(layout.branch_pointer_offset)
        .ok_or_else(|| error("QUEST resident compare branch offset overflow"))?;
    let branch_target_runtime = read_u32(text, pointer_offset, "compare branch pointer")?;
    if !map.contains(branch_target_runtime) {
        return Err(error(format!(
            "QUEST resident compare branch target이 EXE text 밖임: 0x{record_runtime:08X}->0x{branch_target_runtime:08X}"
        )));
    }
    let fallthrough_offset = offset
        .checked_add(layout.fallthrough_offset)
        .ok_or_else(|| error("QUEST resident compare fallthrough offset overflow"))?;
    let fallthrough_runtime = map.text_runtime_addr(fallthrough_offset).ok_or_else(|| {
        error(format!(
            "QUEST resident compare fallthrough가 EXE text 밖임: +0x{fallthrough_offset:X}"
        ))
    })?;
    let runtime_probe_address = match (layout.left_source, left_parameter) {
        (QuestSeqBaseCompareOperandSource::GlobalState, Some(index)) => {
            QUEST_SEQ_BASE_COMPARE_STATE_BASE_RUNTIME.checked_add(
                index
                    .checked_mul(4)
                    .ok_or_else(|| error("QUEST resident compare global-state index overflow"))?,
            )
        }
        _ => None,
    };

    Ok(QuestResidentSeqCompareFrontier {
        operation: layout.operation,
        value_bytes: layout.value_bytes,
        left_source: layout.left_source,
        left_parameter,
        right_source: layout.right_source,
        right_parameter,
        branch_target_runtime,
        fallthrough_runtime,
        runtime_probe_address,
        runtime_probe_length: runtime_probe_address.map(|_| layout.value_bytes),
    })
}

fn spawn_stream(
    map: &ExeMap,
    text: &[u8],
    record_runtime: u32,
) -> Result<u32, QuestResidentSeqFrontierError> {
    let offset = map.text_offset(record_runtime).ok_or_else(|| {
        error(format!(
            "QUEST resident OP80 record가 EXE text 밖임: 0x{record_runtime:08X}"
        ))
    })?;
    let raw = text
        .get(
            offset
                ..offset
                    .checked_add(8)
                    .ok_or_else(|| error("OP80 범위 overflow"))?,
        )
        .ok_or_else(|| error("QUEST resident OP80 record가 잘림"))?;
    if raw[..4] != [0x80, 0, 0, 0] {
        return Err(error(format!(
            "QUEST resident OP80 spawn 조건 불일치: 0x{record_runtime:08X}, raw={:02X?}",
            &raw[..4]
        )));
    }
    let target = u32::from_le_bytes(raw[4..8].try_into().expect("8바이트 범위를 확인함"));
    if !map.contains(target) {
        return Err(error(format!(
            "QUEST resident OP80 spawn target이 EXE text 밖임: 0x{target:08X}"
        )));
    }
    Ok(target)
}

/// 지원 `QUEST.EXE` literal path의 네 runtime frontier를 검증한다.
pub fn verify_box_quest_resident_seq_frontiers(
    map: &ExeMap,
    text: &[u8],
    paths: &QuestResidentSeqPathSummary,
) -> Result<QuestResidentSeqFrontierSummary, QuestResidentSeqFrontierError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST resident frontier 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len
        )));
    }
    let mut frontiers = Vec::with_capacity(paths.paths.len());
    for path in &paths.paths {
        if path.status != QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier {
            return Err(error(format!(
                "QUEST resident path가 runtime frontier에서 끝나지 않음: root=0x{:08X}, status={:?}",
                path.root_runtime, path.status
            )));
        }
        let terminal = path
            .nodes
            .last()
            .ok_or_else(|| error("QUEST resident path node가 없음"))?;
        let (requirement, compare, spawn_stream_runtime) = match terminal.opcode {
            0x13 | 0x14 => (
                QuestResidentSeqRuntimeRequirement::GlobalStateWord,
                Some(compare_frontier(map, text, terminal.runtime)?),
                None,
            ),
            0x80 => (
                QuestResidentSeqRuntimeRequirement::SpawnObjectState,
                None,
                Some(spawn_stream(map, text, terminal.runtime)?),
            ),
            opcode => {
                return Err(error(format!(
                    "QUEST resident frontier opcode를 구조화하지 못함: root=0x{:08X}, OP{opcode:02X}",
                    path.root_runtime
                )));
            }
        };
        frontiers.push(QuestResidentSeqFrontier {
            root_runtime: path.root_runtime,
            record_runtime: terminal.runtime,
            opcode: terminal.opcode,
            requirement,
            compare,
            spawn_stream_runtime,
        });
    }

    let summary = QuestResidentSeqFrontierSummary {
        compare_frontiers: frontiers
            .iter()
            .filter(|frontier| frontier.compare.is_some())
            .count(),
        spawn_frontiers: frontiers
            .iter()
            .filter(|frontier| frontier.spawn_stream_runtime.is_some())
            .count(),
        unique_probe_addresses: frontiers
            .iter()
            .filter_map(|frontier| frontier.compare?.runtime_probe_address)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        unique_spawn_streams: frontiers
            .iter()
            .filter_map(|frontier| frontier.spawn_stream_runtime)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        frontiers,
    };
    if summary.frontiers.len() != BOX_QUEST_RESIDENT_FRONTIERS
        || summary.compare_frontiers != BOX_QUEST_RESIDENT_COMPARE_FRONTIERS
        || summary.spawn_frontiers != BOX_QUEST_RESIDENT_SPAWN_FRONTIERS
        || summary.unique_probe_addresses != 1
        || summary.unique_spawn_streams != 1
    {
        return Err(error(format!(
            "QUEST resident frontier 분모 drift: rows {}, compare {}, spawn {}, probes {}, streams {}",
            summary.frontiers.len(),
            summary.compare_frontiers,
            summary.spawn_frontiers,
            summary.unique_probe_addresses,
            summary.unique_spawn_streams,
        )));
    }
    let expected = [
        (0x8008_97DC, 0x8008_9854, 0x8008_9850, 0x8008_9860),
        (0x8008_C498, 0x8008_C518, 0x8008_C514, 0x8008_C524),
        (0x8008_C590, 0x8008_C5A4, 0x8008_C5A0, 0x8008_C5B0),
    ];
    let compares = summary
        .frontiers
        .iter()
        .filter_map(|frontier| frontier.compare.map(|compare| (frontier, compare)))
        .collect::<Vec<_>>();
    for ((frontier, compare), (root, record, branch, fallthrough)) in
        compares.into_iter().zip(expected)
    {
        if frontier.root_runtime != root
            || frontier.record_runtime != record
            || compare.operation != QuestSeqBaseCompareOperation::Equal
            || compare.value_bytes != 4
            || compare.left_source != QuestSeqBaseCompareOperandSource::GlobalState
            || compare.left_parameter != Some(0)
            || compare.right_source != QuestSeqBaseCompareOperandSource::Inline
            || compare.right_parameter != 0
            || compare.branch_target_runtime != branch
            || compare.fallthrough_runtime != fallthrough
            || compare.runtime_probe_address != Some(QUEST_SEQ_BASE_COMPARE_STATE_BASE_RUNTIME)
            || compare.runtime_probe_length != Some(4)
        {
            return Err(error(format!(
                "QUEST resident compare frontier drift: root=0x{root:08X}, actual={frontier:?}/{compare:?}"
            )));
        }
    }
    let spawn = summary
        .frontiers
        .iter()
        .find(|frontier| frontier.spawn_stream_runtime.is_some())
        .expect("spawn frontier 분모를 확인함");
    if spawn.root_runtime != 0x8008_9888
        || spawn.record_runtime != 0x8008_9898
        || spawn.spawn_stream_runtime != Some(0x8008_9648)
    {
        return Err(error(format!(
            "QUEST resident spawn frontier drift: {spawn:?}"
        )));
    }
    Ok(summary)
}

#[cfg(test)]
#[path = "quest_resident_seq_frontiers_tests.rs"]
mod tests;
