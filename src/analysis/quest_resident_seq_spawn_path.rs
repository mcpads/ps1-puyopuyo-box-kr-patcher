//! resident OP80 child seed에서 시작 메뉴 branch까지의 정적 path.
//!
//! literal root `0x80089888`의 OP80 side edge가 가리키는 `0x80089648`을
//! 별도 seed로 걷는다. active OPC1의 deferred stream과 현재 stream 전진을
//! 둘 다 보존한다. deferred stream은 별도로 걸어 OP00 해제에서 멈추며,
//! OP09 selector 값은 고르지 않는다. constructor literal root 분모에는 이
//! 파생 seed를 더하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::font::QuestCodeTable;

use super::quest_resident_seq_frontiers::QuestResidentSeqFrontierSummary;
use super::quest_resident_seq_paths::{
    walk_resident_stream, QuestResidentSeqFrontierRequirement, QuestResidentSeqPath,
    QuestResidentSeqPathStatus, QuestResidentSeqPathTransition,
};
use super::quest_resident_seq_transition::QuestResidentSeqTransitionSummary;

pub const QUEST_RESIDENT_SPAWN_PARENT_RUNTIME: u32 = 0x8008_9898;
pub const QUEST_RESIDENT_SPAWN_STREAM_RUNTIME: u32 = 0x8008_9648;
pub const QUEST_RESIDENT_DEFERRED_STREAM_RUNTIME: u32 = 0x8008_98D0;
pub const QUEST_RESIDENT_DEFERRED_RELEASE_RUNTIME: u32 = 0x8008_98F8;
pub const QUEST_RESIDENT_DEFERRED_FOLLOWING_CELL_TABLE_RUNTIME: u32 = 0x8008_98FC;
const QUEST_RESIDENT_DEFERRED_OPCODES: [u8; 10] =
    [0xC1, 0x8C, 0xC2, 0x02, 0x3A, 0x3A, 0x02, 0xBC, 0xBC, 0x00];

/// 지원 원본의 OP80 파생 시작 메뉴 path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqSpawnPathSummary {
    pub parent_record_runtime: u32,
    pub spawned_stream_runtime: u32,
    pub path: QuestResidentSeqPath,
    pub deferred_stream_runtime: u32,
    pub deferred_path: QuestResidentSeqPath,
    pub menu_branch_runtime: u32,
    pub menu_selection_field_offset: u8,
    pub menu_selection_count: u16,
    pub new_game_selection_index: usize,
    pub new_game_stream_runtime: u32,
    pub text_glyphs: usize,
    pub text_controls: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqSpawnPathError(String);

impl fmt::Display for QuestResidentSeqSpawnPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentSeqSpawnPathError {}

fn error(message: impl Into<String>) -> QuestResidentSeqSpawnPathError {
    QuestResidentSeqSpawnPathError(message.into())
}

/// 지원 `QUEST.EXE`의 OP80 child seed에서 시작 메뉴 OP09까지를 검증한다.
pub fn verify_box_quest_resident_seq_spawn_path(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    frontiers: &QuestResidentSeqFrontierSummary,
    transition: &QuestResidentSeqTransitionSummary,
) -> Result<QuestResidentSeqSpawnPathSummary, QuestResidentSeqSpawnPathError> {
    let spawn = frontiers
        .frontiers
        .iter()
        .find(|frontier| frontier.spawn_stream_runtime.is_some())
        .ok_or_else(|| error("QUEST resident OP80 spawn frontier가 없음"))?;
    if spawn.record_runtime != QUEST_RESIDENT_SPAWN_PARENT_RUNTIME
        || spawn.spawn_stream_runtime != Some(QUEST_RESIDENT_SPAWN_STREAM_RUNTIME)
    {
        return Err(error(format!(
            "QUEST resident OP80 spawn edge drift: {spawn:?}"
        )));
    }
    let path = walk_resident_stream(
        map,
        text,
        code_table,
        QUEST_RESIDENT_SPAWN_STREAM_RUNTIME,
        0,
    )
    .map_err(|cause| error(cause.to_string()))?;
    let opcodes = path
        .nodes
        .iter()
        .map(|node| node.opcode)
        .collect::<Vec<_>>();
    let terminal = path
        .nodes
        .last()
        .ok_or_else(|| error("QUEST resident spawn path node가 없음"))?;
    let deferred_streams = path
        .nodes
        .iter()
        .filter_map(|node| node.side_stream_runtime)
        .collect::<Vec<_>>();
    if path.install_sites != 0
        || opcodes != [0xC1, 0x21, 0x09]
        || path.status != QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier
        || path.frontier_requirement
            != Some(QuestResidentSeqFrontierRequirement::ObjectBranchSelectorState)
        || terminal.runtime != transition.menu_branch_runtime
        || deferred_streams != [QUEST_RESIDENT_DEFERRED_STREAM_RUNTIME]
        || path.nodes[0].transition != QuestResidentSeqPathTransition::SavedStreamAndAdvance
    {
        return Err(error(format!(
            "QUEST resident spawn path drift: opcodes={opcodes:02X?}, status={:?}, \
             requirement={:?}, terminal=0x{:08X}, deferred={deferred_streams:08X?}",
            path.status, path.frontier_requirement, terminal.runtime,
        )));
    }

    let text_glyphs = path.nodes.iter().map(|node| node.text_glyphs).sum();
    let text_controls = path.nodes.iter().map(|node| node.text_controls).sum();
    if text_glyphs != 23 || text_controls != 19 {
        return Err(error(format!(
            "QUEST resident spawn text lifecycle drift: glyphs={text_glyphs}/23, controls={text_controls}/19"
        )));
    }
    let deferred_path = walk_resident_stream(
        map,
        text,
        code_table,
        QUEST_RESIDENT_DEFERRED_STREAM_RUNTIME,
        0,
    )
    .map_err(|cause| error(cause.to_string()))?;
    let deferred_opcodes = deferred_path
        .nodes
        .iter()
        .map(|node| node.opcode)
        .collect::<Vec<_>>();
    let deferred_terminal = deferred_path
        .nodes
        .last()
        .ok_or_else(|| error("QUEST resident deferred path node가 없음"))?;
    if deferred_path.install_sites != 0
        || deferred_opcodes != QUEST_RESIDENT_DEFERRED_OPCODES
        || deferred_path.status != QuestResidentSeqPathStatus::ReleasedInterpreter
        || deferred_path.frontier_requirement.is_some()
        || deferred_terminal.runtime != QUEST_RESIDENT_DEFERRED_RELEASE_RUNTIME
        || deferred_terminal.transition != QuestResidentSeqPathTransition::ReleaseInterpreter
        || deferred_path
            .nodes
            .iter()
            .any(|node| node.runtime == QUEST_RESIDENT_DEFERRED_FOLLOWING_CELL_TABLE_RUNTIME)
    {
        return Err(error(format!(
            "QUEST resident deferred path drift: opcodes={deferred_opcodes:02X?}, \
             status={:?}, requirement={:?}, terminal=0x{:08X}",
            deferred_path.status, deferred_path.frontier_requirement, deferred_terminal.runtime,
        )));
    }
    Ok(QuestResidentSeqSpawnPathSummary {
        parent_record_runtime: QUEST_RESIDENT_SPAWN_PARENT_RUNTIME,
        spawned_stream_runtime: QUEST_RESIDENT_SPAWN_STREAM_RUNTIME,
        deferred_stream_runtime: QUEST_RESIDENT_DEFERRED_STREAM_RUNTIME,
        deferred_path,
        menu_branch_runtime: transition.menu_branch_runtime,
        menu_selection_field_offset: transition.menu_selection_field_offset,
        menu_selection_count: transition.menu_selection_count,
        new_game_selection_index: transition.new_game_selection_index,
        new_game_stream_runtime: transition.new_game_stream_runtime,
        text_glyphs,
        text_controls,
        path,
    })
}
