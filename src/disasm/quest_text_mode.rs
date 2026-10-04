//! QUEST SEQ OP21에서 text token 소비자로 이어지는 정적 경로.
//!
//! 공용 interpreter는 객체 active word가 1이면 `0x8003B840`을 직접
//! 호출한다. OP21 handler는 이 word를 1로 만들고 stream을 4바이트
//! 전진시킨다. 여기서는 dispatch word, mode 전환, direct call, token
//! 판별과 glyph 전진을 typed MIPS로 함께 고정한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::{
    QUEST_SEQ_BASE_DISPATCH_HANDLERS, QUEST_SEQ_TEXT_MODE_OPCODE, QUEST_SEQ_TEXT_MODE_RECORD_BYTES,
};

use super::linear;
use super::quest_seq::{
    QUEST_SEQ_BASE_DISPATCH_TABLE_RUNTIME, QUEST_SEQ_INTERPRETER_RUNTIME,
    QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
};

pub const QUEST_SEQ_TEXT_MODE_HANDLER_RUNTIME: u32 = 0x8002_2F0C;
pub const QUEST_SEQ_TEXT_CONSUMER_CALL_RUNTIME: u32 = 0x8002_25A8;
pub const QUEST_SEQ_TEXT_CONSUMER_RUNTIME: u32 = 0x8003_B840;
pub const QUEST_SEQ_TEXT_GLYPH_RENDERER_RUNTIME: u32 = 0x8002_BA40;
pub const QUEST_SEQ_TEXT_GLYPH_RENDERER_RETURN_RUNTIME: u32 = 0x8003_B9F4;
pub const QUEST_SEQ_TEXT_ACTIVE_WORD_OFFSET: i16 = 0;

/// OP21에서 text token 소비까지 확인한 정적 요약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestTextModeSummary {
    pub opcode: u8,
    pub record_bytes: usize,
    pub handler_runtime: u32,
    pub interpreter_runtime: u32,
    pub consumer_runtime: u32,
    pub glyph_renderer_runtime: u32,
    pub active_word_offset: i16,
    pub stream_pointer_offset: i16,
}

/// QUEST.EXE 또는 typed 명령 경로 불일치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestTextModeError {
    WrongExecutable(&'static str),
    TextLengthMismatch {
        expected: usize,
        actual: usize,
    },
    UnalignedText,
    MissingInstruction(u32),
    InstructionMismatch {
        address: u32,
        expected: &'static str,
        actual: K,
    },
    DispatchWordMismatch {
        expected: u32,
        actual: u32,
    },
    DispatchSpecMismatch {
        expected: u32,
        actual: u32,
    },
}

impl fmt::Display for QuestTextModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(f, "QUEST text mode 검증 대상이 QUEST.EXE가 아님: {exe}")
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                f,
                "QUEST text mode EXE 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText => write!(f, "QUEST.EXE text 길이가 4바이트 정렬이 아님"),
            Self::MissingInstruction(address) => {
                write!(f, "QUEST text mode 명령이 text 범위 밖임: 0x{address:08X}")
            }
            Self::InstructionMismatch {
                address,
                expected,
                actual,
            } => write!(
                f,
                "QUEST text mode 명령 불일치: address=0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
            Self::DispatchWordMismatch { expected, actual } => write!(
                f,
                "QUEST OP21 dispatch word 불일치: expected=0x{expected:08X}, \
                 actual=0x{actual:08X}"
            ),
            Self::DispatchSpecMismatch { expected, actual } => write!(
                f,
                "QUEST OP21 Rust dispatch spec 불일치: expected=0x{expected:08X}, \
                 actual=0x{actual:08X}"
            ),
        }
    }
}

impl Error for QuestTextModeError {}

fn require(
    instructions: &[Instruction],
    map: &ExeMap,
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestTextModeError> {
    let index = map
        .text_offset(address)
        .ok_or(QuestTextModeError::MissingInstruction(address))?
        / 4;
    let actual = instructions
        .get(index)
        .ok_or(QuestTextModeError::MissingInstruction(address))?;
    if actual.addr != address || actual.kind != kind {
        return Err(QuestTextModeError::InstructionMismatch {
            address,
            expected,
            actual: actual.kind,
        });
    }
    Ok(())
}

fn verify_instructions(
    map: &ExeMap,
    instructions: &[Instruction],
) -> Result<(), QuestTextModeError> {
    for (address, expected, kind) in [
        (
            0x8002_2F0C,
            "lw v0, 0x1C(a0)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8002_2F10,
            "addiu v1, zero, 1",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            0x8002_2F14,
            "sw v1, 0(a0)",
            K::Sw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: QUEST_SEQ_TEXT_ACTIVE_WORD_OFFSET,
            },
        ),
        (
            0x8002_2F18,
            "addiu v0, v0, 4",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_SEQ_TEXT_MODE_RECORD_BYTES as i16,
            },
        ),
        (0x8002_2F1C, "jr ra", K::Jr { rs: Reg::RA }),
        (
            0x8002_2F20,
            "sw v0, 0x1C(a0)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8002_2580,
            "addiu s2, zero, 1",
            K::Addiu {
                rt: Reg::S2,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            0x8002_2598,
            "lw v0, 0(s0)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_TEXT_ACTIVE_WORD_OFFSET,
            },
        ),
        (
            0x8002_25A0,
            "bne v0, s2, 5",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::S2,
                offset: 5,
            },
        ),
        (
            QUEST_SEQ_TEXT_CONSUMER_CALL_RUNTIME,
            "jal 0x8003B840",
            K::Jal {
                target: QUEST_SEQ_TEXT_CONSUMER_RUNTIME,
            },
        ),
        (
            0x8002_25AC,
            "move a0, s0",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8003_B8FC,
            "lw v0, 0x1C(s0)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8003_B904,
            "lhu v1, 0(v0)",
            K::Lhu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8003_B908,
            "ori v0, zero, 0xFEFF",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0xFEFF,
            },
        ),
        (
            0x8003_B930,
            "jalr v0",
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        (
            0x8003_B9E8,
            "lhu a1, 0(v0)",
            K::Lhu {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8003_B9EC,
            "jal 0x8002BA40",
            K::Jal {
                target: QUEST_SEQ_TEXT_GLYPH_RENDERER_RUNTIME,
            },
        ),
        (
            0x8003_B9FC,
            "addiu v0, v0, 2",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 2,
            },
        ),
        (
            0x8003_BA00,
            "sw v0, 0x1C(s0)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
    ] {
        require(instructions, map, address, expected, kind)?;
    }
    Ok(())
}

/// 지원 원본 `QUEST.EXE`의 OP21 → text consumer 경로를 검증한다.
pub fn verify_box_quest_text_mode(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestTextModeSummary, QuestTextModeError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestTextModeError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestTextModeError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }
    let instructions = linear::sweep(map, text).ok_or(QuestTextModeError::UnalignedText)?;

    let dispatch_spec = QUEST_SEQ_BASE_DISPATCH_HANDLERS[usize::from(QUEST_SEQ_TEXT_MODE_OPCODE)];
    if dispatch_spec != QUEST_SEQ_TEXT_MODE_HANDLER_RUNTIME {
        return Err(QuestTextModeError::DispatchSpecMismatch {
            expected: QUEST_SEQ_TEXT_MODE_HANDLER_RUNTIME,
            actual: dispatch_spec,
        });
    }
    let dispatch_runtime =
        QUEST_SEQ_BASE_DISPATCH_TABLE_RUNTIME + u32::from(QUEST_SEQ_TEXT_MODE_OPCODE) * 4;
    let dispatch_offset = map
        .text_offset(dispatch_runtime)
        .ok_or(QuestTextModeError::MissingInstruction(dispatch_runtime))?;
    let dispatch_raw = text
        .get(dispatch_offset..dispatch_offset + 4)
        .ok_or(QuestTextModeError::MissingInstruction(dispatch_runtime))?;
    let dispatch_word = u32::from_le_bytes(
        dispatch_raw
            .try_into()
            .expect("검증한 dispatch slice는 4바이트"),
    );
    if dispatch_word != QUEST_SEQ_TEXT_MODE_HANDLER_RUNTIME {
        return Err(QuestTextModeError::DispatchWordMismatch {
            expected: QUEST_SEQ_TEXT_MODE_HANDLER_RUNTIME,
            actual: dispatch_word,
        });
    }
    verify_instructions(map, &instructions)?;

    Ok(QuestTextModeSummary {
        opcode: QUEST_SEQ_TEXT_MODE_OPCODE,
        record_bytes: QUEST_SEQ_TEXT_MODE_RECORD_BYTES,
        handler_runtime: QUEST_SEQ_TEXT_MODE_HANDLER_RUNTIME,
        interpreter_runtime: QUEST_SEQ_INTERPRETER_RUNTIME,
        consumer_runtime: QUEST_SEQ_TEXT_CONSUMER_RUNTIME,
        glyph_renderer_runtime: QUEST_SEQ_TEXT_GLYPH_RENDERER_RUNTIME,
        active_word_offset: QUEST_SEQ_TEXT_ACTIVE_WORD_OFFSET,
        stream_pointer_offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
    })
}

#[cfg(test)]
#[path = "quest_text_mode_tests.rs"]
mod tests;
