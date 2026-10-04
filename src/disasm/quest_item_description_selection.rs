//! `ITEM_DAT.SEQ` 설명을 만드는 다섯 caller와 커서 전환 경계.
//!
//! 세 목록 callback은 입력 helper가 0을 반환한 순회에서 설명 constructor를
//! 부른다. 방향 입력으로 3을 반환한 순회는 목록 state와 선택 index만
//! 갱신한다. 따라서 원본에는 커서 변경과 같은 순회에서 설명을 다시 만드는
//! 직접 edge가 없다. 이후 무입력 순회가 실제로 오는지와 기존 설명 객체의
//! 수명은 runtime 판정으로 남긴다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{decode_slice, Instruction, InstructionKind as K, Reg};

use super::QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME;

pub const QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_CALL_RUNTIMES: [u32; 5] = [
    0x8003_1CE0,
    0x8003_2998,
    0x8003_8BF8,
    0x8003_9504,
    0x8003_AA8C,
];

const QUEST_ITEM_LIST_INPUT_RUNTIME: u32 = 0x8002_C434;
const QUEST_ITEM_LIST_CURSOR_CHANGED_RESULT: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestItemDescriptionSelectionPath {
    pub input_call_runtime: u32,
    pub no_input_branch_runtime: u32,
    pub constructor_call_runtime: u32,
    pub cursor_changed_branch_runtime: u32,
    pub cursor_changed_handler_runtime: u32,
    pub state_reset_runtime: u32,
    pub selected_index_store_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemDescriptionSelectionSummary {
    pub constructor_call_runtimes: Vec<u32>,
    pub selection_paths: Vec<QuestItemDescriptionSelectionPath>,
    pub cursor_changed_result: u8,
    pub cursor_change_recreates_in_same_iteration: bool,
    pub recreation_requires_later_no_input_iteration: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemDescriptionSelectionError(String);

impl fmt::Display for QuestItemDescriptionSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemDescriptionSelectionError {}

fn error(message: impl Into<String>) -> QuestItemDescriptionSelectionError {
    QuestItemDescriptionSelectionError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    runtime: u32,
) -> Result<&Instruction, QuestItemDescriptionSelectionError> {
    instructions
        .binary_search_by_key(&runtime, |instruction| instruction.addr)
        .ok()
        .and_then(|index| instructions.get(index))
        .ok_or_else(|| {
            error(format!(
                "QUEST item description selection 명령이 없음: 0x{runtime:08X}"
            ))
        })
}

fn require(
    instructions: &[Instruction],
    runtime: u32,
    expected: K,
    role: &str,
) -> Result<(), QuestItemDescriptionSelectionError> {
    let actual = instruction_at(instructions, runtime)?;
    if actual.kind != expected {
        return Err(error(format!(
            "QUEST item description selection {role} drift: 0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct SelectionPathSpec {
    path: QuestItemDescriptionSelectionPath,
    parent: Reg,
    no_input_branch_offset: i16,
    cursor_changed_branch_offset: i16,
}

const SELECTION_PATHS: [SelectionPathSpec; 3] = [
    SelectionPathSpec {
        path: QuestItemDescriptionSelectionPath {
            input_call_runtime: 0x8003_8B64,
            no_input_branch_runtime: 0x8003_8B84,
            constructor_call_runtime: 0x8003_8BF8,
            cursor_changed_branch_runtime: 0x8003_8B9C,
            cursor_changed_handler_runtime: 0x8003_8CF0,
            state_reset_runtime: 0x8003_8D30,
            selected_index_store_runtime: 0x8003_8D38,
        },
        parent: Reg::S5,
        no_input_branch_offset: 9,
        cursor_changed_branch_offset: 84,
    },
    SelectionPathSpec {
        path: QuestItemDescriptionSelectionPath {
            input_call_runtime: 0x8003_9470,
            no_input_branch_runtime: 0x8003_9490,
            constructor_call_runtime: 0x8003_9504,
            cursor_changed_branch_runtime: 0x8003_94A8,
            cursor_changed_handler_runtime: 0x8003_95F0,
            state_reset_runtime: 0x8003_9630,
            selected_index_store_runtime: 0x8003_9638,
        },
        parent: Reg::S5,
        no_input_branch_offset: 9,
        cursor_changed_branch_offset: 81,
    },
    SelectionPathSpec {
        path: QuestItemDescriptionSelectionPath {
            input_call_runtime: 0x8003_A9F8,
            no_input_branch_runtime: 0x8003_AA18,
            constructor_call_runtime: 0x8003_AA8C,
            cursor_changed_branch_runtime: 0x8003_AA30,
            cursor_changed_handler_runtime: 0x8003_ABEC,
            state_reset_runtime: 0x8003_AC2C,
            selected_index_store_runtime: 0x8003_AC34,
        },
        parent: Reg::S6,
        no_input_branch_offset: 9,
        cursor_changed_branch_offset: 110,
    },
];

fn verify_input_result(
    instructions: &[Instruction],
) -> Result<(), QuestItemDescriptionSelectionError> {
    require(
        instructions,
        0x8002_C614,
        K::J {
            target: 0x8002_C644,
        },
        "cursor changed return jump",
    )?;
    require(
        instructions,
        0x8002_C618,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: i16::from(QUEST_ITEM_LIST_CURSOR_CHANGED_RESULT),
        },
        "cursor changed return value",
    )
}

fn verify_selection_path(
    instructions: &[Instruction],
    spec: SelectionPathSpec,
) -> Result<(), QuestItemDescriptionSelectionError> {
    let path = spec.path;
    require(
        instructions,
        path.input_call_runtime,
        K::Jal {
            target: QUEST_ITEM_LIST_INPUT_RUNTIME,
        },
        "input call",
    )?;
    require(
        instructions,
        path.no_input_branch_runtime,
        K::Beq {
            rs: Reg::V1,
            rt: Reg::ZERO,
            offset: spec.no_input_branch_offset,
        },
        "no-input constructor branch",
    )?;
    require(
        instructions,
        path.constructor_call_runtime,
        K::Jal {
            target: QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME,
        },
        "description constructor call",
    )?;
    require(
        instructions,
        path.cursor_changed_branch_runtime,
        K::Beq {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: spec.cursor_changed_branch_offset,
        },
        "cursor changed branch",
    )?;
    require(
        instructions,
        path.cursor_changed_handler_runtime,
        K::Addu {
            rd: Reg::A1,
            rs: Reg::A0,
            rt: Reg::ZERO,
        },
        "cursor changed handler entry",
    )?;
    require(
        instructions,
        path.state_reset_runtime,
        K::Sh {
            rt: Reg::S0,
            base: spec.parent,
            offset: 0x00A2,
        },
        "cursor changed state reset",
    )?;
    require(
        instructions,
        path.selected_index_store_runtime,
        K::Sw {
            rt: Reg::V0,
            base: spec.parent,
            offset: 0x0090,
        },
        "cursor changed selected index store",
    )
}

fn constructor_calls(instructions: &[Instruction]) -> Vec<u32> {
    instructions
        .iter()
        .filter_map(|instruction| {
            (instruction.kind
                == K::Jal {
                    target: QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME,
                })
            .then_some(instruction.addr)
        })
        .collect()
}

/// 지원 `QUEST.EXE`의 설명 constructor caller와 세 커서 callback을 검증한다.
pub fn verify_box_quest_item_description_selection(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestItemDescriptionSelectionSummary, QuestItemDescriptionSelectionError> {
    if map.name != "QUEST.EXE" {
        return Err(error(format!(
            "QUEST item description selection 사상 이름 불일치: {}",
            map.name
        )));
    }
    if text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST item description selection text 길이 불일치: map={}, actual={}",
            map.text_len,
            text.len()
        )));
    }
    let instructions = decode_slice(map.load_addr, text)
        .ok_or_else(|| error("QUEST item description selection text가 4바이트 정렬이 아님"))?;
    verify_input_result(&instructions)?;
    for spec in SELECTION_PATHS {
        verify_selection_path(&instructions, spec)?;
    }
    let constructor_call_runtimes = constructor_calls(&instructions);
    if constructor_call_runtimes != QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_CALL_RUNTIMES {
        return Err(error(format!(
            "QUEST item description constructor caller 분모 불일치: expected={QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_CALL_RUNTIMES:08X?}, actual={constructor_call_runtimes:08X?}"
        )));
    }

    Ok(QuestItemDescriptionSelectionSummary {
        constructor_call_runtimes,
        selection_paths: SELECTION_PATHS.iter().map(|spec| spec.path).collect(),
        cursor_changed_result: QUEST_ITEM_LIST_CURSOR_CHANGED_RESULT,
        cursor_change_recreates_in_same_iteration: false,
        recreation_requires_later_no_input_iteration: true,
    })
}

#[cfg(test)]
#[path = "quest_item_description_selection_tests.rs"]
mod tests;
