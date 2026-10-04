//! QUEST `OP83` child actor 구성 handler.
//!
//! handler는 parent `+0x34`의 child를 대상으로 subtype 0..5를 고른다.
//! 이 모듈은 SUKE 조우가 쓰는 subtype 0(절대 위치), 2(접촉 영역),
//! 3(상대 위치·속도)과 공통 12바이트 전진을 검증한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::quest_seq_opcode_handler;

use super::linear;

pub const QUEST_SEQ_CHILD_CONFIGURATION_OPCODE: u8 = 0x83;
pub const QUEST_SEQ_CHILD_CONFIGURATION_HANDLER_RUNTIME: u32 = 0x8007_33A4;
pub const QUEST_SEQ_CHILD_CONFIGURATION_SUBTYPE_LIMIT: u8 = 6;
pub const QUEST_SEQ_CHILD_CONFIGURATION_TABLE_RUNTIME: u32 = 0x8001_3038;
pub const QUEST_SEQ_CHILD_CONFIGURATION_RECORD_BYTES: usize = 12;
pub const QUEST_SEQ_CHILD_POINTER_FIELD_OFFSET: i16 = 0x34;
pub const QUEST_SEQ_CHILD_POSITION_ARM_RUNTIME: u32 = 0x8007_33F0;
pub const QUEST_SEQ_CHILD_BOUNDS_ARM_RUNTIME: u32 = 0x8007_34B8;
pub const QUEST_SEQ_CHILD_MOTION_ARM_RUNTIME: u32 = 0x8007_3508;
pub const QUEST_SEQ_CHILD_FIXED_POINT_SHIFT: u8 = 12;

const JUMP_TABLE: [u32; QUEST_SEQ_CHILD_CONFIGURATION_SUBTYPE_LIMIT as usize] = [
    QUEST_SEQ_CHILD_POSITION_ARM_RUNTIME,
    0x8007_3444,
    QUEST_SEQ_CHILD_BOUNDS_ARM_RUNTIME,
    QUEST_SEQ_CHILD_MOTION_ARM_RUNTIME,
    0x8007_3574,
    0x8007_3594,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqChildConfigurationSummary {
    pub opcode: u8,
    pub handler_runtime: u32,
    pub subtype_limit: u8,
    pub table_runtime: u32,
    pub record_bytes: usize,
    pub child_pointer_field_offset: i16,
    pub position_arm_runtime: u32,
    pub bounds_arm_runtime: u32,
    pub motion_arm_runtime: u32,
    pub fixed_point_shift: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqChildConfigurationError(String);

impl fmt::Display for QuestSeqChildConfigurationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestSeqChildConfigurationError {}

fn error(message: impl Into<String>) -> QuestSeqChildConfigurationError {
    QuestSeqChildConfigurationError(message.into())
}

fn required_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        (
            0x8007_33B8,
            "lw stream, 0x1C(parent)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            0x8007_33C0,
            "lbu subtype, 1(stream)",
            K::Lbu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            0x8007_33C4,
            "lw child, 0x34(parent)",
            K::Lw {
                rt: Reg::S0,
                base: Reg::S1,
                offset: QUEST_SEQ_CHILD_POINTER_FIELD_OFFSET,
            },
        ),
        (
            0x8007_33C8,
            "sltiu subtype, 6",
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: i16::from(QUEST_SEQ_CHILD_CONFIGURATION_SUBTYPE_LIMIT),
            },
        ),
        // subtype 0: `(tile * 16 + 16) << 12` 위치와 layer.
        (
            0x8007_33F8,
            "lhu x, 2(stream)",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 2,
            },
        ),
        (
            0x8007_3400,
            "sll x, 4",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 4,
            },
        ),
        (
            0x8007_3404,
            "addiu x, 16",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 16,
            },
        ),
        (
            0x8007_3408,
            "sll x, 12",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: QUEST_SEQ_CHILD_FIXED_POINT_SHIFT,
            },
        ),
        (
            0x8007_340C,
            "sw x, 0xA8(child)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0xA8,
            },
        ),
        (
            0x8007_3418,
            "lhu y, 4(stream)",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8007_3420,
            "sll y, 4",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 4,
            },
        ),
        (
            0x8007_3424,
            "addiu y, 16",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 16,
            },
        ),
        (
            0x8007_3428,
            "sll y, 12",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: QUEST_SEQ_CHILD_FIXED_POINT_SHIFT,
            },
        ),
        (
            0x8007_342C,
            "sw y, 0xAC(child)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0xAC,
            },
        ),
        (
            0x8007_3438,
            "lhu layer, 6(stream)",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 6,
            },
        ),
        (
            0x8007_3440,
            "sw layer, 0x54(child)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x54,
            },
        ),
        // subtype 2: 접촉 사각형 offset과 폭·높이.
        (
            0x8007_34C0,
            "lhu bounds left, 2(stream)",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 2,
            },
        ),
        (
            0x8007_34C8,
            "sh bounds left, 0xB0(child)",
            K::Sh {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0xB0,
            },
        ),
        (
            0x8007_34D4,
            "lhu bounds top, 4(stream)",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8007_34DC,
            "sh bounds top, 0xB2(child)",
            K::Sh {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0xB2,
            },
        ),
        (
            0x8007_34E8,
            "lhu bounds width, 6(stream)",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 6,
            },
        ),
        (
            0x8007_34F0,
            "sh bounds width, 0xB4(child)",
            K::Sh {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0xB4,
            },
        ),
        (
            0x8007_34FC,
            "lhu bounds height, 8(stream)",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 8,
            },
        ),
        (
            0x8007_3504,
            "sh bounds height, 0xB6(child)",
            K::Sh {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0xB6,
            },
        ),
        // subtype 3: signed pixel delta와 fixed-point 속도.
        (
            0x8007_3510,
            "lh delta x, 2(stream)",
            K::Lh {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 2,
            },
        ),
        (
            0x8007_3518,
            "sll delta x, 12",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: QUEST_SEQ_CHILD_FIXED_POINT_SHIFT,
            },
        ),
        (
            0x8007_351C,
            "addu child x, delta x",
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8007_3520,
            "sw child x, 0xA8(child)",
            K::Sw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0xA8,
            },
        ),
        (
            0x8007_352C,
            "lh delta y, 4(stream)",
            K::Lh {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8007_3534,
            "sll delta y, 12",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: QUEST_SEQ_CHILD_FIXED_POINT_SHIFT,
            },
        ),
        (
            0x8007_3538,
            "addu child y, delta y",
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8007_353C,
            "sw child y, 0xAC(child)",
            K::Sw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0xAC,
            },
        ),
        (
            0x8007_3548,
            "lh velocity x, 6(stream)",
            K::Lh {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 6,
            },
        ),
        (
            0x8007_3550,
            "sll velocity x, 12",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: QUEST_SEQ_CHILD_FIXED_POINT_SHIFT,
            },
        ),
        (
            0x8007_3554,
            "sw velocity x, 0x44(child)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x44,
            },
        ),
        (
            0x8007_3560,
            "lh velocity y, 8(stream)",
            K::Lh {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 8,
            },
        ),
        (
            0x8007_3568,
            "sll velocity y, 12",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: QUEST_SEQ_CHILD_FIXED_POINT_SHIFT,
            },
        ),
        (
            0x8007_3570,
            "sw velocity y, 0x48(child)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x48,
            },
        ),
        (
            0x8007_35AC,
            "lw stream, 0x1C(parent)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            0x8007_35B4,
            "addiu stream, stream, 12",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_SEQ_CHILD_CONFIGURATION_RECORD_BYTES as i16,
            },
        ),
        (
            0x8007_35B8,
            "sw stream, 0x1C(parent)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
    ]
}

/// 지원 `QUEST.EXE`의 `OP83` child 구성 의미를 검증한다.
pub fn verify_box_quest_seq_child_configuration(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqChildConfigurationSummary, QuestSeqChildConfigurationError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST OP83 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len
        )));
    }
    if quest_seq_opcode_handler(QUEST_SEQ_CHILD_CONFIGURATION_OPCODE)
        != Some(QUEST_SEQ_CHILD_CONFIGURATION_HANDLER_RUNTIME)
    {
        return Err(error("QUEST OP83 dispatch handler 불일치"));
    }
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error("QUEST.EXE text 길이가 4바이트 정렬이 아님"))?;
    for (address, expected, kind) in required_instructions() {
        require(&instructions, address, expected, kind)?;
    }
    for (index, target) in JUMP_TABLE.into_iter().enumerate() {
        let address = QUEST_SEQ_CHILD_CONFIGURATION_TABLE_RUNTIME
            + u32::try_from(index).expect("여섯 table index") * 4;
        require_word(map, text, address, target)?;
    }

    Ok(QuestSeqChildConfigurationSummary {
        opcode: QUEST_SEQ_CHILD_CONFIGURATION_OPCODE,
        handler_runtime: QUEST_SEQ_CHILD_CONFIGURATION_HANDLER_RUNTIME,
        subtype_limit: QUEST_SEQ_CHILD_CONFIGURATION_SUBTYPE_LIMIT,
        table_runtime: QUEST_SEQ_CHILD_CONFIGURATION_TABLE_RUNTIME,
        record_bytes: QUEST_SEQ_CHILD_CONFIGURATION_RECORD_BYTES,
        child_pointer_field_offset: QUEST_SEQ_CHILD_POINTER_FIELD_OFFSET,
        position_arm_runtime: QUEST_SEQ_CHILD_POSITION_ARM_RUNTIME,
        bounds_arm_runtime: QUEST_SEQ_CHILD_BOUNDS_ARM_RUNTIME,
        motion_arm_runtime: QUEST_SEQ_CHILD_MOTION_ARM_RUNTIME,
        fixed_point_shift: QUEST_SEQ_CHILD_FIXED_POINT_SHIFT,
    })
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: &str,
    kind: K,
) -> Result<(), QuestSeqChildConfigurationError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| error(format!("QUEST OP83 명령이 text 밖임: 0x{address:08X}")))?;
    if actual.kind != kind {
        return Err(error(format!(
            "QUEST OP83 명령 불일치: address=0x{address:08X}, \
             expected={expected}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn require_word(
    map: &ExeMap,
    text: &[u8],
    address: u32,
    expected: u32,
) -> Result<(), QuestSeqChildConfigurationError> {
    let offset = map
        .text_offset(address)
        .ok_or_else(|| error(format!("QUEST OP83 table 주소 없음: 0x{address:08X}")))?;
    let actual = u32::from_le_bytes(
        text.get(offset..offset + 4)
            .ok_or_else(|| error(format!("QUEST OP83 table word가 잘림: 0x{address:08X}")))?
            .try_into()
            .expect("4바이트 범위를 확인함"),
    );
    if actual != expected {
        return Err(error(format!(
            "QUEST OP83 table word 불일치: address=0x{address:08X}, \
             expected=0x{expected:08X}, actual=0x{actual:08X}"
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_seq_child_configuration_tests.rs"]
mod tests;
