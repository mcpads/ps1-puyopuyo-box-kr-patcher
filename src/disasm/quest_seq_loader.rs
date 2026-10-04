//! QUEST SEQ의 resource 선택, staging 적재와 active buffer 활성화 경로.
//!
//! `RPG_SEQ` resource ID는 descriptor root에 36바이트 stride로 적용되고,
//! 공용 loader는 선택 결과를 고정 staging에 적재한다. 이후 word copy가
//! staging을 고정 active buffer로 옮기며 두 초기화 분기는 그 active base를
//! interpreter stream으로 설정한다. 어느 resource ID가 실제 실행에서
//! 선택되는지는 이 정적 검증의 주장에 포함하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{verify_bytes, InstructionKind as K, IsaProfile, Reg, VerifyError};
use crate::script::QUEST_SEQ_ACTIVE_RUNTIME_BASE;

pub const QUEST_SEQ_DESCRIPTOR_SLOT_RUNTIME: u32 =
    super::quest_seq::QUEST_SEQ_DESCRIPTOR_SLOT_RUNTIME;
pub const QUEST_SEQ_DESCRIPTOR_STRIDE: u32 = 36;
pub const QUEST_SEQ_RESOURCE_ID_STATE_OFFSET: i16 = 2;
pub const QUEST_SEQ_STAGING_RUNTIME: u32 = 0x801D_1270;
pub const QUEST_SEQ_ACTIVE_RUNTIME: u32 = QUEST_SEQ_ACTIVE_RUNTIME_BASE;
pub const QUEST_SEQ_LOAD_RUNTIME: u32 = super::quest_seq::QUEST_OP85_GRP_LOADER_RUNTIME;
pub const QUEST_SEQ_WORD_COPY_RUNTIME: u32 = 0x8007_E618;
pub const QUEST_SEQ_ACTIVE_COPY_WORDS: u32 = 0x2000;
pub const QUEST_SEQ_STREAM_POINTER_STATE_OFFSET: i16 =
    super::quest_seq::QUEST_SEQ_STREAM_POINTER_STATE_OFFSET;

const VERIFIED_RANGES: [(u32, usize); 6] = [
    (0x8007_1050, 0x20),
    (0x8007_1418, 0x18),
    (0x8007_3C68, 0x24),
    (0x8007_3CDC, 0x2C),
    (0x8007_3D8C, 0x18),
    (0x8007_E618, 0x28),
];

/// 지원 원본에서 검증한 공용 SEQ active-base 결합.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqLoaderSummary {
    pub profile: IsaProfile,
    pub descriptor_slot_runtime: u32,
    pub descriptor_stride: u32,
    pub resource_id_state_offset: i16,
    pub staging_runtime: u32,
    pub active_runtime: u32,
    pub loader_runtime: u32,
    pub copy_runtime: u32,
    pub active_copy_words: u32,
    pub stream_entry_sites: [u32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestSeqLoaderError {
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

impl fmt::Display for QuestSeqLoaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(f, "QUEST SEQ loader 검증 대상이 QUEST.EXE가 아님: {exe}")
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                f,
                "QUEST SEQ loader EXE 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::CoordinateOverflow => f.write_str("QUEST SEQ loader 좌표 overflow"),
            Self::RangeOutsideText { start, bytes } => write!(
                f,
                "QUEST SEQ loader 원본 범위 이탈: 0x{start:08X} + {bytes}B"
            ),
            Self::MachineCode(source) => write!(f, "QUEST SEQ loader ISA 검산 실패: {source}"),
            Self::InstructionMismatch {
                address,
                expected,
                actual,
            } => write!(
                f,
                "QUEST SEQ loader 명령 불일치: address=0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
        }
    }
}

impl Error for QuestSeqLoaderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MachineCode(source) => Some(source),
            _ => None,
        }
    }
}

impl From<VerifyError> for QuestSeqLoaderError {
    fn from(value: VerifyError) -> Self {
        Self::MachineCode(value)
    }
}

fn original_range<'a>(
    map: &ExeMap,
    text: &'a [u8],
    start: u32,
    bytes: usize,
) -> Result<&'a [u8], QuestSeqLoaderError> {
    let offset = map
        .text_offset(start)
        .ok_or(QuestSeqLoaderError::RangeOutsideText { start, bytes })?;
    let end = offset
        .checked_add(bytes)
        .ok_or(QuestSeqLoaderError::CoordinateOverflow)?;
    text.get(offset..end)
        .ok_or(QuestSeqLoaderError::RangeOutsideText { start, bytes })
}

fn expect_instruction(
    map: &ExeMap,
    text: &[u8],
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestSeqLoaderError> {
    let raw = original_range(map, text, address, 4)?;
    let verified = verify_bytes(IsaProfile::DECLARED, address, raw)?;
    let actual = verified.instructions()[0].kind;
    if actual != kind {
        return Err(QuestSeqLoaderError::InstructionMismatch {
            address,
            expected,
            actual,
        });
    }
    Ok(())
}

/// 지원 원본의 공용 SEQ resource 선택부터 active stream 설정까지 검증한다.
pub fn verify_box_quest_seq_loader(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqLoaderSummary, QuestSeqLoaderError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestSeqLoaderError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestSeqLoaderError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }
    for (start, bytes) in VERIFIED_RANGES {
        verify_bytes(
            IsaProfile::DECLARED,
            start,
            original_range(map, text, start, bytes)?,
        )?;
    }

    let expected = [
        (
            0x8007_1050,
            "lui a1, 0x801D",
            K::Lui {
                rt: Reg::A1,
                imm: 0x801D,
            },
        ),
        (
            0x8007_1054,
            "ori a1, a1, 0x1270",
            K::Ori {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x1270,
            },
        ),
        (
            0x8007_1060,
            "lhu v1, 2(s0)",
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S0,
                offset: QUEST_SEQ_RESOURCE_ID_STATE_OFFSET,
            },
        ),
        (
            0x8007_1064,
            "lw a0, -0x4CD4(a0)",
            K::Lw {
                rt: Reg::A0,
                base: Reg::A0,
                offset: -0x4CD4,
            },
        ),
        (
            0x8007_1068,
            "j 0x80071418",
            K::J {
                target: 0x8007_1418,
            },
        ),
        (
            0x8007_106C,
            "addiu a2, zero, 0x97",
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: 0x97,
            },
        ),
        (
            0x8007_1418,
            "sll v0, v1, 3",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V1,
                sa: 3,
            },
        ),
        (
            0x8007_141C,
            "addu v0, v0, v1",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8007_1420,
            "sll v0, v0, 2",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8007_1424,
            "addu a0, a0, v0",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A0,
                rt: Reg::V0,
            },
        ),
        (
            0x8007_1428,
            "jal 0x8001B1D4",
            K::Jal {
                target: QUEST_SEQ_LOAD_RUNTIME,
            },
        ),
        (
            0x8007_142C,
            "sw a0, 0x28(s0)",
            K::Sw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x28,
            },
        ),
        (
            0x8007_3C68,
            "lui a0, 0x8019",
            K::Lui {
                rt: Reg::A0,
                imm: 0x8019,
            },
        ),
        (
            0x8007_3C74,
            "ori a0, a0, 0x9A70",
            K::Ori {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x9A70,
            },
        ),
        (
            0x8007_3C88,
            "sw a0, 0x1C(s1)",
            K::Sw {
                rt: Reg::A0,
                base: Reg(17),
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_3CDC,
            "lui a1, 0x8019",
            K::Lui {
                rt: Reg::A1,
                imm: 0x8019,
            },
        ),
        (
            0x8007_3CF4,
            "ori a1, a1, 0x9A70",
            K::Ori {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x9A70,
            },
        ),
        (
            0x8007_3D04,
            "sw a1, 0x1C(s1)",
            K::Sw {
                rt: Reg::A1,
                base: Reg(17),
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_3D8C,
            "lui a0, 0x8019",
            K::Lui {
                rt: Reg::A0,
                imm: 0x8019,
            },
        ),
        (
            0x8007_3D90,
            "ori a0, a0, 0x9A70",
            K::Ori {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x9A70,
            },
        ),
        (
            0x8007_3D94,
            "lui a1, 0x801D",
            K::Lui {
                rt: Reg::A1,
                imm: 0x801D,
            },
        ),
        (
            0x8007_3D98,
            "ori a1, a1, 0x1270",
            K::Ori {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x1270,
            },
        ),
        (
            0x8007_3D9C,
            "jal 0x8007E618",
            K::Jal {
                target: QUEST_SEQ_WORD_COPY_RUNTIME,
            },
        ),
        (
            0x8007_3DA0,
            "addiu a2, zero, 0x2000",
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: QUEST_SEQ_ACTIVE_COPY_WORDS as i16,
            },
        ),
        (
            0x8007_E618,
            "beq a2, zero, return",
            K::Beq {
                rs: Reg::A2,
                rt: Reg::ZERO,
                offset: 7,
            },
        ),
        (
            0x8007_E620,
            "lw v0, 0(a1)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 0,
            },
        ),
        (
            0x8007_E62C,
            "sw v0, 0(a0)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0,
            },
        ),
        (
            0x8007_E630,
            "bne a2, zero, loop",
            K::Bne {
                rs: Reg::A2,
                rt: Reg::ZERO,
                offset: -5,
            },
        ),
    ];
    for (address, description, kind) in expected {
        expect_instruction(map, text, address, description, kind)?;
    }

    Ok(QuestSeqLoaderSummary {
        profile: IsaProfile::DECLARED,
        descriptor_slot_runtime: QUEST_SEQ_DESCRIPTOR_SLOT_RUNTIME,
        descriptor_stride: QUEST_SEQ_DESCRIPTOR_STRIDE,
        resource_id_state_offset: QUEST_SEQ_RESOURCE_ID_STATE_OFFSET,
        staging_runtime: QUEST_SEQ_STAGING_RUNTIME,
        active_runtime: QUEST_SEQ_ACTIVE_RUNTIME,
        loader_runtime: QUEST_SEQ_LOAD_RUNTIME,
        copy_runtime: QUEST_SEQ_WORD_COPY_RUNTIME,
        active_copy_words: QUEST_SEQ_ACTIVE_COPY_WORDS,
        stream_entry_sites: [0x8007_3C88, 0x8007_3D04],
    })
}

#[cfg(test)]
#[path = "quest_seq_loader_tests.rs"]
mod tests;
