//! QUEST 정상 보행 입력으로 재생 가능한 보수적 map 경로를 찾는다.
//!
//! player state 0의 방향키 입력은 한 축에 `±0x2000` fixed delta를 만들고,
//! map/object collision 뒤 남은 delta만 위치에 더한다. 이 모델은 매 전이의
//! 도착 사각형이 map collision과 명시한 객체에 전혀 닿지 않는 경우만 허용한다.
//! 따라서 원본 sliding이나 collision adjustment에 기대지 않는다. 목록에 없는
//! 동적 객체와 encounter, controller polling timing은 여전히 모델 밖이다.

use std::error::Error;
use std::fmt;

use crate::disasm::quest_map_collision::{
    QuestCollisionRectangle, QUEST_COLLISION_FIXED_POINT_SHIFT,
    QUEST_PLAYER_NORMAL_MOVEMENT_DELTAS_FIXED, QUEST_PLAYER_NORMAL_MOVEMENT_INPUT_MASKS,
    QUEST_PLAYER_NORMAL_MOVEMENT_INPUT_RUNTIME, QUEST_PLAYER_NORMAL_MOVEMENT_STATE,
    QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS,
};

use super::quest_map_route::find_direction_robust_quest_map_route_to_any_avoiding_rectangles;
use super::{QuestMapCollisionStateReport, QuestMapRoute, QuestMapRouteError};

const FIXED_POINT_SCALE: i32 = 1 << QUEST_COLLISION_FIXED_POINT_SHIFT;

/// 정상 보행 source 계약과 결속한 2px 단위 경로.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestMapWalkRoute {
    pub route: QuestMapRoute,
    pub normal_movement_state: i16,
    pub input_runtime: u32,
    /// actor 방향 0/1/2/3(아래/왼쪽/위/오른쪽) 순서.
    pub direction_input_masks: [u16; 4],
    /// actor 방향 0/1/2/3 순서의 fixed-point 위치 delta.
    pub direction_deltas_fixed: [[i32; 2]; 4],
    pub position_step_pixels: i32,
    pub movement_steps: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestMapWalkRouteError {
    UnsupportedActorState {
        expected: i16,
        actual: i16,
    },
    StartNotPixelAligned([i32; 2]),
    GoalOutsideMovementLattice {
        start: [i32; 2],
        goal: [i32; 2],
        step_pixels: i32,
    },
    Route(QuestMapRouteError),
}

impl fmt::Display for QuestMapWalkRouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedActorState { expected, actual } => write!(
                formatter,
                "QUEST 정상 보행 route를 적용할 수 없는 actor state: expected={expected}, actual={actual}",
            ),
            Self::StartNotPixelAligned(subpixel) => write!(
                formatter,
                "QUEST 정상 보행 route 시작점이 pixel 경계가 아님: subpixel fixed=({},{})",
                subpixel[0], subpixel[1],
            ),
            Self::GoalOutsideMovementLattice {
                start,
                goal,
                step_pixels,
            } => write!(
                formatter,
                "QUEST 목표가 정상 보행 도달 격자 밖임: start=({},{}), goal=({},{}), step={}px",
                start[0], start[1], goal[0], goal[1], step_pixels,
            ),
            Self::Route(source) => write!(formatter, "QUEST 정상 보행 route 실패: {source}"),
        }
    }
}

impl Error for QuestMapWalkRouteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Route(source) => Some(source),
            _ => None,
        }
    }
}

impl From<QuestMapRouteError> for QuestMapWalkRouteError {
    fn from(value: QuestMapRouteError) -> Self {
        Self::Route(value)
    }
}

fn actor_position_pixel(report: &QuestMapCollisionStateReport) -> [i32; 2] {
    [
        report.actor_geometry.position_fixed[0] / FIXED_POINT_SCALE,
        report.actor_geometry.position_fixed[1] / FIXED_POINT_SCALE,
    ]
}

fn verify_walk_start(
    report: &QuestMapCollisionStateReport,
) -> Result<[i32; 2], QuestMapWalkRouteError> {
    if report.classifier_context.actor_state != QUEST_PLAYER_NORMAL_MOVEMENT_STATE {
        return Err(QuestMapWalkRouteError::UnsupportedActorState {
            expected: QUEST_PLAYER_NORMAL_MOVEMENT_STATE,
            actual: report.classifier_context.actor_state,
        });
    }
    let subpixel = [
        report.actor_geometry.position_fixed[0] % FIXED_POINT_SCALE,
        report.actor_geometry.position_fixed[1] % FIXED_POINT_SCALE,
    ];
    if subpixel != [0, 0] {
        return Err(QuestMapWalkRouteError::StartNotPixelAligned(subpixel));
    }
    Ok(actor_position_pixel(report))
}

fn verify_goal_lattice(start: [i32; 2], goal: [i32; 2]) -> Result<(), QuestMapWalkRouteError> {
    if (goal[0] - start[0]).rem_euclid(QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS) != 0
        || (goal[1] - start[1]).rem_euclid(QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS) != 0
    {
        return Err(QuestMapWalkRouteError::GoalOutsideMovementLattice {
            start,
            goal,
            step_pixels: QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS,
        });
    }
    Ok(())
}

/// 정확히 한 목표까지 정상 보행 delta만 사용하는 경로를 찾는다.
pub fn find_direction_robust_quest_map_walk_route_avoiding_rectangles(
    report: &QuestMapCollisionStateReport,
    goal_pixel: [i32; 2],
    object_obstacles: &[QuestCollisionRectangle],
) -> Result<QuestMapWalkRoute, QuestMapWalkRouteError> {
    find_direction_robust_quest_map_walk_route_to_any_avoiding_rectangles(
        report,
        &[goal_pixel],
        object_obstacles,
    )
}

/// 여러 목표 후보 중 정상 보행 delta로 가장 먼저 닿는 경로를 찾는다.
pub fn find_direction_robust_quest_map_walk_route_to_any_avoiding_rectangles(
    report: &QuestMapCollisionStateReport,
    goal_pixels: &[[i32; 2]],
    object_obstacles: &[QuestCollisionRectangle],
) -> Result<QuestMapWalkRoute, QuestMapWalkRouteError> {
    let start = verify_walk_start(report)?;
    for &goal in goal_pixels {
        verify_goal_lattice(start, goal)?;
    }
    let route = find_direction_robust_quest_map_route_to_any_avoiding_rectangles(
        report,
        goal_pixels,
        object_obstacles,
        QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS,
    )?;
    let movement_steps = route.pixel_steps
        / usize::try_from(QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS)
            .expect("source-bound 양수 보행 step");
    Ok(QuestMapWalkRoute {
        route,
        normal_movement_state: QUEST_PLAYER_NORMAL_MOVEMENT_STATE,
        input_runtime: QUEST_PLAYER_NORMAL_MOVEMENT_INPUT_RUNTIME,
        direction_input_masks: QUEST_PLAYER_NORMAL_MOVEMENT_INPUT_MASKS,
        direction_deltas_fixed: QUEST_PLAYER_NORMAL_MOVEMENT_DELTAS_FIXED,
        position_step_pixels: QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS,
        movement_steps,
    })
}

#[cfg(test)]
#[path = "quest_map_walk_route_tests.rs"]
mod tests;
