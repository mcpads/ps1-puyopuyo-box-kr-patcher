//! QUEST 확장 SEQ `OPA1`의 bit flag 조회 경로.
//!
//! handler는 record `+2`의 halfword와 `+4`의 byte를 조회 함수에 넘긴다.
//! 조회 함수는 두 값을 `0x800DB6C0` bitset의 word 주소와 mask로 바꾸고,
//! 해당 bit가 설정됐는지를 0 또는 1로 반환한다. `OPA1`이 그 결과와
//! record `+1`을 비교해 다음 stream을 고르는 포인터 소비는
//! [`super::quest_seq`]가 별도로 검증한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::quest_seq_opcode_handler;

use super::linear;
use super::quest_seq_base_transition::QUEST_SEQ_BRANCH_STATE_BASE_RUNTIME;

pub const QUEST_SEQ_FLAG_QUERY_OPCODE: u8 = 0xA1;
pub const QUEST_SEQ_FLAG_QUERY_HANDLER_RUNTIME: u32 = 0x8007_8A40;
pub const QUEST_SEQ_FLAG_QUERY_RUNTIME: u32 = 0x8006_687C;
pub const QUEST_SEQ_FLAG_STATE_BASE_RUNTIME: u32 = QUEST_SEQ_BRANCH_STATE_BASE_RUNTIME;
pub const QUEST_SEQ_FLAG_QUERY_RECORD_BYTES: usize = 12;
pub const QUEST_SEQ_FLAG_EXPECTED_OFFSET: usize = 1;
pub const QUEST_SEQ_FLAG_BASE_OFFSET: usize = 2;
pub const QUEST_SEQ_FLAG_OFFSET_OFFSET: usize = 4;
pub const QUEST_SEQ_FLAG_BRANCH_POINTER_OFFSET: usize = 8;

/// `OPA1` record의 두 flag operand가 가리키는 runtime word와 bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqFlagProbe {
    pub word_index: u32,
    pub runtime_address: u32,
    pub bit_index: u8,
    pub mask: u32,
}

impl QuestSeqFlagProbe {
    /// 조회 함수가 반환하는 0 또는 1을 계산한다.
    pub const fn queried_value(self, word: u32) -> u8 {
        if word & self.mask == 0 {
            0
        } else {
            1
        }
    }
}

/// 원본 조회 함수와 같은 산술로 flag word와 mask를 계산한다.
pub fn resolve_quest_seq_flag_probe(flag_base: u16, flag_offset: u8) -> Option<QuestSeqFlagProbe> {
    let combined_offset = u32::from(flag_offset).checked_add(u32::from(flag_base & 0x00FF))?;
    let wrapped_offset = combined_offset & u32::from(u16::MAX);
    let word_index = u32::from(flag_base >> 8).checked_add(wrapped_offset >> 5)?;
    let runtime_address =
        QUEST_SEQ_FLAG_STATE_BASE_RUNTIME.checked_add(word_index.checked_mul(4)?)?;
    let bit_index = u8::try_from(wrapped_offset & 0x1F).ok()?;
    let mask = 1u32.checked_shl(u32::from(bit_index))?;
    Some(QuestSeqFlagProbe {
        word_index,
        runtime_address,
        bit_index,
        mask,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqFlagQuerySummary {
    pub opcode: u8,
    pub handler_runtime: u32,
    pub query_runtime: u32,
    pub state_base_runtime: u32,
    pub record_bytes: usize,
    pub expected_offset: usize,
    pub flag_base_offset: usize,
    pub flag_offset_offset: usize,
    pub branch_pointer_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqFlagQueryError(String);

impl fmt::Display for QuestSeqFlagQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestSeqFlagQueryError {}

fn error(message: impl Into<String>) -> QuestSeqFlagQueryError {
    QuestSeqFlagQueryError(message.into())
}

/// 지원 `QUEST.EXE`에서 `OPA1` operand와 bitset 조회 산술을 검증한다.
pub fn verify_box_quest_seq_flag_query(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqFlagQuerySummary, QuestSeqFlagQueryError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST OPA1 flag query 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len,
        )));
    }
    if quest_seq_opcode_handler(QUEST_SEQ_FLAG_QUERY_OPCODE)
        != Some(QUEST_SEQ_FLAG_QUERY_HANDLER_RUNTIME)
    {
        return Err(error("QUEST OPA1 dispatch handler 불일치"));
    }
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error("QUEST.EXE text 길이가 4바이트 정렬이 아님"))?;

    for (address, expected, kind) in [
        (
            0x8007_8A50,
            "lw v0, 0x1C(object)",
            K::Lw {
                rt: Reg::V0,
                base: Reg(16),
                offset: 0x1C,
            },
        ),
        (
            0x8007_8A58,
            "lhu a0, 2(stream)",
            K::Lhu {
                rt: Reg::A0,
                base: Reg::V0,
                offset: QUEST_SEQ_FLAG_BASE_OFFSET as i16,
            },
        ),
        (
            0x8007_8A5C,
            "lbu a1, 4(stream)",
            K::Lbu {
                rt: Reg::A1,
                base: Reg::V0,
                offset: QUEST_SEQ_FLAG_OFFSET_OFFSET as i16,
            },
        ),
        (
            0x8007_8A60,
            "jal flag query",
            K::Jal {
                target: QUEST_SEQ_FLAG_QUERY_RUNTIME,
            },
        ),
        (
            0x8007_8A70,
            "lbu v1, 1(stream)",
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A0,
                offset: QUEST_SEQ_FLAG_EXPECTED_OFFSET as i16,
            },
        ),
        (
            0x8007_8A74,
            "andi v0, v0, 0x00FF",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x00FF,
            },
        ),
        (
            0x8007_8A78,
            "beq queried, expected, fallthrough",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::V1,
                offset: 4,
            },
        ),
        (
            0x8006_687C,
            "andi v0, a0, 0x00FF",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::A0,
                imm: 0x00FF,
            },
        ),
        (
            0x8006_6880,
            "addu a1, a1, v0",
            K::Addu {
                rd: Reg::A1,
                rs: Reg::A1,
                rt: Reg::V0,
            },
        ),
        (
            0x8006_6884,
            "andi a0, a0, 0xFFFF",
            K::Andi {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0xFFFF,
            },
        ),
        (
            0x8006_6888,
            "andi a2, a1, 0x001F",
            K::Andi {
                rt: Reg::A2,
                rs: Reg::A1,
                imm: 0x001F,
            },
        ),
        (
            0x8006_688C,
            "addiu v1, zero, 1",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            0x8006_6890,
            "lui v0, state base high",
            K::Lui {
                rt: Reg::V0,
                imm: 0x800E,
            },
        ),
        (
            0x8006_6894,
            "addiu v0, v0, state base low",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -18_752,
            },
        ),
        (
            0x8006_6898,
            "srl a0, a0, 8",
            K::Srl {
                rd: Reg::A0,
                rt: Reg::A0,
                sa: 8,
            },
        ),
        (
            0x8006_689C,
            "andi a1, a1, 0xFFFF",
            K::Andi {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0xFFFF,
            },
        ),
        (
            0x8006_68A0,
            "srl a1, a1, 5",
            K::Srl {
                rd: Reg::A1,
                rt: Reg::A1,
                sa: 5,
            },
        ),
        (
            0x8006_68A4,
            "addu a0, a0, a1",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A0,
                rt: Reg::A1,
            },
        ),
        (
            0x8006_68A8,
            "sll a0, a0, 2",
            K::Sll {
                rd: Reg::A0,
                rt: Reg::A0,
                sa: 2,
            },
        ),
        (
            0x8006_68AC,
            "addu a0, a0, v0",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A0,
                rt: Reg::V0,
            },
        ),
        (
            0x8006_68B0,
            "lw v0, 0(a0)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0,
            },
        ),
        (
            0x8006_68B4,
            "sllv v1, v1, a2",
            K::Sllv {
                rd: Reg::V1,
                rt: Reg::V1,
                rs: Reg::A2,
            },
        ),
        (
            0x8006_68B8,
            "and v0, v0, v1",
            K::And {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (0x8006_68BC, "jr ra", K::Jr { rs: Reg::RA }),
        (
            0x8006_68C0,
            "sltu v0, zero, v0",
            K::Sltu {
                rd: Reg::V0,
                rs: Reg::ZERO,
                rt: Reg::V0,
            },
        ),
    ] {
        require(&instructions, address, expected, kind)?;
    }

    Ok(QuestSeqFlagQuerySummary {
        opcode: QUEST_SEQ_FLAG_QUERY_OPCODE,
        handler_runtime: QUEST_SEQ_FLAG_QUERY_HANDLER_RUNTIME,
        query_runtime: QUEST_SEQ_FLAG_QUERY_RUNTIME,
        state_base_runtime: QUEST_SEQ_FLAG_STATE_BASE_RUNTIME,
        record_bytes: QUEST_SEQ_FLAG_QUERY_RECORD_BYTES,
        expected_offset: QUEST_SEQ_FLAG_EXPECTED_OFFSET,
        flag_base_offset: QUEST_SEQ_FLAG_BASE_OFFSET,
        flag_offset_offset: QUEST_SEQ_FLAG_OFFSET_OFFSET,
        branch_pointer_offset: QUEST_SEQ_FLAG_BRANCH_POINTER_OFFSET,
    })
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: &str,
    kind: K,
) -> Result<(), QuestSeqFlagQueryError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| error(format!("QUEST OPA1 조회 명령이 text 밖임: 0x{address:08X}")))?;
    if actual.kind != kind {
        return Err(error(format!(
            "QUEST OPA1 조회 명령 불일치: address=0x{address:08X}, expected={expected}, actual={:?}",
            actual.kind,
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_seq_flag_query_tests.rs"]
mod tests;
