//! GOCHA1 코스 결과의 이름 입력과 다음 상태 전이 경로.
//!
//! 이 모듈은 지원 `GOCHA1.EXE`에서 선택한 문자를 3칸 이름에 기록하고,
//! 확정·취소 입력이 현재 칸과 결과 상태를 바꾸는 명령을 typed MIPS로
//! 검산한다. 논리 버튼과 화면 도달은 별도 runtime 증거다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{decode, InstructionKind as K, Reg};

const GOCHA1_EXE: &str = "GOCHA1.EXE";

const INPUT_UPDATE_RUNTIME: u32 = 0x8003_0BC8;
const CONFIRM_MASK: u16 = 0x0020;
const ERASE_MASK: u16 = 0x0040;
const NAME_SLOTS: u8 = 3;
const SELECTED_CHARACTER_FIELD_OFFSET: u16 = 0x006A;
const NAME_CHARACTERS_FIELD_OFFSET: u16 = 0x0050;
const STATE_FIELD_OFFSET: u16 = 0x00D0;
const POSITION_FIELD_OFFSET: u16 = 0x00D4;
const CONFIRM_POSITION_STORE_RUNTIME: u32 = 0x8003_0C8C;
const STATE_INCREMENT_STORE_RUNTIME: u32 = 0x8003_0CA8;
const ERASE_POSITION_STORE_RUNTIME: u32 = 0x8003_0CE4;

/// 지원 원본에서 검산한 GOCHA1 결과 이름 입력 경로.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gocha1ResultNameEntry {
    pub input_update_runtime: u32,
    pub confirm_event_mask: u16,
    pub erase_event_mask: u16,
    pub name_slots: u8,
    pub selected_character_field_offset: u16,
    pub name_characters_field_offset: u16,
    pub state_field_offset: u16,
    pub position_field_offset: u16,
    pub confirm_position_store_runtime: u32,
    pub state_increment_store_runtime: u32,
    pub erase_position_store_runtime: u32,
}

/// GOCHA1 결과 이름 입력 정적 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gocha1ResultNameEntryError(String);

impl fmt::Display for Gocha1ResultNameEntryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Gocha1ResultNameEntryError {}

fn error(message: impl Into<String>) -> Gocha1ResultNameEntryError {
    Gocha1ResultNameEntryError(message.into())
}

fn executable_word(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
) -> Result<u32, Gocha1ResultNameEntryError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "GOCHA1 결과 이름 입력 runtime 주소가 EXE text 밖임: 0x{runtime:08X}"
        ))
    })?;
    let bytes = text.get(offset..offset + 4).ok_or_else(|| {
        error(format!(
            "GOCHA1 결과 이름 입력 EXE text 범위 이탈: 0x{runtime:08X}"
        ))
    })?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), Gocha1ResultNameEntryError> {
    let actual = decode(runtime, executable_word(map, text, runtime)?);
    if actual.kind != expected {
        return Err(error(format!(
            "GOCHA1 결과 이름 입력 typed 명령 불일치: address=0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn result_name_entry_source_instructions() -> [(u32, K); 26] {
    [
        (
            INPUT_UPDATE_RUNTIME,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S2,
                offset: 0x000C,
            },
        ),
        (
            0x8003_0C48,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: POSITION_FIELD_OFFSET as i16,
            },
        ),
        (
            0x8003_0C4C,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S1,
                offset: SELECTED_CHARACTER_FIELD_OFFSET as i16,
            },
        ),
        (
            0x8003_0C50,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8003_0C54,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::S1,
                rt: Reg::V0,
            },
        ),
        (
            0x8003_0C58,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: NAME_CHARACTERS_FIELD_OFFSET as i16,
            },
        ),
        (
            0x8003_0C5C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S2,
                offset: 0x000C,
            },
        ),
        (
            0x8003_0C64,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0x0004,
            },
        ),
        (
            0x8003_0C6C,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: CONFIRM_MASK,
            },
        ),
        (
            0x8003_0C70,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 14,
            },
        ),
        (
            0x8003_0C88,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
        ),
        (
            CONFIRM_POSITION_STORE_RUNTIME,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: POSITION_FIELD_OFFSET as i16,
            },
        ),
        (
            0x8003_0C90,
            K::Slti {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: NAME_SLOTS as i16,
            },
        ),
        (
            0x8003_0C94,
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 5,
            },
        ),
        (
            0x8003_0C9C,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::S1,
                offset: STATE_FIELD_OFFSET as i16,
            },
        ),
        (
            0x8003_0CA0,
            K::Sh {
                rt: Reg::ZERO,
                base: Reg::S1,
                offset: 0x003C,
            },
        ),
        (
            0x8003_0CA4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
        ),
        (
            STATE_INCREMENT_STORE_RUNTIME,
            K::Sh {
                rt: Reg::V0,
                base: Reg::S1,
                offset: STATE_FIELD_OFFSET as i16,
            },
        ),
        (
            0x8003_0CAC,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S2,
                offset: 0x000C,
            },
        ),
        (
            0x8003_0CB4,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0x0004,
            },
        ),
        (
            0x8003_0CBC,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: ERASE_MASK,
            },
        ),
        (
            0x8003_0CC0,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 180,
            },
        ),
        (
            0x8003_0CD0,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: POSITION_FIELD_OFFSET as i16,
            },
        ),
        (
            0x8003_0CD8,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 174,
            },
        ),
        (
            0x8003_0CDC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -1,
            },
        ),
        (
            ERASE_POSITION_STORE_RUNTIME,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: POSITION_FIELD_OFFSET as i16,
            },
        ),
    ]
}

/// 지원 `GOCHA1.EXE`에서 결과 이름 3칸의 확정·취소와 상태 전이를 검증한다.
pub fn verify_gocha1_result_name_entry(
    map: &ExeMap,
    text: &[u8],
) -> Result<Gocha1ResultNameEntry, Gocha1ResultNameEntryError> {
    if map.name != GOCHA1_EXE {
        return Err(error(format!(
            "GOCHA1 결과 이름 입력 검증에 다른 EXE를 받음: {}",
            map.name
        )));
    }
    for (runtime, expected) in result_name_entry_source_instructions() {
        require_instruction(map, text, runtime, expected)?;
    }

    Ok(Gocha1ResultNameEntry {
        input_update_runtime: INPUT_UPDATE_RUNTIME,
        confirm_event_mask: CONFIRM_MASK,
        erase_event_mask: ERASE_MASK,
        name_slots: NAME_SLOTS,
        selected_character_field_offset: SELECTED_CHARACTER_FIELD_OFFSET,
        name_characters_field_offset: NAME_CHARACTERS_FIELD_OFFSET,
        state_field_offset: STATE_FIELD_OFFSET,
        position_field_offset: POSITION_FIELD_OFFSET,
        confirm_position_store_runtime: CONFIRM_POSITION_STORE_RUNTIME,
        state_increment_store_runtime: STATE_INCREMENT_STORE_RUNTIME,
        erase_position_store_runtime: ERASE_POSITION_STORE_RUNTIME,
    })
}

#[cfg(test)]
#[path = "gocha1_result_name_entry_tests.rs"]
mod tests;
