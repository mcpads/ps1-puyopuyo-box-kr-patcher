//! stream-pointer target에서 OP21까지의 보수적 정적 선형 경로 후보.
//!
//! 파일 내부 stream-pointer target을 seed로 삼고, handler source에서
//! 관측한 직접 증가 폭이 opcode 전체에서 하나뿐인 경우에만 다음 record로
//! 이동한다. OP10/11의 record mode, OP3A sub02/sub08/sub0A/sub16과 공용 fallback은
//! 원본 byte로 직접 폭을 고른다. 검증된 OP03 direct branch는 같은-file
//! target으로 옮기고, OP28은 하위 flow와 원본 condition byte로
//! advance/yield 또는 branch를 고른다. OP00은 interpreter release로
//! 종단하고, OP1C는 parent `current+8`과 record `+4` child pointer를
//! 한 graph edge로 보존한다. OP82는 지원 subtype의 `record+8` child를
//! 별도 경로로 만들고 parent를 12바이트 전진시킨다. 다른 stream 전이
//! opcode, 미분류 복수 폭, 상태 의존 branch에서는 멈춘다. 이는 source record와
//! branch 조건의 정상 실행 도달을 증명하지 않으며, section parser 전에
//! 사용할 수 있는 정적 하한이다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::RuntimeAssetMap;
use crate::disasm::{
    quest_seq_base_fallback_advance, quest_seq_base_record_selected_advance,
    QuestSeqBaseTransitionMeaning, QUEST_SEQ_BASE_DIRECT_ADVANCE_SITES,
    QUEST_SEQ_BASE_TRANSITION_SPECS, QUEST_SEQ_CHILD_STREAM_OPCODE,
    QUEST_SEQ_CHILD_STREAM_POINTER_OFFSET, QUEST_SEQ_CHILD_STREAM_RECORD_BYTES,
    QUEST_SEQ_CHILD_STREAM_SUBTYPE_LIMIT, QUEST_SEQ_DIRECT_ADVANCE_SITES,
};
use crate::font::QuestCodeTable;

use crate::script::quest_seq::QUEST_SEQ_ACTIVE_RUNTIME_BASE;
#[cfg(test)]
use crate::script::quest_seq::TOWN00_SEQ_RUNTIME_BASE;
use crate::script::quest_seq_inventory::{
    audit_quest_seq_addresses, verify_box_quest_seq_addresses, QuestSeqAddressAnalysis,
    QuestSeqAddressCandidate, QuestSeqPointerBehavior, QUEST_SEQ_POINTER_RECORD_SPECS,
};
use crate::script::quest_seq_op28::{quest_seq_op28_flow, QuestSeqOp28Flow};
use crate::script::quest_seq_text_roots::QuestSeqTextRootBaseEvidence;
use crate::script::quest_seq_text_streams::{
    audit_quest_seq_text_streams, verify_box_quest_seq_text_streams, QuestSeqTextStreamAnalysis,
};

pub const QUEST_SEQ_TEXT_PATH_SCHEMA_VERSION: &str = "quest-seq-text-paths-v5";
pub const BOX_QUEST_SEQ_TEXT_PATH_SEEDS: usize = 383;
pub const BOX_QUEST_SEQ_TEXT_PATH_SOURCE_EDGES: usize = 487;
pub const BOX_QUEST_SEQ_TEXT_PATH_REACHED_MARKERS: usize = 124;
pub const BOX_QUEST_SEQ_TEXT_PATH_DIRECT_MARKERS: usize = 57;
pub const BOX_QUEST_SEQ_TEXT_PATH_SEQUENTIAL_MARKERS: usize = 67;
pub const BOX_QUEST_SEQ_TEXT_PATH_DIRECT_BRANCH_MARKERS: usize = 16;
pub const BOX_QUEST_SEQ_TEXT_PATH_DIRECT_BRANCH_EDGE_OCCURRENCES: usize = 67;
pub const BOX_QUEST_SEQ_TEXT_PATH_UNIQUE_DIRECT_BRANCH_EDGES: usize = 53;
pub const BOX_QUEST_SEQ_TEXT_PATH_RECORD_MODE_ADVANCES: usize = 149;
pub const BOX_QUEST_SEQ_TEXT_PATH_SUBDISPATCH_ADVANCES: usize = 9;
pub const BOX_QUEST_SEQ_TEXT_PATH_FALLBACK_ADVANCES: usize = 0;
pub const BOX_QUEST_SEQ_TEXT_PATH_RELEASED: usize = 14;
pub const BOX_QUEST_SEQ_TEXT_PATH_STOPPED: usize = 245;
pub const BOX_QUEST_SEQ_TEXT_PATH_STOPPED_AT_STREAM_OPCODE: usize = 153;
pub const BOX_QUEST_SEQ_TEXT_PATH_STOPPED_WITHOUT_UNIQUE_WIDTH: usize = 72;
pub const BOX_QUEST_SEQ_TEXT_PATH_STOPPED_AT_INVALID_DIRECT_BRANCH: usize = 0;
pub const BOX_QUEST_SEQ_TEXT_PATH_STOPPED_AT_CYCLE: usize = 20;
pub const BOX_QUEST_SEQ_TEXT_PATH_NODE_OCCURRENCES: usize = 1_762;
pub const BOX_QUEST_SEQ_TEXT_PATH_UNIQUE_NODES: usize = 1_563;
pub const BOX_QUEST_SEQ_TEXT_PATH_UNIQUE_WIDTH_OPCODES: usize = 101;
pub const BOX_QUEST_SEQ_TEXT_PATH_AMBIGUOUS_WIDTH_OPCODES: usize = 11;
pub const BOX_QUEST_SEQ_TEXT_PATH_ROOTED_RUNS: usize = 511;
pub const BOX_QUEST_SEQ_TEXT_PATH_ROOTED_CODE_UNITS: usize = 4_988;
pub const BOX_QUEST_SEQ_TEXT_PATH_ROOTED_JAPANESE_CODE_UNITS: usize = 3_793;
pub const BOX_QUEST_SEQ_TEXT_PATH_ROOTED_UNIQUE_CHARACTERS: usize = 188;
pub const BOX_QUEST_SEQ_TEXT_PATH_REPORT_SHA256: &str =
    "0f94827c016172891aebd199a801930944cd16ddecdc9b973cf08f57cdc05da5";

/// `(파일, seed, direct, sequential, branch-reached, branch-edge occurrence, released, stopped, node occurrence)`.
pub type QuestSeqTextPathFileExpectation = (
    &'static str,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
);
pub const BOX_QUEST_SEQ_TEXT_PATH_FILE_EXPECTATIONS: [QuestSeqTextPathFileExpectation; 13] = [
    ("ALLMAP.SEQ", 16, 0, 0, 0, 15, 1, 15, 166),
    ("HASI.SEQ", 7, 1, 1, 0, 1, 1, 4, 32),
    ("ISEKI.SEQ", 34, 1, 11, 4, 6, 1, 21, 230),
    ("KAWA.SEQ", 38, 9, 5, 0, 4, 1, 23, 206),
    ("MITI.SEQ", 19, 1, 4, 4, 9, 1, 13, 128),
    ("MORI.SEQ", 21, 3, 2, 0, 4, 1, 15, 97),
    ("SHEZ.SEQ", 47, 2, 16, 0, 6, 1, 28, 163),
    ("SUKE.SEQ", 11, 1, 1, 0, 3, 1, 8, 75),
    ("TOU.SEQ", 35, 1, 11, 8, 9, 2, 21, 151),
    ("TOWN00.SEQ", 43, 11, 3, 0, 2, 1, 28, 135),
    ("TOWN10.SEQ", 56, 12, 8, 0, 3, 1, 35, 200),
    ("TOWN20.SEQ", 41, 11, 5, 0, 4, 1, 24, 132),
    ("TOWN30.SEQ", 15, 4, 0, 0, 1, 1, 10, 47),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqTextPathStatus {
    ReachedDirectOp21,
    ReachedSequentialOp21,
    ReleasedInterpreter,
    StoppedAtStreamPointerOpcode,
    StoppedWithoutUniqueDirectWidth,
    StoppedAtInvalidDirectBranchTarget,
    StoppedAtInvalidOp28Flow,
    StoppedAtUnparseableOp21,
    StoppedAtTruncatedRecord,
    StoppedAtCycle,
}

impl QuestSeqTextPathStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReachedDirectOp21 => "reached-direct-op21-candidate",
            Self::ReachedSequentialOp21 => "reached-sequential-op21-candidate",
            Self::ReleasedInterpreter => "released-interpreter",
            Self::StoppedAtStreamPointerOpcode => "stopped-at-stream-pointer-opcode",
            Self::StoppedWithoutUniqueDirectWidth => "stopped-without-unique-direct-width",
            Self::StoppedAtInvalidDirectBranchTarget => "stopped-at-invalid-direct-branch-target",
            Self::StoppedAtInvalidOp28Flow => "stopped-at-invalid-op28-flow",
            Self::StoppedAtUnparseableOp21 => "stopped-at-unparseable-op21",
            Self::StoppedAtTruncatedRecord => "stopped-at-truncated-record",
            Self::StoppedAtCycle => "stopped-at-cycle",
        }
    }

    pub const fn reached_op21(self) -> bool {
        matches!(self, Self::ReachedDirectOp21 | Self::ReachedSequentialOp21)
    }

    pub const fn is_runtime_frontier(self) -> bool {
        !matches!(
            self,
            Self::ReachedDirectOp21 | Self::ReachedSequentialOp21 | Self::ReleasedInterpreter
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqTextPathTransition {
    End,
    ReleaseInterpreter,
    DirectAdvance,
    RecordModeAdvance,
    SubdispatchAdvance,
    FallbackAdvance,
    DirectBranch,
    SpawnChildAndAdvance,
    Op28FixedAdvance,
    Op28AdvanceYield,
    Op28Branch,
}

impl QuestSeqTextPathTransition {
    pub const fn label(self) -> &'static str {
        match self {
            Self::End => "end",
            Self::ReleaseInterpreter => "release-interpreter",
            Self::DirectAdvance => "direct-advance",
            Self::RecordModeAdvance => "record-mode-advance",
            Self::SubdispatchAdvance => "subdispatch-advance",
            Self::FallbackAdvance => "fallback-advance",
            Self::DirectBranch => "direct-branch",
            Self::SpawnChildAndAdvance => "spawn-child-and-advance",
            Self::Op28FixedAdvance => "op28-fixed-advance",
            Self::Op28AdvanceYield => "op28-advance-yield",
            Self::Op28Branch => "op28-branch",
        }
    }
}

/// 한 seed target을 만든 source pointer field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqTextPathSource {
    pub source_record_offset: usize,
    pub pointer_field_offset: usize,
    pub opcode: u8,
    pub behavior: QuestSeqPointerBehavior,
    pub pointer_consumer_runtime: u32,
}

/// 선형 후보 경로에서 방문한 record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqTextPathNode {
    pub offset: usize,
    pub opcode: u8,
    pub transition: QuestSeqTextPathTransition,
    pub next_offset: Option<usize>,
    /// OP1C가 record `+4` pointer로 생성하는 child stream.
    pub child_stream_offset: Option<usize>,
    /// handler가 다음 전이 전에 적용한 직접 증가 폭.
    ///
    /// OP28 branch에서는 이 폭으로 먼저 옮긴 record에서 pointer를 읽으므로
    /// `next_offset - offset`과 같지 않을 수 있다.
    pub direct_advance_bytes: Option<usize>,
}

/// stream-pointer seed 하나의 보수적 walk 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextPath {
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub seed_offset: usize,
    pub seed_runtime: u32,
    pub base_evidence: QuestSeqTextRootBaseEvidence,
    pub sources: Vec<QuestSeqTextPathSource>,
    pub nodes: Vec<QuestSeqTextPathNode>,
    pub status: QuestSeqTextPathStatus,
    pub reached_op21_offset: Option<usize>,
    pub reached_rooted_run_count: usize,
}

/// 파일별 seed walk 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextPathSummary {
    pub sequence: String,
    pub seeds: usize,
    pub direct_markers: usize,
    pub sequential_markers: usize,
    pub direct_branch_markers: usize,
    pub direct_branch_edge_occurrences: usize,
    pub released: usize,
    pub stopped: usize,
    pub path_node_occurrences: usize,
}

/// source-backed stream target의 보수적 선형 경로 분석.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextPathAnalysis {
    pub summaries: Vec<QuestSeqTextPathSummary>,
    pub paths: Vec<QuestSeqTextPath>,
    pub direct_branch_edge_occurrences: usize,
    pub unique_direct_branch_edges: usize,
    pub rooted_runs: usize,
    pub rooted_code_units: usize,
    pub rooted_japanese_code_units: usize,
    pub rooted_unique_characters: usize,
}

impl QuestSeqTextPathAnalysis {
    /// 모든 seed 경로에서 지정한 전이가 나타난 횟수를 센다.
    pub fn transition_count(&self, transition: QuestSeqTextPathTransition) -> usize {
        self.paths
            .iter()
            .flat_map(|path| &path.nodes)
            .filter(|node| node.transition == transition)
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextPathError(String);

impl fmt::Display for QuestSeqTextPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestSeqTextPathError {}

fn error(message: impl Into<String>) -> QuestSeqTextPathError {
    QuestSeqTextPathError(message.into())
}

fn is_stream_pointer_behavior(behavior: QuestSeqPointerBehavior) -> bool {
    matches!(
        behavior,
        QuestSeqPointerBehavior::SpawnStream
            | QuestSeqPointerBehavior::ScheduledStream
            | QuestSeqPointerBehavior::SavedStream
            | QuestSeqPointerBehavior::Branch
            | QuestSeqPointerBehavior::StackBranch
    )
}

fn source(candidate: &QuestSeqAddressCandidate) -> Option<QuestSeqTextPathSource> {
    let spec = candidate.pointer_spec?;
    if !is_stream_pointer_behavior(spec.behavior) {
        return None;
    }
    Some(QuestSeqTextPathSource {
        source_record_offset: candidate.field_offset.checked_sub(spec.pointer_offset)?,
        pointer_field_offset: candidate.field_offset,
        opcode: spec.opcode,
        behavior: spec.behavior,
        pointer_consumer_runtime: spec.pointer_consumer_runtime,
    })
}

pub(super) fn width_profile() -> (BTreeMap<u8, usize>, BTreeSet<u8>) {
    let mut widths = BTreeMap::<u8, BTreeSet<usize>>::new();
    for (opcode, record_bytes) in QUEST_SEQ_BASE_DIRECT_ADVANCE_SITES
        .iter()
        .map(|site| (site.opcode, site.record_bytes))
        .chain(
            QUEST_SEQ_DIRECT_ADVANCE_SITES
                .iter()
                .map(|site| (site.opcode, site.record_bytes)),
        )
    {
        let width = usize::try_from(record_bytes).expect("직접 증가 폭은 양수");
        widths.entry(opcode).or_default().insert(width);
    }
    let unique = widths
        .iter()
        .filter(|(_, values)| values.len() == 1)
        .map(|(&opcode, values)| {
            (
                opcode,
                *values.first().expect("길이가 1인 direct width 집합"),
            )
        })
        .collect();
    let ambiguous = widths
        .into_iter()
        .filter_map(|(opcode, values)| (values.len() > 1).then_some(opcode))
        .collect();
    (unique, ambiguous)
}

fn stream_pointer_opcodes() -> BTreeSet<u8> {
    QUEST_SEQ_POINTER_RECORD_SPECS
        .iter()
        .filter(|spec| is_stream_pointer_behavior(spec.behavior))
        .map(|spec| spec.opcode)
        .collect()
}

fn record_selected_step(
    raw: &[u8],
    offset: usize,
    opcode: u8,
) -> Result<Option<(QuestSeqTextPathTransition, usize)>, ()> {
    if let Some(width) = quest_seq_base_fallback_advance(opcode) {
        return Ok(Some((QuestSeqTextPathTransition::FallbackAdvance, width)));
    }
    if !matches!(opcode, 0x10 | 0x11 | 0x3A) {
        return Ok(None);
    }
    let byte1 = *raw.get(offset.checked_add(1).ok_or(())?).ok_or(())?;
    Ok(
        quest_seq_base_record_selected_advance(opcode, byte1).map(|width| {
            let transition = if opcode == 0x3A {
                QuestSeqTextPathTransition::SubdispatchAdvance
            } else {
                QuestSeqTextPathTransition::RecordModeAdvance
            };
            (transition, width)
        }),
    )
}

fn is_direct_branch_opcode(opcode: u8) -> bool {
    QUEST_SEQ_BASE_TRANSITION_SPECS.iter().any(|spec| {
        spec.opcode == opcode && spec.meaning == QuestSeqBaseTransitionMeaning::DirectBranchPointer
    })
}

fn direct_branch_target(raw: &[u8], offset: usize, address_map: RuntimeAssetMap) -> Option<usize> {
    let pointer_start = offset.checked_add(4)?;
    runtime_pointer_target(raw, pointer_start, address_map)
}

fn runtime_pointer_target(
    raw: &[u8],
    pointer_start: usize,
    address_map: RuntimeAssetMap,
) -> Option<usize> {
    let pointer_end = pointer_start.checked_add(4)?;
    let target_runtime = u32::from_le_bytes(raw.get(pointer_start..pointer_end)?.try_into().ok()?);
    address_map.file_offset(target_runtime)
}

fn child_stream_step(
    raw: &[u8],
    offset: usize,
    pointer_offset: usize,
    record_bytes: usize,
    address_map: RuntimeAssetMap,
) -> Option<(usize, usize)> {
    let pointer_start = offset.checked_add(pointer_offset)?;
    let child_stream_offset = runtime_pointer_target(raw, pointer_start, address_map)?;
    let parent_next = offset.checked_add(record_bytes)?;
    (parent_next < raw.len()).then_some((child_stream_offset, parent_next))
}

fn op28_step(
    raw: &[u8],
    offset: usize,
    address_map: RuntimeAssetMap,
) -> Option<(QuestSeqTextPathTransition, usize, usize)> {
    let subopcode = *raw.get(offset.checked_add(1)?)?;
    match quest_seq_op28_flow(subopcode)? {
        QuestSeqOp28Flow::FixedAdvance { bytes } => {
            let width = usize::try_from(bytes).ok()?;
            let next = offset.checked_add(width)?;
            (next < raw.len()).then_some((
                QuestSeqTextPathTransition::Op28FixedAdvance,
                next,
                width,
            ))
        }
        QuestSeqOp28Flow::AdvanceOrBranch {
            advance_bytes,
            condition_offset,
            pointer_offset,
        } => {
            let width = usize::try_from(advance_bytes).ok()?;
            let advanced = offset.checked_add(width)?;
            let condition =
                *raw.get(advanced.checked_add(usize::try_from(condition_offset).ok()?)?)?;
            if condition == 0 {
                return (advanced < raw.len()).then_some((
                    QuestSeqTextPathTransition::Op28AdvanceYield,
                    advanced,
                    width,
                ));
            }
            let pointer_start = advanced.checked_add(usize::try_from(pointer_offset).ok()?)?;
            let target = runtime_pointer_target(raw, pointer_start, address_map)?;
            Some((QuestSeqTextPathTransition::Op28Branch, target, width))
        }
    }
}

fn collect_input<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Result<BTreeMap<String, &'a [u8]>, QuestSeqTextPathError> {
    let mut input = BTreeMap::new();
    for (name, raw) in sequences {
        if name.is_empty() {
            return Err(error("QUEST SEQ text path 입력 이름이 비어 있음"));
        }
        if input.insert(name.to_string(), raw).is_some() {
            return Err(error(format!("QUEST SEQ text path 입력 중복: {name}")));
        }
    }
    if input.is_empty() {
        return Err(error("QUEST SEQ text path 입력이 없음"));
    }
    Ok(input)
}

fn build_analysis(
    input: &BTreeMap<String, &[u8]>,
    text: &QuestSeqTextStreamAnalysis,
    addresses: &QuestSeqAddressAnalysis,
) -> Result<QuestSeqTextPathAnalysis, QuestSeqTextPathError> {
    let (unique_widths, _) = width_profile();
    let stream_opcodes = stream_pointer_opcodes();
    let parseable_markers = text
        .markers
        .iter()
        .filter(|marker| marker.is_parseable())
        .map(|marker| ((marker.sequence.as_str(), marker.record_offset), marker))
        .collect::<BTreeMap<_, _>>();

    let mut seeds = BTreeMap::<(String, usize), Vec<QuestSeqTextPathSource>>::new();
    for candidate in &addresses.candidates {
        let Some(source) = source(candidate) else {
            continue;
        };
        seeds
            .entry((candidate.sequence.clone(), candidate.target_offset))
            .or_default()
            .push(source);
    }
    for sources in seeds.values_mut() {
        sources.sort_unstable_by_key(|source| {
            (
                source.source_record_offset,
                source.pointer_field_offset,
                source.opcode,
                source.pointer_consumer_runtime,
            )
        });
        sources.dedup();
    }

    let mut paths = Vec::with_capacity(seeds.len());
    for ((sequence, seed_offset), sources) in seeds {
        let raw = input
            .get(&sequence)
            .ok_or_else(|| error(format!("{sequence}: text path 원본이 없음")))?;
        let raw_len = u32::try_from(raw.len())
            .map_err(|_| error(format!("{sequence}: SEQ 길이가 u32 범위를 넘음")))?;
        let address_map = RuntimeAssetMap::new(QUEST_SEQ_ACTIVE_RUNTIME_BASE, raw_len)
            .ok_or_else(|| error(format!("{sequence}: SEQ runtime 사상 overflow")))?;
        let mut offset = seed_offset;
        let mut visited = BTreeSet::new();
        let mut nodes = Vec::new();
        let (status, reached_op21_offset, reached_rooted_run_count) = loop {
            if !visited.insert(offset) {
                break (QuestSeqTextPathStatus::StoppedAtCycle, None, 0);
            }
            let opcode = *raw.get(offset).ok_or_else(|| {
                error(format!(
                    "{sequence}: stream seed 0x{seed_offset:04X}가 파일 밖 0x{offset:04X}에 도달"
                ))
            })?;

            if let Some(marker) = parseable_markers.get(&(sequence.as_str(), offset)) {
                nodes.push(QuestSeqTextPathNode {
                    offset,
                    opcode,
                    transition: QuestSeqTextPathTransition::End,
                    next_offset: None,
                    child_stream_offset: None,
                    direct_advance_bytes: None,
                });
                let status = if nodes.len() == 1 {
                    QuestSeqTextPathStatus::ReachedDirectOp21
                } else {
                    QuestSeqTextPathStatus::ReachedSequentialOp21
                };
                break (status, Some(offset), marker.rooted_run_count);
            }
            if opcode == 0x21 {
                nodes.push(QuestSeqTextPathNode {
                    offset,
                    opcode,
                    transition: QuestSeqTextPathTransition::End,
                    next_offset: None,
                    child_stream_offset: None,
                    direct_advance_bytes: None,
                });
                break (QuestSeqTextPathStatus::StoppedAtUnparseableOp21, None, 0);
            }
            if opcode == 0x00 {
                nodes.push(QuestSeqTextPathNode {
                    offset,
                    opcode,
                    transition: QuestSeqTextPathTransition::ReleaseInterpreter,
                    next_offset: None,
                    child_stream_offset: None,
                    direct_advance_bytes: None,
                });
                break (QuestSeqTextPathStatus::ReleasedInterpreter, None, 0);
            }
            if opcode == 0x1C {
                let (child_stream_offset, parent_next) =
                    child_stream_step(raw, offset, 4, 8, address_map).ok_or_else(|| {
                        error(format!(
                            "{sequence}: OP1C child stream 또는 parent next가 파일 밖임: \
                             0x{offset:04X}"
                        ))
                    })?;
                nodes.push(QuestSeqTextPathNode {
                    offset,
                    opcode,
                    transition: QuestSeqTextPathTransition::SpawnChildAndAdvance,
                    next_offset: Some(parent_next),
                    child_stream_offset: Some(child_stream_offset),
                    direct_advance_bytes: Some(8),
                });
                offset = parent_next;
                continue;
            }
            if opcode == QUEST_SEQ_CHILD_STREAM_OPCODE {
                let Some(&subtype) = raw.get(offset.saturating_add(1)) else {
                    nodes.push(QuestSeqTextPathNode {
                        offset,
                        opcode,
                        transition: QuestSeqTextPathTransition::End,
                        next_offset: None,
                        child_stream_offset: None,
                        direct_advance_bytes: None,
                    });
                    break (QuestSeqTextPathStatus::StoppedAtTruncatedRecord, None, 0);
                };
                if subtype >= QUEST_SEQ_CHILD_STREAM_SUBTYPE_LIMIT {
                    nodes.push(QuestSeqTextPathNode {
                        offset,
                        opcode,
                        transition: QuestSeqTextPathTransition::End,
                        next_offset: None,
                        child_stream_offset: None,
                        direct_advance_bytes: None,
                    });
                    break (
                        QuestSeqTextPathStatus::StoppedAtStreamPointerOpcode,
                        None,
                        0,
                    );
                }
                let Some(parent_next) = offset
                    .checked_add(QUEST_SEQ_CHILD_STREAM_RECORD_BYTES)
                    .filter(|next| *next < raw.len())
                else {
                    nodes.push(QuestSeqTextPathNode {
                        offset,
                        opcode,
                        transition: QuestSeqTextPathTransition::End,
                        next_offset: None,
                        child_stream_offset: None,
                        direct_advance_bytes: None,
                    });
                    break (QuestSeqTextPathStatus::StoppedAtTruncatedRecord, None, 0);
                };
                let child_stream_offset = offset
                    .checked_add(QUEST_SEQ_CHILD_STREAM_POINTER_OFFSET)
                    .and_then(|pointer_start| {
                        runtime_pointer_target(raw, pointer_start, address_map)
                    });
                nodes.push(QuestSeqTextPathNode {
                    offset,
                    opcode,
                    transition: QuestSeqTextPathTransition::SpawnChildAndAdvance,
                    next_offset: Some(parent_next),
                    child_stream_offset,
                    direct_advance_bytes: Some(QUEST_SEQ_CHILD_STREAM_RECORD_BYTES),
                });
                offset = parent_next;
                continue;
            }
            if opcode == 0x28 {
                let Some((transition, next, width)) = op28_step(raw, offset, address_map) else {
                    nodes.push(QuestSeqTextPathNode {
                        offset,
                        opcode,
                        transition: QuestSeqTextPathTransition::End,
                        next_offset: None,
                        child_stream_offset: None,
                        direct_advance_bytes: None,
                    });
                    break (QuestSeqTextPathStatus::StoppedAtInvalidOp28Flow, None, 0);
                };
                nodes.push(QuestSeqTextPathNode {
                    offset,
                    opcode,
                    transition,
                    next_offset: Some(next),
                    child_stream_offset: None,
                    direct_advance_bytes: Some(width),
                });
                offset = next;
                continue;
            }
            if is_direct_branch_opcode(opcode) {
                let Some(target) = direct_branch_target(raw, offset, address_map) else {
                    nodes.push(QuestSeqTextPathNode {
                        offset,
                        opcode,
                        transition: QuestSeqTextPathTransition::End,
                        next_offset: None,
                        child_stream_offset: None,
                        direct_advance_bytes: None,
                    });
                    break (
                        QuestSeqTextPathStatus::StoppedAtInvalidDirectBranchTarget,
                        None,
                        0,
                    );
                };
                nodes.push(QuestSeqTextPathNode {
                    offset,
                    opcode,
                    transition: QuestSeqTextPathTransition::DirectBranch,
                    next_offset: Some(target),
                    child_stream_offset: None,
                    direct_advance_bytes: None,
                });
                offset = target;
                continue;
            }
            let selected = match record_selected_step(raw, offset, opcode) {
                Ok(selected) => selected,
                Err(()) => {
                    nodes.push(QuestSeqTextPathNode {
                        offset,
                        opcode,
                        transition: QuestSeqTextPathTransition::End,
                        next_offset: None,
                        child_stream_offset: None,
                        direct_advance_bytes: None,
                    });
                    break (QuestSeqTextPathStatus::StoppedAtTruncatedRecord, None, 0);
                }
            };
            let (transition, width) = if let Some(selected) = selected {
                selected
            } else {
                if stream_opcodes.contains(&opcode) {
                    nodes.push(QuestSeqTextPathNode {
                        offset,
                        opcode,
                        transition: QuestSeqTextPathTransition::End,
                        next_offset: None,
                        child_stream_offset: None,
                        direct_advance_bytes: None,
                    });
                    break (
                        QuestSeqTextPathStatus::StoppedAtStreamPointerOpcode,
                        None,
                        0,
                    );
                }
                let Some(&width) = unique_widths.get(&opcode) else {
                    nodes.push(QuestSeqTextPathNode {
                        offset,
                        opcode,
                        transition: QuestSeqTextPathTransition::End,
                        next_offset: None,
                        child_stream_offset: None,
                        direct_advance_bytes: None,
                    });
                    break (
                        QuestSeqTextPathStatus::StoppedWithoutUniqueDirectWidth,
                        None,
                        0,
                    );
                };
                (QuestSeqTextPathTransition::DirectAdvance, width)
            };
            let Some(next) = offset.checked_add(width) else {
                return Err(error(format!(
                    "{sequence}: text path offset 증가 overflow: 0x{offset:04X}+{width}"
                )));
            };
            if next > raw.len() {
                nodes.push(QuestSeqTextPathNode {
                    offset,
                    opcode,
                    transition: QuestSeqTextPathTransition::End,
                    next_offset: None,
                    child_stream_offset: None,
                    direct_advance_bytes: None,
                });
                break (QuestSeqTextPathStatus::StoppedAtTruncatedRecord, None, 0);
            }
            nodes.push(QuestSeqTextPathNode {
                offset,
                opcode,
                transition,
                next_offset: Some(next),
                child_stream_offset: None,
                direct_advance_bytes: Some(width),
            });
            offset = next;
        };

        let seed_runtime = QUEST_SEQ_ACTIVE_RUNTIME_BASE
            .checked_add(
                u32::try_from(seed_offset)
                    .map_err(|_| error(format!("{sequence}: seed offset이 u32 범위를 넘음")))?,
            )
            .ok_or_else(|| error(format!("{sequence}: seed runtime 주소 overflow")))?;
        paths.push(QuestSeqTextPath {
            sequence_sha256: Sha256::digest(raw).into(),
            base_evidence: if sequence == "TOWN00.SEQ" {
                QuestSeqTextRootBaseEvidence::RuntimeConfirmedTown00
            } else {
                QuestSeqTextRootBaseEvidence::StaticCommonLoader
            },
            sequence,
            seed_offset,
            seed_runtime,
            sources,
            nodes,
            status,
            reached_op21_offset,
            reached_rooted_run_count,
        });
    }

    let direct_branch_edge_occurrences = paths
        .iter()
        .flat_map(|path| &path.nodes)
        .filter(|node| node.transition == QuestSeqTextPathTransition::DirectBranch)
        .count();
    let unique_direct_branch_edges = paths
        .iter()
        .flat_map(|path| {
            path.nodes.iter().filter_map(|node| {
                (node.transition == QuestSeqTextPathTransition::DirectBranch).then_some((
                    path.sequence.as_str(),
                    node.offset,
                    node.next_offset,
                ))
            })
        })
        .collect::<BTreeSet<_>>()
        .len();
    let reached_markers = paths
        .iter()
        .filter_map(|path| {
            path.reached_op21_offset
                .map(|offset| (path.sequence.as_str(), offset))
        })
        .collect::<BTreeSet<_>>();
    let rooted = text
        .rooted_runs
        .iter()
        .filter(|run| {
            run.owner_record_offsets
                .iter()
                .any(|offset| reached_markers.contains(&(run.sequence.as_str(), *offset)))
        })
        .collect::<Vec<_>>();
    let rooted_code_units = rooted.iter().map(|run| run.code_units()).sum();
    let rooted_japanese_code_units = rooted.iter().map(|run| run.japanese_code_units).sum();
    let rooted_unique_characters = rooted
        .iter()
        .flat_map(|run| run.source.chars())
        .collect::<BTreeSet<_>>()
        .len();

    let mut summaries = Vec::new();
    for sequence in paths
        .iter()
        .map(|path| path.sequence.as_str())
        .collect::<BTreeSet<_>>()
    {
        let file_paths = paths
            .iter()
            .filter(|path| path.sequence == sequence)
            .collect::<Vec<_>>();
        summaries.push(QuestSeqTextPathSummary {
            sequence: sequence.to_string(),
            seeds: file_paths.len(),
            direct_markers: file_paths
                .iter()
                .filter(|path| path.status == QuestSeqTextPathStatus::ReachedDirectOp21)
                .count(),
            sequential_markers: file_paths
                .iter()
                .filter(|path| path.status == QuestSeqTextPathStatus::ReachedSequentialOp21)
                .count(),
            direct_branch_markers: file_paths
                .iter()
                .filter(|path| {
                    path.status.reached_op21()
                        && path
                            .nodes
                            .iter()
                            .any(|node| node.transition == QuestSeqTextPathTransition::DirectBranch)
                })
                .count(),
            direct_branch_edge_occurrences: file_paths
                .iter()
                .flat_map(|path| &path.nodes)
                .filter(|node| node.transition == QuestSeqTextPathTransition::DirectBranch)
                .count(),
            released: file_paths
                .iter()
                .filter(|path| path.status == QuestSeqTextPathStatus::ReleasedInterpreter)
                .count(),
            stopped: file_paths
                .iter()
                .filter(|path| path.status.is_runtime_frontier())
                .count(),
            path_node_occurrences: file_paths.iter().map(|path| path.nodes.len()).sum(),
        });
    }

    Ok(QuestSeqTextPathAnalysis {
        summaries,
        paths,
        direct_branch_edge_occurrences,
        unique_direct_branch_edges,
        rooted_runs: rooted.len(),
        rooted_code_units,
        rooted_japanese_code_units,
        rooted_unique_characters,
    })
}

/// 임의 SEQ 집합의 보수적 stream-target walk를 만든다.
pub fn audit_quest_seq_text_paths<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    code_table: &QuestCodeTable,
) -> Result<QuestSeqTextPathAnalysis, QuestSeqTextPathError> {
    let input = collect_input(sequences)?;
    let text = audit_quest_seq_text_streams(
        input.iter().map(|(name, raw)| (name.as_str(), *raw)),
        code_table,
    )
    .map_err(|source| error(source.to_string()))?;
    let addresses =
        audit_quest_seq_addresses(input.iter().map(|(name, raw)| (name.as_str(), *raw)))
            .map_err(|source| error(source.to_string()))?;
    build_analysis(&input, &text, &addresses)
}

/// 지원 원본의 seed walk 분모와 report digest를 검증한다.
pub fn verify_box_quest_seq_text_paths<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    code_table: &QuestCodeTable,
) -> Result<QuestSeqTextPathAnalysis, QuestSeqTextPathError> {
    let input = collect_input(sequences)?;
    let text = verify_box_quest_seq_text_streams(
        input.iter().map(|(name, raw)| (name.as_str(), *raw)),
        code_table,
    )
    .map_err(|source| error(source.to_string()))?;
    let addresses =
        audit_quest_seq_addresses(input.iter().map(|(name, raw)| (name.as_str(), *raw)))
            .map_err(|source| error(source.to_string()))?;
    verify_box_quest_seq_addresses(&addresses).map_err(|source| error(source.to_string()))?;
    let analysis = build_analysis(&input, &text, &addresses)?;
    let (unique_widths, ambiguous_widths) = width_profile();
    let direct = analysis
        .paths
        .iter()
        .filter(|path| path.status == QuestSeqTextPathStatus::ReachedDirectOp21)
        .count();
    let sequential = analysis
        .paths
        .iter()
        .filter(|path| path.status == QuestSeqTextPathStatus::ReachedSequentialOp21)
        .count();
    let released = analysis
        .paths
        .iter()
        .filter(|path| path.status == QuestSeqTextPathStatus::ReleasedInterpreter)
        .count();
    let stopped_at_stream = analysis
        .paths
        .iter()
        .filter(|path| path.status == QuestSeqTextPathStatus::StoppedAtStreamPointerOpcode)
        .count();
    let stopped_without_width = analysis
        .paths
        .iter()
        .filter(|path| path.status == QuestSeqTextPathStatus::StoppedWithoutUniqueDirectWidth)
        .count();
    let stopped_at_invalid_direct_branch = analysis
        .paths
        .iter()
        .filter(|path| path.status == QuestSeqTextPathStatus::StoppedAtInvalidDirectBranchTarget)
        .count();
    let record_mode_advances = analysis
        .paths
        .iter()
        .flat_map(|path| &path.nodes)
        .filter(|node| node.transition == QuestSeqTextPathTransition::RecordModeAdvance)
        .count();
    let subdispatch_advances = analysis
        .paths
        .iter()
        .flat_map(|path| &path.nodes)
        .filter(|node| node.transition == QuestSeqTextPathTransition::SubdispatchAdvance)
        .count();
    let fallback_advances = analysis
        .paths
        .iter()
        .flat_map(|path| &path.nodes)
        .filter(|node| node.transition == QuestSeqTextPathTransition::FallbackAdvance)
        .count();
    let direct_branch_markers = analysis
        .paths
        .iter()
        .filter(|path| {
            path.status.reached_op21()
                && path
                    .nodes
                    .iter()
                    .any(|node| node.transition == QuestSeqTextPathTransition::DirectBranch)
        })
        .count();
    let stopped_at_cycle = analysis
        .paths
        .iter()
        .filter(|path| path.status == QuestSeqTextPathStatus::StoppedAtCycle)
        .count();
    let stopped_other = analysis
        .paths
        .iter()
        .filter(|path| {
            !matches!(
                path.status,
                QuestSeqTextPathStatus::ReachedDirectOp21
                    | QuestSeqTextPathStatus::ReachedSequentialOp21
                    | QuestSeqTextPathStatus::ReleasedInterpreter
                    | QuestSeqTextPathStatus::StoppedAtStreamPointerOpcode
                    | QuestSeqTextPathStatus::StoppedWithoutUniqueDirectWidth
                    | QuestSeqTextPathStatus::StoppedAtInvalidDirectBranchTarget
                    | QuestSeqTextPathStatus::StoppedAtCycle
            )
        })
        .count();
    let unique_nodes = analysis
        .paths
        .iter()
        .flat_map(|path| {
            path.nodes
                .iter()
                .map(|node| (path.sequence.as_str(), node.offset))
        })
        .collect::<BTreeSet<_>>()
        .len();
    let actual = [
        analysis.paths.len(),
        analysis.paths.iter().map(|path| path.sources.len()).sum(),
        direct + sequential,
        direct,
        sequential,
        direct_branch_markers,
        analysis.direct_branch_edge_occurrences,
        analysis.unique_direct_branch_edges,
        record_mode_advances,
        subdispatch_advances,
        fallback_advances,
        released,
        analysis.paths.len() - direct - sequential - released,
        stopped_at_stream,
        stopped_without_width,
        stopped_at_invalid_direct_branch,
        stopped_at_cycle,
        stopped_other,
        analysis.paths.iter().map(|path| path.nodes.len()).sum(),
        unique_nodes,
        unique_widths.len(),
        ambiguous_widths.len(),
        analysis.rooted_runs,
        analysis.rooted_code_units,
        analysis.rooted_japanese_code_units,
        analysis.rooted_unique_characters,
    ];
    let expected = [
        BOX_QUEST_SEQ_TEXT_PATH_SEEDS,
        BOX_QUEST_SEQ_TEXT_PATH_SOURCE_EDGES,
        BOX_QUEST_SEQ_TEXT_PATH_REACHED_MARKERS,
        BOX_QUEST_SEQ_TEXT_PATH_DIRECT_MARKERS,
        BOX_QUEST_SEQ_TEXT_PATH_SEQUENTIAL_MARKERS,
        BOX_QUEST_SEQ_TEXT_PATH_DIRECT_BRANCH_MARKERS,
        BOX_QUEST_SEQ_TEXT_PATH_DIRECT_BRANCH_EDGE_OCCURRENCES,
        BOX_QUEST_SEQ_TEXT_PATH_UNIQUE_DIRECT_BRANCH_EDGES,
        BOX_QUEST_SEQ_TEXT_PATH_RECORD_MODE_ADVANCES,
        BOX_QUEST_SEQ_TEXT_PATH_SUBDISPATCH_ADVANCES,
        BOX_QUEST_SEQ_TEXT_PATH_FALLBACK_ADVANCES,
        BOX_QUEST_SEQ_TEXT_PATH_RELEASED,
        BOX_QUEST_SEQ_TEXT_PATH_STOPPED,
        BOX_QUEST_SEQ_TEXT_PATH_STOPPED_AT_STREAM_OPCODE,
        BOX_QUEST_SEQ_TEXT_PATH_STOPPED_WITHOUT_UNIQUE_WIDTH,
        BOX_QUEST_SEQ_TEXT_PATH_STOPPED_AT_INVALID_DIRECT_BRANCH,
        BOX_QUEST_SEQ_TEXT_PATH_STOPPED_AT_CYCLE,
        0,
        BOX_QUEST_SEQ_TEXT_PATH_NODE_OCCURRENCES,
        BOX_QUEST_SEQ_TEXT_PATH_UNIQUE_NODES,
        BOX_QUEST_SEQ_TEXT_PATH_UNIQUE_WIDTH_OPCODES,
        BOX_QUEST_SEQ_TEXT_PATH_AMBIGUOUS_WIDTH_OPCODES,
        BOX_QUEST_SEQ_TEXT_PATH_ROOTED_RUNS,
        BOX_QUEST_SEQ_TEXT_PATH_ROOTED_CODE_UNITS,
        BOX_QUEST_SEQ_TEXT_PATH_ROOTED_JAPANESE_CODE_UNITS,
        BOX_QUEST_SEQ_TEXT_PATH_ROOTED_UNIQUE_CHARACTERS,
    ];
    if actual != expected {
        return Err(error(format!(
            "QUEST SEQ text path 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }

    let actual_files = analysis
        .summaries
        .iter()
        .map(|summary| {
            (
                summary.sequence.as_str(),
                summary.seeds,
                summary.direct_markers,
                summary.sequential_markers,
                summary.direct_branch_markers,
                summary.direct_branch_edge_occurrences,
                summary.released,
                summary.stopped,
                summary.path_node_occurrences,
            )
        })
        .collect::<Vec<_>>();
    if actual_files != BOX_QUEST_SEQ_TEXT_PATH_FILE_EXPECTATIONS {
        return Err(error(format!(
            "QUEST SEQ text path 파일별 분모 불일치: \
             expected={BOX_QUEST_SEQ_TEXT_PATH_FILE_EXPECTATIONS:?}, actual={actual_files:?}"
        )));
    }

    let report = render_quest_seq_text_paths_tsv(&analysis);
    let digest = format!("{:x}", Sha256::digest(report.as_bytes()));
    if digest != BOX_QUEST_SEQ_TEXT_PATH_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST SEQ text path report SHA-256 불일치: \
             expected={BOX_QUEST_SEQ_TEXT_PATH_REPORT_SHA256}, actual={digest}"
        )));
    }
    Ok(analysis)
}

fn join<T>(values: impl IntoIterator<Item = T>, render: impl Fn(T) -> String) -> String {
    values.into_iter().map(render).collect::<Vec<_>>().join(",")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 모든 seed의 성공·중단을 보존하는 결정적 TSV.
pub fn render_quest_seq_text_paths_tsv(analysis: &QuestSeqTextPathAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tarchive_path\tarchive_sequence_count\tsequence_name\t\
         sequence_sha256\tsequence_runtime_base\tbase_evidence\tseed_offset\t\
         seed_runtime\tsource_pointer_count\tsource_opcodes\tsource_record_offsets\t\
         source_pointer_fields\tsource_behaviors\tsource_consumers\tpath_node_count\t\
         path_offsets\tpath_opcodes\tpath_transitions\tpath_next_offsets\t\
         path_child_stream_offsets\tpath_direct_advance_bytes\tstatus\t\
         reached_op21_offset\treached_rooted_run_count\n",
    );
    for path in &analysis.paths {
        writeln!(
            output,
            "{}\t/QUEST/RPG_SEQ.VDD;1\t113\t{}\t{}\t0x{:08X}\t{}\t0x{:04X}\t\
             0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            QUEST_SEQ_TEXT_PATH_SCHEMA_VERSION,
            path.sequence,
            hex(&path.sequence_sha256),
            QUEST_SEQ_ACTIVE_RUNTIME_BASE,
            path.base_evidence.label(),
            path.seed_offset,
            path.seed_runtime,
            path.sources.len(),
            join(&path.sources, |source| format!("0x{:02X}", source.opcode)),
            join(&path.sources, |source| {
                format!("0x{:04X}", source.source_record_offset)
            }),
            join(&path.sources, |source| {
                format!("0x{:04X}", source.pointer_field_offset)
            }),
            join(&path.sources, |source| source.behavior.label().to_string()),
            join(&path.sources, |source| {
                format!("0x{:08X}", source.pointer_consumer_runtime)
            }),
            path.nodes.len(),
            join(&path.nodes, |node| format!("0x{:04X}", node.offset)),
            join(&path.nodes, |node| format!("0x{:02X}", node.opcode)),
            join(&path.nodes, |node| node.transition.label().to_string()),
            join(&path.nodes, |node| {
                node.next_offset
                    .map_or_else(String::new, |offset| format!("0x{offset:04X}"))
            }),
            join(&path.nodes, |node| {
                node.child_stream_offset
                    .map_or_else(String::new, |offset| format!("0x{offset:04X}"))
            }),
            join(&path.nodes, |node| {
                node.direct_advance_bytes
                    .map_or_else(String::new, |width| width.to_string())
            }),
            path.status.label(),
            path.reached_op21_offset
                .map_or_else(String::new, |offset| format!("0x{offset:04X}")),
            path.reached_rooted_run_count,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_seq_text_paths_tests.rs"]
mod tests;
