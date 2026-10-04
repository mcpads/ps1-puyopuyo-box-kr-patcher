//! QUEST 플레이어의 충돌 상대 contact flag 기록 경로.
//!
//! 플레이어 callback `0x80055F44`는 이동 처리 중 `0x80055DA0`을 호출한다.
//! 이 함수는 공용 사각형 충돌 검색 결과의 `+0xB8`에 방향별 high bit를
//! 기록한다. `OP82` child callback은 같은 word의 `0xF000`을 활성화 조건으로
//! 읽는다. 이 정적 연결은 특정 장면의 도달이나 실제 접촉을 주장하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::linear;
use super::quest_seq_child_activation::{
    QUEST_SEQ_CHILD_COLLISION_QUERY_RUNTIME, QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
};

pub const QUEST_PLAYER_UPDATE_CALLBACK_RUNTIME: u32 = 0x8005_5F44;
pub const QUEST_PLAYER_CONTACT_WRITER_CALL_RUNTIME: u32 = 0x8005_7528;
pub const QUEST_PLAYER_CONTACT_WRITER_RUNTIME: u32 = 0x8005_5DA0;
pub const QUEST_PLAYER_DIRECTION_OFFSET: i16 = 0xA5;
pub const QUEST_PLAYER_DIRECTION_CONTACT_MASKS: [u16; 4] = [0x1000, 0x2000, 0x4000, 0x8000];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestPlayerContactSummary {
    pub update_callback_runtime: u32,
    pub contact_writer_call_runtime: u32,
    pub contact_writer_runtime: u32,
    pub collision_query_runtime: u32,
    pub direction_offset: i16,
    pub contact_flags_offset: i16,
    pub direction_contact_masks: [u16; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPlayerContactError(String);

impl fmt::Display for QuestPlayerContactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestPlayerContactError {}

fn error(message: impl Into<String>) -> QuestPlayerContactError {
    QuestPlayerContactError(message.into())
}

fn required_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        // 플레이어 callback 내부의 writer 호출.
        (
            QUEST_PLAYER_CONTACT_WRITER_CALL_RUNTIME,
            "jal player contact writer",
            K::Jal {
                target: QUEST_PLAYER_CONTACT_WRITER_RUNTIME,
            },
        ),
        (
            0x8005_752C,
            "move player object to a0",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S4,
                rt: Reg::ZERO,
            },
        ),
        // writer가 source object와 방향을 보존하고 공용 충돌 검색을 호출한다.
        (
            0x8005_5DA8,
            "move source object to s0",
            K::Addu {
                rd: Reg::S0,
                rs: Reg::A0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8005_5DB0,
            "lbu source direction",
            K::Lbu {
                rt: Reg::V1,
                base: Reg::S0,
                offset: QUEST_PLAYER_DIRECTION_OFFSET,
            },
        ),
        (
            0x8005_5EA0,
            "jal collision query",
            K::Jal {
                target: QUEST_SEQ_CHILD_COLLISION_QUERY_RUNTIME,
            },
        ),
        (
            0x8005_5EA4,
            "pass directional rectangle",
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::SP,
                imm: 0x10,
            },
        ),
        (
            0x8005_5EA8,
            "move selected candidate",
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8005_5EAC,
            "return when no collision candidate",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::ZERO,
                offset: 33,
            },
        ),
        (
            0x8005_5EB4,
            "reload source direction",
            K::Lbu {
                rt: Reg::A0,
                base: Reg::S0,
                offset: QUEST_PLAYER_DIRECTION_OFFSET,
            },
        ),
        // direction 1/0/2/3을 각 high bit writer로 보낸다.
        (
            0x8005_5EBC,
            "beq direction 1, contact bit 0x2000",
            K::Beq {
                rs: Reg::A0,
                rt: Reg::V0,
                offset: 16,
            },
        ),
        (
            0x8005_5ED0,
            "beq direction 0, contact bit 0x1000",
            K::Beq {
                rs: Reg::A0,
                rt: Reg::ZERO,
                offset: 20,
            },
        ),
        (
            0x8005_5EE4,
            "beq direction 2, contact bit 0x4000",
            K::Beq {
                rs: Reg::A0,
                rt: Reg::V0,
                offset: 12,
            },
        ),
        (
            0x8005_5EF0,
            "beq direction 3, contact bit 0x8000",
            K::Beq {
                rs: Reg::A0,
                rt: Reg::V0,
                offset: 6,
            },
        ),
        (
            0x8005_5F00,
            "lhu flags for direction 1",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_5F08,
            "ori direction 1 contact bit",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_DIRECTION_CONTACT_MASKS[1],
            },
        ),
        (
            0x8005_5F0C,
            "lhu flags for direction 3",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_5F14,
            "ori direction 3 contact bit",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_DIRECTION_CONTACT_MASKS[3],
            },
        ),
        (
            0x8005_5F18,
            "lhu flags for direction 2",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_5F20,
            "ori direction 2 contact bit",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_DIRECTION_CONTACT_MASKS[2],
            },
        ),
        (
            0x8005_5F24,
            "lhu flags for direction 0",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_5F2C,
            "ori direction 0 contact bit",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_DIRECTION_CONTACT_MASKS[0],
            },
        ),
        (
            0x8005_5F30,
            "sh contact flags to candidate",
            K::Sh {
                rt: Reg::V0,
                base: Reg::V1,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
    ]
}

/// 지원 `QUEST.EXE`에서 플레이어 방향→충돌 상대 high-bit 기록을 검증한다.
pub fn verify_box_quest_player_contact(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestPlayerContactSummary, QuestPlayerContactError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST player contact 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len
        )));
    }
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error("QUEST.EXE text 길이가 4바이트 정렬이 아님"))?;
    for (address, expected, kind) in required_instructions() {
        require(&instructions, address, expected, kind)?;
    }

    Ok(QuestPlayerContactSummary {
        update_callback_runtime: QUEST_PLAYER_UPDATE_CALLBACK_RUNTIME,
        contact_writer_call_runtime: QUEST_PLAYER_CONTACT_WRITER_CALL_RUNTIME,
        contact_writer_runtime: QUEST_PLAYER_CONTACT_WRITER_RUNTIME,
        collision_query_runtime: QUEST_SEQ_CHILD_COLLISION_QUERY_RUNTIME,
        direction_offset: QUEST_PLAYER_DIRECTION_OFFSET,
        contact_flags_offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
        direction_contact_masks: QUEST_PLAYER_DIRECTION_CONTACT_MASKS,
    })
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: &str,
    kind: K,
) -> Result<(), QuestPlayerContactError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| {
            error(format!(
                "QUEST player contact 명령이 text 밖임: 0x{address:08X}"
            ))
        })?;
    if actual.kind != kind {
        return Err(error(format!(
            "QUEST player contact 명령 불일치: address=0x{address:08X}, \
             expected={expected}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_player_contact_tests.rs"]
mod tests;
