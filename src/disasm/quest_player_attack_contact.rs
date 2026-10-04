//! QUEST 플레이어 공격 입력과 contact flag 기록 경로.
//!
//! 플레이어 callback은 입력 word의 `0x0040`을 공격 시작 조건으로 읽고,
//! 방향별 공격 사각형으로 공용 충돌 검색을 호출한다. 검색된 상대의 `+0xB8`에는
//! 방향별 `0x0100..0x0800` bit를 기록한다. 이 정적 연결만으로 특정 컨트롤러
//! 버튼 이름이나 실제 파괴 성공을 주장하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::linear;
use super::quest_player_input::{
    QUEST_PLAYER_HELD_INPUT_RUNTIME, QUEST_PLAYER_HELD_INPUT_SIGNED_OFFSET,
    QUEST_PLAYER_INPUT_LUI_IMMEDIATE,
};
use super::quest_seq_child_activation::QUEST_SEQ_CHILD_COLLISION_QUERY_RUNTIME;

pub const QUEST_PLAYER_ATTACK_INPUT_RUNTIME: u32 = QUEST_PLAYER_HELD_INPUT_RUNTIME;
pub const QUEST_PLAYER_ATTACK_INPUT_MASK: u16 = 0x0040;
pub const QUEST_PLAYER_ATTACK_STATE_OFFSET: i16 = 0xC0;
pub const QUEST_PLAYER_ATTACK_STATE: i16 = 2;
pub const QUEST_PLAYER_ATTACK_DIRECTION_OFFSET: i16 = 0xA5;
pub const QUEST_PLAYER_ATTACK_COLLISION_CALL_RUNTIME: u32 = 0x8005_672C;
pub const QUEST_PLAYER_ATTACK_TARGET_CONTACT_FLAGS_OFFSET: i16 = 0xB8;
pub const QUEST_PLAYER_ATTACK_DIRECTION_CONTACT_MASKS: [u16; 4] = [0x0100, 0x0200, 0x0400, 0x0800];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestPlayerAttackContactSummary {
    pub input_runtime: u32,
    pub input_mask: u16,
    pub attack_state_offset: i16,
    pub attack_state: i16,
    pub direction_offset: i16,
    pub collision_call_runtime: u32,
    pub collision_query_runtime: u32,
    pub target_contact_flags_offset: i16,
    pub direction_contact_masks: [u16; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPlayerAttackContactError(String);

impl fmt::Display for QuestPlayerAttackContactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestPlayerAttackContactError {}

fn error(message: impl Into<String>) -> QuestPlayerAttackContactError {
    QuestPlayerAttackContactError(message.into())
}

fn required_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        // 입력 word 0x800FD6B8의 0x0040이 공격 시작 조건이다.
        (
            0x8005_65B0,
            "lui player input high",
            K::Lui {
                rt: Reg::V0,
                imm: QUEST_PLAYER_INPUT_LUI_IMMEDIATE,
            },
        ),
        (
            0x8005_65B4,
            "lhu player input",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: QUEST_PLAYER_HELD_INPUT_SIGNED_OFFSET,
            },
        ),
        (
            0x8005_65BC,
            "andi attack input",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_ATTACK_INPUT_MASK,
            },
        ),
        (
            0x8005_65C0,
            "beq no attack input",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 22,
            },
        ),
        // 공격 상태 2에서 방향별 사각형을 만들고 충돌 검색을 호출한다.
        (
            0x8005_661C,
            "lh player attack state",
            K::Lh {
                rt: Reg::A0,
                base: Reg::S4,
                offset: QUEST_PLAYER_ATTACK_STATE_OFFSET,
            },
        ),
        (
            0x8005_6620,
            "load attack state two",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: QUEST_PLAYER_ATTACK_STATE,
            },
        ),
        (
            0x8005_663C,
            "lbu player direction",
            K::Lbu {
                rt: Reg::V1,
                base: Reg::S4,
                offset: QUEST_PLAYER_ATTACK_DIRECTION_OFFSET,
            },
        ),
        (
            QUEST_PLAYER_ATTACK_COLLISION_CALL_RUNTIME,
            "jal collision query",
            K::Jal {
                target: QUEST_SEQ_CHILD_COLLISION_QUERY_RUNTIME,
            },
        ),
        (
            0x8005_6730,
            "pass attack rectangle",
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::SP,
                imm: 0xD0,
            },
        ),
        (
            0x8005_6734,
            "move selected attack target",
            K::Addu {
                rd: Reg::A1,
                rs: Reg::V0,
                rt: Reg::ZERO,
            },
        ),
        // direction 1/0/2/3을 각 middle bit writer로 보낸다.
        (
            0x8005_6748,
            "beq direction 1, attack bit 0x0200",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 16,
            },
        ),
        (
            0x8005_675C,
            "beq direction 0, attack bit 0x0100",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::ZERO,
                offset: 20,
            },
        ),
        (
            0x8005_6770,
            "beq direction 2, attack bit 0x0400",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 12,
            },
        ),
        (
            0x8005_677C,
            "beq direction 3, attack bit 0x0800",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 6,
            },
        ),
        (
            0x8005_678C,
            "lhu target flags for direction 1",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_PLAYER_ATTACK_TARGET_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_6794,
            "ori direction 1 attack bit",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_ATTACK_DIRECTION_CONTACT_MASKS[1],
            },
        ),
        (
            0x8005_6798,
            "lhu target flags for direction 3",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_PLAYER_ATTACK_TARGET_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_67A0,
            "ori direction 3 attack bit",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_ATTACK_DIRECTION_CONTACT_MASKS[3],
            },
        ),
        (
            0x8005_67A4,
            "lhu target flags for direction 2",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_PLAYER_ATTACK_TARGET_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_67AC,
            "ori direction 2 attack bit",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_ATTACK_DIRECTION_CONTACT_MASKS[2],
            },
        ),
        (
            0x8005_67B0,
            "lhu target flags for direction 0",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_PLAYER_ATTACK_TARGET_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_67B8,
            "ori direction 0 attack bit",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_ATTACK_DIRECTION_CONTACT_MASKS[0],
            },
        ),
        (
            0x8005_67BC,
            "sh attack contact flags to target",
            K::Sh {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_PLAYER_ATTACK_TARGET_CONTACT_FLAGS_OFFSET,
            },
        ),
    ]
}

/// 지원 `QUEST.EXE`에서 공격 입력→상대 contact bit 기록을 검증한다.
pub fn verify_box_quest_player_attack_contact(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestPlayerAttackContactSummary, QuestPlayerAttackContactError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST player attack contact 입력 불일치: name={}, text={}/{}",
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

    Ok(QuestPlayerAttackContactSummary {
        input_runtime: QUEST_PLAYER_ATTACK_INPUT_RUNTIME,
        input_mask: QUEST_PLAYER_ATTACK_INPUT_MASK,
        attack_state_offset: QUEST_PLAYER_ATTACK_STATE_OFFSET,
        attack_state: QUEST_PLAYER_ATTACK_STATE,
        direction_offset: QUEST_PLAYER_ATTACK_DIRECTION_OFFSET,
        collision_call_runtime: QUEST_PLAYER_ATTACK_COLLISION_CALL_RUNTIME,
        collision_query_runtime: QUEST_SEQ_CHILD_COLLISION_QUERY_RUNTIME,
        target_contact_flags_offset: QUEST_PLAYER_ATTACK_TARGET_CONTACT_FLAGS_OFFSET,
        direction_contact_masks: QUEST_PLAYER_ATTACK_DIRECTION_CONTACT_MASKS,
    })
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: &str,
    kind: K,
) -> Result<(), QuestPlayerAttackContactError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| {
            error(format!(
                "QUEST player attack contact 명령이 text 밖임: 0x{address:08X}"
            ))
        })?;
    if actual.kind != kind {
        return Err(error(format!(
            "QUEST player attack contact 명령 불일치: address=0x{address:08X}, \
             expected={expected}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_player_attack_contact_tests.rs"]
mod tests;
