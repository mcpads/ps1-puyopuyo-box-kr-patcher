//! QUEST font page source를 준비할 때 재사용할 원본 CNX 경계.
//!
//! direct CNX wrapper는 `(해제 목적지, CNX source)`를 받아 동기적으로
//! core를 반복 호출한다. streaming wrapper와 같은 `$gp` 상태를 쓰므로
//! 두 경로를 동시에 사용할 수 있다는 뜻은 아니다. 원본 loader arena도
//! 장기 resident 저장소가 아니라 재사용 scratch다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{verify_bytes, InstructionKind as K, IsaProfile, Reg, VerifyError};

pub const QUEST_CNX_CORE_RUNTIME: u32 = 0x8001_651C;
pub const QUEST_CNX_DIRECT_WRAPPER_RUNTIME: u32 = 0x8001_66CC;
pub const QUEST_CNX_DIRECT_WRAPPER_END_RUNTIME: u32 = 0x8001_6774;
pub const QUEST_CNX_DIRECT_CALL_RUNTIME: u32 = 0x8001_8DB4;
pub const QUEST_CNX_STREAMING_WRAPPER_RUNTIME: u32 = 0x8001_67E0;
pub const QUEST_CNX_STREAMING_CORE_CALL_RUNTIME: u32 = 0x8001_6830;

pub const QUEST_CNX_PROGRESS_GP_OFFSET: i16 = 0x04C0;
pub const QUEST_CNX_INPUT_PROGRESS_GP_OFFSET: i16 = 0x04C4;
pub const QUEST_CNX_INPUT_POINTER_GP_OFFSET: i16 = 0x04C8;
pub const QUEST_CNX_OUTPUT_POINTER_GP_OFFSET: i16 = 0x04CC;
pub const QUEST_CNX_OUTPUT_BYTES_GP_OFFSET: i16 = 0x04D0;
pub const QUEST_LOADER_ARENA_POINTER_GP_OFFSET: i16 = 0x0500;

pub const QUEST_LOADER_ARENA_START_RUNTIME: u32 = 0x8012_1724;
pub const QUEST_LOADER_ARENA_END_RUNTIME: u32 = 0x8018_3A00;
pub const QUEST_LOADER_ARENA_BYTES: usize =
    (QUEST_LOADER_ARENA_END_RUNTIME - QUEST_LOADER_ARENA_START_RUNTIME) as usize;

const DIRECT_WRAPPER_BYTES: usize =
    (QUEST_CNX_DIRECT_WRAPPER_END_RUNTIME - QUEST_CNX_DIRECT_WRAPPER_RUNTIME) as usize;

/// QUEST page source 준비에 필요한 원본 direct decoder와 scratch 경계.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestPageSourceSummary {
    pub profile: IsaProfile,
    pub direct_wrapper_runtime: u32,
    pub direct_wrapper_bytes: usize,
    pub direct_call_runtime: u32,
    pub core_runtime: u32,
    pub streaming_wrapper_runtime: u32,
    pub streaming_core_call_runtime: u32,
    pub shared_gp_state_fields: usize,
    pub arena_start_runtime: u32,
    pub arena_end_runtime: u32,
    pub arena_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestPageSourceError {
    WrongExecutable(&'static str),
    TextLengthMismatch {
        expected: usize,
        actual: usize,
    },
    CoordinateOverflow,
    RangeOutsideText {
        start: u32,
        bytes: usize,
    },
    MachineCode(VerifyError),
    InstructionMismatch {
        address: u32,
        expected: &'static str,
        actual: K,
    },
    ArenaMismatch,
}

impl fmt::Display for QuestPageSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(f, "QUEST page source 검증 대상이 QUEST.EXE가 아님: {exe}")
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                f,
                "QUEST page source EXE 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::CoordinateOverflow => f.write_str("QUEST page source 좌표 overflow"),
            Self::RangeOutsideText { start, bytes } => write!(
                f,
                "QUEST page source 원본 범위 이탈: 0x{start:08X} + {bytes}B"
            ),
            Self::MachineCode(source) => {
                write!(f, "QUEST page source ISA 검산 실패: {source}")
            }
            Self::InstructionMismatch {
                address,
                expected,
                actual,
            } => write!(
                f,
                "QUEST page source 명령 불일치: address=0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
            Self::ArenaMismatch => f.write_str("QUEST loader arena 크기 산술 불일치"),
        }
    }
}

impl Error for QuestPageSourceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MachineCode(source) => Some(source),
            _ => None,
        }
    }
}

impl From<VerifyError> for QuestPageSourceError {
    fn from(value: VerifyError) -> Self {
        Self::MachineCode(value)
    }
}

fn verified_range(
    map: &ExeMap,
    text: &[u8],
    start: u32,
    bytes: usize,
) -> Result<crate::isa::VerifiedMachineCode, QuestPageSourceError> {
    let offset = map
        .text_offset(start)
        .ok_or(QuestPageSourceError::RangeOutsideText { start, bytes })?;
    let end = offset
        .checked_add(bytes)
        .ok_or(QuestPageSourceError::CoordinateOverflow)?;
    let raw = text
        .get(offset..end)
        .ok_or(QuestPageSourceError::RangeOutsideText { start, bytes })?;
    verify_bytes(IsaProfile::DECLARED, start, raw).map_err(Into::into)
}

fn expect(
    verified: &crate::isa::VerifiedMachineCode,
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestPageSourceError> {
    let actual = verified
        .instructions()
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or(QuestPageSourceError::RangeOutsideText {
            start: address,
            bytes: 4,
        })?
        .kind;
    if actual != kind {
        return Err(QuestPageSourceError::InstructionMismatch {
            address,
            expected,
            actual,
        });
    }
    Ok(())
}

/// 지원 원본의 동기 CNX wrapper와 공유 scratch 상태를 typed MIPS로 검증한다.
pub fn verify_box_quest_page_source(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestPageSourceSummary, QuestPageSourceError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestPageSourceError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestPageSourceError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }
    if QUEST_LOADER_ARENA_BYTES != 402_140 {
        return Err(QuestPageSourceError::ArenaMismatch);
    }

    let direct = verified_range(
        map,
        text,
        QUEST_CNX_DIRECT_WRAPPER_RUNTIME,
        DIRECT_WRAPPER_BYTES,
    )?;
    let direct_expected = [
        (
            0x8001_66D0,
            "move a2, a1",
            K::Addu {
                rd: Reg::A2,
                rs: Reg::A1,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8001_66D4,
            "lui v1, 0x0258",
            K::Lui {
                rt: Reg::V1,
                imm: 0x0258,
            },
        ),
        (
            0x8001_66E0,
            "ori v1, v1, 0x4E43",
            K::Ori {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x4E43,
            },
        ),
        (
            0x8001_66E4,
            "bne v0, v1, failure",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::V1,
                offset: 31,
            },
        ),
        (
            0x8001_66E8,
            "move v0, zero",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8001_66EC,
            "lw a1, 0x0C(a2)",
            K::Lw {
                rt: Reg::A1,
                base: Reg::A2,
                offset: 0x0C,
            },
        ),
        (
            0x8001_66F4,
            "sw 16, input progress",
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: QUEST_CNX_INPUT_PROGRESS_GP_OFFSET,
            },
        ),
        (
            0x8001_66FC,
            "sw output pointer",
            K::Sw {
                rt: Reg::A0,
                base: Reg::GP,
                offset: QUEST_CNX_OUTPUT_POINTER_GP_OFFSET,
            },
        ),
        (
            0x8001_6704,
            "sw zero, output progress",
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::GP,
                offset: QUEST_CNX_PROGRESS_GP_OFFSET,
            },
        ),
        (
            0x8001_6708,
            "sw CNX body pointer",
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: QUEST_CNX_INPUT_POINTER_GP_OFFSET,
            },
        ),
        (
            0x8001_6730,
            "sw decoded byte count",
            K::Sw {
                rt: Reg::V1,
                base: Reg::GP,
                offset: QUEST_CNX_OUTPUT_BYTES_GP_OFFSET,
            },
        ),
        (
            0x8001_6744,
            "jal CNX core",
            K::Jal {
                target: QUEST_CNX_CORE_RUNTIME,
            },
        ),
        (
            0x8001_6748,
            "move a2, zero",
            K::Addu {
                rd: Reg::A2,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8001_675C,
            "bne v0, zero, decode loop",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: -9,
            },
        ),
        (
            0x8001_6760,
            "addiu v0, zero, 1",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
    ];
    for (address, label, kind) in direct_expected {
        expect(&direct, address, label, kind)?;
    }

    let call = verified_range(map, text, QUEST_CNX_DIRECT_CALL_RUNTIME - 8, 16)?;
    let call_expected = [
        (
            QUEST_CNX_DIRECT_CALL_RUNTIME - 8,
            "lw a0, 0x18(s0)",
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x18,
            },
        ),
        (
            QUEST_CNX_DIRECT_CALL_RUNTIME - 4,
            "lw a1, 0x14(s0)",
            K::Lw {
                rt: Reg::A1,
                base: Reg::S0,
                offset: 0x14,
            },
        ),
        (
            QUEST_CNX_DIRECT_CALL_RUNTIME,
            "jal direct CNX wrapper",
            K::Jal {
                target: QUEST_CNX_DIRECT_WRAPPER_RUNTIME,
            },
        ),
    ];
    for (address, label, kind) in call_expected {
        expect(&call, address, label, kind)?;
    }

    let streaming = verified_range(map, text, QUEST_CNX_STREAMING_WRAPPER_RUNTIME, 0x58)?;
    let streaming_expected = [
        (
            0x8001_67E0,
            "lw output progress",
            K::Lw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: QUEST_CNX_PROGRESS_GP_OFFSET,
            },
        ),
        (
            0x8001_67E4,
            "lw decoded byte count",
            K::Lw {
                rt: Reg::V1,
                base: Reg::GP,
                offset: QUEST_CNX_OUTPUT_BYTES_GP_OFFSET,
            },
        ),
        (
            0x8001_680C,
            "sw streaming output pointer",
            K::Sw {
                rt: Reg::A0,
                base: Reg::GP,
                offset: QUEST_CNX_OUTPUT_POINTER_GP_OFFSET,
            },
        ),
        (
            0x8001_6810,
            "sw streaming input pointer",
            K::Sw {
                rt: Reg::A1,
                base: Reg::GP,
                offset: QUEST_CNX_INPUT_POINTER_GP_OFFSET,
            },
        ),
        (
            QUEST_CNX_STREAMING_CORE_CALL_RUNTIME,
            "jal CNX core from streaming wrapper",
            K::Jal {
                target: QUEST_CNX_CORE_RUNTIME,
            },
        ),
        (
            QUEST_CNX_STREAMING_CORE_CALL_RUNTIME + 4,
            "move a2, s4",
            K::Addu {
                rd: Reg::A2,
                rs: Reg::S4,
                rt: Reg::ZERO,
            },
        ),
    ];
    for (address, label, kind) in streaming_expected {
        expect(&streaming, address, label, kind)?;
    }

    let arena_init = verified_range(map, text, 0x8001_B0F8, 0x18)?;
    let arena_init_expected = [
        (
            0x8001_B0F8,
            "lui arena start",
            K::Lui {
                rt: Reg::V0,
                imm: 0x8012,
            },
        ),
        (
            0x8001_B0FC,
            "ori arena start",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x1724,
            },
        ),
        (
            0x8001_B10C,
            "sw arena pointer",
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: QUEST_LOADER_ARENA_POINTER_GP_OFFSET,
            },
        ),
    ];
    for (address, label, kind) in arena_init_expected {
        expect(&arena_init, address, label, kind)?;
    }

    let arena_wrap = verified_range(map, text, 0x8001_BDA0, 0x4C)?;
    let arena_wrap_expected = [
        (
            0x8001_BDC0,
            "lui arena end",
            K::Lui {
                rt: Reg::V0,
                imm: 0x8018,
            },
        ),
        (
            0x8001_BDC4,
            "ori arena end",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x3A00,
            },
        ),
        (
            0x8001_BDCC,
            "lw arena pointer",
            K::Lw {
                rt: Reg::A0,
                base: Reg::GP,
                offset: QUEST_LOADER_ARENA_POINTER_GP_OFFSET,
            },
        ),
        (
            0x8001_BDE4,
            "ori wrapped arena start",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x1724,
            },
        ),
        (
            0x8001_BDE8,
            "sw wrapped arena pointer",
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: QUEST_LOADER_ARENA_POINTER_GP_OFFSET,
            },
        ),
    ];
    for (address, label, kind) in arena_wrap_expected {
        expect(&arena_wrap, address, label, kind)?;
    }

    Ok(QuestPageSourceSummary {
        profile: IsaProfile::DECLARED,
        direct_wrapper_runtime: QUEST_CNX_DIRECT_WRAPPER_RUNTIME,
        direct_wrapper_bytes: DIRECT_WRAPPER_BYTES,
        direct_call_runtime: QUEST_CNX_DIRECT_CALL_RUNTIME,
        core_runtime: QUEST_CNX_CORE_RUNTIME,
        streaming_wrapper_runtime: QUEST_CNX_STREAMING_WRAPPER_RUNTIME,
        streaming_core_call_runtime: QUEST_CNX_STREAMING_CORE_CALL_RUNTIME,
        shared_gp_state_fields: 5,
        arena_start_runtime: QUEST_LOADER_ARENA_START_RUNTIME,
        arena_end_runtime: QUEST_LOADER_ARENA_END_RUNTIME,
        arena_bytes: QUEST_LOADER_ARENA_BYTES,
    })
}

#[cfg(test)]
#[path = "quest_page_source_tests.rs"]
mod tests;
