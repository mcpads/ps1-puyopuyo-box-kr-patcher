//! `QUEST.EXE` 상주 OP1C child stream의 보수적 정적 graph.
//!
//! literal root path에서 이미 확인한 OP1C edge를 시작점으로 삼고, child를
//! 별도 interpreter seed로 걷는다. child path 안의 OP1C도 다시 별도 edge로
//! 확장하되 이미 방문한 seed는 한 번만 분석한다. pointer 존재는 정상 실행이나
//! parent와 child의 동시 생존을 증명하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::font::QuestCodeTable;

use super::quest_resident_seq_paths::{
    walk_resident_stream, QuestResidentSeqFrontierRequirement, QuestResidentSeqPath,
    QuestResidentSeqPathStatus, QuestResidentSeqPathSummary, QuestResidentSeqPathTransition,
};

const BOX_QUEST_RESIDENT_OP1C_CHILD_ROOTS: [u32; 3] = [0x8008_4D40, 0x8008_9868, 0x8008_C548];
type ChildPathExpectation = (
    u32,
    &'static [u8],
    u32,
    QuestResidentSeqPathStatus,
    Option<QuestResidentSeqFrontierRequirement>,
);
const BOX_QUEST_RESIDENT_OP1C_CHILD_PATHS: [ChildPathExpectation; 3] = [
    (
        0x8008_4D40,
        &[0x11, 0x09],
        0x8008_4D44,
        QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier,
        Some(QuestResidentSeqFrontierRequirement::ObjectBranchSelectorState),
    ),
    (
        0x8008_9868,
        &[0x3A, 0x3A, 0x10, 0xBC, 0x11, 0x00],
        0x8008_9884,
        QuestResidentSeqPathStatus::ReleasedInterpreter,
        None,
    ),
    (
        0x8008_C548,
        &[0x33],
        0x8008_C548,
        QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier,
        Some(QuestResidentSeqFrontierRequirement::HandlerState),
    ),
];

/// OP1C parent path와 별도 child stream 사이의 source edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct QuestResidentSeqChildEdge {
    pub owner_stream_runtime: u32,
    pub spawn_record_runtime: u32,
    pub parent_next_runtime: u32,
    pub child_stream_runtime: u32,
}

/// child seed를 재귀적으로 걷되 각 seed를 한 번만 분석한 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqChildGraphAnalysis {
    pub edges: Vec<QuestResidentSeqChildEdge>,
    pub paths: Vec<QuestResidentSeqPath>,
    pub op21_records: usize,
    pub released_paths: usize,
    pub runtime_frontiers: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentSeqChildGraphError(String);

impl fmt::Display for QuestResidentSeqChildGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentSeqChildGraphError {}

fn error(message: impl Into<String>) -> QuestResidentSeqChildGraphError {
    QuestResidentSeqChildGraphError(message.into())
}

fn collect_child_edges(
    owner_stream_runtime: u32,
    path: &QuestResidentSeqPath,
) -> Result<Vec<QuestResidentSeqChildEdge>, QuestResidentSeqChildGraphError> {
    path.nodes
        .iter()
        .filter_map(|node| {
            node.child_stream_runtime.map(|child_stream_runtime| {
                if node.transition != QuestResidentSeqPathTransition::ChildStreamAndAdvance {
                    return Err(error(format!(
                        "QUEST resident child edge의 전이 종류가 OP1C가 아님: 0x{:08X}",
                        node.runtime
                    )));
                }
                let parent_next_runtime = node.next_runtime.ok_or_else(|| {
                    error(format!(
                        "QUEST resident OP1C parent next가 없음: 0x{:08X}",
                        node.runtime
                    ))
                })?;
                Ok(QuestResidentSeqChildEdge {
                    owner_stream_runtime,
                    spawn_record_runtime: node.runtime,
                    parent_next_runtime,
                    child_stream_runtime,
                })
            })
        })
        .collect()
}

/// parent path의 OP1C child를 별도 seed로 걷고 nested child까지 확장한다.
pub fn audit_quest_resident_seq_child_graph(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    parents: &QuestResidentSeqPathSummary,
) -> Result<QuestResidentSeqChildGraphAnalysis, QuestResidentSeqChildGraphError> {
    let mut edges = BTreeSet::new();
    let mut pending = BTreeSet::new();
    for parent in &parents.paths {
        for edge in collect_child_edges(parent.root_runtime, parent)? {
            pending.insert(edge.child_stream_runtime);
            edges.insert(edge);
        }
    }

    let mut paths = BTreeMap::new();
    while let Some(child_stream_runtime) = pending.pop_first() {
        if paths.contains_key(&child_stream_runtime) {
            continue;
        }
        let path = walk_resident_stream(map, text, code_table, child_stream_runtime, 0).map_err(
            |cause| {
                error(format!(
                    "QUEST resident child path 0x{child_stream_runtime:08X}: {cause}"
                ))
            },
        )?;
        for edge in collect_child_edges(child_stream_runtime, &path)? {
            pending.insert(edge.child_stream_runtime);
            edges.insert(edge);
        }
        paths.insert(child_stream_runtime, path);
    }

    let paths = paths.into_values().collect::<Vec<_>>();
    Ok(QuestResidentSeqChildGraphAnalysis {
        op21_records: paths
            .iter()
            .flat_map(|path| &path.nodes)
            .filter(|node| node.opcode == 0x21)
            .count(),
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
        edges: edges.into_iter().collect(),
        paths,
    })
}

/// 지원 원본의 literal root에서 나온 첫 OP1C child 분모와 graph를 검증한다.
pub fn verify_box_quest_resident_seq_child_graph(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    parents: &QuestResidentSeqPathSummary,
) -> Result<QuestResidentSeqChildGraphAnalysis, QuestResidentSeqChildGraphError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST resident child graph 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len
        )));
    }
    let first_generation = parents
        .paths
        .iter()
        .flat_map(|path| &path.nodes)
        .filter_map(|node| node.child_stream_runtime)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if first_generation != BOX_QUEST_RESIDENT_OP1C_CHILD_ROOTS {
        return Err(error(format!(
            "QUEST resident OP1C child root 분모 drift: expected={BOX_QUEST_RESIDENT_OP1C_CHILD_ROOTS:08X?}, actual={first_generation:08X?}"
        )));
    }
    let analysis = audit_quest_resident_seq_child_graph(map, text, code_table, parents)?;
    if analysis.edges.len() != BOX_QUEST_RESIDENT_OP1C_CHILD_ROOTS.len()
        || analysis.paths.len() != BOX_QUEST_RESIDENT_OP1C_CHILD_ROOTS.len()
        || analysis.op21_records != 0
        || analysis.released_paths != 1
        || analysis.runtime_frontiers != 2
    {
        return Err(error(format!(
            "QUEST resident OP1C child graph 분모 drift: edges {}, paths {}, OP21 {}, released {}, frontiers {}",
            analysis.edges.len(),
            analysis.paths.len(),
            analysis.op21_records,
            analysis.released_paths,
            analysis.runtime_frontiers,
        )));
    }
    for (path, (root, opcodes, endpoint, status, requirement)) in analysis
        .paths
        .iter()
        .zip(BOX_QUEST_RESIDENT_OP1C_CHILD_PATHS)
    {
        let actual_opcodes = path
            .nodes
            .iter()
            .map(|node| node.opcode)
            .collect::<Vec<_>>();
        let actual_endpoint = path.nodes.last().map(|node| node.runtime);
        if path.root_runtime != root
            || actual_opcodes != opcodes
            || actual_endpoint != Some(endpoint)
            || path.status != status
            || path.frontier_requirement != requirement
        {
            return Err(error(format!(
                "QUEST resident OP1C child path drift: root=0x{root:08X}, opcodes={actual_opcodes:02X?}/{opcodes:02X?}, endpoint={actual_endpoint:08X?}/0x{endpoint:08X}, status={:?}/{status:?}, requirement={:?}/{requirement:?}",
                path.status, path.frontier_requirement,
            )));
        }
    }
    Ok(analysis)
}

#[cfg(test)]
#[path = "quest_resident_seq_child_graph_tests.rs"]
mod tests;
