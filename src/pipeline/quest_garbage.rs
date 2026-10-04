//! QUEST 필드와 방해뿌요 대기량 소비 경로의 정적 원본 계약.
//!
//! 지원 `QUEST.EXE`에서만 필드 배열, callback field pointer, 셀 좌표계와
//! 네이티브 incoming-garbage consumer를 검산한다. 에뮬레이터 메모리 쓰기는
//! 이 모듈의 책임이 아니며, 다른 EXE에서 확인한 주소나 정수 폭을 이식하지
//! 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::{psx_main_ram_offset, ExeMap};
use crate::isa::{decode, InstructionKind as K, Reg};

const QUEST_EXE: &str = "QUEST.EXE";

const FIELD_OBJECTS_RUNTIME: u32 = 0x800F_0D60;
const FIELD_OBJECT_STRIDE: u32 = 0x13A0;
const ALLOCATED_FIELD_OBJECTS: u8 = 4;
const CONTROLLED_BATTLE_FIELDS: u8 = 2;
const PLAYER_FIELD_INDEX: u8 = 0;
const OPPONENT_FIELD_INDEX: u8 = 1;
const CALLBACK_FIELD_POINTER_OFFSET: u32 = 0x00D4;

const CELL_TYPE_FIELD_OFFSET: u32 = 0x004C;
const CELL_BYTES: u32 = 0x0024;
const CELL_ROW_BYTES: u32 = 0x0120;
const INITIALIZED_CELL_COLUMNS: u8 = 8;
const INITIALIZED_CELL_ROWS: u8 = 16;
const PLAYABLE_COLUMN_FIRST: u8 = 1;
const PLAYABLE_COLUMN_LAST: u8 = 6;
const PLAYABLE_ROW_FIRST: u8 = 1;
const PLAYABLE_ROW_LAST: u8 = 13;
const EMPTY_CELL_TYPE: u8 = 0x0E;
const GARBAGE_CELL_TYPE: u8 = 0x05;
const WALL_CELL_TYPE: u8 = 0x0F;

const PENDING_GARBAGE_FIELD_OFFSET: u32 = 0x135C;
const PENDING_GARBAGE_BYTES: usize = 4;
const INCOMING_GARBAGE_CONSUMER_RUNTIME: u32 = 0x8004_47B8;
const GARBAGE_DROP_BATCH_LIMIT: u16 = 30;

/// 지원 원본에서 검산한 QUEST 방해뿌요 소비 경로.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestGarbageRoute {
    pub field_objects_runtime: u32,
    pub field_object_stride: u32,
    pub allocated_field_objects: u8,
    pub controlled_battle_fields: u8,
    pub player_field_runtime: u32,
    pub opponent_field_runtime: u32,
    pub callback_field_pointer_offset: u32,
    pub cell_type_field_offset: u32,
    pub cell_bytes: u32,
    pub cell_row_bytes: u32,
    pub initialized_cell_columns: u8,
    pub initialized_cell_rows: u8,
    pub playable_column_first: u8,
    pub playable_column_last: u8,
    pub playable_row_first: u8,
    pub playable_row_last: u8,
    pub empty_cell_type: u8,
    pub garbage_cell_type: u8,
    pub wall_cell_type: u8,
    pub pending_garbage_field_offset: u32,
    pub pending_garbage_bytes: usize,
    pub player_pending_garbage_runtime: u32,
    pub opponent_pending_garbage_runtime: u32,
    pub player_pending_garbage_ram_offset: usize,
    pub opponent_pending_garbage_ram_offset: usize,
    pub incoming_garbage_consumer_runtime: u32,
    pub garbage_drop_batch_limit: u16,
}

impl QuestGarbageRoute {
    /// 필드 배열 index와 초기화 격자 좌표를 셀 type runtime 주소로 바꾼다.
    pub fn cell_type_runtime(self, field_index: u8, column: u8, row: u8) -> Option<u32> {
        if field_index >= self.allocated_field_objects
            || column >= self.initialized_cell_columns
            || row >= self.initialized_cell_rows
        {
            return None;
        }
        self.field_objects_runtime
            .checked_add(u32::from(field_index) * self.field_object_stride)?
            .checked_add(self.cell_type_field_offset)?
            .checked_add(u32::from(row) * self.cell_row_bytes)?
            .checked_add(u32::from(column) * self.cell_bytes)
    }
}

/// QUEST 방해뿌요 정적 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGarbageRouteError(String);

impl fmt::Display for QuestGarbageRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestGarbageRouteError {}

fn error(message: impl Into<String>) -> QuestGarbageRouteError {
    QuestGarbageRouteError(message.into())
}

fn executable_word(map: &ExeMap, text: &[u8], runtime: u32) -> Result<u32, QuestGarbageRouteError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "QUEST 방해뿌요 runtime 주소가 EXE text 밖임: 0x{runtime:08X}"
        ))
    })?;
    let bytes = text.get(offset..offset + 4).ok_or_else(|| {
        error(format!(
            "QUEST 방해뿌요 EXE text 범위 이탈: 0x{runtime:08X}"
        ))
    })?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), QuestGarbageRouteError> {
    let actual = decode(runtime, executable_word(map, text, runtime)?);
    if actual.kind != expected {
        return Err(error(format!(
            "QUEST 방해뿌요 typed 명령 불일치: address=0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn field_layout_source_instructions() -> [(u32, K); 17] {
    [
        (
            0x8003_E6E0,
            K::Addiu {
                rt: Reg::T2,
                rs: Reg::ZERO,
                imm: 0x000E,
            },
        ),
        (
            0x8003_E6E8,
            K::Lw {
                rt: Reg::A2,
                base: Reg::A0,
                offset: 0x00D4,
            },
        ),
        (
            0x8003_E71C,
            K::Sb {
                rt: Reg::T2,
                base: Reg::V0,
                offset: 0x004C,
            },
        ),
        (
            0x8003_E734,
            K::Slti {
                rt: Reg::V0,
                rs: Reg::A0,
                imm: 8,
            },
        ),
        (
            0x8003_E73C,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x0024,
            },
        ),
        (
            0x8003_E740,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x000F,
            },
        ),
        (
            0x8003_E748,
            K::Sb {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 0x004C,
            },
        ),
        (
            0x8003_E74C,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x0120,
            },
        ),
        (
            0x8003_E754,
            K::Slti {
                rt: Reg::V0,
                rs: Reg::T0,
                imm: 0x0010,
            },
        ),
        (
            0x8003_E75C,
            K::Addiu {
                rt: Reg::A3,
                rs: Reg::A3,
                imm: 0x0120,
            },
        ),
        (
            0x8003_E794,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::A2,
                offset: 0x135C,
            },
        ),
        (
            0x8004_05C8,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800F,
            },
        ),
        (
            0x8004_05CC,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::V0,
                imm: 0x0D60,
            },
        ),
        (
            0x8004_05FC,
            K::Lw {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 0x135C,
            },
        ),
        (
            0x8004_0614,
            K::Sw {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 0x135C,
            },
        ),
        (
            0x8004_0634,
            K::Slti {
                rt: Reg::V0,
                rs: Reg::A3,
                imm: 4,
            },
        ),
        (
            0x8004_063C,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::A2,
                imm: 0x13A0,
            },
        ),
    ]
}

fn garbage_consumer_source_instructions() -> [(u32, K); 23] {
    [
        (
            INCOMING_GARBAGE_CONSUMER_RUNTIME,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -0x0070,
            },
        ),
        (
            0x8004_47E8,
            K::Lw {
                rt: Reg::S3,
                base: Reg::A0,
                offset: 0x00D4,
            },
        ),
        (
            0x8004_47F0,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S3,
                offset: 0x135C,
            },
        ),
        (
            0x8004_4804,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: 0x000E,
            },
        ),
        (
            0x8004_4810,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A0,
                sa: 3,
            },
        ),
        (
            0x8004_4814,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            0x8004_4818,
            K::Sll {
                rd: Reg::A1,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8004_4820,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V0,
                sa: 3,
            },
        ),
        (
            0x8004_4824,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8004_4828,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 5,
            },
        ),
        (
            0x8004_4834,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x004C,
            },
        ),
        (
            0x8004_484C,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x000E,
            },
        ),
        (
            0x8004_486C,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x0007,
            },
        ),
        (
            0x8004_4894,
            K::Addiu {
                rt: Reg::T0,
                rs: Reg::ZERO,
                imm: 0x0005,
            },
        ),
        (
            0x8004_48C8,
            K::Lhu {
                rt: Reg::S7,
                base: Reg::S3,
                offset: 0x135C,
            },
        ),
        (
            0x8004_48D0,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::S7,
                imm: 0x001F,
            },
        ),
        (
            0x8004_48DC,
            K::Addiu {
                rt: Reg::S7,
                rs: Reg::ZERO,
                imm: 0x001E,
            },
        ),
        (
            0x8004_48E0,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S3,
                offset: 0x135C,
            },
        ),
        (
            0x8004_48E4,
            K::Andi {
                rt: Reg::V1,
                rs: Reg::S7,
                imm: 0xFFFF,
            },
        ),
        (
            0x8004_48E8,
            K::Subu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8004_48EC,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S3,
                offset: 0x135C,
            },
        ),
        (
            0x8004_4BA4,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S3,
                offset: 0x135C,
            },
        ),
        (
            0x8004_4BB4,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S3,
                offset: 0x135C,
            },
        ),
    ]
}

fn verify_source_instructions(map: &ExeMap, text: &[u8]) -> Result<(), QuestGarbageRouteError> {
    for (runtime, expected) in field_layout_source_instructions()
        .into_iter()
        .chain(garbage_consumer_source_instructions())
    {
        require_instruction(map, text, runtime, expected)?;
    }
    Ok(())
}

/// 지원 `QUEST.EXE`에서 필드와 방해뿌요 대기량 소비 경로를 검증한다.
pub fn verify_quest_garbage_route(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestGarbageRoute, QuestGarbageRouteError> {
    if map.name != QUEST_EXE {
        return Err(error(format!(
            "QUEST 방해뿌요 검증에 다른 EXE를 받음: {}",
            map.name
        )));
    }
    verify_source_instructions(map, text)?;

    let player_field_runtime =
        FIELD_OBJECTS_RUNTIME + u32::from(PLAYER_FIELD_INDEX) * FIELD_OBJECT_STRIDE;
    let opponent_field_runtime =
        FIELD_OBJECTS_RUNTIME + u32::from(OPPONENT_FIELD_INDEX) * FIELD_OBJECT_STRIDE;
    let player_pending_garbage_runtime = player_field_runtime + PENDING_GARBAGE_FIELD_OFFSET;
    let opponent_pending_garbage_runtime = opponent_field_runtime + PENDING_GARBAGE_FIELD_OFFSET;
    let player_pending_garbage_ram_offset =
        psx_main_ram_offset(player_pending_garbage_runtime, PENDING_GARBAGE_BYTES)
            .ok_or_else(|| error("QUEST player 방해뿌요 주소가 PS1 main RAM 밖임"))?;
    let opponent_pending_garbage_ram_offset =
        psx_main_ram_offset(opponent_pending_garbage_runtime, PENDING_GARBAGE_BYTES)
            .ok_or_else(|| error("QUEST opponent 방해뿌요 주소가 PS1 main RAM 밖임"))?;

    Ok(QuestGarbageRoute {
        field_objects_runtime: FIELD_OBJECTS_RUNTIME,
        field_object_stride: FIELD_OBJECT_STRIDE,
        allocated_field_objects: ALLOCATED_FIELD_OBJECTS,
        controlled_battle_fields: CONTROLLED_BATTLE_FIELDS,
        player_field_runtime,
        opponent_field_runtime,
        callback_field_pointer_offset: CALLBACK_FIELD_POINTER_OFFSET,
        cell_type_field_offset: CELL_TYPE_FIELD_OFFSET,
        cell_bytes: CELL_BYTES,
        cell_row_bytes: CELL_ROW_BYTES,
        initialized_cell_columns: INITIALIZED_CELL_COLUMNS,
        initialized_cell_rows: INITIALIZED_CELL_ROWS,
        playable_column_first: PLAYABLE_COLUMN_FIRST,
        playable_column_last: PLAYABLE_COLUMN_LAST,
        playable_row_first: PLAYABLE_ROW_FIRST,
        playable_row_last: PLAYABLE_ROW_LAST,
        empty_cell_type: EMPTY_CELL_TYPE,
        garbage_cell_type: GARBAGE_CELL_TYPE,
        wall_cell_type: WALL_CELL_TYPE,
        pending_garbage_field_offset: PENDING_GARBAGE_FIELD_OFFSET,
        pending_garbage_bytes: PENDING_GARBAGE_BYTES,
        player_pending_garbage_runtime,
        opponent_pending_garbage_runtime,
        player_pending_garbage_ram_offset,
        opponent_pending_garbage_ram_offset,
        incoming_garbage_consumer_runtime: INCOMING_GARBAGE_CONSUMER_RUNTIME,
        garbage_drop_batch_limit: GARBAGE_DROP_BATCH_LIMIT,
    })
}

#[cfg(test)]
#[path = "quest_garbage_tests.rs"]
mod tests;
