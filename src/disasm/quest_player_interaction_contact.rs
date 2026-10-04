//! QUEST 동굴형 플레이어의 방향 상호작용 contact 기록 경로.
//!
//! 플레이어 callback `0x80055F44`는 pressed-input word의 `0x0020`이 켜지면
//! `0x80055BFC`를 호출한다. 이 writer는 플레이어 앞의 4px probe 띠로 공용
//! 객체 충돌 검색을 수행하고, 선택한 객체 `+0xB8`에 방향별 low bit를 쓴다.
//! 이 모듈은 입력·probe·bit 대응을 원본 명령에 결속한다. 특정 버튼 이름이나
//! 특정 장면에서의 실제 접촉은 주장하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::linear;
use super::quest_map_collision::{
    QuestCollisionActorGeometry, QuestCollisionRectangle, QUEST_COLLISION_ACTOR_FIXED_X_OFFSET,
    QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET, QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS,
    QUEST_COLLISION_FIXED_POINT_SHIFT,
};
use super::quest_player_input::{
    QUEST_PLAYER_HELD_INPUT_SIGNED_OFFSET, QUEST_PLAYER_INPUT_LUI_IMMEDIATE,
    QUEST_PLAYER_PRESSED_INPUT_FIELD_OFFSET, QUEST_PLAYER_PRESSED_INPUT_RUNTIME,
};
use super::quest_seq_child_activation::{
    QUEST_SEQ_CHILD_COLLISION_QUERY_RUNTIME, QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
};

pub const QUEST_PLAYER_INTERACTION_INPUT_RUNTIME: u32 = QUEST_PLAYER_PRESSED_INPUT_RUNTIME;
pub const QUEST_PLAYER_INTERACTION_INPUT_MASK: u16 = 0x0020;
pub const QUEST_PLAYER_INTERACTION_WRITER_CALL_RUNTIME: u32 = 0x8005_7510;
pub const QUEST_PLAYER_INTERACTION_WRITER_RUNTIME: u32 = 0x8005_5BFC;
pub const QUEST_PLAYER_INTERACTION_COLLISION_CALL_RUNTIME: u32 = 0x8005_5CFC;
pub const QUEST_PLAYER_INTERACTION_DIRECTION_OFFSET: i16 = 0xA5;
pub const QUEST_PLAYER_INTERACTION_DIRECTION_CONTACT_MASKS: [u16; 4] =
    [0x0001, 0x0002, 0x0004, 0x0008];

/// 방향 0/1/2/3이 가리키는 world pixel 축: 아래/왼쪽/위/오른쪽.
pub const QUEST_PLAYER_INTERACTION_DIRECTION_VECTORS: [[i32; 2]; 4] =
    [[0, 1], [-1, 0], [0, -1], [1, 0]];

pub const QUEST_PLAYER_INTERACTION_PROBE_GAP_PIXELS: i32 = 4;
pub const QUEST_PLAYER_INTERACTION_PROBE_EXTENT_PIXELS: i32 = 4;
pub const QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_INSET_PIXELS: i32 = 1;
pub const QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_EXTENT_TRIM_PIXELS: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestPlayerInteractionContactSummary {
    pub input_runtime: u32,
    pub input_mask: u16,
    pub writer_call_runtime: u32,
    pub writer_runtime: u32,
    pub collision_call_runtime: u32,
    pub collision_query_runtime: u32,
    pub direction_offset: i16,
    pub fixed_position_offsets: [i16; 2],
    pub rectangle_offsets: [i16; 4],
    pub contact_flags_offset: i16,
    pub direction_contact_masks: [u16; 4],
    pub direction_vectors: [[i32; 2]; 4],
    pub probe_gap_pixels: i32,
    pub probe_extent_pixels: i32,
    pub probe_cross_axis_inset_pixels: i32,
    pub probe_cross_axis_extent_trim_pixels: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPlayerInteractionContactError(String);

impl fmt::Display for QuestPlayerInteractionContactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestPlayerInteractionContactError {}

fn error(message: impl Into<String>) -> QuestPlayerInteractionContactError {
    QuestPlayerInteractionContactError(message.into())
}

fn required_instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        // callback의 pressed-input word에서 공격 입력과 상호작용 입력을 분리한다.
        (
            0x8005_7338,
            "lui pressed input base",
            K::Lui {
                rt: Reg::A0,
                imm: QUEST_PLAYER_INPUT_LUI_IMMEDIATE,
            },
        ),
        (
            0x8005_7380,
            "form held-input address",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::A0,
                imm: QUEST_PLAYER_HELD_INPUT_SIGNED_OFFSET,
            },
        ),
        (
            0x8005_7384,
            "load pressed-input word",
            K::Lhu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: QUEST_PLAYER_PRESSED_INPUT_FIELD_OFFSET as i16,
            },
        ),
        (
            0x8005_7394,
            "mask attack input before interaction",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 0x0040,
            },
        ),
        (
            0x8005_73D0,
            "branch from absent attack input",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 77,
            },
        ),
        (
            0x8005_73D4,
            "mask interaction input in delay slot",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: QUEST_PLAYER_INTERACTION_INPUT_MASK,
            },
        ),
        (
            0x8005_7508,
            "skip interaction writer when input is clear",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 5,
            },
        ),
        (
            QUEST_PLAYER_INTERACTION_WRITER_CALL_RUNTIME,
            "call interaction writer",
            K::Jal {
                target: QUEST_PLAYER_INTERACTION_WRITER_RUNTIME,
            },
        ),
        (
            0x8005_7514,
            "pass player object to interaction writer",
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S4,
                rt: Reg::ZERO,
            },
        ),
        // writer의 방향 dispatch.
        (
            0x8005_5C04,
            "preserve player object",
            K::Addu {
                rd: Reg::S0,
                rs: Reg::A0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8005_5C0C,
            "load player direction",
            K::Lbu {
                rt: Reg::V1,
                base: Reg::S0,
                offset: QUEST_PLAYER_INTERACTION_DIRECTION_OFFSET,
            },
        ),
        (
            0x8005_5C14,
            "direction 1 selects left probe",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 15,
            },
        ),
        (
            0x8005_5C28,
            "direction 0 selects down probe",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::ZERO,
                offset: 36,
            },
        ),
        (
            0x8005_5C38,
            "direction 2 selects up probe",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 25,
            },
        ),
        (
            0x8005_5C44,
            "direction 3 selects right probe",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 6,
            },
        ),
        // 왼쪽 probe: x=left-4, y=top+1, extent=(4,height-2).
        (
            0x8005_5C54,
            "load left for left probe",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[0],
            },
        ),
        (
            0x8005_5C5C,
            "move left probe outward",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -4,
            },
        ),
        // 오른쪽 probe: x=left+width+4, y=top+1, extent=(4,height-2).
        (
            0x8005_5C60,
            "load left for right probe",
            K::Lhu {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[0],
            },
        ),
        (
            0x8005_5C64,
            "load width for right probe",
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S0,
                offset: QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[2],
            },
        ),
        (
            0x8005_5C70,
            "move right probe outward",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 4,
            },
        ),
        (
            0x8005_5C84,
            "inset vertical probe top",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
        ),
        (
            0x8005_5C94,
            "trim vertical probe height",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -2,
            },
        ),
        // 위 probe: x=left+1, y=top-4, extent=(width-2,4).
        (
            0x8005_5CA8,
            "inset up probe left",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
        ),
        (
            0x8005_5CB8,
            "move up probe outward",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -4,
            },
        ),
        // 아래 probe: x=left+1, y=top+height+4, extent=(width-2,4).
        (
            0x8005_5CC4,
            "inset down probe left",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
        ),
        (
            0x8005_5CD0,
            "load height for down probe",
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S0,
                offset: QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[3],
            },
        ),
        (
            0x8005_5CDC,
            "move down probe outward",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 4,
            },
        ),
        (
            0x8005_5CF0,
            "trim horizontal probe width",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -2,
            },
        ),
        (
            QUEST_PLAYER_INTERACTION_COLLISION_CALL_RUNTIME,
            "query object collision with interaction probe",
            K::Jal {
                target: QUEST_SEQ_CHILD_COLLISION_QUERY_RUNTIME,
            },
        ),
        (
            0x8005_5D00,
            "pass interaction probe",
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::SP,
                imm: 0x10,
            },
        ),
        // 방향 0/1/2/3은 low bit 1/2/4/8을 쓴다.
        (
            0x8005_5D18,
            "direction 1 selects bit 2",
            K::Beq {
                rs: Reg::A0,
                rt: Reg::V0,
                offset: 16,
            },
        ),
        (
            0x8005_5D2C,
            "direction 0 selects bit 1",
            K::Beq {
                rs: Reg::A0,
                rt: Reg::ZERO,
                offset: 20,
            },
        ),
        (
            0x8005_5D40,
            "direction 2 selects bit 4",
            K::Beq {
                rs: Reg::A0,
                rt: Reg::V0,
                offset: 12,
            },
        ),
        (
            0x8005_5D4C,
            "direction 3 selects bit 8",
            K::Beq {
                rs: Reg::A0,
                rt: Reg::V0,
                offset: 6,
            },
        ),
        (
            0x8005_5D64,
            "write direction 1 bit",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_INTERACTION_DIRECTION_CONTACT_MASKS[1],
            },
        ),
        (
            0x8005_5D70,
            "write direction 3 bit",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_INTERACTION_DIRECTION_CONTACT_MASKS[3],
            },
        ),
        (
            0x8005_5D7C,
            "write direction 2 bit",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_INTERACTION_DIRECTION_CONTACT_MASKS[2],
            },
        ),
        (
            0x8005_5D88,
            "write direction 0 bit",
            K::Ori {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_INTERACTION_DIRECTION_CONTACT_MASKS[0],
            },
        ),
        (
            0x8005_5D8C,
            "store interaction contact flag",
            K::Sh {
                rt: Reg::V0,
                base: Reg::V1,
                offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
            },
        ),
    ]
}

/// actor anchor 기준의 방향별 interaction probe 사각형을 만든다.
pub fn quest_player_interaction_local_probe(
    local_rectangle: [i16; 4],
    direction: u8,
) -> Result<QuestCollisionRectangle, QuestPlayerInteractionContactError> {
    let [left, top, horizontal_extent, vertical_extent] = local_rectangle.map(i32::from);
    if horizontal_extent < QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_EXTENT_TRIM_PIXELS
        || vertical_extent < QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_EXTENT_TRIM_PIXELS
    {
        return Err(error(format!(
            "QUEST player interaction 원본 사각형이 probe보다 작음: {local_rectangle:?}"
        )));
    }
    let rectangle = match direction {
        0 => QuestCollisionRectangle {
            x: left + QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_INSET_PIXELS,
            y: top
                .checked_add(vertical_extent)
                .and_then(|value| value.checked_add(QUEST_PLAYER_INTERACTION_PROBE_GAP_PIXELS))
                .ok_or_else(|| error("QUEST player interaction down probe 좌표 overflow"))?,
            horizontal_extent: horizontal_extent
                - QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_EXTENT_TRIM_PIXELS,
            vertical_extent: QUEST_PLAYER_INTERACTION_PROBE_EXTENT_PIXELS,
        },
        1 => QuestCollisionRectangle {
            x: left
                .checked_sub(QUEST_PLAYER_INTERACTION_PROBE_GAP_PIXELS)
                .ok_or_else(|| error("QUEST player interaction left probe 좌표 overflow"))?,
            y: top + QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_INSET_PIXELS,
            horizontal_extent: QUEST_PLAYER_INTERACTION_PROBE_EXTENT_PIXELS,
            vertical_extent: vertical_extent
                - QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_EXTENT_TRIM_PIXELS,
        },
        2 => QuestCollisionRectangle {
            x: left + QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_INSET_PIXELS,
            y: top
                .checked_sub(QUEST_PLAYER_INTERACTION_PROBE_GAP_PIXELS)
                .ok_or_else(|| error("QUEST player interaction up probe 좌표 overflow"))?,
            horizontal_extent: horizontal_extent
                - QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_EXTENT_TRIM_PIXELS,
            vertical_extent: QUEST_PLAYER_INTERACTION_PROBE_EXTENT_PIXELS,
        },
        3 => QuestCollisionRectangle {
            x: left
                .checked_add(horizontal_extent)
                .and_then(|value| value.checked_add(QUEST_PLAYER_INTERACTION_PROBE_GAP_PIXELS))
                .ok_or_else(|| error("QUEST player interaction right probe 좌표 overflow"))?,
            y: top + QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_INSET_PIXELS,
            horizontal_extent: QUEST_PLAYER_INTERACTION_PROBE_EXTENT_PIXELS,
            vertical_extent: vertical_extent
                - QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_EXTENT_TRIM_PIXELS,
        },
        _ => {
            return Err(error(format!(
                "QUEST player interaction 방향 범위 이탈: {direction}"
            )))
        }
    };
    Ok(rectangle)
}

/// fixed-point actor 위치를 더한 방향별 interaction probe 사각형을 만든다.
pub fn quest_player_interaction_probe_rectangle(
    actor: QuestCollisionActorGeometry,
    direction: u8,
) -> Result<QuestCollisionRectangle, QuestPlayerInteractionContactError> {
    let local = quest_player_interaction_local_probe(actor.local_rectangle, direction)?;
    let scale = 1_i32 << QUEST_COLLISION_FIXED_POINT_SHIFT;
    let anchor_x = actor.position_fixed[0] / scale;
    let anchor_y = actor.position_fixed[1] / scale;
    Ok(QuestCollisionRectangle {
        x: anchor_x
            .checked_add(local.x)
            .ok_or_else(|| error("QUEST player interaction probe x overflow"))?,
        y: anchor_y
            .checked_add(local.y)
            .ok_or_else(|| error("QUEST player interaction probe y overflow"))?,
        horizontal_extent: local.horizontal_extent,
        vertical_extent: local.vertical_extent,
    })
}

/// 지원 `QUEST.EXE`에서 플레이어 상호작용 입력→low contact bit 경로를 검증한다.
pub fn verify_box_quest_player_interaction_contact(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestPlayerInteractionContactSummary, QuestPlayerInteractionContactError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST player interaction 입력 불일치: name={}, text={}/{}",
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

    Ok(QuestPlayerInteractionContactSummary {
        input_runtime: QUEST_PLAYER_INTERACTION_INPUT_RUNTIME,
        input_mask: QUEST_PLAYER_INTERACTION_INPUT_MASK,
        writer_call_runtime: QUEST_PLAYER_INTERACTION_WRITER_CALL_RUNTIME,
        writer_runtime: QUEST_PLAYER_INTERACTION_WRITER_RUNTIME,
        collision_call_runtime: QUEST_PLAYER_INTERACTION_COLLISION_CALL_RUNTIME,
        collision_query_runtime: QUEST_SEQ_CHILD_COLLISION_QUERY_RUNTIME,
        direction_offset: QUEST_PLAYER_INTERACTION_DIRECTION_OFFSET,
        fixed_position_offsets: [
            QUEST_COLLISION_ACTOR_FIXED_X_OFFSET,
            QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET,
        ],
        rectangle_offsets: QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS,
        contact_flags_offset: QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
        direction_contact_masks: QUEST_PLAYER_INTERACTION_DIRECTION_CONTACT_MASKS,
        direction_vectors: QUEST_PLAYER_INTERACTION_DIRECTION_VECTORS,
        probe_gap_pixels: QUEST_PLAYER_INTERACTION_PROBE_GAP_PIXELS,
        probe_extent_pixels: QUEST_PLAYER_INTERACTION_PROBE_EXTENT_PIXELS,
        probe_cross_axis_inset_pixels: QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_INSET_PIXELS,
        probe_cross_axis_extent_trim_pixels:
            QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_EXTENT_TRIM_PIXELS,
    })
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: &str,
    kind: K,
) -> Result<(), QuestPlayerInteractionContactError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| {
            error(format!(
                "QUEST player interaction 명령이 text 밖임: 0x{address:08X}"
            ))
        })?;
    if actual.kind != kind {
        return Err(error(format!(
            "QUEST player interaction 명령 불일치: address=0x{address:08X}, expected={expected}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_player_interaction_contact_tests.rs"]
mod tests;
