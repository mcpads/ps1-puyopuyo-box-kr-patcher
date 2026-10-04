//! 직접 고정 증가식이 없는 QUEST SEQ opcode의 stream 전이 의미.
//!
//! OP84/8A/AD/C0은 현재 stream에 상수를 더하지 않는다. 각각 저장 stream
//! 복원, 선택한 SEQ resource의 active stream 전환, interpreter 해제,
//! preset stream interpreter 생성을 수행하는 원본 소비 명령을 검증한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::{quest_seq_opcode_handler, QUEST_SEQ_ACTIVE_RUNTIME_BASE};

use super::linear;
use super::quest_seq::{QUEST_SEQ_DEFAULT_RESOURCE_ID_RUNTIME, QUEST_SEQ_DESCRIPTOR_SLOT_RUNTIME};
use super::quest_seq_advance::QUEST_SEQ_WITHOUT_DIRECT_ADVANCE;
use super::vdd_registration::VDD_DESCRIPTOR_STRIDE;

pub const QUEST_SEQ_PRESET_INTERPRETER_STREAM_RUNTIME: u32 = 0x8009_9888;
pub const QUEST_SEQ_RELEASE_OBJECT_RUNTIME: u32 = 0x8001_4CD0;
pub const QUEST_SEQ_ALLOCATE_OBJECT_RUNTIME: u32 = 0x8001_4B14;
pub const QUEST_SEQ_TRANSITION_RESOURCE_ID_OFFSET: i16 = 2;
pub const QUEST_SEQ_RESOURCE_READY_RUNTIME: u32 = 0x8001_B678;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqNonAdvanceMeaning {
    RestoreSavedObjectStream,
    SwitchToSelectedResourceStream,
    ReleaseInterpreter,
    SpawnPresetInterpreterStream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqNonAdvanceSpec {
    pub opcode: u8,
    pub handler_runtime: u32,
    pub meaning: QuestSeqNonAdvanceMeaning,
}

pub const QUEST_SEQ_NON_ADVANCE_SPECS: [QuestSeqNonAdvanceSpec; 4] = [
    QuestSeqNonAdvanceSpec {
        opcode: 0x84,
        handler_runtime: 0x8007_81D8,
        meaning: QuestSeqNonAdvanceMeaning::RestoreSavedObjectStream,
    },
    QuestSeqNonAdvanceSpec {
        opcode: 0x8A,
        handler_runtime: 0x8007_3A9C,
        meaning: QuestSeqNonAdvanceMeaning::SwitchToSelectedResourceStream,
    },
    QuestSeqNonAdvanceSpec {
        opcode: 0xAD,
        handler_runtime: 0x8007_8DC8,
        meaning: QuestSeqNonAdvanceMeaning::ReleaseInterpreter,
    },
    QuestSeqNonAdvanceSpec {
        opcode: 0xC0,
        handler_runtime: 0x8007_66B8,
        meaning: QuestSeqNonAdvanceMeaning::SpawnPresetInterpreterStream,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqNonAdvanceSummary {
    pub meanings: usize,
    pub active_stream_runtime: u32,
    pub seq_descriptor_slot_runtime: u32,
    pub descriptor_stride: u32,
    pub default_resource_id_runtime: u32,
    pub resource_ready_runtime: u32,
    pub preset_stream_runtime: u32,
    pub release_object_runtime: u32,
    pub allocate_object_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestSeqNonAdvanceError {
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

impl fmt::Display for QuestSeqNonAdvanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(f, "SEQ 비증가 전이 검증 대상이 QUEST.EXE가 아님: {exe}")
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
                "SEQ OP{opcode:02X} 비증가 전이 handler 불일치: \
                 expected=0x{expected_handler:08X}, actual={actual_handler:?}"
            ),
            Self::MissingInstruction(address) => {
                write!(f, "SEQ 비증가 전이 명령이 text 밖임: 0x{address:08X}")
            }
            Self::InstructionMismatch {
                opcode,
                address,
                expected,
                actual,
            } => write!(
                f,
                "SEQ OP{opcode:02X} 비증가 전이 명령 불일치: address=0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
        }
    }
}

impl Error for QuestSeqNonAdvanceError {}

pub fn verify_box_quest_seq_non_advance_meanings(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqNonAdvanceSummary, QuestSeqNonAdvanceError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestSeqNonAdvanceError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestSeqNonAdvanceError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }
    let instructions = linear::sweep(map, text).ok_or(QuestSeqNonAdvanceError::UnalignedText)?;

    for (index, spec) in QUEST_SEQ_NON_ADVANCE_SPECS.iter().enumerate() {
        if spec.opcode != QUEST_SEQ_WITHOUT_DIRECT_ADVANCE[index]
            || quest_seq_opcode_handler(spec.opcode) != Some(spec.handler_runtime)
        {
            return Err(QuestSeqNonAdvanceError::SpecMismatch {
                opcode: spec.opcode,
                expected_handler: spec.handler_runtime,
                actual_handler: quest_seq_opcode_handler(spec.opcode),
            });
        }
    }

    verify_op84(&instructions)?;
    verify_op8a(&instructions)?;
    verify_opad(&instructions)?;
    verify_opc0(&instructions)?;

    Ok(QuestSeqNonAdvanceSummary {
        meanings: QUEST_SEQ_NON_ADVANCE_SPECS.len(),
        active_stream_runtime: QUEST_SEQ_ACTIVE_RUNTIME_BASE,
        seq_descriptor_slot_runtime: QUEST_SEQ_DESCRIPTOR_SLOT_RUNTIME,
        descriptor_stride: VDD_DESCRIPTOR_STRIDE,
        default_resource_id_runtime: QUEST_SEQ_DEFAULT_RESOURCE_ID_RUNTIME,
        resource_ready_runtime: QUEST_SEQ_RESOURCE_READY_RUNTIME,
        preset_stream_runtime: QUEST_SEQ_PRESET_INTERPRETER_STREAM_RUNTIME,
        release_object_runtime: QUEST_SEQ_RELEASE_OBJECT_RUNTIME,
        allocate_object_runtime: QUEST_SEQ_ALLOCATE_OBJECT_RUNTIME,
    })
}

fn verify_op84(instructions: &[Instruction]) -> Result<(), QuestSeqNonAdvanceError> {
    require(
        instructions,
        0x84,
        0x8007_81D8,
        "lw v1, 0xD4(a0)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0xD4,
        },
    )?;
    require(
        instructions,
        0x84,
        0x8007_81F4,
        "sw v1, 0x1C(a0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0x1C,
        },
    )
}

fn verify_op8a(instructions: &[Instruction]) -> Result<(), QuestSeqNonAdvanceError> {
    require(
        instructions,
        0x8A,
        0x8007_3AB4,
        "lw a0, 0x1C(s1)",
        K::Lw {
            rt: Reg::A0,
            base: Reg::S1,
            offset: 0x1C,
        },
    )?;
    require(
        instructions,
        0x8A,
        0x8007_3ABC,
        "lhu s2, 2(a0)",
        K::Lhu {
            rt: Reg::S2,
            base: Reg::A0,
            offset: QUEST_SEQ_TRANSITION_RESOURCE_ID_OFFSET,
        },
    )?;
    // record ID가 0이 아니면 기본값 load를 건너뛰고, 0일 때만 공용
    // runtime 기본 SEQ ID를 s2에 넣는다. delay slot의 lui는 양쪽에서 돈다.
    for (address, expected, kind) in [
        (
            0x8007_3AC4,
            "bne s2, zero, keep record resource ID",
            K::Bne {
                rs: Reg::S2,
                rt: Reg::ZERO,
                offset: 2,
            },
        ),
        (
            0x8007_3AC8,
            "lui v0, 0x8010",
            K::Lui {
                rt: Reg::V0,
                imm: 0x8010,
            },
        ),
        (
            0x8007_3ACC,
            "lhu s2, default resource ID",
            K::Lhu {
                rt: Reg::S2,
                base: Reg::V0,
                offset: (QUEST_SEQ_DEFAULT_RESOURCE_ID_RUNTIME as u16) as i16,
            },
        ),
    ] {
        require(instructions, 0x8A, address, expected, kind)?;
    }
    // OP8A record +2의 ID를 9배한 뒤 4배해 VDD 36바이트 descriptor
    // stride로 바꾸고, RPG_SEQ slot의 선택 descriptor 상태를 확인한다.
    for (address, expected, kind) in [
        (
            0x8007_3BF0,
            "lui v0, 0x800E",
            K::Lui {
                rt: Reg::V0,
                imm: 0x800E,
            },
        ),
        (
            0x8007_3BF4,
            "sll a0, s2, 3",
            K::Sll {
                rd: Reg::A0,
                rt: Reg::S2,
                sa: 3,
            },
        ),
        (
            0x8007_3BF8,
            "addu a0, a0, s2",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A0,
                rt: Reg::S2,
            },
        ),
        (
            0x8007_3BFC,
            "lw v0, RPG_SEQ descriptor slot",
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: (QUEST_SEQ_DESCRIPTOR_SLOT_RUNTIME as u16) as i16,
            },
        ),
        (
            0x8007_3C00,
            "sll a0, a0, 2",
            K::Sll {
                rd: Reg::A0,
                rt: Reg::A0,
                sa: 2,
            },
        ),
        (
            0x8007_3C04,
            "jal resource-ready",
            K::Jal {
                target: QUEST_SEQ_RESOURCE_READY_RUNTIME,
            },
        ),
        (
            0x8007_3C08,
            "addu a0, v0, a0",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
    ] {
        require(instructions, 0x8A, address, expected, kind)?;
    }
    for (lui, low, store, value) in [
        (0x8007_3C68, 0x8007_3C74, 0x8007_3C88, Reg::A0),
        (0x8007_3CDC, 0x8007_3CF4, 0x8007_3D04, Reg::A1),
    ] {
        require(
            instructions,
            0x8A,
            lui,
            "lui value, 0x8019",
            K::Lui {
                rt: value,
                imm: 0x8019,
            },
        )?;
        require(
            instructions,
            0x8A,
            low,
            "ori value, value, 0x9A70",
            K::Ori {
                rt: value,
                rs: value,
                imm: u16::try_from(QUEST_SEQ_ACTIVE_RUNTIME_BASE & 0xFFFF)
                    .expect("QUEST SEQ active base low half"),
            },
        )?;
        require(
            instructions,
            0x8A,
            store,
            "sw value, 0x1C(s1)",
            K::Sw {
                rt: value,
                base: Reg::S1,
                offset: 0x1C,
            },
        )?;
    }
    Ok(())
}

fn verify_opad(instructions: &[Instruction]) -> Result<(), QuestSeqNonAdvanceError> {
    require(
        instructions,
        0xAD,
        0x8007_8DD8,
        "lw a0, 0x14(s0)",
        K::Lw {
            rt: Reg::A0,
            base: Reg::S0,
            offset: 0x14,
        },
    )?;
    require(
        instructions,
        0xAD,
        0x8007_8DE8,
        "jal release object",
        K::Jal {
            target: QUEST_SEQ_RELEASE_OBJECT_RUNTIME,
        },
    )?;
    require(
        instructions,
        0xAD,
        0x8007_8DF0,
        "jal release interpreter",
        K::Jal {
            target: QUEST_SEQ_RELEASE_OBJECT_RUNTIME,
        },
    )?;
    require(
        instructions,
        0xAD,
        0x8007_8DF4,
        "move a0, s0",
        K::Addu {
            rd: Reg::A0,
            rs: Reg::S0,
            rt: Reg::ZERO,
        },
    )
}

fn verify_opc0(instructions: &[Instruction]) -> Result<(), QuestSeqNonAdvanceError> {
    require(
        instructions,
        0xC0,
        0x8007_66C4,
        "lh v1, 0xC6(a0)",
        K::Lh {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0xC6,
        },
    )?;
    require(
        instructions,
        0xC0,
        0x8007_67AC,
        "jal allocate object",
        K::Jal {
            target: QUEST_SEQ_ALLOCATE_OBJECT_RUNTIME,
        },
    )?;
    require(
        instructions,
        0xC0,
        0x8007_67B4,
        "lui v1, 0x8009",
        K::Lui {
            rt: Reg::V1,
            imm: 0x8009,
        },
    )?;
    require(
        instructions,
        0xC0,
        0x8007_67B8,
        "addiu v1, v1, -0x6778",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V1,
            imm: -0x6778,
        },
    )?;
    require(
        instructions,
        0xC0,
        0x8007_67BC,
        "sw v1, 0x1C(v0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x1C,
        },
    )
}

fn require(
    instructions: &[Instruction],
    opcode: u8,
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestSeqNonAdvanceError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or(QuestSeqNonAdvanceError::MissingInstruction(address))?;
    if actual.kind != kind {
        return Err(QuestSeqNonAdvanceError::InstructionMismatch {
            opcode,
            address,
            expected,
            actual: actual.kind,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_seq_transition_tests.rs"]
mod tests;
