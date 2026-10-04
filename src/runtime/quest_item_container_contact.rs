//! exact QUEST state의 OP8E item container와 상호작용 접촉면을 결속한다.
//!
//! object pool에서 source-bound callback과 호출자가 지정한 world anchor가 정확히
//! 일치하는 객체를 고른다. 플레이어 callback·layer·사각형·초기 state를 확인한
//! 뒤 정상 개방 방향에서 충분한 probe overlap을 갖는 접촉면을 계산하고, 현재
//! player의 2px 보행 격자에서 닿는 anchor까지 모든 OP8E container를 피하는
//! map route를 만든다. 다른 종류의 동적 객체와 polling timing은 모델 밖이다.

use std::error::Error;
use std::fmt;

use crate::disasm::quest_font_object_lifetime::{
    QUEST_OBJECT_CALLBACK_FIELD_OFFSET, QUEST_OBJECT_POOL_RECORD_BYTES,
    QUEST_OBJECT_POOL_RECORD_COUNT, QUEST_OBJECT_POOL_RUNTIME,
};
use crate::disasm::quest_item_container::{
    QUEST_ITEM_CONTAINER_CALLBACK_RUNTIME, QUEST_ITEM_CONTAINER_ITEM_ID_FIELD_OFFSET,
    QUEST_ITEM_CONTAINER_LOCAL_RECTANGLE, QUEST_ITEM_CONTAINER_OPEN_CONTACT_MASK,
    QUEST_ITEM_CONTAINER_OPEN_DIRECTION, QUEST_ITEM_CONTAINER_PARAMETER_FIELD_OFFSET,
    QUEST_ITEM_CONTAINER_PERSISTENCE_FIELD_OFFSET,
};
use crate::disasm::quest_map_collision::{
    quest_collision_actor_rectangle, quest_collision_rectangle_overlap,
    QuestCollisionActorGeometry, QuestCollisionGeometryError, QuestCollisionOverlap,
    QuestCollisionRectangle, QUEST_COLLISION_ACTOR_FIXED_X_OFFSET,
    QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET, QUEST_COLLISION_ACTOR_LAYER_OFFSET,
    QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET, QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS,
    QUEST_COLLISION_FIXED_POINT_SHIFT, QUEST_COLLISION_PRIMARY_ACTOR_RUNTIME,
    QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS,
};
use crate::disasm::quest_player_contact::QUEST_PLAYER_UPDATE_CALLBACK_RUNTIME;
use crate::disasm::quest_player_interaction_contact::{
    quest_player_interaction_local_probe, quest_player_interaction_probe_rectangle,
    QuestPlayerInteractionContactError, QUEST_PLAYER_INTERACTION_DIRECTION_CONTACT_MASKS,
    QUEST_PLAYER_INTERACTION_INPUT_MASK, QUEST_PLAYER_INTERACTION_INPUT_RUNTIME,
};
use crate::disasm::quest_seq_child_activation::{
    QUEST_SEQ_CHILD_COLLISION_MEMBER_MASK, QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
    QUEST_SEQ_CHILD_STATE_OFFSET,
};

use super::{
    find_direction_robust_quest_map_walk_route_to_any_avoiding_rectangles, MednafenPsxState,
    MednafenStateError, QuestMapCollisionStateReport, QuestMapWalkRoute, QuestMapWalkRouteError,
};

const FIXED_POINT_SCALE: i32 = 1 << QUEST_COLLISION_FIXED_POINT_SHIFT;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemContainerRuntimeObject {
    pub pool_index: usize,
    pub object_runtime: u32,
    pub callback_runtime: u32,
    pub layer: u32,
    pub collision_member_flags: u8,
    pub geometry: QuestCollisionActorGeometry,
    pub world_rectangle: QuestCollisionRectangle,
    pub contact_flags: u16,
    pub state: i16,
    pub persistence_selector: u8,
    pub parameter: i16,
    pub item_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemContainerContactReport {
    pub primary_actor_runtime: u32,
    pub player_callback_runtime: u32,
    pub player_layer: u32,
    pub container_object_count: usize,
    pub target_anchor_pixel: [i32; 2],
    pub target: QuestItemContainerRuntimeObject,
    pub interaction_input_runtime: u32,
    pub interaction_input_mask: u16,
    pub interaction_direction: u8,
    pub interaction_contact_mask: u16,
    pub centered_contact_anchor_pixel: [i32; 2],
    pub contact_anchor_candidate_count: usize,
    pub contact_anchor_pixel: [i32; 2],
    pub player_rectangle_at_contact: QuestCollisionRectangle,
    pub interaction_probe_at_contact: QuestCollisionRectangle,
    pub interaction_overlap: QuestCollisionOverlap,
    pub route: QuestMapWalkRoute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestItemContainerContactError {
    CoordinateOverflow,
    PrimaryActorMismatch {
        report: u32,
        state: u32,
    },
    PlayerLayerMismatch {
        report: u32,
        state: u32,
    },
    PlayerMemberFlagsMismatch {
        report: u8,
        state: u8,
    },
    PlayerGeometryMismatch {
        report: QuestCollisionActorGeometry,
        state: QuestCollisionActorGeometry,
    },
    UnsupportedPlayerCallback(u32),
    NoContainerAtAnchor([i32; 2]),
    MultipleContainersAtAnchor {
        anchor: [i32; 2],
        count: usize,
    },
    TargetLayerMismatch {
        player: u32,
        target: u32,
    },
    TargetCollisionMembershipDisabled(u8),
    TargetGeometryDrift {
        actual: [i16; 4],
    },
    TargetNotIdle(i16),
    TargetContactFlagsNotClear(u16),
    SourceDirectionMaskDrift {
        direction: u8,
        mask: u16,
    },
    ContactPlayerOverlapsTarget,
    ContactProbeMissesTarget,
    NoWalkLatticeContactCandidates,
    State(MednafenStateError),
    Geometry(QuestCollisionGeometryError),
    Interaction(QuestPlayerInteractionContactError),
    WalkRoute(QuestMapWalkRouteError),
}

impl fmt::Display for QuestItemContainerContactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinateOverflow => formatter.write_str("QUEST item container 좌표 overflow"),
            Self::PrimaryActorMismatch { report, state } => write!(
                formatter,
                "QUEST item container state와 map report의 primary actor가 다름: report=0x{report:08X}, state=0x{state:08X}",
            ),
            Self::PlayerLayerMismatch { report, state } => write!(
                formatter,
                "QUEST item container state와 map report의 player layer가 다름: report={report}, state={state}",
            ),
            Self::PlayerMemberFlagsMismatch { report, state } => write!(
                formatter,
                "QUEST item container state와 map report의 player member flags가 다름: report=0x{report:02X}, state=0x{state:02X}",
            ),
            Self::PlayerGeometryMismatch { report, state } => write!(
                formatter,
                "QUEST item container state와 map report의 player geometry가 다름: report={report:?}, state={state:?}",
            ),
            Self::UnsupportedPlayerCallback(callback) => write!(
                formatter,
                "QUEST item container 상호작용을 지원하지 않는 player callback: 0x{callback:08X}",
            ),
            Self::NoContainerAtAnchor(anchor) => write!(
                formatter,
                "QUEST item container가 지정 anchor에 없음: ({},{})",
                anchor[0], anchor[1],
            ),
            Self::MultipleContainersAtAnchor { anchor, count } => write!(
                formatter,
                "QUEST item container가 지정 anchor에서 유일하지 않음: ({},{}), count={count}",
                anchor[0], anchor[1],
            ),
            Self::TargetLayerMismatch { player, target } => write!(
                formatter,
                "QUEST item container와 player layer가 다름: player={player}, target={target}",
            ),
            Self::TargetCollisionMembershipDisabled(flags) => write!(
                formatter,
                "QUEST item container collision membership bit가 꺼져 있음: flags=0x{flags:02X}",
            ),
            Self::TargetGeometryDrift { actual } => write!(
                formatter,
                "QUEST item container local rectangle이 OP8E source와 다름: expected={QUEST_ITEM_CONTAINER_LOCAL_RECTANGLE:?}, actual={actual:?}",
            ),
            Self::TargetNotIdle(state) => write!(
                formatter,
                "QUEST item container가 개방 대기 state 0이 아님: state={state}",
            ),
            Self::TargetContactFlagsNotClear(flags) => write!(
                formatter,
                "QUEST item container의 기존 contact flags가 남아 있음: flags=0x{flags:04X}",
            ),
            Self::SourceDirectionMaskDrift { direction, mask } => write!(
                formatter,
                "QUEST item container 개방 방향과 player contact mask가 다름: direction={direction}, mask=0x{mask:04X}",
            ),
            Self::ContactPlayerOverlapsTarget => formatter.write_str(
                "QUEST item container 접촉 anchor에서 player 본체가 target과 겹침",
            ),
            Self::ContactProbeMissesTarget => formatter.write_str(
                "QUEST item container 접촉 anchor의 interaction probe가 target을 놓침",
            ),
            Self::NoWalkLatticeContactCandidates => formatter.write_str(
                "QUEST item container 접촉면에 현재 2px 보행 격자와 맞는 anchor가 없음",
            ),
            Self::State(source) => write!(formatter, "QUEST item container state 읽기 실패: {source}"),
            Self::Geometry(source) => write!(formatter, "QUEST item container geometry 실패: {source}"),
            Self::Interaction(source) => write!(formatter, "QUEST item container interaction 실패: {source}"),
            Self::WalkRoute(source) => {
                write!(formatter, "QUEST item container 정상 보행 route 실패: {source}")
            }
        }
    }
}

impl Error for QuestItemContainerContactError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State(source) => Some(source),
            Self::Geometry(source) => Some(source),
            Self::Interaction(source) => Some(source),
            Self::WalkRoute(source) => Some(source),
            _ => None,
        }
    }
}

impl From<MednafenStateError> for QuestItemContainerContactError {
    fn from(value: MednafenStateError) -> Self {
        Self::State(value)
    }
}

impl From<QuestCollisionGeometryError> for QuestItemContainerContactError {
    fn from(value: QuestCollisionGeometryError) -> Self {
        Self::Geometry(value)
    }
}

impl From<QuestPlayerInteractionContactError> for QuestItemContainerContactError {
    fn from(value: QuestPlayerInteractionContactError) -> Self {
        Self::Interaction(value)
    }
}

impl From<QuestMapWalkRouteError> for QuestItemContainerContactError {
    fn from(value: QuestMapWalkRouteError) -> Self {
        Self::WalkRoute(value)
    }
}

fn runtime_field(base: u32, offset: i16) -> Result<u32, QuestItemContainerContactError> {
    let offset =
        u32::try_from(offset).map_err(|_| QuestItemContainerContactError::CoordinateOverflow)?;
    base.checked_add(offset)
        .ok_or(QuestItemContainerContactError::CoordinateOverflow)
}

fn pool_object_runtime(index: usize) -> Result<u32, QuestItemContainerContactError> {
    let offset = index
        .checked_mul(QUEST_OBJECT_POOL_RECORD_BYTES)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(QuestItemContainerContactError::CoordinateOverflow)?;
    QUEST_OBJECT_POOL_RUNTIME
        .checked_add(offset)
        .ok_or(QuestItemContainerContactError::CoordinateOverflow)
}

fn read_actor_geometry(
    state: &MednafenPsxState<'_>,
    object_runtime: u32,
) -> Result<QuestCollisionActorGeometry, QuestItemContainerContactError> {
    Ok(QuestCollisionActorGeometry {
        position_fixed: [
            state.read_u32_le(runtime_field(
                object_runtime,
                QUEST_COLLISION_ACTOR_FIXED_X_OFFSET,
            )?)? as i32,
            state.read_u32_le(runtime_field(
                object_runtime,
                QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET,
            )?)? as i32,
        ],
        local_rectangle: [
            state.read_u16_le(runtime_field(
                object_runtime,
                QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[0],
            )?)? as i16,
            state.read_u16_le(runtime_field(
                object_runtime,
                QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[1],
            )?)? as i16,
            state.read_u16_le(runtime_field(
                object_runtime,
                QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[2],
            )?)? as i16,
            state.read_u16_le(runtime_field(
                object_runtime,
                QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[3],
            )?)? as i16,
        ],
    })
}

fn read_container(
    state: &MednafenPsxState<'_>,
    pool_index: usize,
    object_runtime: u32,
) -> Result<QuestItemContainerRuntimeObject, QuestItemContainerContactError> {
    let geometry = read_actor_geometry(state, object_runtime)?;
    Ok(QuestItemContainerRuntimeObject {
        pool_index,
        object_runtime,
        callback_runtime: state.read_u32_le(runtime_field(
            object_runtime,
            QUEST_OBJECT_CALLBACK_FIELD_OFFSET,
        )?)?,
        layer: state.read_u32_le(runtime_field(
            object_runtime,
            QUEST_COLLISION_ACTOR_LAYER_OFFSET,
        )?)?,
        collision_member_flags: state.read_u8(runtime_field(
            object_runtime,
            QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET,
        )?)?,
        geometry,
        world_rectangle: quest_collision_actor_rectangle(geometry, [0, 0])?,
        contact_flags: state.read_u16_le(runtime_field(
            object_runtime,
            QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET,
        )?)?,
        state: state.read_u16_le(runtime_field(object_runtime, QUEST_SEQ_CHILD_STATE_OFFSET)?)?
            as i16,
        persistence_selector: state.read_u8(runtime_field(
            object_runtime,
            QUEST_ITEM_CONTAINER_PERSISTENCE_FIELD_OFFSET,
        )?)?,
        parameter: state.read_u16_le(runtime_field(
            object_runtime,
            QUEST_ITEM_CONTAINER_PARAMETER_FIELD_OFFSET,
        )?)? as i16,
        item_id: state.read_u32_le(runtime_field(
            object_runtime,
            QUEST_ITEM_CONTAINER_ITEM_ID_FIELD_OFFSET,
        )?)?,
    })
}

fn rectangle_center(start: i32, extent: i32) -> Result<i32, QuestItemContainerContactError> {
    start
        .checked_add(extent / 2)
        .ok_or(QuestItemContainerContactError::CoordinateOverflow)
}

fn contact_anchor(
    player_local: [i16; 4],
    target: QuestCollisionRectangle,
    direction: u8,
) -> Result<[i32; 2], QuestItemContainerContactError> {
    let probe = quest_player_interaction_local_probe(player_local, direction)?;
    let body = player_local.map(i32::from);
    let target_right = target
        .x
        .checked_add(target.horizontal_extent)
        .ok_or(QuestItemContainerContactError::CoordinateOverflow)?;
    let target_bottom = target
        .y
        .checked_add(target.vertical_extent)
        .ok_or(QuestItemContainerContactError::CoordinateOverflow)?;
    let body_right = body[0]
        .checked_add(body[2])
        .ok_or(QuestItemContainerContactError::CoordinateOverflow)?;
    let body_bottom = body[1]
        .checked_add(body[3])
        .ok_or(QuestItemContainerContactError::CoordinateOverflow)?;
    let target_center_x = rectangle_center(target.x, target.horizontal_extent)?;
    let target_center_y = rectangle_center(target.y, target.vertical_extent)?;
    let probe_center_x = rectangle_center(probe.x, probe.horizontal_extent)?;
    let probe_center_y = rectangle_center(probe.y, probe.vertical_extent)?;

    match direction {
        // 아래를 보는 player는 target 북쪽에 선다.
        0 => Ok([
            target_center_x
                .checked_sub(probe_center_x)
                .ok_or(QuestItemContainerContactError::CoordinateOverflow)?,
            target
                .y
                .checked_sub(1)
                .and_then(|value| value.checked_sub(body_bottom))
                .ok_or(QuestItemContainerContactError::CoordinateOverflow)?,
        ]),
        // 왼쪽을 보는 player는 target 동쪽에 선다.
        1 => Ok([
            target_right
                .checked_add(1)
                .and_then(|value| value.checked_sub(body[0]))
                .ok_or(QuestItemContainerContactError::CoordinateOverflow)?,
            target_center_y
                .checked_sub(probe_center_y)
                .ok_or(QuestItemContainerContactError::CoordinateOverflow)?,
        ]),
        // 위를 보는 player는 target 남쪽에 선다.
        2 => Ok([
            target_center_x
                .checked_sub(probe_center_x)
                .ok_or(QuestItemContainerContactError::CoordinateOverflow)?,
            target_bottom
                .checked_add(1)
                .and_then(|value| value.checked_sub(body[1]))
                .ok_or(QuestItemContainerContactError::CoordinateOverflow)?,
        ]),
        // 오른쪽을 보는 player는 target 서쪽에 선다.
        3 => Ok([
            target
                .x
                .checked_sub(1)
                .and_then(|value| value.checked_sub(body_right))
                .ok_or(QuestItemContainerContactError::CoordinateOverflow)?,
            target_center_y
                .checked_sub(probe_center_y)
                .ok_or(QuestItemContainerContactError::CoordinateOverflow)?,
        ]),
        _ => Err(QuestItemContainerContactError::SourceDirectionMaskDrift { direction, mask: 0 }),
    }
}

fn rectangle_right(
    rectangle: QuestCollisionRectangle,
) -> Result<i32, QuestItemContainerContactError> {
    rectangle
        .x
        .checked_add(rectangle.horizontal_extent)
        .ok_or(QuestItemContainerContactError::CoordinateOverflow)
}

fn rectangle_bottom(
    rectangle: QuestCollisionRectangle,
) -> Result<i32, QuestItemContainerContactError> {
    rectangle
        .y
        .checked_add(rectangle.vertical_extent)
        .ok_or(QuestItemContainerContactError::CoordinateOverflow)
}

fn faces_target_from_expected_side(
    body: QuestCollisionRectangle,
    target: QuestCollisionRectangle,
    direction: u8,
) -> Result<bool, QuestItemContainerContactError> {
    Ok(match direction {
        0 => rectangle_bottom(body)? < target.y,
        1 => body.x > rectangle_right(target)?,
        2 => body.y > rectangle_bottom(target)?,
        3 => rectangle_right(body)? < target.x,
        _ => {
            return Err(QuestItemContainerContactError::SourceDirectionMaskDrift {
                direction,
                mask: 0,
            })
        }
    })
}

fn contact_anchor_candidates(
    player: QuestCollisionActorGeometry,
    target: QuestCollisionRectangle,
    direction: u8,
) -> Result<Vec<[i32; 2]>, QuestItemContainerContactError> {
    let start_pixel = [
        player.position_fixed[0] / FIXED_POINT_SCALE,
        player.position_fixed[1] / FIXED_POINT_SCALE,
    ];
    let local_probe = quest_player_interaction_local_probe(player.local_rectangle, direction)?;
    let target_right = rectangle_right(target)?;
    let target_bottom = rectangle_bottom(target)?;
    let probe_right = rectangle_right(local_probe)?;
    let probe_bottom = rectangle_bottom(local_probe)?;
    let min_x = target
        .x
        .checked_sub(probe_right)
        .ok_or(QuestItemContainerContactError::CoordinateOverflow)?;
    let max_x = target_right
        .checked_sub(local_probe.x)
        .ok_or(QuestItemContainerContactError::CoordinateOverflow)?;
    let min_y = target
        .y
        .checked_sub(probe_bottom)
        .ok_or(QuestItemContainerContactError::CoordinateOverflow)?;
    let max_y = target_bottom
        .checked_sub(local_probe.y)
        .ok_or(QuestItemContainerContactError::CoordinateOverflow)?;

    let required_cross_axis_overlap = match direction {
        0 | 2 => local_probe
            .horizontal_extent
            .checked_add(1)
            .ok_or(QuestItemContainerContactError::CoordinateOverflow)?,
        1 | 3 => local_probe
            .vertical_extent
            .checked_add(1)
            .ok_or(QuestItemContainerContactError::CoordinateOverflow)?,
        _ => {
            return Err(QuestItemContainerContactError::SourceDirectionMaskDrift {
                direction,
                mask: 0,
            })
        }
    };
    let mut candidates = Vec::new();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if (x - start_pixel[0]).rem_euclid(QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS) != 0
                || (y - start_pixel[1]).rem_euclid(QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS) != 0
            {
                continue;
            }
            let geometry = QuestCollisionActorGeometry {
                position_fixed: [
                    x.checked_mul(FIXED_POINT_SCALE)
                        .ok_or(QuestItemContainerContactError::CoordinateOverflow)?,
                    y.checked_mul(FIXED_POINT_SCALE)
                        .ok_or(QuestItemContainerContactError::CoordinateOverflow)?,
                ],
                local_rectangle: player.local_rectangle,
            };
            let body = quest_collision_actor_rectangle(geometry, [0, 0])?;
            if !faces_target_from_expected_side(body, target, direction)?
                || quest_collision_rectangle_overlap(body, target)?.is_some()
            {
                continue;
            }
            let probe = quest_player_interaction_probe_rectangle(geometry, direction)?;
            let Some(overlap) = quest_collision_rectangle_overlap(probe, target)? else {
                continue;
            };
            let cross_axis_overlap = match direction {
                0 | 2 => overlap.width_pixels,
                1 | 3 => overlap.height_pixels,
                _ => unreachable!("direction은 위에서 검증됨"),
            };
            if cross_axis_overlap == required_cross_axis_overlap {
                candidates.push([x, y]);
            }
        }
    }
    if candidates.is_empty() {
        return Err(QuestItemContainerContactError::NoWalkLatticeContactCandidates);
    }
    Ok(candidates)
}

/// exact state에서 지정한 world anchor의 OP8E item container 접촉 경로를 만든다.
pub fn inspect_quest_item_container_contact_state(
    state: &MednafenPsxState<'_>,
    map_report: &QuestMapCollisionStateReport,
    target_anchor_pixel: [i32; 2],
) -> Result<QuestItemContainerContactReport, QuestItemContainerContactError> {
    let primary_actor_runtime = state.read_u32_le(QUEST_COLLISION_PRIMARY_ACTOR_RUNTIME)?;
    if primary_actor_runtime != map_report.primary_actor_runtime {
        return Err(QuestItemContainerContactError::PrimaryActorMismatch {
            report: map_report.primary_actor_runtime,
            state: primary_actor_runtime,
        });
    }
    let player_layer = state.read_u32_le(runtime_field(
        primary_actor_runtime,
        QUEST_COLLISION_ACTOR_LAYER_OFFSET,
    )?)?;
    if player_layer != map_report.classifier_context.actor_layer {
        return Err(QuestItemContainerContactError::PlayerLayerMismatch {
            report: map_report.classifier_context.actor_layer,
            state: player_layer,
        });
    }
    let player_member_flags = state.read_u8(runtime_field(
        primary_actor_runtime,
        QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET,
    )?)?;
    if player_member_flags != map_report.actor_member_flags {
        return Err(QuestItemContainerContactError::PlayerMemberFlagsMismatch {
            report: map_report.actor_member_flags,
            state: player_member_flags,
        });
    }
    let player_geometry = read_actor_geometry(state, primary_actor_runtime)?;
    if player_geometry != map_report.actor_geometry {
        return Err(QuestItemContainerContactError::PlayerGeometryMismatch {
            report: map_report.actor_geometry,
            state: player_geometry,
        });
    }
    let player_callback_runtime = state.read_u32_le(runtime_field(
        primary_actor_runtime,
        QUEST_OBJECT_CALLBACK_FIELD_OFFSET,
    )?)?;
    if player_callback_runtime != QUEST_PLAYER_UPDATE_CALLBACK_RUNTIME {
        return Err(QuestItemContainerContactError::UnsupportedPlayerCallback(
            player_callback_runtime,
        ));
    }

    let mut containers = Vec::new();
    for pool_index in 0..QUEST_OBJECT_POOL_RECORD_COUNT {
        let object_runtime = pool_object_runtime(pool_index)?;
        let callback_runtime = state.read_u32_le(runtime_field(
            object_runtime,
            QUEST_OBJECT_CALLBACK_FIELD_OFFSET,
        )?)?;
        if callback_runtime == QUEST_ITEM_CONTAINER_CALLBACK_RUNTIME {
            containers.push(read_container(state, pool_index, object_runtime)?);
        }
    }
    let matching = containers
        .iter()
        .filter(|container| {
            [
                container.geometry.position_fixed[0] / FIXED_POINT_SCALE,
                container.geometry.position_fixed[1] / FIXED_POINT_SCALE,
            ] == target_anchor_pixel
        })
        .cloned()
        .collect::<Vec<_>>();
    let target = match matching.as_slice() {
        [] => {
            return Err(QuestItemContainerContactError::NoContainerAtAnchor(
                target_anchor_pixel,
            ))
        }
        [target] => target.clone(),
        _ => {
            return Err(QuestItemContainerContactError::MultipleContainersAtAnchor {
                anchor: target_anchor_pixel,
                count: matching.len(),
            })
        }
    };

    if target.layer != player_layer {
        return Err(QuestItemContainerContactError::TargetLayerMismatch {
            player: player_layer,
            target: target.layer,
        });
    }
    if u16::from(target.collision_member_flags) & QUEST_SEQ_CHILD_COLLISION_MEMBER_MASK == 0 {
        return Err(
            QuestItemContainerContactError::TargetCollisionMembershipDisabled(
                target.collision_member_flags,
            ),
        );
    }
    if target.geometry.local_rectangle != QUEST_ITEM_CONTAINER_LOCAL_RECTANGLE {
        return Err(QuestItemContainerContactError::TargetGeometryDrift {
            actual: target.geometry.local_rectangle,
        });
    }
    if target.state != 0 {
        return Err(QuestItemContainerContactError::TargetNotIdle(target.state));
    }
    if target.contact_flags != 0 {
        return Err(QuestItemContainerContactError::TargetContactFlagsNotClear(
            target.contact_flags,
        ));
    }

    let interaction_direction = QUEST_ITEM_CONTAINER_OPEN_DIRECTION;
    let interaction_contact_mask = *QUEST_PLAYER_INTERACTION_DIRECTION_CONTACT_MASKS
        .get(usize::from(interaction_direction))
        .ok_or(QuestItemContainerContactError::SourceDirectionMaskDrift {
            direction: interaction_direction,
            mask: 0,
        })?;
    if interaction_contact_mask != QUEST_ITEM_CONTAINER_OPEN_CONTACT_MASK {
        return Err(QuestItemContainerContactError::SourceDirectionMaskDrift {
            direction: interaction_direction,
            mask: interaction_contact_mask,
        });
    }

    let centered_contact_anchor_pixel = contact_anchor(
        map_report.actor_geometry.local_rectangle,
        target.world_rectangle,
        interaction_direction,
    )?;
    let contact_anchor_candidates = contact_anchor_candidates(
        map_report.actor_geometry,
        target.world_rectangle,
        interaction_direction,
    )?;
    let obstacle_rectangles = containers
        .iter()
        .map(|container| container.world_rectangle)
        .collect::<Vec<_>>();
    let route = find_direction_robust_quest_map_walk_route_to_any_avoiding_rectangles(
        map_report,
        &contact_anchor_candidates,
        &obstacle_rectangles,
    )?;
    let contact_anchor_pixel = route.route.goal_pixel;
    let contact_geometry = QuestCollisionActorGeometry {
        position_fixed: [
            contact_anchor_pixel[0]
                .checked_mul(FIXED_POINT_SCALE)
                .ok_or(QuestItemContainerContactError::CoordinateOverflow)?,
            contact_anchor_pixel[1]
                .checked_mul(FIXED_POINT_SCALE)
                .ok_or(QuestItemContainerContactError::CoordinateOverflow)?,
        ],
        local_rectangle: map_report.actor_geometry.local_rectangle,
    };
    let player_rectangle_at_contact = quest_collision_actor_rectangle(contact_geometry, [0, 0])?;
    if quest_collision_rectangle_overlap(player_rectangle_at_contact, target.world_rectangle)?
        .is_some()
    {
        return Err(QuestItemContainerContactError::ContactPlayerOverlapsTarget);
    }
    let interaction_probe_at_contact =
        quest_player_interaction_probe_rectangle(contact_geometry, interaction_direction)?;
    let interaction_overlap =
        quest_collision_rectangle_overlap(interaction_probe_at_contact, target.world_rectangle)?
            .ok_or(QuestItemContainerContactError::ContactProbeMissesTarget)?;
    Ok(QuestItemContainerContactReport {
        primary_actor_runtime,
        player_callback_runtime,
        player_layer,
        container_object_count: containers.len(),
        target_anchor_pixel,
        target,
        interaction_input_runtime: QUEST_PLAYER_INTERACTION_INPUT_RUNTIME,
        interaction_input_mask: QUEST_PLAYER_INTERACTION_INPUT_MASK,
        interaction_direction,
        interaction_contact_mask,
        centered_contact_anchor_pixel,
        contact_anchor_candidate_count: contact_anchor_candidates.len(),
        contact_anchor_pixel,
        player_rectangle_at_contact,
        interaction_probe_at_contact,
        interaction_overlap,
        route,
    })
}

#[cfg(test)]
#[path = "quest_item_container_contact_tests.rs"]
mod tests;
