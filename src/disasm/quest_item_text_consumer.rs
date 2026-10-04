//! `ITEM_DAT.SEQ` item record의 이름과 설명 소비 경로.
//!
//! 이름 route는 `+0x0C` NUL 종단 glyph pointer를 길이 계산 뒤 QUEST glyph
//! writer에 넘긴다. 설명 route는 `+0x10` stream pointer를 새 interpreter
//! 객체에 설치하면서 active word를 1로 만들어 OP21 없이 text consumer로
//! 들어간다. 정적 route는 개별 item의 실행·표시 전수를 뜻하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{decode_slice, Instruction, InstructionKind as K, Reg};

use super::find_address_loads;

pub const QUEST_ITEM_RECORD_TABLE_RUNTIME: u32 = 0x801A_1A70;
pub const QUEST_ITEM_NAME_POINTER_FIELD_OFFSET: i16 = 0x0C;
pub const QUEST_ITEM_DESCRIPTION_POINTER_FIELD_OFFSET: i16 = 0x10;
pub const QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME: u32 = 0x8003_9E20;
pub const QUEST_ITEM_NAME_DRAW_ROUTE_RUNTIME: u32 = 0x8003_2458;
pub const QUEST_ITEM_DESCRIPTION_ACTIVE_STORE_RUNTIME: u32 = 0x8003_A3C4;

const QUEST_SEQ_INTERPRETER_RUNTIME: u32 = 0x8002_2570;
const QUEST_OBJECT_ALLOCATE_RUNTIME: u32 = 0x8001_4B14;
const QUEST_GLYPH_WRITER_RUNTIME: u32 = 0x8002_BA40;
const QUEST_U16_STRING_BYTES_RUNTIME: u32 = 0x8009_1D98;
pub const QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME: u32 = 0x8003_CD04;
pub const QUEST_ITEM_NAME_WHOLE_STRING_GLYPH_CALL_RUNTIME: u32 = 0x8003_CD44;
pub const QUEST_ITEM_NAME_WHOLE_STRING_GLYPH_RETURN_RUNTIME: u32 = 0x8003_CD4C;
const QUEST_SEQ_STREAM_FIELD_OFFSET: i16 = 0x1C;

pub const QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES: [u32; 15] = [
    0x8003_121C,
    0x8003_2880,
    0x8003_30D0,
    0x8003_3264,
    0x8003_33F4,
    0x8003_541C,
    0x8003_5580,
    0x8003_56E0,
    0x8003_6D7C,
    0x8003_6F10,
    0x8003_70A0,
    0x8003_7C58,
    0x8003_89C8,
    0x8003_92D4,
    0x8003_A774,
];
pub const QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES: [u32; 3] =
    [0x8005_3388, 0x8006_14D0, 0x8006_18F4];

const QUEST_ITEM_TABLE_ADDRESS_LOADS: [u32; 18] = [
    0x8002_D1D0,
    0x8003_1188,
    0x8003_2478,
    0x8003_7BF4,
    0x8003_896C,
    0x8003_9278,
    0x8003_9E2C,
    0x8003_A6BC,
    0x8003_AAC0,
    0x8003_AC44,
    0x8006_13CC,
    0x8006_143C,
    0x8006_1520,
    0x8006_18C0,
    0x8006_6E50,
    0x8006_7B58,
    0x8007_4EC8,
    0x8007_75FC,
];

// 1-based item ID는 표 직전 주소에 ID × 4를 더한다.
const QUEST_ITEM_ONE_BASED_TABLE_ADDRESS_LOADS: [u32; 24] = [
    0x8003_14B0,
    0x8003_15E8,
    0x8003_1644,
    0x8003_22DC,
    0x8003_283C,
    0x8003_3090,
    0x8003_3224,
    0x8003_33B8,
    0x8003_3590,
    0x8003_4184,
    0x8003_53DC,
    0x8003_5540,
    0x8003_56A4,
    0x8003_6D3C,
    0x8003_6ED0,
    0x8003_7064,
    0x8003_722C,
    0x8003_D7CC,
    0x8003_F25C,
    0x8005_30CC,
    0x8005_3F34,
    0x8007_20A4,
    0x8007_69F8,
    0x8007_6C44,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemTextConsumerSummary {
    pub record_table_runtime: u32,
    pub table_address_loads: Vec<u32>,
    pub name_pointer_field_offset: i16,
    pub name_draw_route_runtime: u32,
    pub name_length_call_runtime: u32,
    pub name_glyph_writer_call_runtime: u32,
    pub direct_name_glyph_call_runtimes: Vec<u32>,
    pub whole_string_name_call_runtimes: Vec<u32>,
    pub whole_string_draw_runtime: u32,
    pub whole_string_glyph_call_runtime: u32,
    pub whole_string_glyph_return_runtime: u32,
    pub whole_string_result_ignored_by_item_callers: bool,
    pub description_pointer_field_offset: i16,
    pub description_constructor_runtime: u32,
    pub description_stream_load_runtime: u32,
    pub description_active_store_runtime: u32,
    pub description_stream_store_runtime: u32,
    pub direct_text_mode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirectNameGlyphConsumer {
    table_address_load_runtime: u32,
    name_pointer_load_runtime: u32,
    name_pointer_load: K,
    length_call_runtime: u32,
    glyph_call_runtime: u32,
}

const DIRECT_NAME_GLYPH_CONSUMERS: [DirectNameGlyphConsumer; 15] = [
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_1188,
        name_pointer_load_runtime: 0x8003_11F0,
        name_pointer_load: K::Lw {
            rt: Reg::S2,
            base: Reg::S0,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_11FC,
        glyph_call_runtime: 0x8003_121C,
    },
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_283C,
        name_pointer_load_runtime: 0x8003_2858,
        name_pointer_load: K::Lw {
            rt: Reg::S3,
            base: Reg::V0,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_2860,
        glyph_call_runtime: 0x8003_2880,
    },
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_3090,
        name_pointer_load_runtime: 0x8003_30A8,
        name_pointer_load: K::Lw {
            rt: Reg::S2,
            base: Reg::V0,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_30B0,
        glyph_call_runtime: 0x8003_30D0,
    },
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_3224,
        name_pointer_load_runtime: 0x8003_323C,
        name_pointer_load: K::Lw {
            rt: Reg::S2,
            base: Reg::V0,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_3244,
        glyph_call_runtime: 0x8003_3264,
    },
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_33B8,
        name_pointer_load_runtime: 0x8003_33D0,
        name_pointer_load: K::Lw {
            rt: Reg::S2,
            base: Reg::V0,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_33D8,
        glyph_call_runtime: 0x8003_33F4,
    },
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_53DC,
        name_pointer_load_runtime: 0x8003_53F4,
        name_pointer_load: K::Lw {
            rt: Reg::S2,
            base: Reg::A0,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_53FC,
        glyph_call_runtime: 0x8003_541C,
    },
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_5540,
        name_pointer_load_runtime: 0x8003_5558,
        name_pointer_load: K::Lw {
            rt: Reg::S2,
            base: Reg::A0,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_5560,
        glyph_call_runtime: 0x8003_5580,
    },
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_56A4,
        name_pointer_load_runtime: 0x8003_56BC,
        name_pointer_load: K::Lw {
            rt: Reg::S2,
            base: Reg::A0,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_56C4,
        glyph_call_runtime: 0x8003_56E0,
    },
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_6D3C,
        name_pointer_load_runtime: 0x8003_6D54,
        name_pointer_load: K::Lw {
            rt: Reg::S2,
            base: Reg::V0,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_6D5C,
        glyph_call_runtime: 0x8003_6D7C,
    },
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_6ED0,
        name_pointer_load_runtime: 0x8003_6EE8,
        name_pointer_load: K::Lw {
            rt: Reg::S2,
            base: Reg::V0,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_6EF0,
        glyph_call_runtime: 0x8003_6F10,
    },
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_7064,
        name_pointer_load_runtime: 0x8003_707C,
        name_pointer_load: K::Lw {
            rt: Reg::S2,
            base: Reg::V0,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_7084,
        glyph_call_runtime: 0x8003_70A0,
    },
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_7BF4,
        name_pointer_load_runtime: 0x8003_7C30,
        name_pointer_load: K::Lw {
            rt: Reg::S2,
            base: Reg::V0,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_7C38,
        glyph_call_runtime: 0x8003_7C58,
    },
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_896C,
        name_pointer_load_runtime: 0x8003_89A4,
        name_pointer_load: K::Lw {
            rt: Reg::S2,
            base: Reg::S0,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_89AC,
        glyph_call_runtime: 0x8003_89C8,
    },
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_9278,
        name_pointer_load_runtime: 0x8003_92B0,
        name_pointer_load: K::Lw {
            rt: Reg::S2,
            base: Reg::S0,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_92B8,
        glyph_call_runtime: 0x8003_92D4,
    },
    DirectNameGlyphConsumer {
        table_address_load_runtime: 0x8003_A6BC,
        name_pointer_load_runtime: 0x8003_A750,
        name_pointer_load: K::Lw {
            rt: Reg::S3,
            base: Reg::S2,
            offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        },
        length_call_runtime: 0x8003_A758,
        glyph_call_runtime: 0x8003_A774,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WholeStringNameConsumer {
    table_address_load_runtime: u32,
    name_pointer_load_runtime: u32,
    draw_call_runtime: u32,
    result_discard_continuation: &'static [(u32, K)],
}

const WHOLE_STRING_RESULT_DISCARD_614D0: [(u32, K); 4] = [
    (
        0x8006_14D8,
        K::Addu {
            rd: Reg::A0,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
    ),
    (
        0x8006_14DC,
        K::Lui {
            rt: Reg::A1,
            imm: 0x8001,
        },
    ),
    (
        0x8006_14E0,
        K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 0x2224,
        },
    ),
    (
        0x8006_14E4,
        K::Jal {
            target: QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME,
        },
    ),
];

const WHOLE_STRING_RESULT_DISCARD_618F4: [(u32, K); 3] = [
    (
        0x8006_18FC,
        K::Addu {
            rd: Reg::A0,
            rs: Reg::ZERO,
            rt: Reg::ZERO,
        },
    ),
    (
        0x8006_1900,
        K::Lui {
            rt: Reg::A1,
            imm: 0x800C,
        },
    ),
    (
        0x8006_1904,
        K::Jal {
            target: QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME,
        },
    ),
];

const WHOLE_STRING_NAME_CONSUMERS: [WholeStringNameConsumer; 3] = [
    WholeStringNameConsumer {
        table_address_load_runtime: 0x8005_30CC,
        name_pointer_load_runtime: 0x8005_3384,
        draw_call_runtime: 0x8005_3388,
        result_discard_continuation: &[
            (
                0x8005_3390,
                K::Addu {
                    rd: Reg::A0,
                    rs: Reg::ZERO,
                    rt: Reg::ZERO,
                },
            ),
            (
                0x8005_3394,
                K::Lui {
                    rt: Reg::A1,
                    imm: 0x800C,
                },
            ),
            (
                0x8005_3398,
                K::Jal {
                    target: QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME,
                },
            ),
        ],
    },
    WholeStringNameConsumer {
        table_address_load_runtime: 0x8006_143C,
        name_pointer_load_runtime: 0x8006_14CC,
        draw_call_runtime: 0x8006_14D0,
        result_discard_continuation: &WHOLE_STRING_RESULT_DISCARD_614D0,
    },
    WholeStringNameConsumer {
        table_address_load_runtime: 0x8006_18C0,
        name_pointer_load_runtime: 0x8006_18F0,
        draw_call_runtime: 0x8006_18F4,
        result_discard_continuation: &WHOLE_STRING_RESULT_DISCARD_618F4,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemTextConsumerError(String);

impl fmt::Display for QuestItemTextConsumerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemTextConsumerError {}

fn error(message: impl Into<String>) -> QuestItemTextConsumerError {
    QuestItemTextConsumerError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    runtime: u32,
) -> Result<&Instruction, QuestItemTextConsumerError> {
    instructions
        .binary_search_by_key(&runtime, |instruction| instruction.addr)
        .ok()
        .and_then(|index| instructions.get(index))
        .ok_or_else(|| {
            error(format!(
                "QUEST item text consumer 명령이 없음: 0x{runtime:08X}"
            ))
        })
}

fn require(
    instructions: &[Instruction],
    runtime: u32,
    expected: K,
    role: &str,
) -> Result<(), QuestItemTextConsumerError> {
    let actual = instruction_at(instructions, runtime)?;
    if actual.kind != expected {
        return Err(error(format!(
            "QUEST item text consumer {role} drift: 0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn is_direct_text_install(
    stream_load: &K,
    active_value: &K,
    active_store: &K,
    stream_store: &K,
) -> bool {
    matches!(
        (stream_load, active_value, active_store, stream_store),
        (
            K::Lw {
                rt: Reg::A1,
                base: Reg::S5,
                offset: QUEST_ITEM_DESCRIPTION_POINTER_FIELD_OFFSET,
            },
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 1,
            },
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0,
            },
            K::Sw {
                rt: Reg::A1,
                base: Reg::V0,
                offset: QUEST_SEQ_STREAM_FIELD_OFFSET,
            },
        )
    )
}

fn verify_direct_text_install(
    instructions: &[Instruction],
) -> Result<(), QuestItemTextConsumerError> {
    let stream_load = &instruction_at(instructions, 0x8003_A3B0)?.kind;
    let active_value = &instruction_at(instructions, 0x8003_A3C0)?.kind;
    let active_store = &instruction_at(instructions, 0x8003_A3C4)?.kind;
    let stream_store = &instruction_at(instructions, 0x8003_A3C8)?.kind;
    if !is_direct_text_install(stream_load, active_value, active_store, stream_store) {
        return Err(error(format!(
            "QUEST item description direct text install drift: stream_load={stream_load:?}, active_value={active_value:?}, active_store={active_store:?}, stream_store={stream_store:?}"
        )));
    }
    Ok(())
}

fn verify_name_route(instructions: &[Instruction]) -> Result<(), QuestItemTextConsumerError> {
    for (runtime, expected, role) in [
        (
            0x8003_2478,
            K::Lui {
                rt: Reg::V1,
                imm: 0x801A,
            },
            "name table lui",
        ),
        (
            0x8003_247C,
            K::Ori {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: QUEST_ITEM_RECORD_TABLE_RUNTIME as u16,
            },
            "name table combine",
        ),
        (
            0x8003_2480,
            K::Andi {
                rt: Reg::A1,
                rs: Reg::A0,
                imm: 0xFF,
            },
            "name index mask",
        ),
        (
            0x8003_2484,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A1,
                sa: 2,
            },
            "name word index",
        ),
        (
            0x8003_2488,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
            "name table entry",
        ),
        (
            0x8003_2490,
            K::Lw {
                rt: Reg::S0,
                base: Reg::V0,
                offset: 0,
            },
            "name record load",
        ),
        (
            0x8003_249C,
            K::Lw {
                rt: Reg::S3,
                base: Reg::S0,
                offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
            },
            "name pointer load",
        ),
        (
            0x8003_24A8,
            K::Jal {
                target: QUEST_U16_STRING_BYTES_RUNTIME,
            },
            "name length call",
        ),
        (
            0x8003_24AC,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S3,
                rt: Reg::ZERO,
            },
            "name length argument",
        ),
        (
            0x8003_24C0,
            K::Lhu {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0,
            },
            "name glyph load",
        ),
        (
            0x8003_24C4,
            K::Jal {
                target: QUEST_GLYPH_WRITER_RUNTIME,
            },
            "name glyph writer",
        ),
    ] {
        require(instructions, runtime, expected, role)?;
    }
    Ok(())
}

fn verify_direct_name_glyph_consumers(
    instructions: &[Instruction],
) -> Result<(), QuestItemTextConsumerError> {
    for consumer in DIRECT_NAME_GLYPH_CONSUMERS {
        if !QUEST_ITEM_TABLE_ADDRESS_LOADS.contains(&consumer.table_address_load_runtime)
            && !QUEST_ITEM_ONE_BASED_TABLE_ADDRESS_LOADS
                .contains(&consumer.table_address_load_runtime)
        {
            return Err(error(format!(
                "QUEST direct item name table load가 전체 분모에 없음: 0x{:08X}",
                consumer.table_address_load_runtime
            )));
        }
        require(
            instructions,
            consumer.name_pointer_load_runtime,
            consumer.name_pointer_load,
            "direct name pointer load",
        )?;
        require(
            instructions,
            consumer.length_call_runtime,
            K::Jal {
                target: QUEST_U16_STRING_BYTES_RUNTIME,
            },
            "direct name length call",
        )?;
        require(
            instructions,
            consumer.glyph_call_runtime,
            K::Jal {
                target: QUEST_GLYPH_WRITER_RUNTIME,
            },
            "direct name glyph call",
        )?;
        require(
            instructions,
            consumer.glyph_call_runtime + 4,
            K::Addiu {
                rt: Reg::S0,
                rs: Reg::S0,
                imm: 2,
            },
            "direct name glyph call delay",
        )?;
    }
    let actual = DIRECT_NAME_GLYPH_CONSUMERS
        .iter()
        .map(|consumer| consumer.glyph_call_runtime)
        .collect::<Vec<_>>();
    if actual != QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES {
        return Err(error(format!(
            "QUEST direct item name glyph call 분모 drift: expected={QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES:08X?}, actual={actual:08X?}"
        )));
    }
    Ok(())
}

fn verify_whole_string_name_consumers(
    instructions: &[Instruction],
) -> Result<(), QuestItemTextConsumerError> {
    for consumer in WHOLE_STRING_NAME_CONSUMERS {
        if consumer.table_address_load_runtime == 0x8005_30CC {
            // 사용 item ID는 1-based이며 table 직전 주소에서 record를 고른다.
            for (runtime, expected) in [
                (
                    0x8005_30CC,
                    K::Lui {
                        rt: Reg::V1,
                        imm: 0x801A,
                    },
                ),
                (
                    0x8005_30D0,
                    K::Ori {
                        rt: Reg::V1,
                        rs: Reg::V1,
                        imm: (QUEST_ITEM_RECORD_TABLE_RUNTIME - 4) as u16,
                    },
                ),
                (
                    0x8005_311C,
                    K::Lbu {
                        rt: Reg::A3,
                        base: Reg::V0,
                        offset: 0x1F,
                    },
                ),
                (
                    0x8005_3124,
                    K::Sll {
                        rd: Reg::V0,
                        rt: Reg::A3,
                        sa: 2,
                    },
                ),
                (
                    0x8005_3128,
                    K::Addu {
                        rd: Reg::V0,
                        rs: Reg::V0,
                        rt: Reg::V1,
                    },
                ),
                (
                    0x8005_3130,
                    K::Lw {
                        rt: Reg::S0,
                        base: Reg::V0,
                        offset: 0,
                    },
                ),
            ] {
                require(instructions, runtime, expected, "사용 아이템 record 선택")?;
            }
        } else if !QUEST_ITEM_TABLE_ADDRESS_LOADS.contains(&consumer.table_address_load_runtime)
            && !QUEST_ITEM_ONE_BASED_TABLE_ADDRESS_LOADS
                .contains(&consumer.table_address_load_runtime)
        {
            return Err(error(format!(
                "QUEST whole-string item name table load가 전체 분모에 없음: 0x{:08X}",
                consumer.table_address_load_runtime
            )));
        }
        require(
            instructions,
            consumer.name_pointer_load_runtime,
            K::Lw {
                rt: Reg::A1,
                base: Reg::S0,
                offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
            },
            "whole-string name pointer load",
        )?;
        require(
            instructions,
            consumer.draw_call_runtime,
            K::Jal {
                target: QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME,
            },
            "whole-string name draw call",
        )?;
        require(
            instructions,
            consumer.draw_call_runtime + 4,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
            "whole-string name draw argument",
        )?;
        for &(runtime, expected) in consumer.result_discard_continuation {
            require(
                instructions,
                runtime,
                expected,
                "whole-string name discarded result continuation",
            )?;
        }
    }
    let actual = WHOLE_STRING_NAME_CONSUMERS
        .iter()
        .map(|consumer| consumer.draw_call_runtime)
        .collect::<Vec<_>>();
    if actual != QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES {
        return Err(error(format!(
            "QUEST whole-string item name call 분모 drift: expected={QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES:08X?}, actual={actual:08X?}"
        )));
    }
    Ok(())
}

fn verify_whole_string_draw_route(
    instructions: &[Instruction],
) -> Result<(), QuestItemTextConsumerError> {
    for (runtime, expected, role) in [
        (
            QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -40,
            },
            "whole-string draw entry",
        ),
        (
            0x8003_CD28,
            K::Jal {
                target: QUEST_U16_STRING_BYTES_RUNTIME,
            },
            "whole-string byte length call",
        ),
        (
            0x8003_CD34,
            K::Beq {
                rs: Reg::S1,
                rt: Reg::ZERO,
                offset: 8,
            },
            "whole-string empty branch",
        ),
        (
            0x8003_CD3C,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::S2,
                rt: Reg::S0,
            },
            "whole-string cursor address",
        ),
        (
            0x8003_CD40,
            K::Lhu {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0,
            },
            "whole-string glyph load",
        ),
        (
            QUEST_ITEM_NAME_WHOLE_STRING_GLYPH_CALL_RUNTIME,
            K::Jal {
                target: QUEST_GLYPH_WRITER_RUNTIME,
            },
            "whole-string glyph writer call",
        ),
        (
            QUEST_ITEM_NAME_WHOLE_STRING_GLYPH_CALL_RUNTIME + 4,
            K::Addiu {
                rt: Reg::S0,
                rs: Reg::S0,
                imm: 2,
            },
            "whole-string glyph cursor advance",
        ),
        (
            QUEST_ITEM_NAME_WHOLE_STRING_GLYPH_RETURN_RUNTIME,
            K::Sltu {
                rd: Reg::V0,
                rs: Reg::S0,
                rt: Reg::S1,
            },
            "whole-string glyph return",
        ),
        (
            0x8003_CD50,
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: -6,
            },
            "whole-string glyph loop",
        ),
        (
            0x8003_CD6C,
            K::Jr { rs: Reg::RA },
            "whole-string draw return",
        ),
    ] {
        require(instructions, runtime, expected, role)?;
    }
    Ok(())
}

fn verify_description_route(
    instructions: &[Instruction],
) -> Result<(), QuestItemTextConsumerError> {
    for (runtime, expected, role) in [
        (
            QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -56,
            },
            "description function entry",
        ),
        (
            0x8003_9E28,
            K::Addu {
                rd: Reg::S4,
                rs: Reg::A0,
                rt: Reg::ZERO,
            },
            "description index capture",
        ),
        (
            0x8003_9E2C,
            K::Lui {
                rt: Reg::V1,
                imm: 0x801A,
            },
            "description table lui",
        ),
        (
            0x8003_9E30,
            K::Ori {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: QUEST_ITEM_RECORD_TABLE_RUNTIME as u16,
            },
            "description table combine",
        ),
        (
            0x8003_9E40,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::S4,
                imm: 0xFF,
            },
            "description index mask",
        ),
        (
            0x8003_9E44,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
            "description word index",
        ),
        (
            0x8003_9E48,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
            "description table entry",
        ),
        (
            0x8003_9E64,
            K::Lw {
                rt: Reg::S5,
                base: Reg::V0,
                offset: 0,
            },
            "description record load",
        ),
        (
            0x8003_A36C,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8002,
            },
            "description callback lui",
        ),
        (
            0x8003_A370,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: QUEST_SEQ_INTERPRETER_RUNTIME as u16 as i16,
            },
            "description callback combine",
        ),
        (
            0x8003_A3A4,
            K::Jal {
                target: QUEST_OBJECT_ALLOCATE_RUNTIME,
            },
            "description object allocate",
        ),
    ] {
        require(instructions, runtime, expected, role)?;
    }
    verify_direct_text_install(instructions)?;
    Ok(())
}

/// 지원 `QUEST.EXE`의 item name·description 소비 route를 검증한다.
pub fn verify_box_quest_item_text_consumer(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestItemTextConsumerSummary, QuestItemTextConsumerError> {
    if map.name != "QUEST.EXE" {
        return Err(error(format!(
            "QUEST item text consumer 사상 이름 불일치: {}",
            map.name
        )));
    }
    if text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST item text consumer text 길이 불일치: map={}, actual={}",
            map.text_len,
            text.len()
        )));
    }
    let instructions = decode_slice(map.load_addr, text)
        .ok_or_else(|| error("QUEST item text consumer text가 4바이트 정렬이 아님"))?;
    verify_name_route(&instructions)?;
    verify_direct_name_glyph_consumers(&instructions)?;
    verify_whole_string_name_consumers(&instructions)?;
    verify_whole_string_draw_route(&instructions)?;
    verify_description_route(&instructions)?;

    let table_address_loads = find_address_loads(&instructions, 8)
        .into_iter()
        .filter(|load| load.value == QUEST_ITEM_RECORD_TABLE_RUNTIME)
        .map(|load| load.lui_addr)
        .collect::<Vec<_>>();
    if table_address_loads != QUEST_ITEM_TABLE_ADDRESS_LOADS {
        return Err(error(format!(
            "QUEST item record table address-load 분모 불일치: expected={QUEST_ITEM_TABLE_ADDRESS_LOADS:08X?}, actual={table_address_loads:08X?}"
        )));
    }

    let one_based_loads = find_address_loads(&instructions, 8)
        .into_iter()
        .filter(|load| load.value == QUEST_ITEM_RECORD_TABLE_RUNTIME - 4)
        .map(|load| load.lui_addr)
        .collect::<Vec<_>>();
    if one_based_loads != QUEST_ITEM_ONE_BASED_TABLE_ADDRESS_LOADS {
        return Err(error(format!(
            "QUEST 1-based item table load drift: {one_based_loads:08X?}"
        )));
    }

    Ok(QuestItemTextConsumerSummary {
        record_table_runtime: QUEST_ITEM_RECORD_TABLE_RUNTIME,
        table_address_loads,
        name_pointer_field_offset: QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
        name_draw_route_runtime: QUEST_ITEM_NAME_DRAW_ROUTE_RUNTIME,
        name_length_call_runtime: 0x8003_24A8,
        name_glyph_writer_call_runtime: 0x8003_24C4,
        direct_name_glyph_call_runtimes: QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES.to_vec(),
        whole_string_name_call_runtimes: QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES.to_vec(),
        whole_string_draw_runtime: QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME,
        whole_string_glyph_call_runtime: QUEST_ITEM_NAME_WHOLE_STRING_GLYPH_CALL_RUNTIME,
        whole_string_glyph_return_runtime: QUEST_ITEM_NAME_WHOLE_STRING_GLYPH_RETURN_RUNTIME,
        whole_string_result_ignored_by_item_callers: true,
        description_pointer_field_offset: QUEST_ITEM_DESCRIPTION_POINTER_FIELD_OFFSET,
        description_constructor_runtime: QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME,
        description_stream_load_runtime: 0x8003_A3B0,
        description_active_store_runtime: 0x8003_A3C4,
        description_stream_store_runtime: 0x8003_A3C8,
        direct_text_mode: true,
    })
}

#[cfg(test)]
#[path = "quest_item_text_consumer_tests.rs"]
mod tests;
