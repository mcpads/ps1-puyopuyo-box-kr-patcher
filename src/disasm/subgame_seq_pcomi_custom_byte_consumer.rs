//! PCOMI custom byte slot에서 직접 cell renderer까지의 정적 소비 경로.
//!
//! `PCOMI1.SEQ`의 root `+0x2C`는 11개 고정 slot의 pointer table을
//! 가리킨다. 이 모듈은 `PCOMI.EXE`가 selector 7에서 그 root를 읽고,
//! 난수 `mod 11`로 slot을 고른 뒤 byte를 임시 buffer에 펼치는 경로를
//! 검산한다. slot 경로가 callback object cell 0, action 좌표 (16, 4), modal
//! state 2와 selector mode 0에서만 열리는 admission도 함께 고정한다. 이어
//! byte를 zero-extend한 u16 stream과 UI text consumer의 direct cell store까지
//! 잇는다.
//!
//! 이 경로는 generic SEQ glyph consumer의 `code + 0x96` 분기와 다르다.
//! custom byte 하나가 그대로 cell index가 된다. 실제 BTX descriptor와
//! cell metadata·픽셀은 별도 `subgame_seq_pcomi_custom_byte_atlas` gate가
//! 소유한다. 이 모듈은 slot별 UX 의미나 플레이어 표시를 주장하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::SubgameSeqCustomByteRecordAnalysis;

use super::linear;

pub const PCOMI_CUSTOM_BYTE_FORMATTER_RUNTIME: u32 = 0x8002_0368;
pub const PCOMI_CUSTOM_BYTE_ROOT_FIELD_RUNTIME: u32 = 0x801A_629C;
pub const PCOMI_CUSTOM_BYTE_SLOT_TABLE_RUNTIME: u32 = 0x801A_8F40;
pub const PCOMI_CUSTOM_BYTE_SLOT_SELECTOR: u8 = 7;
pub const PCOMI_CUSTOM_BYTE_SLOT_COUNT: usize = 11;
pub const PCOMI_CUSTOM_BYTE_SLOT_PAYLOAD_BYTES: usize = 0xD8;
pub const PCOMI_CUSTOM_BYTE_BUFFER_RUNTIME: u32 = 0x8008_96E0;
pub const PCOMI_CUSTOM_BYTE_FORMAT_SCRATCH_RUNTIME: u32 = 0x8008_A1E8;
pub const PCOMI_CUSTOM_BYTE_U16_STREAM_RUNTIME: u32 = 0x8008_A0C8;
pub const PCOMI_CUSTOM_BYTE_UI_CONSUMER_RUNTIME: u32 = 0x8002_8D20;
pub const PCOMI_CUSTOM_BYTE_DIRECT_CELL_STORE_RUNTIME: u32 = 0x8002_9498;
pub const PCOMI_CUSTOM_BYTE_RENDERER_EXTENDED_RUNTIME: u32 = 0x8001_92F0;
pub const PCOMI_CUSTOM_BYTE_RENDERER_SHORT_RUNTIME: u32 = 0x8001_8FE4;

const PCOMI_CUSTOM_BYTE_SLOT_CALLBACK_RUNTIME: u32 = 0x8002_1E8C;
const PCOMI_CUSTOM_BYTE_SLOT_ACTION_TABLE_RUNTIME: u32 = 0x8001_0A78;
const PCOMI_CUSTOM_BYTE_SLOT_ACTION_COLUMN: u8 = 16;
const PCOMI_CUSTOM_BYTE_SLOT_ACTION_ROW: u8 = 4;
const PCOMI_CUSTOM_BYTE_SLOT_SOURCE_CELL: u16 = 0;
const PCOMI_CUSTOM_BYTE_SLOT_MODAL_STATE: u8 = 2;
const PCOMI_CUSTOM_BYTE_SLOT_SELECTOR_MODE: u8 = 0;
const PCOMI_CUSTOM_BYTE_ROOT_TABLE_RUNTIME: u32 = 0x8001_09D0;
const PCOMI_CUSTOM_BYTE_RNG_RUNTIME: u32 = 0x8002_E5E4;
const PCOMI_CUSTOM_BYTE_COPY_RUNTIME: u32 = 0x8002_E548;
const PCOMI_CUSTOM_BYTE_SLOT_CALL_RUNTIME: u32 = 0x8002_30B4;
const PCOMI_CUSTOM_BYTE_U16_BUILD_RUNTIME: u32 = 0x8002_3400;
const PCOMI_CUSTOM_BYTE_ORDINARY_TOKEN_LOAD_RUNTIME: u32 = 0x8002_8F68;
const PCOMI_CUSTOM_BYTE_TOKEN_NODE_STORE_RUNTIME: u32 = 0x8002_9040;
const PCOMI_CUSTOM_BYTE_TOKEN_NODE_LOAD_RUNTIME: u32 = 0x8002_948C;

const ROOT_CASES: [(u32, u32); 8] = [
    (0x8002_03FC, 0x801A_6280),
    (0x8002_040C, 0x801A_6284),
    (0x8002_041C, 0x801A_6288),
    (0x8002_042C, 0x801A_628C),
    (0x8002_043C, 0x801A_6290),
    (0x8002_044C, 0x801A_6294),
    (0x8002_045C, 0x801A_6298),
    (0x8002_046C, PCOMI_CUSTOM_BYTE_ROOT_FIELD_RUNTIME),
];

const FORMATTER_CALL_SITES: [u32; 4] = [
    0x8002_294C,
    PCOMI_CUSTOM_BYTE_SLOT_CALL_RUNTIME,
    0x8002_3104,
    0x8002_320C,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcomiCustomByteConsumerAnalysis {
    pub formatter_runtime: u32,
    pub formatter_call_sites: Vec<u32>,
    pub slot_selector: u8,
    pub slot_count: usize,
    pub slot_payload_bytes: usize,
    pub rng_modulus: u16,
    pub admission_callback_runtime: u32,
    pub admission_action_column: u8,
    pub admission_action_row: u8,
    pub admission_source_cell: u16,
    pub admission_modal_state: u8,
    pub admission_selector_mode: u8,
    pub root_field_runtime: u32,
    pub slot_table_runtime: u32,
    pub byte_buffer_runtime: u32,
    pub u16_stream_runtime: u32,
    pub ui_consumer_runtime: u32,
    pub direct_cell_store_runtime: u32,
    pub renderer_extended_runtime: u32,
    pub renderer_short_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcomiCustomByteConsumerError(String);

impl fmt::Display for PcomiCustomByteConsumerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for PcomiCustomByteConsumerError {}

fn error(message: impl Into<String>) -> PcomiCustomByteConsumerError {
    PcomiCustomByteConsumerError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, PcomiCustomByteConsumerError> {
    instructions
        .binary_search_by_key(&address, |instruction| instruction.addr)
        .ok()
        .map(|index| &instructions[index])
        .ok_or_else(|| error(format!("PCOMI custom byte 명령 주소 없음: 0x{address:08X}")))
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: K,
) -> Result<(), PcomiCustomByteConsumerError> {
    let actual = instruction_at(instructions, address)?;
    if actual.kind != expected {
        return Err(error(format!(
            "PCOMI custom byte 명령 불일치: address=0x{address:08X}, \
             expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn read_text_word(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
) -> Result<u32, PcomiCustomByteConsumerError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "PCOMI custom byte EXE 주소 범위 이탈: 0x{runtime:08X}"
        ))
    })?;
    let bytes = text
        .get(offset..offset + 4)
        .ok_or_else(|| error("PCOMI custom byte EXE word 범위 이탈"))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
}

fn verify_storage_boundary(
    custom: &SubgameSeqCustomByteRecordAnalysis,
) -> Result<(), PcomiCustomByteConsumerError> {
    let [row] = custom.rows.as_slice() else {
        return Err(error(format!(
            "PCOMI custom byte storage 행 수 불일치: {}",
            custom.rows.len()
        )));
    };
    if row.scope != "PCOMI"
        || row.sequence != "PCOMI1.SEQ"
        || row.slot_table_root_field_runtime != PCOMI_CUSTOM_BYTE_ROOT_FIELD_RUNTIME
        || row.slot_table_runtime != PCOMI_CUSTOM_BYTE_SLOT_TABLE_RUNTIME
        || row.slot_count != PCOMI_CUSTOM_BYTE_SLOT_COUNT
        || row.slot_payload_bytes != PCOMI_CUSTOM_BYTE_SLOT_PAYLOAD_BYTES
    {
        return Err(error("PCOMI custom byte storage/consumer 경계 불일치"));
    }
    Ok(())
}

fn verify_root_dispatch(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
) -> Result<(), PcomiCustomByteConsumerError> {
    require(
        instructions,
        PCOMI_CUSTOM_BYTE_FORMATTER_RUNTIME + 0x14,
        K::Addiu {
            rt: Reg::S3,
            rs: Reg::ZERO,
            imm: PCOMI_CUSTOM_BYTE_SLOT_PAYLOAD_BYTES as i16,
        },
    )?;
    require(
        instructions,
        PCOMI_CUSTOM_BYTE_FORMATTER_RUNTIME + 0x54,
        K::Sltiu {
            rt: Reg::A0,
            rs: Reg::S5,
            imm: PCOMI_CUSTOM_BYTE_SLOT_PAYLOAD_BYTES as i16,
        },
    )?;
    require(
        instructions,
        PCOMI_CUSTOM_BYTE_FORMATTER_RUNTIME + 0x68,
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::A2,
            imm: 8,
        },
    )?;
    require(
        instructions,
        PCOMI_CUSTOM_BYTE_FORMATTER_RUNTIME + 0x78,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0x09D0,
        },
    )?;
    require(
        instructions,
        PCOMI_CUSTOM_BYTE_FORMATTER_RUNTIME + 0x8C,
        K::Jr { rs: Reg::A0 },
    )?;

    for (index, &(case_runtime, root_runtime)) in ROOT_CASES.iter().enumerate() {
        if read_text_word(
            map,
            text,
            PCOMI_CUSTOM_BYTE_ROOT_TABLE_RUNTIME + u32::try_from(index).expect("8개 case") * 4,
        )? != case_runtime
        {
            return Err(error(format!(
                "PCOMI custom byte root jump table 불일치: selector={index}"
            )));
        }
        require(
            instructions,
            case_runtime,
            K::Lui {
                rt: Reg::V1,
                imm: 0x801A,
            },
        )?;
        require(
            instructions,
            case_runtime + 4,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V1,
                offset: root_runtime as u16 as i16,
            },
        )?;
    }

    require(
        instructions,
        0x8002_0474,
        K::Sll {
            rd: Reg::V0,
            rt: Reg::A3,
            sa: 2,
        },
    )?;
    require(
        instructions,
        0x8002_0478,
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::V1,
        },
    )?;
    require(
        instructions,
        0x8002_047C,
        K::Lw {
            rt: Reg::S1,
            base: Reg::V0,
            offset: 0,
        },
    )?;
    require(
        instructions,
        0x8002_0490,
        K::Lbu {
            rt: Reg::V1,
            base: Reg::S1,
            offset: 0,
        },
    )?;
    Ok(())
}

fn verify_formatter_calls(
    instructions: &[Instruction],
) -> Result<Vec<u32>, PcomiCustomByteConsumerError> {
    let actual = instructions
        .iter()
        .filter(|instruction| {
            instruction.kind
                == (K::Jal {
                    target: PCOMI_CUSTOM_BYTE_FORMATTER_RUNTIME,
                })
        })
        .map(|instruction| instruction.addr)
        .collect::<Vec<_>>();
    if actual != FORMATTER_CALL_SITES {
        return Err(error(format!(
            "PCOMI custom byte formatter call site 불일치: expected={FORMATTER_CALL_SITES:08X?}, \
             actual={actual:08X?}"
        )));
    }

    let checks = [
        (
            0x8002_3090,
            K::Jal {
                target: PCOMI_CUSTOM_BYTE_RNG_RUNTIME,
            },
        ),
        (
            0x8002_3094,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::ZERO,
                imm: PCOMI_CUSTOM_BYTE_SLOT_COUNT as i16,
            },
        ),
        (
            0x8002_3098,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: PCOMI_CUSTOM_BYTE_SLOT_SELECTOR as i16,
            },
        ),
        (
            0x8002_309C,
            K::Andi {
                rt: Reg::A3,
                rs: Reg::V0,
                imm: 0x00FF,
            },
        ),
        (
            PCOMI_CUSTOM_BYTE_SLOT_CALL_RUNTIME,
            K::Jal {
                target: PCOMI_CUSTOM_BYTE_FORMATTER_RUNTIME,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, address, expected)?;
    }

    require(
        instructions,
        PCOMI_CUSTOM_BYTE_RNG_RUNTIME + 0x28,
        K::Divu {
            rs: Reg::V1,
            rt: Reg::A1,
        },
    )?;
    require(
        instructions,
        PCOMI_CUSTOM_BYTE_RNG_RUNTIME + 0x2C,
        K::Sw {
            rt: Reg::A0,
            base: Reg::GP,
            offset: 0x68,
        },
    )?;
    require(
        instructions,
        PCOMI_CUSTOM_BYTE_RNG_RUNTIME + 0x30,
        K::Mfhi { rd: Reg::V0 },
    )?;
    Ok(actual)
}

fn slot_admission_checks() -> [(u32, K); 20] {
    [
        (
            PCOMI_CUSTOM_BYTE_SLOT_CALLBACK_RUNTIME,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -0x40,
            },
        ),
        (
            0x8002_1EA4,
            K::Addu {
                rd: Reg::S5,
                rs: Reg::A0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8002_1EF4,
            K::Sb {
                rt: Reg::ZERO,
                base: Reg::GP,
                offset: 0x01D2,
            },
        ),
        (
            0x8002_1EFC,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S5,
                offset: 0x006A,
            },
        ),
        (
            0x8002_27BC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: PCOMI_CUSTOM_BYTE_SLOT_ACTION_COLUMN as i16,
            },
        ),
        (
            0x8002_27C0,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::GP,
                offset: 0x01CC,
            },
        ),
        (
            0x8002_27C8,
            K::Bne {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 388,
            },
        ),
        (
            0x8002_27D0,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::GP,
                offset: 0x01CD,
            },
        ),
        (
            0x8002_27D8,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 7,
            },
        ),
        (
            0x8002_28E0,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S5,
                offset: 0x006A,
            },
        ),
        (
            0x8002_2960,
            K::Bne {
                rs: Reg::V1,
                rt: Reg::ZERO,
                offset: 44,
            },
        ),
        (
            0x8002_2A04,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: PCOMI_CUSTOM_BYTE_SLOT_MODAL_STATE as i16,
            },
        ),
        (
            0x8002_2A08,
            K::Sb {
                rt: Reg::V0,
                base: Reg::GP,
                offset: 0x01D0,
            },
        ),
        (
            0x8002_2E44,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::GP,
                offset: 0x01D0,
            },
        ),
        (
            0x8002_2E48,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::ZERO,
                imm: PCOMI_CUSTOM_BYTE_SLOT_MODAL_STATE as i16,
            },
        ),
        (
            0x8002_2E4C,
            K::Beq {
                rs: Reg::V1,
                rt: Reg::A0,
                offset: 118,
            },
        ),
        (
            0x8002_3030,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::A3,
                imm: 0x0020,
            },
        ),
        (
            0x8002_3034,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 124,
            },
        ),
        (
            0x8002_303C,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::GP,
                offset: 0x01D2,
            },
        ),
        (
            0x8002_3054,
            K::Beq {
                rs: Reg::V1,
                rt: Reg::ZERO,
                offset: 7,
            },
        ),
    ]
}

fn verify_slot_admission(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
) -> Result<(), PcomiCustomByteConsumerError> {
    for (address, expected) in slot_admission_checks() {
        require(instructions, address, expected)?;
    }

    let action_slot = PCOMI_CUSTOM_BYTE_SLOT_ACTION_TABLE_RUNTIME
        + u32::from(PCOMI_CUSTOM_BYTE_SLOT_ACTION_ROW) * 4;
    if read_text_word(map, text, action_slot)? != 0x8002_28E0 {
        return Err(error("PCOMI custom byte action row dispatch 불일치"));
    }
    Ok(())
}

fn verify_byte_to_cell_handoff(
    instructions: &[Instruction],
) -> Result<(), PcomiCustomByteConsumerError> {
    let checks = [
        (
            0x8002_0684,
            K::Sb {
                rt: Reg::V1,
                base: Reg::S2,
                offset: 0,
            },
        ),
        (
            0x8002_06F8,
            K::Jal {
                target: PCOMI_CUSTOM_BYTE_COPY_RUNTIME,
            },
        ),
        (
            PCOMI_CUSTOM_BYTE_U16_BUILD_RUNTIME,
            K::Addiu {
                rt: Reg::A3,
                rs: Reg::V1,
                imm: PCOMI_CUSTOM_BYTE_U16_STREAM_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8002_3454,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: PCOMI_CUSTOM_BYTE_BUFFER_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8002_3488,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::A2,
                offset: 0,
            },
        ),
        (
            0x8002_3494,
            K::Sh {
                rt: Reg::V0,
                base: Reg::A3,
                offset: 0,
            },
        ),
        (
            0x8002_34A0,
            K::Sh {
                rt: Reg::T2,
                base: Reg::A3,
                offset: 0,
            },
        ),
        (
            0x8002_34BC,
            K::Sh {
                rt: Reg::V0,
                base: Reg::A3,
                offset: 0,
            },
        ),
        (
            0x8002_34CC,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x8C,
            },
        ),
        (
            0x8002_34D4,
            K::Jal {
                target: PCOMI_CUSTOM_BYTE_UI_CONSUMER_RUNTIME,
            },
        ),
        (
            PCOMI_CUSTOM_BYTE_ORDINARY_TOKEN_LOAD_RUNTIME,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::T0,
                offset: 0,
            },
        ),
        (
            PCOMI_CUSTOM_BYTE_TOKEN_NODE_STORE_RUNTIME,
            K::Sh {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 4,
            },
        ),
        (
            PCOMI_CUSTOM_BYTE_TOKEN_NODE_LOAD_RUNTIME,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 4,
            },
        ),
        (
            PCOMI_CUSTOM_BYTE_DIRECT_CELL_STORE_RUNTIME,
            K::Sh {
                rt: Reg::V1,
                base: Reg::S2,
                offset: 0x6A,
            },
        ),
        (
            0x8002_94AC,
            K::Jal {
                target: PCOMI_CUSTOM_BYTE_RENDERER_EXTENDED_RUNTIME,
            },
        ),
        (
            0x8002_94BC,
            K::Jal {
                target: PCOMI_CUSTOM_BYTE_RENDERER_SHORT_RUNTIME,
            },
        ),
    ];
    for (address, expected) in checks {
        require(instructions, address, expected)?;
    }
    Ok(())
}

/// 지원 원본의 PCOMI custom byte slot 소비와 direct cell handoff를 검산한다.
pub fn verify_box_pcomi_custom_byte_consumer(
    map: &ExeMap,
    text: &[u8],
    custom: &SubgameSeqCustomByteRecordAnalysis,
) -> Result<PcomiCustomByteConsumerAnalysis, PcomiCustomByteConsumerError> {
    if map.name != "PCOMI.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "PCOMI custom byte EXE 사상 불일치: name={}, expected_len={}, actual_len={}",
            map.name,
            map.text_len,
            text.len()
        )));
    }
    verify_storage_boundary(custom)?;
    let instructions =
        linear::sweep(map, text).ok_or_else(|| error("PCOMI.EXE text 정렬 불일치"))?;
    verify_root_dispatch(map, text, &instructions)?;
    verify_slot_admission(map, text, &instructions)?;
    let formatter_call_sites = verify_formatter_calls(&instructions)?;
    verify_byte_to_cell_handoff(&instructions)?;

    Ok(PcomiCustomByteConsumerAnalysis {
        formatter_runtime: PCOMI_CUSTOM_BYTE_FORMATTER_RUNTIME,
        formatter_call_sites,
        slot_selector: PCOMI_CUSTOM_BYTE_SLOT_SELECTOR,
        slot_count: PCOMI_CUSTOM_BYTE_SLOT_COUNT,
        slot_payload_bytes: PCOMI_CUSTOM_BYTE_SLOT_PAYLOAD_BYTES,
        rng_modulus: PCOMI_CUSTOM_BYTE_SLOT_COUNT as u16,
        admission_callback_runtime: PCOMI_CUSTOM_BYTE_SLOT_CALLBACK_RUNTIME,
        admission_action_column: PCOMI_CUSTOM_BYTE_SLOT_ACTION_COLUMN,
        admission_action_row: PCOMI_CUSTOM_BYTE_SLOT_ACTION_ROW,
        admission_source_cell: PCOMI_CUSTOM_BYTE_SLOT_SOURCE_CELL,
        admission_modal_state: PCOMI_CUSTOM_BYTE_SLOT_MODAL_STATE,
        admission_selector_mode: PCOMI_CUSTOM_BYTE_SLOT_SELECTOR_MODE,
        root_field_runtime: PCOMI_CUSTOM_BYTE_ROOT_FIELD_RUNTIME,
        slot_table_runtime: PCOMI_CUSTOM_BYTE_SLOT_TABLE_RUNTIME,
        byte_buffer_runtime: PCOMI_CUSTOM_BYTE_BUFFER_RUNTIME,
        u16_stream_runtime: PCOMI_CUSTOM_BYTE_U16_STREAM_RUNTIME,
        ui_consumer_runtime: PCOMI_CUSTOM_BYTE_UI_CONSUMER_RUNTIME,
        direct_cell_store_runtime: PCOMI_CUSTOM_BYTE_DIRECT_CELL_STORE_RUNTIME,
        renderer_extended_runtime: PCOMI_CUSTOM_BYTE_RENDERER_EXTENDED_RUNTIME,
        renderer_short_runtime: PCOMI_CUSTOM_BYTE_RENDERER_SHORT_RUNTIME,
    })
}

#[cfg(test)]
#[path = "subgame_seq_pcomi_custom_byte_consumer_tests.rs"]
mod tests;
