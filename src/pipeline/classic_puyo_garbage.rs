//! 고전 PUYO1/PUYO2 필드와 방해뿌요 대기량 소비 경로의 정적 원본 계약.
//!
//! 에뮬레이터 메모리 쓰기는 이 모듈의 책임이 아니다. 지원 `PUYO1.EXE`와
//! `PUYO2.EXE`를 별도 spec으로 검증해 필드 객체 좌표계와 방해뿌요 생성
//! 루틴을 typed MIPS로 검산한다. 한 EXE의 주소는 다른 EXE에 이식하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::{psx_main_ram_offset, ExeMap};
use crate::isa::{decode, InstructionKind as K, Reg};

const PUYO1_EXE: &str = "PUYO1.EXE";
const PUYO2_EXE: &str = "PUYO2.EXE";

const FIELD_OBJECT_STRIDE: u32 = 0x139C;
const CONTROLLED_FIELD_OBJECTS: u8 = 2;
const PLAYER_FIELD_INDEX: u8 = 0;
const OPPONENT_FIELD_INDEX: u8 = 1;
const CALLBACK_FIELD_POINTER_OFFSET: u32 = 0x00E0;

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
const PENDING_GARBAGE_BYTES: usize = 2;
const GARBAGE_DROP_BATCH_LIMIT: u16 = 30;

#[derive(Debug, Clone, Copy)]
struct ClassicPuyoGarbageSpec {
    exe: &'static str,
    field_objects_runtime: u32,
    field_initializer_runtime: u32,
    field_selector_runtime: u32,
    incoming_garbage_consumer_runtime: u32,
}

const PUYO1_SPEC: ClassicPuyoGarbageSpec = ClassicPuyoGarbageSpec {
    exe: PUYO1_EXE,
    field_objects_runtime: 0x800C_9DB8,
    field_initializer_runtime: 0x8002_ECD4,
    field_selector_runtime: 0x8002_F020,
    incoming_garbage_consumer_runtime: 0x8004_994C,
};

const PUYO2_SPEC: ClassicPuyoGarbageSpec = ClassicPuyoGarbageSpec {
    exe: PUYO2_EXE,
    field_objects_runtime: 0x800C_AB08,
    field_initializer_runtime: 0x8002_8DC8,
    field_selector_runtime: 0x8002_9118,
    incoming_garbage_consumer_runtime: 0x8004_0184,
};

/// 지원 원본에서 검산한 고전 PUYO 방해뿌요 소비 경로.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassicPuyoGarbageRoute {
    pub exe: &'static str,
    pub field_objects_runtime: u32,
    pub field_object_stride: u32,
    pub controlled_field_objects: u8,
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

impl ClassicPuyoGarbageRoute {
    /// 필드 객체 index와 초기화 격자 좌표를 셀 type의 runtime 주소로 바꾼다.
    pub fn cell_type_runtime(self, field_index: u8, column: u8, row: u8) -> Option<u32> {
        if field_index >= self.controlled_field_objects
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

/// 고전 PUYO 방해뿌요 정적 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassicPuyoGarbageRouteError(String);

impl fmt::Display for ClassicPuyoGarbageRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for ClassicPuyoGarbageRouteError {}

fn error(message: impl Into<String>) -> ClassicPuyoGarbageRouteError {
    ClassicPuyoGarbageRouteError(message.into())
}

fn executable_word(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
) -> Result<u32, ClassicPuyoGarbageRouteError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "{} 방해뿌요 runtime 주소가 EXE text 밖임: 0x{runtime:08X}",
            map.name
        ))
    })?;
    let bytes = text.get(offset..offset + 4).ok_or_else(|| {
        error(format!(
            "{} 방해뿌요 EXE text 범위 이탈: 0x{runtime:08X}",
            map.name
        ))
    })?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), ClassicPuyoGarbageRouteError> {
    let actual = decode(runtime, executable_word(map, text, runtime)?);
    if actual.kind != expected {
        return Err(error(format!(
            "{} 방해뿌요 typed 명령 불일치: address=0x{runtime:08X}, expected={expected:?}, actual={:?}",
            map.name,
            actual.kind
        )));
    }
    Ok(())
}

fn field_layout_source_instructions(spec: ClassicPuyoGarbageSpec) -> [(u32, K); 23] {
    [
        (
            spec.field_initializer_runtime,
            K::Addiu {
                rt: Reg::T2,
                rs: Reg::ZERO,
                imm: 0x000E,
            },
        ),
        (
            spec.field_initializer_runtime + 0x08,
            K::Lw {
                rt: Reg::A2,
                base: Reg::A0,
                offset: 0x00E0,
            },
        ),
        (
            spec.field_initializer_runtime + 0x3C,
            K::Sb {
                rt: Reg::T2,
                base: Reg::V0,
                offset: 0x004C,
            },
        ),
        (
            spec.field_initializer_runtime + 0x54,
            K::Slti {
                rt: Reg::V0,
                rs: Reg::A0,
                imm: 8,
            },
        ),
        (
            spec.field_initializer_runtime + 0x5C,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x0024,
            },
        ),
        (
            spec.field_initializer_runtime + 0x60,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x000F,
            },
        ),
        (
            spec.field_initializer_runtime + 0x68,
            K::Sb {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 0x004C,
            },
        ),
        (
            spec.field_initializer_runtime + 0x6C,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x0120,
            },
        ),
        (
            spec.field_initializer_runtime + 0x74,
            K::Slti {
                rt: Reg::V0,
                rs: Reg::T0,
                imm: 0x0010,
            },
        ),
        (
            spec.field_initializer_runtime + 0x7C,
            K::Addiu {
                rt: Reg::A3,
                rs: Reg::A3,
                imm: 0x0120,
            },
        ),
        (
            spec.field_initializer_runtime + 0xB4,
            K::Sh {
                rt: Reg::ZERO,
                base: Reg::A2,
                offset: 0x135C,
            },
        ),
        (
            spec.field_selector_runtime,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::S2,
                sa: 6,
            },
        ),
        (
            spec.field_selector_runtime + 0x04,
            K::Subu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::S2,
            },
        ),
        (
            spec.field_selector_runtime + 0x08,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            spec.field_selector_runtime + 0x0C,
            K::Subu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::S2,
            },
        ),
        (
            spec.field_selector_runtime + 0x10,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            spec.field_selector_runtime + 0x14,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            spec.field_selector_runtime + 0x18,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            spec.field_selector_runtime + 0x1C,
            K::Lui {
                rt: Reg::V1,
                imm: 0x800D,
            },
        ),
        (
            spec.field_selector_runtime + 0x20,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: spec.field_objects_runtime as i16,
            },
        ),
        (
            spec.field_selector_runtime + 0x24,
            K::Addu {
                rd: Reg::S1,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            spec.field_selector_runtime + 0x70,
            K::Sw {
                rt: Reg::S1,
                base: Reg::S3,
                offset: 0x00E0,
            },
        ),
        (
            spec.field_selector_runtime + 0x74,
            K::Sb {
                rt: Reg::FP,
                base: Reg::S1,
                offset: 0,
            },
        ),
    ]
}

fn garbage_consumer_source_instructions(spec: ClassicPuyoGarbageSpec) -> [(u32, K); 21] {
    [
        (
            spec.incoming_garbage_consumer_runtime,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -0x0068,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x30,
            K::Lw {
                rt: Reg::S2,
                base: Reg::A0,
                offset: 0x00E0,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x38,
            K::Lh {
                rt: Reg::V0,
                base: Reg::S2,
                offset: 0x135C,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x4C,
            K::Addiu {
                rt: Reg::A3,
                rs: Reg::ZERO,
                imm: 0x000E,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x58,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A0,
                sa: 3,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x5C,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x60,
            K::Sll {
                rd: Reg::A1,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x68,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V0,
                sa: 3,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x6C,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x70,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 5,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x7C,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x004C,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x94,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x000E,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0xB4,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x0007,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0xDC,
            K::Addiu {
                rt: Reg::T0,
                rs: Reg::ZERO,
                imm: 0x0005,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x110,
            K::Lhu {
                rt: Reg::S6,
                base: Reg::S2,
                offset: 0x135C,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x118,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::S6,
                imm: 0x001F,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x124,
            K::Addiu {
                rt: Reg::S6,
                rs: Reg::ZERO,
                imm: 0x001E,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x128,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::S2,
                offset: 0x135C,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x130,
            K::Subu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::S6,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x138,
            K::Sh {
                rt: Reg::V0,
                base: Reg::S2,
                offset: 0x135C,
            },
        ),
        (
            spec.incoming_garbage_consumer_runtime + 0x290,
            K::Sb {
                rt: Reg::T0,
                base: Reg::V1,
                offset: 0x004C,
            },
        ),
    ]
}

fn verify_source_instructions(
    map: &ExeMap,
    text: &[u8],
    spec: ClassicPuyoGarbageSpec,
) -> Result<(), ClassicPuyoGarbageRouteError> {
    for (runtime, expected) in field_layout_source_instructions(spec)
        .into_iter()
        .chain(garbage_consumer_source_instructions(spec))
    {
        require_instruction(map, text, runtime, expected)?;
    }
    Ok(())
}

fn verify_classic_puyo_garbage_route(
    map: &ExeMap,
    text: &[u8],
) -> Result<ClassicPuyoGarbageRoute, ClassicPuyoGarbageRouteError> {
    let spec = match map.name {
        PUYO1_EXE => PUYO1_SPEC,
        PUYO2_EXE => PUYO2_SPEC,
        _ => {
            return Err(error(format!(
                "고전 PUYO 방해뿌요 검증에 다른 EXE를 받음: {}",
                map.name
            )))
        }
    };
    if map.name != spec.exe {
        return Err(error(format!(
            "고전 PUYO 방해뿌요 spec과 EXE 불일치: {} != {}",
            spec.exe, map.name
        )));
    }
    verify_source_instructions(map, text, spec)?;

    let player_field_runtime =
        spec.field_objects_runtime + u32::from(PLAYER_FIELD_INDEX) * FIELD_OBJECT_STRIDE;
    let opponent_field_runtime =
        spec.field_objects_runtime + u32::from(OPPONENT_FIELD_INDEX) * FIELD_OBJECT_STRIDE;
    let player_pending_garbage_runtime = player_field_runtime + PENDING_GARBAGE_FIELD_OFFSET;
    let opponent_pending_garbage_runtime = opponent_field_runtime + PENDING_GARBAGE_FIELD_OFFSET;
    let player_pending_garbage_ram_offset =
        psx_main_ram_offset(player_pending_garbage_runtime, PENDING_GARBAGE_BYTES).ok_or_else(
            || {
                error(format!(
                    "{} player 방해뿌요 주소가 PS1 main RAM 밖임",
                    spec.exe
                ))
            },
        )?;
    let opponent_pending_garbage_ram_offset =
        psx_main_ram_offset(opponent_pending_garbage_runtime, PENDING_GARBAGE_BYTES).ok_or_else(
            || {
                error(format!(
                    "{} opponent 방해뿌요 주소가 PS1 main RAM 밖임",
                    spec.exe
                ))
            },
        )?;

    Ok(ClassicPuyoGarbageRoute {
        exe: spec.exe,
        field_objects_runtime: spec.field_objects_runtime,
        field_object_stride: FIELD_OBJECT_STRIDE,
        controlled_field_objects: CONTROLLED_FIELD_OBJECTS,
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
        incoming_garbage_consumer_runtime: spec.incoming_garbage_consumer_runtime,
        garbage_drop_batch_limit: GARBAGE_DROP_BATCH_LIMIT,
    })
}

/// 지원 `PUYO1.EXE`에서 필드와 방해뿌요 대기량 소비 경로를 검증한다.
pub fn verify_puyo1_garbage_route(
    map: &ExeMap,
    text: &[u8],
) -> Result<ClassicPuyoGarbageRoute, ClassicPuyoGarbageRouteError> {
    if map.name != PUYO1_EXE {
        return Err(error(format!(
            "PUYO1 방해뿌요 검증에 다른 EXE를 받음: {}",
            map.name
        )));
    }
    verify_classic_puyo_garbage_route(map, text)
}

/// 지원 `PUYO2.EXE`에서 필드와 방해뿌요 대기량 소비 경로를 검증한다.
pub fn verify_puyo2_garbage_route(
    map: &ExeMap,
    text: &[u8],
) -> Result<ClassicPuyoGarbageRoute, ClassicPuyoGarbageRouteError> {
    if map.name != PUYO2_EXE {
        return Err(error(format!(
            "PUYO2 방해뿌요 검증에 다른 EXE를 받음: {}",
            map.name
        )));
    }
    verify_classic_puyo_garbage_route(map, text)
}

#[cfg(test)]
#[path = "classic_puyo_garbage_tests.rs"]
mod tests;
