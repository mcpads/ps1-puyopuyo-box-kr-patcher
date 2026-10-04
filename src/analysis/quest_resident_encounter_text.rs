//! `QUEST.EXE` 자연 조우의 동적 이름·등장문·선택지 OP21 원문 단위.
//!
//! 보이는 `たたかう / にげる`만 잘라내지 않고 graph-bound OP21 전체를
//! 보호한다. `FF1E mode 0`은 별도 전투 이름 표가 채우는 동적 위치로 남기며,
//! 앞뒤 glyph run과 모든 control을 원형 보존한다. 이 정적 단위는 번역문,
//! 조판, writer나 runtime 표시 승인을 포함하지 않는다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::disasm::QUEST_FIXED_TEXT_POINTER_RUNTIME;
use crate::font::QuestCodeTable;
use crate::isa::{verify_typed_source, InstructionKind as K, IsaProfile, Reg};

pub const QUEST_ENCOUNTER_NAME_GLYPH_RETURN_RUNTIME: u32 = 0x8003_C704;
pub const QUEST_ENCOUNTER_NAME_STREAM_CURSOR_RUNTIME: u32 = 0x8008_C5F2;

// renderer가 검사하는 RA와 S4+0x1C의 의미를 불변 원본 명령에 결속한다.
fn verify_name_consumer(map: &ExeMap, text: &[u8]) -> Result<(), QuestResidentEncounterTextError> {
    for (runtime, instructions) in [
        (
            0x8003_C678,
            vec![K::Addu {
                rd: Reg::S4,
                rs: Reg::A0,
                rt: Reg::ZERO,
            }],
        ),
        (
            0x8003_C690,
            vec![
                K::Lw {
                    rt: Reg::V0,
                    base: Reg::S4,
                    offset: 0x1C,
                },
                K::Sll {
                    rd: Reg::ZERO,
                    rt: Reg::ZERO,
                    sa: 0,
                },
                K::Addiu {
                    rt: Reg::V1,
                    rs: Reg::V0,
                    imm: 2,
                },
                K::Sw {
                    rt: Reg::V1,
                    base: Reg::S4,
                    offset: 0x1C,
                },
            ],
        ),
        (
            QUEST_ENCOUNTER_NAME_GLYPH_RETURN_RUNTIME - 12,
            vec![
                K::Lhu {
                    rt: Reg::A1,
                    base: Reg::V0,
                    offset: 0,
                },
                K::Jal {
                    target: crate::disasm::QUEST_GLYPH_RENDERER_RUNTIME,
                },
                K::Addiu {
                    rt: Reg::S0,
                    rs: Reg::S0,
                    imm: 2,
                },
            ],
        ),
    ] {
        let code = verify_typed_source(
            IsaProfile::DECLARED,
            runtime,
            "analysis::quest_resident_encounter_text::name_consumer",
            &instructions,
        )
        .map_err(|e| error(e.to_string()))?;
        let offset = map
            .text_offset(runtime)
            .ok_or_else(|| error("이름 consumer 좌표 누락"))?;
        if text.get(offset..offset + code.bytes().len()) != Some(code.bytes()) {
            return Err(error("이름 consumer 원본 명령 불일치"));
        }
    }
    Ok(())
}

use super::quest_battle_name_table::{QuestBattleNameTableAnalysis, BOX_QUEST_BATTLE_NAME_ENTRIES};
use super::quest_resident_encounter_path::{
    QuestResidentEncounterPathAnalysis, QUEST_RESIDENT_ENCOUNTER_NEXT_SELECTOR_RUNTIME,
    QUEST_RESIDENT_ENCOUNTER_TEXT_RUNTIME,
};
use super::quest_resident_text_units::{
    extract_quest_resident_text_unit, QuestResidentTextRole, QuestResidentTextTokenKind,
    QuestResidentTextUnit,
};

pub const QUEST_RESIDENT_ENCOUNTER_TEXT_ID: &str =
    "QUEST-EXE-RESIDENT-TEXT:encounter-menu:0x8008C5D4";
pub const QUEST_RESIDENT_ENCOUNTER_TEXT_SHA256: &str =
    "db2abeca7778f783cc4f92f0a796db77a467891e7d1c847dadb13ab7cb0a1e38";
pub const BOX_QUEST_RESIDENT_ENCOUNTER_TEXT_GLYPHS: usize = 18;
pub const BOX_QUEST_RESIDENT_ENCOUNTER_JAPANESE_GLYPHS: usize = 13;
pub const BOX_QUEST_RESIDENT_ENCOUNTER_TEXT_CONTROLS: usize = 19;
pub const BOX_QUEST_RESIDENT_ENCOUNTER_TEXT_RUNS: usize = 4;

const ENCOUNTER_RUN_SOURCES: [&str; BOX_QUEST_RESIDENT_ENCOUNTER_TEXT_RUNS] = [
    "\u{3000}\u{3000}",
    "が\u{3000}あらわれた！！",
    "たたかう",
    "にげる",
];
const ENCOUNTER_PROTECTED_SOURCE: &str = "\u{3000}\u{3000}が\u{3000}あらわれた！！たたかうにげる";

/// 동적 이름 control과 59-entry 이름 모집단에 결속한 OP21 단위.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentEncounterTextAnalysis {
    pub unit: QuestResidentTextUnit,
    pub dynamic_name_controls: usize,
    pub battle_name_entries: usize,
    pub fixed_text_state_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentEncounterTextError(String);

impl fmt::Display for QuestResidentEncounterTextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentEncounterTextError {}

fn error(message: impl Into<String>) -> QuestResidentEncounterTextError {
    QuestResidentEncounterTextError(message.into())
}

/// 조건부 display path의 유일한 OP21을 전체 원문 단위로 추출한다.
pub fn audit_quest_resident_encounter_text(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    path: &QuestResidentEncounterPathAnalysis,
    battle_names: &QuestBattleNameTableAnalysis,
) -> Result<QuestResidentEncounterTextAnalysis, QuestResidentEncounterTextError> {
    if battle_names.fixed_text_state_runtime != QUEST_FIXED_TEXT_POINTER_RUNTIME {
        return Err(error(format!(
            "QUEST resident encounter FF1E pointer와 battle name state가 다름: 0x{:08X}/0x{QUEST_FIXED_TEXT_POINTER_RUNTIME:08X}",
            battle_names.fixed_text_state_runtime,
        )));
    }
    let mut op21_nodes = path
        .display_path
        .nodes
        .iter()
        .filter(|node| node.opcode == 0x21);
    let node = op21_nodes
        .next()
        .ok_or_else(|| error("QUEST resident encounter display path에 OP21이 없음"))?;
    if op21_nodes.next().is_some() {
        return Err(error(
            "QUEST resident encounter display path에 OP21이 둘 이상임",
        ));
    }
    let unit = extract_quest_resident_text_unit(
        map,
        text,
        code_table,
        node,
        QUEST_RESIDENT_ENCOUNTER_TEXT_ID,
        QuestResidentTextRole::EncounterMenu,
    )
    .map_err(|cause| error(cause.to_string()))?;
    let dynamic_name_tokens = unit
        .tokens
        .iter()
        .filter(|token| {
            token.kind == QuestResidentTextTokenKind::Control && token.control_code == Some(0x1E)
        })
        .collect::<Vec<_>>();
    if dynamic_name_tokens.len() != 1 || dynamic_name_tokens[0].raw != [0x1E, 0xFF, 0, 0] {
        return Err(error(format!(
            "QUEST resident encounter FF1E mode-0 분모 drift: {:?}",
            dynamic_name_tokens
                .iter()
                .map(|token| token.raw.as_slice())
                .collect::<Vec<_>>(),
        )));
    }

    Ok(QuestResidentEncounterTextAnalysis {
        dynamic_name_controls: dynamic_name_tokens.len(),
        battle_name_entries: battle_names.entries.len(),
        fixed_text_state_runtime: battle_names.fixed_text_state_runtime,
        unit,
    })
}

/// 지원 원본의 전체 조우 OP21과 동적 이름 결속을 fail closed로 검증한다.
pub fn verify_box_quest_resident_encounter_text(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    path: &QuestResidentEncounterPathAnalysis,
    battle_names: &QuestBattleNameTableAnalysis,
) -> Result<QuestResidentEncounterTextAnalysis, QuestResidentEncounterTextError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST resident encounter text 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len,
        )));
    }
    verify_name_consumer(map, text)?;
    let analysis = audit_quest_resident_encounter_text(map, text, code_table, path, battle_names)?;
    let unit = &analysis.unit;
    let actual_runs = unit
        .runs
        .iter()
        .map(|run| run.source.as_str())
        .collect::<Vec<_>>();
    let actual_sha256 = format!("{:x}", Sha256::digest(&unit.protected_raw));
    if unit.id != QUEST_RESIDENT_ENCOUNTER_TEXT_ID
        || unit.role != QuestResidentTextRole::EncounterMenu
        || unit.record_runtime != QUEST_RESIDENT_ENCOUNTER_TEXT_RUNTIME
        || unit.body_runtime != QUEST_RESIDENT_ENCOUNTER_TEXT_RUNTIME + 4
        || unit.end_runtime != QUEST_RESIDENT_ENCOUNTER_NEXT_SELECTOR_RUNTIME
        || unit.glyph_occurrences != BOX_QUEST_RESIDENT_ENCOUNTER_TEXT_GLYPHS
        || unit.japanese_glyph_occurrences != BOX_QUEST_RESIDENT_ENCOUNTER_JAPANESE_GLYPHS
        || unit.control_occurrences != BOX_QUEST_RESIDENT_ENCOUNTER_TEXT_CONTROLS
        || unit.runs.len() != BOX_QUEST_RESIDENT_ENCOUNTER_TEXT_RUNS
        || actual_runs != ENCOUNTER_RUN_SOURCES
        || unit.protected_source != ENCOUNTER_PROTECTED_SOURCE
        || actual_sha256 != QUEST_RESIDENT_ENCOUNTER_TEXT_SHA256
        || analysis.dynamic_name_controls != 1
        || !unit.tokens.iter().any(|token| {
            token.control_code == Some(0x1E)
                && token.runtime + 2 == QUEST_ENCOUNTER_NAME_STREAM_CURSOR_RUNTIME
        })
        || analysis.battle_name_entries != BOX_QUEST_BATTLE_NAME_ENTRIES
        || analysis.fixed_text_state_runtime != QUEST_FIXED_TEXT_POINTER_RUNTIME
        || unit.runtime_evidence.is_some()
    {
        return Err(error(format!(
            "QUEST resident encounter 보호 source drift: id={}, role={:?}, record/body/end=0x{:08X}/0x{:08X}/0x{:08X}, glyph/Japanese/control={}/{}/{}, runs={actual_runs:?}, source={:?}, SHA-256={actual_sha256}, dynamic/battle={}/{}, state=0x{:08X}, runtime={:?}",
            unit.id,
            unit.role,
            unit.record_runtime,
            unit.body_runtime,
            unit.end_runtime,
            unit.glyph_occurrences,
            unit.japanese_glyph_occurrences,
            unit.control_occurrences,
            unit.protected_source,
            analysis.dynamic_name_controls,
            analysis.battle_name_entries,
            analysis.fixed_text_state_runtime,
            unit.runtime_evidence,
        )));
    }
    Ok(analysis)
}

#[cfg(test)]
#[path = "quest_resident_encounter_text_tests.rs"]
mod tests;
