//! `QUEST.EXE` 상주 literal SEQ root의 첫 실행 경계.
//!
//! constructor가 객체 `+0x1C`에 직접 설치하는 네 literal root만 시작점으로
//! 삼는다. handler source로 고정한 전진 폭과 direct branch를 따라가고,
//! OP21은 text lifecycle 종단까지 읽는다. FF03은 정렬된 다음 record로
//! 계속하고 OP00과 FF00/FF1A는 객체 해제를 종단으로 보존한다. OP1C는
//! parent의 8바이트 전진과 record `+4`의 child stream edge를 함께 보존한다. runtime 상태가
//! 필요한 비교·하위 dispatch·stream 전이에서는 후보를 임의 선택하지 않고
//! 멈춘다. 이 결과는 정적 source path이며 정상 실행 도달 증거가 아니다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::disasm::{
    quest_seq_base_fallback_advance, quest_seq_base_record_selected_advance,
    verify_box_quest_resident_seq_roots, QuestSeqBaseTransitionMeaning,
    QUEST_SEQ_BASE_TRANSITION_SPECS,
};
use crate::font::QuestCodeTable;

use super::quest_seq_text_paths::width_profile;
use crate::script::quest_seq_inventory::{QuestSeqPointerBehavior, QUEST_SEQ_POINTER_RECORD_SPECS};
use crate::script::quest_seq_text_streams::{
    parse_quest_text_stream, QuestTextTerminal, QUEST_SEQ_TEXT_MODE_RECORD_BYTES,
};

const MAX_PATH_NODES: usize = 1_024;
const BOX_QUEST_RESIDENT_LITERAL_ROOTS: [u32; 4] =
    [0x8008_97DC, 0x8008_9888, 0x8008_C498, 0x8008_C590];
const BOX_QUEST_RESIDENT_PATH_NODES: usize = 29;
const BOX_QUEST_RESIDENT_PATH_OP21_RECORDS: usize = 2;
const BOX_QUEST_RESIDENT_PATH_TEXT_GLYPHS: usize = 38;
const BOX_QUEST_RESIDENT_PATH_TEXT_CONTROLS: usize = 20;

/// 다음 record를 고르는 데 필요한 runtime 입력.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestResidentSeqFrontierRequirement {
    SubdispatchState,
    CompareState,
    SpawnObjectState,
    StreamTransitionState,
    ObjectBranchSelectorState,
    HandlerState,
}

impl QuestResidentSeqFrontierRequirement {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SubdispatchState => "subdispatch-state",
            Self::CompareState => "compare-state",
            Self::SpawnObjectState => "spawn-object-state",
            Self::StreamTransitionState => "stream-transition-state",
            Self::ObjectBranchSelectorState => "object-branch-selector-state",
            Self::HandlerState => "handler-state",
        }
    }
}

type ResidentPathExpectation = (
    u32,
    usize,
    &'static [u8],
    u32,
    QuestResidentSeqFrontierRequirement,
);

const BOX_QUEST_RESIDENT_PATH_EXPECTATIONS: [ResidentPathExpectation; 4] = [
    (
        0x8008_97DC,
        2,
        &[0xC1, 0x11, 0x1C, 0x02, 0x21, 0x8C, 0x02, 0x01, 0x14],
        0x8008_9854,
        QuestResidentSeqFrontierRequirement::CompareState,
    ),
    (
        0x8008_9888,
        1,
        &[0x3A, 0x3A, 0x02, 0x8C, 0x80],
        0x8008_9898,
        QuestResidentSeqFrontierRequirement::SpawnObjectState,
    ),
    (
        0x8008_C498,
        2,
        &[
            0xA4, 0x90, 0x21, 0x3A, 0x8C, 0x11, 0x1C, 0x02, 0x3A, 0x01, 0x14,
        ],
        0x8008_C518,
        QuestResidentSeqFrontierRequirement::CompareState,
    ),
    (
        0x8008_C590,
        1,
        &[0x11, 0x1C, 0x01, 0x14],
        0x8008_C5A4,
        QuestResidentSeqFrontierRequirement::CompareState,
    ),
];

/// 한 record에서 다음 위치로 옮긴 근거.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestResidentSeqPathTransition {
    DirectAdvance,
    SavedStreamAndAdvance,
    ChildStreamAndAdvance,
    RecordModeAdvance,
    SubdispatchAdvance,
    FallbackAdvance,
    DirectBranch,
    ReleaseInterpreter,
    TextDeactivateAndAlign,
    TextRelease,
    TextNotifyAndRelease,
    RuntimeFrontier,
}

/// 상주 root path의 종단 판정.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestResidentSeqPathStatus {
    ReleasedInterpreter,
    NotifiedParentAndReleased,
    StoppedAtRuntimeFrontier,
}

/// 상주 root에서 방문한 record 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestResidentSeqPathNode {
    pub runtime: u32,
    pub opcode: u8,
    pub transition: QuestResidentSeqPathTransition,
    pub next_runtime: Option<u32>,
    /// OPC1의 지연 stream. OP1C child edge와는 다르다.
    pub side_stream_runtime: Option<u32>,
    /// OP1C가 새 interpreter에 설치하는 record `+4` child stream.
    pub child_stream_runtime: Option<u32>,
    pub text_terminal: Option<QuestTextTerminal>,
    pub text_glyphs: usize,
    pub text_controls: usize,
}

/// literal root 하나의 보수적 첫 path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqPath {
    pub root_runtime: u32,
    pub install_sites: usize,
    pub nodes: Vec<QuestResidentSeqPathNode>,
    pub status: QuestResidentSeqPathStatus,
    pub frontier_requirement: Option<QuestResidentSeqFrontierRequirement>,
}

/// 지원 원본의 네 literal root path 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqPathSummary {
    pub paths: Vec<QuestResidentSeqPath>,
    pub install_sites: usize,
    pub nodes: usize,
    pub op21_records: usize,
    pub text_glyphs: usize,
    pub text_controls: usize,
    pub released_paths: usize,
    pub runtime_frontiers: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqPathError(String);

impl fmt::Display for QuestResidentSeqPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentSeqPathError {}

fn error(message: impl Into<String>) -> QuestResidentSeqPathError {
    QuestResidentSeqPathError(message.into())
}

fn is_direct_branch(opcode: u8) -> bool {
    QUEST_SEQ_BASE_TRANSITION_SPECS.iter().any(|spec| {
        spec.opcode == opcode && spec.meaning == QuestSeqBaseTransitionMeaning::DirectBranchPointer
    })
}

fn stream_transition_opcodes() -> BTreeSet<u8> {
    QUEST_SEQ_POINTER_RECORD_SPECS
        .iter()
        .filter(|spec| {
            matches!(
                spec.behavior,
                QuestSeqPointerBehavior::SpawnStream
                    | QuestSeqPointerBehavior::ScheduledStream
                    | QuestSeqPointerBehavior::SavedStream
                    | QuestSeqPointerBehavior::Branch
                    | QuestSeqPointerBehavior::StackBranch
            )
        })
        .map(|spec| spec.opcode)
        .collect()
}

fn frontier_requirement(opcode: u8) -> QuestResidentSeqFrontierRequirement {
    match opcode {
        0x3A => QuestResidentSeqFrontierRequirement::SubdispatchState,
        0x13 | 0x14 => QuestResidentSeqFrontierRequirement::CompareState,
        0x80 | 0x9D => QuestResidentSeqFrontierRequirement::SpawnObjectState,
        0x09 | 0x0A => QuestResidentSeqFrontierRequirement::ObjectBranchSelectorState,
        _ if stream_transition_opcodes().contains(&opcode) => {
            QuestResidentSeqFrontierRequirement::StreamTransitionState
        }
        _ => QuestResidentSeqFrontierRequirement::HandlerState,
    }
}

fn read_u32(text: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(text.get(offset..end)?.try_into().ok()?))
}

fn selected_advance(
    text: &[u8],
    offset: usize,
    opcode: u8,
) -> Option<(QuestResidentSeqPathTransition, usize)> {
    if let Some(width) = quest_seq_base_fallback_advance(opcode) {
        return Some((QuestResidentSeqPathTransition::FallbackAdvance, width));
    }
    if !matches!(opcode, 0x10 | 0x11 | 0x3A) {
        return None;
    }
    let byte1 = *text.get(offset.checked_add(1)?)?;
    quest_seq_base_record_selected_advance(opcode, byte1).map(|width| {
        let transition = if opcode == 0x3A {
            QuestResidentSeqPathTransition::SubdispatchAdvance
        } else {
            QuestResidentSeqPathTransition::RecordModeAdvance
        };
        (transition, width)
    })
}

fn text_runtime(map: &ExeMap, offset: usize, role: &str) -> Result<u32, QuestResidentSeqPathError> {
    map.text_runtime_addr(offset).ok_or_else(|| {
        error(format!(
            "QUEST resident SEQ {role}가 EXE text 밖임: +0x{offset:X}"
        ))
    })
}

fn push_runtime_frontier(
    path: &mut Vec<QuestResidentSeqPathNode>,
    runtime: u32,
    opcode: u8,
) -> (
    QuestResidentSeqPathStatus,
    QuestResidentSeqFrontierRequirement,
) {
    path.push(QuestResidentSeqPathNode {
        runtime,
        opcode,
        transition: QuestResidentSeqPathTransition::RuntimeFrontier,
        next_runtime: None,
        side_stream_runtime: None,
        child_stream_runtime: None,
        text_terminal: None,
        text_glyphs: 0,
        text_controls: 0,
    });
    (
        QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier,
        frontier_requirement(opcode),
    )
}

pub(super) fn walk_resident_stream(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    root_runtime: u32,
    install_sites: usize,
) -> Result<QuestResidentSeqPath, QuestResidentSeqPathError> {
    let (unique_widths, _) = width_profile();
    let stream_opcodes = stream_transition_opcodes();
    let mut offset = map.text_offset(root_runtime).ok_or_else(|| {
        error(format!(
            "QUEST resident SEQ root가 EXE text 밖임: 0x{root_runtime:08X}"
        ))
    })?;
    let mut visited = BTreeSet::new();
    let mut nodes = Vec::new();

    let (status, frontier_requirement) = loop {
        if nodes.len() >= MAX_PATH_NODES {
            return Err(error(format!(
                "QUEST resident SEQ root 0x{root_runtime:08X}가 {MAX_PATH_NODES}개 record 안에 수렴하지 않음"
            )));
        }
        if !visited.insert(offset) {
            return Err(error(format!(
                "QUEST resident SEQ root 0x{root_runtime:08X} path cycle: +0x{offset:X}"
            )));
        }
        let runtime = text_runtime(map, offset, "record")?;
        let opcode = *text.get(offset).ok_or_else(|| {
            error(format!(
                "QUEST resident SEQ root 0x{root_runtime:08X} record가 잘림: 0x{runtime:08X}"
            ))
        })?;

        if opcode == 0x00 {
            nodes.push(QuestResidentSeqPathNode {
                runtime,
                opcode,
                transition: QuestResidentSeqPathTransition::ReleaseInterpreter,
                next_runtime: None,
                side_stream_runtime: None,
                child_stream_runtime: None,
                text_terminal: None,
                text_glyphs: 0,
                text_controls: 0,
            });
            break (QuestResidentSeqPathStatus::ReleasedInterpreter, None);
        }

        if opcode == 0x21 {
            let record_end = offset
                .checked_add(QUEST_SEQ_TEXT_MODE_RECORD_BYTES)
                .ok_or_else(|| error("QUEST resident OP21 record 범위 overflow"))?;
            let record = text.get(offset..record_end).ok_or_else(|| {
                error(format!(
                    "QUEST resident OP21 record가 잘림: 0x{runtime:08X}"
                ))
            })?;
            if record[1..] != [0, 0, 0] {
                return Err(error(format!(
                    "QUEST resident OP21 미소비 byte가 canonical zero가 아님: 0x{runtime:08X}, raw={record:02X?}"
                )));
            }
            let parsed = parse_quest_text_stream("QUEST.EXE", text, record_end, code_table)
                .map_err(|cause| {
                    error(format!(
                        "QUEST resident OP21 text lifecycle 기각: 0x{runtime:08X}: {cause}"
                    ))
                })?;
            let transition = match parsed.terminal {
                QuestTextTerminal::ReleaseInterpreter => {
                    QuestResidentSeqPathTransition::TextRelease
                }
                QuestTextTerminal::DeactivateAndAlign => {
                    QuestResidentSeqPathTransition::TextDeactivateAndAlign
                }
                QuestTextTerminal::NotifyParentAndRelease => {
                    QuestResidentSeqPathTransition::TextNotifyAndRelease
                }
            };
            let next_runtime = (parsed.terminal == QuestTextTerminal::DeactivateAndAlign)
                .then(|| text_runtime(map, parsed.end_offset, "OP21 다음 record"))
                .transpose()?;
            nodes.push(QuestResidentSeqPathNode {
                runtime,
                opcode,
                transition,
                next_runtime,
                side_stream_runtime: None,
                child_stream_runtime: None,
                text_terminal: Some(parsed.terminal),
                text_glyphs: parsed.glyph_count,
                text_controls: parsed.control_count,
            });
            match parsed.terminal {
                QuestTextTerminal::ReleaseInterpreter => {
                    break (QuestResidentSeqPathStatus::ReleasedInterpreter, None);
                }
                QuestTextTerminal::NotifyParentAndRelease => {
                    break (QuestResidentSeqPathStatus::NotifiedParentAndReleased, None);
                }
                QuestTextTerminal::DeactivateAndAlign => {
                    offset = parsed.end_offset;
                    continue;
                }
            }
        }

        if is_direct_branch(opcode) {
            let pointer_offset = offset
                .checked_add(4)
                .ok_or_else(|| error("QUEST resident direct branch pointer overflow"))?;
            let target = read_u32(text, pointer_offset).ok_or_else(|| {
                error(format!(
                    "QUEST resident direct branch pointer가 잘림: 0x{runtime:08X}"
                ))
            })?;
            let target_offset = map.text_offset(target).ok_or_else(|| {
                error(format!(
                    "QUEST resident direct branch target이 EXE text 밖임: 0x{runtime:08X}->0x{target:08X}"
                ))
            })?;
            nodes.push(QuestResidentSeqPathNode {
                runtime,
                opcode,
                transition: QuestResidentSeqPathTransition::DirectBranch,
                next_runtime: Some(target),
                side_stream_runtime: None,
                child_stream_runtime: None,
                text_terminal: None,
                text_glyphs: 0,
                text_controls: 0,
            });
            offset = target_offset;
            continue;
        }

        if opcode == 0x1C {
            let child_pointer_offset = offset
                .checked_add(4)
                .ok_or_else(|| error("QUEST resident OP1C child pointer offset overflow"))?;
            let child_runtime = read_u32(text, child_pointer_offset).ok_or_else(|| {
                error(format!(
                    "QUEST resident OP1C child stream pointer가 잘림: 0x{runtime:08X}"
                ))
            })?;
            map.text_offset(child_runtime).ok_or_else(|| {
                error(format!(
                    "QUEST resident OP1C child stream이 EXE text 밖임: 0x{runtime:08X}->0x{child_runtime:08X}"
                ))
            })?;
            let next_offset = offset
                .checked_add(8)
                .ok_or_else(|| error("QUEST resident OP1C parent record 증가 overflow"))?;
            let next_runtime = text_runtime(map, next_offset, "OP1C parent 다음 record")?;
            nodes.push(QuestResidentSeqPathNode {
                runtime,
                opcode,
                transition: QuestResidentSeqPathTransition::ChildStreamAndAdvance,
                next_runtime: Some(next_runtime),
                side_stream_runtime: None,
                child_stream_runtime: Some(child_runtime),
                text_terminal: None,
                text_glyphs: 0,
                text_controls: 0,
            });
            offset = next_offset;
            continue;
        }

        let selected = selected_advance(text, offset, opcode);
        let c1_saved_stream = if opcode == 0xC1
            && text.get(offset + 1).is_some_and(|byte| *byte != 0)
        {
            let pointer_offset = offset
                .checked_add(4)
                .ok_or_else(|| error("QUEST resident OPC1 saved stream offset overflow"))?;
            let target = read_u32(text, pointer_offset)
                .ok_or_else(|| error("QUEST resident OPC1 saved stream pointer가 잘림"))?;
            if !map.contains(target) {
                return Err(error(format!(
                    "QUEST resident OPC1 saved stream이 EXE text 밖임: 0x{runtime:08X}->0x{target:08X}"
                )));
            }
            Some(target)
        } else {
            None
        };
        let (transition, width) = if let Some(selected) = selected {
            selected
        } else if opcode == 0xC1 {
            (
                if c1_saved_stream.is_some() {
                    QuestResidentSeqPathTransition::SavedStreamAndAdvance
                } else {
                    QuestResidentSeqPathTransition::DirectAdvance
                },
                *unique_widths
                    .get(&opcode)
                    .ok_or_else(|| error("QUEST resident OPC1 직접 전진 폭이 없음"))?,
            )
        } else if stream_opcodes.contains(&opcode) {
            let (status, requirement) = push_runtime_frontier(&mut nodes, runtime, opcode);
            break (status, Some(requirement));
        } else if let Some(&width) = unique_widths.get(&opcode) {
            (QuestResidentSeqPathTransition::DirectAdvance, width)
        } else {
            let (status, requirement) = push_runtime_frontier(&mut nodes, runtime, opcode);
            break (status, Some(requirement));
        };
        let next_offset = offset.checked_add(width).ok_or_else(|| {
            error(format!(
                "QUEST resident SEQ record 증가 overflow: 0x{runtime:08X}+{width}"
            ))
        })?;
        let next_runtime = text_runtime(map, next_offset, "다음 record")?;
        nodes.push(QuestResidentSeqPathNode {
            runtime,
            opcode,
            transition,
            next_runtime: Some(next_runtime),
            side_stream_runtime: c1_saved_stream,
            child_stream_runtime: None,
            text_terminal: None,
            text_glyphs: 0,
            text_controls: 0,
        });
        offset = next_offset;
    };

    Ok(QuestResidentSeqPath {
        root_runtime,
        install_sites,
        nodes,
        status,
        frontier_requirement,
    })
}

/// 지원 `QUEST.EXE`의 네 literal root 첫 path를 검증한다.
pub fn verify_box_quest_resident_seq_paths(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
) -> Result<QuestResidentSeqPathSummary, QuestResidentSeqPathError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST resident SEQ path 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len
        )));
    }
    let roots =
        verify_box_quest_resident_seq_roots(map, text).map_err(|cause| error(cause.to_string()))?;
    if roots.literal_roots != BOX_QUEST_RESIDENT_LITERAL_ROOTS {
        return Err(error(format!(
            "QUEST resident SEQ literal root 분모 drift: expected={BOX_QUEST_RESIDENT_LITERAL_ROOTS:08X?}, actual={:08X?}",
            roots.literal_roots
        )));
    }
    let mut paths = roots
        .literal_roots
        .iter()
        .map(|&root_runtime| {
            let install_sites = roots
                .installs
                .iter()
                .filter(|install| install.literal_root_runtime == Some(root_runtime))
                .count();
            walk_resident_stream(map, text, code_table, root_runtime, install_sites)
        })
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort_unstable_by_key(|path| path.root_runtime);

    let summary = QuestResidentSeqPathSummary {
        install_sites: paths.iter().map(|path| path.install_sites).sum(),
        nodes: paths.iter().map(|path| path.nodes.len()).sum(),
        op21_records: paths
            .iter()
            .flat_map(|path| &path.nodes)
            .filter(|node| node.opcode == 0x21)
            .count(),
        text_glyphs: paths
            .iter()
            .flat_map(|path| &path.nodes)
            .map(|node| node.text_glyphs)
            .sum(),
        text_controls: paths
            .iter()
            .flat_map(|path| &path.nodes)
            .map(|node| node.text_controls)
            .sum(),
        released_paths: paths
            .iter()
            .filter(|path| {
                matches!(
                    path.status,
                    QuestResidentSeqPathStatus::ReleasedInterpreter
                        | QuestResidentSeqPathStatus::NotifiedParentAndReleased
                )
            })
            .count(),
        runtime_frontiers: paths
            .iter()
            .filter(|path| path.status == QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier)
            .count(),
        paths,
    };

    if summary.install_sites != 6 {
        return Err(error(format!(
            "QUEST resident SEQ literal install 분모 drift: expected=6, actual={}",
            summary.install_sites
        )));
    }
    if summary.nodes != BOX_QUEST_RESIDENT_PATH_NODES
        || summary.op21_records != BOX_QUEST_RESIDENT_PATH_OP21_RECORDS
        || summary.text_glyphs != BOX_QUEST_RESIDENT_PATH_TEXT_GLYPHS
        || summary.text_controls != BOX_QUEST_RESIDENT_PATH_TEXT_CONTROLS
        || summary.released_paths != 0
        || summary.runtime_frontiers != BOX_QUEST_RESIDENT_LITERAL_ROOTS.len()
    {
        return Err(error(format!(
            "QUEST resident SEQ path 분모 drift: nodes {}/{}, OP21 {}/{}, \
             glyphs {}/{}, controls {}/{}, released {}, frontiers {}/{}",
            summary.nodes,
            BOX_QUEST_RESIDENT_PATH_NODES,
            summary.op21_records,
            BOX_QUEST_RESIDENT_PATH_OP21_RECORDS,
            summary.text_glyphs,
            BOX_QUEST_RESIDENT_PATH_TEXT_GLYPHS,
            summary.text_controls,
            BOX_QUEST_RESIDENT_PATH_TEXT_CONTROLS,
            summary.released_paths,
            summary.runtime_frontiers,
            BOX_QUEST_RESIDENT_LITERAL_ROOTS.len(),
        )));
    }
    for (path, (root, installs, opcodes, endpoint, requirement)) in summary
        .paths
        .iter()
        .zip(BOX_QUEST_RESIDENT_PATH_EXPECTATIONS)
    {
        let actual_opcodes = path
            .nodes
            .iter()
            .map(|node| node.opcode)
            .collect::<Vec<_>>();
        let actual_endpoint = path.nodes.last().map(|node| node.runtime);
        if path.root_runtime != root
            || path.install_sites != installs
            || actual_opcodes != opcodes
            || actual_endpoint != Some(endpoint)
            || path.status != QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier
            || path.frontier_requirement != Some(requirement)
        {
            return Err(error(format!(
                "QUEST resident SEQ path drift: root=0x{root:08X}, \
                 installs={}/{installs}, opcodes={actual_opcodes:02X?}/{opcodes:02X?}, \
                 endpoint={actual_endpoint:08X?}/0x{endpoint:08X}, \
                 status={:?}, requirement={:?}/{requirement:?}",
                path.install_sites, path.status, path.frontier_requirement,
            )));
        }
    }
    Ok(summary)
}

#[cfg(test)]
#[path = "quest_resident_seq_paths_tests.rs"]
mod tests;
