//! QUEST tile classifier의 문맥 의존 반환값 모델.
//!
//! 이 모델은 정적 조사와 제한 route 후보 계산에 쓴다. 제품 patch writer의
//! 입력이 아니며, 실행 중 active collision plane과 문맥을 별도로 읽어야 한다.

use super::{
    QUEST_COLLISION_INPUT_STATE_MASK, QUEST_COLLISION_TILE_FLAG_MASKS,
    QUEST_COLLISION_TILE_LOW_BYTE_MASK,
};

/// `0x8006B928`이 반환하고 movement consumer가 구분하는 세 class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestCollisionClass {
    /// movement consumer가 tile 처리를 건너뛴다.
    NoCollision,
    /// 겹침 조정 뒤 객체 contact counter를 증가시킨다.
    SpecialContact,
    /// 겹침을 조정하고 인접 probe의 blocking predicate가 된다.
    Blocking,
}

impl QuestCollisionClass {
    /// 원본 함수의 u8 반환값.
    pub const fn code(self) -> u8 {
        match self {
            Self::NoCollision => 0,
            Self::SpecialContact => 1,
            Self::Blocking => 2,
        }
    }
}

/// 한 classifier 호출에서 객체와 전역 상태가 제공하는 문맥.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestCollisionClassifierContext {
    pub actor_is_primary: bool,
    pub actor_layer: u32,
    pub actor_direction: u8,
    pub actor_state: i16,
    pub input_state: u16,
    pub op92_conditions: [u8; 2],
}

/// 원본 `0x8006B928`의 분기를 같은 class로 평가한다.
pub fn classify_quest_collision_tile(
    tile: u16,
    context: QuestCollisionClassifierContext,
) -> QuestCollisionClass {
    let low = (tile & QUEST_COLLISION_TILE_LOW_BYTE_MASK) as u8;

    // ID 2와 3..=5는 일반 plane 선택 분기보다 먼저 갈라진다.
    if low == 2 {
        return if context.actor_layer == 0 {
            QuestCollisionClass::Blocking
        } else {
            classify_selected_tile(low, tile, context)
        };
    }
    if (3..=5).contains(&low) {
        return classify_selected_tile(low, tile, context);
    }

    if !selects_actor_plane(tile, context.actor_layer) {
        return QuestCollisionClass::Blocking;
    }
    classify_selected_tile(low, tile, context)
}

fn selects_actor_plane(tile: u16, actor_layer: u32) -> bool {
    tile & QUEST_COLLISION_TILE_FLAG_MASKS[1] != 0 || actor_plane_matches(tile, actor_layer)
}

fn actor_plane_matches(tile: u16, actor_layer: u32) -> bool {
    let upper_plane = tile & QUEST_COLLISION_TILE_FLAG_MASKS[0] != 0;
    (actor_layer == 0 && !upper_plane) || (actor_layer != 0 && upper_plane)
}

fn classify_selected_tile(
    low: u8,
    tile: u16,
    context: QuestCollisionClassifierContext,
) -> QuestCollisionClass {
    match low {
        1 => QuestCollisionClass::Blocking,
        6 => classify_directional_tile(tile, context),
        97 if context.op92_conditions[0] == 0 => classify_condition_tile(tile, context.actor_layer),
        99 if context.op92_conditions[1] == 0 => classify_condition_tile(tile, context.actor_layer),
        102 | 112..=127 => classify_primary_actor_tile(tile, context),
        109 if context.op92_conditions[0] != 0 => {
            classify_condition_tile(tile, context.actor_layer)
        }
        110 if context.op92_conditions[1] != 0 => {
            classify_condition_tile(tile, context.actor_layer)
        }
        _ => QuestCollisionClass::NoCollision,
    }
}

fn classify_condition_tile(tile: u16, actor_layer: u32) -> QuestCollisionClass {
    if actor_plane_matches(tile, actor_layer) {
        QuestCollisionClass::Blocking
    } else {
        QuestCollisionClass::NoCollision
    }
}

fn classify_directional_tile(
    tile: u16,
    context: QuestCollisionClassifierContext,
) -> QuestCollisionClass {
    if !actor_plane_matches(tile, context.actor_layer) {
        return QuestCollisionClass::NoCollision;
    }
    if context.actor_direction != 0 {
        return QuestCollisionClass::Blocking;
    }
    if context.input_state & QUEST_COLLISION_INPUT_STATE_MASK != 0 {
        QuestCollisionClass::SpecialContact
    } else {
        QuestCollisionClass::Blocking
    }
}

fn classify_primary_actor_tile(
    tile: u16,
    context: QuestCollisionClassifierContext,
) -> QuestCollisionClass {
    if !context.actor_is_primary {
        return QuestCollisionClass::Blocking;
    }
    if !actor_plane_matches(tile, context.actor_layer) || context.actor_state == 5 {
        return QuestCollisionClass::NoCollision;
    }
    QuestCollisionClass::SpecialContact
}

#[cfg(test)]
#[path = "classifier_tests.rs"]
mod tests;
