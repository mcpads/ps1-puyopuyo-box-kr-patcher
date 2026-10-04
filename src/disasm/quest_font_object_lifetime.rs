//! QUEST font object의 생성·초기화·draw 수명 경계.
//!
//! 공용 glyph renderer가 만든 type 14 object는 `0xE0`바이트 고정 pool에서
//! 나오고 같은 type list에 연결된다. pool은 boot에서 전부 0으로 초기화되며
//! release도 object 전체를 word 단위로 0으로 지운다. 따라서 원본이 전혀
//! 읽거나 쓰지 않는 `+0xDD` byte는 한글 page 표식 후보가 될 수 있다.
//!
//! draw는 매 frame object `+0xDE`의 state index로 descriptor와 texture page를
//! 다시 고른다. renderer가 끝났을 때만 원본 atlas를 잠시 복구하는 방식은
//! 살아 있는 object를 보존하지 못한다. 이 모듈은 그 결론에 필요한 원본
//! 명령과 향후 hook site만 검증하며, 대체 VRAM 소유권이나 실제 hook 실행을
//! 증명하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::linear;

pub const QUEST_OBJECT_POOL_RUNTIME: u32 = 0x800B_E998;
pub const QUEST_OBJECT_POOL_RECORD_WORDS: usize = 0x38;
pub const QUEST_OBJECT_POOL_RECORD_BYTES: usize = QUEST_OBJECT_POOL_RECORD_WORDS * 4;
pub const QUEST_OBJECT_POOL_RECORD_COUNT: usize = 500;
pub const QUEST_OBJECT_CALLBACK_FIELD_OFFSET: i16 = 0x18;
pub const QUEST_OBJECT_POOL_ZERO_WORDS: usize =
    QUEST_OBJECT_POOL_RECORD_WORDS * QUEST_OBJECT_POOL_RECORD_COUNT;
pub const QUEST_OBJECT_POOL_ZERO_CALL_RUNTIME: u32 = 0x8001_49F4;
pub const QUEST_OBJECT_POOL_LINK_LOOP_RUNTIME: u32 = 0x8001_4A18;
pub const QUEST_OBJECT_ALLOCATE_RUNTIME: u32 = 0x8001_4B14;
pub const QUEST_OBJECT_RELEASE_RUNTIME: u32 = 0x8001_4CD0;
pub const QUEST_OBJECT_RELEASE_ZERO_CALL_RUNTIME: u32 = 0x8001_4D04;
pub const QUEST_WORD_FILL_RUNTIME: u32 = 0x8007_E690;

pub const QUEST_FONT_OBJECT_TYPE: u8 = 14;
pub const QUEST_FONT_OBJECT_LIST_HEAD_RUNTIME: u32 = 0x800D_A0B0;
pub const QUEST_FONT_OBJECT_LIST_SENTINEL_RUNTIME: u32 = 0x800D_A0B4;
pub const QUEST_FONT_OBJECT_NEXT_OFFSET: i16 = 0x08;
pub const QUEST_FONT_OBJECT_CELL_INDEX_OFFSET: i16 = 0x66;
pub const QUEST_FONT_OBJECT_FLAGS_OFFSET: i16 = 0x6C;
pub const QUEST_FONT_OBJECT_SCALE_X_OFFSET: i16 = 0x58;
pub const QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET: i16 = 0xDD;
pub const QUEST_FONT_OBJECT_STATE_INDEX_OFFSET: i16 = 0xDE;

pub const QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME: u32 = 0x8002_BC84;
pub const QUEST_FONT_DRAW_RUNTIME: u32 = 0x8002_B3B8;
pub const QUEST_FONT_DRAW_LIST_HEAD_RUNTIME: u32 = 0x8002_B4C8;
pub const QUEST_FONT_DRAW_LIST_SENTINEL_RUNTIME: u32 = 0x8002_B4CC;
pub const QUEST_FONT_DRAW_STATE_INDEX_LOAD_RUNTIME: u32 = 0x8002_B500;
pub const QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME: u32 = 0x8002_B558;
pub const QUEST_FONT_DRAW_SHORT_PATH_SCALE_X_LOAD_RUNTIME: u32 = 0x8002_B5C4;
pub const QUEST_FONT_DRAW_QUAD_PATH_SCALE_X_LOAD_RUNTIME: u32 = 0x8002_B714;
pub const QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME: u32 = 0x8002_B8BC;
pub const QUEST_FONT_OBJECT_SCALE_X_INITIALIZE_RUNTIME: u32 = 0x8002_BC58;

const QUEST_FONT_OBJECT_IMMEDIATE_ALLOCATIONS: [u32; 5] = [
    0x8002_BB0C,
    0x8002_BB38,
    0x8002_BEC0,
    0x8002_C278,
    0x8002_CC8C,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestFontObjectLifetimeSummary {
    pub pool_runtime: u32,
    pub object_bytes: usize,
    pub object_count: usize,
    pub pool_zero_words: usize,
    pub release_zero_words: usize,
    pub font_object_type: u8,
    pub immediate_font_allocations: usize,
    pub list_head_runtime: u32,
    pub list_sentinel_runtime: u32,
    pub next_offset: i16,
    pub scale_x_offset: i16,
    pub page_marker_offset: i16,
    pub state_index_offset: i16,
    pub original_page_marker_accesses: usize,
    pub draw_runtime: u32,
    pub marker_site_runtime: u32,
    pub descriptor_site_runtime: u32,
    pub texture_page_site_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestFontObjectLifetimeError(String);

impl fmt::Display for QuestFontObjectLifetimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestFontObjectLifetimeError {}

fn error(message: impl Into<String>) -> QuestFontObjectLifetimeError {
    QuestFontObjectLifetimeError(message.into())
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: K,
) -> Result<(), QuestFontObjectLifetimeError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| error(format!("QUEST font object 명령 주소 없음: 0x{address:08X}")))?;
    if actual.kind != expected {
        return Err(error(format!(
            "QUEST font object 명령 불일치: 0x{address:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn memory_offset(kind: K) -> Option<i16> {
    match kind {
        K::Lb { offset, .. }
        | K::Lbu { offset, .. }
        | K::Lh { offset, .. }
        | K::Lhu { offset, .. }
        | K::Lw { offset, .. }
        | K::Lwl { offset, .. }
        | K::Lwr { offset, .. }
        | K::Sb { offset, .. }
        | K::Sh { offset, .. }
        | K::Sw { offset, .. }
        | K::Swl { offset, .. }
        | K::Swr { offset, .. }
        | K::Lwc2 { offset, .. }
        | K::Swc2 { offset, .. } => Some(offset),
        _ => None,
    }
}

fn verify_immediate_font_allocations(
    instructions: &[Instruction],
) -> Result<(), QuestFontObjectLifetimeError> {
    for call_runtime in QUEST_FONT_OBJECT_IMMEDIATE_ALLOCATIONS {
        require(
            instructions,
            call_runtime,
            K::Jal {
                target: QUEST_OBJECT_ALLOCATE_RUNTIME,
            },
        )?;
        let start = call_runtime.saturating_sub(24);
        let end = call_runtime + 4;
        let sets_type = instructions.iter().any(|instruction| {
            (start..=end).contains(&instruction.addr)
                && instruction.kind
                    == (K::Addiu {
                        rt: Reg::A1,
                        rs: Reg::ZERO,
                        imm: i16::from(QUEST_FONT_OBJECT_TYPE),
                    })
        });
        if !sets_type {
            return Err(error(format!(
                "QUEST font object type 14 설정 누락: call=0x{call_runtime:08X}"
            )));
        }
    }
    Ok(())
}

/// QUEST 원본에서 font object의 pool·list·매 frame 소비 경계를 검증한다.
pub fn verify_box_quest_font_object_lifetime(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestFontObjectLifetimeSummary, QuestFontObjectLifetimeError> {
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error("QUEST font object text 길이가 4바이트 배수가 아님"))?;

    let expected_pool_words = QUEST_OBJECT_POOL_RECORD_WORDS
        .checked_mul(QUEST_OBJECT_POOL_RECORD_COUNT)
        .ok_or_else(|| error("QUEST object pool word 수 overflow"))?;
    if QUEST_OBJECT_POOL_RECORD_BYTES != 0xE0
        || expected_pool_words != QUEST_OBJECT_POOL_ZERO_WORDS
        || QUEST_OBJECT_POOL_ZERO_WORDS != 0x6D60
    {
        return Err(error("QUEST object pool 정적 분모 drift"));
    }

    for (address, expected) in [
        // cursor 보정은 현재 object 좌표 생성 뒤, 원본 advance 전의 normal 효과만 쓴다.
        (
            0x8002_BBAC,
            K::Lh {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 0x0A,
            },
        ),
        (
            0x8002_BBB4,
            K::Sll {
                rd: Reg::A0,
                rt: Reg::V0,
                sa: 12,
            },
        ),
        (
            0x8002_BBB8,
            K::Sw {
                rt: Reg::A0,
                base: Reg::A2,
                offset: 0x4C,
            },
        ),
        (
            0x8002_BCA8,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x18,
            },
        ),
        (
            0x8002_BE24,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x16,
            },
        ),
        (
            0x8002_BE28,
            K::Lhu {
                rt: Reg::A0,
                base: Reg::V0,
                offset: 0x0A,
            },
        ),
        (
            0x8002_BE2C,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::A1,
                rt: Reg::V1,
            },
        ),
        (
            0x8002_BE30,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A0,
                rt: Reg::V1,
            },
        ),
        (
            0x8002_BE34,
            K::Sh {
                rt: Reg::A0,
                base: Reg::V0,
                offset: 0x0A,
            },
        ),
        (
            QUEST_OBJECT_POOL_ZERO_CALL_RUNTIME - 20,
            K::Lui {
                rt: Reg::S0,
                imm: (QUEST_OBJECT_POOL_RUNTIME.wrapping_add(0x8000) >> 16) as u16,
            },
        ),
        (
            QUEST_OBJECT_POOL_ZERO_CALL_RUNTIME - 16,
            K::Addiu {
                rt: Reg::S0,
                rs: Reg::S0,
                imm: QUEST_OBJECT_POOL_RUNTIME as u16 as i16,
            },
        ),
        (
            QUEST_OBJECT_POOL_ZERO_CALL_RUNTIME,
            K::Jal {
                target: QUEST_WORD_FILL_RUNTIME,
            },
        ),
        (
            QUEST_OBJECT_POOL_ZERO_CALL_RUNTIME + 4,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: QUEST_OBJECT_POOL_ZERO_WORDS as i16,
            },
        ),
        (
            QUEST_OBJECT_POOL_LINK_LOOP_RUNTIME,
            K::Sw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0x0C,
            },
        ),
        (
            QUEST_OBJECT_POOL_LINK_LOOP_RUNTIME + 4,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: QUEST_OBJECT_POOL_RECORD_BYTES as i16,
            },
        ),
        (
            QUEST_OBJECT_POOL_LINK_LOOP_RUNTIME + 12,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::A0,
                imm: (QUEST_OBJECT_POOL_RECORD_COUNT - 1) as i16,
            },
        ),
        (
            QUEST_OBJECT_POOL_LINK_LOOP_RUNTIME + 20,
            K::Addiu {
                rt: Reg::S0,
                rs: Reg::S0,
                imm: QUEST_OBJECT_POOL_RECORD_BYTES as i16,
            },
        ),
        (
            QUEST_OBJECT_RELEASE_ZERO_CALL_RUNTIME - 4,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: QUEST_OBJECT_POOL_RECORD_WORDS as i16,
            },
        ),
        (
            QUEST_OBJECT_RELEASE_ZERO_CALL_RUNTIME,
            K::Jal {
                target: QUEST_WORD_FILL_RUNTIME,
            },
        ),
        (
            QUEST_FONT_DRAW_RUNTIME,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -0xA0,
            },
        ),
        (
            QUEST_FONT_DRAW_LIST_HEAD_RUNTIME - 8,
            K::Lui {
                rt: Reg::V1,
                imm: 0x800E,
            },
        ),
        (
            QUEST_FONT_DRAW_LIST_HEAD_RUNTIME - 4,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x800D_9F58u32 as u16 as i16,
            },
        ),
        (
            QUEST_FONT_DRAW_LIST_HEAD_RUNTIME,
            K::Lw {
                rt: Reg::S1,
                base: Reg::V1,
                offset: 0x158,
            },
        ),
        (
            QUEST_FONT_DRAW_LIST_SENTINEL_RUNTIME,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x15C,
            },
        ),
        (
            QUEST_FONT_DRAW_STATE_INDEX_LOAD_RUNTIME,
            K::Lbu {
                rt: Reg::T2,
                base: Reg::S1,
                offset: QUEST_FONT_OBJECT_STATE_INDEX_OFFSET,
            },
        ),
        (
            QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x0E,
            },
        ),
        (
            QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME + 4,
            K::Sb {
                rt: Reg::S0,
                base: Reg::A2,
                offset: QUEST_FONT_OBJECT_STATE_INDEX_OFFSET,
            },
        ),
        (
            QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME,
            K::Lw {
                rt: Reg::A3,
                base: Reg::V1,
                offset: 0x10,
            },
        ),
        (
            QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME + 4,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            QUEST_FONT_DRAW_SHORT_PATH_SCALE_X_LOAD_RUNTIME,
            K::Lw {
                rt: Reg::V1,
                base: Reg::S1,
                offset: QUEST_FONT_OBJECT_SCALE_X_OFFSET,
            },
        ),
        (
            QUEST_FONT_DRAW_QUAD_PATH_SCALE_X_LOAD_RUNTIME,
            K::Lw {
                rt: Reg::A3,
                base: Reg::S1,
                offset: QUEST_FONT_OBJECT_SCALE_X_OFFSET,
            },
        ),
        (
            QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0x38,
            },
        ),
        (
            QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME + 4,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::A0,
                rt: Reg::T5,
            },
        ),
        (
            QUEST_FONT_OBJECT_SCALE_X_INITIALIZE_RUNTIME,
            K::Sw {
                rt: Reg::V1,
                base: Reg::A2,
                offset: QUEST_FONT_OBJECT_SCALE_X_OFFSET,
            },
        ),
    ] {
        require(&instructions, address, expected)?;
    }
    verify_immediate_font_allocations(&instructions)?;

    let original_page_marker_accesses = instructions
        .iter()
        .filter(|instruction| {
            memory_offset(instruction.kind) == Some(QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET)
        })
        .count();
    if original_page_marker_accesses != 0 {
        return Err(error(format!(
            "QUEST font object +0xDD 원본 접근이 생김: count={original_page_marker_accesses}"
        )));
    }

    Ok(QuestFontObjectLifetimeSummary {
        pool_runtime: QUEST_OBJECT_POOL_RUNTIME,
        object_bytes: QUEST_OBJECT_POOL_RECORD_BYTES,
        object_count: QUEST_OBJECT_POOL_RECORD_COUNT,
        pool_zero_words: QUEST_OBJECT_POOL_ZERO_WORDS,
        release_zero_words: QUEST_OBJECT_POOL_RECORD_WORDS,
        font_object_type: QUEST_FONT_OBJECT_TYPE,
        immediate_font_allocations: QUEST_FONT_OBJECT_IMMEDIATE_ALLOCATIONS.len(),
        list_head_runtime: QUEST_FONT_OBJECT_LIST_HEAD_RUNTIME,
        list_sentinel_runtime: QUEST_FONT_OBJECT_LIST_SENTINEL_RUNTIME,
        next_offset: QUEST_FONT_OBJECT_NEXT_OFFSET,
        scale_x_offset: QUEST_FONT_OBJECT_SCALE_X_OFFSET,
        page_marker_offset: QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET,
        state_index_offset: QUEST_FONT_OBJECT_STATE_INDEX_OFFSET,
        original_page_marker_accesses,
        draw_runtime: QUEST_FONT_DRAW_RUNTIME,
        marker_site_runtime: QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME,
        descriptor_site_runtime: QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME,
        texture_page_site_runtime: QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME,
    })
}

#[cfg(test)]
#[path = "quest_font_object_lifetime_tests.rs"]
mod tests;
