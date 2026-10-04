//! QUEST OP21 정적 path의 고유 중단 frontier.
//!
//! 여러 seed path가 같은 `(SEQ, offset, status)`에 합류하는 중복을
//! 제거하고, 다음 판정에 필요한 runtime 입력 종류를 붙인다. OP0D는
//! record `+1`의 전역 word index로 정확한 감시 주소를 계산한다.
//! OP13/14는 source-gated 비교 layout으로 operand 원천·연산·branch와
//! fallthrough 후보를 보존한다. 이 report는 정상 실행 도달을 주장하지
//! 않으며 runtime 관측 계획의 입력이다. OP00 interpreter release는
//! 실행 입력이 필요한 frontier가 아니므로 이 원장에서 제외한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::RuntimeAssetMap;
use crate::disasm::{
    quest_seq_base_compare_record_layout, QuestSeqBaseCompareOperandSource,
    QuestSeqBaseCompareOperation, QUEST_SEQ_BRANCH_STATE_BASE_RUNTIME,
};

use super::quest_seq_text_paths::{QuestSeqTextPathAnalysis, QuestSeqTextPathStatus};
use crate::script::quest_seq::QUEST_SEQ_ACTIVE_RUNTIME_BASE;
#[cfg(test)]
use crate::script::quest_seq::TOWN00_SEQ_RUNTIME_BASE;
use crate::script::quest_seq_text_roots::QuestSeqTextRootBaseEvidence;

pub const QUEST_SEQ_TEXT_FRONTIER_SCHEMA_VERSION: &str = "quest-seq-text-frontiers-v4";
pub const BOX_QUEST_SEQ_TEXT_FRONTIER_ROWS: usize = 223;
pub const BOX_QUEST_SEQ_TEXT_FRONTIER_PATH_OCCURRENCES: usize = 245;
pub const BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_RECORDS: usize = 24;
pub const BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_OBJECT_RECORDS: usize = 24;
pub const BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_ARGUMENT_RECORDS: usize = 0;
pub const BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_IN_RANGE_BRANCH_TARGETS: usize = 24;
pub const BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_OPERATION_EXPECTATIONS: [(&str, usize); 14] = [
    ("not-equal", 6),
    ("equal", 17),
    ("unsigned-less", 0),
    ("unsigned-greater", 0),
    ("unsigned-less-or-equal", 1),
    ("unsigned-greater-or-equal", 0),
    ("bit-and-nonzero", 0),
    ("bit-or-nonzero", 0),
    ("xor-nonzero", 0),
    ("left-nonzero", 0),
    ("signed-less", 0),
    ("signed-greater", 0),
    ("signed-less-or-equal", 0),
    ("signed-greater-or-equal", 0),
];
pub const BOX_QUEST_SEQ_TEXT_FRONTIER_REPORT_SHA256: &str =
    "6861f63c8fbfedae40a8b3b2980b38b1d28d35e727c677f5a047efb7d705cc53";

/// `(opcode, 고유 frontier, path occurrence)`.
pub const BOX_QUEST_SEQ_TEXT_FRONTIER_OPCODE_EXPECTATIONS: [(u8, usize, usize); 16] = [
    (0x03, 20, 20),
    (0x09, 1, 1),
    (0x0C, 0, 0),
    (0x0D, 13, 27),
    (0x13, 2, 2),
    (0x14, 22, 22),
    (0x80, 0, 0),
    (0x83, 18, 20),
    (0x84, 4, 4),
    (0x89, 2, 2),
    (0x8A, 16, 16),
    (0x95, 15, 15),
    (0x9D, 7, 7),
    (0xA1, 97, 103),
    (0xA8, 6, 6),
    (0xC0, 0, 0),
];

/// `(runtime 입력 종류, 고유 frontier, path occurrence)`.
pub const BOX_QUEST_SEQ_TEXT_FRONTIER_REQUIREMENT_EXPECTATIONS: [(&str, usize, usize); 14] = [
    ("candidate-path-condition-selection", 20, 20),
    ("global-state-word", 13, 27),
    ("handler-branch-condition", 7, 7),
    ("handler-selector-state", 97, 103),
    ("global-branch-condition", 2, 2),
    ("compare-handler-argument-state", 0, 0),
    ("compare-global-state", 0, 0),
    ("compare-object-state", 24, 24),
    ("spawn-object-state", 7, 7),
    ("scheduler-object-state", 18, 20),
    ("object-saved-stream", 19, 19),
    ("preset-stream-state", 0, 0),
    ("resource-transition-state", 16, 16),
    ("return-stack-branch-condition", 0, 0),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqTextFrontierRequirement {
    CandidatePathConditionSelection,
    GlobalStateWord,
    HandlerBranchCondition,
    HandlerSelectorState,
    GlobalBranchCondition,
    CompareHandlerArgumentState,
    CompareGlobalState,
    CompareObjectState,
    SpawnObjectState,
    SchedulerObjectState,
    ObjectSavedStream,
    PresetStreamState,
    ResourceTransitionState,
    ReturnStackBranchCondition,
}

impl QuestSeqTextFrontierRequirement {
    pub const fn label(self) -> &'static str {
        match self {
            Self::CandidatePathConditionSelection => "candidate-path-condition-selection",
            Self::GlobalStateWord => "global-state-word",
            Self::HandlerBranchCondition => "handler-branch-condition",
            Self::HandlerSelectorState => "handler-selector-state",
            Self::GlobalBranchCondition => "global-branch-condition",
            Self::CompareHandlerArgumentState => "compare-handler-argument-state",
            Self::CompareGlobalState => "compare-global-state",
            Self::CompareObjectState => "compare-object-state",
            Self::SpawnObjectState => "spawn-object-state",
            Self::SchedulerObjectState => "scheduler-object-state",
            Self::ObjectSavedStream => "object-saved-stream",
            Self::PresetStreamState => "preset-stream-state",
            Self::ResourceTransitionState => "resource-transition-state",
            Self::ReturnStackBranchCondition => "return-stack-branch-condition",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqTextFrontierParameter {
    GlobalStateWordIndex { index: u8 },
    ResourceId { id: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqTextFrontierCompare {
    pub operation: QuestSeqBaseCompareOperation,
    pub value_bytes: usize,
    pub left_source: QuestSeqBaseCompareOperandSource,
    pub left_parameter: Option<u32>,
    pub right_source: QuestSeqBaseCompareOperandSource,
    pub right_parameter: u32,
    pub branch_target_runtime: u32,
    pub branch_target_offset: Option<usize>,
    pub fallthrough_runtime: u32,
    pub fallthrough_offset: usize,
}

impl QuestSeqTextFrontierParameter {
    pub const fn kind(self) -> &'static str {
        match self {
            Self::GlobalStateWordIndex { .. } => "global-state-word-index",
            Self::ResourceId { .. } => "resource-id",
        }
    }

    pub const fn offset(self) -> usize {
        match self {
            Self::GlobalStateWordIndex { .. } => 1,
            Self::ResourceId { .. } => 2,
        }
    }

    pub const fn value(self) -> u32 {
        match self {
            Self::GlobalStateWordIndex { index } => index as u32,
            Self::ResourceId { id } => id as u32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextFrontier {
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub terminal_offset: usize,
    pub terminal_runtime: u32,
    pub base_evidence: QuestSeqTextRootBaseEvidence,
    pub opcode: u8,
    pub status: QuestSeqTextPathStatus,
    pub terminal_raw: Vec<u8>,
    pub path_occurrences: usize,
    pub seed_offsets: Vec<usize>,
    pub source_opcodes: Vec<u8>,
    pub requirement: QuestSeqTextFrontierRequirement,
    pub parameter: Option<QuestSeqTextFrontierParameter>,
    pub compare: Option<QuestSeqTextFrontierCompare>,
    pub runtime_probe_address: Option<u32>,
    pub runtime_probe_length: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextFrontierAnalysis {
    pub frontiers: Vec<QuestSeqTextFrontier>,
    pub path_occurrences: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextFrontierError(String);

impl fmt::Display for QuestSeqTextFrontierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestSeqTextFrontierError {}

fn error(message: impl Into<String>) -> QuestSeqTextFrontierError {
    QuestSeqTextFrontierError(message.into())
}

fn requirement(
    status: QuestSeqTextPathStatus,
    opcode: u8,
    compare: Option<QuestSeqTextFrontierCompare>,
) -> Result<QuestSeqTextFrontierRequirement, QuestSeqTextFrontierError> {
    if status == QuestSeqTextPathStatus::StoppedAtCycle {
        return Ok(QuestSeqTextFrontierRequirement::CandidatePathConditionSelection);
    }
    if let Some(compare) = compare {
        let sources = [compare.left_source, compare.right_source];
        if sources.contains(&QuestSeqBaseCompareOperandSource::HandlerArgumentA2) {
            return Ok(QuestSeqTextFrontierRequirement::CompareHandlerArgumentState);
        }
        if sources.iter().any(|source| {
            matches!(
                source,
                QuestSeqBaseCompareOperandSource::ObjectField
                    | QuestSeqBaseCompareOperandSource::ObjectIndirectField
            )
        }) {
            return Ok(QuestSeqTextFrontierRequirement::CompareObjectState);
        }
        if sources.contains(&QuestSeqBaseCompareOperandSource::GlobalState) {
            return Ok(QuestSeqTextFrontierRequirement::CompareGlobalState);
        }
        return Err(error(format!(
            "QUEST SEQ OP{opcode:02X} 비교 runtime 입력 원천이 없음"
        )));
    }
    match opcode {
        0x0D => Ok(QuestSeqTextFrontierRequirement::GlobalStateWord),
        0x80 | 0x9D => Ok(QuestSeqTextFrontierRequirement::SpawnObjectState),
        0x83 => Ok(QuestSeqTextFrontierRequirement::SchedulerObjectState),
        0x84 | 0x95 | 0xC1 => Ok(QuestSeqTextFrontierRequirement::ObjectSavedStream),
        0x89 => Ok(QuestSeqTextFrontierRequirement::GlobalBranchCondition),
        0x8A => Ok(QuestSeqTextFrontierRequirement::ResourceTransitionState),
        0xA1 => Ok(QuestSeqTextFrontierRequirement::HandlerSelectorState),
        0xA2 | 0xA9 => Ok(QuestSeqTextFrontierRequirement::ReturnStackBranchCondition),
        0xA8 => Ok(QuestSeqTextFrontierRequirement::HandlerBranchCondition),
        0xC0 => Ok(QuestSeqTextFrontierRequirement::PresetStreamState),
        _ if status == QuestSeqTextPathStatus::StoppedWithoutUniqueDirectWidth => {
            Ok(QuestSeqTextFrontierRequirement::HandlerBranchCondition)
        }
        _ => Err(error(format!(
            "QUEST SEQ text frontier runtime 입력 분류가 없음: \
             status={}, opcode=0x{opcode:02X}",
            status.label()
        ))),
    }
}

fn read_compare_value(
    raw: &[u8],
    start: usize,
    value_bytes: usize,
) -> Result<u32, QuestSeqTextFrontierError> {
    let end = start
        .checked_add(value_bytes)
        .ok_or_else(|| error("QUEST SEQ 비교 operand 끝 offset overflow"))?;
    let bytes = raw
        .get(start..end)
        .ok_or_else(|| error("QUEST SEQ 비교 operand가 잘림"))?;
    match value_bytes {
        2 => Ok(u32::from(u16::from_le_bytes(
            bytes.try_into().expect("2바이트 범위를 확인함"),
        ))),
        4 => Ok(u32::from_le_bytes(
            bytes.try_into().expect("4바이트 범위를 확인함"),
        )),
        _ => Err(error(format!(
            "QUEST SEQ 비교 operand 폭이 지원 범위 밖임: {value_bytes}"
        ))),
    }
}

fn compare_record(
    raw: &[u8],
    offset: usize,
    opcode: u8,
    map: RuntimeAssetMap,
) -> Result<Option<QuestSeqTextFrontierCompare>, QuestSeqTextFrontierError> {
    if !matches!(opcode, 0x13 | 0x14) {
        return Ok(None);
    }
    let byte1_offset = offset
        .checked_add(1)
        .ok_or_else(|| error("QUEST SEQ 비교 selector offset overflow"))?;
    let byte1 = *raw
        .get(byte1_offset)
        .ok_or_else(|| error("QUEST SEQ 비교 selector가 잘림"))?;
    let layout = quest_seq_base_compare_record_layout(opcode, byte1).ok_or_else(|| {
        error(format!(
            "QUEST SEQ OP{opcode:02X} 비교 selector가 지원 범위 밖임: 0x{byte1:02X}"
        ))
    })?;

    let left_parameter = layout
        .left_parameter_offset
        .map(|relative| {
            offset
                .checked_add(relative)
                .and_then(|at| raw.get(at))
                .copied()
                .map(u32::from)
                .ok_or_else(|| error("QUEST SEQ 비교 left parameter가 잘림"))
        })
        .transpose()?;
    let right_at = offset
        .checked_add(layout.right_parameter_offset)
        .ok_or_else(|| error("QUEST SEQ 비교 right parameter offset overflow"))?;
    let right_parameter = if layout.right_source == QuestSeqBaseCompareOperandSource::Inline {
        read_compare_value(raw, right_at, layout.value_bytes)?
    } else {
        u32::from(
            *raw.get(right_at)
                .ok_or_else(|| error("QUEST SEQ 비교 right parameter가 잘림"))?,
        )
    };

    let pointer_at = offset
        .checked_add(layout.branch_pointer_offset)
        .ok_or_else(|| error("QUEST SEQ 비교 branch pointer offset overflow"))?;
    let pointer_end = pointer_at
        .checked_add(4)
        .ok_or_else(|| error("QUEST SEQ 비교 branch pointer 끝 overflow"))?;
    let branch_target_runtime = u32::from_le_bytes(
        raw.get(pointer_at..pointer_end)
            .ok_or_else(|| error("QUEST SEQ 비교 branch pointer가 잘림"))?
            .try_into()
            .expect("4바이트 범위를 확인함"),
    );
    let fallthrough_offset = offset
        .checked_add(layout.fallthrough_offset)
        .ok_or_else(|| error("QUEST SEQ 비교 fallthrough offset overflow"))?;
    let fallthrough_runtime = map.runtime_addr(fallthrough_offset).ok_or_else(|| {
        error(format!(
            "QUEST SEQ 비교 fallthrough가 파일 밖임: 0x{fallthrough_offset:04X}"
        ))
    })?;

    Ok(Some(QuestSeqTextFrontierCompare {
        operation: layout.operation,
        value_bytes: layout.value_bytes,
        left_source: layout.left_source,
        left_parameter,
        right_source: layout.right_source,
        right_parameter,
        branch_target_runtime,
        branch_target_offset: map.file_offset(branch_target_runtime),
        fallthrough_runtime,
        fallthrough_offset,
    }))
}

fn parameter(
    raw: &[u8],
    offset: usize,
    opcode: u8,
) -> Result<Option<QuestSeqTextFrontierParameter>, QuestSeqTextFrontierError> {
    match opcode {
        0x0D => {
            let index = *raw
                .get(offset.checked_add(1).ok_or_else(|| {
                    error("QUEST SEQ text frontier OP0D parameter offset overflow")
                })?)
                .ok_or_else(|| error("QUEST SEQ text frontier OP0D index가 잘림"))?;
            Ok(Some(QuestSeqTextFrontierParameter::GlobalStateWordIndex {
                index,
            }))
        }
        0x8A => {
            let start = offset
                .checked_add(2)
                .ok_or_else(|| error("QUEST SEQ text frontier OP8A parameter offset overflow"))?;
            let end = start
                .checked_add(2)
                .ok_or_else(|| error("QUEST SEQ text frontier OP8A parameter end overflow"))?;
            let id = u16::from_le_bytes(
                raw.get(start..end)
                    .ok_or_else(|| error("QUEST SEQ text frontier OP8A resource ID가 잘림"))?
                    .try_into()
                    .expect("2바이트 범위를 확인함"),
            );
            Ok(Some(QuestSeqTextFrontierParameter::ResourceId { id }))
        }
        _ => Ok(None),
    }
}

fn probe_address(
    parameter: Option<QuestSeqTextFrontierParameter>,
) -> Result<Option<u32>, QuestSeqTextFrontierError> {
    let Some(QuestSeqTextFrontierParameter::GlobalStateWordIndex { index }) = parameter else {
        return Ok(None);
    };
    QUEST_SEQ_BRANCH_STATE_BASE_RUNTIME
        .checked_add(u32::from(index) * 4)
        .map(Some)
        .ok_or_else(|| error("QUEST SEQ OP0D runtime probe 주소 overflow"))
}

#[derive(Debug)]
struct FrontierBuilder {
    sequence_sha256: [u8; 32],
    terminal_runtime: u32,
    base_evidence: QuestSeqTextRootBaseEvidence,
    opcode: u8,
    status: QuestSeqTextPathStatus,
    terminal_raw: Vec<u8>,
    path_occurrences: usize,
    seed_offsets: BTreeSet<usize>,
    source_opcodes: BTreeSet<u8>,
    requirement: QuestSeqTextFrontierRequirement,
    parameter: Option<QuestSeqTextFrontierParameter>,
    compare: Option<QuestSeqTextFrontierCompare>,
    runtime_probe_address: Option<u32>,
}

/// 정적 path 중단을 고유 `(SEQ, offset, status)` frontier로 합친다.
pub fn audit_quest_seq_text_frontiers<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    paths: &QuestSeqTextPathAnalysis,
) -> Result<QuestSeqTextFrontierAnalysis, QuestSeqTextFrontierError> {
    let mut input = BTreeMap::new();
    for (name, raw) in sequences {
        if name.is_empty() {
            return Err(error("QUEST SEQ text frontier 입력 이름이 비어 있음"));
        }
        if input.insert(name.to_string(), raw).is_some() {
            return Err(error(format!(
                "QUEST SEQ text frontier 입력 이름 중복: {name}"
            )));
        }
    }
    if input.is_empty() {
        return Err(error("QUEST SEQ text frontier 입력이 없음"));
    }

    let mut grouped = BTreeMap::<(String, usize, &'static str), FrontierBuilder>::new();
    let mut path_occurrences = 0usize;
    for path in paths
        .paths
        .iter()
        .filter(|path| path.status.is_runtime_frontier())
    {
        path_occurrences += 1;
        let terminal = path
            .nodes
            .last()
            .ok_or_else(|| error(format!("{}: 중단 path node가 없음", path.sequence)))?;
        let raw = input
            .get(&path.sequence)
            .ok_or_else(|| error(format!("{}: frontier 원본이 없음", path.sequence)))?;
        let raw_len = u32::try_from(raw.len())
            .map_err(|_| error(format!("{}: SEQ 길이가 u32 범위를 넘음", path.sequence)))?;
        let map = RuntimeAssetMap::new(QUEST_SEQ_ACTIVE_RUNTIME_BASE, raw_len)
            .ok_or_else(|| error(format!("{}: SEQ runtime 사상 overflow", path.sequence)))?;
        let terminal_runtime = map.runtime_addr(terminal.offset).ok_or_else(|| {
            error(format!(
                "{}: frontier offset이 파일 밖임: 0x{:04X}",
                path.sequence, terminal.offset
            ))
        })?;
        let terminal_end = terminal.offset.saturating_add(16).min(raw.len());
        let compare = compare_record(raw, terminal.offset, terminal.opcode, map)?;
        let requirement = requirement(path.status, terminal.opcode, compare)?;
        let parameter = parameter(raw, terminal.offset, terminal.opcode)?;
        let runtime_probe_address = probe_address(parameter)?;
        let key = (path.sequence.clone(), terminal.offset, path.status.label());
        let builder = grouped.entry(key).or_insert_with(|| FrontierBuilder {
            sequence_sha256: Sha256::digest(*raw).into(),
            terminal_runtime,
            base_evidence: path.base_evidence,
            opcode: terminal.opcode,
            status: path.status,
            terminal_raw: raw[terminal.offset..terminal_end].to_vec(),
            path_occurrences: 0,
            seed_offsets: BTreeSet::new(),
            source_opcodes: BTreeSet::new(),
            requirement,
            parameter,
            compare,
            runtime_probe_address,
        });
        if builder.opcode != terminal.opcode
            || builder.requirement != requirement
            || builder.parameter != parameter
            || builder.compare != compare
            || builder.runtime_probe_address != runtime_probe_address
        {
            return Err(error(format!(
                "{}: 같은 frontier의 의미가 충돌함: 0x{:04X}",
                path.sequence, terminal.offset
            )));
        }
        builder.path_occurrences += 1;
        builder.seed_offsets.insert(path.seed_offset);
        builder
            .source_opcodes
            .extend(path.sources.iter().map(|source| source.opcode));
    }

    let frontiers = grouped
        .into_iter()
        .map(
            |((sequence, terminal_offset, _), builder)| QuestSeqTextFrontier {
                sequence,
                sequence_sha256: builder.sequence_sha256,
                terminal_offset,
                terminal_runtime: builder.terminal_runtime,
                base_evidence: builder.base_evidence,
                opcode: builder.opcode,
                status: builder.status,
                terminal_raw: builder.terminal_raw,
                path_occurrences: builder.path_occurrences,
                seed_offsets: builder.seed_offsets.into_iter().collect(),
                source_opcodes: builder.source_opcodes.into_iter().collect(),
                requirement: builder.requirement,
                parameter: builder.parameter,
                compare: builder.compare,
                runtime_probe_address: builder.runtime_probe_address,
                runtime_probe_length: builder.runtime_probe_address.map(|_| 4),
            },
        )
        .collect();
    Ok(QuestSeqTextFrontierAnalysis {
        frontiers,
        path_occurrences,
    })
}

/// 지원 원본의 frontier 분모와 결정적 report digest를 검증한다.
pub fn verify_box_quest_seq_text_frontiers<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    paths: &QuestSeqTextPathAnalysis,
) -> Result<QuestSeqTextFrontierAnalysis, QuestSeqTextFrontierError> {
    let analysis = audit_quest_seq_text_frontiers(sequences, paths)?;
    if analysis.frontiers.len() != BOX_QUEST_SEQ_TEXT_FRONTIER_ROWS
        || analysis.path_occurrences != BOX_QUEST_SEQ_TEXT_FRONTIER_PATH_OCCURRENCES
    {
        return Err(error(format!(
            "QUEST SEQ text frontier 분모 불일치: expected={}/{}, actual={}/{}",
            BOX_QUEST_SEQ_TEXT_FRONTIER_ROWS,
            BOX_QUEST_SEQ_TEXT_FRONTIER_PATH_OCCURRENCES,
            analysis.frontiers.len(),
            analysis.path_occurrences,
        )));
    }

    let actual_opcodes = BOX_QUEST_SEQ_TEXT_FRONTIER_OPCODE_EXPECTATIONS
        .iter()
        .map(|&(opcode, _, _)| {
            let matching = analysis
                .frontiers
                .iter()
                .filter(|frontier| frontier.opcode == opcode)
                .collect::<Vec<_>>();
            (
                opcode,
                matching.len(),
                matching
                    .iter()
                    .map(|frontier| frontier.path_occurrences)
                    .sum(),
            )
        })
        .collect::<Vec<_>>();
    if actual_opcodes != BOX_QUEST_SEQ_TEXT_FRONTIER_OPCODE_EXPECTATIONS {
        return Err(error(format!(
            "QUEST SEQ text frontier opcode 분모 불일치: \
             expected={BOX_QUEST_SEQ_TEXT_FRONTIER_OPCODE_EXPECTATIONS:?}, \
             actual={actual_opcodes:?}"
        )));
    }
    let covered_rows = actual_opcodes.iter().map(|row| row.1).sum::<usize>();
    if covered_rows != analysis.frontiers.len() {
        let registered = BOX_QUEST_SEQ_TEXT_FRONTIER_OPCODE_EXPECTATIONS
            .iter()
            .map(|row| row.0)
            .collect::<BTreeSet<_>>();
        let mut uncovered = BTreeMap::<u8, (usize, usize)>::new();
        for frontier in analysis
            .frontiers
            .iter()
            .filter(|frontier| !registered.contains(&frontier.opcode))
        {
            let counts = uncovered.entry(frontier.opcode).or_default();
            counts.0 += 1;
            counts.1 += frontier.path_occurrences;
        }
        return Err(error(format!(
            "QUEST SEQ text frontier 미등록 opcode가 있음: \
             covered={covered_rows}, actual={}, uncovered={uncovered:?}",
            analysis.frontiers.len(),
        )));
    }

    let actual_requirements = BOX_QUEST_SEQ_TEXT_FRONTIER_REQUIREMENT_EXPECTATIONS
        .iter()
        .map(|&(label, _, _)| {
            let matching = analysis
                .frontiers
                .iter()
                .filter(|frontier| frontier.requirement.label() == label)
                .collect::<Vec<_>>();
            (
                label,
                matching.len(),
                matching
                    .iter()
                    .map(|frontier| frontier.path_occurrences)
                    .sum(),
            )
        })
        .collect::<Vec<_>>();
    if actual_requirements != BOX_QUEST_SEQ_TEXT_FRONTIER_REQUIREMENT_EXPECTATIONS {
        return Err(error(format!(
            "QUEST SEQ text frontier runtime 입력 분모 불일치: \
             expected={BOX_QUEST_SEQ_TEXT_FRONTIER_REQUIREMENT_EXPECTATIONS:?}, \
             actual={actual_requirements:?}"
        )));
    }

    let compare_records = analysis
        .frontiers
        .iter()
        .filter_map(|frontier| frontier.compare)
        .collect::<Vec<_>>();
    let compare_object_records = compare_records
        .iter()
        .filter(|compare| {
            [compare.left_source, compare.right_source]
                .iter()
                .any(|source| {
                    matches!(
                        source,
                        QuestSeqBaseCompareOperandSource::ObjectField
                            | QuestSeqBaseCompareOperandSource::ObjectIndirectField
                    )
                })
        })
        .count();
    let compare_argument_records = compare_records
        .iter()
        .filter(|compare| {
            [compare.left_source, compare.right_source]
                .contains(&QuestSeqBaseCompareOperandSource::HandlerArgumentA2)
        })
        .count();
    let compare_in_range_branch_targets = compare_records
        .iter()
        .filter(|compare| compare.branch_target_offset.is_some())
        .count();
    if compare_records.len() != BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_RECORDS
        || compare_object_records != BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_OBJECT_RECORDS
        || compare_argument_records != BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_ARGUMENT_RECORDS
        || compare_in_range_branch_targets
            != BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_IN_RANGE_BRANCH_TARGETS
    {
        return Err(error(format!(
            "QUEST SEQ text frontier 비교 record 분모 불일치: \
             expected={}/{}/{}/{}, actual={}/{}/{}/{}",
            BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_RECORDS,
            BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_OBJECT_RECORDS,
            BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_ARGUMENT_RECORDS,
            BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_IN_RANGE_BRANCH_TARGETS,
            compare_records.len(),
            compare_object_records,
            compare_argument_records,
            compare_in_range_branch_targets,
        )));
    }
    let compare_operations =
        BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_OPERATION_EXPECTATIONS.map(|(label, _)| {
            (
                label,
                compare_records
                    .iter()
                    .filter(|compare| compare.operation.label() == label)
                    .count(),
            )
        });
    if compare_operations != BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_OPERATION_EXPECTATIONS {
        return Err(error(format!(
            "QUEST SEQ text frontier 비교 operation 분모 불일치: \
             expected={BOX_QUEST_SEQ_TEXT_FRONTIER_COMPARE_OPERATION_EXPECTATIONS:?}, \
             actual={compare_operations:?}"
        )));
    }

    let report = render_quest_seq_text_frontiers_tsv(&analysis);
    let digest = format!("{:x}", Sha256::digest(report.as_bytes()));
    if digest != BOX_QUEST_SEQ_TEXT_FRONTIER_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST SEQ text frontier report SHA-256 불일치: \
             expected={BOX_QUEST_SEQ_TEXT_FRONTIER_REPORT_SHA256}, actual={digest}"
        )));
    }
    Ok(analysis)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn join_hex<T>(values: impl IntoIterator<Item = T>, render: impl Fn(T) -> String) -> String {
    values.into_iter().map(render).collect::<Vec<_>>().join(",")
}

/// 고유 frontier와 다음 runtime 입력을 보존하는 결정적 TSV.
pub fn render_quest_seq_text_frontiers_tsv(analysis: &QuestSeqTextFrontierAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tarchive_path\tarchive_sequence_count\tsequence_name\t\
         sequence_sha256\tsequence_runtime_base\tbase_evidence\t\
         terminal_offset\tterminal_runtime\topcode\tstatus\tterminal_raw_16\t\
         path_occurrences\tseed_offsets\tsource_opcodes\truntime_requirement\t\
         parameter_kind\tparameter_offset\tparameter_value\truntime_probe_address\t\
         runtime_probe_length\tcompare_operation\tcompare_value_bytes\t\
         compare_left_source\tcompare_left_parameter\tcompare_right_source\t\
         compare_right_parameter\tcompare_branch_target_runtime\t\
         compare_branch_target_offset\tcompare_fallthrough_runtime\t\
         compare_fallthrough_offset\n",
    );
    for frontier in &analysis.frontiers {
        let (parameter_kind, parameter_offset, parameter_value) = frontier.parameter.map_or_else(
            || (String::new(), String::new(), String::new()),
            |parameter| {
                (
                    parameter.kind().to_string(),
                    format!("0x{:X}", parameter.offset()),
                    format!("0x{:X}", parameter.value()),
                )
            },
        );
        let compare_columns = frontier.compare.map_or_else(
            || vec![String::new(); 10],
            |compare| {
                vec![
                    compare.operation.label().to_string(),
                    compare.value_bytes.to_string(),
                    compare.left_source.label().to_string(),
                    compare
                        .left_parameter
                        .map_or_else(String::new, |value| format!("0x{value:X}")),
                    compare.right_source.label().to_string(),
                    format!("0x{:X}", compare.right_parameter),
                    format!("0x{:08X}", compare.branch_target_runtime),
                    compare
                        .branch_target_offset
                        .map_or_else(String::new, |offset| format!("0x{offset:04X}")),
                    format!("0x{:08X}", compare.fallthrough_runtime),
                    format!("0x{:04X}", compare.fallthrough_offset),
                ]
            },
        );
        writeln!(
            output,
            "{}\t/QUEST/RPG_SEQ.VDD;1\t113\t{}\t{}\t0x{:08X}\t{}\t0x{:04X}\t\
             0x{:08X}\t0x{:02X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            QUEST_SEQ_TEXT_FRONTIER_SCHEMA_VERSION,
            frontier.sequence,
            hex(&frontier.sequence_sha256),
            QUEST_SEQ_ACTIVE_RUNTIME_BASE,
            frontier.base_evidence.label(),
            frontier.terminal_offset,
            frontier.terminal_runtime,
            frontier.opcode,
            frontier.status.label(),
            hex(&frontier.terminal_raw),
            frontier.path_occurrences,
            join_hex(&frontier.seed_offsets, |offset| format!("0x{offset:04X}")),
            join_hex(&frontier.source_opcodes, |opcode| format!("0x{opcode:02X}")),
            frontier.requirement.label(),
            parameter_kind,
            parameter_offset,
            parameter_value,
            frontier
                .runtime_probe_address
                .map_or_else(String::new, |address| format!("0x{address:08X}")),
            frontier
                .runtime_probe_length
                .map_or_else(String::new, |length| length.to_string()),
            compare_columns.join("\t"),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_seq_text_frontiers_tests.rs"]
mod tests;
