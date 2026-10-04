//! GOCHA1 필드와 방해뿌요 대기량 소비 경로의 정적 원본 계약.
//!
//! 에뮬레이터 메모리 쓰기는 이 모듈의 책임이 아니다. 이 모듈은 지원
//! `GOCHA1.EXE`에서 필드 객체 배열, 셀 레코드 좌표계, 방해뿌요 type과
//! 대기량 소비 루틴을 typed MIPS로 검산하고 관측에 쓸 주소만 돌려준다.

use std::error::Error;
use std::fmt;

use crate::coords::{psx_main_ram_offset, ExeMap};
use crate::isa::{decode, InstructionKind as K, Reg};

const GOCHA1_EXE: &str = "GOCHA1.EXE";

const FIELD_OBJECTS_RUNTIME: u32 = 0x800D_1B20;
const FIELD_OBJECT_STRIDE: u32 = 0x1418;
const FIELD_OBJECTS: u8 = 4;
const PLAYER_FIELD_INDEX: u8 = 0;
const OPPONENT_FIELD_INDEX: u8 = 1;

const CELL_TYPE_FIELD_OFFSET: u32 = 0x003C;
const CELL_BYTES: u32 = 0x0024;
const CELL_ROW_BYTES: u32 = 0x0120;
const INITIALIZED_CELL_COLUMNS: u8 = 8;
const INITIALIZED_CELL_ROWS: u8 = 16;
const EMPTY_CELL_TYPE: u8 = 0x0E;
const GARBAGE_CELL_TYPE: u8 = 0x05;
const WALL_CELL_TYPE: u8 = 0x0F;

const PENDING_GARBAGE_FIELD_OFFSET: u32 = 0x1344;
const INCOMING_GARBAGE_CONSUMER_RUNTIME: u32 = 0x8003_73B4;
const SHARED_CELL_TYPE_WRITER_RUNTIME: u32 = 0x8003_5D70;
const GARBAGE_DROP_BATCH_LIMIT: u16 = 30;

/// 지원 원본에서 검산한 GOCHA1 방해뿌요 소비 경로.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gocha1GarbageRoute {
    pub field_objects_runtime: u32,
    pub field_object_stride: u32,
    pub field_objects: u8,
    pub player_field_runtime: u32,
    pub opponent_field_runtime: u32,
    pub cell_type_field_offset: u32,
    pub cell_bytes: u32,
    pub cell_row_bytes: u32,
    pub initialized_cell_columns: u8,
    pub initialized_cell_rows: u8,
    pub empty_cell_type: u8,
    pub garbage_cell_type: u8,
    pub wall_cell_type: u8,
    pub pending_garbage_field_offset: u32,
    pub player_pending_garbage_runtime: u32,
    pub opponent_pending_garbage_runtime: u32,
    pub player_pending_garbage_ram_offset: usize,
    pub opponent_pending_garbage_ram_offset: usize,
    pub incoming_garbage_consumer_runtime: u32,
    pub shared_cell_type_writer_runtime: u32,
    pub garbage_drop_batch_limit: u16,
}

impl Gocha1GarbageRoute {
    /// 필드 객체 index와 셀 좌표를 셀 type의 runtime 주소로 바꾼다.
    pub fn cell_type_runtime(self, field_index: u8, column: u8, row: u8) -> Option<u32> {
        if field_index >= self.field_objects
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

/// GOCHA1 방해뿌요 정적 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gocha1GarbageRouteError(String);

impl fmt::Display for Gocha1GarbageRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Gocha1GarbageRouteError {}

fn error(message: impl Into<String>) -> Gocha1GarbageRouteError {
    Gocha1GarbageRouteError(message.into())
}

fn executable_word(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
) -> Result<u32, Gocha1GarbageRouteError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "GOCHA1 방해뿌요 runtime 주소가 EXE text 밖임: 0x{runtime:08X}"
        ))
    })?;
    let bytes = text.get(offset..offset + 4).ok_or_else(|| {
        error(format!(
            "GOCHA1 방해뿌요 EXE text 범위 이탈: 0x{runtime:08X}"
        ))
    })?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), Gocha1GarbageRouteError> {
    let actual = decode(runtime, executable_word(map, text, runtime)?);
    if actual.kind != expected {
        return Err(error(format!(
            "GOCHA1 방해뿌요 typed 명령 불일치: address=0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn field_layout_source_instructions() -> [(u32, K); 16] {
    [
        (
            0x8003_30BC,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800D,
            },
        ),
        (
            0x8003_30C0,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::V0,
                imm: 0x1B20,
            },
        ),
        (
            0x8003_3130,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x1418,
            },
        ),
        (
            0x8003_3128,
            K::Slti {
                rt: Reg::V0,
                rs: Reg::A1,
                imm: 4,
            },
        ),
        (
            0x8002_CF64,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 0x000E,
            },
        ),
        (
            0x8002_CF68,
            K::Sb {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x003C,
            },
        ),
        (
            0x8002_CFBC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x0005,
            },
        ),
        (
            0x8002_CFC0,
            K::Sb {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x003C,
            },
        ),
        (
            0x8002_CFEC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x000F,
            },
        ),
        (
            0x8002_CFF8,
            K::Addiu {
                rt: Reg::T0,
                rs: Reg::T0,
                imm: 0x0120,
            },
        ),
        (
            0x8002_CFE8,
            K::Addiu {
                rt: Reg::A3,
                rs: Reg::A3,
                imm: 0x0024,
            },
        ),
        (
            0x8002_CFE0,
            K::Slti {
                rt: Reg::V0,
                rs: Reg::A0,
                imm: 8,
            },
        ),
        (
            0x8002_CFF0,
            K::Sb {
                rt: Reg::V0,
                base: Reg::T0,
                offset: 0x0138,
            },
        ),
        (
            0x8002_CFF4,
            K::Sb {
                rt: Reg::V0,
                base: Reg::T0,
                offset: 0x003C,
            },
        ),
        (
            0x8002_D000,
            K::Slti {
                rt: Reg::V0,
                rs: Reg::T1,
                imm: 0x0010,
            },
        ),
        (
            SHARED_CELL_TYPE_WRITER_RUNTIME,
            K::Sb {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x003C,
            },
        ),
    ]
}

fn verify_field_layout_source(map: &ExeMap, text: &[u8]) -> Result<(), Gocha1GarbageRouteError> {
    for (runtime, expected) in field_layout_source_instructions() {
        require_instruction(map, text, runtime, expected)?;
    }
    Ok(())
}

fn pending_garbage_source_instructions() -> [(u32, K); 18] {
    [
        (
            0x8002_D050,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::A2,
                offset: 0x1344,
            },
        ),
        (
            INCOMING_GARBAGE_CONSUMER_RUNTIME,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -0x0060,
            },
        ),
        (
            0x8003_73E4,
            K::Lw {
                rt: Reg::S1,
                base: Reg::A0,
                offset: 0x00E0,
            },
        ),
        (
            0x8003_73EC,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1344,
            },
        ),
        (
            0x8003_7400,
            K::Addiu {
                rt: Reg::A3,
                rs: Reg::ZERO,
                imm: 0x000E,
            },
        ),
        (
            0x8003_740C,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A0,
                sa: 3,
            },
        ),
        (
            0x8003_7410,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            0x8003_7414,
            K::Sll {
                rd: Reg::A1,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8003_741C,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V0,
                sa: 3,
            },
        ),
        (
            0x8003_7420,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8003_7424,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 5,
            },
        ),
        (
            0x8003_7430,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x003C,
            },
        ),
        (
            0x8003_7500,
            K::Lhu {
                rt: Reg::S7,
                base: Reg::S1,
                offset: 0x1344,
            },
        ),
        (
            0x8003_750C,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x001F,
            },
        ),
        (
            0x8003_7518,
            K::Addiu {
                rt: Reg::S7,
                rs: Reg::ZERO,
                imm: 0x001E,
            },
        ),
        (
            0x8003_751C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1344,
            },
        ),
        (
            0x8003_7524,
            K::Subu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8003_7528,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1344,
            },
        ),
    ]
}

fn verify_pending_garbage_source(map: &ExeMap, text: &[u8]) -> Result<(), Gocha1GarbageRouteError> {
    for (runtime, expected) in pending_garbage_source_instructions() {
        require_instruction(map, text, runtime, expected)?;
    }
    Ok(())
}

/// 지원 `GOCHA1.EXE`에서 필드와 방해뿌요 대기량 소비 경로를 검증한다.
pub fn verify_gocha1_garbage_route(
    map: &ExeMap,
    text: &[u8],
) -> Result<Gocha1GarbageRoute, Gocha1GarbageRouteError> {
    if map.name != GOCHA1_EXE {
        return Err(error(format!(
            "GOCHA1 방해뿌요 검증에 다른 EXE를 받음: {}",
            map.name
        )));
    }
    verify_field_layout_source(map, text)?;
    verify_pending_garbage_source(map, text)?;

    let player_field_runtime =
        FIELD_OBJECTS_RUNTIME + u32::from(PLAYER_FIELD_INDEX) * FIELD_OBJECT_STRIDE;
    let opponent_field_runtime =
        FIELD_OBJECTS_RUNTIME + u32::from(OPPONENT_FIELD_INDEX) * FIELD_OBJECT_STRIDE;
    let player_pending_garbage_runtime = player_field_runtime + PENDING_GARBAGE_FIELD_OFFSET;
    let opponent_pending_garbage_runtime = opponent_field_runtime + PENDING_GARBAGE_FIELD_OFFSET;
    let player_pending_garbage_ram_offset = psx_main_ram_offset(player_pending_garbage_runtime, 4)
        .ok_or_else(|| error("GOCHA1 player 방해뿌요 주소가 PS1 main RAM 밖임"))?;
    let opponent_pending_garbage_ram_offset =
        psx_main_ram_offset(opponent_pending_garbage_runtime, 4)
            .ok_or_else(|| error("GOCHA1 opponent 방해뿌요 주소가 PS1 main RAM 밖임"))?;

    Ok(Gocha1GarbageRoute {
        field_objects_runtime: FIELD_OBJECTS_RUNTIME,
        field_object_stride: FIELD_OBJECT_STRIDE,
        field_objects: FIELD_OBJECTS,
        player_field_runtime,
        opponent_field_runtime,
        cell_type_field_offset: CELL_TYPE_FIELD_OFFSET,
        cell_bytes: CELL_BYTES,
        cell_row_bytes: CELL_ROW_BYTES,
        initialized_cell_columns: INITIALIZED_CELL_COLUMNS,
        initialized_cell_rows: INITIALIZED_CELL_ROWS,
        empty_cell_type: EMPTY_CELL_TYPE,
        garbage_cell_type: GARBAGE_CELL_TYPE,
        wall_cell_type: WALL_CELL_TYPE,
        pending_garbage_field_offset: PENDING_GARBAGE_FIELD_OFFSET,
        player_pending_garbage_runtime,
        opponent_pending_garbage_runtime,
        player_pending_garbage_ram_offset,
        opponent_pending_garbage_ram_offset,
        incoming_garbage_consumer_runtime: INCOMING_GARBAGE_CONSUMER_RUNTIME,
        shared_cell_type_writer_runtime: SHARED_CELL_TYPE_WRITER_RUNTIME,
        garbage_drop_batch_limit: GARBAGE_DROP_BATCH_LIMIT,
    })
}

#[cfg(test)]
#[path = "gocha1_garbage_tests.rs"]
mod tests;
