//! QUEST QMP 충돌 plane의 적재·분류·이동 소비 경로.
//!
//! OP85의 세 QMP payload는 두 map buffer slot의 세 plane에 적재된다. 세 번째
//! plane은 활성화할 때 고정 collision buffer로 publish되고, 이동 함수는
//! `y * width + x`의 u16 tile을 읽어 classifier에 넘긴다. classifier는 low
//! byte와 `0x4000`·`0x8000`, 객체 상태, SEQ `OP92` 조건을 함께 사용한다.
//! 이 정적 계약은 특정 map의 통행 가능 경로나 실제 플레이어 이동을 주장하지
//! 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K};
use crate::qmp::QMP_HEADER_LEN;
use crate::script::quest_seq_opcode_handler;

use super::linear;
use super::quest_player_input::QUEST_PLAYER_HELD_INPUT_RUNTIME;

#[path = "quest_map_collision/classifier.rs"]
mod classifier;
#[path = "quest_map_collision/geometry.rs"]
mod geometry;

pub use classifier::{
    classify_quest_collision_tile, QuestCollisionClass, QuestCollisionClassifierContext,
};
pub use geometry::{
    quest_collision_actor_rectangle, quest_collision_rectangle_overlap,
    quest_collision_tile_probes, quest_collision_tile_rectangle, QuestCollisionActorGeometry,
    QuestCollisionGeometryError, QuestCollisionOverlap, QuestCollisionRectangle,
    QuestCollisionTileBounds, QuestCollisionTileProbe,
};

pub const QUEST_MAP_ACTIVE_SLOT_RUNTIME: u32 = 0x800B_E5A4;
pub const QUEST_MAP_SLOT_BUFFER_ROOTS_RUNTIME: u32 = 0x800B_E5A8;
pub const QUEST_MAP_SLOT_WIDTHS_RUNTIME: u32 = 0x800B_E5E0;
pub const QUEST_MAP_SLOT_HEIGHTS_RUNTIME: u32 = 0x800B_E5E8;
pub const QUEST_MAP_COLLISION_BUFFER_RUNTIME: u32 = 0x800F_7488;
pub const QUEST_MAP_SLOT_COUNT: usize = 2;
pub const QUEST_MAP_PLANE_CAPACITY_CELLS: usize = 0x2400;
pub const QUEST_MAP_PLANE_STRIDE_BYTES: u32 = 0x4800;
pub const QUEST_COLLISION_PLANE_OFFSET: u32 = 0x9000;
pub const QUEST_MAP_HALFWORD_COPY_RUNTIME: u32 = 0x8007_E640;
pub const QUEST_MAP_HALFWORD_FILL_RUNTIME: u32 = 0x8007_E6B4;
pub const QUEST_MAP_FIRST_PLANE_LOADER_RUNTIME: u32 = 0x8007_6E50;
pub const QUEST_MAP_SECOND_PLANE_LOADER_RUNTIME: u32 = 0x8007_6F04;
pub const QUEST_MAP_COLLISION_PLANE_LOADER_RUNTIME: u32 = 0x8007_6F98;
pub const QUEST_MAP_COLLISION_PUBLISH_RUNTIME: u32 = 0x8007_3808;

pub const QUEST_COLLISION_CLASSIFIER_RUNTIME: u32 = 0x8006_B928;
pub const QUEST_MAP_MOVEMENT_RUNTIME: u32 = 0x8006_BB9C;
pub const QUEST_PLAYER_MAP_MOVEMENT_CALL_RUNTIME: u32 = 0x8005_7EA0;
pub const QUEST_PLAYER_OBJECT_MOVEMENT_CALL_RUNTIME: u32 = 0x8005_7EC4;
pub const QUEST_PLAYER_COLLISION_CANCEL_RUNTIME: u32 = 0x8005_8000;
pub const QUEST_PLAYER_POSITION_COMMIT_RUNTIME: u32 = 0x8005_81C0;
pub const QUEST_PLAYER_NORMAL_MOVEMENT_STATE: i16 = 0;
pub const QUEST_PLAYER_NORMAL_MOVEMENT_INPUT_RUNTIME: u32 = QUEST_PLAYER_HELD_INPUT_RUNTIME;
pub const QUEST_PLAYER_NORMAL_MOVEMENT_INPUT_MASKS: [u16; 4] = [0x4000, 0x8000, 0x1000, 0x2000];
pub const QUEST_PLAYER_NORMAL_MOVEMENT_DELTAS_FIXED: [[i32; 2]; 4] =
    [[0, 0x2000], [-0x2000, 0], [0, -0x2000], [0x2000, 0]];
pub const QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS: i32 = 2;
pub const QUEST_COLLISION_RECTANGLE_OVERLAP_RUNTIME: u32 = 0x8006_B61C;
pub const QUEST_COLLISION_MOVEMENT_ADJUST_RUNTIME: u32 = 0x8006_B7BC;
pub const QUEST_COLLISION_ACTOR_FIXED_X_OFFSET: i16 = 0xA8;
pub const QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET: i16 = 0xAC;
pub const QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS: [i16; 4] = [0xB0, 0xB2, 0xB4, 0xB6];
pub const QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET: i16 = 0xBA;
pub const QUEST_COLLISION_ACTOR_MAP_MEMBER_MASK: u16 = 0x0001;
pub const QUEST_COLLISION_FIXED_POINT_SHIFT: u8 = 12;
pub const QUEST_COLLISION_TILE_SIZE_PIXELS: u8 = 16;
pub const QUEST_COLLISION_TILE_INTERIOR_INSET_PIXELS: u8 = 1;
pub const QUEST_COLLISION_TILE_INTERIOR_EXTENT_PIXELS: u8 = 14;
pub const QUEST_COLLISION_PROBE_EXTENT_MARGIN_PIXELS: u8 = 4;
pub const QUEST_COLLISION_MAP_BOUNDS_RUNTIMES: [u32; 4] =
    [0x800B_E5C8, 0x800B_E5CA, 0x800B_E5CC, 0x800B_E5CE];
pub const QUEST_COLLISION_TILE_LOW_BYTE_MASK: u16 = 0x00FF;
pub const QUEST_COLLISION_TILE_FLAG_MASKS: [u16; 2] = [0x4000, 0x8000];
pub const QUEST_COLLISION_EXPLICIT_LOW_BYTES: [u8; 8] = [1, 2, 6, 97, 99, 102, 109, 110];
pub const QUEST_COLLISION_EXPLICIT_RANGE_START: u8 = 112;
pub const QUEST_COLLISION_EXPLICIT_RANGE_END: u8 = 127;
pub const QUEST_COLLISION_NONE_CLASS: u8 = 0;
pub const QUEST_COLLISION_SPECIAL_CLASS: u8 = 1;
pub const QUEST_COLLISION_SOLID_CLASS: u8 = 2;
pub const QUEST_COLLISION_ACTOR_LAYER_OFFSET: i16 = 0x54;
pub const QUEST_COLLISION_ACTOR_DIRECTION_OFFSET: i16 = 0xA5;
pub const QUEST_COLLISION_ACTOR_STATE_OFFSET: i16 = 0xC6;
pub const QUEST_COLLISION_PRIMARY_ACTOR_RUNTIME: u32 = 0x800B_E600;
pub const QUEST_COLLISION_INPUT_STATE_RUNTIME: u32 = 0x800F_BF52;
pub const QUEST_COLLISION_INPUT_STATE_MASK: u16 = 0x0004;
pub const QUEST_COLLISION_CONDITION_RUNTIMES: [u32; 2] = [0x800B_E5FC, 0x800B_E5FD];
pub const QUEST_COLLISION_CONDITION_OPCODE: u8 = 0x92;
pub const QUEST_COLLISION_CONDITION_WRITER_RUNTIME: u32 = 0x8007_8614;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestMapCollisionSummary {
    pub active_slot_runtime: u32,
    pub slot_buffer_roots_runtime: u32,
    pub slot_widths_runtime: u32,
    pub slot_heights_runtime: u32,
    pub collision_buffer_runtime: u32,
    pub slot_count: usize,
    pub plane_capacity_cells: usize,
    pub plane_stride_bytes: u32,
    pub collision_plane_offset: u32,
    pub plane_loader_runtimes: [u32; 3],
    pub qmp_header_bytes: usize,
    pub classifier_runtime: u32,
    pub movement_runtime: u32,
    pub player_movement_call_runtime: u32,
    pub player_object_movement_call_runtime: u32,
    pub player_collision_cancel_runtime: u32,
    pub player_position_commit_runtime: u32,
    pub player_normal_movement_state: i16,
    pub player_normal_movement_input_runtime: u32,
    pub player_normal_movement_input_masks: [u16; 4],
    pub player_normal_movement_deltas_fixed: [[i32; 2]; 4],
    pub player_normal_movement_step_pixels: i32,
    pub rectangle_overlap_runtime: u32,
    pub movement_adjust_runtime: u32,
    pub actor_fixed_position_offsets: [i16; 2],
    pub actor_rectangle_offsets: [i16; 4],
    pub actor_member_flags_offset: i16,
    pub actor_map_member_mask: u16,
    pub fixed_point_shift: u8,
    pub tile_size_pixels: u8,
    pub tile_interior_inset_pixels: u8,
    pub tile_interior_extent_pixels: u8,
    pub probe_extent_margin_pixels: u8,
    pub map_bounds_runtimes: [u32; 4],
    pub tile_low_byte_mask: u16,
    pub tile_flag_masks: [u16; 2],
    pub explicit_low_bytes: [u8; 8],
    pub explicit_range_start: u8,
    pub explicit_range_end: u8,
    pub none_class: u8,
    pub special_class: u8,
    pub solid_class: u8,
    pub actor_layer_offset: i16,
    pub actor_direction_offset: i16,
    pub actor_state_offset: i16,
    pub primary_actor_runtime: u32,
    pub condition_opcode: u8,
    pub condition_writer_runtime: u32,
    pub condition_runtimes: [u32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestMapCollisionError(String);

impl fmt::Display for QuestMapCollisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestMapCollisionError {}

fn error(message: impl Into<String>) -> QuestMapCollisionError {
    QuestMapCollisionError(message.into())
}

#[path = "quest_map_collision/evidence.rs"]
mod evidence;

use evidence::{required_data_words, required_instructions};

/// 지원 `QUEST.EXE`에서 QMP collision plane의 실제 소비 경계를 검증한다.
pub fn verify_box_quest_map_collision(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestMapCollisionSummary, QuestMapCollisionError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST map collision 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len
        )));
    }
    if quest_seq_opcode_handler(QUEST_COLLISION_CONDITION_OPCODE)
        != Some(QUEST_COLLISION_CONDITION_WRITER_RUNTIME)
    {
        return Err(error(format!(
            "QUEST map collision OP92 handler 불일치: opcode=0x{:02X}",
            QUEST_COLLISION_CONDITION_OPCODE
        )));
    }

    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error("QUEST.EXE text 길이가 4바이트 정렬이 아님"))?;
    for (address, expected, kind) in required_instructions() {
        require(&instructions, address, expected, kind)?;
    }
    for (address, expected, word) in required_data_words() {
        require_data_word(map, text, address, expected, word)?;
    }

    Ok(QuestMapCollisionSummary {
        active_slot_runtime: QUEST_MAP_ACTIVE_SLOT_RUNTIME,
        slot_buffer_roots_runtime: QUEST_MAP_SLOT_BUFFER_ROOTS_RUNTIME,
        slot_widths_runtime: QUEST_MAP_SLOT_WIDTHS_RUNTIME,
        slot_heights_runtime: QUEST_MAP_SLOT_HEIGHTS_RUNTIME,
        collision_buffer_runtime: QUEST_MAP_COLLISION_BUFFER_RUNTIME,
        slot_count: QUEST_MAP_SLOT_COUNT,
        plane_capacity_cells: QUEST_MAP_PLANE_CAPACITY_CELLS,
        plane_stride_bytes: QUEST_MAP_PLANE_STRIDE_BYTES,
        collision_plane_offset: QUEST_COLLISION_PLANE_OFFSET,
        plane_loader_runtimes: [
            QUEST_MAP_FIRST_PLANE_LOADER_RUNTIME,
            QUEST_MAP_SECOND_PLANE_LOADER_RUNTIME,
            QUEST_MAP_COLLISION_PLANE_LOADER_RUNTIME,
        ],
        qmp_header_bytes: QMP_HEADER_LEN,
        classifier_runtime: QUEST_COLLISION_CLASSIFIER_RUNTIME,
        movement_runtime: QUEST_MAP_MOVEMENT_RUNTIME,
        player_movement_call_runtime: QUEST_PLAYER_MAP_MOVEMENT_CALL_RUNTIME,
        player_object_movement_call_runtime: QUEST_PLAYER_OBJECT_MOVEMENT_CALL_RUNTIME,
        player_collision_cancel_runtime: QUEST_PLAYER_COLLISION_CANCEL_RUNTIME,
        player_position_commit_runtime: QUEST_PLAYER_POSITION_COMMIT_RUNTIME,
        player_normal_movement_state: QUEST_PLAYER_NORMAL_MOVEMENT_STATE,
        player_normal_movement_input_runtime: QUEST_PLAYER_NORMAL_MOVEMENT_INPUT_RUNTIME,
        player_normal_movement_input_masks: QUEST_PLAYER_NORMAL_MOVEMENT_INPUT_MASKS,
        player_normal_movement_deltas_fixed: QUEST_PLAYER_NORMAL_MOVEMENT_DELTAS_FIXED,
        player_normal_movement_step_pixels: QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS,
        rectangle_overlap_runtime: QUEST_COLLISION_RECTANGLE_OVERLAP_RUNTIME,
        movement_adjust_runtime: QUEST_COLLISION_MOVEMENT_ADJUST_RUNTIME,
        actor_fixed_position_offsets: [
            QUEST_COLLISION_ACTOR_FIXED_X_OFFSET,
            QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET,
        ],
        actor_rectangle_offsets: QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS,
        actor_member_flags_offset: QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET,
        actor_map_member_mask: QUEST_COLLISION_ACTOR_MAP_MEMBER_MASK,
        fixed_point_shift: QUEST_COLLISION_FIXED_POINT_SHIFT,
        tile_size_pixels: QUEST_COLLISION_TILE_SIZE_PIXELS,
        tile_interior_inset_pixels: QUEST_COLLISION_TILE_INTERIOR_INSET_PIXELS,
        tile_interior_extent_pixels: QUEST_COLLISION_TILE_INTERIOR_EXTENT_PIXELS,
        probe_extent_margin_pixels: QUEST_COLLISION_PROBE_EXTENT_MARGIN_PIXELS,
        map_bounds_runtimes: QUEST_COLLISION_MAP_BOUNDS_RUNTIMES,
        tile_low_byte_mask: QUEST_COLLISION_TILE_LOW_BYTE_MASK,
        tile_flag_masks: QUEST_COLLISION_TILE_FLAG_MASKS,
        explicit_low_bytes: QUEST_COLLISION_EXPLICIT_LOW_BYTES,
        explicit_range_start: QUEST_COLLISION_EXPLICIT_RANGE_START,
        explicit_range_end: QUEST_COLLISION_EXPLICIT_RANGE_END,
        none_class: QUEST_COLLISION_NONE_CLASS,
        special_class: QUEST_COLLISION_SPECIAL_CLASS,
        solid_class: QUEST_COLLISION_SOLID_CLASS,
        actor_layer_offset: QUEST_COLLISION_ACTOR_LAYER_OFFSET,
        actor_direction_offset: QUEST_COLLISION_ACTOR_DIRECTION_OFFSET,
        actor_state_offset: QUEST_COLLISION_ACTOR_STATE_OFFSET,
        primary_actor_runtime: QUEST_COLLISION_PRIMARY_ACTOR_RUNTIME,
        condition_opcode: QUEST_COLLISION_CONDITION_OPCODE,
        condition_writer_runtime: QUEST_COLLISION_CONDITION_WRITER_RUNTIME,
        condition_runtimes: QUEST_COLLISION_CONDITION_RUNTIMES,
    })
}

fn require_data_word(
    map: &ExeMap,
    text: &[u8],
    address: u32,
    expected: &str,
    word: u32,
) -> Result<(), QuestMapCollisionError> {
    let offset = map.text_offset(address).ok_or_else(|| {
        error(format!(
            "QUEST map collision data가 text 밖임: 0x{address:08X}"
        ))
    })?;
    let bytes = text
        .get(offset..offset + 4)
        .ok_or_else(|| error(format!("QUEST map collision data가 잘림: 0x{address:08X}")))?;
    let actual = u32::from_le_bytes(
        bytes
            .try_into()
            .map_err(|_| error("QUEST map collision data word 길이 불일치"))?,
    );
    if actual != word {
        return Err(error(format!(
            "QUEST map collision data 불일치: address=0x{address:08X}, expected={expected} 0x{word:08X}, actual=0x{actual:08X}"
        )));
    }
    Ok(())
}

fn require(
    instructions: &[Instruction],
    address: u32,
    expected: &str,
    kind: K,
) -> Result<(), QuestMapCollisionError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| {
            error(format!(
                "QUEST map collision 명령이 text 밖임: 0x{address:08X}"
            ))
        })?;
    if actual.kind != kind {
        return Err(error(format!(
            "QUEST map collision 명령 불일치: address=0x{address:08X}, \
             expected={expected}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_map_collision_tests.rs"]
mod tests;
