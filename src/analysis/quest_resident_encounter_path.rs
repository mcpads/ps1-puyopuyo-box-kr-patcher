//! `QUEST.EXE` 자연 조우 표시로 이어지는 조건부 상주 SEQ 경로.
//!
//! literal root `0x8008C590`의 OP14 false edge와 뒤 OP13 false edge를
//! 임의의 runtime 선택으로 승격하지 않고, 각각 가능한 source edge로만
//! 보존한다. 두 번째 edge가 가리키는 stream은 공용 walker로 OP21과 다음
//! OP09 frontier까지 읽는다. 이 결과는 조건부 정적 경로이며 실제 조우
//! 도달이나 특정 상대 선택을 뜻하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::disasm::{QuestSeqBaseCompareOperandSource, QuestSeqBaseCompareOperation};
use crate::font::QuestCodeTable;

use super::quest_resident_seq_frontiers::{
    compare_frontier, QuestResidentSeqCompareFrontier, QuestResidentSeqFrontier,
    QuestResidentSeqFrontierSummary,
};
use super::quest_resident_seq_paths::{
    walk_resident_stream, QuestResidentSeqFrontierRequirement, QuestResidentSeqPath,
    QuestResidentSeqPathStatus,
};

pub const QUEST_RESIDENT_ENCOUNTER_LITERAL_ROOT_RUNTIME: u32 = 0x8008_C590;
pub const QUEST_RESIDENT_ENCOUNTER_FIRST_COMPARE_RUNTIME: u32 = 0x8008_C5A4;
pub const QUEST_RESIDENT_ENCOUNTER_WAIT_RUNTIME: u32 = 0x8008_C5B0;
pub const QUEST_RESIDENT_ENCOUNTER_READY_COMPARE_RUNTIME: u32 = 0x8008_C5B4;
pub const QUEST_RESIDENT_ENCOUNTER_DISPLAY_RUNTIME: u32 = 0x8008_C5C0;
pub const QUEST_RESIDENT_ENCOUNTER_TEXT_RUNTIME: u32 = 0x8008_C5D4;
pub const QUEST_RESIDENT_ENCOUNTER_NEXT_SELECTOR_RUNTIME: u32 = 0x8008_C648;

/// 두 compare edge와 표시 stream을 분리해 보존한 조건부 source path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentEncounterPathAnalysis {
    pub literal_root_runtime: u32,
    pub first_compare_runtime: u32,
    pub first_compare: QuestResidentSeqCompareFrontier,
    pub wait_path: QuestResidentSeqPath,
    pub ready_compare: QuestResidentSeqCompareFrontier,
    pub display_path: QuestResidentSeqPath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentEncounterPathError(String);

impl fmt::Display for QuestResidentEncounterPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentEncounterPathError {}

fn error(message: impl Into<String>) -> QuestResidentEncounterPathError {
    QuestResidentEncounterPathError(message.into())
}

/// 한 literal frontier에서 두 false edge와 조우 표시 stream을 구조화한다.
pub fn audit_quest_resident_encounter_path(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    source_frontier: &QuestResidentSeqFrontier,
) -> Result<QuestResidentEncounterPathAnalysis, QuestResidentEncounterPathError> {
    let first_compare = source_frontier.compare.ok_or_else(|| {
        error(format!(
            "QUEST resident encounter root 0x{:08X} frontier가 compare가 아님",
            source_frontier.root_runtime
        ))
    })?;
    if source_frontier.record_runtime == first_compare.fallthrough_runtime {
        return Err(error(
            "QUEST resident encounter 첫 compare가 자기 자신으로 fallthrough함",
        ));
    }

    let wait_path =
        walk_resident_stream(map, text, code_table, first_compare.fallthrough_runtime, 0)
            .map_err(|cause| error(format!("QUEST resident encounter wait path: {cause}")))?;
    let wait_terminal = wait_path
        .nodes
        .last()
        .ok_or_else(|| error("QUEST resident encounter wait path가 비었음"))?;
    if wait_path.status != QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier
        || wait_terminal.opcode != 0x13
        || wait_path.frontier_requirement != Some(QuestResidentSeqFrontierRequirement::CompareState)
    {
        return Err(error(format!(
            "QUEST resident encounter wait path가 OP13 compare에서 끝나지 않음: status={:?}, opcode=OP{:02X}, requirement={:?}",
            wait_path.status, wait_terminal.opcode, wait_path.frontier_requirement,
        )));
    }
    let ready_compare = compare_frontier(map, text, wait_terminal.runtime)
        .map_err(|cause| error(format!("QUEST resident encounter ready compare: {cause}")))?;
    if ready_compare.branch_target_runtime != wait_path.root_runtime {
        return Err(error(format!(
            "QUEST resident encounter OP13 true edge가 wait loop로 돌아가지 않음: 0x{:08X}->0x{:08X}",
            wait_terminal.runtime, ready_compare.branch_target_runtime,
        )));
    }

    let display_path =
        walk_resident_stream(map, text, code_table, ready_compare.fallthrough_runtime, 0)
            .map_err(|cause| error(format!("QUEST resident encounter display path: {cause}")))?;

    Ok(QuestResidentEncounterPathAnalysis {
        literal_root_runtime: source_frontier.root_runtime,
        first_compare_runtime: source_frontier.record_runtime,
        first_compare,
        wait_path,
        ready_compare,
        display_path,
    })
}

/// 지원 원본의 자연 조우 조건부 source path를 fail closed로 검증한다.
pub fn verify_box_quest_resident_encounter_path(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    frontiers: &QuestResidentSeqFrontierSummary,
) -> Result<QuestResidentEncounterPathAnalysis, QuestResidentEncounterPathError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST resident encounter 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len,
        )));
    }
    let mut candidates = frontiers
        .frontiers
        .iter()
        .filter(|frontier| frontier.root_runtime == QUEST_RESIDENT_ENCOUNTER_LITERAL_ROOT_RUNTIME);
    let source_frontier = candidates
        .next()
        .ok_or_else(|| error("QUEST resident encounter literal frontier가 없음"))?;
    if candidates.next().is_some() {
        return Err(error("QUEST resident encounter literal frontier가 중복됨"));
    }
    let analysis = audit_quest_resident_encounter_path(map, text, code_table, source_frontier)?;

    let first = analysis.first_compare;
    if analysis.first_compare_runtime != QUEST_RESIDENT_ENCOUNTER_FIRST_COMPARE_RUNTIME
        || first.operation != QuestSeqBaseCompareOperation::Equal
        || first.value_bytes != 4
        || first.left_source != QuestSeqBaseCompareOperandSource::GlobalState
        || first.left_parameter != Some(0)
        || first.right_source != QuestSeqBaseCompareOperandSource::Inline
        || first.right_parameter != 0
        || first.branch_target_runtime != 0x8008_C5A0
        || first.fallthrough_runtime != QUEST_RESIDENT_ENCOUNTER_WAIT_RUNTIME
    {
        return Err(error(format!(
            "QUEST resident encounter 첫 OP14 drift: runtime=0x{:08X}, compare={first:?}",
            analysis.first_compare_runtime,
        )));
    }

    let wait_opcodes = analysis
        .wait_path
        .nodes
        .iter()
        .map(|node| node.opcode)
        .collect::<Vec<_>>();
    let ready = analysis.ready_compare;
    if analysis.wait_path.root_runtime != QUEST_RESIDENT_ENCOUNTER_WAIT_RUNTIME
        || wait_opcodes != [0x01, 0x13]
        || analysis.wait_path.nodes.last().map(|node| node.runtime)
            != Some(QUEST_RESIDENT_ENCOUNTER_READY_COMPARE_RUNTIME)
        || ready.operation != QuestSeqBaseCompareOperation::Equal
        || ready.value_bytes != 2
        || ready.left_source != QuestSeqBaseCompareOperandSource::ObjectField
        || ready.left_parameter != Some(0xC6)
        || ready.right_source != QuestSeqBaseCompareOperandSource::Inline
        || ready.right_parameter != 0
        || ready.branch_target_runtime != QUEST_RESIDENT_ENCOUNTER_WAIT_RUNTIME
        || ready.fallthrough_runtime != QUEST_RESIDENT_ENCOUNTER_DISPLAY_RUNTIME
    {
        return Err(error(format!(
            "QUEST resident encounter wait/OP13 drift: root=0x{:08X}, opcodes={wait_opcodes:02X?}, compare={ready:?}",
            analysis.wait_path.root_runtime,
        )));
    }

    let display_opcodes = analysis
        .display_path
        .nodes
        .iter()
        .map(|node| node.opcode)
        .collect::<Vec<_>>();
    let text_node = analysis
        .display_path
        .nodes
        .iter()
        .find(|node| node.opcode == 0x21);
    if analysis.display_path.root_runtime != QUEST_RESIDENT_ENCOUNTER_DISPLAY_RUNTIME
        || display_opcodes != [0x02, 0x9A, 0xBE, 0x8C, 0x02, 0x21, 0x09]
        || analysis.display_path.status != QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier
        || analysis.display_path.frontier_requirement
            != Some(QuestResidentSeqFrontierRequirement::ObjectBranchSelectorState)
        || analysis.display_path.nodes.last().map(|node| node.runtime)
            != Some(QUEST_RESIDENT_ENCOUNTER_NEXT_SELECTOR_RUNTIME)
        || text_node.map(|node| node.runtime) != Some(QUEST_RESIDENT_ENCOUNTER_TEXT_RUNTIME)
        || text_node.map(|node| node.next_runtime)
            != Some(Some(QUEST_RESIDENT_ENCOUNTER_NEXT_SELECTOR_RUNTIME))
        || text_node.map(|node| node.text_glyphs) != Some(18)
        || text_node.map(|node| node.text_controls) != Some(19)
    {
        return Err(error(format!(
            "QUEST resident encounter display path drift: opcodes={display_opcodes:02X?}, status={:?}, requirement={:?}, text={text_node:?}",
            analysis.display_path.status, analysis.display_path.frontier_requirement,
        )));
    }

    Ok(analysis)
}

#[cfg(test)]
#[path = "quest_resident_encounter_path_tests.rs"]
mod tests;
