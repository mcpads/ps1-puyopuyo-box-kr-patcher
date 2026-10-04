//! `QUEST.EXE`의 `FF0F` input wait 경계 검증.
//!
//! `FF0F`는 입력 대기 state와 indicator 수명만 직접 다룬다. 이 모듈은
//! handler의 4바이트 stream 소비, start/poll helper의 state 변경과 호출
//! 경계를 함께 검증한다. 실제 줄 이동이나 화면 clear는 별도 consumer에
//! 귀속하며 `FF0F`라는 이름만으로 추론하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::linear;
use super::quest_control::{
    QUEST_ADVANCE_LINE_RUNTIME, QUEST_ALLOCATE_OBJECT_RUNTIME, QUEST_CLOSE_TEXT_WINDOW_RUNTIME,
    QUEST_OPEN_TEXT_WINDOW_RUNTIME, QUEST_RELEASE_OBJECT_RUNTIME,
};

pub const QUEST_INPUT_WAIT_HANDLER_RUNTIME: u32 = 0x8003_E218;
pub const QUEST_INPUT_WAIT_START_RUNTIME: u32 = 0x8002_CC18;
pub const QUEST_INPUT_WAIT_POLL_RUNTIME: u32 = 0x8002_CCDC;
pub const QUEST_INPUT_WAIT_INITIAL_COUNT: u8 = 8;
pub const QUEST_INPUT_WAIT_BUTTON_MASK: u16 = 0x00F5;
pub const QUEST_INPUT_WAIT_STATE_OFFSET: i16 = 0x11;
pub const QUEST_INPUT_WAIT_INDICATOR_OFFSET: i16 = 0x64;
pub const QUEST_INPUT_WAIT_STREAM_BYTES: usize = 4;

const QUEST_INPUT_WAIT_START_END_RUNTIME: u32 = QUEST_INPUT_WAIT_POLL_RUNTIME;
const QUEST_INPUT_WAIT_POLL_END_RUNTIME: u32 = 0x8002_CD7C;
const QUEST_INPUT_WAIT_WINDOW_STATE_WRITES: usize = 5;
const QUEST_INPUT_WAIT_DIRECT_CALLS: usize = 2;

const QUEST_INPUT_WAIT_EVIDENCE: &[(u32, &str, K)] = &[
    // FF0F handler는 state index와 인자를 helper에 넘기고 record 4바이트를 소비한다.
    (
        0x8003_E228,
        "lw v1, 0x1C(s0)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::S0,
            offset: 0x1C,
        },
    ),
    (
        0x8003_E22C,
        "lbu a0, 0xDE(s0)",
        K::Lbu {
            rt: Reg::A0,
            base: Reg::S0,
            offset: 0xDE,
        },
    ),
    (
        0x8003_E230,
        "addiu v0, v1, 2",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: 2,
        },
    ),
    (
        0x8003_E234,
        "sw v0, 0x1C(s0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: 0x1C,
        },
    ),
    (
        0x8003_E238,
        "lbu a1, 2(v1)",
        K::Lbu {
            rt: Reg::A1,
            base: Reg::V1,
            offset: 2,
        },
    ),
    (
        0x8003_E23C,
        "jal 0x8002CC18",
        K::Jal {
            target: QUEST_INPUT_WAIT_START_RUNTIME,
        },
    ),
    (
        0x8003_E244,
        "lw v0, 0x1C(s0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: 0x1C,
        },
    ),
    (
        0x8003_E24C,
        "addiu v0, v0, 2",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 2,
        },
    ),
    (
        0x8003_E250,
        "sw v0, 0x1C(s0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: 0x1C,
        },
    ),
    (0x8003_E25C, "jr ra", K::Jr { rs: Reg::RA }),
    // start helper는 state가 0일 때만 8을 쓰고, 인자 1이면 indicator를 만든다.
    (
        0x8002_CC54,
        "lbu v0, 0x11(s0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::S0,
            offset: QUEST_INPUT_WAIT_STATE_OFFSET,
        },
    ),
    (
        0x8002_CC5C,
        "bne v0, zero, 25",
        K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 25,
        },
    ),
    (
        0x8002_CC60,
        "addiu v0, zero, 8",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: QUEST_INPUT_WAIT_INITIAL_COUNT as i16,
        },
    ),
    (
        0x8002_CC64,
        "sb v0, 0x11(s0)",
        K::Sb {
            rt: Reg::V0,
            base: Reg::S0,
            offset: QUEST_INPUT_WAIT_STATE_OFFSET,
        },
    ),
    (
        0x8002_CC68,
        "andi s1, a1, 0xFF",
        K::Andi {
            rt: Reg::S1,
            rs: Reg::A1,
            imm: 0x00FF,
        },
    ),
    (
        0x8002_CC70,
        "bne s1, v0, 20",
        K::Bne {
            rs: Reg::S1,
            rt: Reg::V0,
            offset: 20,
        },
    ),
    (
        0x8002_CC78,
        "lw v0, 0x64(s0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: QUEST_INPUT_WAIT_INDICATOR_OFFSET,
        },
    ),
    (
        0x8002_CC80,
        "bne v0, zero, 16",
        K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 16,
        },
    ),
    (
        0x8002_CC8C,
        "jal 0x80014B14",
        K::Jal {
            target: QUEST_ALLOCATE_OBJECT_RUNTIME,
        },
    ),
    (
        0x8002_CC90,
        "addiu a1, zero, 14",
        K::Addiu {
            rt: Reg::A1,
            rs: Reg::ZERO,
            imm: 14,
        },
    ),
    (
        0x8002_CCC0,
        "sw v0, 0x64(s0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: QUEST_INPUT_WAIT_INDICATOR_OFFSET,
        },
    ),
    (0x8002_CCD4, "jr ra", K::Jr { rs: Reg::RA }),
    // poll helper는 8→1 countdown 뒤 지정 button mask를 기다리고 state와
    // indicator만 해제한다.
    (
        0x8002_CD0C,
        "lbu v1, 0x11(s0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::S0,
            offset: QUEST_INPUT_WAIT_STATE_OFFSET,
        },
    ),
    (
        0x8002_CD18,
        "beq v0, zero, 19",
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 19,
        },
    ),
    (
        0x8002_CD1C,
        "sltiu v0, v0, 2",
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 2,
        },
    ),
    (
        0x8002_CD20,
        "bne v0, zero, 4",
        K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 4,
        },
    ),
    (
        0x8002_CD24,
        "addiu v0, v1, -1",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: -1,
        },
    ),
    (
        0x8002_CD28,
        "sb v0, 0x11(s0)",
        K::Sb {
            rt: Reg::V0,
            base: Reg::S0,
            offset: QUEST_INPUT_WAIT_STATE_OFFSET,
        },
    ),
    (
        0x8002_CD2C,
        "j 0x8002CD6C",
        K::J {
            target: 0x8002_CD6C,
        },
    ),
    (
        0x8002_CD38,
        "lhu v0, -0x2944(v0)",
        K::Lhu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: -0x2944,
        },
    ),
    (
        0x8002_CD40,
        "andi v0, v0, 0xF5",
        K::Andi {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: QUEST_INPUT_WAIT_BUTTON_MASK,
        },
    ),
    (
        0x8002_CD44,
        "beq v0, zero, 9",
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 9,
        },
    ),
    (
        0x8002_CD4C,
        "lw a0, 0x64(s0)",
        K::Lw {
            rt: Reg::A0,
            base: Reg::S0,
            offset: QUEST_INPUT_WAIT_INDICATOR_OFFSET,
        },
    ),
    (
        0x8002_CD54,
        "beq a0, zero, 4",
        K::Beq {
            rs: Reg::A0,
            rt: Reg::ZERO,
            offset: 4,
        },
    ),
    (
        0x8002_CD58,
        "sb zero, 0x11(s0)",
        K::Sb {
            rt: Reg::ZERO,
            base: Reg::S0,
            offset: QUEST_INPUT_WAIT_STATE_OFFSET,
        },
    ),
    (
        0x8002_CD5C,
        "jal 0x80014CD0",
        K::Jal {
            target: QUEST_RELEASE_OBJECT_RUNTIME,
        },
    ),
    (
        0x8002_CD64,
        "sw zero, 0x64(s0)",
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::S0,
            offset: QUEST_INPUT_WAIT_INDICATOR_OFFSET,
        },
    ),
    (
        0x8002_CD68,
        "addu v0, zero, zero",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
    ),
    (0x8002_CD74, "jr ra", K::Jr { rs: Reg::RA }),
];

/// `FF0F`가 직접 소유하는 input wait 동작.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestInputWaitSummary {
    pub handler_runtime: u32,
    pub start_runtime: u32,
    pub poll_runtime: u32,
    pub stream_bytes: usize,
    pub initial_count: u8,
    pub button_mask: u16,
    pub window_state_writes: usize,
    pub cursor_or_window_activation_writes: usize,
    pub known_window_transition_calls: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestInputWaitError {
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
    UnexpectedWindowStateWrite {
        address: u32,
        offset: i16,
    },
    UnexpectedHelperCall {
        address: u32,
        target: u32,
    },
    UnexpectedHelperJump {
        address: u32,
        target: u32,
    },
    UnexpectedIndirectJump(u32),
    EffectCountMismatch {
        expected_writes: usize,
        actual_writes: usize,
        expected_calls: usize,
        actual_calls: usize,
    },
}

impl fmt::Display for QuestInputWaitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(formatter, "QUEST FF0F 검증 대상이 QUEST.EXE가 아님: {exe}")
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                formatter,
                "QUEST.EXE text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText => formatter.write_str("QUEST.EXE text 길이가 4바이트 정렬이 아님"),
            Self::MissingInstruction(address) => write!(
                formatter,
                "QUEST FF0F 명령이 text 밖임: 0x{address:08X}"
            ),
            Self::InstructionMismatch {
                address,
                expected,
                actual,
            } => write!(
                formatter,
                "QUEST FF0F 명령 불일치: address=0x{address:08X}, expected={expected}, actual={actual:?}"
            ),
            Self::UnexpectedWindowStateWrite { address, offset } => write!(
                formatter,
                "QUEST FF0F helper가 input wait 밖 window state를 씀: address=0x{address:08X}, offset={offset:#X}"
            ),
            Self::UnexpectedHelperCall { address, target } => write!(
                formatter,
                "QUEST FF0F helper의 미등록 call: address=0x{address:08X}, target=0x{target:08X}"
            ),
            Self::UnexpectedHelperJump { address, target } => write!(
                formatter,
                "QUEST FF0F helper 밖 direct jump: address=0x{address:08X}, target=0x{target:08X}"
            ),
            Self::UnexpectedIndirectJump(address) => write!(
                formatter,
                "QUEST FF0F helper의 미등록 indirect jump: 0x{address:08X}"
            ),
            Self::EffectCountMismatch {
                expected_writes,
                actual_writes,
                expected_calls,
                actual_calls,
            } => write!(
                formatter,
                "QUEST FF0F helper effect 분모 불일치: writes {actual_writes}/{expected_writes}, calls {actual_calls}/{expected_calls}"
            ),
        }
    }
}

impl Error for QuestInputWaitError {}

fn require(
    instructions: &[Instruction],
    map: &ExeMap,
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestInputWaitError> {
    let offset = map
        .text_offset(address)
        .ok_or(QuestInputWaitError::MissingInstruction(address))?;
    let actual = instructions
        .get(offset / 4)
        .filter(|instruction| instruction.addr == address)
        .ok_or(QuestInputWaitError::MissingInstruction(address))?;
    if actual.kind != kind {
        return Err(QuestInputWaitError::InstructionMismatch {
            address,
            expected,
            actual: actual.kind,
        });
    }
    Ok(())
}

fn is_helper_address(address: u32) -> bool {
    (QUEST_INPUT_WAIT_START_RUNTIME..QUEST_INPUT_WAIT_START_END_RUNTIME).contains(&address)
        || (QUEST_INPUT_WAIT_POLL_RUNTIME..QUEST_INPUT_WAIT_POLL_END_RUNTIME).contains(&address)
}

fn audit_helper_effects(
    instructions: &[Instruction],
) -> Result<(usize, usize), QuestInputWaitError> {
    let mut window_state_writes = 0;
    let mut direct_calls = 0;
    for instruction in instructions
        .iter()
        .filter(|instruction| is_helper_address(instruction.addr))
    {
        match instruction.kind {
            K::Sb {
                base: Reg::S0,
                offset,
                ..
            } if offset == QUEST_INPUT_WAIT_STATE_OFFSET => {
                window_state_writes += 1;
            }
            K::Sw {
                base: Reg::S0,
                offset,
                ..
            } if offset == QUEST_INPUT_WAIT_INDICATOR_OFFSET => {
                window_state_writes += 1;
            }
            K::Sb {
                base: Reg::S0,
                offset,
                ..
            }
            | K::Sh {
                base: Reg::S0,
                offset,
                ..
            }
            | K::Sw {
                base: Reg::S0,
                offset,
                ..
            } => {
                return Err(QuestInputWaitError::UnexpectedWindowStateWrite {
                    address: instruction.addr,
                    offset,
                });
            }
            K::Jal { target }
                if target == QUEST_ALLOCATE_OBJECT_RUNTIME
                    || target == QUEST_RELEASE_OBJECT_RUNTIME =>
            {
                direct_calls += 1;
            }
            K::Jal { target } => {
                return Err(QuestInputWaitError::UnexpectedHelperCall {
                    address: instruction.addr,
                    target,
                });
            }
            K::J { target } if is_helper_address(target) => {}
            K::J { target } => {
                return Err(QuestInputWaitError::UnexpectedHelperJump {
                    address: instruction.addr,
                    target,
                });
            }
            K::Jr { rs: Reg::RA } => {}
            K::Jalr { .. } | K::Jr { .. } => {
                return Err(QuestInputWaitError::UnexpectedIndirectJump(
                    instruction.addr,
                ));
            }
            _ => {}
        }
    }
    Ok((window_state_writes, direct_calls))
}

/// 지원 원본의 `FF0F` handler와 start/poll helper를 함께 검증한다.
pub fn verify_box_quest_input_wait(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestInputWaitSummary, QuestInputWaitError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestInputWaitError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestInputWaitError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }
    let instructions = linear::sweep(map, text).ok_or(QuestInputWaitError::UnalignedText)?;
    for &(address, expected, kind) in QUEST_INPUT_WAIT_EVIDENCE {
        require(&instructions, map, address, expected, kind)?;
    }
    let (window_state_writes, direct_calls) = audit_helper_effects(&instructions)?;
    if window_state_writes != QUEST_INPUT_WAIT_WINDOW_STATE_WRITES
        || direct_calls != QUEST_INPUT_WAIT_DIRECT_CALLS
    {
        return Err(QuestInputWaitError::EffectCountMismatch {
            expected_writes: QUEST_INPUT_WAIT_WINDOW_STATE_WRITES,
            actual_writes: window_state_writes,
            expected_calls: QUEST_INPUT_WAIT_DIRECT_CALLS,
            actual_calls: direct_calls,
        });
    }

    // helper가 직접 쓰는 window state는 +0x11/+0x64뿐이고, direct call은
    // indicator allocate/release뿐이다. 알려진 line/open/close helper는 없다.
    let known_window_transition_calls = instructions
        .iter()
        .filter(|instruction| is_helper_address(instruction.addr))
        .filter(|instruction| {
            matches!(
                instruction.kind,
                K::Jal { target }
                    if target == QUEST_ADVANCE_LINE_RUNTIME
                        || target == QUEST_OPEN_TEXT_WINDOW_RUNTIME
                        || target == QUEST_CLOSE_TEXT_WINDOW_RUNTIME
            )
        })
        .count();

    Ok(QuestInputWaitSummary {
        handler_runtime: QUEST_INPUT_WAIT_HANDLER_RUNTIME,
        start_runtime: QUEST_INPUT_WAIT_START_RUNTIME,
        poll_runtime: QUEST_INPUT_WAIT_POLL_RUNTIME,
        stream_bytes: QUEST_INPUT_WAIT_STREAM_BYTES,
        initial_count: QUEST_INPUT_WAIT_INITIAL_COUNT,
        button_mask: QUEST_INPUT_WAIT_BUTTON_MASK,
        window_state_writes,
        cursor_or_window_activation_writes: 0,
        known_window_transition_calls,
    })
}

#[cfg(test)]
#[path = "quest_input_wait_tests.rs"]
mod tests;
