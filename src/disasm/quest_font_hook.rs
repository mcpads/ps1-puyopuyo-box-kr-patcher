//! QUEST OP21 font page hook 후보의 원본 명령·호출 ABI 경계.
//!
//! 이 모듈은 설치할 code cave나 hook body를 만들지 않는다. 지원 원본의
//! 간접 호출점, 6-word leaf handler, 보존 가능한 delay slot과 원본 tail을
//! 선언 ISA profile에서 연속 해석·재조립해 후보 계약만 고정한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{verify_bytes, InstructionKind as K, IsaProfile, Reg, VerifyError};

pub const QUEST_FONT_HOOK_SITE_RUNTIME: u32 = 0x8002_2F0C;
pub const QUEST_FONT_HOOK_PRESERVED_DELAY_RUNTIME: u32 = 0x8002_2F10;
pub const QUEST_FONT_HOOK_CONTINUATION_RUNTIME: u32 = 0x8002_2F14;
pub const QUEST_FONT_HOOK_HANDLER_END_RUNTIME: u32 = 0x8002_2F24;
pub const QUEST_FONT_HOOK_CALLER_LOAD_RUNTIME: u32 = 0x8002_25D0;
pub const QUEST_FONT_HOOK_CALLER_JALR_RUNTIME: u32 = 0x8002_25D8;
pub const QUEST_FONT_HOOK_CALLER_DELAY_RUNTIME: u32 = 0x8002_25DC;
pub const QUEST_FONT_HOOK_CALLER_RETURN_RUNTIME: u32 = 0x8002_25E0;
pub const QUEST_FONT_HOOK_OVERWRITE_BYTES: usize = 4;
pub const QUEST_FONT_HOOK_HANDLER_BYTES: usize = 24;

/// OP21 entry에 한 word jump를 둘 수 있는 원본 경계 요약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestFontHookSiteSummary {
    pub profile: IsaProfile,
    pub site_runtime: u32,
    pub overwrite_bytes: usize,
    pub preserved_delay_runtime: u32,
    pub continuation_runtime: u32,
    pub handler_words: usize,
    pub caller_jalr_runtime: u32,
    pub caller_return_runtime: u32,
    pub object_register: Reg,
    pub return_register: Reg,
    pub continuation_stream_register: Reg,
    pub continuation_active_register: Reg,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestFontHookSiteError {
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
}

impl fmt::Display for QuestFontHookSiteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(f, "QUEST font hook 검증 대상이 QUEST.EXE가 아님: {exe}")
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                f,
                "QUEST font hook EXE 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::CoordinateOverflow => f.write_str("QUEST font hook 좌표 overflow"),
            Self::RangeOutsideText { start, bytes } => write!(
                f,
                "QUEST font hook 원본 범위 이탈: 0x{start:08X} + {bytes}B"
            ),
            Self::MachineCode(source) => write!(f, "QUEST font hook ISA 검산 실패: {source}"),
            Self::InstructionMismatch {
                address,
                expected,
                actual,
            } => write!(
                f,
                "QUEST font hook 명령 불일치: address=0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
        }
    }
}

impl Error for QuestFontHookSiteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MachineCode(source) => Some(source),
            _ => None,
        }
    }
}

impl From<VerifyError> for QuestFontHookSiteError {
    fn from(value: VerifyError) -> Self {
        Self::MachineCode(value)
    }
}

fn verified_range(
    map: &ExeMap,
    text: &[u8],
    start: u32,
    bytes: usize,
) -> Result<crate::isa::VerifiedMachineCode, QuestFontHookSiteError> {
    let offset = map
        .text_offset(start)
        .ok_or(QuestFontHookSiteError::RangeOutsideText { start, bytes })?;
    let end = offset
        .checked_add(bytes)
        .ok_or(QuestFontHookSiteError::CoordinateOverflow)?;
    let raw = text
        .get(offset..end)
        .ok_or(QuestFontHookSiteError::RangeOutsideText { start, bytes })?;
    verify_bytes(IsaProfile::DECLARED, start, raw).map_err(Into::into)
}

fn expect(
    actual: K,
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestFontHookSiteError> {
    if actual != kind {
        return Err(QuestFontHookSiteError::InstructionMismatch {
            address,
            expected,
            actual,
        });
    }
    Ok(())
}

/// 지원 원본의 OP21 hook 후보 entry와 간접 호출 ABI를 검증한다.
pub fn verify_box_quest_font_hook_site(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestFontHookSiteSummary, QuestFontHookSiteError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestFontHookSiteError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestFontHookSiteError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }

    let handler = verified_range(
        map,
        text,
        QUEST_FONT_HOOK_SITE_RUNTIME,
        QUEST_FONT_HOOK_HANDLER_BYTES,
    )?;
    let handler_expected = [
        (
            QUEST_FONT_HOOK_SITE_RUNTIME,
            "lw v0, 0x1C(a0)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            QUEST_FONT_HOOK_PRESERVED_DELAY_RUNTIME,
            "addiu v1, zero, 1",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            QUEST_FONT_HOOK_CONTINUATION_RUNTIME,
            "sw v1, 0(a0)",
            K::Sw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0,
            },
        ),
        (
            0x8002_2F18,
            "addiu v0, v0, 4",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 4,
            },
        ),
        (0x8002_2F1C, "jr ra", K::Jr { rs: Reg::RA }),
        (
            0x8002_2F20,
            "sw v0, 0x1C(a0)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
    ];
    for (instruction, (address, expected, kind)) in
        handler.instructions().iter().zip(handler_expected)
    {
        if instruction.addr != address {
            return Err(QuestFontHookSiteError::InstructionMismatch {
                address,
                expected,
                actual: instruction.kind,
            });
        }
        expect(instruction.kind, address, expected, kind)?;
    }

    let caller_bytes = usize::try_from(
        QUEST_FONT_HOOK_CALLER_RETURN_RUNTIME
            .checked_add(4)
            .and_then(|end| end.checked_sub(QUEST_FONT_HOOK_CALLER_LOAD_RUNTIME))
            .ok_or(QuestFontHookSiteError::CoordinateOverflow)?,
    )
    .map_err(|_| QuestFontHookSiteError::CoordinateOverflow)?;
    let caller = verified_range(map, text, QUEST_FONT_HOOK_CALLER_LOAD_RUNTIME, caller_bytes)?;
    let caller_expected = [
        (
            QUEST_FONT_HOOK_CALLER_LOAD_RUNTIME,
            "lw v0, 0(v0)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8002_25D4,
            "nop",
            K::Sll {
                rd: Reg::ZERO,
                rt: Reg::ZERO,
                sa: 0,
            },
        ),
        (
            QUEST_FONT_HOOK_CALLER_JALR_RUNTIME,
            "jalr v0",
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        (
            QUEST_FONT_HOOK_CALLER_DELAY_RUNTIME,
            "move a0, s0",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ),
        (
            QUEST_FONT_HOOK_CALLER_RETURN_RUNTIME,
            "lw v0, 0x518(gp)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: 0x518,
            },
        ),
    ];
    for (instruction, (address, expected, kind)) in
        caller.instructions().iter().zip(caller_expected)
    {
        if instruction.addr != address {
            return Err(QuestFontHookSiteError::InstructionMismatch {
                address,
                expected,
                actual: instruction.kind,
            });
        }
        expect(instruction.kind, address, expected, kind)?;
    }

    Ok(QuestFontHookSiteSummary {
        profile: IsaProfile::DECLARED,
        site_runtime: QUEST_FONT_HOOK_SITE_RUNTIME,
        overwrite_bytes: QUEST_FONT_HOOK_OVERWRITE_BYTES,
        preserved_delay_runtime: QUEST_FONT_HOOK_PRESERVED_DELAY_RUNTIME,
        continuation_runtime: QUEST_FONT_HOOK_CONTINUATION_RUNTIME,
        handler_words: handler.len_words(),
        caller_jalr_runtime: QUEST_FONT_HOOK_CALLER_JALR_RUNTIME,
        caller_return_runtime: QUEST_FONT_HOOK_CALLER_RETURN_RUNTIME,
        object_register: Reg::A0,
        return_register: Reg::RA,
        continuation_stream_register: Reg::V0,
        continuation_active_register: Reg::V1,
    })
}

#[cfg(test)]
#[path = "quest_font_hook_tests.rs"]
mod tests;
