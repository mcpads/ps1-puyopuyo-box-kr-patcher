//! QUEST exact save-state의 활성 collision plane과 원본 QMP를 교차한다.
//!
//! runtime buffer는 문·파괴물 등에 의해 원본 cell이 0으로 바뀔 수 있다.
//! 따라서 원본과 완전 동일하다고 가정하지 않고, 같은 크기이면서 차이가
//! `원본 nonzero -> runtime zero`뿐인 QMP 후보를 모두 보존한다. 후보가 하나인
//! 경우에도 이 state만으로 실행 BIN이나 route의 사람 승인을 대신하지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::disasm::quest_map_collision::{
    classify_quest_collision_tile, quest_collision_actor_rectangle,
    quest_collision_rectangle_overlap, quest_collision_tile_probes, QuestCollisionActorGeometry,
    QuestCollisionClass, QuestCollisionClassifierContext, QuestCollisionGeometryError,
    QuestCollisionOverlap, QuestCollisionRectangle, QuestCollisionTileBounds,
    QUEST_COLLISION_ACTOR_DIRECTION_OFFSET, QUEST_COLLISION_ACTOR_FIXED_X_OFFSET,
    QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET, QUEST_COLLISION_ACTOR_LAYER_OFFSET,
    QUEST_COLLISION_ACTOR_MAP_MEMBER_MASK, QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET,
    QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS, QUEST_COLLISION_ACTOR_STATE_OFFSET,
    QUEST_COLLISION_CONDITION_RUNTIMES, QUEST_COLLISION_INPUT_STATE_RUNTIME,
    QUEST_COLLISION_MAP_BOUNDS_RUNTIMES, QUEST_COLLISION_PRIMARY_ACTOR_RUNTIME,
    QUEST_MAP_ACTIVE_SLOT_RUNTIME, QUEST_MAP_COLLISION_BUFFER_RUNTIME,
    QUEST_MAP_PLANE_CAPACITY_CELLS, QUEST_MAP_SLOT_COUNT, QUEST_MAP_SLOT_HEIGHTS_RUNTIME,
    QUEST_MAP_SLOT_WIDTHS_RUNTIME,
};
use crate::qmp::QmpMap;

use super::{MednafenPsxState, MednafenStateError};

/// 원본 `RPG_MAP.VDD`의 resource ID와 QMP 하나.
#[derive(Debug, Clone, Copy)]
pub struct QuestMapCollisionSource<'a> {
    pub resource_id: u16,
    pub name: &'a str,
    pub map: &'a QmpMap,
}

/// 원본 QMP에서 runtime에 의해 0으로 바뀐 cell 후보.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestMapCollisionClearedCell {
    pub index: usize,
    pub x: u32,
    pub y: u32,
    pub source_tile: u16,
    pub runtime_tile: u16,
}

/// 활성 plane과 0-clearing 규칙으로 양립하는 원본 QMP 후보.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestMapCollisionSourceMatch {
    pub resource_id: u16,
    pub name: String,
    pub zero_cleared_cells: Vec<QuestMapCollisionClearedCell>,
}

/// zero-delta actor 사각형이 원본 이동 함수에서 확인할 tile 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestMapCollisionStationaryProbe {
    pub tile_x: i32,
    pub tile_y: i32,
    pub in_bounds: bool,
    pub tile: u16,
    pub class: QuestCollisionClass,
    pub overlap: Option<QuestCollisionOverlap>,
}

/// 한 exact state에서 읽은 활성 collision plane과 player 문맥.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestMapCollisionStateReport {
    pub active_slot: u8,
    pub width: u32,
    pub height: u32,
    pub primary_actor_runtime: u32,
    pub actor_member_flags: u8,
    pub actor_map_collision_enabled: bool,
    pub actor_geometry: QuestCollisionActorGeometry,
    pub stationary_actor_rectangle: QuestCollisionRectangle,
    pub tile_bounds: QuestCollisionTileBounds,
    pub stationary_probes: Vec<QuestMapCollisionStationaryProbe>,
    pub classifier_context: QuestCollisionClassifierContext,
    pub live_cells: Vec<u16>,
    pub live_tail_nonzero_cells: usize,
    pub class_counts: [usize; 3],
    pub dimension_candidate_count: usize,
    pub source_matches: Vec<QuestMapCollisionSourceMatch>,
}

impl QuestMapCollisionStateReport {
    /// 0-clearing 규칙으로 양립하는 후보가 정확히 하나일 때만 반환한다.
    pub fn unique_source_match(&self) -> Option<&QuestMapCollisionSourceMatch> {
        let [source_match] = self.source_matches.as_slice() else {
            return None;
        };
        Some(source_match)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestMapCollisionStateError {
    EmptySources,
    InvalidSourceResourceId(u16),
    DuplicateSourceResourceId(u16),
    DuplicateSourceName(String),
    InvalidActiveSlot(u8),
    EmptyDimensions {
        width: u32,
        height: u32,
    },
    MapCapacityExceeded {
        cells: usize,
        capacity: usize,
    },
    MapBoundsOutsideActiveDimensions {
        bounds: QuestCollisionTileBounds,
        width: u32,
        height: u32,
    },
    NullPrimaryActor,
    CoordinateOverflow,
    Geometry(QuestCollisionGeometryError),
    State(MednafenStateError),
}

impl fmt::Display for QuestMapCollisionStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySources => f.write_str("QUEST collision 원본 QMP 입력이 비어 있음"),
            Self::InvalidSourceResourceId(resource_id) => {
                write!(f, "QUEST collision QMP resource ID가 0임: {resource_id}")
            }
            Self::DuplicateSourceResourceId(resource_id) => {
                write!(f, "QUEST collision QMP resource ID 중복: {resource_id}")
            }
            Self::DuplicateSourceName(name) => {
                write!(f, "QUEST collision QMP 이름 중복: {name}")
            }
            Self::InvalidActiveSlot(slot) => write!(
                f,
                "QUEST collision active slot 범위 이탈: slot={slot}, slots={QUEST_MAP_SLOT_COUNT}"
            ),
            Self::EmptyDimensions { width, height } => {
                write!(
                    f,
                    "QUEST collision active map 크기가 비어 있음: {width}x{height}"
                )
            }
            Self::MapCapacityExceeded { cells, capacity } => write!(
                f,
                "QUEST collision active map 용량 초과: cells={cells}, capacity={capacity}"
            ),
            Self::MapBoundsOutsideActiveDimensions {
                bounds,
                width,
                height,
            } => write!(
                f,
                "QUEST collision tile 경계가 active map 밖임: bounds=({},{})..({},{}), map={}x{}",
                bounds.min_x,
                bounds.min_y,
                bounds.max_x_exclusive,
                bounds.max_y_exclusive,
                width,
                height,
            ),
            Self::NullPrimaryActor => f.write_str("QUEST collision primary actor pointer가 0임"),
            Self::CoordinateOverflow => f.write_str("QUEST collision runtime 좌표 overflow"),
            Self::Geometry(source) => write!(f, "QUEST collision geometry 실패: {source}"),
            Self::State(source) => write!(f, "QUEST collision state 읽기 실패: {source}"),
        }
    }
}

impl Error for QuestMapCollisionStateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Geometry(source) => Some(source),
            Self::State(source) => Some(source),
            _ => None,
        }
    }
}

impl From<MednafenStateError> for QuestMapCollisionStateError {
    fn from(value: MednafenStateError) -> Self {
        Self::State(value)
    }
}

impl From<QuestCollisionGeometryError> for QuestMapCollisionStateError {
    fn from(value: QuestCollisionGeometryError) -> Self {
        Self::Geometry(value)
    }
}

fn runtime_field(base: u32, offset: u32) -> Result<u32, QuestMapCollisionStateError> {
    base.checked_add(offset)
        .ok_or(QuestMapCollisionStateError::CoordinateOverflow)
}

fn positive_offset(offset: i16) -> Result<u32, QuestMapCollisionStateError> {
    u32::try_from(offset).map_err(|_| QuestMapCollisionStateError::CoordinateOverflow)
}

fn verify_sources(
    sources: &[QuestMapCollisionSource<'_>],
) -> Result<(), QuestMapCollisionStateError> {
    if sources.is_empty() {
        return Err(QuestMapCollisionStateError::EmptySources);
    }
    let mut resource_ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for source in sources {
        if source.resource_id == 0 {
            return Err(QuestMapCollisionStateError::InvalidSourceResourceId(
                source.resource_id,
            ));
        }
        if !resource_ids.insert(source.resource_id) {
            return Err(QuestMapCollisionStateError::DuplicateSourceResourceId(
                source.resource_id,
            ));
        }
        let normalized = source.name.to_ascii_uppercase();
        if !names.insert(normalized) {
            return Err(QuestMapCollisionStateError::DuplicateSourceName(
                source.name.to_string(),
            ));
        }
    }
    Ok(())
}

fn match_sources(
    width: u32,
    height: u32,
    live_cells: &[u16],
    sources: &[QuestMapCollisionSource<'_>],
) -> (usize, Vec<QuestMapCollisionSourceMatch>) {
    let mut dimension_candidate_count = 0usize;
    let mut source_matches = Vec::new();
    for source in sources {
        if source.map.width() != width || source.map.height() != height {
            continue;
        }
        dimension_candidate_count += 1;
        let mut zero_cleared_cells = Vec::new();
        let compatible = source.map.cells().iter().zip(live_cells).enumerate().all(
            |(index, (&source_tile, &runtime_tile))| {
                if source_tile == runtime_tile {
                    return true;
                }
                if source_tile != 0 && runtime_tile == 0 {
                    zero_cleared_cells.push(QuestMapCollisionClearedCell {
                        index,
                        x: u32::try_from(index).expect("QMP cell 수는 u32 범위") % width,
                        y: u32::try_from(index).expect("QMP cell 수는 u32 범위") / width,
                        source_tile,
                        runtime_tile,
                    });
                    return true;
                }
                false
            },
        );
        if compatible {
            source_matches.push(QuestMapCollisionSourceMatch {
                resource_id: source.resource_id,
                name: source.name.to_string(),
                zero_cleared_cells,
            });
        }
    }
    (dimension_candidate_count, source_matches)
}

/// exact Mednafen state의 활성 collision plane을 원본 QMP 전수와 교차한다.
pub fn inspect_quest_map_collision_state(
    state: &MednafenPsxState<'_>,
    sources: &[QuestMapCollisionSource<'_>],
) -> Result<QuestMapCollisionStateReport, QuestMapCollisionStateError> {
    verify_sources(sources)?;

    let active_slot = state.read_u8(QUEST_MAP_ACTIVE_SLOT_RUNTIME)?;
    if usize::from(active_slot) >= QUEST_MAP_SLOT_COUNT {
        return Err(QuestMapCollisionStateError::InvalidActiveSlot(active_slot));
    }
    let slot_offset = u32::from(active_slot)
        .checked_mul(4)
        .ok_or(QuestMapCollisionStateError::CoordinateOverflow)?;
    let width = state.read_u32_le(runtime_field(QUEST_MAP_SLOT_WIDTHS_RUNTIME, slot_offset)?)?;
    let height = state.read_u32_le(runtime_field(QUEST_MAP_SLOT_HEIGHTS_RUNTIME, slot_offset)?)?;
    if width == 0 || height == 0 {
        return Err(QuestMapCollisionStateError::EmptyDimensions { width, height });
    }
    let cell_count = width
        .checked_mul(height)
        .and_then(|cells| usize::try_from(cells).ok())
        .ok_or(QuestMapCollisionStateError::CoordinateOverflow)?;
    if cell_count > QUEST_MAP_PLANE_CAPACITY_CELLS {
        return Err(QuestMapCollisionStateError::MapCapacityExceeded {
            cells: cell_count,
            capacity: QUEST_MAP_PLANE_CAPACITY_CELLS,
        });
    }

    let raw_plane = state.read_bytes(
        QUEST_MAP_COLLISION_BUFFER_RUNTIME,
        QUEST_MAP_PLANE_CAPACITY_CELLS * 2,
    )?;
    let plane = raw_plane
        .chunks_exact(2)
        .map(|raw| u16::from_le_bytes([raw[0], raw[1]]))
        .collect::<Vec<_>>();
    let live_cells = plane[..cell_count].to_vec();
    let live_tail_nonzero_cells = plane[cell_count..]
        .iter()
        .filter(|tile| **tile != 0)
        .count();

    let primary_actor_runtime = state.read_u32_le(QUEST_COLLISION_PRIMARY_ACTOR_RUNTIME)?;
    if primary_actor_runtime == 0 {
        return Err(QuestMapCollisionStateError::NullPrimaryActor);
    }
    let actor_member_flags = state.read_u8(runtime_field(
        primary_actor_runtime,
        positive_offset(QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET)?,
    )?)?;
    let actor_geometry = QuestCollisionActorGeometry {
        position_fixed: [
            state.read_u32_le(runtime_field(
                primary_actor_runtime,
                positive_offset(QUEST_COLLISION_ACTOR_FIXED_X_OFFSET)?,
            )?)? as i32,
            state.read_u32_le(runtime_field(
                primary_actor_runtime,
                positive_offset(QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET)?,
            )?)? as i32,
        ],
        local_rectangle: [
            state.read_u16_le(runtime_field(
                primary_actor_runtime,
                positive_offset(QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[0])?,
            )?)? as i16,
            state.read_u16_le(runtime_field(
                primary_actor_runtime,
                positive_offset(QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[1])?,
            )?)? as i16,
            state.read_u16_le(runtime_field(
                primary_actor_runtime,
                positive_offset(QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[2])?,
            )?)? as i16,
            state.read_u16_le(runtime_field(
                primary_actor_runtime,
                positive_offset(QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[3])?,
            )?)? as i16,
        ],
    };
    let stationary_actor_rectangle = quest_collision_actor_rectangle(actor_geometry, [0, 0])?;
    let tile_bounds = QuestCollisionTileBounds {
        min_x: i32::from(state.read_u16_le(QUEST_COLLISION_MAP_BOUNDS_RUNTIMES[0])? as i16),
        min_y: i32::from(state.read_u16_le(QUEST_COLLISION_MAP_BOUNDS_RUNTIMES[1])? as i16),
        max_x_exclusive: i32::from(
            state.read_u16_le(QUEST_COLLISION_MAP_BOUNDS_RUNTIMES[2])? as i16
        ),
        max_y_exclusive: i32::from(
            state.read_u16_le(QUEST_COLLISION_MAP_BOUNDS_RUNTIMES[3])? as i16
        ),
    };
    let width_i32 =
        i32::try_from(width).map_err(|_| QuestMapCollisionStateError::CoordinateOverflow)?;
    let height_i32 =
        i32::try_from(height).map_err(|_| QuestMapCollisionStateError::CoordinateOverflow)?;
    if tile_bounds.min_x < 0
        || tile_bounds.min_y < 0
        || tile_bounds.max_x_exclusive > width_i32
        || tile_bounds.max_y_exclusive > height_i32
    {
        return Err(
            QuestMapCollisionStateError::MapBoundsOutsideActiveDimensions {
                bounds: tile_bounds,
                width,
                height,
            },
        );
    }

    let classifier_context = QuestCollisionClassifierContext {
        actor_is_primary: true,
        actor_layer: state.read_u32_le(runtime_field(
            primary_actor_runtime,
            positive_offset(QUEST_COLLISION_ACTOR_LAYER_OFFSET)?,
        )?)?,
        actor_direction: state.read_u8(runtime_field(
            primary_actor_runtime,
            positive_offset(QUEST_COLLISION_ACTOR_DIRECTION_OFFSET)?,
        )?)?,
        actor_state: state.read_u16_le(runtime_field(
            primary_actor_runtime,
            positive_offset(QUEST_COLLISION_ACTOR_STATE_OFFSET)?,
        )?)? as i16,
        input_state: state.read_u16_le(QUEST_COLLISION_INPUT_STATE_RUNTIME)?,
        op92_conditions: [
            state.read_u8(QUEST_COLLISION_CONDITION_RUNTIMES[0])?,
            state.read_u8(QUEST_COLLISION_CONDITION_RUNTIMES[1])?,
        ],
    };

    let stationary_probes = quest_collision_tile_probes(stationary_actor_rectangle, tile_bounds)?
        .into_iter()
        .map(|probe| {
            let tile = if probe.in_bounds {
                let x = usize::try_from(probe.x)
                    .map_err(|_| QuestMapCollisionStateError::CoordinateOverflow)?;
                let y = usize::try_from(probe.y)
                    .map_err(|_| QuestMapCollisionStateError::CoordinateOverflow)?;
                let width = usize::try_from(width)
                    .map_err(|_| QuestMapCollisionStateError::CoordinateOverflow)?;
                let index = y
                    .checked_mul(width)
                    .and_then(|row| row.checked_add(x))
                    .ok_or(QuestMapCollisionStateError::CoordinateOverflow)?;
                *live_cells
                    .get(index)
                    .ok_or(QuestMapCollisionStateError::CoordinateOverflow)?
            } else {
                1
            };
            Ok(QuestMapCollisionStationaryProbe {
                tile_x: probe.x,
                tile_y: probe.y,
                in_bounds: probe.in_bounds,
                tile,
                class: classify_quest_collision_tile(tile, classifier_context),
                overlap: quest_collision_rectangle_overlap(
                    stationary_actor_rectangle,
                    probe.rectangle,
                )?,
            })
        })
        .collect::<Result<Vec<_>, QuestMapCollisionStateError>>()?;

    let mut class_counts = [0usize; 3];
    for &tile in &live_cells {
        let class = classify_quest_collision_tile(tile, classifier_context);
        class_counts[usize::from(class.code())] += 1;
    }
    debug_assert_eq!(
        class_counts.iter().sum::<usize>(),
        cell_count,
        "classifier class 분모"
    );

    let (dimension_candidate_count, source_matches) =
        match_sources(width, height, &live_cells, sources);
    Ok(QuestMapCollisionStateReport {
        active_slot,
        width,
        height,
        primary_actor_runtime,
        actor_member_flags,
        actor_map_collision_enabled: u16::from(actor_member_flags)
            & QUEST_COLLISION_ACTOR_MAP_MEMBER_MASK
            != 0,
        actor_geometry,
        stationary_actor_rectangle,
        tile_bounds,
        stationary_probes,
        classifier_context,
        live_cells,
        live_tail_nonzero_cells,
        class_counts,
        dimension_candidate_count,
        source_matches,
    })
}

#[cfg(test)]
#[path = "quest_map_collision_tests.rs"]
mod tests;
