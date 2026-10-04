//! `QUEST.EXE`의 SEQ `OP82` child stream 설치 경로.
//!
//! handler는 record `+1`의 subtype을 0..26으로 제한해 객체 초기화 arm을
//! 고른 뒤, 공통 tail에서 record `+8` 포인터를 child 객체의 현재 stream과
//! 원본 stream field에 함께 저장한다. 이 모듈은 특히 `SUKE.SEQ`가 쓰는
//! subtype 2의 jump-table entry와 객체 할당 arm도 함께 검증한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::quest_seq_opcode_handler;

use super::linear;
use super::quest_control::QUEST_ALLOCATE_OBJECT_RUNTIME;

pub const QUEST_SEQ_CHILD_STREAM_OPCODE: u8 = 0x82;
pub const QUEST_SEQ_CHILD_STREAM_HANDLER_RUNTIME: u32 = 0x8007_2844;
pub const QUEST_SEQ_CHILD_STREAM_SUBTYPE_LIMIT: u8 = 27;
pub const QUEST_SEQ_CHILD_STREAM_RECORD_BYTES: usize = 12;
pub const QUEST_SEQ_CHILD_STREAM_POINTER_OFFSET: usize = 8;
pub const QUEST_SEQ_CHILD_STREAM_SUBTYPE2_TABLE_RUNTIME: u32 = 0x8001_2FD0;
pub const QUEST_SEQ_CHILD_STREAM_SUBTYPE2_ARM_RUNTIME: u32 = 0x8007_29E0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqChildStreamSummary {
    pub opcode: u8,
    pub handler_runtime: u32,
    pub subtype_limit: u8,
    pub record_bytes: usize,
    pub pointer_offset: usize,
    pub subtype2_arm_runtime: u32,
    pub allocate_object_runtime: u32,
    pub child_stream_store_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqChildStreamError(String);

impl fmt::Display for QuestSeqChildStreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestSeqChildStreamError {}

fn error(message: impl Into<String>) -> QuestSeqChildStreamError {
    QuestSeqChildStreamError(message.into())
}

fn require(
    instructions: &[Instruction],
    map: &ExeMap,
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestSeqChildStreamError> {
    let index = map
        .text_offset(address)
        .and_then(|offset| offset.checked_div(4))
        .ok_or_else(|| error(format!("QUEST OP82 명령 주소 없음: 0x{address:08X}")))?;
    let instruction = instructions
        .get(index)
        .ok_or_else(|| error(format!("QUEST OP82 명령이 text 밖임: 0x{address:08X}")))?;
    if instruction.addr != address || instruction.kind != kind {
        return Err(error(format!(
            "QUEST OP82 명령 불일치: address=0x{address:08X}, \
             expected={expected}, actual={:?}",
            instruction.kind
        )));
    }
    Ok(())
}

fn require_word(
    text: &[u8],
    map: &ExeMap,
    address: u32,
    expected: u32,
) -> Result<(), QuestSeqChildStreamError> {
    let offset = map
        .text_offset(address)
        .ok_or_else(|| error(format!("QUEST OP82 table 주소 없음: 0x{address:08X}")))?;
    let actual = u32::from_le_bytes(
        text.get(offset..offset + 4)
            .ok_or_else(|| error(format!("QUEST OP82 table word가 잘림: 0x{address:08X}")))?
            .try_into()
            .expect("4바이트 범위를 확인함"),
    );
    if actual != expected {
        return Err(error(format!(
            "QUEST OP82 table word 불일치: address=0x{address:08X}, \
             expected=0x{expected:08X}, actual=0x{actual:08X}"
        )));
    }
    Ok(())
}

/// 지원 `QUEST.EXE`의 `OP82 +8` child stream 설치를 검증한다.
pub fn verify_box_quest_seq_child_stream(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqChildStreamSummary, QuestSeqChildStreamError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST OP82 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len,
        )));
    }
    if quest_seq_opcode_handler(QUEST_SEQ_CHILD_STREAM_OPCODE)
        != Some(QUEST_SEQ_CHILD_STREAM_HANDLER_RUNTIME)
    {
        return Err(error("QUEST OP82 dispatch handler 불일치"));
    }
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error("QUEST.EXE text 길이가 4바이트 정렬이 아님"))?;

    for (address, expected, kind) in [
        (
            0x8007_2870,
            "lw v0, 0x1C(s3)",
            K::Lw {
                rt: Reg::V0,
                base: Reg(19),
                offset: 0x1C,
            },
        ),
        (
            0x8007_2878,
            "lbu v1, 1(stream)",
            K::Lbu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            0x8007_2880,
            "sltiu v0, v1, 27",
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: i16::from(QUEST_SEQ_CHILD_STREAM_SUBTYPE_LIMIT),
            },
        ),
        (
            QUEST_SEQ_CHILD_STREAM_SUBTYPE2_ARM_RUNTIME,
            "lui a0, 0x8006",
            K::Lui {
                rt: Reg::A0,
                imm: 0x8006,
            },
        ),
        (
            0x8007_29E4,
            "j subtype 2 allocator",
            K::J {
                target: 0x8007_3294,
            },
        ),
        (
            0x8007_3294,
            "jal allocate object",
            K::Jal {
                target: QUEST_ALLOCATE_OBJECT_RUNTIME,
            },
        ),
        (
            0x8007_329C,
            "move s1, v0",
            K::Addu {
                rd: Reg(17),
                rs: Reg::V0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8007_3350,
            "lw v0, 0x1C(s3)",
            K::Lw {
                rt: Reg::V0,
                base: Reg(19),
                offset: 0x1C,
            },
        ),
        (
            0x8007_3358,
            "lw v0, 8(stream)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: QUEST_SEQ_CHILD_STREAM_POINTER_OFFSET as i16,
            },
        ),
        (
            0x8007_3360,
            "sw v0, 0x1C(child)",
            K::Sw {
                rt: Reg::V0,
                base: Reg(17),
                offset: 0x1C,
            },
        ),
        (
            0x8007_3364,
            "sw v0, 0xD4(child)",
            K::Sw {
                rt: Reg::V0,
                base: Reg(17),
                offset: 0xD4,
            },
        ),
        (
            0x8007_3370,
            "addiu v0, v0, 12",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_SEQ_CHILD_STREAM_RECORD_BYTES as i16,
            },
        ),
        (
            0x8007_3374,
            "sw v0, 0x1C(parent)",
            K::Sw {
                rt: Reg::V0,
                base: Reg(19),
                offset: 0x1C,
            },
        ),
    ] {
        require(&instructions, map, address, expected, kind)?;
    }
    require_word(
        text,
        map,
        QUEST_SEQ_CHILD_STREAM_SUBTYPE2_TABLE_RUNTIME,
        QUEST_SEQ_CHILD_STREAM_SUBTYPE2_ARM_RUNTIME,
    )?;

    Ok(QuestSeqChildStreamSummary {
        opcode: QUEST_SEQ_CHILD_STREAM_OPCODE,
        handler_runtime: QUEST_SEQ_CHILD_STREAM_HANDLER_RUNTIME,
        subtype_limit: QUEST_SEQ_CHILD_STREAM_SUBTYPE_LIMIT,
        record_bytes: QUEST_SEQ_CHILD_STREAM_RECORD_BYTES,
        pointer_offset: QUEST_SEQ_CHILD_STREAM_POINTER_OFFSET,
        subtype2_arm_runtime: QUEST_SEQ_CHILD_STREAM_SUBTYPE2_ARM_RUNTIME,
        allocate_object_runtime: QUEST_ALLOCATE_OBJECT_RUNTIME,
        child_stream_store_runtime: 0x8007_3360,
    })
}

#[cfg(test)]
#[path = "quest_seq_child_stream_tests.rs"]
mod tests;
