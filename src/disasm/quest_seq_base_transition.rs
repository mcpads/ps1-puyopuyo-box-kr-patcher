//! QUEST 공용 SEQ base opcode의 branch/return 전이와 handler 완료 의미.
//!
//! OP03/04/05/09/0A/0D/0E/39는 현재 stream을 단순 고정 폭으로
//! 전진시키지 않는다. record 포인터, object byte, 전역 word, catalog
//! lookup으로 다음 stream을 고르거나 객체 내부 return stack을 사용한다.
//! 이 모듈은 그 주소 계산과 최종 `+0x1C` store를 원본 명령으로 검증한다.
//! OP00은 `+4` 직접 전진이면서 현재 interpreter를 해제하고 dispatcher를
//! yield한다. 이 handler 완료 의미는 직접 전진 여부와 직교하므로 branch/
//! return 전이 분류에 섞지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::quest_seq_opcode_handler;

use super::linear;
use super::quest_control::{QUEST_RELEASE_OBJECT_RUNTIME, QUEST_SEQ_DISPATCH_YIELD_GP_OFFSET};

pub const QUEST_SEQ_RETURN_DEPTH_OFFSET: i16 = 0x3C;
pub const QUEST_SEQ_RETURN_STACK_OFFSET: i16 = 0x28;
pub const QUEST_SEQ_BRANCH_STATE_BASE_RUNTIME: u32 = 0x800D_B6C0;
pub const QUEST_SEQ_CATALOG_GROUP_TABLE_RUNTIME: u32 = 0x800D_B328;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqBaseTransitionMeaning {
    DirectBranchPointer,
    PushReturnAndDirectBranchPointer,
    PopReturnStream,
    ObjectByteIndexedBranchTable,
    PushReturnAndObjectByteIndexedBranchTable,
    GlobalWordIndexedBranchTable,
    PushReturnAndGlobalWordIndexedBranchTable,
    PushReturnAndCatalogBranch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqBaseTransitionSpec {
    pub opcode: u8,
    pub handler_runtime: u32,
    pub meaning: QuestSeqBaseTransitionMeaning,
}

pub const QUEST_SEQ_BASE_TRANSITION_SPECS: [QuestSeqBaseTransitionSpec; 8] = [
    QuestSeqBaseTransitionSpec {
        opcode: 0x03,
        handler_runtime: 0x8002_2700,
        meaning: QuestSeqBaseTransitionMeaning::DirectBranchPointer,
    },
    QuestSeqBaseTransitionSpec {
        opcode: 0x04,
        handler_runtime: 0x8002_2714,
        meaning: QuestSeqBaseTransitionMeaning::PushReturnAndDirectBranchPointer,
    },
    QuestSeqBaseTransitionSpec {
        opcode: 0x05,
        handler_runtime: 0x8002_2748,
        meaning: QuestSeqBaseTransitionMeaning::PopReturnStream,
    },
    QuestSeqBaseTransitionSpec {
        opcode: 0x09,
        handler_runtime: 0x8002_28BC,
        meaning: QuestSeqBaseTransitionMeaning::ObjectByteIndexedBranchTable,
    },
    QuestSeqBaseTransitionSpec {
        opcode: 0x0A,
        handler_runtime: 0x8002_28EC,
        meaning: QuestSeqBaseTransitionMeaning::PushReturnAndObjectByteIndexedBranchTable,
    },
    QuestSeqBaseTransitionSpec {
        opcode: 0x0D,
        handler_runtime: 0x8002_2948,
        meaning: QuestSeqBaseTransitionMeaning::GlobalWordIndexedBranchTable,
    },
    QuestSeqBaseTransitionSpec {
        opcode: 0x0E,
        handler_runtime: 0x8002_297C,
        meaning: QuestSeqBaseTransitionMeaning::PushReturnAndGlobalWordIndexedBranchTable,
    },
    QuestSeqBaseTransitionSpec {
        opcode: 0x39,
        handler_runtime: 0x8002_32DC,
        meaning: QuestSeqBaseTransitionMeaning::PushReturnAndCatalogBranch,
    },
];

/// 직접 전진 여부와 독립적인 base handler의 수명주기 효과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqBaseLifecycleMeaning {
    ReleaseInterpreterAndYieldDispatcher,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqBaseLifecycleSpec {
    pub opcode: u8,
    pub handler_runtime: u32,
    pub meaning: QuestSeqBaseLifecycleMeaning,
}

/// OP00은 stream을 `+4`로 옮기지만 같은 pass에서 다음 record를 소비하지 않는다.
pub const QUEST_SEQ_BASE_LIFECYCLE_SPECS: [QuestSeqBaseLifecycleSpec; 1] =
    [QuestSeqBaseLifecycleSpec {
        opcode: 0x00,
        handler_runtime: 0x8002_2650,
        meaning: QuestSeqBaseLifecycleMeaning::ReleaseInterpreterAndYieldDispatcher,
    }];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqBaseTransitionSummary {
    pub meanings: usize,
    pub lifecycle_meanings: usize,
    pub stack_branch_meanings: usize,
    pub return_meanings: usize,
    pub branch_table_meanings: usize,
    pub branch_state_base_runtime: u32,
    pub catalog_group_table_runtime: u32,
    pub release_interpreter_runtime: u32,
    pub dispatch_yield_gp_offset: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestSeqBaseTransitionError {
    WrongExecutable(&'static str),
    TextLengthMismatch {
        expected: usize,
        actual: usize,
    },
    UnalignedText,
    SpecMismatch {
        opcode: u8,
        expected_handler: u32,
        actual_handler: Option<u32>,
    },
    MissingInstruction(u32),
    InstructionMismatch {
        opcode: u8,
        address: u32,
        expected: &'static str,
        actual: K,
    },
}

impl fmt::Display for QuestSeqBaseTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(f, "SEQ base 전이 검증 대상이 QUEST.EXE가 아님: {exe}")
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                f,
                "QUEST.EXE text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText => write!(f, "QUEST.EXE text 길이가 4바이트 정렬이 아님"),
            Self::SpecMismatch {
                opcode,
                expected_handler,
                actual_handler,
            } => write!(
                f,
                "SEQ OP{opcode:02X} base 전이 handler 불일치: \
                 expected=0x{expected_handler:08X}, actual={actual_handler:?}"
            ),
            Self::MissingInstruction(address) => {
                write!(f, "SEQ base 전이 명령이 text 밖임: 0x{address:08X}")
            }
            Self::InstructionMismatch {
                opcode,
                address,
                expected,
                actual,
            } => write!(
                f,
                "SEQ OP{opcode:02X} base 전이 명령 불일치: address=0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
        }
    }
}

impl Error for QuestSeqBaseTransitionError {}

#[derive(Debug, Clone, Copy)]
struct Evidence {
    opcode: u8,
    address: u32,
    expected: &'static str,
    kind: K,
}

const EVIDENCE: &[Evidence] = &[
    evidence(
        0x00,
        0x8002_265C,
        "jal 0x80014CD0",
        K::Jal {
            target: QUEST_RELEASE_OBJECT_RUNTIME,
        },
    ),
    evidence(
        0x00,
        0x8002_2660,
        "move s0, a0",
        K::Addu {
            rd: Reg::S0,
            rs: Reg::A0,
            rt: Reg::ZERO,
        },
    ),
    evidence(
        0x00,
        0x8002_2664,
        "lw v0, 0x1C(s0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x00,
        0x8002_266C,
        "addiu v0, v0, 4",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 4,
        },
    ),
    evidence(
        0x00,
        0x8002_2670,
        "sw v0, 0x1C(s0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x00,
        0x8002_267C,
        "addiu v1, zero, 1",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 1,
        },
    ),
    evidence(
        0x00,
        0x8002_2680,
        "sw v1, 0x518(gp)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::GP,
            offset: QUEST_SEQ_DISPATCH_YIELD_GP_OFFSET,
        },
    ),
    evidence(
        0x03,
        0x8002_2700,
        "lw v0, 0x1C(a0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x03,
        0x8002_2708,
        "lw v0, 4(v0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 4,
        },
    ),
    evidence(
        0x03,
        0x8002_2710,
        "sw v0, 0x1C(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x04,
        0x8002_2714,
        "lbu v1, 0x3C(a0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: QUEST_SEQ_RETURN_DEPTH_OFFSET,
        },
    ),
    evidence(
        0x04,
        0x8002_2718,
        "lw v0, 0x1C(a0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x04,
        0x8002_271C,
        "sll v1, v1, 2",
        K::Sll {
            rd: Reg::V1,
            rt: Reg::V1,
            sa: 2,
        },
    ),
    evidence(
        0x04,
        0x8002_2720,
        "addu v1, v1, a0",
        K::Addu {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::A0,
        },
    ),
    evidence(
        0x04,
        0x8002_2724,
        "addiu v0, v0, 8",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 8,
        },
    ),
    evidence(
        0x04,
        0x8002_2728,
        "sw v0, 0x28(v1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: QUEST_SEQ_RETURN_STACK_OFFSET,
        },
    ),
    evidence(
        0x04,
        0x8002_272C,
        "lbu v0, 0x3C(a0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_RETURN_DEPTH_OFFSET,
        },
    ),
    evidence(
        0x04,
        0x8002_2730,
        "lw v1, 0x1C(a0)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x04,
        0x8002_2734,
        "addiu v0, v0, 1",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 1,
        },
    ),
    evidence(
        0x04,
        0x8002_2738,
        "sb v0, 0x3C(a0)",
        K::Sb {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_RETURN_DEPTH_OFFSET,
        },
    ),
    evidence(
        0x04,
        0x8002_273C,
        "lw v0, 4(v1)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 4,
        },
    ),
    evidence(
        0x04,
        0x8002_2744,
        "sw v0, 0x1C(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x05,
        0x8002_2748,
        "lbu v0, 0x3C(a0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_RETURN_DEPTH_OFFSET,
        },
    ),
    evidence(
        0x05,
        0x8002_2750,
        "addiu v0, v0, -1",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: -1,
        },
    ),
    evidence(
        0x05,
        0x8002_2754,
        "sb v0, 0x3C(a0)",
        K::Sb {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_RETURN_DEPTH_OFFSET,
        },
    ),
    evidence(
        0x05,
        0x8002_2758,
        "andi v0, v0, 0xFF",
        K::Andi {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0xFF,
        },
    ),
    evidence(
        0x05,
        0x8002_275C,
        "sll v0, v0, 2",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    ),
    evidence(
        0x05,
        0x8002_2760,
        "addu v0, v0, a0",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::A0,
        },
    ),
    evidence(
        0x05,
        0x8002_2764,
        "lw v0, 0x28(v0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: QUEST_SEQ_RETURN_STACK_OFFSET,
        },
    ),
    evidence(
        0x05,
        0x8002_276C,
        "sw v0, 0x1C(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x09,
        0x8002_28BC,
        "lw v1, 0x1C(a0)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x09,
        0x8002_28C4,
        "lbu v0, 1(v1)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 1,
        },
    ),
    evidence(
        0x09,
        0x8002_28CC,
        "addu v0, a0, v0",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::A0,
            rt: Reg::V0,
        },
    ),
    evidence(
        0x09,
        0x8002_28D0,
        "lbu v0, 0(v0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 0,
        },
    ),
    evidence(
        0x09,
        0x8002_28D8,
        "sll v0, v0, 2",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    ),
    evidence(
        0x09,
        0x8002_28DC,
        "addu v0, v0, v1",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::V1,
        },
    ),
    evidence(
        0x09,
        0x8002_28E0,
        "lw v0, 4(v0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 4,
        },
    ),
    evidence(
        0x09,
        0x8002_28E8,
        "sw v0, 0x1C(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x0A,
        0x8002_28EC,
        "lbu v1, 0x3C(a0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: QUEST_SEQ_RETURN_DEPTH_OFFSET,
        },
    ),
    evidence(
        0x0A,
        0x8002_28F0,
        "lw a1, 0x1C(a0)",
        K::Lw {
            rt: Reg::A1,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x0A,
        0x8002_28F4,
        "sll v1, v1, 2",
        K::Sll {
            rd: Reg::V1,
            rt: Reg::V1,
            sa: 2,
        },
    ),
    evidence(
        0x0A,
        0x8002_28F8,
        "lbu v0, 2(a1)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::A1,
            offset: 2,
        },
    ),
    evidence(
        0x0A,
        0x8002_28FC,
        "addu v1, v1, a0",
        K::Addu {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::A0,
        },
    ),
    evidence(
        0x0A,
        0x8002_2904,
        "addiu v0, v0, 4",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 4,
        },
    ),
    evidence(
        0x0A,
        0x8002_2908,
        "addu a1, a1, v0",
        K::Addu {
            rd: Reg::A1,
            rs: Reg::A1,
            rt: Reg::V0,
        },
    ),
    evidence(
        0x0A,
        0x8002_290C,
        "sw a1, 0x28(v1)",
        K::Sw {
            rt: Reg::A1,
            base: Reg::V1,
            offset: QUEST_SEQ_RETURN_STACK_OFFSET,
        },
    ),
    evidence(
        0x0A,
        0x8002_2910,
        "lbu v0, 0x3C(a0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_RETURN_DEPTH_OFFSET,
        },
    ),
    evidence(
        0x0A,
        0x8002_2914,
        "lw v1, 0x1C(a0)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x0A,
        0x8002_2918,
        "addiu v0, v0, 1",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 1,
        },
    ),
    evidence(
        0x0A,
        0x8002_291C,
        "sb v0, 0x3C(a0)",
        K::Sb {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_RETURN_DEPTH_OFFSET,
        },
    ),
    evidence(
        0x0A,
        0x8002_2920,
        "lbu v0, 1(v1)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 1,
        },
    ),
    evidence(
        0x0A,
        0x8002_2928,
        "addu v0, a0, v0",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::A0,
            rt: Reg::V0,
        },
    ),
    evidence(
        0x0A,
        0x8002_292C,
        "lbu v0, 0(v0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 0,
        },
    ),
    evidence(
        0x0A,
        0x8002_2934,
        "sll v0, v0, 2",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    ),
    evidence(
        0x0A,
        0x8002_2938,
        "addu v0, v0, v1",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::V1,
        },
    ),
    evidence(
        0x0A,
        0x8002_293C,
        "lw v0, 4(v0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 4,
        },
    ),
    evidence(
        0x0A,
        0x8002_2944,
        "sw v0, 0x1C(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x0D,
        0x8002_2948,
        "lw a1, 0x1C(a0)",
        K::Lw {
            rt: Reg::A1,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x0D,
        0x8002_294C,
        "lui v1, 0x800E",
        K::Lui {
            rt: Reg::V1,
            imm: 0x800E,
        },
    ),
    evidence(
        0x0D,
        0x8002_2950,
        "lbu v0, 1(a1)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::A1,
            offset: 1,
        },
    ),
    evidence(
        0x0D,
        0x8002_2954,
        "addiu v1, v1, -0x4940",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V1,
            imm: -0x4940,
        },
    ),
    evidence(
        0x0D,
        0x8002_2958,
        "sll v0, v0, 2",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    ),
    evidence(
        0x0D,
        0x8002_295C,
        "addu v0, v0, v1",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::V1,
        },
    ),
    evidence(
        0x0D,
        0x8002_2960,
        "lw v0, 0(v0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 0,
        },
    ),
    evidence(
        0x0D,
        0x8002_2968,
        "sll v0, v0, 2",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    ),
    evidence(
        0x0D,
        0x8002_296C,
        "addu v0, v0, a1",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x0D,
        0x8002_2970,
        "lw v0, 4(v0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 4,
        },
    ),
    evidence(
        0x0D,
        0x8002_2978,
        "sw v0, 0x1C(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x0E,
        0x8002_297C,
        "lbu v1, 0x3C(a0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: QUEST_SEQ_RETURN_DEPTH_OFFSET,
        },
    ),
    evidence(
        0x0E,
        0x8002_2980,
        "lw a1, 0x1C(a0)",
        K::Lw {
            rt: Reg::A1,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x0E,
        0x8002_2984,
        "sll v1, v1, 2",
        K::Sll {
            rd: Reg::V1,
            rt: Reg::V1,
            sa: 2,
        },
    ),
    evidence(
        0x0E,
        0x8002_2988,
        "lbu v0, 2(a1)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::A1,
            offset: 2,
        },
    ),
    evidence(
        0x0E,
        0x8002_298C,
        "addu v1, v1, a0",
        K::Addu {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::A0,
        },
    ),
    evidence(
        0x0E,
        0x8002_2994,
        "addiu v0, v0, 4",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 4,
        },
    ),
    evidence(
        0x0E,
        0x8002_2998,
        "addu a1, a1, v0",
        K::Addu {
            rd: Reg::A1,
            rs: Reg::A1,
            rt: Reg::V0,
        },
    ),
    evidence(
        0x0E,
        0x8002_299C,
        "sw a1, 0x28(v1)",
        K::Sw {
            rt: Reg::A1,
            base: Reg::V1,
            offset: QUEST_SEQ_RETURN_STACK_OFFSET,
        },
    ),
    evidence(
        0x0E,
        0x8002_29A0,
        "lui v1, 0x800E",
        K::Lui {
            rt: Reg::V1,
            imm: 0x800E,
        },
    ),
    evidence(
        0x0E,
        0x8002_29A4,
        "lbu v0, 0x3C(a0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_RETURN_DEPTH_OFFSET,
        },
    ),
    evidence(
        0x0E,
        0x8002_29A8,
        "lw a1, 0x1C(a0)",
        K::Lw {
            rt: Reg::A1,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x0E,
        0x8002_29AC,
        "addiu v0, v0, 1",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 1,
        },
    ),
    evidence(
        0x0E,
        0x8002_29B0,
        "sb v0, 0x3C(a0)",
        K::Sb {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_RETURN_DEPTH_OFFSET,
        },
    ),
    evidence(
        0x0E,
        0x8002_29B4,
        "lbu v0, 1(a1)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::A1,
            offset: 1,
        },
    ),
    evidence(
        0x0E,
        0x8002_29B8,
        "addiu v1, v1, -0x4940",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V1,
            imm: -0x4940,
        },
    ),
    evidence(
        0x0E,
        0x8002_29BC,
        "sll v0, v0, 2",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    ),
    evidence(
        0x0E,
        0x8002_29C0,
        "addu v0, v0, v1",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::V1,
        },
    ),
    evidence(
        0x0E,
        0x8002_29C4,
        "lw v0, 0(v0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 0,
        },
    ),
    evidence(
        0x0E,
        0x8002_29CC,
        "sll v0, v0, 2",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    ),
    evidence(
        0x0E,
        0x8002_29D0,
        "addu v0, v0, a1",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::A1,
        },
    ),
    evidence(
        0x0E,
        0x8002_29D4,
        "lw v0, 4(v0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 4,
        },
    ),
    evidence(
        0x0E,
        0x8002_29DC,
        "sw v0, 0x1C(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x39,
        0x8002_32DC,
        "lbu v1, 0x3C(a0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: QUEST_SEQ_RETURN_DEPTH_OFFSET,
        },
    ),
    evidence(
        0x39,
        0x8002_32E0,
        "lw v0, 0x1C(a0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x39,
        0x8002_32E4,
        "sll v1, v1, 2",
        K::Sll {
            rd: Reg::V1,
            rt: Reg::V1,
            sa: 2,
        },
    ),
    evidence(
        0x39,
        0x8002_32E8,
        "addu v1, v1, a0",
        K::Addu {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::A0,
        },
    ),
    evidence(
        0x39,
        0x8002_32EC,
        "addiu v0, v0, 8",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 8,
        },
    ),
    evidence(
        0x39,
        0x8002_32F0,
        "sw v0, 0x28(v1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: QUEST_SEQ_RETURN_STACK_OFFSET,
        },
    ),
    evidence(
        0x39,
        0x8002_32F4,
        "lbu v0, 0x3C(a0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_RETURN_DEPTH_OFFSET,
        },
    ),
    evidence(
        0x39,
        0x8002_32F8,
        "lw a2, 0x1C(a0)",
        K::Lw {
            rt: Reg::A2,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    evidence(
        0x39,
        0x8002_32FC,
        "addiu v0, v0, 1",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 1,
        },
    ),
    evidence(
        0x39,
        0x8002_3300,
        "sb v0, 0x3C(a0)",
        K::Sb {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_RETURN_DEPTH_OFFSET,
        },
    ),
    evidence(
        0x39,
        0x8002_3304,
        "lui v0, 0x800E",
        K::Lui {
            rt: Reg::V0,
            imm: 0x800E,
        },
    ),
    evidence(
        0x39,
        0x8002_3308,
        "addiu v0, v0, -0x4CD8",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: -0x4CD8,
        },
    ),
    evidence(
        0x39,
        0x8002_330C,
        "lbu v1, 1(a2)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A2,
            offset: 1,
        },
    ),
    evidence(
        0x39,
        0x8002_3310,
        "lhu a1, 2(a2)",
        K::Lhu {
            rt: Reg::A1,
            base: Reg::A2,
            offset: 2,
        },
    ),
    evidence(
        0x39,
        0x8002_3324,
        "lw v1, 0(v1)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::V1,
            offset: 0,
        },
    ),
    evidence(
        0x39,
        0x8002_3330,
        "lbu v1, 4(a2)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A2,
            offset: 4,
        },
    ),
    evidence(
        0x39,
        0x8002_3334,
        "lw v0, 16(v0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 16,
        },
    ),
    evidence(
        0x39,
        0x8002_3340,
        "lw v0, 4(v1)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 4,
        },
    ),
    evidence(
        0x39,
        0x8002_3348,
        "sw v0, 0x1C(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
];

const fn evidence(opcode: u8, address: u32, expected: &'static str, kind: K) -> Evidence {
    Evidence {
        opcode,
        address,
        expected,
        kind,
    }
}

pub fn verify_box_quest_seq_base_transitions(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqBaseTransitionSummary, QuestSeqBaseTransitionError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestSeqBaseTransitionError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestSeqBaseTransitionError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }
    let instructions =
        linear::sweep(map, text).ok_or(QuestSeqBaseTransitionError::UnalignedText)?;

    for spec in QUEST_SEQ_BASE_TRANSITION_SPECS {
        if quest_seq_opcode_handler(spec.opcode) != Some(spec.handler_runtime) {
            return Err(QuestSeqBaseTransitionError::SpecMismatch {
                opcode: spec.opcode,
                expected_handler: spec.handler_runtime,
                actual_handler: quest_seq_opcode_handler(spec.opcode),
            });
        }
    }
    for spec in QUEST_SEQ_BASE_LIFECYCLE_SPECS {
        if quest_seq_opcode_handler(spec.opcode) != Some(spec.handler_runtime) {
            return Err(QuestSeqBaseTransitionError::SpecMismatch {
                opcode: spec.opcode,
                expected_handler: spec.handler_runtime,
                actual_handler: quest_seq_opcode_handler(spec.opcode),
            });
        }
    }
    for evidence in EVIDENCE {
        require(&instructions, *evidence)?;
    }

    Ok(QuestSeqBaseTransitionSummary {
        meanings: QUEST_SEQ_BASE_TRANSITION_SPECS.len(),
        lifecycle_meanings: QUEST_SEQ_BASE_LIFECYCLE_SPECS.len(),
        stack_branch_meanings: 4,
        return_meanings: 1,
        branch_table_meanings: 5,
        branch_state_base_runtime: QUEST_SEQ_BRANCH_STATE_BASE_RUNTIME,
        catalog_group_table_runtime: QUEST_SEQ_CATALOG_GROUP_TABLE_RUNTIME,
        release_interpreter_runtime: QUEST_RELEASE_OBJECT_RUNTIME,
        dispatch_yield_gp_offset: QUEST_SEQ_DISPATCH_YIELD_GP_OFFSET,
    })
}

fn require(
    instructions: &[Instruction],
    evidence: Evidence,
) -> Result<(), QuestSeqBaseTransitionError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == evidence.address)
        .ok_or(QuestSeqBaseTransitionError::MissingInstruction(
            evidence.address,
        ))?;
    if actual.kind != evidence.kind {
        return Err(QuestSeqBaseTransitionError::InstructionMismatch {
            opcode: evidence.opcode,
            address: evidence.address,
            expected: evidence.expected,
            actual: actual.kind,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_seq_base_transition_tests.rs"]
mod tests;
