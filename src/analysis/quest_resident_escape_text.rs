//! QUEST 자연 조우의 도주 시도·실패 상주 OP21 원문.
//!
//! 조우 메뉴 OP21 뒤의 OP09 branch table에서 도주 target을 고정하고, 첫
//! OP21 뒤 OP14의 false edge가 두 번째 OP21로 이어지는 source graph만
//! 따른다. 화면 문자열 검색 결과만으로 writer 입력을 만들지 않는다.
//! 실제 selector 값·분기 선택·표시는 runtime 판정으로 남긴다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::disasm::{QuestSeqBaseCompareOperandSource, QuestSeqBaseCompareOperation};
use crate::font::QuestCodeTable;

use super::quest_resident_encounter_path::{
    QuestResidentEncounterPathAnalysis, QUEST_RESIDENT_ENCOUNTER_NEXT_SELECTOR_RUNTIME,
};
use super::quest_resident_seq_frontiers::{compare_frontier, QuestResidentSeqCompareFrontier};
use super::quest_resident_seq_paths::{
    walk_resident_stream, QuestResidentSeqFrontierRequirement, QuestResidentSeqPath,
    QuestResidentSeqPathNode, QuestResidentSeqPathStatus, QuestResidentSeqPathTransition,
};
use super::quest_resident_text_units::{
    combine_quest_resident_text_units, extract_quest_resident_text_unit, QuestResidentTextRole,
    QuestResidentTextUnitAnalysis,
};
use crate::script::quest_seq_text_streams::{
    parse_quest_text_stream, QuestTextTerminal, QUEST_SEQ_TEXT_MODE_OPCODE,
    QUEST_SEQ_TEXT_MODE_RECORD_BYTES,
};

pub const QUEST_RESIDENT_ESCAPE_ATTEMPT_TEXT_ID: &str =
    "QUEST-EXE-RESIDENT-TEXT:escape-attempt:0x8008C65C";
pub const QUEST_RESIDENT_ESCAPE_FAILURE_TEXT_ID: &str =
    "QUEST-EXE-RESIDENT-TEXT:escape-failure:0x8008C698";
pub const QUEST_RESIDENT_ESCAPE_ATTEMPT_TEXT_SHA256: &str =
    "e64ff8348939a4d4e1a9c8fc9202050c14ce492e6b722d5d8d12284e9886fcbe";
pub const QUEST_RESIDENT_ESCAPE_FAILURE_TEXT_SHA256: &str =
    "07060b59ec7d07c411180e643f5bb043d9713260e7986169a2088fa2b1c93a0d";

pub const QUEST_RESIDENT_ESCAPE_SELECTOR_RUNTIME: u32 =
    QUEST_RESIDENT_ENCOUNTER_NEXT_SELECTOR_RUNTIME;
pub const QUEST_RESIDENT_ESCAPE_ATTEMPT_RUNTIME: u32 = 0x8008_C65C;
pub const QUEST_RESIDENT_ESCAPE_OUTCOME_COMPARE_RUNTIME: u32 = 0x8008_C68C;
pub const QUEST_RESIDENT_ESCAPE_FAILURE_RUNTIME: u32 = 0x8008_C698;

const ESCAPE_SELECTOR_FIELD_OFFSET: u8 = 0x34;
const ESCAPE_SELECTOR_RAW: [u8; 4] = [0x09, ESCAPE_SELECTOR_FIELD_OFFSET, 0x02, 0x00];
const ESCAPE_SELECTOR_TARGETS: [u32; 4] = [
    0x8008_C7F0,
    QUEST_RESIDENT_ESCAPE_ATTEMPT_RUNTIME,
    0x8008_C6FC,
    0x8008_C728,
];
const ESCAPE_ATTEMPT_BRANCH_INDEX: usize = 1;
const ESCAPE_ATTEMPT_RUN_SOURCE: &str = "　　アルルは逃げ出した！！";
const ESCAPE_FAILURE_RUN_SOURCE: &str = "　　しかし、逃げられなかった！！";
const BOX_QUEST_RESIDENT_ESCAPE_UNITS: usize = 2;
const BOX_QUEST_RESIDENT_ESCAPE_GLYPHS: usize = 29;
const BOX_QUEST_RESIDENT_ESCAPE_JAPANESE_GLYPHS: usize = 20;
const BOX_QUEST_RESIDENT_ESCAPE_CONTROLS: usize = 7;
const BOX_QUEST_RESIDENT_ESCAPE_RUNS: usize = 2;

/// 조우 메뉴 OP09에서 두 도주 결과 OP21까지의 source-bound 경로.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentEscapeTextAnalysis {
    pub selector_runtime: u32,
    pub selector_field_offset: u8,
    pub selector_targets: [u32; 4],
    pub attempt_path: QuestResidentSeqPath,
    pub outcome_compare: QuestResidentSeqCompareFrontier,
    pub units: QuestResidentTextUnitAnalysis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentEscapeTextError(String);

impl fmt::Display for QuestResidentEscapeTextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentEscapeTextError {}

fn error(message: impl Into<String>) -> QuestResidentEscapeTextError {
    QuestResidentEscapeTextError(message.into())
}

fn text_slice<'a>(
    map: &ExeMap,
    text: &'a [u8],
    runtime: u32,
    length: usize,
    role: &str,
) -> Result<&'a [u8], QuestResidentEscapeTextError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "QUEST resident escape {role}가 EXE text 밖임: 0x{runtime:08X}"
        ))
    })?;
    let end = offset
        .checked_add(length)
        .ok_or_else(|| error(format!("QUEST resident escape {role} 범위 overflow")))?;
    text.get(offset..end)
        .ok_or_else(|| error(format!("QUEST resident escape {role}가 잘림")))
}

fn read_u32(raw: &[u8], offset: usize) -> Result<u32, QuestResidentEscapeTextError> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| error("QUEST resident escape pointer 범위 overflow"))?;
    Ok(u32::from_le_bytes(
        raw.get(offset..end)
            .ok_or_else(|| error("QUEST resident escape pointer가 잘림"))?
            .try_into()
            .expect("4바이트 범위를 확인함"),
    ))
}

fn op21_node(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    runtime: u32,
) -> Result<QuestResidentSeqPathNode, QuestResidentEscapeTextError> {
    let record = text_slice(
        map,
        text,
        runtime,
        QUEST_SEQ_TEXT_MODE_RECORD_BYTES,
        "OP21 record",
    )?;
    if record != [QUEST_SEQ_TEXT_MODE_OPCODE, 0, 0, 0] {
        return Err(error(format!(
            "QUEST resident escape OP21 record drift: 0x{runtime:08X} {record:02X?}"
        )));
    }
    let record_offset = map
        .text_offset(runtime)
        .expect("text_slice에서 runtime 범위를 확인함");
    let body_offset = record_offset
        .checked_add(QUEST_SEQ_TEXT_MODE_RECORD_BYTES)
        .ok_or_else(|| error("QUEST resident escape OP21 body offset overflow"))?;
    let parsed = parse_quest_text_stream("QUEST.EXE", text, body_offset, code_table)
        .map_err(|cause| error(format!("QUEST resident escape OP21 파싱 실패: {cause}")))?;
    let transition = match parsed.terminal {
        QuestTextTerminal::ReleaseInterpreter => QuestResidentSeqPathTransition::TextRelease,
        QuestTextTerminal::DeactivateAndAlign => {
            QuestResidentSeqPathTransition::TextDeactivateAndAlign
        }
        QuestTextTerminal::NotifyParentAndRelease => {
            QuestResidentSeqPathTransition::TextNotifyAndRelease
        }
    };
    let next_runtime = (parsed.terminal == QuestTextTerminal::DeactivateAndAlign)
        .then(|| {
            map.text_runtime_addr(parsed.end_offset)
                .ok_or_else(|| error("QUEST resident escape OP21 다음 record가 text 밖임"))
        })
        .transpose()?;
    Ok(QuestResidentSeqPathNode {
        runtime,
        opcode: QUEST_SEQ_TEXT_MODE_OPCODE,
        transition,
        next_runtime,
        side_stream_runtime: None,
        child_stream_runtime: None,
        text_terminal: Some(parsed.terminal),
        text_glyphs: parsed.glyph_count,
        text_controls: parsed.control_count,
    })
}

/// 조우 표시 path의 OP09와 두 결과 문구를 source graph에 결속한다.
pub fn audit_quest_resident_escape_text(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    encounter: &QuestResidentEncounterPathAnalysis,
) -> Result<QuestResidentEscapeTextAnalysis, QuestResidentEscapeTextError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST resident escape 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len
        )));
    }
    let selector_node = encounter
        .display_path
        .nodes
        .last()
        .ok_or_else(|| error("QUEST resident escape selector source path가 비었음"))?;
    if selector_node.runtime != QUEST_RESIDENT_ESCAPE_SELECTOR_RUNTIME
        || selector_node.opcode != 0x09
        || selector_node.transition != QuestResidentSeqPathTransition::RuntimeFrontier
        || encounter.display_path.status != QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier
        || encounter.display_path.frontier_requirement
            != Some(QuestResidentSeqFrontierRequirement::ObjectBranchSelectorState)
    {
        return Err(error(format!(
            "QUEST resident escape OP09 source path drift: node={selector_node:?}, status={:?}, requirement={:?}",
            encounter.display_path.status, encounter.display_path.frontier_requirement
        )));
    }

    let selector_bytes = 4 + ESCAPE_SELECTOR_TARGETS.len() * 4;
    let selector = text_slice(
        map,
        text,
        QUEST_RESIDENT_ESCAPE_SELECTOR_RUNTIME,
        selector_bytes,
        "OP09 selector",
    )?;
    let header: [u8; 4] = selector[..4]
        .try_into()
        .expect("selector header 폭을 확인함");
    let mut selector_targets = [0u32; ESCAPE_SELECTOR_TARGETS.len()];
    for (index, target) in selector_targets.iter_mut().enumerate() {
        *target = read_u32(selector, 4 + index * 4)?;
        map.text_offset(*target).ok_or_else(|| {
            error(format!(
                "QUEST resident escape OP09 target #{index}가 EXE text 밖임: 0x{target:08X}"
            ))
        })?;
    }
    if header[0] != 0x09 {
        return Err(error(format!(
            "QUEST resident escape selector가 OP09가 아님: {header:02X?}"
        )));
    }

    let attempt_path = walk_resident_stream(
        map,
        text,
        code_table,
        selector_targets[ESCAPE_ATTEMPT_BRANCH_INDEX],
        0,
    )
    .map_err(|cause| error(format!("QUEST resident escape attempt path: {cause}")))?;
    let attempt_node = attempt_path
        .nodes
        .iter()
        .find(|node| node.opcode == QUEST_SEQ_TEXT_MODE_OPCODE)
        .ok_or_else(|| error("QUEST resident escape attempt path에 OP21이 없음"))?;
    let compare_node = attempt_path
        .nodes
        .last()
        .ok_or_else(|| error("QUEST resident escape attempt path가 비었음"))?;
    if attempt_path.status != QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier
        || attempt_path.frontier_requirement
            != Some(QuestResidentSeqFrontierRequirement::CompareState)
        || compare_node.opcode != 0x14
    {
        return Err(error(format!(
            "QUEST resident escape outcome compare 경계 drift: path={attempt_path:?}"
        )));
    }
    let outcome_compare = compare_frontier(map, text, compare_node.runtime)
        .map_err(|cause| error(format!("QUEST resident escape outcome compare: {cause}")))?;
    let failure_node = op21_node(map, text, code_table, outcome_compare.fallthrough_runtime)?;
    let attempt = extract_quest_resident_text_unit(
        map,
        text,
        code_table,
        attempt_node,
        QUEST_RESIDENT_ESCAPE_ATTEMPT_TEXT_ID,
        QuestResidentTextRole::EscapeAttempt,
    )
    .map_err(|cause| error(cause.to_string()))?;
    let failure = extract_quest_resident_text_unit(
        map,
        text,
        code_table,
        &failure_node,
        QUEST_RESIDENT_ESCAPE_FAILURE_TEXT_ID,
        QuestResidentTextRole::EscapeFailure,
    )
    .map_err(|cause| error(cause.to_string()))?;
    let source_units = [attempt, failure];
    let units = combine_quest_resident_text_units([source_units.as_slice()])
        .map_err(|cause| error(cause.to_string()))?;

    Ok(QuestResidentEscapeTextAnalysis {
        selector_runtime: QUEST_RESIDENT_ESCAPE_SELECTOR_RUNTIME,
        selector_field_offset: header[1],
        selector_targets,
        attempt_path,
        outcome_compare,
        units,
    })
}

/// 지원 원본의 OP09·OP14 edge와 두 보호 원문을 fail closed로 검증한다.
pub fn verify_box_quest_resident_escape_text(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    encounter: &QuestResidentEncounterPathAnalysis,
) -> Result<QuestResidentEscapeTextAnalysis, QuestResidentEscapeTextError> {
    let analysis = audit_quest_resident_escape_text(map, text, code_table, encounter)?;
    let attempt_opcodes = analysis
        .attempt_path
        .nodes
        .iter()
        .map(|node| node.opcode)
        .collect::<Vec<_>>();
    let compare = analysis.outcome_compare;
    let expected_totals = [
        BOX_QUEST_RESIDENT_ESCAPE_UNITS,
        BOX_QUEST_RESIDENT_ESCAPE_GLYPHS,
        BOX_QUEST_RESIDENT_ESCAPE_JAPANESE_GLYPHS,
        BOX_QUEST_RESIDENT_ESCAPE_CONTROLS,
        BOX_QUEST_RESIDENT_ESCAPE_RUNS,
        0,
    ];
    let actual_totals = [
        analysis.units.units.len(),
        analysis.units.glyph_occurrences,
        analysis.units.japanese_glyph_occurrences,
        analysis.units.control_occurrences,
        analysis.units.runs,
        analysis.units.runtime_observed_units,
    ];
    if text_slice(
        map,
        text,
        QUEST_RESIDENT_ESCAPE_SELECTOR_RUNTIME,
        ESCAPE_SELECTOR_RAW.len(),
        "OP09 header",
    )? != ESCAPE_SELECTOR_RAW
        || analysis.selector_field_offset != ESCAPE_SELECTOR_FIELD_OFFSET
        || analysis.selector_targets != ESCAPE_SELECTOR_TARGETS
        || analysis.attempt_path.root_runtime != QUEST_RESIDENT_ESCAPE_ATTEMPT_RUNTIME
        || attempt_opcodes != [0x21, 0xC8, 0x14]
        || analysis.attempt_path.nodes.last().map(|node| node.runtime)
            != Some(QUEST_RESIDENT_ESCAPE_OUTCOME_COMPARE_RUNTIME)
        || compare.operation != QuestSeqBaseCompareOperation::Equal
        || compare.value_bytes != 4
        || compare.left_source != QuestSeqBaseCompareOperandSource::ObjectField
        || compare.left_parameter != Some(ESCAPE_SELECTOR_FIELD_OFFSET.into())
        || compare.right_source != QuestSeqBaseCompareOperandSource::Inline
        || compare.right_parameter != 1
        || compare.branch_target_runtime != 0x8008_C6D0
        || compare.fallthrough_runtime != QUEST_RESIDENT_ESCAPE_FAILURE_RUNTIME
        || compare.runtime_probe_address.is_some()
        || compare.runtime_probe_length.is_some()
        || actual_totals != expected_totals
    {
        return Err(error(format!(
            "QUEST resident escape source 경계 drift: selector={:02X?}/{:08X?}, path={attempt_opcodes:02X?}, compare={compare:?}, totals={actual_totals:?}",
            text_slice(
                map,
                text,
                QUEST_RESIDENT_ESCAPE_SELECTOR_RUNTIME,
                ESCAPE_SELECTOR_RAW.len(),
                "OP09 header"
            )?,
            analysis.selector_targets,
        )));
    }

    let expectations = [
        (
            QUEST_RESIDENT_ESCAPE_ATTEMPT_TEXT_ID,
            QuestResidentTextRole::EscapeAttempt,
            QUEST_RESIDENT_ESCAPE_ATTEMPT_RUNTIME,
            0x8008_C660,
            0x8008_C688,
            13,
            9,
            4,
            ESCAPE_ATTEMPT_RUN_SOURCE,
            QUEST_RESIDENT_ESCAPE_ATTEMPT_TEXT_SHA256,
        ),
        (
            QUEST_RESIDENT_ESCAPE_FAILURE_TEXT_ID,
            QuestResidentTextRole::EscapeFailure,
            QUEST_RESIDENT_ESCAPE_FAILURE_RUNTIME,
            0x8008_C69C,
            0x8008_C6C8,
            16,
            11,
            3,
            ESCAPE_FAILURE_RUN_SOURCE,
            QUEST_RESIDENT_ESCAPE_FAILURE_TEXT_SHA256,
        ),
    ];
    for (unit, expected) in analysis.units.units.iter().zip(expectations) {
        let runs = unit
            .runs
            .iter()
            .map(|run| run.source.as_str())
            .collect::<Vec<_>>();
        let digest = format!("{:x}", Sha256::digest(&unit.protected_raw));
        if unit.id != expected.0
            || unit.role != expected.1
            || unit.record_runtime != expected.2
            || unit.body_runtime != expected.3
            || unit.end_runtime != expected.4
            || unit.glyph_occurrences != expected.5
            || unit.japanese_glyph_occurrences != expected.6
            || unit.control_occurrences != expected.7
            || runs != [expected.8]
            || unit.protected_source != expected.8
            || digest != expected.9
            || unit.runtime_evidence.is_some()
        {
            return Err(error(format!(
                "{}: QUEST resident escape 보호 source drift: role={:?}, record/body/end=0x{:08X}/0x{:08X}/0x{:08X}, glyph/Japanese/control={}/{}/{}, runs={runs:?}, source={:?}, SHA-256={digest}",
                expected.0,
                unit.role,
                unit.record_runtime,
                unit.body_runtime,
                unit.end_runtime,
                unit.glyph_occurrences,
                unit.japanese_glyph_occurrences,
                unit.control_occurrences,
                unit.protected_source,
            )));
        }
    }
    Ok(analysis)
}

#[cfg(test)]
#[path = "quest_resident_escape_text_tests.rs"]
mod tests;
