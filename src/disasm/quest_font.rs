//! QUEST 고정 atlas 글리프 lookup의 소비 경계.
//!
//! `QUEST.EXE`의 대사 renderer는 u16 code를 345-entry 표에서 선형
//! 검색하고 찾지 못하면 마지막 index 344를 쓴다. 찾은 index는 object
//! `+0x66`에 저장되며, 같은 object가 고른 resource descriptor의
//! `+0x1C` cell table에서 `index × 12` record를 읽는다. renderer state
//! 10개는 QUEST 초기화에서 108바이트 stride로 한꺼번에 만들고 resource
//! selector를 모두 1로 둔다.
//!
//! 고정 `RPG_FNT1.BTX` descriptor는 해제 RAM 필드, VRAM 좌표, cell
//! table과 pixel format을 함께 가진다. 해제 RAM 필드는 원본에서 0이며
//! loader가 staging과 최종 해제 결과를 차례로 기록한다. resource loader는
//! 36바이트 descriptor를 고른 뒤 upload callback에서 `RECT`를 만들고
//! `LoadImage` wrapper에 `(RECT*, 해제 RAM)`을 넘긴 다음 `DrawSync(0)`로
//! 전송 완료를 기다린다. 이 경계는 자산 저장과 renderer lookup 사이의
//! 실제 공급 경로이며, 해제 RAM의 장기 수명은 보장하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::font::{QUEST_CODE_TABLE_COUNT, QUEST_CODE_TABLE_RUNTIME};
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::linear;

pub const QUEST_GLYPH_RENDERER_RUNTIME: u32 = 0x8002_BA40;
pub const QUEST_GLYPH_RENDERER_CONTINUATION_RUNTIME: u32 = QUEST_GLYPH_RENDERER_RUNTIME + 8;
pub const QUEST_GLYPH_CODE_TABLE_LIMIT_RUNTIME: u32 = 0x8002_BB44;
pub const QUEST_GLYPH_CODE_TABLE_POINTER_RUNTIME: u32 = 0x8002_BB50;
pub const QUEST_GLYPH_FALLBACK_INDEX_RUNTIME: u32 = 0x8002_BB78;
pub const QUEST_GLYPH_CELL_INDEX_STORE_RUNTIME: u32 = 0x8002_BB7C;
pub const QUEST_GLYPH_RESOURCE_SELECTOR_LOAD_RUNTIME: u32 = 0x8002_BBD4;
pub const QUEST_GLYPH_CELL_TABLE_LOAD_RUNTIME: u32 = 0x8002_BBF8;
pub const QUEST_GLYPH_CELL_STRIDE_RUNTIME: u32 = 0x8002_BC04;
pub const QUEST_GLYPH_NARROW_BOUNDARY_RUNTIME: u32 = 0x8002_BC1C;
pub const QUEST_FONT_STATE_INITIALIZER_RUNTIME: u32 = 0x8002_A15C;
pub const QUEST_FONT_STATE_BOOT_CALL_RUNTIME: u32 = 0x8002_7510;
pub const QUEST_FONT_STATE_CONDITIONAL_RESET_CALL_RUNTIME: u32 = 0x8007_F17C;
pub const QUEST_FONT_STATE_BASE_RUNTIME: u32 = 0x800E_F608;
pub const QUEST_FONT_DESCRIPTOR_ROOT_RUNTIME: u32 = 0x8008_C8F8;
pub const QUEST_FONT_DESCRIPTOR_ROOT_POINTERS_RUNTIME: u32 = 0x800D_B328;
pub const QUEST_FONT_DESCRIPTOR_RUNTIME: u32 = 0x8008_C91C;
pub const QUEST_FONT_DESCRIPTOR_BIND_RUNTIME: u32 = 0x8002_2528;
pub const QUEST_FONT_DESCRIPTOR_COPY_RUNTIME: u32 = 0x8001_897C;
pub const QUEST_FONT_DESCRIPTOR_STAGING_STORE_RUNTIME: u32 = 0x8001_9734;
pub const QUEST_FONT_DESCRIPTOR_DECODED_STORE_RUNTIME: u32 = 0x8001_BED8;
pub const QUEST_FONT_UPLOAD_CALLBACK_RUNTIME: u32 = 0x8001_BFA4;
pub const QUEST_FONT_UPLOAD_CALL_RUNTIME: u32 = 0x8001_C06C;
pub const QUEST_FONT_UPLOAD_TARGET_RUNTIME: u32 = 0x8009_38B0;
pub const QUEST_FONT_UPLOAD_NAME_RUNTIME: u32 = 0x8001_3DB4;
pub const QUEST_FONT_UPLOAD_SYNC_CALL_RUNTIME: u32 = 0x8001_C074;
pub const QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME: u32 = 0x8009_3604;
pub const QUEST_FONT_UPLOAD_SYNC_NAME_RUNTIME: u32 = 0x8001_3D64;
pub const QUEST_FONT_ASSET_NAME_RUNTIME: u32 = 0x8008_DB04;
pub const QUEST_FONT_CELL_TABLE_RUNTIME: u32 = 0x8008_9D8C;

pub const QUEST_GLYPH_FALLBACK_INDEX: usize = QUEST_CODE_TABLE_COUNT - 1;
pub const QUEST_GLYPH_CELL_INDEX_OBJECT_OFFSET: i16 = 0x66;
pub const QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET: i16 = 0x1E;
pub const QUEST_GLYPH_CELL_TABLE_DESCRIPTOR_OFFSET: i16 = 0x1C;
pub const QUEST_GLYPH_CELL_RECORD_STRIDE: usize = 12;
pub const QUEST_FONT_STATE_RECORD_COUNT: usize = 10;
pub const QUEST_FONT_STATE_RECORD_STRIDE: usize = 108;
pub const QUEST_FONT_STATE_BYTES: usize =
    QUEST_FONT_STATE_RECORD_COUNT * QUEST_FONT_STATE_RECORD_STRIDE;
pub const QUEST_FONT_INITIAL_RESOURCE_SELECTOR: u8 = 1;
pub const QUEST_FONT_DESCRIPTOR_STRIDE: usize = 36;
pub const QUEST_FONT_DESCRIPTOR_NAME_OFFSET: i16 = 0x04;
pub const QUEST_FONT_DESCRIPTOR_COMPRESSED_BYTES_OFFSET: i16 = 0x08;
pub const QUEST_FONT_DESCRIPTOR_DECODED_BYTES_OFFSET: i16 = 0x0C;
pub const QUEST_FONT_DESCRIPTOR_SOURCE_OFFSET: i16 = 0x10;
pub const QUEST_FONT_DESCRIPTOR_AUX_OFFSET: i16 = 0x14;
pub const QUEST_FONT_DESCRIPTOR_VRAM_X_OFFSET: i16 = 0x18;
pub const QUEST_FONT_DESCRIPTOR_VRAM_Y_OFFSET: i16 = 0x1A;
pub const QUEST_FONT_DESCRIPTOR_CELL_TABLE_OFFSET: i16 = 0x1C;
pub const QUEST_FONT_DESCRIPTOR_PIXEL_FORMAT_OFFSET: i16 = 0x20;
pub const QUEST_FONT_DESCRIPTOR_INITIAL_FLAGS: u32 = 0x0001_0000;
pub const QUEST_FONT_COMPRESSED_BYTES: usize = 27_104;
pub const QUEST_FONT_SOURCE_BYTES: usize = 32_768;
pub const QUEST_FONT_ATLAS_WIDTH: u16 = 256;
pub const QUEST_FONT_ATLAS_HEIGHT: u16 = 256;
pub const QUEST_FONT_VRAM_X_WORDS: u16 = 576;
pub const QUEST_FONT_VRAM_Y: u16 = 0;
pub const QUEST_FONT_VRAM_WIDTH_WORDS: u16 = 64;
pub const QUEST_FONT_PIXEL_FORMAT_INDEXED4: u32 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestFontConsumerSummary {
    pub renderer_runtime: u32,
    pub code_table_runtime: u32,
    pub code_table_entries: usize,
    pub fallback_index: usize,
    pub cell_index_object_offset: i16,
    pub resource_selector_state_offset: i16,
    pub cell_table_descriptor_offset: i16,
    pub cell_record_stride: usize,
    pub narrow_boundary: usize,
    pub state_initializer_runtime: u32,
    pub state_boot_call_runtime: u32,
    pub state_conditional_reset_call_runtime: u32,
    pub state_initializer_direct_calls: usize,
    pub state_base_runtime: u32,
    pub state_record_count: usize,
    pub state_record_stride: usize,
    pub state_bytes: usize,
    pub initial_resource_selector: u8,
    pub descriptor_root_runtime: u32,
    pub descriptor_runtime: u32,
    pub descriptor_stride: usize,
    pub descriptor_copy_runtime: u32,
    pub source_field_initial: u32,
    pub asset_name_runtime: u32,
    pub compressed_bytes: usize,
    pub source_bytes: usize,
    pub source_staging_store_runtime: u32,
    pub source_decoded_store_runtime: u32,
    pub cell_table_runtime: u32,
    pub pixel_format: u32,
    pub vram_width_words: u16,
    pub vram_height: u16,
    pub upload_callback_runtime: u32,
    pub upload_call_runtime: u32,
    pub upload_target_runtime: u32,
    pub upload_sync_call_runtime: u32,
    pub upload_sync_target_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestFontConsumerError(String);

impl fmt::Display for QuestFontConsumerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestFontConsumerError {}

fn error(message: impl Into<String>) -> QuestFontConsumerError {
    QuestFontConsumerError(message.into())
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: K,
) -> Result<(), QuestFontConsumerError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| error(format!("QUEST font 명령 주소 없음: 0x{address:08X}")))?;
    if actual.kind != expected {
        return Err(error(format!(
            "QUEST font 명령 불일치: 0x{address:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn runtime_slice<'a>(
    map: &ExeMap,
    text: &'a [u8],
    address: u32,
    len: usize,
) -> Result<&'a [u8], QuestFontConsumerError> {
    let start = map
        .text_offset(address)
        .ok_or_else(|| error(format!("QUEST font data 주소 범위 이탈: 0x{address:08X}")))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| error("QUEST font data 범위 overflow"))?;
    text.get(start..end).ok_or_else(|| {
        error(format!(
            "QUEST font data slice 범위 이탈: 0x{address:08X}+0x{len:X}"
        ))
    })
}

fn runtime_u16(map: &ExeMap, text: &[u8], address: u32) -> Result<u16, QuestFontConsumerError> {
    let raw = runtime_slice(map, text, address, 2)?;
    Ok(u16::from_le_bytes([raw[0], raw[1]]))
}

fn runtime_u32(map: &ExeMap, text: &[u8], address: u32) -> Result<u32, QuestFontConsumerError> {
    let raw = runtime_slice(map, text, address, 4)?;
    Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn require_u16(
    map: &ExeMap,
    text: &[u8],
    address: u32,
    expected: u16,
    field: &str,
) -> Result<(), QuestFontConsumerError> {
    let actual = runtime_u16(map, text, address)?;
    if actual != expected {
        return Err(error(format!(
            "QUEST font {field} 불일치: address=0x{address:08X}, \
             expected=0x{expected:04X}, actual=0x{actual:04X}"
        )));
    }
    Ok(())
}

fn require_u32(
    map: &ExeMap,
    text: &[u8],
    address: u32,
    expected: u32,
    field: &str,
) -> Result<(), QuestFontConsumerError> {
    let actual = runtime_u32(map, text, address)?;
    if actual != expected {
        return Err(error(format!(
            "QUEST font {field} 불일치: address=0x{address:08X}, \
             expected=0x{expected:08X}, actual=0x{actual:08X}"
        )));
    }
    Ok(())
}

fn verify_state_initializer(instructions: &[Instruction]) -> Result<(), QuestFontConsumerError> {
    let expected = [
        (
            QUEST_FONT_STATE_BOOT_CALL_RUNTIME,
            K::Jal {
                target: QUEST_FONT_STATE_INITIALIZER_RUNTIME,
            },
        ),
        (
            QUEST_FONT_STATE_INITIALIZER_RUNTIME + 8,
            K::Lui {
                rt: Reg::S0,
                imm: (QUEST_FONT_STATE_BASE_RUNTIME.wrapping_add(0x8000) >> 16) as u16,
            },
        ),
        (
            QUEST_FONT_STATE_INITIALIZER_RUNTIME + 12,
            K::Addiu {
                rt: Reg::S0,
                rs: Reg::S0,
                imm: QUEST_FONT_STATE_BASE_RUNTIME as u16 as i16,
            },
        ),
        (
            QUEST_FONT_STATE_INITIALIZER_RUNTIME + 16,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ),
        (
            QUEST_FONT_STATE_INITIALIZER_RUNTIME + 20,
            K::Addu {
                rd: Reg::A1,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8002_A198,
            K::Jal {
                target: 0x8007_E6D8,
            },
        ),
        (
            0x8002_A19C,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: QUEST_FONT_STATE_BYTES as i16,
            },
        ),
        (
            0x8002_A1A0,
            K::Addu {
                rd: Reg::S1,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8002_A1A4,
            K::Addu {
                rd: Reg::S4,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8002_A1B8,
            K::Andi {
                rt: Reg::V1,
                rs: Reg::S1,
                imm: 0x00FF,
            },
        ),
        (
            0x8002_A1BC,
            K::Sll {
                rd: Reg::S0,
                rt: Reg::V1,
                sa: 3,
            },
        ),
        (
            0x8002_A1C0,
            K::Subu {
                rd: Reg::S0,
                rs: Reg::S0,
                rt: Reg::V1,
            },
        ),
        (
            0x8002_A1C4,
            K::Sll {
                rd: Reg::S0,
                rt: Reg::S0,
                sa: 2,
            },
        ),
        (
            0x8002_A1C8,
            K::Subu {
                rd: Reg::S0,
                rs: Reg::S0,
                rt: Reg::V1,
            },
        ),
        (
            0x8002_A1CC,
            K::Sll {
                rd: Reg::S0,
                rt: Reg::S0,
                sa: 2,
            },
        ),
        (
            0x8002_A1D0,
            K::Addu {
                rd: Reg::S0,
                rs: Reg::S0,
                rt: Reg::S4,
            },
        ),
        (
            0x8002_A1E8,
            K::Addiu {
                rt: Reg::S2,
                rs: Reg::ZERO,
                imm: QUEST_FONT_INITIAL_RESOURCE_SELECTOR as i16,
            },
        ),
        (
            0x8002_A204,
            K::Sb {
                rt: Reg::S2,
                base: Reg::S0,
                offset: QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET,
            },
        ),
        (
            0x8002_A218,
            K::Addiu {
                rt: Reg::S1,
                rs: Reg::S1,
                imm: 1,
            },
        ),
        (
            0x8002_A220,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::S1,
                imm: 0x00FF,
            },
        ),
        (
            0x8002_A224,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_FONT_STATE_RECORD_COUNT as i16,
            },
        ),
        (
            0x8002_A228,
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: -31,
            },
        ),
        (
            0x8007_F160,
            K::Addu {
                rd: Reg::S0,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8007_F164,
            K::Jal {
                target: 0x8001_4D38,
            },
        ),
        (
            0x8007_F16C,
            K::Addiu {
                rt: Reg::S0,
                rs: Reg::S0,
                imm: 1,
            },
        ),
        (
            0x8007_F170,
            K::Slti {
                rt: Reg::V0,
                rs: Reg::S0,
                imm: 16,
            },
        ),
        (
            0x8007_F174,
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: -5,
            },
        ),
        (
            QUEST_FONT_STATE_CONDITIONAL_RESET_CALL_RUNTIME,
            K::Jal {
                target: QUEST_FONT_STATE_INITIALIZER_RUNTIME,
            },
        ),
    ];
    for (address, kind) in expected {
        require(instructions, address, kind)?;
    }

    let direct_calls = instructions
        .iter()
        .filter(|instruction| {
            instruction.kind
                == (K::Jal {
                    target: QUEST_FONT_STATE_INITIALIZER_RUNTIME,
                })
        })
        .map(|instruction| instruction.addr)
        .collect::<Vec<_>>();
    let expected_calls = [
        QUEST_FONT_STATE_BOOT_CALL_RUNTIME,
        QUEST_FONT_STATE_CONDITIONAL_RESET_CALL_RUNTIME,
    ];
    if direct_calls != expected_calls {
        return Err(error(format!(
            "QUEST font state initializer 직접 callsite 불일치: expected={expected_calls:?}, \
             actual={direct_calls:?}"
        )));
    }
    Ok(())
}

fn verify_descriptor_upload(
    map: &ExeMap,
    text: &[u8],
    instructions: &[Instruction],
) -> Result<(), QuestFontConsumerError> {
    let descriptor_index = usize::from(QUEST_FONT_INITIAL_RESOURCE_SELECTOR);
    let expected_descriptor = QUEST_FONT_DESCRIPTOR_ROOT_RUNTIME
        .checked_add((descriptor_index * QUEST_FONT_DESCRIPTOR_STRIDE) as u32)
        .ok_or_else(|| error("QUEST font descriptor 주소 overflow"))?;
    if expected_descriptor != QUEST_FONT_DESCRIPTOR_RUNTIME {
        return Err(error(format!(
            "QUEST font descriptor 산술 불일치: expected=0x{expected_descriptor:08X}, \
             contract=0x{QUEST_FONT_DESCRIPTOR_RUNTIME:08X}"
        )));
    }

    require_u32(
        map,
        text,
        QUEST_FONT_DESCRIPTOR_RUNTIME,
        QUEST_FONT_DESCRIPTOR_INITIAL_FLAGS,
        "초기 flags",
    )?;
    require_u32(
        map,
        text,
        QUEST_FONT_DESCRIPTOR_RUNTIME + QUEST_FONT_DESCRIPTOR_NAME_OFFSET as u32,
        QUEST_FONT_ASSET_NAME_RUNTIME,
        "자산 이름 pointer",
    )?;
    require_u32(
        map,
        text,
        QUEST_FONT_DESCRIPTOR_RUNTIME + QUEST_FONT_DESCRIPTOR_COMPRESSED_BYTES_OFFSET as u32,
        QUEST_FONT_COMPRESSED_BYTES as u32,
        "압축 byte 수",
    )?;
    require_u32(
        map,
        text,
        QUEST_FONT_DESCRIPTOR_RUNTIME + QUEST_FONT_DESCRIPTOR_DECODED_BYTES_OFFSET as u32,
        QUEST_FONT_SOURCE_BYTES as u32,
        "해제 byte 수",
    )?;
    require_u32(
        map,
        text,
        QUEST_FONT_DESCRIPTOR_RUNTIME + QUEST_FONT_DESCRIPTOR_SOURCE_OFFSET as u32,
        0,
        "초기 해제 RAM",
    )?;
    require_u32(
        map,
        text,
        QUEST_FONT_DESCRIPTOR_RUNTIME + QUEST_FONT_DESCRIPTOR_AUX_OFFSET as u32,
        0,
        "초기 보조 field",
    )?;
    require_u16(
        map,
        text,
        QUEST_FONT_DESCRIPTOR_RUNTIME + QUEST_FONT_DESCRIPTOR_VRAM_X_OFFSET as u32,
        0,
        "초기 VRAM x",
    )?;
    require_u16(
        map,
        text,
        QUEST_FONT_DESCRIPTOR_RUNTIME + QUEST_FONT_DESCRIPTOR_VRAM_Y_OFFSET as u32,
        0,
        "초기 VRAM y",
    )?;
    require_u32(
        map,
        text,
        QUEST_FONT_DESCRIPTOR_RUNTIME + QUEST_FONT_DESCRIPTOR_CELL_TABLE_OFFSET as u32,
        0,
        "초기 cell table",
    )?;
    require_u32(
        map,
        text,
        QUEST_FONT_DESCRIPTOR_RUNTIME + QUEST_FONT_DESCRIPTOR_PIXEL_FORMAT_OFFSET as u32,
        QUEST_FONT_PIXEL_FORMAT_INDEXED4,
        "pixel format",
    )?;
    require_u16(
        map,
        text,
        QUEST_FONT_CELL_TABLE_RUNTIME + 4,
        QUEST_FONT_ATLAS_WIDTH,
        "atlas width",
    )?;
    require_u16(
        map,
        text,
        QUEST_FONT_CELL_TABLE_RUNTIME + 6,
        QUEST_FONT_ATLAS_HEIGHT,
        "atlas height",
    )?;
    let source_bytes = usize::from(QUEST_FONT_VRAM_WIDTH_WORDS)
        .checked_mul(2)
        .and_then(|row| row.checked_mul(usize::from(QUEST_FONT_ATLAS_HEIGHT)))
        .ok_or_else(|| error("QUEST font VRAM byte 수 overflow"))?;
    if source_bytes != QUEST_FONT_SOURCE_BYTES {
        return Err(error(format!(
            "QUEST font source byte 수 불일치: expected={}, actual={source_bytes}",
            QUEST_FONT_SOURCE_BYTES
        )));
    }

    let expected = [
        (
            QUEST_FONT_DESCRIPTOR_BIND_RUNTIME,
            K::Lui {
                rt: Reg::V0,
                imm: (QUEST_FONT_DESCRIPTOR_ROOT_RUNTIME.wrapping_add(0x8000) >> 16) as u16,
            },
        ),
        (
            QUEST_FONT_DESCRIPTOR_BIND_RUNTIME + 4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_FONT_DESCRIPTOR_ROOT_RUNTIME as u16 as i16,
            },
        ),
        (
            QUEST_FONT_DESCRIPTOR_BIND_RUNTIME + 16,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: QUEST_FONT_DESCRIPTOR_ROOT_POINTERS_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8001_8948,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A0,
                sa: 3,
            },
        ),
        (
            0x8001_894C,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            0x8001_8954,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8001_8958,
            K::Addu {
                rd: Reg::S1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            QUEST_FONT_DESCRIPTOR_COPY_RUNTIME,
            K::Lw {
                rt: Reg::A2,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            QUEST_FONT_DESCRIPTOR_STAGING_STORE_RUNTIME - 12,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: QUEST_FONT_DESCRIPTOR_SOURCE_OFFSET,
            },
        ),
        (
            QUEST_FONT_DESCRIPTOR_STAGING_STORE_RUNTIME - 8,
            K::Lw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 4,
            },
        ),
        (
            QUEST_FONT_DESCRIPTOR_STAGING_STORE_RUNTIME,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: QUEST_FONT_DESCRIPTOR_SOURCE_OFFSET,
            },
        ),
        (
            QUEST_FONT_DESCRIPTOR_DECODED_STORE_RUNTIME - 28,
            K::Lw {
                rt: Reg::S1,
                base: Reg::S0,
                offset: 4,
            },
        ),
        (
            QUEST_FONT_DESCRIPTOR_DECODED_STORE_RUNTIME - 24,
            K::Jal {
                target: 0x8001_67E0,
            },
        ),
        (
            QUEST_FONT_DESCRIPTOR_DECODED_STORE_RUNTIME - 8,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x18,
            },
        ),
        (
            QUEST_FONT_DESCRIPTOR_DECODED_STORE_RUNTIME,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: QUEST_FONT_DESCRIPTOR_SOURCE_OFFSET,
            },
        ),
        (
            QUEST_FONT_UPLOAD_CALLBACK_RUNTIME + 16,
            K::Lw {
                rt: Reg::A1,
                base: Reg::S0,
                offset: 4,
            },
        ),
        (
            0x8001_BFBC,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_FONT_DESCRIPTOR_VRAM_X_OFFSET,
            },
        ),
        (
            0x8001_BFC8,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_FONT_DESCRIPTOR_VRAM_Y_OFFSET,
            },
        ),
        (
            0x8001_BFD4,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_FONT_DESCRIPTOR_CELL_TABLE_OFFSET,
            },
        ),
        (
            0x8001_BFFC,
            K::Lh {
                rt: Reg::V1,
                base: Reg::A1,
                offset: QUEST_FONT_DESCRIPTOR_PIXEL_FORMAT_OFFSET,
            },
        ),
        (
            0x8001_C040,
            K::Sra {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 18,
            },
        ),
        (
            0x8001_C064,
            K::Sh {
                rt: Reg::V0,
                base: Reg::SP,
                offset: 0x14,
            },
        ),
        (
            0x8001_C068,
            K::Lw {
                rt: Reg::A1,
                base: Reg::A1,
                offset: QUEST_FONT_DESCRIPTOR_SOURCE_OFFSET,
            },
        ),
        (
            QUEST_FONT_UPLOAD_CALL_RUNTIME,
            K::Jal {
                target: QUEST_FONT_UPLOAD_TARGET_RUNTIME,
            },
        ),
        (
            QUEST_FONT_UPLOAD_CALL_RUNTIME + 4,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::SP,
                imm: 0x10,
            },
        ),
        (
            QUEST_FONT_UPLOAD_SYNC_CALL_RUNTIME,
            K::Jal {
                target: QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME,
            },
        ),
        (
            QUEST_FONT_UPLOAD_SYNC_CALL_RUNTIME + 4,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            QUEST_FONT_UPLOAD_TARGET_RUNTIME + 20,
            K::Lui {
                rt: Reg::A0,
                imm: (QUEST_FONT_UPLOAD_NAME_RUNTIME.wrapping_add(0x8000) >> 16) as u16,
            },
        ),
        (
            QUEST_FONT_UPLOAD_TARGET_RUNTIME + 24,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: QUEST_FONT_UPLOAD_NAME_RUNTIME as u16 as i16,
            },
        ),
        (
            QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME + 32,
            K::Lui {
                rt: Reg::A0,
                imm: (QUEST_FONT_UPLOAD_SYNC_NAME_RUNTIME.wrapping_add(0x8000) >> 16) as u16,
            },
        ),
        (
            QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME + 36,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: QUEST_FONT_UPLOAD_SYNC_NAME_RUNTIME as u16 as i16,
            },
        ),
    ];
    for (address, kind) in expected {
        require(instructions, address, kind)?;
    }

    let upload_name = runtime_slice(
        map,
        text,
        QUEST_FONT_UPLOAD_NAME_RUNTIME,
        b"LoadImage\0".len(),
    )?;
    if upload_name != b"LoadImage\0" {
        return Err(error(format!(
            "QUEST font upload target 이름 불일치: expected=LoadImage, actual={upload_name:?}"
        )));
    }
    let sync_name = runtime_slice(
        map,
        text,
        QUEST_FONT_UPLOAD_SYNC_NAME_RUNTIME,
        b"DrawSync(%d)...\n\0".len(),
    )?;
    if sync_name != b"DrawSync(%d)...\n\0" {
        return Err(error(format!(
            "QUEST font upload sync 이름 불일치: \
             expected=DrawSync(%d)..., actual={sync_name:?}"
        )));
    }
    let asset_name = runtime_slice(
        map,
        text,
        QUEST_FONT_ASSET_NAME_RUNTIME,
        b"RPG_FNT1.BTX\0".len(),
    )?;
    if asset_name != b"RPG_FNT1.BTX\0" {
        return Err(error(format!(
            "QUEST font descriptor 자산 이름 불일치: \
             expected=RPG_FNT1.BTX, actual={asset_name:?}"
        )));
    }
    Ok(())
}

/// 지원 원본의 code→resource/cell lookup과 state 초기화 shape를 typed MIPS로 검증한다.
pub fn verify_box_quest_font_consumer(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestFontConsumerSummary, QuestFontConsumerError> {
    if map.name != "QUEST.EXE" {
        return Err(error(format!(
            "QUEST font consumer에 다른 EXE를 사용함: {}",
            map.name
        )));
    }
    if text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST font consumer text 길이 불일치: expected={}, actual={}",
            map.text_len,
            text.len()
        )));
    }
    let instructions =
        linear::sweep(map, text).ok_or_else(|| error("QUEST.EXE text가 4바이트 정렬이 아님"))?;
    verify_state_initializer(&instructions)?;
    verify_descriptor_upload(map, text, &instructions)?;

    require(
        &instructions,
        QUEST_GLYPH_RENDERER_RUNTIME,
        K::Addiu {
            rt: Reg::SP,
            rs: Reg::SP,
            imm: -32,
        },
    )?;
    require(
        &instructions,
        QUEST_GLYPH_RENDERER_RUNTIME + 4,
        K::Sw {
            rt: Reg::S0,
            base: Reg::SP,
            offset: 16,
        },
    )?;

    require(
        &instructions,
        QUEST_GLYPH_CODE_TABLE_LIMIT_RUNTIME,
        K::Addiu {
            rt: Reg::A3,
            rs: Reg::ZERO,
            imm: QUEST_CODE_TABLE_COUNT as i16,
        },
    )?;
    require(
        &instructions,
        QUEST_GLYPH_CODE_TABLE_POINTER_RUNTIME,
        K::Lui {
            rt: Reg::V0,
            imm: (QUEST_CODE_TABLE_RUNTIME >> 16) as u16,
        },
    )?;
    require(
        &instructions,
        QUEST_GLYPH_CODE_TABLE_POINTER_RUNTIME + 4,
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::V0,
            imm: QUEST_CODE_TABLE_RUNTIME as u16 as i16,
        },
    )?;
    require(
        &instructions,
        QUEST_GLYPH_FALLBACK_INDEX_RUNTIME,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::A3,
            imm: -1,
        },
    )?;
    require(
        &instructions,
        QUEST_GLYPH_CELL_INDEX_STORE_RUNTIME,
        K::Sh {
            rt: Reg::V0,
            base: Reg::A2,
            offset: QUEST_GLYPH_CELL_INDEX_OBJECT_OFFSET,
        },
    )?;
    require(
        &instructions,
        QUEST_GLYPH_RESOURCE_SELECTOR_LOAD_RUNTIME,
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A1,
            offset: QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET,
        },
    )?;
    require(
        &instructions,
        QUEST_GLYPH_CELL_TABLE_LOAD_RUNTIME,
        K::Lw {
            rt: Reg::V1,
            base: Reg::V1,
            offset: QUEST_GLYPH_CELL_TABLE_DESCRIPTOR_OFFSET,
        },
    )?;
    require(
        &instructions,
        QUEST_GLYPH_CELL_STRIDE_RUNTIME,
        K::Sll {
            rd: Reg::V0,
            rt: Reg::A0,
            sa: 1,
        },
    )?;
    require(
        &instructions,
        QUEST_GLYPH_CELL_STRIDE_RUNTIME + 4,
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::A0,
        },
    )?;
    require(
        &instructions,
        QUEST_GLYPH_CELL_STRIDE_RUNTIME + 8,
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    )?;
    require(
        &instructions,
        QUEST_GLYPH_NARROW_BOUNDARY_RUNTIME,
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: crate::font::QUEST_FONT_NARROW_SLOTS as i16,
        },
    )?;

    Ok(QuestFontConsumerSummary {
        renderer_runtime: QUEST_GLYPH_RENDERER_RUNTIME,
        code_table_runtime: QUEST_CODE_TABLE_RUNTIME,
        code_table_entries: QUEST_CODE_TABLE_COUNT,
        fallback_index: QUEST_GLYPH_FALLBACK_INDEX,
        cell_index_object_offset: QUEST_GLYPH_CELL_INDEX_OBJECT_OFFSET,
        resource_selector_state_offset: QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET,
        cell_table_descriptor_offset: QUEST_GLYPH_CELL_TABLE_DESCRIPTOR_OFFSET,
        cell_record_stride: QUEST_GLYPH_CELL_RECORD_STRIDE,
        narrow_boundary: crate::font::QUEST_FONT_NARROW_SLOTS,
        state_initializer_runtime: QUEST_FONT_STATE_INITIALIZER_RUNTIME,
        state_boot_call_runtime: QUEST_FONT_STATE_BOOT_CALL_RUNTIME,
        state_conditional_reset_call_runtime: QUEST_FONT_STATE_CONDITIONAL_RESET_CALL_RUNTIME,
        state_initializer_direct_calls: 2,
        state_base_runtime: QUEST_FONT_STATE_BASE_RUNTIME,
        state_record_count: QUEST_FONT_STATE_RECORD_COUNT,
        state_record_stride: QUEST_FONT_STATE_RECORD_STRIDE,
        state_bytes: QUEST_FONT_STATE_BYTES,
        initial_resource_selector: QUEST_FONT_INITIAL_RESOURCE_SELECTOR,
        descriptor_root_runtime: QUEST_FONT_DESCRIPTOR_ROOT_RUNTIME,
        descriptor_runtime: QUEST_FONT_DESCRIPTOR_RUNTIME,
        descriptor_stride: QUEST_FONT_DESCRIPTOR_STRIDE,
        descriptor_copy_runtime: QUEST_FONT_DESCRIPTOR_COPY_RUNTIME,
        source_field_initial: 0,
        asset_name_runtime: QUEST_FONT_ASSET_NAME_RUNTIME,
        compressed_bytes: QUEST_FONT_COMPRESSED_BYTES,
        source_bytes: QUEST_FONT_SOURCE_BYTES,
        source_staging_store_runtime: QUEST_FONT_DESCRIPTOR_STAGING_STORE_RUNTIME,
        source_decoded_store_runtime: QUEST_FONT_DESCRIPTOR_DECODED_STORE_RUNTIME,
        cell_table_runtime: QUEST_FONT_CELL_TABLE_RUNTIME,
        pixel_format: QUEST_FONT_PIXEL_FORMAT_INDEXED4,
        vram_width_words: QUEST_FONT_VRAM_WIDTH_WORDS,
        vram_height: QUEST_FONT_ATLAS_HEIGHT,
        upload_callback_runtime: QUEST_FONT_UPLOAD_CALLBACK_RUNTIME,
        upload_call_runtime: QUEST_FONT_UPLOAD_CALL_RUNTIME,
        upload_target_runtime: QUEST_FONT_UPLOAD_TARGET_RUNTIME,
        upload_sync_call_runtime: QUEST_FONT_UPLOAD_SYNC_CALL_RUNTIME,
        upload_sync_target_runtime: QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME,
    })
}

#[cfg(test)]
#[path = "quest_font_tests.rs"]
mod tests;
