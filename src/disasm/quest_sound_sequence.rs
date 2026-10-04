//! QUEST sound sequence의 적재, slot 등록, 재생 소비 경로.
//!
//! sound VDD의 `_01WAKU.SEQ` descriptor에서 시작해 공용 loader, sequence
//! open, application slot 등록, 초기 start wrapper까지 지원 원본의 연속
//! 명령 범위를 선언 ISA profile로 검산한다. 런타임에서 실제로 어느 sequence가
//! 진행했는지는 별도 runtime verifier가 판정한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{verify_bytes, InstructionKind as K, IsaProfile, Reg, VerifyError};

pub const QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME: u32 = 0x800B_4C90;
pub const QUEST_WAKEUP_SEQUENCE_NAME_RUNTIME: u32 = 0x8001_339C;
pub const QUEST_WAKEUP_SEQUENCE_BYTES: usize = 0x5B28;
pub const QUEST_SOUND_LOAD_DESTINATION_RUNTIME: u32 = 0x8013_9200;
pub const QUEST_SOUND_STAGING_RUNTIME: u32 = 0x8018_8E00;
pub const QUEST_SOUND_LOADER_STATE_RUNTIME: u32 = 0x800F_C0B0;
pub const QUEST_SOUND_APPLICATION_SLOT_RUNTIME: u32 = 0x800F_C110;
pub const QUEST_SOUND_APPLICATION_SLOT_STRIDE: usize = 0x20;
pub const QUEST_SOUND_SEQUENCE_POINTER_TABLE_RUNTIME: u32 = 0x8010_6260;
pub const QUEST_SOUND_SEQUENCE_STATE_RUNTIME: u32 = 0x800F_D558;

const WAKEUP_SEQUENCE_NAME: &[u8] = b"_01WAKU.SEQ\0";
const WAKEUP_SEQUENCE_DESCRIPTOR: [u8; 32] = [
    0x00, 0x00, 0x01, 0x00, 0x9C, 0x33, 0x01, 0x80, 0x28, 0x5B, 0x00, 0x00, 0x28, 0x5B, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const VERIFIED_RANGES: [(u32, usize); 6] = [
    (0x8007_B554, 0x38),
    (0x8007_B63C, 0x18),
    (0x8007_B6B8, 0x98),
    (0x8007_BD58, 0x3C),
    (0x8007_D5DC, 0x10),
    (0x800A_08C8, 0x28),
];

/// 원본에서 고정한 QUEST sound sequence 소비 경로.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSoundSequenceConsumerSummary {
    pub profile: IsaProfile,
    pub sequence_descriptor_runtime: u32,
    pub sequence_name_runtime: u32,
    pub sequence_bytes: usize,
    pub load_destination_runtime: u32,
    pub loader_function_runtime: u32,
    pub sequence_open_runtime: u32,
    pub application_slot_runtime: u32,
    pub application_slot_stride: usize,
    pub start_controller_runtime: u32,
    pub sequence_start_runtime: u32,
    pub low_level_start_wrapper_runtime: u32,
    pub low_level_start_core_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestSoundSequenceConsumerError {
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
    OriginalBytesMismatch {
        runtime: u32,
        expected: Vec<u8>,
        actual: Vec<u8>,
    },
    MachineCode(VerifyError),
    InstructionMismatch {
        address: u32,
        expected: &'static str,
        actual: K,
    },
}

impl fmt::Display for QuestSoundSequenceConsumerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(
                    f,
                    "QUEST sound sequence 검증 대상이 QUEST.EXE가 아님: {exe}"
                )
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                f,
                "QUEST sound sequence EXE 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::CoordinateOverflow => f.write_str("QUEST sound sequence 좌표 overflow"),
            Self::RangeOutsideText { start, bytes } => write!(
                f,
                "QUEST sound sequence 원본 범위 이탈: 0x{start:08X} + {bytes}B"
            ),
            Self::OriginalBytesMismatch {
                runtime,
                expected,
                actual,
            } => write!(
                f,
                "QUEST sound sequence 원본 바이트 불일치: runtime=0x{runtime:08X}, \
                 expected={expected:02X?}, actual={actual:02X?}"
            ),
            Self::MachineCode(source) => {
                write!(f, "QUEST sound sequence ISA 검산 실패: {source}")
            }
            Self::InstructionMismatch {
                address,
                expected,
                actual,
            } => write!(
                f,
                "QUEST sound sequence 명령 불일치: address=0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
        }
    }
}

impl Error for QuestSoundSequenceConsumerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MachineCode(source) => Some(source),
            _ => None,
        }
    }
}

impl From<VerifyError> for QuestSoundSequenceConsumerError {
    fn from(value: VerifyError) -> Self {
        Self::MachineCode(value)
    }
}

fn original_range<'a>(
    map: &ExeMap,
    text: &'a [u8],
    start: u32,
    bytes: usize,
) -> Result<&'a [u8], QuestSoundSequenceConsumerError> {
    let offset = map
        .text_offset(start)
        .ok_or(QuestSoundSequenceConsumerError::RangeOutsideText { start, bytes })?;
    let end = offset
        .checked_add(bytes)
        .ok_or(QuestSoundSequenceConsumerError::CoordinateOverflow)?;
    text.get(offset..end)
        .ok_or(QuestSoundSequenceConsumerError::RangeOutsideText { start, bytes })
}

fn expect_bytes(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: &[u8],
) -> Result<(), QuestSoundSequenceConsumerError> {
    let actual = original_range(map, text, runtime, expected.len())?;
    if actual != expected {
        return Err(QuestSoundSequenceConsumerError::OriginalBytesMismatch {
            runtime,
            expected: expected.to_vec(),
            actual: actual.to_vec(),
        });
    }
    Ok(())
}

fn expect_instruction(
    map: &ExeMap,
    text: &[u8],
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestSoundSequenceConsumerError> {
    let raw = original_range(map, text, address, 4)?;
    let verified = verify_bytes(IsaProfile::DECLARED, address, raw)?;
    let actual = verified.instructions()[0].kind;
    if actual != kind {
        return Err(QuestSoundSequenceConsumerError::InstructionMismatch {
            address,
            expected,
            actual,
        });
    }
    Ok(())
}

/// 지원 원본의 `_01WAKU.SEQ` descriptor부터 initial start까지 검증한다.
pub fn verify_box_quest_sound_sequence_consumer(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSoundSequenceConsumerSummary, QuestSoundSequenceConsumerError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestSoundSequenceConsumerError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestSoundSequenceConsumerError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }

    expect_bytes(
        map,
        text,
        QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME,
        &WAKEUP_SEQUENCE_DESCRIPTOR,
    )?;
    expect_bytes(
        map,
        text,
        QUEST_WAKEUP_SEQUENCE_NAME_RUNTIME,
        WAKEUP_SEQUENCE_NAME,
    )?;
    for (start, bytes) in VERIFIED_RANGES {
        verify_bytes(
            IsaProfile::DECLARED,
            start,
            original_range(map, text, start, bytes)?,
        )?;
    }

    let expected = [
        (
            0x8007_B554,
            "lui a1, 0x8013",
            K::Lui {
                rt: Reg::A1,
                imm: 0x8013,
            },
        ),
        (
            0x8007_B570,
            "ori a1, a1, 0x9200",
            K::Ori {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x9200,
            },
        ),
        (
            0x8007_B574,
            "lw a0, 8(s0)",
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 8,
            },
        ),
        (
            0x8007_B578,
            "jal 0x8001B12C",
            K::Jal {
                target: 0x8001_B12C,
            },
        ),
        (
            0x8007_B57C,
            "addiu a2, zero, 129",
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: 129,
            },
        ),
        (
            0x8007_B640,
            "lw a0, 12(s0)",
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 12,
            },
        ),
        (
            0x8007_B644,
            "lh a1, -0x36FA(v0)",
            K::Lh {
                rt: Reg::A1,
                base: Reg::V0,
                offset: -0x36FA,
            },
        ),
        (
            0x8007_B648,
            "jal 0x8009E348",
            K::Jal {
                target: 0x8009_E348,
            },
        ),
        (
            0x8007_B650,
            "sh v0, 16(s0)",
            K::Sh {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 16,
            },
        ),
        (
            0x8007_B6B8,
            "addiu a2, a2, -0x3EF0",
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::A2,
                imm: -0x3EF0,
            },
        ),
        (
            0x8007_B6C4,
            "sll v0, v0, 5",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 5,
            },
        ),
        (
            0x8007_B6CC,
            "sh v1, 0(v0)",
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8007_B6E0,
            "sh v1, 2(v0)",
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 2,
            },
        ),
        (
            0x8007_B6F4,
            "sw v1, 12(v0)",
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 12,
            },
        ),
        (
            0x8007_B708,
            "sh v1, 16(v0)",
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 16,
            },
        ),
        (
            0x8007_B74C,
            "sw a1, 28(v0)",
            K::Sw {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 28,
            },
        ),
        (
            0x8007_BD58,
            "lbu v0, 18(s0)",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 18,
            },
        ),
        (
            0x8007_BD60,
            "andi v0, v0, 1",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
        ),
        (
            0x8007_BD7C,
            "jal 0x8007D534",
            K::Jal {
                target: 0x8007_D534,
            },
        ),
        (
            0x8007_BD8C,
            "xori v0, v0, 3",
            K::Xori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 3,
            },
        ),
        (
            0x8007_BD90,
            "sb v0, 18(s0)",
            K::Sb {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 18,
            },
        ),
        (
            0x8007_D5DC,
            "addiu a1, zero, 1",
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: 1,
            },
        ),
        (
            0x8007_D5E0,
            "lh a0, 2(v1)",
            K::Lh {
                rt: Reg::A0,
                base: Reg::V1,
                offset: 2,
            },
        ),
        (
            0x8007_D5E4,
            "jal 0x800A08C8",
            K::Jal {
                target: 0x800A_08C8,
            },
        ),
        (
            0x8007_D5E8,
            "addu a2, a1, zero",
            K::Addu {
                rd: Reg::A2,
                rs: Reg::A1,
                rt: Reg::ZERO,
            },
        ),
        (
            0x800A_08E0,
            "addu a1, zero, zero",
            K::Addu {
                rd: Reg::A1,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            0x800A_08E8,
            "jal 0x800A0A18",
            K::Jal {
                target: 0x800A_0A18,
            },
        ),
    ];
    for (address, description, kind) in expected {
        expect_instruction(map, text, address, description, kind)?;
    }

    Ok(QuestSoundSequenceConsumerSummary {
        profile: IsaProfile::DECLARED,
        sequence_descriptor_runtime: QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME,
        sequence_name_runtime: QUEST_WAKEUP_SEQUENCE_NAME_RUNTIME,
        sequence_bytes: QUEST_WAKEUP_SEQUENCE_BYTES,
        load_destination_runtime: QUEST_SOUND_LOAD_DESTINATION_RUNTIME,
        loader_function_runtime: 0x8001_B12C,
        sequence_open_runtime: 0x8009_E348,
        application_slot_runtime: QUEST_SOUND_APPLICATION_SLOT_RUNTIME,
        application_slot_stride: QUEST_SOUND_APPLICATION_SLOT_STRIDE,
        start_controller_runtime: 0x8007_BD58,
        sequence_start_runtime: 0x8007_D5E4,
        low_level_start_wrapper_runtime: 0x800A_08C8,
        low_level_start_core_runtime: 0x800A_0A18,
    })
}

#[cfg(test)]
#[path = "quest_sound_sequence_tests.rs"]
mod tests;
