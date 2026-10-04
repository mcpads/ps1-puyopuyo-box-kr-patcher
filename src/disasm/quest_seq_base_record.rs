//! QUEST 공용 SEQ handler의 원본 record 선택형 전진 증거.
//!
//! OP10/11은 record `+1`의 `0x30` mask만으로 4/8바이트 전진을
//! 선택한다. OP3A는 record `+1` 하위 dispatch 중 현재 text path에
//! 나타난 sub02/sub08/sub0A/sub16이 공용 4바이트 전진으로 합류한다. 공용
//! fallback handler도 모든 등록 entry에서 4바이트를 전진한다.
//! 이 사실은 handler 실행 도달이나 record의 section 소유권을 뜻하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::{quest_seq_opcode_handler, QUEST_SEQ_BASE_FALLBACK_HANDLER_RUNTIME};

use super::linear;

pub const QUEST_SEQ_BASE_RECORD_MODE_MASK: u8 = 0x30;
pub const QUEST_SEQ_OP3A_HANDLER_RUNTIME: u32 = 0x8002_1F70;
pub const QUEST_SEQ_OP3A_SUBDISPATCH_TABLE_RUNTIME: u32 = 0x8001_04B4;
pub const QUEST_SEQ_OP3A_STATIC_ADVANCE_SUBOPCODES: [u8; 4] = [0x02, 0x08, 0x0A, 0x16];
pub const QUEST_SEQ_OP3A_SUBDISPATCH_HANDLERS: [u32; 25] = [
    0x8002_1FCC,
    0x8002_1FF4,
    0x8002_1FCC,
    0x8002_2068,
    0x8002_209C,
    0x8002_20B8,
    0x8002_20D4,
    0x8002_20F0,
    0x8002_2100,
    0x8002_2110,
    0x8002_2138,
    0x8002_214C,
    0x8002_2170,
    0x8002_21B0,
    0x8002_21F8,
    0x8002_2224,
    0x8002_2250,
    0x8002_2280,
    0x8002_22B0,
    0x8002_2404,
    0x8002_2428,
    0x8002_2440,
    0x8002_245C,
    0x8002_246C,
    0x8002_2474,
];

/// 원본 record byte만으로 선택할 수 있는 직접 전진 폭.
pub fn quest_seq_base_record_selected_advance(opcode: u8, byte1: u8) -> Option<usize> {
    match opcode {
        0x10 | 0x11 => Some(if byte1 & QUEST_SEQ_BASE_RECORD_MODE_MASK == 0 {
            8
        } else {
            4
        }),
        0x3A if QUEST_SEQ_OP3A_STATIC_ADVANCE_SUBOPCODES.contains(&byte1) => Some(4),
        _ => None,
    }
}

/// 공용 fallback handler가 소유한 base opcode의 고정 전진 폭.
pub fn quest_seq_base_fallback_advance(opcode: u8) -> Option<usize> {
    (quest_seq_opcode_handler(opcode) == Some(QUEST_SEQ_BASE_FALLBACK_HANDLER_RUNTIME)).then_some(4)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqBaseRecordSummary {
    pub mode_selected_opcodes: usize,
    pub mode_mask: u8,
    pub op3a_dispatch_entries: usize,
    pub op3a_static_advance_subopcodes: usize,
    pub fallback_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestSeqBaseRecordError {
    WrongExecutable(&'static str),
    TextLengthMismatch {
        expected: usize,
        actual: usize,
    },
    UnalignedText,
    HandlerMismatch {
        opcode: u8,
        expected: u32,
        actual: Option<u32>,
    },
    MissingInstruction(u32),
    InstructionMismatch {
        opcode: u8,
        address: u32,
        expected: &'static str,
        actual: K,
    },
    DispatchWordMismatch {
        subopcode: u8,
        expected: u32,
        actual: u32,
    },
}

impl fmt::Display for QuestSeqBaseRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(
                    f,
                    "SEQ base record 선택 검증 대상이 QUEST.EXE가 아님: {exe}"
                )
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                f,
                "QUEST.EXE text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText => write!(f, "QUEST.EXE text 길이가 4바이트 정렬이 아님"),
            Self::HandlerMismatch {
                opcode,
                expected,
                actual,
            } => write!(
                f,
                "SEQ base OP{opcode:02X} handler 불일치: \
                 expected=0x{expected:08X}, actual={actual:?}"
            ),
            Self::MissingInstruction(address) => {
                write!(f, "SEQ base record 선택 명령이 text 밖임: 0x{address:08X}")
            }
            Self::InstructionMismatch {
                opcode,
                address,
                expected,
                actual,
            } => write!(
                f,
                "SEQ base OP{opcode:02X} record 선택 명령 불일치: \
                 address=0x{address:08X}, expected={expected}, actual={actual:?}"
            ),
            Self::DispatchWordMismatch {
                subopcode,
                expected,
                actual,
            } => write!(
                f,
                "SEQ base OP3A sub{subopcode:02X} dispatch word 불일치: \
                 expected=0x{expected:08X}, actual=0x{actual:08X}"
            ),
        }
    }
}

impl Error for QuestSeqBaseRecordError {}

/// 지원 `QUEST.EXE`에서 record 선택과 OP3A 하위 합류를 검증한다.
pub fn verify_box_quest_seq_base_record_advances(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqBaseRecordSummary, QuestSeqBaseRecordError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestSeqBaseRecordError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestSeqBaseRecordError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }
    let instructions = linear::sweep(map, text).ok_or(QuestSeqBaseRecordError::UnalignedText)?;

    for (opcode, expected) in [
        (0x10, 0x8002_0730),
        (0x11, 0x8002_0A48),
        (0x3A, QUEST_SEQ_OP3A_HANDLER_RUNTIME),
    ] {
        let actual = quest_seq_opcode_handler(opcode);
        if actual != Some(expected) {
            return Err(QuestSeqBaseRecordError::HandlerMismatch {
                opcode,
                expected,
                actual,
            });
        }
    }

    for (opcode, selector, mask, branch) in [
        (0x10, 0x8002_0754, 0x8002_0A04, 0x8002_0A08),
        (0x11, 0x8002_0A6C, 0x8002_0CFC, 0x8002_0D00),
    ] {
        require(
            &instructions,
            opcode,
            selector,
            "lbu s3, 1(stream)",
            K::Lbu {
                rt: Reg::S3,
                base: Reg::A0,
                offset: 1,
            },
        )?;
        require(
            &instructions,
            opcode,
            mask,
            "andi v0, s3, 0x30",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::S3,
                imm: u16::from(QUEST_SEQ_BASE_RECORD_MODE_MASK),
            },
        )?;
        require(
            &instructions,
            opcode,
            branch,
            "beq v0, zero, 8-byte path",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 4,
            },
        )?;
    }

    verify_op3a_dispatcher(&instructions)?;
    verify_op3a_dispatch_words(map, text)?;
    for (address, expected, kind) in [
        (
            0x8002_1FEC,
            "sub02 j common 4-byte advance",
            K::J {
                target: 0x8002_2474,
            },
        ),
        (
            0x8002_2108,
            "sub08 j common 4-byte advance",
            K::J {
                target: 0x8002_2474,
            },
        ),
        (
            0x8002_2144,
            "sub0A j shared call",
            K::J {
                target: 0x8002_2120,
            },
        ),
        (
            0x8002_2464,
            "sub16 j common 4-byte advance",
            K::J {
                target: 0x8002_2474,
            },
        ),
        (
            0x8002_2130,
            "sub0A shared call j common 4-byte advance",
            K::J {
                target: 0x8002_2474,
            },
        ),
        (
            0x8002_2474,
            "lw v0, 0x1C(s0)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x1C,
            },
        ),
        (
            0x8002_247C,
            "addiu v0, v0, 4",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 4,
            },
        ),
        (
            0x8002_2480,
            "sw v0, 0x1C(s0)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x1C,
            },
        ),
    ] {
        require(&instructions, 0x3A, address, expected, kind)?;
    }

    let fallback_entries = (0u8..0x80)
        .filter(|&opcode| quest_seq_base_fallback_advance(opcode).is_some())
        .count();
    Ok(QuestSeqBaseRecordSummary {
        mode_selected_opcodes: 2,
        mode_mask: QUEST_SEQ_BASE_RECORD_MODE_MASK,
        op3a_dispatch_entries: QUEST_SEQ_OP3A_SUBDISPATCH_HANDLERS.len(),
        op3a_static_advance_subopcodes: QUEST_SEQ_OP3A_STATIC_ADVANCE_SUBOPCODES.len(),
        fallback_entries,
    })
}

fn verify_op3a_dispatcher(instructions: &[Instruction]) -> Result<(), QuestSeqBaseRecordError> {
    for (address, expected, kind) in [
        (
            0x8002_1F9C,
            "lbu v1, 1(stream)",
            K::Lbu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            0x8002_1FA4,
            "sltiu v0, v1, 25",
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 25,
            },
        ),
        (
            0x8002_1FA8,
            "beq v0, zero, fallback",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 0x136,
            },
        ),
        (
            0x8002_1FAC,
            "lui v0, 0x8001",
            K::Lui {
                rt: Reg::V0,
                imm: 0x8001,
            },
        ),
        (
            0x8002_1FB0,
            "addiu v0, v0, 0x4B4",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x4B4,
            },
        ),
        (
            0x8002_1FB4,
            "sll v1, v1, 2",
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
        ),
        (
            0x8002_1FB8,
            "addu v1, v1, v0",
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8002_1FBC,
            "lw v0, 0(v1)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (0x8002_1FC4, "jr v0", K::Jr { rs: Reg::V0 }),
    ] {
        require(instructions, 0x3A, address, expected, kind)?;
    }
    Ok(())
}

fn verify_op3a_dispatch_words(map: &ExeMap, text: &[u8]) -> Result<(), QuestSeqBaseRecordError> {
    for (index, expected) in QUEST_SEQ_OP3A_SUBDISPATCH_HANDLERS
        .iter()
        .copied()
        .enumerate()
    {
        let address =
            QUEST_SEQ_OP3A_SUBDISPATCH_TABLE_RUNTIME + u32::try_from(index).expect("25") * 4;
        let offset = map
            .text_offset(address)
            .ok_or(QuestSeqBaseRecordError::MissingInstruction(address))?;
        let actual = u32::from_le_bytes(
            text.get(offset..offset + 4)
                .ok_or(QuestSeqBaseRecordError::MissingInstruction(address))?
                .try_into()
                .expect("4바이트 범위를 확인함"),
        );
        if actual != expected {
            return Err(QuestSeqBaseRecordError::DispatchWordMismatch {
                subopcode: u8::try_from(index).expect("25"),
                expected,
                actual,
            });
        }
    }
    Ok(())
}

fn require(
    instructions: &[Instruction],
    opcode: u8,
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestSeqBaseRecordError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or(QuestSeqBaseRecordError::MissingInstruction(address))?;
    if actual.kind != kind {
        return Err(QuestSeqBaseRecordError::InstructionMismatch {
            opcode,
            address,
            expected,
            actual: actual.kind,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_seq_base_record_tests.rs"]
mod tests;
