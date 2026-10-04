//! QUEST 파괴물 callback의 공격 contact 소비 경로.
//!
//! callback `0x80063820`의 상태 0은 `+0xB8 & 0x0F00`을 기다린다.
//! 공격 contact가 있으면 flag를 지우고 상태를 1로 옮기며, 상태 1 경로는
//! 객체 해제 함수를 호출한다. 이 정적 연결은 특정 맵 객체의 종류나 실제
//! 파괴 장면을 주장하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::linear;
use super::quest_font_object_lifetime::QUEST_OBJECT_RELEASE_RUNTIME;

pub const QUEST_BREAKABLE_CONTACT_CALLBACK_RUNTIME: u32 = 0x8006_3820;
pub const QUEST_BREAKABLE_STATE_OFFSET: i16 = 0xC6;
pub const QUEST_BREAKABLE_CONTACT_FLAGS_OFFSET: i16 = 0xB8;
pub const QUEST_BREAKABLE_ATTACK_CONTACT_MASK: u16 = 0x0F00;
pub const QUEST_BREAKABLE_RELEASE_CALL_RUNTIME: u32 = 0x8006_3B38;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestBreakableContactSummary {
    pub update_callback_runtime: u32,
    pub state_offset: i16,
    pub contact_flags_offset: i16,
    pub attack_contact_mask: u16,
    pub release_call_runtime: u32,
    pub release_object_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestBreakableContactError(String);

impl fmt::Display for QuestBreakableContactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestBreakableContactError {}

fn error(message: impl Into<String>) -> QuestBreakableContactError {
    QuestBreakableContactError(message.into())
}

fn required_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        (
            0x8006_383C,
            "lh breakable state",
            K::Lh {
                rt: Reg::V1,
                base: Reg::S3,
                offset: QUEST_BREAKABLE_STATE_OFFSET,
            },
        ),
        (
            0x8006_3844,
            "beq state zero, contact check",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::ZERO,
                offset: 5,
            },
        ),
        (
            0x8006_384C,
            "beq state one, release path",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 141,
            },
        ),
        (
            0x8006_385C,
            "lhu breakable contact flags",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::S3,
                offset: QUEST_BREAKABLE_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8006_3864,
            "andi attack contact flags",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_BREAKABLE_ATTACK_CONTACT_MASK,
            },
        ),
        (
            0x8006_3868,
            "beq no attack contact, return",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 181,
            },
        ),
        // 상태 0의 공격 반응은 contact를 비우고 상태를 1로 옮긴다.
        (
            0x8006_3A70,
            "lhu breakable state before increment",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::S3,
                offset: QUEST_BREAKABLE_STATE_OFFSET,
            },
        ),
        (
            0x8006_3A74,
            "clear breakable contact flags",
            K::Sh {
                rt: Reg::ZERO,
                base: Reg::S3,
                offset: QUEST_BREAKABLE_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8006_3A78,
            "increment breakable state",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
        ),
        (
            0x8006_3A80,
            "store breakable state",
            K::Sh {
                rt: Reg::V0,
                base: Reg::S3,
                offset: QUEST_BREAKABLE_STATE_OFFSET,
            },
        ),
        // 상태 1의 연출 뒤 객체를 해제한다.
        (
            QUEST_BREAKABLE_RELEASE_CALL_RUNTIME,
            "jal release breakable object",
            K::Jal {
                target: QUEST_OBJECT_RELEASE_RUNTIME,
            },
        ),
        (
            0x8006_3B3C,
            "pass breakable object to release",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S3,
                rt: Reg::ZERO,
            },
        ),
    ]
}

/// 지원 `QUEST.EXE`에서 파괴물의 공격 contact 소비와 해제를 검증한다.
pub fn verify_box_quest_breakable_contact(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestBreakableContactSummary, QuestBreakableContactError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST breakable contact 입력 불일치: name={}, text={}/{}",
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

    Ok(QuestBreakableContactSummary {
        update_callback_runtime: QUEST_BREAKABLE_CONTACT_CALLBACK_RUNTIME,
        state_offset: QUEST_BREAKABLE_STATE_OFFSET,
        contact_flags_offset: QUEST_BREAKABLE_CONTACT_FLAGS_OFFSET,
        attack_contact_mask: QUEST_BREAKABLE_ATTACK_CONTACT_MASK,
        release_call_runtime: QUEST_BREAKABLE_RELEASE_CALL_RUNTIME,
        release_object_runtime: QUEST_OBJECT_RELEASE_RUNTIME,
    })
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: &str,
    kind: K,
) -> Result<(), QuestBreakableContactError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| {
            error(format!(
                "QUEST breakable contact 명령이 text 밖임: 0x{address:08X}"
            ))
        })?;
    if actual.kind != kind {
        return Err(error(format!(
            "QUEST breakable contact 명령 불일치: address=0x{address:08X}, \
             expected={expected}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_breakable_contact_tests.rs"]
mod tests;
