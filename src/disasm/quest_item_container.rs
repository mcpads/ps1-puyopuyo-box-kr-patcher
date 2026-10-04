//! QUEST `OP8E` item container의 생성·접촉·획득 state machine.
//!
//! `OP8E`는 12바이트 record에서 위치·layer·item 정보를 읽어 type 4 객체를
//! 만들고 callback `0x80061A80`을 설치한다. callback state 0은 방향 low bit
//! `0x0004`만 정상 개방 조건으로 소비하며, 공격 bit `0x0F00`은 별도 반응
//! state로 보낸다. 이후 원본 item 획득 state machine을 호출한다. 이 정적
//! 연결만으로 특정 record의 자연 생성이나 화면상 외형을 주장하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::quest_seq_opcode::quest_seq_opcode_handler;

use super::linear;
use super::quest_font_object_lifetime::{
    QUEST_OBJECT_ALLOCATE_RUNTIME, QUEST_OBJECT_CALLBACK_FIELD_OFFSET,
};
use super::quest_item_acquisition::QUEST_ITEM_ACQUISITION_STATE_MACHINE_RUNTIME;
use super::quest_map_collision::{
    QUEST_COLLISION_ACTOR_FIXED_X_OFFSET, QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET,
    QUEST_COLLISION_ACTOR_LAYER_OFFSET, QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET,
    QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS, QUEST_COLLISION_FIXED_POINT_SHIFT,
};
use super::quest_seq_child_activation::{
    QUEST_SEQ_CHILD_COLLISION_MEMBER_MASK, QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
    QUEST_SEQ_CHILD_STATE_OFFSET,
};

pub const QUEST_ITEM_CONTAINER_OPCODE: u8 = 0x8E;
pub const QUEST_ITEM_CONTAINER_HANDLER_RUNTIME: u32 = 0x8007_41C8;
pub const QUEST_ITEM_CONTAINER_CALLBACK_RUNTIME: u32 = 0x8006_1A80;
pub const QUEST_ITEM_CONTAINER_OBJECT_TYPE: u8 = 4;
pub const QUEST_ITEM_CONTAINER_RECORD_BYTES: usize = 12;
pub const QUEST_ITEM_CONTAINER_RECORD_PERSISTENCE_OFFSET: usize = 1;
pub const QUEST_ITEM_CONTAINER_RECORD_X_OFFSET: usize = 2;
pub const QUEST_ITEM_CONTAINER_RECORD_Y_OFFSET: usize = 4;
pub const QUEST_ITEM_CONTAINER_RECORD_LAYER_OFFSET: usize = 6;
pub const QUEST_ITEM_CONTAINER_RECORD_ITEM_ID_OFFSET: usize = 7;
pub const QUEST_ITEM_CONTAINER_RECORD_PARAMETER_OFFSET: usize = 8;

pub const QUEST_ITEM_CONTAINER_PERSISTENCE_FIELD_OFFSET: i16 = 0xA4;
pub const QUEST_ITEM_CONTAINER_PARAMETER_FIELD_OFFSET: i16 = 0x9C;
pub const QUEST_ITEM_CONTAINER_ITEM_ID_FIELD_OFFSET: i16 = 0xCC;
pub const QUEST_ITEM_CONTAINER_LOCAL_RECTANGLE: [i16; 4] = [-15, -15, 30, 15];
pub const QUEST_ITEM_CONTAINER_OPEN_CONTACT_MASK: u16 = 0x0004;
pub const QUEST_ITEM_CONTAINER_ATTACK_REACTION_MASK: u16 = 0x0F00;
pub const QUEST_ITEM_CONTAINER_OPEN_DIRECTION: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestItemContainerSummary {
    pub opcode: u8,
    pub handler_runtime: u32,
    pub callback_runtime: u32,
    pub object_type: u8,
    pub record_bytes: usize,
    pub record_offsets: [usize; 6],
    pub callback_offset: i16,
    pub layer_offset: i16,
    pub fixed_position_offsets: [i16; 2],
    pub rectangle_offsets: [i16; 4],
    pub local_rectangle: [i16; 4],
    pub collision_member_flags_offset: i16,
    pub collision_member_mask: u16,
    pub contact_flags_offset: i16,
    pub state_offset: i16,
    pub persistence_field_offset: i16,
    pub parameter_field_offset: i16,
    pub item_id_field_offset: i16,
    pub open_contact_mask: u16,
    pub attack_reaction_mask: u16,
    pub open_direction: u8,
    pub fixed_point_shift: u8,
    pub item_acquisition_state_machine_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemContainerError(String);

impl fmt::Display for QuestItemContainerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemContainerError {}

fn error(message: impl Into<String>) -> QuestItemContainerError {
    QuestItemContainerError(message.into())
}

fn required_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        (
            0x8007_41D4,
            "load container callback high",
            K::Lui {
                rt: Reg::A0,
                imm: 0x8006,
            },
        ),
        (
            0x8007_41D8,
            "load container callback low",
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x1A80,
            },
        ),
        (
            0x8007_41F8,
            "load current OP8E record",
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            0x8007_4200,
            "load container x tile",
            K::Lhu {
                rt: Reg::S2,
                base: Reg::V0,
                offset: QUEST_ITEM_CONTAINER_RECORD_X_OFFSET as i16,
            },
        ),
        (
            0x8007_4204,
            "load container y tile",
            K::Lhu {
                rt: Reg::S3,
                base: Reg::V0,
                offset: QUEST_ITEM_CONTAINER_RECORD_Y_OFFSET as i16,
            },
        ),
        (
            0x8007_4208,
            "load container layer",
            K::Lbu {
                rt: Reg::S4,
                base: Reg::V0,
                offset: QUEST_ITEM_CONTAINER_RECORD_LAYER_OFFSET as i16,
            },
        ),
        (
            0x8007_420C,
            "load container persistence offset",
            K::Lbu {
                rt: Reg::S6,
                base: Reg::V0,
                offset: QUEST_ITEM_CONTAINER_RECORD_PERSISTENCE_OFFSET as i16,
            },
        ),
        (
            0x8007_4210,
            "load container item id",
            K::Lbu {
                rt: Reg::S0,
                base: Reg::V0,
                offset: QUEST_ITEM_CONTAINER_RECORD_ITEM_ID_OFFSET as i16,
            },
        ),
        (
            0x8007_4214,
            "load container parameter",
            K::Lhu {
                rt: Reg::S5,
                base: Reg::V0,
                offset: QUEST_ITEM_CONTAINER_RECORD_PARAMETER_OFFSET as i16,
            },
        ),
        (
            0x8007_4218,
            "allocate container object",
            K::Jal {
                target: QUEST_OBJECT_ALLOCATE_RUNTIME,
            },
        ),
        (
            0x8007_421C,
            "select object type 4",
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: i16::from(QUEST_ITEM_CONTAINER_OBJECT_TYPE),
            },
        ),
        (
            0x8007_4244,
            "load collision member flags",
            K::Lbu {
                rt: Reg::A0,
                base: Reg::V0,
                offset: QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET,
            },
        ),
        (
            0x8007_424C,
            "initialize container state",
            K::Sh {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: QUEST_SEQ_CHILD_STATE_OFFSET,
            },
        ),
        (
            0x8007_4258,
            "enable object collision membership",
            K::Ori {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: QUEST_SEQ_CHILD_COLLISION_MEMBER_MASK,
            },
        ),
        (
            0x8007_425C,
            "scale x tile by 16",
            K::Sll {
                rd: Reg::V1,
                rt: Reg::S2,
                sa: 4,
            },
        ),
        (
            0x8007_4260,
            "inset x anchor by 16",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 16,
            },
        ),
        (
            0x8007_4264,
            "convert x anchor to fixed point",
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: QUEST_COLLISION_FIXED_POINT_SHIFT,
            },
        ),
        (
            0x8007_4268,
            "store container fixed x",
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: QUEST_COLLISION_ACTOR_FIXED_X_OFFSET,
            },
        ),
        (
            0x8007_426C,
            "scale y tile by 16",
            K::Sll {
                rd: Reg::V1,
                rt: Reg::S3,
                sa: 4,
            },
        ),
        (
            0x8007_4270,
            "inset y anchor by 16",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 16,
            },
        ),
        (
            0x8007_4274,
            "convert y anchor to fixed point",
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: QUEST_COLLISION_FIXED_POINT_SHIFT,
            },
        ),
        (
            0x8007_4278,
            "store container fixed y",
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET,
            },
        ),
        (
            0x8007_4284,
            "store collision member flags",
            K::Sb {
                rt: Reg::A0,
                base: Reg::V0,
                offset: QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET,
            },
        ),
        (
            0x8007_4288,
            "store container layer",
            K::Sw {
                rt: Reg::S4,
                base: Reg::V0,
                offset: QUEST_COLLISION_ACTOR_LAYER_OFFSET,
            },
        ),
        (
            0x8007_4298,
            "load negative rectangle origin",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: QUEST_ITEM_CONTAINER_LOCAL_RECTANGLE[0],
            },
        ),
        (
            0x8007_429C,
            "store container left",
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[0],
            },
        ),
        (
            0x8007_42A0,
            "store container top",
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[1],
            },
        ),
        (
            0x8007_42A4,
            "load container width extent",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: QUEST_ITEM_CONTAINER_LOCAL_RECTANGLE[2],
            },
        ),
        (
            0x8007_42A8,
            "store container width extent",
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[2],
            },
        ),
        (
            0x8007_42AC,
            "load container height extent",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: QUEST_ITEM_CONTAINER_LOCAL_RECTANGLE[3],
            },
        ),
        (
            0x8007_42B0,
            "store container parameter",
            K::Sh {
                rt: Reg::S5,
                base: Reg::V0,
                offset: QUEST_ITEM_CONTAINER_PARAMETER_FIELD_OFFSET,
            },
        ),
        (
            0x8007_42B4,
            "store container item id",
            K::Sw {
                rt: Reg::S0,
                base: Reg::V0,
                offset: QUEST_ITEM_CONTAINER_ITEM_ID_FIELD_OFFSET,
            },
        ),
        (
            0x8007_42B8,
            "store container height extent",
            K::Sh {
                rt: Reg::V1,
                base: Reg::V0,
                offset: QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[3],
            },
        ),
        (
            0x8007_42C0,
            "store packed persistence selector",
            K::Sb {
                rt: Reg::A0,
                base: Reg::V0,
                offset: QUEST_ITEM_CONTAINER_PERSISTENCE_FIELD_OFFSET,
            },
        ),
        (
            0x8007_42CC,
            "advance OP8E record by 12",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_ITEM_CONTAINER_RECORD_BYTES as i16,
            },
        ),
        // callback state 0의 정상 개방과 별도 공격 반응.
        (
            0x8006_1A94,
            "load container state",
            K::Lh {
                rt: Reg::S0,
                base: Reg::S1,
                offset: QUEST_SEQ_CHILD_STATE_OFFSET,
            },
        ),
        (
            0x8006_1AF8,
            "load container contact flags",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::S1,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8006_1B00,
            "mask normal open direction",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_ITEM_CONTAINER_OPEN_CONTACT_MASK,
            },
        ),
        (
            0x8006_1B18,
            "clear consumed open contact",
            K::Sh {
                rt: Reg::ZERO,
                base: Reg::S1,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8006_1B6C,
            "advance container state after open contact",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 1,
            },
        ),
        (
            0x8006_1B7C,
            "store state after open contact",
            K::Sh {
                rt: Reg::V1,
                base: Reg::S1,
                offset: QUEST_SEQ_CHILD_STATE_OFFSET,
            },
        ),
        (
            0x8006_1B80,
            "reload contact for attack reaction",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::S1,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
        (
            0x8006_1B88,
            "mask attack reaction bits",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_ITEM_CONTAINER_ATTACK_REACTION_MASK,
            },
        ),
        (
            0x8006_1BB8,
            "store attack reaction state",
            K::Sh {
                rt: Reg::V0,
                base: Reg::S1,
                offset: QUEST_SEQ_CHILD_STATE_OFFSET,
            },
        ),
        // 정상 state 2은 공용 item 획득 state machine에 객체를 넘긴다.
        (
            0x8006_1BEC,
            "load container item id",
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: QUEST_ITEM_CONTAINER_ITEM_ID_FIELD_OFFSET,
            },
        ),
        (
            0x8006_1BF8,
            "load container parameter",
            K::Lh {
                rt: Reg::V0,
                base: Reg::S1,
                offset: QUEST_ITEM_CONTAINER_PARAMETER_FIELD_OFFSET,
            },
        ),
        (
            0x8006_1C10,
            "pass container object to acquisition",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S1,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8006_1C14,
            "call item acquisition state machine",
            K::Jal {
                target: QUEST_ITEM_ACQUISITION_STATE_MACHINE_RUNTIME,
            },
        ),
        (
            0x8006_1C18,
            "select acquisition mode zero",
            K::Addu {
                rd: Reg::A1,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
    ]
}

/// 지원 `QUEST.EXE`에서 OP8E item container 생성·개방·획득 경로를 검증한다.
pub fn verify_box_quest_item_container(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestItemContainerSummary, QuestItemContainerError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST item container 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len
        )));
    }
    if quest_seq_opcode_handler(QUEST_ITEM_CONTAINER_OPCODE)
        != Some(QUEST_ITEM_CONTAINER_HANDLER_RUNTIME)
    {
        return Err(error("QUEST OP8E handler 모델 불일치"));
    }
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error("QUEST.EXE text 길이가 4바이트 정렬이 아님"))?;
    for (address, expected, kind) in required_instructions() {
        require(&instructions, address, expected, kind)?;
    }

    Ok(QuestItemContainerSummary {
        opcode: QUEST_ITEM_CONTAINER_OPCODE,
        handler_runtime: QUEST_ITEM_CONTAINER_HANDLER_RUNTIME,
        callback_runtime: QUEST_ITEM_CONTAINER_CALLBACK_RUNTIME,
        object_type: QUEST_ITEM_CONTAINER_OBJECT_TYPE,
        record_bytes: QUEST_ITEM_CONTAINER_RECORD_BYTES,
        record_offsets: [
            QUEST_ITEM_CONTAINER_RECORD_PERSISTENCE_OFFSET,
            QUEST_ITEM_CONTAINER_RECORD_X_OFFSET,
            QUEST_ITEM_CONTAINER_RECORD_Y_OFFSET,
            QUEST_ITEM_CONTAINER_RECORD_LAYER_OFFSET,
            QUEST_ITEM_CONTAINER_RECORD_ITEM_ID_OFFSET,
            QUEST_ITEM_CONTAINER_RECORD_PARAMETER_OFFSET,
        ],
        callback_offset: QUEST_OBJECT_CALLBACK_FIELD_OFFSET,
        layer_offset: QUEST_COLLISION_ACTOR_LAYER_OFFSET,
        fixed_position_offsets: [
            QUEST_COLLISION_ACTOR_FIXED_X_OFFSET,
            QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET,
        ],
        rectangle_offsets: QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS,
        local_rectangle: QUEST_ITEM_CONTAINER_LOCAL_RECTANGLE,
        collision_member_flags_offset: QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET,
        collision_member_mask: QUEST_SEQ_CHILD_COLLISION_MEMBER_MASK,
        contact_flags_offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
        state_offset: QUEST_SEQ_CHILD_STATE_OFFSET,
        persistence_field_offset: QUEST_ITEM_CONTAINER_PERSISTENCE_FIELD_OFFSET,
        parameter_field_offset: QUEST_ITEM_CONTAINER_PARAMETER_FIELD_OFFSET,
        item_id_field_offset: QUEST_ITEM_CONTAINER_ITEM_ID_FIELD_OFFSET,
        open_contact_mask: QUEST_ITEM_CONTAINER_OPEN_CONTACT_MASK,
        attack_reaction_mask: QUEST_ITEM_CONTAINER_ATTACK_REACTION_MASK,
        open_direction: QUEST_ITEM_CONTAINER_OPEN_DIRECTION,
        fixed_point_shift: QUEST_COLLISION_FIXED_POINT_SHIFT,
        item_acquisition_state_machine_runtime: QUEST_ITEM_ACQUISITION_STATE_MACHINE_RUNTIME,
    })
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: &str,
    kind: K,
) -> Result<(), QuestItemContainerError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| {
            error(format!(
                "QUEST item container 명령이 text 밖임: 0x{address:08X}"
            ))
        })?;
    if actual.kind != kind {
        return Err(error(format!(
            "QUEST item container 명령 불일치: address=0x{address:08X}, expected={expected}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_item_container_tests.rs"]
mod tests;
