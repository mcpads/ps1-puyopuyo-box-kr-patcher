//! QUEST 공용 SEQ OP13/14 비교 branch record의 원본 증거.
//!
//! 두 handler는 record `+1`의 bit field로 두 operand 원천과 14종 비교를
//! 고르고, 조건이 참이면 record 뒤 pointer로 분기하며 거짓이면 그
//! pointer 다음 record로 진행한다. OP13은 signed halfword, OP14는 word를
//! 읽는다. 이 모듈은 selector·operand load·비교 table·최종 stream store를
//! typed MIPS와 원본 table로 검증한다.
//!
//! record 해석은 가능한 두 다음 위치를 제공할 뿐이다. 비교에 필요한
//! object·전역·호출 register의 실제 값과 record 정상 실행 도달은 별도다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::quest_seq_opcode_handler;

use super::linear;

pub const QUEST_SEQ_BASE_COMPARE_LEFT_SOURCE_MASK: u8 = 0xC0;
pub const QUEST_SEQ_BASE_COMPARE_RIGHT_SOURCE_MASK: u8 = 0x30;
pub const QUEST_SEQ_BASE_COMPARE_OPERATION_MASK: u8 = 0x0F;
pub const QUEST_SEQ_BASE_COMPARE_OPERATION_COUNT: usize = 14;
pub const QUEST_SEQ_BASE_COMPARE_STATE_BASE_RUNTIME: u32 = 0x800D_B6C0;

pub const QUEST_SEQ_OP13_COMPARE_TABLE_RUNTIME: u32 = 0x8001_0384;
pub const QUEST_SEQ_OP14_COMPARE_TABLE_RUNTIME: u32 = 0x8001_03BC;

pub const QUEST_SEQ_OP13_COMPARE_HANDLERS: [u32; QUEST_SEQ_BASE_COMPARE_OPERATION_COUNT] = [
    0x8002_10FC,
    0x8002_10B8,
    0x8002_10C4,
    0x8002_10CC,
    0x8002_10D4,
    0x8002_10DC,
    0x8002_10E4,
    0x8002_10F0,
    0x8002_10FC,
    0x8002_1108,
    0x8002_1110,
    0x8002_1118,
    0x8002_1120,
    0x8002_1128,
];

pub const QUEST_SEQ_OP14_COMPARE_HANDLERS: [u32; QUEST_SEQ_BASE_COMPARE_OPERATION_COUNT] = [
    0x8002_1318,
    0x8002_12D4,
    0x8002_12E0,
    0x8002_12E8,
    0x8002_12F0,
    0x8002_12F8,
    0x8002_1300,
    0x8002_130C,
    0x8002_1318,
    0x8002_1324,
    0x8002_132C,
    0x8002_1334,
    0x8002_133C,
    0x8002_1344,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqBaseCompareOperation {
    NotEqual,
    Equal,
    UnsignedLess,
    UnsignedGreater,
    UnsignedLessOrEqual,
    UnsignedGreaterOrEqual,
    BitAndNonzero,
    BitOrNonzero,
    XorNonzero,
    LeftNonzero,
    SignedLess,
    SignedGreater,
    SignedLessOrEqual,
    SignedGreaterOrEqual,
}

impl QuestSeqBaseCompareOperation {
    pub const fn from_selector(selector: u8) -> Option<Self> {
        match selector {
            0 => Some(Self::NotEqual),
            1 => Some(Self::Equal),
            2 => Some(Self::UnsignedLess),
            3 => Some(Self::UnsignedGreater),
            4 => Some(Self::UnsignedLessOrEqual),
            5 => Some(Self::UnsignedGreaterOrEqual),
            6 => Some(Self::BitAndNonzero),
            7 => Some(Self::BitOrNonzero),
            8 => Some(Self::XorNonzero),
            9 => Some(Self::LeftNonzero),
            10 => Some(Self::SignedLess),
            11 => Some(Self::SignedGreater),
            12 => Some(Self::SignedLessOrEqual),
            13 => Some(Self::SignedGreaterOrEqual),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::NotEqual => "not-equal",
            Self::Equal => "equal",
            Self::UnsignedLess => "unsigned-less",
            Self::UnsignedGreater => "unsigned-greater",
            Self::UnsignedLessOrEqual => "unsigned-less-or-equal",
            Self::UnsignedGreaterOrEqual => "unsigned-greater-or-equal",
            Self::BitAndNonzero => "bit-and-nonzero",
            Self::BitOrNonzero => "bit-or-nonzero",
            Self::XorNonzero => "xor-nonzero",
            Self::LeftNonzero => "left-nonzero",
            Self::SignedLess => "signed-less",
            Self::SignedGreater => "signed-greater",
            Self::SignedLessOrEqual => "signed-less-or-equal",
            Self::SignedGreaterOrEqual => "signed-greater-or-equal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqBaseCompareOperandSource {
    HandlerArgumentA2,
    Inline,
    GlobalState,
    ObjectField,
    ObjectIndirectField,
}

impl QuestSeqBaseCompareOperandSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::HandlerArgumentA2 => "handler-argument-a2",
            Self::Inline => "inline",
            Self::GlobalState => "global-state",
            Self::ObjectField => "object-field",
            Self::ObjectIndirectField => "object-indirect-field",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqBaseCompareRecordLayout {
    pub opcode: u8,
    pub value_bytes: usize,
    pub operation: QuestSeqBaseCompareOperation,
    pub left_source: QuestSeqBaseCompareOperandSource,
    pub right_source: QuestSeqBaseCompareOperandSource,
    pub left_parameter_offset: Option<usize>,
    pub right_parameter_offset: usize,
    pub branch_pointer_offset: usize,
    pub fallthrough_offset: usize,
}

/// OP13/14 record byte `+1`이 고르는 비교 layout.
pub fn quest_seq_base_compare_record_layout(
    opcode: u8,
    byte1: u8,
) -> Option<QuestSeqBaseCompareRecordLayout> {
    let value_bytes = match opcode {
        0x13 => 2,
        0x14 => 4,
        _ => return None,
    };
    let operation =
        QuestSeqBaseCompareOperation::from_selector(byte1 & QUEST_SEQ_BASE_COMPARE_OPERATION_MASK)?;
    let left_source = match byte1 & QUEST_SEQ_BASE_COMPARE_LEFT_SOURCE_MASK {
        0x00 => QuestSeqBaseCompareOperandSource::HandlerArgumentA2,
        0x40 => QuestSeqBaseCompareOperandSource::GlobalState,
        0x80 => QuestSeqBaseCompareOperandSource::ObjectField,
        0xC0 => QuestSeqBaseCompareOperandSource::ObjectIndirectField,
        _ => unreachable!("0xC0 mask의 가능한 값"),
    };
    let right_source = match byte1 & QUEST_SEQ_BASE_COMPARE_RIGHT_SOURCE_MASK {
        0x00 => QuestSeqBaseCompareOperandSource::Inline,
        0x10 => QuestSeqBaseCompareOperandSource::GlobalState,
        0x20 => QuestSeqBaseCompareOperandSource::ObjectField,
        0x30 => QuestSeqBaseCompareOperandSource::ObjectIndirectField,
        _ => unreachable!("0x30 mask의 가능한 값"),
    };
    let branch_pointer_offset = if right_source == QuestSeqBaseCompareOperandSource::Inline {
        8
    } else {
        4
    };
    Some(QuestSeqBaseCompareRecordLayout {
        opcode,
        value_bytes,
        operation,
        left_source,
        right_source,
        left_parameter_offset: (left_source != QuestSeqBaseCompareOperandSource::HandlerArgumentA2)
            .then_some(2),
        right_parameter_offset: if right_source == QuestSeqBaseCompareOperandSource::Inline {
            4
        } else {
            3
        },
        branch_pointer_offset,
        fallthrough_offset: branch_pointer_offset + 4,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqBaseCompareSummary {
    pub opcodes: usize,
    pub operations: usize,
    pub halfword_opcodes: usize,
    pub word_opcodes: usize,
    pub state_base_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestSeqBaseCompareError {
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
        opcode: u8,
        selector: u8,
        expected: u32,
        actual: u32,
    },
}

impl fmt::Display for QuestSeqBaseCompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(f, "SEQ base 비교 검증 대상이 QUEST.EXE가 아님: {exe}")
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
                "SEQ base OP{opcode:02X} 비교 handler 불일치: \
                 expected=0x{expected:08X}, actual={actual:?}"
            ),
            Self::MissingInstruction(address) => {
                write!(f, "SEQ base 비교 명령/table이 text 밖임: 0x{address:08X}")
            }
            Self::InstructionMismatch {
                opcode,
                address,
                expected,
                actual,
            } => write!(
                f,
                "SEQ base OP{opcode:02X} 비교 명령 불일치: address=0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
            Self::DispatchWordMismatch {
                opcode,
                selector,
                expected,
                actual,
            } => write!(
                f,
                "SEQ base OP{opcode:02X} 비교 selector {selector} table word 불일치: \
                 expected=0x{expected:08X}, actual=0x{actual:08X}"
            ),
        }
    }
}

impl Error for QuestSeqBaseCompareError {}

#[derive(Debug, Clone, Copy)]
struct Evidence {
    opcode: u8,
    address: u32,
    expected: &'static str,
    kind: K,
}

const fn evidence(opcode: u8, address: u32, expected: &'static str, kind: K) -> Evidence {
    Evidence {
        opcode,
        address,
        expected,
        kind,
    }
}

const EVIDENCE: &[Evidence] = &[
    // OP13 selector와 operand 원천.
    evidence(
        0x13,
        0x8002_0F50,
        "lbu t1, 1(stream)",
        K::Lbu {
            rt: Reg::T1,
            base: Reg::A0,
            offset: 1,
        },
    ),
    evidence(
        0x13,
        0x8002_0F58,
        "andi v1, t1, 0xC0",
        K::Andi {
            rt: Reg::V1,
            rs: Reg::T1,
            imm: 0xC0,
        },
    ),
    evidence(
        0x13,
        0x8002_0F98,
        "lbu v0, 2(stream)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 2,
        },
    ),
    evidence(
        0x13,
        0x8002_0F9C,
        "addiu v1, v1, state base",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V1,
            imm: -18752,
        },
    ),
    evidence(
        0x13,
        0x8002_0FA4,
        "sll v0, v0, 2",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    ),
    evidence(
        0x13,
        0x8002_0FB8,
        "lw v0, 0x20(object)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::T0,
            offset: 0x20,
        },
    ),
    evidence(
        0x13,
        0x8002_0FC4,
        "lh a2, 0(selected)",
        K::Lh {
            rt: Reg::A2,
            base: Reg::V0,
            offset: 0,
        },
    ),
    evidence(
        0x13,
        0x8002_0FC8,
        "andi v1, t1, 0x30",
        K::Andi {
            rt: Reg::V1,
            rs: Reg::T1,
            imm: 0x30,
        },
    ),
    evidence(
        0x13,
        0x8002_1014,
        "lh a1, 4(stream)",
        K::Lh {
            rt: Reg::A1,
            base: Reg::V0,
            offset: 4,
        },
    ),
    evidence(
        0x13,
        0x8002_101C,
        "addiu stream, stream, 8",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 8,
        },
    ),
    evidence(
        0x13,
        0x8002_1038,
        "lh a1, 0(global state)",
        K::Lh {
            rt: Reg::A1,
            base: Reg::V0,
            offset: 0,
        },
    ),
    evidence(
        0x13,
        0x8002_108C,
        "andi v1, t1, 0x0F",
        K::Andi {
            rt: Reg::V1,
            rs: Reg::T1,
            imm: 0x0F,
        },
    ),
    evidence(
        0x13,
        0x8002_1090,
        "sltiu v0, v1, 14",
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: 14,
        },
    ),
    evidence(
        0x13,
        0x8002_109C,
        "addiu v0, v0, OP13 compare table",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0x384,
        },
    ),
    evidence(
        0x13,
        0x8002_1130,
        "beq compare result, zero, fallthrough",
        K::Beq {
            rs: Reg::A3,
            rt: Reg::ZERO,
            offset: 6,
        },
    ),
    evidence(
        0x13,
        0x8002_1140,
        "lw branch target, 0(stream)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 0,
        },
    ),
    evidence(
        0x13,
        0x8002_1148,
        "sw branch target, 0x1C(object)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::T0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x13,
        0x8002_1154,
        "addiu fallthrough, stream, 4",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 4,
        },
    ),
    evidence(
        0x13,
        0x8002_115C,
        "sw fallthrough, 0x1C(object)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::T0,
            offset: 0x1C,
        },
    ),
    // OP14는 같은 selector를 word 폭으로 소비한다.
    evidence(
        0x14,
        0x8002_116C,
        "lbu t1, 1(stream)",
        K::Lbu {
            rt: Reg::T1,
            base: Reg::A0,
            offset: 1,
        },
    ),
    evidence(
        0x14,
        0x8002_1174,
        "andi v1, t1, 0xC0",
        K::Andi {
            rt: Reg::V1,
            rs: Reg::T1,
            imm: 0xC0,
        },
    ),
    evidence(
        0x14,
        0x8002_11B4,
        "lbu v0, 2(stream)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 2,
        },
    ),
    evidence(
        0x14,
        0x8002_11B8,
        "addiu v1, v1, state base",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V1,
            imm: -18752,
        },
    ),
    evidence(
        0x14,
        0x8002_11C0,
        "sll v0, v0, 2",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    ),
    evidence(
        0x14,
        0x8002_11D4,
        "lw v0, 0x20(object)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::T0,
            offset: 0x20,
        },
    ),
    evidence(
        0x14,
        0x8002_11E0,
        "lw a2, 0(selected)",
        K::Lw {
            rt: Reg::A2,
            base: Reg::V0,
            offset: 0,
        },
    ),
    evidence(
        0x14,
        0x8002_11E4,
        "andi v1, t1, 0x30",
        K::Andi {
            rt: Reg::V1,
            rs: Reg::T1,
            imm: 0x30,
        },
    ),
    evidence(
        0x14,
        0x8002_1230,
        "lw a1, 4(stream)",
        K::Lw {
            rt: Reg::A1,
            base: Reg::V0,
            offset: 4,
        },
    ),
    evidence(
        0x14,
        0x8002_1238,
        "addiu stream, stream, 8",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 8,
        },
    ),
    evidence(
        0x14,
        0x8002_1254,
        "lw a1, 0(global state)",
        K::Lw {
            rt: Reg::A1,
            base: Reg::V0,
            offset: 0,
        },
    ),
    evidence(
        0x14,
        0x8002_12A8,
        "andi v1, t1, 0x0F",
        K::Andi {
            rt: Reg::V1,
            rs: Reg::T1,
            imm: 0x0F,
        },
    ),
    evidence(
        0x14,
        0x8002_12AC,
        "sltiu v0, v1, 14",
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: 14,
        },
    ),
    evidence(
        0x14,
        0x8002_12B8,
        "addiu v0, v0, OP14 compare table",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0x3BC,
        },
    ),
    evidence(
        0x14,
        0x8002_134C,
        "beq compare result, zero, fallthrough",
        K::Beq {
            rs: Reg::A3,
            rt: Reg::ZERO,
            offset: 6,
        },
    ),
    evidence(
        0x14,
        0x8002_135C,
        "lw branch target, 0(stream)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 0,
        },
    ),
    evidence(
        0x14,
        0x8002_1364,
        "sw branch target, 0x1C(object)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::T0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x14,
        0x8002_1370,
        "addiu fallthrough, stream, 4",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 4,
        },
    ),
    evidence(
        0x14,
        0x8002_1378,
        "sw fallthrough, 0x1C(object)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::T0,
            offset: 0x1C,
        },
    ),
    // operand source bit의 실제 분기 routing.
    evidence(
        0x13,
        0x8002_0F5C,
        "beq left mode 0x80, object field",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 18,
        },
    ),
    evidence(
        0x13,
        0x8002_0F70,
        "beq left mode 0x40, global state",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 8,
        },
    ),
    evidence(
        0x13,
        0x8002_0F84,
        "beq left mode 0xC0, indirect object field",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 11,
        },
    ),
    evidence(
        0x13,
        0x8002_0FD0,
        "beq right mode 0x10, global state",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 19,
        },
    ),
    evidence(
        0x13,
        0x8002_0FE0,
        "beq right mode 0x00, inline",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::ZERO,
            offset: 10,
        },
    ),
    evidence(
        0x13,
        0x8002_0FF4,
        "beq right mode 0x20, object field",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 20,
        },
    ),
    evidence(
        0x13,
        0x8002_0FFC,
        "beq right mode 0x30, indirect object field",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 27,
        },
    ),
    evidence(
        0x14,
        0x8002_1178,
        "beq left mode 0x80, object field",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 18,
        },
    ),
    evidence(
        0x14,
        0x8002_118C,
        "beq left mode 0x40, global state",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 8,
        },
    ),
    evidence(
        0x14,
        0x8002_11A0,
        "beq left mode 0xC0, indirect object field",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 11,
        },
    ),
    evidence(
        0x14,
        0x8002_11EC,
        "beq right mode 0x10, global state",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 19,
        },
    ),
    evidence(
        0x14,
        0x8002_11FC,
        "beq right mode 0x00, inline",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::ZERO,
            offset: 10,
        },
    ),
    evidence(
        0x14,
        0x8002_1210,
        "beq right mode 0x20, object field",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 20,
        },
    ),
    evidence(
        0x14,
        0x8002_1218,
        "beq right mode 0x30, indirect object field",
        K::Beq {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 27,
        },
    ),
    // 14종 비교 selector의 의미. jump 자체보다 결과 register를 만드는
    // 명령과 공용 negate/nonzero delay slot을 고정한다.
    evidence(
        0x13,
        0x8002_10B8,
        "equal xor",
        K::Xor {
            rd: Reg::V0,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x13,
        0x8002_10C0,
        "equal result",
        K::Sltiu {
            rt: Reg::A3,
            rs: Reg::V0,
            imm: 1,
        },
    ),
    evidence(
        0x13,
        0x8002_10C8,
        "unsigned less result",
        K::Sltu {
            rd: Reg::A3,
            rs: Reg::A1,
            rt: Reg::A2,
        },
    ),
    evidence(
        0x13,
        0x8002_10D0,
        "unsigned greater result",
        K::Sltu {
            rd: Reg::A3,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x13,
        0x8002_10D8,
        "unsigned less-or-equal base",
        K::Sltu {
            rd: Reg::V0,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x13,
        0x8002_10E0,
        "unsigned greater-or-equal base",
        K::Sltu {
            rd: Reg::V0,
            rs: Reg::A1,
            rt: Reg::A2,
        },
    ),
    evidence(
        0x13,
        0x8002_10E4,
        "bit-and",
        K::And {
            rd: Reg::V0,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x13,
        0x8002_10EC,
        "bit-and nonzero result",
        K::Sltu {
            rd: Reg::A3,
            rs: Reg::ZERO,
            rt: Reg::V0,
        },
    ),
    evidence(
        0x13,
        0x8002_10F0,
        "bit-or",
        K::Or {
            rd: Reg::V0,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x13,
        0x8002_10F8,
        "bit-or nonzero result",
        K::Sltu {
            rd: Reg::A3,
            rs: Reg::ZERO,
            rt: Reg::V0,
        },
    ),
    evidence(
        0x13,
        0x8002_10FC,
        "not-equal xor",
        K::Xor {
            rd: Reg::V0,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x13,
        0x8002_1104,
        "not-equal result",
        K::Sltu {
            rd: Reg::A3,
            rs: Reg::ZERO,
            rt: Reg::V0,
        },
    ),
    evidence(
        0x13,
        0x8002_110C,
        "left nonzero result",
        K::Sltu {
            rd: Reg::A3,
            rs: Reg::ZERO,
            rt: Reg::A2,
        },
    ),
    evidence(
        0x13,
        0x8002_1114,
        "signed less result",
        K::Slt {
            rd: Reg::A3,
            rs: Reg::A1,
            rt: Reg::A2,
        },
    ),
    evidence(
        0x13,
        0x8002_111C,
        "signed greater result",
        K::Slt {
            rd: Reg::A3,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x13,
        0x8002_1124,
        "signed less-or-equal base",
        K::Slt {
            rd: Reg::V0,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x13,
        0x8002_1128,
        "signed greater-or-equal base",
        K::Slt {
            rd: Reg::V0,
            rs: Reg::A1,
            rt: Reg::A2,
        },
    ),
    evidence(
        0x13,
        0x8002_112C,
        "less-or-equal negate",
        K::Xori {
            rt: Reg::A3,
            rs: Reg::V0,
            imm: 1,
        },
    ),
    evidence(
        0x14,
        0x8002_12D4,
        "equal xor",
        K::Xor {
            rd: Reg::V0,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x14,
        0x8002_12DC,
        "equal result",
        K::Sltiu {
            rt: Reg::A3,
            rs: Reg::V0,
            imm: 1,
        },
    ),
    evidence(
        0x14,
        0x8002_12E4,
        "unsigned less result",
        K::Sltu {
            rd: Reg::A3,
            rs: Reg::A1,
            rt: Reg::A2,
        },
    ),
    evidence(
        0x14,
        0x8002_12EC,
        "unsigned greater result",
        K::Sltu {
            rd: Reg::A3,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x14,
        0x8002_12F4,
        "unsigned less-or-equal base",
        K::Sltu {
            rd: Reg::V0,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x14,
        0x8002_12FC,
        "unsigned greater-or-equal base",
        K::Sltu {
            rd: Reg::V0,
            rs: Reg::A1,
            rt: Reg::A2,
        },
    ),
    evidence(
        0x14,
        0x8002_1300,
        "bit-and",
        K::And {
            rd: Reg::V0,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x14,
        0x8002_1308,
        "bit-and nonzero result",
        K::Sltu {
            rd: Reg::A3,
            rs: Reg::ZERO,
            rt: Reg::V0,
        },
    ),
    evidence(
        0x14,
        0x8002_130C,
        "bit-or",
        K::Or {
            rd: Reg::V0,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x14,
        0x8002_1314,
        "bit-or nonzero result",
        K::Sltu {
            rd: Reg::A3,
            rs: Reg::ZERO,
            rt: Reg::V0,
        },
    ),
    evidence(
        0x14,
        0x8002_1318,
        "not-equal xor",
        K::Xor {
            rd: Reg::V0,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x14,
        0x8002_1320,
        "not-equal result",
        K::Sltu {
            rd: Reg::A3,
            rs: Reg::ZERO,
            rt: Reg::V0,
        },
    ),
    evidence(
        0x14,
        0x8002_1328,
        "left nonzero result",
        K::Sltu {
            rd: Reg::A3,
            rs: Reg::ZERO,
            rt: Reg::A2,
        },
    ),
    evidence(
        0x14,
        0x8002_1330,
        "signed less result",
        K::Slt {
            rd: Reg::A3,
            rs: Reg::A1,
            rt: Reg::A2,
        },
    ),
    evidence(
        0x14,
        0x8002_1338,
        "signed greater result",
        K::Slt {
            rd: Reg::A3,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x14,
        0x8002_1340,
        "signed less-or-equal base",
        K::Slt {
            rd: Reg::V0,
            rs: Reg::A2,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x14,
        0x8002_1344,
        "signed greater-or-equal base",
        K::Slt {
            rd: Reg::V0,
            rs: Reg::A1,
            rt: Reg::A2,
        },
    ),
    evidence(
        0x14,
        0x8002_1348,
        "less-or-equal negate",
        K::Xori {
            rt: Reg::A3,
            rs: Reg::V0,
            imm: 1,
        },
    ),
];

/// 지원 `QUEST.EXE`에서 OP13/14 비교 branch record를 검증한다.
pub fn verify_box_quest_seq_base_compares(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqBaseCompareSummary, QuestSeqBaseCompareError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestSeqBaseCompareError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestSeqBaseCompareError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }
    let instructions = linear::sweep(map, text).ok_or(QuestSeqBaseCompareError::UnalignedText)?;

    for (opcode, expected) in [(0x13, 0x8002_0F44), (0x14, 0x8002_1160)] {
        let actual = quest_seq_opcode_handler(opcode);
        if actual != Some(expected) {
            return Err(QuestSeqBaseCompareError::HandlerMismatch {
                opcode,
                expected,
                actual,
            });
        }
    }
    for evidence in EVIDENCE {
        require(&instructions, *evidence)?;
    }
    verify_compare_words(
        map,
        text,
        0x13,
        QUEST_SEQ_OP13_COMPARE_TABLE_RUNTIME,
        &QUEST_SEQ_OP13_COMPARE_HANDLERS,
    )?;
    verify_compare_words(
        map,
        text,
        0x14,
        QUEST_SEQ_OP14_COMPARE_TABLE_RUNTIME,
        &QUEST_SEQ_OP14_COMPARE_HANDLERS,
    )?;

    Ok(QuestSeqBaseCompareSummary {
        opcodes: 2,
        operations: QUEST_SEQ_BASE_COMPARE_OPERATION_COUNT,
        halfword_opcodes: 1,
        word_opcodes: 1,
        state_base_runtime: QUEST_SEQ_BASE_COMPARE_STATE_BASE_RUNTIME,
    })
}

fn verify_compare_words(
    map: &ExeMap,
    text: &[u8],
    opcode: u8,
    table_runtime: u32,
    expected_handlers: &[u32; QUEST_SEQ_BASE_COMPARE_OPERATION_COUNT],
) -> Result<(), QuestSeqBaseCompareError> {
    for (index, expected) in expected_handlers.iter().copied().enumerate() {
        let address = table_runtime + u32::try_from(index).expect("14") * 4;
        let offset = map
            .text_offset(address)
            .ok_or(QuestSeqBaseCompareError::MissingInstruction(address))?;
        let actual = u32::from_le_bytes(
            text.get(offset..offset + 4)
                .ok_or(QuestSeqBaseCompareError::MissingInstruction(address))?
                .try_into()
                .expect("4바이트 범위를 확인함"),
        );
        if actual != expected {
            return Err(QuestSeqBaseCompareError::DispatchWordMismatch {
                opcode,
                selector: u8::try_from(index).expect("14"),
                expected,
                actual,
            });
        }
    }
    Ok(())
}

fn require(
    instructions: &[Instruction],
    evidence: Evidence,
) -> Result<(), QuestSeqBaseCompareError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == evidence.address)
        .ok_or(QuestSeqBaseCompareError::MissingInstruction(
            evidence.address,
        ))?;
    if actual.kind != evidence.kind {
        return Err(QuestSeqBaseCompareError::InstructionMismatch {
            opcode: evidence.opcode,
            address: evidence.address,
            expected: evidence.expected,
            actual: actual.kind,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_seq_base_compare_tests.rs"]
mod tests;
