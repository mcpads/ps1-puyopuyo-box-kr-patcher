//! QUEST actor 사각형과 map tile probe의 좌표 모델.
//!
//! 원본 이동 함수는 actor 위치와 이동 delta를 12-bit fixed-point에서 pixel로
//! 자른 뒤 actor-local 사각형을 더한다. 그 사각형 주변의 16px tile을 훑고,
//! 각 tile에서는 가장자리 1px을 뺀 내부 사각형과 inclusive overlap을 검사한다.
//! 이 모델은 조사와 제한 route 후보 계산용이며 제품 patch 입력이 아니다.

use std::error::Error;
use std::fmt;

use super::{
    QUEST_COLLISION_FIXED_POINT_SHIFT, QUEST_COLLISION_PROBE_EXTENT_MARGIN_PIXELS,
    QUEST_COLLISION_TILE_INTERIOR_EXTENT_PIXELS, QUEST_COLLISION_TILE_INTERIOR_INSET_PIXELS,
    QUEST_COLLISION_TILE_SIZE_PIXELS,
};

/// actor 또는 tile이 overlap 함수에 넘기는 사각형.
///
/// `horizontal_extent`와 `vertical_extent`는 pixel 수가 아니라 시작 좌표에서
/// inclusive 끝 좌표까지의 차이다. 따라서 extent 14는 15px을 덮는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestCollisionRectangle {
    pub x: i32,
    pub y: i32,
    pub horizontal_extent: i32,
    pub vertical_extent: i32,
}

/// overlap 함수가 선택적으로 쓰는 실제 교집합 pixel 수.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestCollisionOverlap {
    pub x: i32,
    pub y: i32,
    pub width_pixels: i32,
    pub height_pixels: i32,
}

/// actor 객체의 fixed-point 위치와 local collision rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestCollisionActorGeometry {
    pub position_fixed: [i32; 2],
    pub local_rectangle: [i16; 4],
}

/// 이동 함수가 유효한 map tile로 인정하는 half-open 경계.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestCollisionTileBounds {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x_exclusive: i32,
    pub max_y_exclusive: i32,
}

impl QuestCollisionTileBounds {
    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.min_x && x < self.max_x_exclusive && y >= self.min_y && y < self.max_y_exclusive
    }
}

/// actor 사각형 주위를 훑는 tile 하나와 그 내부 사각형.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestCollisionTileProbe {
    pub x: i32,
    pub y: i32,
    pub in_bounds: bool,
    pub rectangle: QuestCollisionRectangle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestCollisionGeometryError {
    InvalidRectangleExtent { horizontal: i32, vertical: i32 },
    InvalidTileBounds(QuestCollisionTileBounds),
    ProbeRadiusOutsideSourceByte { horizontal: i32, vertical: i32 },
    CoordinateOverflow,
}

impl fmt::Display for QuestCollisionGeometryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRectangleExtent {
                horizontal,
                vertical,
            } => write!(
                formatter,
                "QUEST collision 사각형 extent가 음수임: horizontal={horizontal}, vertical={vertical}"
            ),
            Self::InvalidTileBounds(bounds) => write!(
                formatter,
                "QUEST collision tile 경계가 비어 있거나 역전됨: ({},{})..({},{})",
                bounds.min_x,
                bounds.min_y,
                bounds.max_x_exclusive,
                bounds.max_y_exclusive,
            ),
            Self::ProbeRadiusOutsideSourceByte {
                horizontal,
                vertical,
            } => write!(
                formatter,
                "QUEST collision probe 반경이 source signed byte 범위 밖임: horizontal={horizontal}, vertical={vertical}"
            ),
            Self::CoordinateOverflow => formatter.write_str("QUEST collision geometry overflow"),
        }
    }
}

impl Error for QuestCollisionGeometryError {}

fn verify_rectangle(rectangle: QuestCollisionRectangle) -> Result<(), QuestCollisionGeometryError> {
    if rectangle.horizontal_extent < 0 || rectangle.vertical_extent < 0 {
        return Err(QuestCollisionGeometryError::InvalidRectangleExtent {
            horizontal: rectangle.horizontal_extent,
            vertical: rectangle.vertical_extent,
        });
    }
    Ok(())
}

fn fixed_to_pixel(value: i32) -> i32 {
    value / (1_i32 << QUEST_COLLISION_FIXED_POINT_SHIFT)
}

/// actor 위치에 fixed-point delta를 더해 원본 이동 함수의 pixel 사각형을 만든다.
pub fn quest_collision_actor_rectangle(
    actor: QuestCollisionActorGeometry,
    delta_fixed: [i32; 2],
) -> Result<QuestCollisionRectangle, QuestCollisionGeometryError> {
    let pixel_x = fixed_to_pixel(
        actor.position_fixed[0]
            .checked_add(delta_fixed[0])
            .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?,
    );
    let pixel_y = fixed_to_pixel(
        actor.position_fixed[1]
            .checked_add(delta_fixed[1])
            .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?,
    );
    let rectangle = QuestCollisionRectangle {
        x: pixel_x
            .checked_add(i32::from(actor.local_rectangle[0]))
            .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?,
        y: pixel_y
            .checked_add(i32::from(actor.local_rectangle[1]))
            .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?,
        horizontal_extent: i32::from(actor.local_rectangle[2]),
        vertical_extent: i32::from(actor.local_rectangle[3]),
    };
    verify_rectangle(rectangle)?;
    Ok(rectangle)
}

/// 한 map tile에서 원본 이동 함수가 overlap 대상으로 만드는 내부 사각형.
pub fn quest_collision_tile_rectangle(
    x: i32,
    y: i32,
) -> Result<QuestCollisionRectangle, QuestCollisionGeometryError> {
    let tile_size = i32::from(QUEST_COLLISION_TILE_SIZE_PIXELS);
    let inset = i32::from(QUEST_COLLISION_TILE_INTERIOR_INSET_PIXELS);
    Ok(QuestCollisionRectangle {
        x: x.checked_mul(tile_size)
            .and_then(|value| value.checked_add(inset))
            .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?,
        y: y.checked_mul(tile_size)
            .and_then(|value| value.checked_add(inset))
            .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?,
        horizontal_extent: i32::from(QUEST_COLLISION_TILE_INTERIOR_EXTENT_PIXELS),
        vertical_extent: i32::from(QUEST_COLLISION_TILE_INTERIOR_EXTENT_PIXELS),
    })
}

/// 원본 `0x8006B61C`처럼 경계가 닿는 경우를 포함해 교집합 pixel 수를 만든다.
pub fn quest_collision_rectangle_overlap(
    first: QuestCollisionRectangle,
    second: QuestCollisionRectangle,
) -> Result<Option<QuestCollisionOverlap>, QuestCollisionGeometryError> {
    verify_rectangle(first)?;
    verify_rectangle(second)?;
    let first_right = first
        .x
        .checked_add(first.horizontal_extent)
        .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?;
    let second_right = second
        .x
        .checked_add(second.horizontal_extent)
        .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?;
    let first_bottom = first
        .y
        .checked_add(first.vertical_extent)
        .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?;
    let second_bottom = second
        .y
        .checked_add(second.vertical_extent)
        .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?;
    if first_right < second.x
        || second_right < first.x
        || first_bottom < second.y
        || second_bottom < first.y
    {
        return Ok(None);
    }

    let x = first.x.max(second.x);
    let y = first.y.max(second.y);
    let width_pixels = first_right
        .min(second_right)
        .checked_sub(x)
        .and_then(|value| value.checked_add(1))
        .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?;
    let height_pixels = first_bottom
        .min(second_bottom)
        .checked_sub(y)
        .and_then(|value| value.checked_add(1))
        .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?;
    Ok(Some(QuestCollisionOverlap {
        x,
        y,
        width_pixels,
        height_pixels,
    }))
}

/// actor 사각형 주위에서 원본 이동 함수가 훑는 tile과 내부 사각형을 만든다.
pub fn quest_collision_tile_probes(
    actor_rectangle: QuestCollisionRectangle,
    bounds: QuestCollisionTileBounds,
) -> Result<Vec<QuestCollisionTileProbe>, QuestCollisionGeometryError> {
    verify_rectangle(actor_rectangle)?;
    if bounds.min_x >= bounds.max_x_exclusive || bounds.min_y >= bounds.max_y_exclusive {
        return Err(QuestCollisionGeometryError::InvalidTileBounds(bounds));
    }

    let tile_size = i32::from(QUEST_COLLISION_TILE_SIZE_PIXELS);
    let margin = i32::from(QUEST_COLLISION_PROBE_EXTENT_MARGIN_PIXELS);
    let base_x = actor_rectangle.x / tile_size;
    let base_y = actor_rectangle.y / tile_size;
    let radius_x = actor_rectangle
        .horizontal_extent
        .checked_add(margin)
        .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?
        / tile_size;
    let radius_y = actor_rectangle
        .vertical_extent
        .checked_add(margin)
        .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?
        / tile_size;
    if radius_x > i32::from(i8::MAX) || radius_y > i32::from(i8::MAX) {
        return Err(QuestCollisionGeometryError::ProbeRadiusOutsideSourceByte {
            horizontal: radius_x,
            vertical: radius_y,
        });
    }
    let start_x = base_x
        .checked_sub(radius_x)
        .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?;
    let end_x = base_x
        .checked_add(radius_x)
        .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?;
    let start_y = base_y
        .checked_sub(radius_y)
        .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?;
    let end_y = base_y
        .checked_add(radius_y)
        .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?;

    let width = end_x
        .checked_sub(start_x)
        .and_then(|value| value.checked_add(1))
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?;
    let height = end_y
        .checked_sub(start_y)
        .and_then(|value| value.checked_add(1))
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?;
    let mut probes = Vec::with_capacity(
        width
            .checked_mul(height)
            .ok_or(QuestCollisionGeometryError::CoordinateOverflow)?,
    );
    for y in start_y..=end_y {
        for x in start_x..=end_x {
            probes.push(QuestCollisionTileProbe {
                x,
                y,
                in_bounds: bounds.contains(x, y),
                rectangle: quest_collision_tile_rectangle(x, y)?,
            });
        }
    }
    Ok(probes)
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod tests;
