//! QUEST font hook 설치 공간의 EXE tail 경계.
//!
//! 원본 text 끝의 0바이트를 곧바로 code cave로 취급하지 않는다. startup
//! zero-clear loop, linker section descriptor, `$gp` 기준과 실제 mutable
//! global 접근을 함께 검증해 설치 불가 범위를 고정한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{verify_bytes, InstructionKind as K, IsaProfile, Reg, VerifyError};

pub const QUEST_HOOK_LAST_INITIALIZED_WORD_RUNTIME: u32 = 0x800B_E408;
pub const QUEST_HOOK_INITIALIZED_END_RUNTIME: u32 = 0x800B_E40C;
pub const QUEST_HOOK_ZERO_CLEAR_START_RUNTIME: u32 = 0x800B_E410;
pub const QUEST_HOOK_ZERO_CLEAR_END_RUNTIME: u32 = 0x8010_8348;
pub const QUEST_HOOK_TEXT_END_RUNTIME: u32 = 0x800B_E800;
pub const QUEST_HOOK_SECTION_TABLE_RUNTIME: u32 = 0x800B_DE40;
pub const QUEST_HOOK_GP_RUNTIME: u32 = 0x800B_DF88;
pub const QUEST_HOOK_MUTABLE_SELECTOR_RUNTIME: u32 = 0x800B_E470;

const LAST_INITIALIZED_WORD: u32 = 0x47FF_0606;
const STARTUP_CLEAR_RUNTIME: u32 = 0x800A_A6E8;
const STARTUP_CLEAR_BYTES: usize = 36;
const GP_INITIALIZER_RUNTIME: u32 = 0x800A_A764;
const MUTABLE_SELECTOR_STORE_RUNTIME: u32 = 0x8001_B11C;

const SECTION_WORDS: [u32; 6] = [
    0x8001_49D8,
    0x0009_C8D4,
    0x800B_12AC,
    0x0000_CCDC,
    0x800B_E6B8,
    0x0004_9C90,
];

/// QUEST EXE tail을 code cave로 쓸 수 없는 원본 경계 요약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestHookSpaceSummary {
    pub profile: IsaProfile,
    pub initialized_end_runtime: u32,
    pub zero_clear_start_runtime: u32,
    pub zero_clear_end_runtime: u32,
    pub text_end_runtime: u32,
    pub alignment_padding_bytes: usize,
    pub zero_cleared_bytes: usize,
    pub zero_cleared_text_overlap_bytes: usize,
    pub gp_runtime: u32,
    pub mutable_selector_runtime: u32,
    pub section_zero_start_runtime: u32,
    pub section_zero_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestHookSpaceError {
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
    WordMismatch {
        address: u32,
        expected: u32,
        actual: u32,
    },
    NonZeroTail {
        address: u32,
        actual: u8,
    },
    LastInitializedBoundary {
        expected: u32,
        actual: Option<u32>,
    },
    MachineCode(VerifyError),
    InstructionMismatch {
        address: u32,
        expected: &'static str,
        actual: K,
    },
}

impl fmt::Display for QuestHookSpaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(f, "QUEST hook 공간 검증 대상이 QUEST.EXE가 아님: {exe}")
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                f,
                "QUEST hook 공간 EXE 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::CoordinateOverflow => f.write_str("QUEST hook 공간 좌표 overflow"),
            Self::RangeOutsideText { start, bytes } => write!(
                f,
                "QUEST hook 공간 원본 범위 이탈: 0x{start:08X} + {bytes}B"
            ),
            Self::WordMismatch {
                address,
                expected,
                actual,
            } => write!(
                f,
                "QUEST hook 공간 word 불일치: address=0x{address:08X}, \
                 expected=0x{expected:08X}, actual=0x{actual:08X}"
            ),
            Self::NonZeroTail { address, actual } => write!(
                f,
                "QUEST hook 0 tail에 비0 byte가 있음: address=0x{address:08X}, \
                 actual=0x{actual:02X}"
            ),
            Self::LastInitializedBoundary { expected, actual } => write!(
                f,
                "QUEST 마지막 비0 byte 경계 불일치: expected=0x{expected:08X}, \
                 actual={}",
                actual
                    .map(|address| format!("0x{address:08X}"))
                    .unwrap_or_else(|| "없음".to_string())
            ),
            Self::MachineCode(source) => {
                write!(f, "QUEST hook 공간 ISA 검산 실패: {source}")
            }
            Self::InstructionMismatch {
                address,
                expected,
                actual,
            } => write!(
                f,
                "QUEST hook 공간 명령 불일치: address=0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
        }
    }
}

impl Error for QuestHookSpaceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MachineCode(source) => Some(source),
            _ => None,
        }
    }
}

impl From<VerifyError> for QuestHookSpaceError {
    fn from(value: VerifyError) -> Self {
        Self::MachineCode(value)
    }
}

fn text_slice<'a>(
    map: &ExeMap,
    text: &'a [u8],
    start: u32,
    bytes: usize,
) -> Result<&'a [u8], QuestHookSpaceError> {
    let offset = map
        .text_offset(start)
        .ok_or(QuestHookSpaceError::RangeOutsideText { start, bytes })?;
    let end = offset
        .checked_add(bytes)
        .ok_or(QuestHookSpaceError::CoordinateOverflow)?;
    text.get(offset..end)
        .ok_or(QuestHookSpaceError::RangeOutsideText { start, bytes })
}

fn read_word(map: &ExeMap, text: &[u8], address: u32) -> Result<u32, QuestHookSpaceError> {
    let raw: [u8; 4] = text_slice(map, text, address, 4)?
        .try_into()
        .expect("4바이트 slice를 확인함");
    Ok(u32::from_le_bytes(raw))
}

fn expect_instruction(
    actual: K,
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestHookSpaceError> {
    if actual != kind {
        return Err(QuestHookSpaceError::InstructionMismatch {
            address,
            expected,
            actual,
        });
    }
    Ok(())
}

fn verify_startup_clear(map: &ExeMap, text: &[u8]) -> Result<(), QuestHookSpaceError> {
    let raw = text_slice(map, text, STARTUP_CLEAR_RUNTIME, STARTUP_CLEAR_BYTES)?;
    let verified = verify_bytes(IsaProfile::DECLARED, STARTUP_CLEAR_RUNTIME, raw)?;
    let expected = [
        (
            "lui v0, 0x800C",
            K::Lui {
                rt: Reg::V0,
                imm: 0x800C,
            },
        ),
        (
            "addiu v0, v0, -0x1BF0",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -0x1BF0,
            },
        ),
        (
            "lui v1, 0x8011",
            K::Lui {
                rt: Reg::V1,
                imm: 0x8011,
            },
        ),
        (
            "addiu v1, v1, -0x7CB8",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: -0x7CB8,
            },
        ),
        (
            "sw zero, 0(v0)",
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            "addiu v0, v0, 4",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 4,
            },
        ),
        (
            "sltu at, v0, v1",
            K::Sltu {
                rd: Reg::AT,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            "bne at, zero, clear loop",
            K::Bne {
                rs: Reg::AT,
                rt: Reg::ZERO,
                offset: -4,
            },
        ),
        (
            "nop",
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
    ];
    for (instruction, (expected_text, expected_kind)) in
        verified.instructions().iter().zip(expected)
    {
        expect_instruction(
            instruction.kind,
            instruction.addr,
            expected_text,
            expected_kind,
        )?;
    }
    Ok(())
}

fn verify_gp_and_mutable_global(map: &ExeMap, text: &[u8]) -> Result<(), QuestHookSpaceError> {
    let gp = verify_bytes(
        IsaProfile::DECLARED,
        GP_INITIALIZER_RUNTIME,
        text_slice(map, text, GP_INITIALIZER_RUNTIME, 8)?,
    )?;
    let gp_expected = [
        (
            "lui gp, 0x800C",
            K::Lui {
                rt: Reg::GP,
                imm: 0x800C,
            },
        ),
        (
            "addiu gp, gp, -0x2078",
            K::Addiu {
                rt: Reg::GP,
                rs: Reg::GP,
                imm: -0x2078,
            },
        ),
    ];
    for (instruction, (expected_text, expected_kind)) in gp.instructions().iter().zip(gp_expected) {
        expect_instruction(
            instruction.kind,
            instruction.addr,
            expected_text,
            expected_kind,
        )?;
    }

    let mutable = verify_bytes(
        IsaProfile::DECLARED,
        MUTABLE_SELECTOR_STORE_RUNTIME,
        text_slice(map, text, MUTABLE_SELECTOR_STORE_RUNTIME, 8)?,
    )?;
    let mutable_expected = [
        (
            "addiu v0, zero, 1",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            "sw v0, 0x4E8(gp)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: 0x04E8,
            },
        ),
    ];
    for (instruction, (expected_text, expected_kind)) in
        mutable.instructions().iter().zip(mutable_expected)
    {
        expect_instruction(
            instruction.kind,
            instruction.addr,
            expected_text,
            expected_kind,
        )?;
    }
    Ok(())
}

/// 지원 원본의 QUEST EXE tail과 startup zero-clear 경계를 검증한다.
pub fn verify_box_quest_hook_space(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestHookSpaceSummary, QuestHookSpaceError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestHookSpaceError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestHookSpaceError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }
    let (_, actual_text_end) = map.runtime_range();
    if actual_text_end != QUEST_HOOK_TEXT_END_RUNTIME {
        return Err(QuestHookSpaceError::RangeOutsideText {
            start: QUEST_HOOK_TEXT_END_RUNTIME,
            bytes: 0,
        });
    }

    for (index, expected) in SECTION_WORDS.into_iter().enumerate() {
        let address = QUEST_HOOK_SECTION_TABLE_RUNTIME
            .checked_add(
                u32::try_from(index)
                    .map_err(|_| QuestHookSpaceError::CoordinateOverflow)?
                    .checked_mul(4)
                    .ok_or(QuestHookSpaceError::CoordinateOverflow)?,
            )
            .ok_or(QuestHookSpaceError::CoordinateOverflow)?;
        let actual = read_word(map, text, address)?;
        if actual != expected {
            return Err(QuestHookSpaceError::WordMismatch {
                address,
                expected,
                actual,
            });
        }
    }

    let actual_last_word = read_word(map, text, QUEST_HOOK_LAST_INITIALIZED_WORD_RUNTIME)?;
    if actual_last_word != LAST_INITIALIZED_WORD {
        return Err(QuestHookSpaceError::WordMismatch {
            address: QUEST_HOOK_LAST_INITIALIZED_WORD_RUNTIME,
            expected: LAST_INITIALIZED_WORD,
            actual: actual_last_word,
        });
    }
    let tail = text_slice(
        map,
        text,
        QUEST_HOOK_INITIALIZED_END_RUNTIME,
        usize::try_from(
            QUEST_HOOK_TEXT_END_RUNTIME
                .checked_sub(QUEST_HOOK_INITIALIZED_END_RUNTIME)
                .ok_or(QuestHookSpaceError::CoordinateOverflow)?,
        )
        .map_err(|_| QuestHookSpaceError::CoordinateOverflow)?,
    )?;
    if let Some((offset, &actual)) = tail.iter().enumerate().find(|(_, byte)| **byte != 0) {
        let address = QUEST_HOOK_INITIALIZED_END_RUNTIME
            .checked_add(
                u32::try_from(offset).map_err(|_| QuestHookSpaceError::CoordinateOverflow)?,
            )
            .ok_or(QuestHookSpaceError::CoordinateOverflow)?;
        return Err(QuestHookSpaceError::NonZeroTail { address, actual });
    }
    let actual_last = text
        .iter()
        .rposition(|byte| *byte != 0)
        .and_then(|offset| map.load_addr.checked_add(u32::try_from(offset).ok()?));
    let expected_last = QUEST_HOOK_INITIALIZED_END_RUNTIME - 1;
    if actual_last != Some(expected_last) {
        return Err(QuestHookSpaceError::LastInitializedBoundary {
            expected: expected_last,
            actual: actual_last,
        });
    }

    verify_startup_clear(map, text)?;
    verify_gp_and_mutable_global(map, text)?;

    let alignment_padding_bytes = usize::try_from(
        QUEST_HOOK_ZERO_CLEAR_START_RUNTIME
            .checked_sub(QUEST_HOOK_INITIALIZED_END_RUNTIME)
            .ok_or(QuestHookSpaceError::CoordinateOverflow)?,
    )
    .map_err(|_| QuestHookSpaceError::CoordinateOverflow)?;
    let zero_cleared_bytes = usize::try_from(
        QUEST_HOOK_ZERO_CLEAR_END_RUNTIME
            .checked_sub(QUEST_HOOK_ZERO_CLEAR_START_RUNTIME)
            .ok_or(QuestHookSpaceError::CoordinateOverflow)?,
    )
    .map_err(|_| QuestHookSpaceError::CoordinateOverflow)?;
    let zero_cleared_text_overlap_bytes = usize::try_from(
        QUEST_HOOK_TEXT_END_RUNTIME
            .checked_sub(QUEST_HOOK_ZERO_CLEAR_START_RUNTIME)
            .ok_or(QuestHookSpaceError::CoordinateOverflow)?,
    )
    .map_err(|_| QuestHookSpaceError::CoordinateOverflow)?;

    Ok(QuestHookSpaceSummary {
        profile: IsaProfile::DECLARED,
        initialized_end_runtime: QUEST_HOOK_INITIALIZED_END_RUNTIME,
        zero_clear_start_runtime: QUEST_HOOK_ZERO_CLEAR_START_RUNTIME,
        zero_clear_end_runtime: QUEST_HOOK_ZERO_CLEAR_END_RUNTIME,
        text_end_runtime: QUEST_HOOK_TEXT_END_RUNTIME,
        alignment_padding_bytes,
        zero_cleared_bytes,
        zero_cleared_text_overlap_bytes,
        gp_runtime: QUEST_HOOK_GP_RUNTIME,
        mutable_selector_runtime: QUEST_HOOK_MUTABLE_SELECTOR_RUNTIME,
        section_zero_start_runtime: SECTION_WORDS[4],
        section_zero_bytes: SECTION_WORDS[5] as usize,
    })
}

#[cfg(test)]
#[path = "quest_hook_space_tests.rs"]
mod tests;
