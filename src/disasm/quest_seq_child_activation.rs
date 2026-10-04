//! QUEST `OP82` subtype 2 child의 접촉 기반 interpreter 활성화.
//!
//! subtype 2는 update callback `0x80058C80`을 설치하고 상태 `+0xC6`와
//! 접촉 flag `+0xB8`을 0으로 시작한다. callback의 상태 0은 접촉 flag의
//! 방향 bit를 기다려 상태 1로 옮기며, 상태 1에서만 공용 SEQ interpreter를
//! 호출한다. 별도 충돌 검색 경로가 같은 layer의 겹친 객체에 그 방향 bit를
//! 쓴다. 이 정적 연결은 실제 플레이어 접촉이나 장면 도달을 주장하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::linear;
use super::quest_seq::QUEST_SEQ_INTERPRETER_RUNTIME;
use super::quest_seq_child_stream::QUEST_SEQ_CHILD_STREAM_SUBTYPE2_ARM_RUNTIME;

pub const QUEST_SEQ_CHILD_UPDATE_CALLBACK_RUNTIME: u32 = 0x8005_8C80;
pub const QUEST_SEQ_CHILD_STATE_OFFSET: i16 = 0xC6;
pub const QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET: i16 = 0xB8;
pub const QUEST_SEQ_CHILD_DIRECT_CONTACT_MASK: u16 = 0x000F;
pub const QUEST_SEQ_CHILD_ALTERNATE_CONTACT_MASK: u16 = 0xF000;
pub const QUEST_SEQ_CHILD_COLLISION_MEMBER_FLAGS_OFFSET: i16 = 0xBA;
pub const QUEST_SEQ_CHILD_COLLISION_MEMBER_MASK: u16 = 0x0002;
pub const QUEST_SEQ_CHILD_COLLISION_LAYER_OFFSET: i16 = 0x54;
pub const QUEST_SEQ_CHILD_COLLISION_QUERY_RUNTIME: u32 = 0x8006_E550;
pub const QUEST_SEQ_CHILD_CONTACT_WRITER_RUNTIME: u32 = 0x8005_4F8C;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqChildActivationSummary {
    pub subtype2_arm_runtime: u32,
    pub update_callback_runtime: u32,
    pub state_offset: i16,
    pub contact_flags_offset: i16,
    pub direct_contact_mask: u16,
    pub alternate_contact_mask: u16,
    pub collision_member_flags_offset: i16,
    pub collision_member_mask: u16,
    pub collision_layer_offset: i16,
    pub collision_query_runtime: u32,
    pub contact_writer_runtime: u32,
    pub interpreter_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqChildActivationError(String);

impl fmt::Display for QuestSeqChildActivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestSeqChildActivationError {}

fn error(message: impl Into<String>) -> QuestSeqChildActivationError {
    QuestSeqChildActivationError(message.into())
}

fn required_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        // OP82 subtype 2 callback과 초기 상태.
        (
            QUEST_SEQ_CHILD_STREAM_SUBTYPE2_ARM_RUNTIME,
            "lui callback high",
            K::Lui {
                rt: Reg::A0,
                imm: 0x8006,
            },
        ),
        (
            0x8007_29E4,
            "j subtype 2 allocator",
            K::J {
                target: 0x8007_3294,
            },
        ),
        (
            0x8007_29E8,
            "addiu callback low",
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: -29_568,
            },
        ),
        (
            0x8007_32A8,
            "sh zero, 0xC6(child)",
            K::Sh {
                rt: Reg::ZERO,
                base: Reg::S1,
                offset: QUEST_SEQ_CHILD_STATE_OFFSET,
            },
        ),
        (
            0x8007_32A0,
            "lbu flags, 0xBA(child)",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::S1,
                offset: QUEST_SEQ_CHILD_COLLISION_MEMBER_FLAGS_OFFSET,
            },
        ),
        (
            0x8007_32B0,
            "ori collision member flags, 3",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 3,
            },
        ),
        (
            0x8007_32B4,
            "sb collision member flags, 0xBA(child)",
            K::Sb {
                rt: Reg::V0,
                base: Reg::S1,
                offset: QUEST_SEQ_CHILD_COLLISION_MEMBER_FLAGS_OFFSET,
            },
        ),
        (
            0x8007_335C,
            "sh zero, 0xB8(child)",
            K::Sh {
                rt: Reg::ZERO,
                base: Reg::S1,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        // update callback의 outer guard 뒤 상태 0/1 분기.
        (
            0x8005_8CEC,
            "bne outer guard, return",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 24,
            },
        ),
        (
            0x8005_8CF4,
            "lh state, 0xC6(child)",
            K::Lh {
                rt: Reg::V1,
                base: Reg::A0,
                offset: QUEST_SEQ_CHILD_STATE_OFFSET,
            },
        ),
        (
            0x8005_8CFC,
            "beq state, zero, contact check",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::ZERO,
                offset: 5,
            },
        ),
        (
            0x8005_8D04,
            "beq state, one, interpreter",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 16,
            },
        ),
        (
            0x8005_8D14,
            "lhu contact flags, 0xB8(child)",
            K::Lhu {
                rt: Reg::V1,
                base: Reg::A0,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_8D1C,
            "andi direct contact flags, 0x000F",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: QUEST_SEQ_CHILD_DIRECT_CONTACT_MASK,
            },
        ),
        (
            0x8005_8D20,
            "bne direct contact, stream check",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 3,
            },
        ),
        (
            0x8005_8D24,
            "andi alternate contact flags, 0xF000",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: QUEST_SEQ_CHILD_ALTERNATE_CONTACT_MASK,
            },
        ),
        (
            0x8005_8D28,
            "beq no alternate contact, return",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 9,
            },
        ),
        (
            0x8005_8D30,
            "lw stream, 0x1C(child)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8005_8D38,
            "beq null stream, return",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 5,
            },
        ),
        (
            0x8005_8D44,
            "sh one, 0xC6(child)",
            K::Sh {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_CHILD_STATE_OFFSET,
            },
        ),
        (
            0x8005_8D48,
            "jal SEQ interpreter",
            K::Jal {
                target: QUEST_SEQ_INTERPRETER_RUNTIME,
            },
        ),
        // 겹침 검색은 collision member bit와 같은 layer를 요구한다.
        (
            0x8006_E5EC,
            "lbu candidate collision flags, 0xBA(candidate)",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_CHILD_COLLISION_MEMBER_FLAGS_OFFSET,
            },
        ),
        (
            0x8006_E5F4,
            "andi collision member bit, 2",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_SEQ_CHILD_COLLISION_MEMBER_MASK,
            },
        ),
        (
            0x8006_E600,
            "lw source layer, 0x54(source)",
            K::Lw {
                rt: Reg::V1,
                base: Reg::S3,
                offset: QUEST_SEQ_CHILD_COLLISION_LAYER_OFFSET,
            },
        ),
        (
            0x8006_E604,
            "lw candidate layer, 0x54(candidate)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_CHILD_COLLISION_LAYER_OFFSET,
            },
        ),
        (
            0x8006_E60C,
            "bne layers, next candidate",
            K::Bne {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 42,
            },
        ),
        (
            0x8006_E614,
            "lw candidate x, 0xA8(candidate)",
            K::Lw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0xA8,
            },
        ),
        (
            0x8006_E628,
            "lhu candidate left, 0xB0(candidate)",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0xB0,
            },
        ),
        (
            0x8006_E638,
            "lw candidate y, 0xAC(candidate)",
            K::Lw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0xAC,
            },
        ),
        (
            0x8006_E650,
            "lhu candidate top, 0xB2(candidate)",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0xB2,
            },
        ),
        (
            0x8006_E660,
            "lhu candidate width, 0xB4(candidate)",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0xB4,
            },
        ),
        (
            0x8006_E66C,
            "lhu candidate height, 0xB6(candidate)",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0xB6,
            },
        ),
        (
            0x8006_E674,
            "jal rectangle overlap",
            K::Jal {
                target: 0x8006_B61C,
            },
        ),
        (
            0x8006_E6B4,
            "select overlapping candidate",
            K::Addu {
                rd: Reg::S2,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8006_E6C8,
            "return selected candidate",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::S2,
                rt: Reg::ZERO,
            },
        ),
        // 이동 경로가 겹친 객체의 +0xB8에 네 방향 bit 중 하나를 쓴다.
        (
            QUEST_SEQ_CHILD_CONTACT_WRITER_RUNTIME,
            "jal collision query",
            K::Jal {
                target: QUEST_SEQ_CHILD_COLLISION_QUERY_RUNTIME,
            },
        ),
        (
            0x8005_4F94,
            "move selected candidate, query result",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::V0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8005_4FA0,
            "lbu source direction, 0xA5(source)",
            K::Lbu {
                rt: Reg::V1,
                base: Reg::S3,
                offset: 0xA5,
            },
        ),
        (
            0x8005_4FA8,
            "beq direction 1, contact bit 2",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 13,
            },
        ),
        (
            0x8005_4FB8,
            "beq direction 0, contact bit 1",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::ZERO,
                offset: 24,
            },
        ),
        (
            0x8005_4FC8,
            "beq direction 2, contact bit 4",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 15,
            },
        ),
        (
            0x8005_4FD0,
            "beq direction 3, contact bit 8",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 8,
            },
        ),
        (
            0x8005_4FE0,
            "lhu existing contact flags for direction 1",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_4FE8,
            "ori contact direction, 2",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x0002,
            },
        ),
        (
            0x8005_4FF0,
            "sh contact direction, 0xB8(candidate)",
            K::Sh {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_4FF4,
            "lhu existing contact flags for direction 3",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_4FFC,
            "ori contact direction, 8",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x0008,
            },
        ),
        (
            0x8005_5004,
            "sh contact direction, 0xB8(candidate)",
            K::Sh {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_5008,
            "lhu existing contact flags for direction 2",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_5010,
            "ori contact direction, 4",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x0004,
            },
        ),
        (
            0x8005_5018,
            "sh contact direction, 0xB8(candidate)",
            K::Sh {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_501C,
            "lhu existing contact flags for direction 0",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8005_5024,
            "ori contact direction, 1",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x0001,
            },
        ),
        (
            0x8005_502C,
            "sh contact direction, 0xB8(candidate)",
            K::Sh {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
    ]
}

/// 지원 `QUEST.EXE`에서 subtype 2 child의 접촉→interpreter 경로를 검증한다.
pub fn verify_box_quest_seq_child_activation(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqChildActivationSummary, QuestSeqChildActivationError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST OP82 child 활성화 입력 불일치: name={}, text={}/{}",
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

    Ok(QuestSeqChildActivationSummary {
        subtype2_arm_runtime: QUEST_SEQ_CHILD_STREAM_SUBTYPE2_ARM_RUNTIME,
        update_callback_runtime: QUEST_SEQ_CHILD_UPDATE_CALLBACK_RUNTIME,
        state_offset: QUEST_SEQ_CHILD_STATE_OFFSET,
        contact_flags_offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
        direct_contact_mask: QUEST_SEQ_CHILD_DIRECT_CONTACT_MASK,
        alternate_contact_mask: QUEST_SEQ_CHILD_ALTERNATE_CONTACT_MASK,
        collision_member_flags_offset: QUEST_SEQ_CHILD_COLLISION_MEMBER_FLAGS_OFFSET,
        collision_member_mask: QUEST_SEQ_CHILD_COLLISION_MEMBER_MASK,
        collision_layer_offset: QUEST_SEQ_CHILD_COLLISION_LAYER_OFFSET,
        collision_query_runtime: QUEST_SEQ_CHILD_COLLISION_QUERY_RUNTIME,
        contact_writer_runtime: QUEST_SEQ_CHILD_CONTACT_WRITER_RUNTIME,
        interpreter_runtime: QUEST_SEQ_INTERPRETER_RUNTIME,
    })
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: &str,
    kind: K,
) -> Result<(), QuestSeqChildActivationError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| {
            error(format!(
                "QUEST OP82 child 활성화 명령이 text 밖임: 0x{address:08X}"
            ))
        })?;
    if actual.kind != kind {
        return Err(error(format!(
            "QUEST OP82 child 활성화 명령 불일치: address=0x{address:08X}, \
             expected={expected}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_seq_child_activation_tests.rs"]
mod tests;
