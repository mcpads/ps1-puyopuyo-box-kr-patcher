//! QUEST `RPG_SYS1.BTX`의 level-up baked label 소비 계약.
//!
//! 지원 `QUEST.EXE`의 level-up object writer와 공용 short renderer를
//! 검산한다. object의 cell selector `95`는 renderer가 12바이트 뒤 레코드를
//! 읽으므로 table record index `96`에 연결된다. descriptor pointer와 cell
//! table은 실행 중 초기화되므로 EXE 원본의 zero 영역에서 검증하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{decode, InstructionKind as K, Reg};

const QUEST_EXE: &str = "QUEST.EXE";
const RPG_SYS1_RESOURCE_ID: u16 = 2;
const CELL_RECORD_BYTES: u32 = 12;
const LEVEL_UP_CELL_SELECTOR: u16 = 95;
const LEVEL_UP_CELL_RECORD_INDEX: u16 = LEVEL_UP_CELL_SELECTOR + 1;
const LEVEL_UP_OBJECT_WRITER_RUNTIME: u32 = 0x8007_68DC;
const SHORT_RENDERER_RUNTIME: u32 = 0x8002_384C;
const LEVEL_UP_RECORD_READ_RUNTIME: u32 = 0x8002_38D4;

/// 지원 원본에서 검산한 `RPG_SYS1` level-up draw source 경로.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestRpgSys1LevelUpRoute {
    pub resource_id: u16,
    pub cell_record_bytes: u32,
    pub level_up_cell_selector: u16,
    pub level_up_cell_record_index: u16,
    pub level_up_object_writer_runtime: u32,
    pub short_renderer_runtime: u32,
    pub level_up_record_read_runtime: u32,
}

/// `RPG_SYS1` level-up 소비 계약 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestRpgSys1LevelUpError(String);

impl fmt::Display for QuestRpgSys1LevelUpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestRpgSys1LevelUpError {}

fn error(message: impl Into<String>) -> QuestRpgSys1LevelUpError {
    QuestRpgSys1LevelUpError(message.into())
}

fn executable_bytes<'a>(
    map: &ExeMap,
    text: &'a [u8],
    runtime: u32,
    length: usize,
) -> Result<&'a [u8], QuestRpgSys1LevelUpError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "RPG_SYS1 runtime 주소가 QUEST.EXE text 밖임: 0x{runtime:08X}"
        ))
    })?;
    text.get(offset..offset + length).ok_or_else(|| {
        error(format!(
            "RPG_SYS1 QUEST.EXE text 범위 이탈: 0x{runtime:08X}+0x{length:X}"
        ))
    })
}

fn executable_word(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
) -> Result<u32, QuestRpgSys1LevelUpError> {
    Ok(u32::from_le_bytes(
        executable_bytes(map, text, runtime, 4)?
            .try_into()
            .expect("4바이트 word"),
    ))
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), QuestRpgSys1LevelUpError> {
    let actual = decode(runtime, executable_word(map, text, runtime)?);
    if actual.kind != expected {
        return Err(error(format!(
            "RPG_SYS1 level-up typed 명령 불일치: address=0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn level_up_source_instructions() -> [(u32, K); 22] {
    [
        (
            SHORT_RENDERER_RUNTIME,
            K::Addiu {
                rt: Reg::SP,
                rs: Reg::SP,
                imm: -0x0030,
            },
        ),
        (
            0x8002_3854,
            K::Addu {
                rd: Reg::S5,
                rs: Reg::A0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8002_3884,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S5,
                offset: 0x0068,
            },
        ),
        (
            0x8002_388C,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V1,
                sa: 3,
            },
        ),
        (
            0x8002_3890,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8002_3894,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8002_3898,
            K::Lw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: -0x4CD8,
            },
        ),
        (
            0x8002_389C,
            K::Lhu {
                rt: Reg::A0,
                base: Reg::S5,
                offset: 0x0066,
            },
        ),
        (
            0x8002_38A4,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::A0,
                sa: 1,
            },
        ),
        (
            0x8002_38A8,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            0x8002_38AC,
            K::Lw {
                rt: Reg::V1,
                base: Reg::S4,
                offset: 0x001C,
            },
        ),
        (
            0x8002_38B0,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8002_38BC,
            K::Addiu {
                rt: Reg::S3,
                rs: Reg::S6,
                imm: 0x000C,
            },
        ),
        (
            LEVEL_UP_RECORD_READ_RUNTIME,
            K::Lh {
                rt: Reg::V1,
                base: Reg::S3,
                offset: 0x0008,
            },
        ),
        (
            0x8002_38EC,
            K::Lh {
                rt: Reg::V1,
                base: Reg::S3,
                offset: 0x000A,
            },
        ),
        (
            LEVEL_UP_OBJECT_WRITER_RUNTIME,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8008,
            },
        ),
        (
            0x8007_68E0,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: -0x687C,
            },
        ),
        (
            0x8007_68F4,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: RPG_SYS1_RESOURCE_ID as i16,
            },
        ),
        (
            0x8007_68F8,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x0068,
            },
        ),
        (
            0x8007_68FC,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: LEVEL_UP_CELL_SELECTOR as i16,
            },
        ),
        (
            0x8007_6900,
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x0066,
            },
        ),
        (
            0x8007_6904,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 0x06E0,
            },
        ),
    ]
}

/// 지원 `QUEST.EXE`에서 `RPG_SYS1` level-up object→cell draw source를 검증한다.
pub fn verify_quest_rpg_sys1_level_up(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestRpgSys1LevelUpRoute, QuestRpgSys1LevelUpError> {
    if map.name != QUEST_EXE {
        return Err(error(format!(
            "RPG_SYS1 level-up 검증에 다른 EXE를 받음: {}",
            map.name
        )));
    }
    for (runtime, expected) in level_up_source_instructions() {
        require_instruction(map, text, runtime, expected)?;
    }

    Ok(QuestRpgSys1LevelUpRoute {
        resource_id: RPG_SYS1_RESOURCE_ID,
        cell_record_bytes: CELL_RECORD_BYTES,
        level_up_cell_selector: LEVEL_UP_CELL_SELECTOR,
        level_up_cell_record_index: LEVEL_UP_CELL_RECORD_INDEX,
        level_up_object_writer_runtime: LEVEL_UP_OBJECT_WRITER_RUNTIME,
        short_renderer_runtime: SHORT_RENDERER_RUNTIME,
        level_up_record_read_runtime: LEVEL_UP_RECORD_READ_RUNTIME,
    })
}

#[cfg(test)]
#[path = "quest_rpg_sys1_level_up_tests.rs"]
mod tests;
