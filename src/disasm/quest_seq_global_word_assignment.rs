//! QUEST 공용 SEQ `OP11`의 즉시값 전역 word 대입 형식.
//!
//! `OP11`은 source·destination·연산 종류에 따라 여러 경로를 가진다. 이
//! 모듈은 그중 byte 1이 `0x40`인 형식만 소유한다. 이 형식은 record `+4`의
//! 즉시 word를 `0x800DB6C0 + record[2] * 4`에 대입하고 8바이트 전진한다.
//! 다른 `OP11` mode를 같은 의미로 넓히지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::quest_seq_opcode_handler;

use super::linear;

pub const QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPCODE: u8 = 0x11;
pub const QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_MODE: u8 = 0x40;
pub const QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_BYTES: usize = 8;
pub const QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_HANDLER_RUNTIME: u32 = 0x8002_0A48;
pub const QUEST_SEQ_GLOBAL_WORD_STATE_BASE_RUNTIME: u32 = 0x800D_B6C0;
pub const QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPERATION_TABLE_RUNTIME: u32 = 0x8001_030C;
pub const QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPERATION_RUNTIME: u32 = 0x8002_0BB4;

/// `OP11/0x40` 한 record가 대입하는 전역 word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqGlobalWordAssignment {
    pub raw: [u8; QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_BYTES],
    pub state_index: u8,
    /// handler가 읽지 않는 record `+3` byte.
    pub unconsumed_parameter: u8,
    pub value: u32,
}

impl QuestSeqGlobalWordAssignment {
    pub fn state_runtime(self) -> Option<u32> {
        QUEST_SEQ_GLOBAL_WORD_STATE_BASE_RUNTIME
            .checked_add(u32::from(self.state_index).checked_mul(4)?)
    }
}

/// byte 1이 `0x40`인 `OP11` record만 구조화한다.
pub fn parse_quest_seq_global_word_assignment(raw: &[u8]) -> Option<QuestSeqGlobalWordAssignment> {
    let record: [u8; QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_BYTES] = raw
        .get(..QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_BYTES)?
        .try_into()
        .ok()?;
    if record[0] != QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPCODE
        || record[1] != QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_MODE
    {
        return None;
    }
    Some(QuestSeqGlobalWordAssignment {
        raw: record,
        state_index: record[2],
        unconsumed_parameter: record[3],
        value: u32::from_le_bytes(record[4..8].try_into().expect("8바이트 record를 확인함")),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqGlobalWordAssignmentSummary {
    pub opcode: u8,
    pub mode: u8,
    pub record_bytes: usize,
    pub state_base_runtime: u32,
    pub operation_table_runtime: u32,
    pub assignment_operation_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqGlobalWordAssignmentError(String);

impl fmt::Display for QuestSeqGlobalWordAssignmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestSeqGlobalWordAssignmentError {}

fn error(message: impl Into<String>) -> QuestSeqGlobalWordAssignmentError {
    QuestSeqGlobalWordAssignmentError(message.into())
}

/// 지원 `QUEST.EXE`에서 `OP11/0x40`의 source·연산·destination·전진을 검증한다.
pub fn verify_box_quest_seq_global_word_assignment(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqGlobalWordAssignmentSummary, QuestSeqGlobalWordAssignmentError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST global word assignment 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len,
        )));
    }
    if quest_seq_opcode_handler(QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPCODE)
        != Some(QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_HANDLER_RUNTIME)
    {
        return Err(error("QUEST OP11 dispatch handler 불일치"));
    }
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error("QUEST.EXE text 길이가 4바이트 정렬이 아님"))?;

    for (address, expected, kind) in [
        (
            0x8002_0A6C,
            "lbu s3, 1(stream)",
            K::Lbu {
                rt: Reg(19),
                base: Reg::A0,
                offset: 1,
            },
        ),
        (
            0x8002_0A74,
            "andi v1, s3, 0x30",
            K::Andi {
                rt: Reg::V1,
                rs: Reg(19),
                imm: 0x30,
            },
        ),
        (
            0x8002_0AB8,
            "lw s1, 4(stream)",
            K::Lw {
                rt: Reg(17),
                base: Reg::A0,
                offset: 4,
            },
        ),
        (
            0x8002_0B38,
            "lw v0, 0x1C(object)",
            K::Lw {
                rt: Reg::V0,
                base: Reg(18),
                offset: 0x1C,
            },
        ),
        (
            0x8002_0B40,
            "lbu v0, 2(stream)",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 2,
            },
        ),
        (
            0x8002_0B44,
            "addiu v1, v1, global-state base low",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: -18_752,
            },
        ),
        (
            0x8002_0B4C,
            "sll v0, v0, 2",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8002_0B88,
            "andi v1, s3, operation mask",
            K::Andi {
                rt: Reg::V1,
                rs: Reg(19),
                imm: 0x0F,
            },
        ),
        (
            QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPERATION_RUNTIME,
            "assignment path moves immediate value",
            K::J {
                target: 0x8002_0C6C,
            },
        ),
        (
            0x8002_0BB8,
            "move a0, s1",
            K::Addu {
                rd: Reg::A0,
                rs: Reg(17),
                rt: Reg::ZERO,
            },
        ),
        (
            0x8002_0CF8,
            "sw result, 0(global word)",
            K::Sw {
                rt: Reg(16),
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8002_0D1C,
            "addiu stream, stream, 8",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_BYTES as i16,
            },
        ),
        (
            0x8002_0D20,
            "sw stream, 0x1C(object)",
            K::Sw {
                rt: Reg::V0,
                base: Reg(18),
                offset: 0x1C,
            },
        ),
    ] {
        require(&instructions, address, expected, kind)?;
    }

    let table_offset = map
        .text_offset(QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPERATION_TABLE_RUNTIME)
        .ok_or_else(|| error("QUEST OP11 operation table이 EXE text 밖임"))?;
    let operation = text
        .get(table_offset..table_offset + 4)
        .ok_or_else(|| error("QUEST OP11 assignment operation word가 잘림"))?;
    let operation = u32::from_le_bytes(operation.try_into().expect("4바이트 범위를 확인함"));
    if operation != QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPERATION_RUNTIME {
        return Err(error(format!(
            "QUEST OP11 assignment operation 불일치: expected=0x{QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPERATION_RUNTIME:08X}, actual=0x{operation:08X}"
        )));
    }

    Ok(QuestSeqGlobalWordAssignmentSummary {
        opcode: QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPCODE,
        mode: QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_MODE,
        record_bytes: QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_BYTES,
        state_base_runtime: QUEST_SEQ_GLOBAL_WORD_STATE_BASE_RUNTIME,
        operation_table_runtime: QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPERATION_TABLE_RUNTIME,
        assignment_operation_runtime: QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPERATION_RUNTIME,
    })
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: &str,
    kind: K,
) -> Result<(), QuestSeqGlobalWordAssignmentError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| error(format!("QUEST OP11 소비 명령이 text 밖임: 0x{address:08X}")))?;
    if actual.kind != kind {
        return Err(error(format!(
            "QUEST OP11 소비 명령 불일치: address=0x{address:08X}, expected={expected}, actual={:?}",
            actual.kind,
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_seq_global_word_assignment_tests.rs"]
mod tests;
