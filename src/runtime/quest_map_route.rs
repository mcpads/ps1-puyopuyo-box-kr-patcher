//! exact QUEST state의 live collision plane에서 보수적 이동 경로 후보를 찾는다.
//!
//! 각 pixel 위치의 zero-delta actor 사각형을 검증하고, classifier의 방향 0과
//! 비0 분기 중 어느 하나에서라도 겹친 tile이 class 1 또는 2이면 그 위치를
//! 막힌 것으로 본다. 결과는 정지 위치를 잇는 기하 후보일 뿐이다. frame별 delta,
//! movement adjust, object collision, 자연 실행과 사람 승인을 주장하지 않는다.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

use crate::disasm::quest_map_collision::{
    classify_quest_collision_tile, quest_collision_actor_rectangle,
    quest_collision_rectangle_overlap, quest_collision_tile_probes, QuestCollisionActorGeometry,
    QuestCollisionClass, QuestCollisionGeometryError, QUEST_COLLISION_FIXED_POINT_SHIFT,
    QUEST_COLLISION_TILE_SIZE_PIXELS,
};

use super::QuestMapCollisionStateReport;

const FIXED_POINT_SCALE: i32 = 1 << QUEST_COLLISION_FIXED_POINT_SHIFT;
const TILE_SIZE_PIXELS: i32 = QUEST_COLLISION_TILE_SIZE_PIXELS as i32;
const UNVISITED: u8 = u8::MAX;
const START: u8 = u8::MAX - 1;

/// 방향과 이동 축의 대응 없이 classifier의 0·비0 분기를 대표하는 값.
pub const QUEST_MAP_ROUTE_DIRECTION_BRANCH_REPRESENTATIVES: [u8; 2] = [0, 1];

const NEIGHBORS: [(i32, i32); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];

/// live collision plane에서 찾은 pixel 단위 정지 위치 경로 후보.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestMapRoute {
    /// exact state actor 위치를 source와 같은 fixed-point 절삭으로 읽은 pixel.
    pub start_pixel: [i32; 2],
    /// 절삭 뒤 남은 fixed-point 값. 0이 아니면 원래 위치는 pixel 경계가 아니다.
    pub start_subpixel_fixed: [i32; 2],
    /// 호출자가 명시한 exact 목표 pixel.
    pub goal_pixel: [i32; 2],
    /// 4-neighbor 경로의 1px 이동 횟수.
    pub pixel_steps: usize,
    /// 시작, 방향 전환점, 끝만 남긴 axis-aligned 경로.
    pub waypoints: Vec<[i32; 2]>,
    /// 탐색 중 collision을 실제 평가한 서로 다른 pixel 수.
    pub evaluated_positions: usize,
    /// 평가한 위치 중 방향 0·비0 분기 모두 class 0이었던 pixel 수.
    pub clear_positions: usize,
    /// 각 위치에서 평가한 actor 방향 분기 대표값.
    pub direction_branch_representatives: [u8; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestMapRouteError {
    MapCollisionDisabled,
    InvalidLivePlaneLength {
        expected: usize,
        actual: usize,
    },
    PositionOutsideSearchBounds {
        role: &'static str,
        position: [i32; 2],
        min: [i32; 2],
        max_exclusive: [i32; 2],
    },
    StartBlocked([i32; 2]),
    GoalBlocked([i32; 2]),
    GoalUnreachable {
        start: [i32; 2],
        goal: [i32; 2],
        evaluated_positions: usize,
        clear_positions: usize,
    },
    EmptyGoalCandidates,
    GoalCandidatesBlocked {
        candidate_count: usize,
    },
    GoalCandidatesUnreachable {
        start: [i32; 2],
        candidate_count: usize,
        evaluated_positions: usize,
        clear_positions: usize,
    },
    InvalidPositionStep(i32),
    CoordinateOverflow,
    Geometry(QuestCollisionGeometryError),
}

impl fmt::Display for QuestMapRouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MapCollisionDisabled => formatter.write_str(
                "QUEST actor의 map collision membership가 꺼져 있어 route 모델을 적용할 수 없음",
            ),
            Self::InvalidLivePlaneLength { expected, actual } => write!(
                formatter,
                "QUEST live collision plane 길이 불일치: expected={expected}, actual={actual}",
            ),
            Self::PositionOutsideSearchBounds {
                role,
                position,
                min,
                max_exclusive,
            } => write!(
                formatter,
                "QUEST {role} pixel이 탐색 경계 밖임: ({},{}), bounds=({},{})..({},{})",
                position[0],
                position[1],
                min[0],
                min[1],
                max_exclusive[0],
                max_exclusive[1],
            ),
            Self::StartBlocked(position) => write!(
                formatter,
                "QUEST 시작 pixel의 정지 사각형이 collision과 겹침: ({},{})",
                position[0], position[1],
            ),
            Self::GoalBlocked(position) => write!(
                formatter,
                "QUEST 목표 pixel의 정지 사각형이 collision과 겹침: ({},{})",
                position[0], position[1],
            ),
            Self::GoalUnreachable {
                start,
                goal,
                evaluated_positions,
                clear_positions,
            } => write!(
                formatter,
                "QUEST live plane에서 목표 pixel에 닿는 보수적 경로가 없음: start=({},{}), goal=({},{}), evaluated={evaluated_positions}, clear={clear_positions}",
                start[0], start[1], goal[0], goal[1],
            ),
            Self::EmptyGoalCandidates => {
                formatter.write_str("QUEST route 목표 후보가 하나도 없음")
            }
            Self::GoalCandidatesBlocked { candidate_count } => write!(
                formatter,
                "QUEST route 목표 후보가 모두 collision과 겹침: candidates={candidate_count}",
            ),
            Self::GoalCandidatesUnreachable {
                start,
                candidate_count,
                evaluated_positions,
                clear_positions,
            } => write!(
                formatter,
                "QUEST live plane에서 목표 후보에 닿는 보수적 경로가 없음: start=({},{}), candidates={candidate_count}, evaluated={evaluated_positions}, clear={clear_positions}",
                start[0], start[1],
            ),
            Self::InvalidPositionStep(step) => {
                write!(formatter, "QUEST route 위치 전이 폭이 양수가 아님: {step}")
            }
            Self::CoordinateOverflow => formatter.write_str("QUEST route 좌표 또는 탐색 크기 overflow"),
            Self::Geometry(source) => write!(formatter, "QUEST route geometry 실패: {source}"),
        }
    }
}

impl Error for QuestMapRouteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Geometry(source) => Some(source),
            _ => None,
        }
    }
}

impl From<QuestCollisionGeometryError> for QuestMapRouteError {
    fn from(value: QuestCollisionGeometryError) -> Self {
        Self::Geometry(value)
    }
}

#[derive(Debug, Clone, Copy)]
struct SearchGrid {
    min: [i32; 2],
    max_exclusive: [i32; 2],
    width: usize,
    height: usize,
}

impl SearchGrid {
    fn from_report(report: &QuestMapCollisionStateReport) -> Result<Self, QuestMapRouteError> {
        let min = [
            report
                .tile_bounds
                .min_x
                .checked_mul(TILE_SIZE_PIXELS)
                .ok_or(QuestMapRouteError::CoordinateOverflow)?,
            report
                .tile_bounds
                .min_y
                .checked_mul(TILE_SIZE_PIXELS)
                .ok_or(QuestMapRouteError::CoordinateOverflow)?,
        ];
        let max_exclusive = [
            report
                .tile_bounds
                .max_x_exclusive
                .checked_mul(TILE_SIZE_PIXELS)
                .ok_or(QuestMapRouteError::CoordinateOverflow)?,
            report
                .tile_bounds
                .max_y_exclusive
                .checked_mul(TILE_SIZE_PIXELS)
                .ok_or(QuestMapRouteError::CoordinateOverflow)?,
        ];
        let width = max_exclusive[0]
            .checked_sub(min[0])
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(QuestMapRouteError::CoordinateOverflow)?;
        let height = max_exclusive[1]
            .checked_sub(min[1])
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(QuestMapRouteError::CoordinateOverflow)?;
        width
            .checked_mul(height)
            .ok_or(QuestMapRouteError::CoordinateOverflow)?;
        Ok(Self {
            min,
            max_exclusive,
            width,
            height,
        })
    }

    fn len(self) -> Result<usize, QuestMapRouteError> {
        self.width
            .checked_mul(self.height)
            .ok_or(QuestMapRouteError::CoordinateOverflow)
    }

    fn contains(self, position: [i32; 2]) -> bool {
        position[0] >= self.min[0]
            && position[0] < self.max_exclusive[0]
            && position[1] >= self.min[1]
            && position[1] < self.max_exclusive[1]
    }

    fn index(self, position: [i32; 2]) -> Result<usize, QuestMapRouteError> {
        let x = position[0]
            .checked_sub(self.min[0])
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(QuestMapRouteError::CoordinateOverflow)?;
        let y = position[1]
            .checked_sub(self.min[1])
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(QuestMapRouteError::CoordinateOverflow)?;
        y.checked_mul(self.width)
            .and_then(|row| row.checked_add(x))
            .ok_or(QuestMapRouteError::CoordinateOverflow)
    }

    fn position(self, index: usize) -> Result<[i32; 2], QuestMapRouteError> {
        let x = i32::try_from(index % self.width)
            .map_err(|_| QuestMapRouteError::CoordinateOverflow)?;
        let y = i32::try_from(index / self.width)
            .map_err(|_| QuestMapRouteError::CoordinateOverflow)?;
        Ok([
            self.min[0]
                .checked_add(x)
                .ok_or(QuestMapRouteError::CoordinateOverflow)?,
            self.min[1]
                .checked_add(y)
                .ok_or(QuestMapRouteError::CoordinateOverflow)?,
        ])
    }
}

fn expected_live_cells(report: &QuestMapCollisionStateReport) -> Result<usize, QuestMapRouteError> {
    usize::try_from(report.width)
        .ok()
        .and_then(|width| {
            usize::try_from(report.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or(QuestMapRouteError::CoordinateOverflow)
}

fn actor_position_pixel(report: &QuestMapCollisionStateReport) -> [i32; 2] {
    [
        report.actor_geometry.position_fixed[0] / FIXED_POINT_SCALE,
        report.actor_geometry.position_fixed[1] / FIXED_POINT_SCALE,
    ]
}

fn position_is_clear(
    report: &QuestMapCollisionStateReport,
    position: [i32; 2],
    object_obstacles: &[crate::disasm::quest_map_collision::QuestCollisionRectangle],
) -> Result<bool, QuestMapRouteError> {
    let actor = QuestCollisionActorGeometry {
        position_fixed: [
            position[0]
                .checked_mul(FIXED_POINT_SCALE)
                .ok_or(QuestMapRouteError::CoordinateOverflow)?,
            position[1]
                .checked_mul(FIXED_POINT_SCALE)
                .ok_or(QuestMapRouteError::CoordinateOverflow)?,
        ],
        local_rectangle: report.actor_geometry.local_rectangle,
    };
    let actor_rectangle = quest_collision_actor_rectangle(actor, [0, 0])?;
    for &obstacle in object_obstacles {
        if quest_collision_rectangle_overlap(actor_rectangle, obstacle)?.is_some() {
            return Ok(false);
        }
    }
    let width =
        usize::try_from(report.width).map_err(|_| QuestMapRouteError::CoordinateOverflow)?;
    for probe in quest_collision_tile_probes(actor_rectangle, report.tile_bounds)? {
        if quest_collision_rectangle_overlap(actor_rectangle, probe.rectangle)?.is_none() {
            continue;
        }
        let tile = if probe.in_bounds {
            let x = usize::try_from(probe.x).map_err(|_| QuestMapRouteError::CoordinateOverflow)?;
            let y = usize::try_from(probe.y).map_err(|_| QuestMapRouteError::CoordinateOverflow)?;
            let index = y
                .checked_mul(width)
                .and_then(|row| row.checked_add(x))
                .ok_or(QuestMapRouteError::CoordinateOverflow)?;
            *report
                .live_cells
                .get(index)
                .ok_or(QuestMapRouteError::CoordinateOverflow)?
        } else {
            1
        };
        for actor_direction in QUEST_MAP_ROUTE_DIRECTION_BRANCH_REPRESENTATIVES {
            let mut context = report.classifier_context;
            context.actor_direction = actor_direction;
            if classify_quest_collision_tile(tile, context) != QuestCollisionClass::NoCollision {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn compress_waypoints(path: &[[i32; 2]]) -> Vec<[i32; 2]> {
    if path.len() <= 2 {
        return path.to_vec();
    }
    let mut waypoints = Vec::with_capacity(path.len());
    waypoints.push(path[0]);
    let mut previous_delta = [path[1][0] - path[0][0], path[1][1] - path[0][1]];
    for index in 2..path.len() {
        let delta = [
            path[index][0] - path[index - 1][0],
            path[index][1] - path[index - 1][1],
        ];
        if delta != previous_delta {
            waypoints.push(path[index - 1]);
            previous_delta = delta;
        }
    }
    waypoints.push(*path.last().expect("길이 3 이상인 path"));
    waypoints
}

/// exact state actor pixel에서 호출자가 지정한 목표 pixel까지 보수적 경로를 찾는다.
pub fn find_direction_robust_quest_map_route(
    report: &QuestMapCollisionStateReport,
    goal_pixel: [i32; 2],
) -> Result<QuestMapRoute, QuestMapRouteError> {
    find_direction_robust_quest_map_route_avoiding_rectangles(report, goal_pixel, &[])
}

/// map collision과 호출자가 명시한 world 객체 사각형을 모두 피하는 경로를 찾는다.
///
/// 객체 목록은 exact state에서 확인한 범위만 넘겨야 한다. 목록에 없는 동적 객체,
/// frame delta와 sliding은 여전히 모델 밖이다.
pub fn find_direction_robust_quest_map_route_avoiding_rectangles(
    report: &QuestMapCollisionStateReport,
    goal_pixel: [i32; 2],
    object_obstacles: &[crate::disasm::quest_map_collision::QuestCollisionRectangle],
) -> Result<QuestMapRoute, QuestMapRouteError> {
    find_direction_robust_quest_map_route_to_any_avoiding_rectangles(
        report,
        &[goal_pixel],
        object_obstacles,
        1,
    )
}

pub(super) fn find_direction_robust_quest_map_route_to_any_avoiding_rectangles(
    report: &QuestMapCollisionStateReport,
    goal_pixels: &[[i32; 2]],
    object_obstacles: &[crate::disasm::quest_map_collision::QuestCollisionRectangle],
    position_step_pixels: i32,
) -> Result<QuestMapRoute, QuestMapRouteError> {
    if !report.actor_map_collision_enabled {
        return Err(QuestMapRouteError::MapCollisionDisabled);
    }
    if goal_pixels.is_empty() {
        return Err(QuestMapRouteError::EmptyGoalCandidates);
    }
    if position_step_pixels <= 0 {
        return Err(QuestMapRouteError::InvalidPositionStep(
            position_step_pixels,
        ));
    }
    let expected = expected_live_cells(report)?;
    if report.live_cells.len() != expected {
        return Err(QuestMapRouteError::InvalidLivePlaneLength {
            expected,
            actual: report.live_cells.len(),
        });
    }

    let grid = SearchGrid::from_report(report)?;
    let start_pixel = actor_position_pixel(report);
    if !grid.contains(start_pixel) {
        return Err(QuestMapRouteError::PositionOutsideSearchBounds {
            role: "시작",
            position: start_pixel,
            min: grid.min,
            max_exclusive: grid.max_exclusive,
        });
    }
    for &position in goal_pixels {
        if !grid.contains(position) {
            return Err(QuestMapRouteError::PositionOutsideSearchBounds {
                role: "목표 후보",
                position,
                min: grid.min,
                max_exclusive: grid.max_exclusive,
            });
        }
    }

    let cell_count = grid.len()?;
    let mut safety = vec![0u8; cell_count];
    let mut evaluated_positions = 0usize;
    let mut clear_positions = 0usize;
    let mut evaluate = |position: [i32; 2]| -> Result<bool, QuestMapRouteError> {
        let index = grid.index(position)?;
        match safety[index] {
            1 => Ok(false),
            2 => Ok(true),
            _ => {
                let clear = position_is_clear(report, position, object_obstacles)?;
                safety[index] = if clear { 2 } else { 1 };
                evaluated_positions += 1;
                clear_positions += usize::from(clear);
                Ok(clear)
            }
        }
    };
    if !evaluate(start_pixel)? {
        return Err(QuestMapRouteError::StartBlocked(start_pixel));
    }

    let start_index = grid.index(start_pixel)?;
    let mut goal_indices = vec![false; cell_count];
    let mut clear_goal_count = 0usize;
    for &goal_pixel in goal_pixels {
        let goal_index = grid.index(goal_pixel)?;
        if !goal_indices[goal_index] && evaluate(goal_pixel)? {
            goal_indices[goal_index] = true;
            clear_goal_count += 1;
        }
    }
    if clear_goal_count == 0 {
        if let [goal_pixel] = goal_pixels {
            return Err(QuestMapRouteError::GoalBlocked(*goal_pixel));
        }
        return Err(QuestMapRouteError::GoalCandidatesBlocked {
            candidate_count: goal_pixels.len(),
        });
    }

    let mut parent_direction = vec![UNVISITED; cell_count];
    parent_direction[start_index] = START;
    let mut queue = VecDeque::from([start_index]);
    let mut reached_goal_index = None;
    while let Some(index) = queue.pop_front() {
        if goal_indices[index] {
            reached_goal_index = Some(index);
            break;
        }
        let position = grid.position(index)?;
        for (direction, &(dx, dy)) in NEIGHBORS.iter().enumerate() {
            let next = [
                position[0]
                    .checked_add(
                        dx.checked_mul(position_step_pixels)
                            .ok_or(QuestMapRouteError::CoordinateOverflow)?,
                    )
                    .ok_or(QuestMapRouteError::CoordinateOverflow)?,
                position[1]
                    .checked_add(
                        dy.checked_mul(position_step_pixels)
                            .ok_or(QuestMapRouteError::CoordinateOverflow)?,
                    )
                    .ok_or(QuestMapRouteError::CoordinateOverflow)?,
            ];
            if !grid.contains(next) {
                continue;
            }
            let next_index = grid.index(next)?;
            if parent_direction[next_index] != UNVISITED || !evaluate(next)? {
                continue;
            }
            parent_direction[next_index] =
                u8::try_from(direction).expect("NEIGHBORS index는 u8 범위");
            queue.push_back(next_index);
        }
    }

    let Some(goal_index) = reached_goal_index else {
        if let [goal_pixel] = goal_pixels {
            return Err(QuestMapRouteError::GoalUnreachable {
                start: start_pixel,
                goal: *goal_pixel,
                evaluated_positions,
                clear_positions,
            });
        }
        return Err(QuestMapRouteError::GoalCandidatesUnreachable {
            start: start_pixel,
            candidate_count: goal_pixels.len(),
            evaluated_positions,
            clear_positions,
        });
    };
    let goal_pixel = grid.position(goal_index)?;

    let mut reverse_path = vec![goal_pixel];
    let mut cursor = goal_pixel;
    while cursor != start_pixel {
        let direction = usize::from(parent_direction[grid.index(cursor)?]);
        let (dx, dy) = NEIGHBORS[direction];
        cursor = [
            cursor[0]
                .checked_sub(
                    dx.checked_mul(position_step_pixels)
                        .ok_or(QuestMapRouteError::CoordinateOverflow)?,
                )
                .ok_or(QuestMapRouteError::CoordinateOverflow)?,
            cursor[1]
                .checked_sub(
                    dy.checked_mul(position_step_pixels)
                        .ok_or(QuestMapRouteError::CoordinateOverflow)?,
                )
                .ok_or(QuestMapRouteError::CoordinateOverflow)?,
        ];
        reverse_path.push(cursor);
    }
    reverse_path.reverse();
    let pixel_steps = (reverse_path.len() - 1)
        .checked_mul(
            usize::try_from(position_step_pixels)
                .map_err(|_| QuestMapRouteError::CoordinateOverflow)?,
        )
        .ok_or(QuestMapRouteError::CoordinateOverflow)?;
    let waypoints = compress_waypoints(&reverse_path);
    Ok(QuestMapRoute {
        start_pixel,
        start_subpixel_fixed: [
            report.actor_geometry.position_fixed[0] % FIXED_POINT_SCALE,
            report.actor_geometry.position_fixed[1] % FIXED_POINT_SCALE,
        ],
        goal_pixel,
        pixel_steps,
        waypoints,
        evaluated_positions,
        clear_positions,
        direction_branch_representatives: QUEST_MAP_ROUTE_DIRECTION_BRANCH_REPRESENTATIVES,
    })
}

#[cfg(test)]
#[path = "quest_map_route_tests.rs"]
mod tests;
