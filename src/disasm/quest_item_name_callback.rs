//! `ITEM_DAT.SEQ` 이름을 그리는 callback의 page 재시도 경계.
//!
//! item 이름은 type-13 callback의 inner state 2에서 공용 glyph writer를
//! 직접 부른다. 이 모듈은 draw 전 residency site와 draw 뒤 state 증가를
//! 분리하고 item/fallback caller return을 구분한다. 실제 hook·page 공급·
//! callback 재호출 cadence나 화면 표시는 증명하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{decode_slice, Instruction, InstructionKind as K, Reg};

use super::quest_font_object_lifetime::{
    QUEST_OBJECT_ALLOCATE_RUNTIME, QUEST_OBJECT_CALLBACK_FIELD_OFFSET,
};

pub const QUEST_ITEM_NAME_CALLBACK_RUNTIME: u32 = 0x8003_20CC;
pub const QUEST_ITEM_NAME_RESIDENCY_SITE_RUNTIME: u32 = 0x8003_22B0;
pub const QUEST_ITEM_NAME_RESIDENCY_CONTINUATION_RUNTIME: u32 = 0x8003_22B8;
pub const QUEST_ITEM_NAME_GLYPH_WRITER_RETURN_RUNTIME: u32 = 0x8003_24CC;
pub const QUEST_ITEM_NAME_FALLBACK_GLYPH_WRITER_RETURN_RUNTIME: u32 = 0x8003_2514;
pub const QUEST_ITEM_NAME_CALLBACK_EPILOGUE_RUNTIME: u32 = 0x8003_2DF4;

const QUEST_OBJECT_CALLBACK_DISPATCH_RUNTIME: u32 = 0x8001_4BA4;
const QUEST_OBJECT_CALLBACK_INDIRECT_CALL_RUNTIME: u32 = 0x8001_4C28;
const QUEST_GLYPH_WRITER_RUNTIME: u32 = 0x8002_BA40;
const QUEST_ITEM_NAME_INNER_STATE_OFFSET: i16 = 0xA2;
const QUEST_ITEM_NAME_DRAW_STATE: i16 = 2;
const QUEST_ITEM_NAME_CALLBACK_CONSTRUCTOR_RUNTIME: u32 = 0x8003_D898;
const QUEST_ITEM_NAME_STATE_ADVANCE_RUNTIME: u32 = 0x8003_268C;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemNameCallbackSummary {
    pub callback_runtime: u32,
    pub callback_constructor_runtime: u32,
    pub callback_dispatch_runtime: u32,
    pub state_offset: i16,
    pub draw_state: i16,
    pub residency_site_runtime: u32,
    pub residency_continuation_runtime: u32,
    pub callback_epilogue_runtime: u32,
    pub glyph_writer_return_runtime: u32,
    pub fallback_glyph_writer_return_runtime: u32,
    pub state_advance_runtime: u32,
    pub retry_boundary_before_state_advance: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemNameCallbackError(String);

impl fmt::Display for QuestItemNameCallbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemNameCallbackError {}

fn error(message: impl Into<String>) -> QuestItemNameCallbackError {
    QuestItemNameCallbackError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    runtime: u32,
) -> Result<&Instruction, QuestItemNameCallbackError> {
    instructions
        .binary_search_by_key(&runtime, |instruction| instruction.addr)
        .ok()
        .and_then(|index| instructions.get(index))
        .ok_or_else(|| {
            error(format!(
                "QUEST item name callback 명령이 없음: 0x{runtime:08X}"
            ))
        })
}

fn require(
    instructions: &[Instruction],
    runtime: u32,
    expected: K,
    role: &str,
) -> Result<(), QuestItemNameCallbackError> {
    let actual = instruction_at(instructions, runtime)?;
    if actual.kind != expected {
        return Err(error(format!(
            "QUEST item name callback {role} drift: 0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn is_retry_boundary(state_path: [&K; 3], residency_site: [&K; 2], state_advance: [&K; 3]) -> bool {
    let [state_load, state_value, state_branch] = state_path;
    let [site_first, site_second] = residency_site;
    let [advance_load, advance_value, advance_store] = state_advance;
    matches!(
        (
            state_load,
            state_value,
            state_branch,
            site_first,
            site_second,
            advance_load,
            advance_value,
            advance_store,
        ),
        (
            K::Lhu {
                rt: Reg::S0,
                base: Reg::S5,
                offset: QUEST_ITEM_NAME_INNER_STATE_OFFSET,
            },
            K::Addiu {
                rt: Reg::S4,
                rs: Reg::ZERO,
                imm: QUEST_ITEM_NAME_DRAW_STATE,
            },
            K::Beq {
                rs: Reg::S0,
                rt: Reg::S4,
                offset: 123,
            },
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::ZERO,
                imm: 1,
            },
            K::Addu {
                rd: Reg::S2,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
            K::Lhu {
                rt: Reg::V0,
                base: Reg::S5,
                offset: QUEST_ITEM_NAME_INNER_STATE_OFFSET,
            },
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
            K::Sh {
                rt: Reg::V0,
                base: Reg::S5,
                offset: QUEST_ITEM_NAME_INNER_STATE_OFFSET,
            },
        )
    )
}

/// 지원 `QUEST.EXE`에서 item 이름 callback의 재시도 가능한 원본 경계를 검증한다.
pub fn verify_box_quest_item_name_callback(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestItemNameCallbackSummary, QuestItemNameCallbackError> {
    if map.name != "QUEST.EXE" {
        return Err(error(format!(
            "QUEST item name callback 사상 이름 불일치: {}",
            map.name
        )));
    }
    if text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST item name callback text 길이 불일치: map={}, actual={}",
            map.text_len,
            text.len()
        )));
    }
    let instructions = decode_slice(map.load_addr, text)
        .ok_or_else(|| error("QUEST item name callback text가 4바이트 정렬이 아님"))?;

    for (runtime, expected, role) in [
        (
            0x8002_75C0,
            K::Jal {
                target: QUEST_OBJECT_CALLBACK_DISPATCH_RUNTIME,
            },
            "callback dispatcher call",
        ),
        (
            0x8001_4C18,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_OBJECT_CALLBACK_FIELD_OFFSET,
            },
            "callback field load",
        ),
        (
            QUEST_OBJECT_CALLBACK_INDIRECT_CALL_RUNTIME,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
            "callback indirect call",
        ),
        (
            0x8001_4C2C,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
            "callback object argument",
        ),
        (
            QUEST_ITEM_NAME_CALLBACK_CONSTRUCTOR_RUNTIME,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -40,
            },
            "constructor entry",
        ),
        (
            0x8003_D8BC,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8003,
            },
            "callback address high",
        ),
        (
            0x8003_D8C0,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: QUEST_ITEM_NAME_CALLBACK_RUNTIME as u16 as i16,
            },
            "callback address low",
        ),
        (
            0x8003_D8C8,
            K::Jal {
                target: QUEST_OBJECT_ALLOCATE_RUNTIME,
            },
            "callback object allocation",
        ),
        (
            0x8003_D8CC,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: 13,
            },
            "callback object type",
        ),
        (
            0x8001_4B30,
            K::Sw {
                rt: Reg::A0,
                base: Reg::A2,
                offset: QUEST_OBJECT_CALLBACK_FIELD_OFFSET,
            },
            "allocator callback store",
        ),
        (
            QUEST_ITEM_NAME_CALLBACK_RUNTIME,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -72,
            },
            "callback entry",
        ),
        (
            0x8003_24C4,
            K::Jal {
                target: QUEST_GLYPH_WRITER_RUNTIME,
            },
            "item glyph call",
        ),
        (
            0x8003_250C,
            K::Jal {
                target: QUEST_GLYPH_WRITER_RUNTIME,
            },
            "fallback glyph call",
        ),
        (
            QUEST_ITEM_NAME_CALLBACK_EPILOGUE_RUNTIME,
            K::Lw {
                rt: Reg::RA,
                base: Reg::SP,
                offset: 68,
            },
            "callback epilogue",
        ),
    ] {
        require(&instructions, runtime, expected, role)?;
    }

    let retry_boundary = is_retry_boundary(
        [
            &instruction_at(&instructions, 0x8003_213C)?.kind,
            &instruction_at(&instructions, 0x8003_2150)?.kind,
            &instruction_at(&instructions, 0x8003_2164)?.kind,
        ],
        [
            &instruction_at(&instructions, QUEST_ITEM_NAME_RESIDENCY_SITE_RUNTIME)?.kind,
            &instruction_at(&instructions, QUEST_ITEM_NAME_RESIDENCY_SITE_RUNTIME + 4)?.kind,
        ],
        [
            &instruction_at(&instructions, 0x8003_267C)?.kind,
            &instruction_at(&instructions, 0x8003_2684)?.kind,
            &instruction_at(&instructions, QUEST_ITEM_NAME_STATE_ADVANCE_RUNTIME)?.kind,
        ],
    );
    if !retry_boundary {
        return Err(error("QUEST item name callback retry 경계 drift"));
    }

    Ok(QuestItemNameCallbackSummary {
        callback_runtime: QUEST_ITEM_NAME_CALLBACK_RUNTIME,
        callback_constructor_runtime: QUEST_ITEM_NAME_CALLBACK_CONSTRUCTOR_RUNTIME,
        callback_dispatch_runtime: QUEST_OBJECT_CALLBACK_DISPATCH_RUNTIME,
        state_offset: QUEST_ITEM_NAME_INNER_STATE_OFFSET,
        draw_state: QUEST_ITEM_NAME_DRAW_STATE,
        residency_site_runtime: QUEST_ITEM_NAME_RESIDENCY_SITE_RUNTIME,
        residency_continuation_runtime: QUEST_ITEM_NAME_RESIDENCY_CONTINUATION_RUNTIME,
        callback_epilogue_runtime: QUEST_ITEM_NAME_CALLBACK_EPILOGUE_RUNTIME,
        glyph_writer_return_runtime: QUEST_ITEM_NAME_GLYPH_WRITER_RETURN_RUNTIME,
        fallback_glyph_writer_return_runtime: QUEST_ITEM_NAME_FALLBACK_GLYPH_WRITER_RETURN_RUNTIME,
        state_advance_runtime: QUEST_ITEM_NAME_STATE_ADVANCE_RUNTIME,
        retry_boundary_before_state_advance: true,
    })
}

#[cfg(test)]
#[path = "quest_item_name_callback_tests.rs"]
mod tests;
