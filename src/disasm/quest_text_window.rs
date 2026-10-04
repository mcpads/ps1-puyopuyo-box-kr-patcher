//! `QUEST.EXE` text window draw callback의 geometry 소비 경계.
//!
//! `FF07`·`FF08`이 쓰는 parent 객체의 위치와 extent가 실제 창 draw에서
//! 읽히는지, window state의 resource/cell이 어떤 record를 고르는지를
//! typed MIPS로 검증한다. 여기에는 특정 장면의 현재 좌표나 창 크기를
//! 넣지 않는다. 그런 값은 exact RAM 관측이 소유한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{linear, QUEST_FONT_STATE_BASE_RUNTIME, QUEST_FONT_STATE_RECORD_STRIDE};

pub const QUEST_TEXT_WINDOW_DRAW_RUNTIME: u32 = 0x8002_A49C;
pub const QUEST_TEXT_WINDOW_PARENT_STATE_OFFSET: i16 = 0x00;
pub const QUEST_TEXT_WINDOW_RESOURCE_STATE_OFFSET: i16 = 0x1A;
pub const QUEST_TEXT_WINDOW_CELL_STATE_OFFSET: i16 = 0x1B;
pub const QUEST_TEXT_WINDOW_PARENT_X_FIXED12_OFFSET: i16 = 0x4C;
pub const QUEST_TEXT_WINDOW_PARENT_Y_FIXED12_OFFSET: i16 = 0x50;
pub const QUEST_TEXT_WINDOW_PARENT_WIDTH_OFFSET: i16 = 0x58;
pub const QUEST_TEXT_WINDOW_PARENT_HEIGHT_OFFSET: i16 = 0x5C;
pub const QUEST_TEXT_WINDOW_POSITION_FRACTION_BITS: u8 = 12;
pub const QUEST_TEXT_WINDOW_FRAME_OUTSET_PX: i16 = 8;
pub const QUEST_TEXT_WINDOW_TILE_STEP_PX: i16 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestTextWindowConsumerSummary {
    pub draw_runtime: u32,
    pub state_base_runtime: u32,
    pub state_record_stride: usize,
    pub parent_state_offset: i16,
    pub resource_state_offset: i16,
    pub cell_state_offset: i16,
    pub parent_x_fixed12_offset: i16,
    pub parent_y_fixed12_offset: i16,
    pub parent_width_offset: i16,
    pub parent_height_offset: i16,
    pub position_fraction_bits: u8,
    pub frame_outset_px: i16,
    pub tile_step_px: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTextWindowConsumerError(String);

impl fmt::Display for QuestTextWindowConsumerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestTextWindowConsumerError {}

fn error(message: impl Into<String>) -> QuestTextWindowConsumerError {
    QuestTextWindowConsumerError(message.into())
}

const DRAW_EVIDENCE: &[(u32, K)] = &[
    // u8 index를 108바이트 state stride로 바꿔 고정 state base에 더한다.
    (
        0x8002_A4E4,
        K::Lui {
            rt: Reg::V1,
            imm: 0x800F,
        },
    ),
    (
        0x8002_A4E8,
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V1,
            imm: -2552,
        },
    ),
    (
        0x8002_A4EC,
        K::Sll {
            rd: Reg::V0,
            rt: Reg::A0,
            sa: 3,
        },
    ),
    (
        0x8002_A4F0,
        K::Subu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::A0,
        },
    ),
    (
        0x8002_A4F4,
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    ),
    (
        0x8002_A4F8,
        K::Subu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::A0,
        },
    ),
    (
        0x8002_A4FC,
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    ),
    (
        0x8002_A500,
        K::Addu {
            rd: Reg::S3,
            rs: Reg::V0,
            rt: Reg::V1,
        },
    ),
    // state +0 parent에서 12-bit fixed 위치와 pixel extent를 읽는다.
    (
        0x8002_A504,
        K::Lw {
            rt: Reg::A1,
            base: Reg::S3,
            offset: QUEST_TEXT_WINDOW_PARENT_STATE_OFFSET,
        },
    ),
    (
        0x8002_A50C,
        K::Lw {
            rt: Reg::V0,
            base: Reg::A1,
            offset: QUEST_TEXT_WINDOW_PARENT_X_FIXED12_OFFSET,
        },
    ),
    (
        0x8002_A520,
        K::Lw {
            rt: Reg::V1,
            base: Reg::A1,
            offset: QUEST_TEXT_WINDOW_PARENT_Y_FIXED12_OFFSET,
        },
    ),
    (
        0x8002_A524,
        K::Sra {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: QUEST_TEXT_WINDOW_POSITION_FRACTION_BITS,
        },
    ),
    (
        0x8002_A534,
        K::Sra {
            rd: Reg::V1,
            rt: Reg::V1,
            sa: QUEST_TEXT_WINDOW_POSITION_FRACTION_BITS,
        },
    ),
    (
        0x8002_A544,
        K::Lw {
            rt: Reg::T3,
            base: Reg::A1,
            offset: QUEST_TEXT_WINDOW_PARENT_WIDTH_OFFSET,
        },
    ),
    (
        0x8002_A55C,
        K::Lw {
            rt: Reg::A1,
            base: Reg::A1,
            offset: QUEST_TEXT_WINDOW_PARENT_HEIGHT_OFFSET,
        },
    ),
    // state의 window resource와 base cell이 descriptor/cell record를 고른다.
    (
        0x8002_A540,
        K::Lbu {
            rt: Reg::V1,
            base: Reg::S3,
            offset: QUEST_TEXT_WINDOW_RESOURCE_STATE_OFFSET,
        },
    ),
    (
        0x8002_A580,
        K::Lbu {
            rt: Reg::V1,
            base: Reg::S3,
            offset: QUEST_TEXT_WINDOW_CELL_STATE_OFFSET,
        },
    ),
    // 첫 frame primitive는 parent 위치보다 8px 바깥에서 시작한다.
    (
        0x8002_A5AC,
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::T4,
            imm: -QUEST_TEXT_WINDOW_FRAME_OUTSET_PX,
        },
    ),
    (
        0x8002_A5B0,
        K::Addiu {
            rt: Reg::A1,
            rs: Reg::T3,
            imm: -QUEST_TEXT_WINDOW_FRAME_OUTSET_PX,
        },
    ),
    // 두 extent는 16px tile 수로 각각 소비된다.
    (
        0x8002_A710,
        K::Lw {
            rt: Reg::V0,
            base: Reg::SP,
            offset: 0x1C,
        },
    ),
    (
        0x8002_A720,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: QUEST_TEXT_WINDOW_TILE_STEP_PX - 1,
        },
    ),
    (
        0x8002_A728,
        K::Sra {
            rd: Reg::S0,
            rt: Reg::V0,
            sa: 4,
        },
    ),
    (
        0x8002_A9EC,
        K::Lw {
            rt: Reg::V0,
            base: Reg::SP,
            offset: 0x20,
        },
    ),
    (
        0x8002_A9FC,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: QUEST_TEXT_WINDOW_TILE_STEP_PX - 1,
        },
    ),
    (
        0x8002_AA00,
        K::Sra {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 4,
        },
    ),
];

fn verify_draw_instructions(
    instructions: &[Instruction],
) -> Result<(), QuestTextWindowConsumerError> {
    for &(address, expected) in DRAW_EVIDENCE {
        let actual = instructions
            .iter()
            .find(|instruction| instruction.addr == address)
            .ok_or_else(|| {
                error(format!(
                    "QUEST text window draw 명령 주소 없음: 0x{address:08X}"
                ))
            })?;
        if actual.kind != expected {
            return Err(error(format!(
                "QUEST text window draw 명령 불일치: 0x{address:08X}, \
                 expected={expected:?}, actual={:?}",
                actual.kind
            )));
        }
    }
    Ok(())
}

/// 지원 원본의 window state→parent→draw geometry 소비를 검증한다.
pub fn verify_box_quest_text_window_consumer(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestTextWindowConsumerSummary, QuestTextWindowConsumerError> {
    if map.name != "QUEST.EXE" {
        return Err(error(format!(
            "QUEST text window consumer에 다른 EXE를 사용함: {}",
            map.name
        )));
    }
    if text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST text window consumer text 길이 불일치: expected={}, actual={}",
            map.text_len,
            text.len()
        )));
    }
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error("QUEST text window consumer text가 4바이트 정렬이 아님"))?;
    verify_draw_instructions(&instructions)?;

    Ok(QuestTextWindowConsumerSummary {
        draw_runtime: QUEST_TEXT_WINDOW_DRAW_RUNTIME,
        state_base_runtime: QUEST_FONT_STATE_BASE_RUNTIME,
        state_record_stride: QUEST_FONT_STATE_RECORD_STRIDE,
        parent_state_offset: QUEST_TEXT_WINDOW_PARENT_STATE_OFFSET,
        resource_state_offset: QUEST_TEXT_WINDOW_RESOURCE_STATE_OFFSET,
        cell_state_offset: QUEST_TEXT_WINDOW_CELL_STATE_OFFSET,
        parent_x_fixed12_offset: QUEST_TEXT_WINDOW_PARENT_X_FIXED12_OFFSET,
        parent_y_fixed12_offset: QUEST_TEXT_WINDOW_PARENT_Y_FIXED12_OFFSET,
        parent_width_offset: QUEST_TEXT_WINDOW_PARENT_WIDTH_OFFSET,
        parent_height_offset: QUEST_TEXT_WINDOW_PARENT_HEIGHT_OFFSET,
        position_fraction_bits: QUEST_TEXT_WINDOW_POSITION_FRACTION_BITS,
        frame_outset_px: QUEST_TEXT_WINDOW_FRAME_OUTSET_PX,
        tile_step_px: QUEST_TEXT_WINDOW_TILE_STEP_PX,
    })
}

#[cfg(test)]
#[path = "quest_text_window_tests.rs"]
mod tests;
