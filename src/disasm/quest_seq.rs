//! `QUEST.EXE`의 SEQ 내부 포인터 record 소비 형식.
//!
//! bounded runtime read trace가 field `0x1B34`, `0x3618`을
//! `0x80078A80`에서 읽은 사실과, 정상 root index 0 경로가
//! `0x0168/0x016C/0x0174/0x017C`를 각 OP85/88/8F consumer에서 읽은
//! 사실을 원본 명령과 결합한다. 또한 opcode `0x80..0xFF`의 두 table
//! 등록·128개 dispatch word와 17개 opcode의 19개 local record 포인터
//! field, OP89의 inner 관계 field 2개 소비를 함께 검증한다. runtime
//! 소비 확정은 위 여섯 field에 한정하며, 나머지 record 형태는 정적 후보다.

mod pointer_consumers;
mod resource_loading;

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::{
    quest_seq_base_dispatch_fallback_entries, quest_seq_base_dispatch_unique_handlers,
    quest_seq_dispatch_fallback_entries, quest_seq_dispatch_unique_handlers,
    QUEST_SEQ_BASE_DISPATCH_HANDLERS, QUEST_SEQ_DISPATCH_HANDLERS, QUEST_SEQ_POINTER_RECORD_SPECS,
};

use super::linear;

pub const QUEST_SEQ_BRANCH_POINTER_CONSUMER_RUNTIME: u32 = 0x8007_8A80;
pub const QUEST_SEQ_STACK_BRANCH_POINTER_CONSUMER_RUNTIME: u32 = 0x8007_8B04;
pub const QUEST_SEQ_BRANCH_RECORD_POINTER_OFFSET: i16 = 8;
pub const QUEST_SEQ_BRANCH_RECORD_STRIDE: i16 = 12;
pub const QUEST_SEQ_STREAM_POINTER_STATE_OFFSET: i16 = 0x1C;
pub const QUEST_SEQ_INTERPRETER_RUNTIME: u32 = 0x8002_2570;
pub const QUEST_SEQ_BASE_DISPATCH_TABLE_RUNTIME: u32 = 0x800B_1C18;
pub const QUEST_SEQ_BRANCH_DISPATCH_REGISTRATION_RUNTIME: u32 = 0x8007_80A8;
pub const QUEST_SEQ_BRANCH_DISPATCH_TABLE_RUNTIME: u32 = 0x800B_36F0;
pub const QUEST_SEQ_BRANCH_DISPATCH_OPCODE_BASE: u8 = 0x80;
pub const QUEST_SEQ_BRANCH_DISPATCH_OPCODE_COUNT: u8 = 0x60;
pub const QUEST_SEQ_EXTENDED_DISPATCH_REGISTRATION_RUNTIME: u32 = 0x8007_0028;
pub const QUEST_SEQ_EXTENDED_DISPATCH_TABLE_RUNTIME: u32 = 0x800B_3670;
pub const QUEST_SEQ_EXTENDED_DISPATCH_OPCODE_BASE: u8 = 0xE0;
pub const QUEST_SEQ_EXTENDED_DISPATCH_OPCODE_COUNT: u8 = 0x20;
pub const QUEST_SEQ_BRANCH_DISPATCH_REGISTER_RUNTIME: u32 = 0x8002_2608;
pub const QUEST_SEQ_OP85_CONSUMER_RUNTIME: u32 = 0x8007_8280;
pub const QUEST_SEQ_OP85_STATE_RUNTIME: u32 = 0x800F_BDA0;
pub const QUEST_SEQ_DEFAULT_RESOURCE_ID_RUNTIME: u32 = 0x800F_BD90;
pub const QUEST_SEQ_OP85_RECORD_STRIDE: i16 = 20;
pub const QUEST_SEQ_OP85_SELECTOR_STATE_OFFSET: i16 = 0x0E;
pub const QUEST_SEQ_OP85_HALFWORD_STATE_OFFSETS: [i16; 5] = [0x10, 0x12, 0x14, 0x16, 0x18];
pub const QUEST_SEQ_OP85_RENDER_POINTER_STATE_OFFSETS: [i16; 2] = [0x1C, 0x20];
pub const QUEST_RESOURCE_SLOT_TABLE_RUNTIME: u32 = 0x800D_B328;
pub const QUEST_GRP_DESCRIPTOR_ROOT_RUNTIME: u32 = 0x8008_C8F8;
pub const QUEST_SEQ_DESCRIPTOR_ROOT_RUNTIME: u32 = 0x8008_F9D0;
pub const QUEST_MAP_DESCRIPTOR_ROOT_RUNTIME: u32 = 0x8008_DFC4;
pub const QUEST_GRP_DESCRIPTOR_SLOT_RUNTIME: u32 = QUEST_RESOURCE_SLOT_TABLE_RUNTIME;
pub const QUEST_SEQ_DESCRIPTOR_SLOT_RUNTIME: u32 = QUEST_RESOURCE_SLOT_TABLE_RUNTIME + 4;
pub const QUEST_MAP_DESCRIPTOR_SLOT_RUNTIME: u32 = QUEST_RESOURCE_SLOT_TABLE_RUNTIME + 8;
pub const QUEST_OP85_MAP_LOADER_RUNTIME: u32 = 0x8001_B12C;
pub const QUEST_OP85_GRP_LOADER_RUNTIME: u32 = 0x8001_B1D4;
pub const QUEST_OP85_GRP_STAGING_RUNTIME: [u32; 2] = [0x801D_9270, 0x801F_1270];
pub const QUEST_OP85_GRP_COPY_SOURCE_RUNTIME: [u32; 2] = [0x8016_2A00, 0x8017_2A00];
pub const QUEST_OP85_GRP_COPY_WORDS: [u32; 2] = [0x6000, 0x2000];

/// 확인한 분기 포인터 소비 형식.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqPointerConsumerSummary {
    pub interpreter_runtime: u32,
    pub base_dispatch_table_runtime: u32,
    pub base_dispatch_entries: usize,
    pub unique_base_dispatch_handlers: usize,
    pub base_fallback_dispatch_entries: usize,
    pub dispatch_registration_runtime: u32,
    pub dispatch_table_runtime: u32,
    pub dispatch_opcode_base: u8,
    pub dispatch_opcode_count: u8,
    pub extended_dispatch_registration_runtime: u32,
    pub extended_dispatch_table_runtime: u32,
    pub extended_dispatch_opcode_base: u8,
    pub extended_dispatch_opcode_count: u8,
    pub dispatch_entries: usize,
    pub unique_dispatch_handlers: usize,
    pub fallback_dispatch_entries: usize,
    pub consumer_runtime: u32,
    pub stack_consumer_runtime: u32,
    pub record_pointer_offset: i16,
    pub record_stride: i16,
    pub stream_pointer_state_offset: i16,
    pub pointer_record_specs: usize,
    pub pointer_consumer_sites: usize,
    pub prefix_entry_runtime: u32,
    pub op85_consumer_runtime: u32,
    pub op85_state_runtime: u32,
    pub op85_record_stride: i16,
    pub op85_selector_state_offset: i16,
    pub op85_halfword_state_offsets: [i16; 5],
    pub op85_render_pointer_state_offsets: [i16; 2],
    pub grp_descriptor_slot_runtime: u32,
    pub seq_descriptor_slot_runtime: u32,
    pub map_descriptor_slot_runtime: u32,
    pub grp_descriptor_root_runtime: u32,
    pub seq_descriptor_root_runtime: u32,
    pub map_descriptor_root_runtime: u32,
    pub op85_map_loader_runtime: u32,
    pub op85_grp_loader_runtime: u32,
    pub op85_grp_staging_runtime: [u32; 2],
    pub op85_grp_copy_source_runtime: [u32; 2],
    pub op85_grp_copy_words: [u32; 2],
}

/// `QUEST.EXE` 분기 포인터 소비 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestSeqPointerConsumerError {
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
    DataWordMismatch {
        address: u32,
        expected: u32,
        actual: u32,
    },
    UnsupportedPointerOpcode(u8),
}

impl fmt::Display for QuestSeqPointerConsumerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(
                    f,
                    "SEQ 분기 포인터 소비 검증 대상이 QUEST.EXE가 아님: {exe}"
                )
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                f,
                "QUEST.EXE text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText => write!(f, "QUEST.EXE text 길이가 4바이트 정렬이 아님"),
            Self::MissingInstruction(address) => {
                write!(f, "SEQ 분기 포인터 소비 명령이 text 밖임: 0x{address:08X}")
            }
            Self::InstructionMismatch {
                address,
                expected,
                actual,
            } => write!(
                f,
                "SEQ 분기 포인터 소비 명령 불일치: address=0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
            Self::DataWordMismatch {
                address,
                expected,
                actual,
            } => write!(
                f,
                "SEQ 분기 dispatch word 불일치: address=0x{address:08X}, \
                 expected=0x{expected:08X}, actual=0x{actual:08X}"
            ),
            Self::UnsupportedPointerOpcode(opcode) => write!(
                f,
                "SEQ 포인터 record opcode가 등록 table 범위 밖임: 0x{opcode:02X}"
            ),
        }
    }
}

impl Error for QuestSeqPointerConsumerError {}

/// 지원 원본 `QUEST.EXE`에서 조건 분기와 pointer load/store를 함께 검증한다.
pub fn verify_box_quest_seq_pointer_consumer(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqPointerConsumerSummary, QuestSeqPointerConsumerError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestSeqPointerConsumerError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestSeqPointerConsumerError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }
    let instructions =
        linear::sweep(map, text).ok_or(QuestSeqPointerConsumerError::UnalignedText)?;

    for (address, expected, kind) in [
        (
            0x8002_2584,
            "lui v0, 0x800B",
            K::Lui {
                rt: Reg::V0,
                imm: 0x800B,
            },
        ),
        (
            0x8002_258C,
            "addiu s1, v0, 0x1C18",
            K::Addiu {
                rt: Reg::S1,
                rs: Reg::V0,
                imm: 0x1C18,
            },
        ),
        (
            0x8002_25B8,
            "lw v0, 0x1C(s0)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8002_25C0,
            "lbu v0, 0(v0)",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8002_25C8,
            "sll v0, v0, 2",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8002_25CC,
            "addu v0, v0, s1",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::S1,
            },
        ),
        (
            0x8002_25D0,
            "lw v0, 0(v0)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8002_25D8,
            "jalr v0",
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
    ] {
        require(&instructions, map, address, expected, kind)?;
    }
    for (index, &handler_runtime) in QUEST_SEQ_BASE_DISPATCH_HANDLERS.iter().enumerate() {
        require_word(
            text,
            map,
            QUEST_SEQ_BASE_DISPATCH_TABLE_RUNTIME
                + u32::try_from(index).expect("128개 base dispatch index") * 4,
            handler_runtime,
        )?;
    }

    require(
        &instructions,
        map,
        0x8007_80AC,
        "lui a0, 0x800B",
        K::Lui {
            rt: Reg::A0,
            imm: 0x800B,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_80B0,
        "addiu a0, a0, 0x36F0",
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: 0x36F0,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_80B4,
        "addiu a1, zero, 0x80",
        K::Addiu {
            rt: Reg::A1,
            rs: Reg::ZERO,
            imm: i16::from(QUEST_SEQ_BRANCH_DISPATCH_OPCODE_BASE),
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_80BC,
        "jal 0x80022608",
        K::Jal {
            target: QUEST_SEQ_BRANCH_DISPATCH_REGISTER_RUNTIME,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_80C0,
        "addiu a2, zero, 0x60",
        K::Addiu {
            rt: Reg::A2,
            rs: Reg::ZERO,
            imm: i16::from(QUEST_SEQ_BRANCH_DISPATCH_OPCODE_COUNT),
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_002C,
        "lui a0, 0x800B",
        K::Lui {
            rt: Reg::A0,
            imm: 0x800B,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_0030,
        "addiu a0, a0, 0x3670",
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: 0x3670,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_0034,
        "addiu a1, zero, 0xE0",
        K::Addiu {
            rt: Reg::A1,
            rs: Reg::ZERO,
            imm: i16::from(QUEST_SEQ_EXTENDED_DISPATCH_OPCODE_BASE),
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_003C,
        "jal 0x80022608",
        K::Jal {
            target: QUEST_SEQ_BRANCH_DISPATCH_REGISTER_RUNTIME,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_0040,
        "addiu a2, zero, 0x20",
        K::Addiu {
            rt: Reg::A2,
            rs: Reg::ZERO,
            imm: i16::from(QUEST_SEQ_EXTENDED_DISPATCH_OPCODE_COUNT),
        },
    )?;
    for (index, &handler_runtime) in QUEST_SEQ_DISPATCH_HANDLERS.iter().enumerate() {
        let opcode = QUEST_SEQ_BRANCH_DISPATCH_OPCODE_BASE
            + u8::try_from(index).expect("128개 dispatch index");
        require_word(text, map, dispatch_word_runtime(opcode)?, handler_runtime)?;
    }
    for spec in QUEST_SEQ_POINTER_RECORD_SPECS {
        require_word(
            text,
            map,
            dispatch_word_runtime(spec.opcode)?,
            spec.handler_runtime,
        )?;
    }
    require_word(text, map, 0x8001_304C, 0x8007_3594)?;
    require_word(text, map, 0x800B_371C, 0x8007_3DE8)?;
    let resource_loading =
        resource_loading::verify_quest_seq_resource_loading(&instructions, map, text)?;
    verify_additional_pointer_consumers(&instructions, map)?;

    require(
        &instructions,
        map,
        0x8007_8A68,
        "lw a0, 0x1C(s0)",
        K::Lw {
            rt: Reg::A0,
            base: Reg::S0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_8A70,
        "lbu v1, 1(a0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 1,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_8A74,
        "andi v0, v0, 0x00FF",
        K::Andi {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0x00FF,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_8A78,
        "beq v0, v1, 0x80078A8C",
        K::Beq {
            rs: Reg::V0,
            rt: Reg::V1,
            offset: 4,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_8A7C,
        "addiu v0, a0, 12",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::A0,
            imm: QUEST_SEQ_BRANCH_RECORD_STRIDE,
        },
    )?;
    require(
        &instructions,
        map,
        QUEST_SEQ_BRANCH_POINTER_CONSUMER_RUNTIME,
        "lw v0, 8(a0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_BRANCH_RECORD_POINTER_OFFSET,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_8A84,
        "j 0x80078A90",
        K::J {
            target: 0x8007_8A90,
        },
    )?;
    for address in [0x8007_8A88, 0x8007_8A8C] {
        require(
            &instructions,
            map,
            address,
            "sw v0, 0x1C(s0)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        )?;
    }

    require(
        &instructions,
        map,
        0x8007_8AC8,
        "lw a0, 0x1C(s0)",
        K::Lw {
            rt: Reg::A0,
            base: Reg::S0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_8AD0,
        "lbu v1, 1(a0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 1,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_8AD8,
        "beq v0, v1, 0x80078B10",
        K::Beq {
            rs: Reg::V0,
            rt: Reg::V1,
            offset: 13,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_8ADC,
        "addiu v1, a0, 12",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::A0,
            imm: QUEST_SEQ_BRANCH_RECORD_STRIDE,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_8AF0,
        "sw v1, 0x28(v0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x28,
        },
    )?;
    require(
        &instructions,
        map,
        QUEST_SEQ_STACK_BRANCH_POINTER_CONSUMER_RUNTIME,
        "lw v0, 8(v1)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: QUEST_SEQ_BRANCH_RECORD_POINTER_OFFSET,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_8B0C,
        "sw v0, 0x1C(s0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_8B10,
        "addiu v0, a0, 12",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::A0,
            imm: QUEST_SEQ_BRANCH_RECORD_STRIDE,
        },
    )?;
    require(
        &instructions,
        map,
        0x8007_8B14,
        "sw v0, 0x1C(s0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;

    Ok(QuestSeqPointerConsumerSummary {
        interpreter_runtime: QUEST_SEQ_INTERPRETER_RUNTIME,
        base_dispatch_table_runtime: QUEST_SEQ_BASE_DISPATCH_TABLE_RUNTIME,
        base_dispatch_entries: QUEST_SEQ_BASE_DISPATCH_HANDLERS.len(),
        unique_base_dispatch_handlers: quest_seq_base_dispatch_unique_handlers(),
        base_fallback_dispatch_entries: quest_seq_base_dispatch_fallback_entries(),
        dispatch_registration_runtime: QUEST_SEQ_BRANCH_DISPATCH_REGISTRATION_RUNTIME,
        dispatch_table_runtime: QUEST_SEQ_BRANCH_DISPATCH_TABLE_RUNTIME,
        dispatch_opcode_base: QUEST_SEQ_BRANCH_DISPATCH_OPCODE_BASE,
        dispatch_opcode_count: QUEST_SEQ_BRANCH_DISPATCH_OPCODE_COUNT,
        extended_dispatch_registration_runtime: QUEST_SEQ_EXTENDED_DISPATCH_REGISTRATION_RUNTIME,
        extended_dispatch_table_runtime: QUEST_SEQ_EXTENDED_DISPATCH_TABLE_RUNTIME,
        extended_dispatch_opcode_base: QUEST_SEQ_EXTENDED_DISPATCH_OPCODE_BASE,
        extended_dispatch_opcode_count: QUEST_SEQ_EXTENDED_DISPATCH_OPCODE_COUNT,
        dispatch_entries: QUEST_SEQ_DISPATCH_HANDLERS.len(),
        unique_dispatch_handlers: quest_seq_dispatch_unique_handlers(),
        fallback_dispatch_entries: quest_seq_dispatch_fallback_entries(),
        consumer_runtime: QUEST_SEQ_BRANCH_POINTER_CONSUMER_RUNTIME,
        stack_consumer_runtime: QUEST_SEQ_STACK_BRANCH_POINTER_CONSUMER_RUNTIME,
        record_pointer_offset: QUEST_SEQ_BRANCH_RECORD_POINTER_OFFSET,
        record_stride: QUEST_SEQ_BRANCH_RECORD_STRIDE,
        stream_pointer_state_offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        pointer_record_specs: QUEST_SEQ_POINTER_RECORD_SPECS.len(),
        pointer_consumer_sites: QUEST_SEQ_POINTER_RECORD_SPECS.len() + 2,
        prefix_entry_runtime: resource_loading.prefix_entry_runtime,
        op85_consumer_runtime: resource_loading.op85_consumer_runtime,
        op85_state_runtime: resource_loading.op85_state_runtime,
        op85_record_stride: resource_loading.op85_record_stride,
        op85_selector_state_offset: resource_loading.op85_selector_state_offset,
        op85_halfword_state_offsets: resource_loading.op85_halfword_state_offsets,
        op85_render_pointer_state_offsets: resource_loading.op85_render_pointer_state_offsets,
        grp_descriptor_slot_runtime: resource_loading.grp_descriptor_slot_runtime,
        seq_descriptor_slot_runtime: resource_loading.seq_descriptor_slot_runtime,
        map_descriptor_slot_runtime: resource_loading.map_descriptor_slot_runtime,
        grp_descriptor_root_runtime: resource_loading.grp_descriptor_root_runtime,
        seq_descriptor_root_runtime: resource_loading.seq_descriptor_root_runtime,
        map_descriptor_root_runtime: resource_loading.map_descriptor_root_runtime,
        op85_map_loader_runtime: resource_loading.op85_map_loader_runtime,
        op85_grp_loader_runtime: resource_loading.op85_grp_loader_runtime,
        op85_grp_staging_runtime: resource_loading.op85_grp_staging_runtime,
        op85_grp_copy_source_runtime: resource_loading.op85_grp_copy_source_runtime,
        op85_grp_copy_words: resource_loading.op85_grp_copy_words,
    })
}

fn dispatch_word_runtime(opcode: u8) -> Result<u32, QuestSeqPointerConsumerError> {
    if (QUEST_SEQ_BRANCH_DISPATCH_OPCODE_BASE
        ..QUEST_SEQ_BRANCH_DISPATCH_OPCODE_BASE + QUEST_SEQ_BRANCH_DISPATCH_OPCODE_COUNT)
        .contains(&opcode)
    {
        return Ok(QUEST_SEQ_BRANCH_DISPATCH_TABLE_RUNTIME
            + u32::from(opcode - QUEST_SEQ_BRANCH_DISPATCH_OPCODE_BASE) * 4);
    }
    if opcode >= QUEST_SEQ_EXTENDED_DISPATCH_OPCODE_BASE {
        return Ok(QUEST_SEQ_EXTENDED_DISPATCH_TABLE_RUNTIME
            + u32::from(opcode - QUEST_SEQ_EXTENDED_DISPATCH_OPCODE_BASE) * 4);
    }
    Err(QuestSeqPointerConsumerError::UnsupportedPointerOpcode(
        opcode,
    ))
}

fn verify_additional_pointer_consumers(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<(), QuestSeqPointerConsumerError> {
    pointer_consumers::verify_additional_pointer_consumers(instructions, map)
}

fn require_advance_8(
    instructions: &[Instruction],
    map: &ExeMap,
    add_runtime: u32,
    store_runtime: u32,
    state: Reg,
) -> Result<(), QuestSeqPointerConsumerError> {
    require(
        instructions,
        map,
        add_runtime,
        "addiu v0, v0, 8",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 8,
        },
    )?;
    require(
        instructions,
        map,
        store_runtime,
        "sw v0, 0x1C(state)",
        K::Sw {
            rt: Reg::V0,
            base: state,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )
}

fn verify_mask_branch(
    instructions: &[Instruction],
    map: &ExeMap,
    stream_load_runtime: u32,
    load_runtime: u32,
    store_runtime: u32,
    stream: Reg,
) -> Result<(), QuestSeqPointerConsumerError> {
    require(
        instructions,
        map,
        stream_load_runtime,
        "lw stream, 0x1C(a0)",
        K::Lw {
            rt: stream,
            base: Reg::A0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        load_runtime,
        "lw v0, 4(stream)",
        K::Lw {
            rt: Reg::V0,
            base: stream,
            offset: 4,
        },
    )?;
    require(
        instructions,
        map,
        store_runtime,
        "sw v0, 0x1C(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )
}

fn require(
    instructions: &[Instruction],
    map: &ExeMap,
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestSeqPointerConsumerError> {
    let offset = map
        .text_offset(address)
        .ok_or(QuestSeqPointerConsumerError::MissingInstruction(address))?;
    let actual = instructions
        .get(offset / 4)
        .filter(|instruction| instruction.addr == address)
        .ok_or(QuestSeqPointerConsumerError::MissingInstruction(address))?;
    if actual.kind != kind {
        return Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address,
            expected,
            actual: actual.kind,
        });
    }
    Ok(())
}

fn require_word(
    text: &[u8],
    map: &ExeMap,
    address: u32,
    expected: u32,
) -> Result<(), QuestSeqPointerConsumerError> {
    let offset = map
        .text_offset(address)
        .ok_or(QuestSeqPointerConsumerError::MissingInstruction(address))?;
    let word = text
        .get(offset..offset + 4)
        .ok_or(QuestSeqPointerConsumerError::MissingInstruction(address))?;
    let actual = u32::from_le_bytes(
        word.try_into()
            .expect("4바이트 범위를 확인한 dispatch word"),
    );
    if actual != expected {
        return Err(QuestSeqPointerConsumerError::DataWordMismatch {
            address,
            expected,
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_seq_tests.rs"]
mod quest_seq_tests;
