use crate::disasm::quest_map_collision::{
    quest_collision_actor_rectangle, QuestCollisionActorGeometry, QuestCollisionClassifierContext,
    QuestCollisionRectangle, QuestCollisionTileBounds,
};

use super::*;
use crate::runtime::QuestMapCollisionStateReport;

fn report_with_cells(width: u32, height: u32, cells: Vec<u16>) -> QuestMapCollisionStateReport {
    let actor_geometry = QuestCollisionActorGeometry {
        position_fixed: [8 << 12, 8 << 12],
        local_rectangle: [-7, -7, 14, 14],
    };
    QuestMapCollisionStateReport {
        active_slot: 0,
        width,
        height,
        primary_actor_runtime: 0x800C_0000,
        actor_member_flags: 1,
        actor_map_collision_enabled: true,
        actor_geometry,
        stationary_actor_rectangle: quest_collision_actor_rectangle(actor_geometry, [0, 0])
            .expect("fixture actor geometry"),
        tile_bounds: QuestCollisionTileBounds {
            min_x: 0,
            min_y: 0,
            max_x_exclusive: i32::try_from(width).expect("fixture width"),
            max_y_exclusive: i32::try_from(height).expect("fixture height"),
        },
        stationary_probes: Vec::new(),
        classifier_context: QuestCollisionClassifierContext {
            actor_is_primary: true,
            actor_layer: 0,
            actor_direction: 2,
            actor_state: 0,
            input_state: 0,
            op92_conditions: [0, 0],
        },
        live_cells: cells,
        live_tail_nonzero_cells: 0,
        class_counts: [0, 0, 0],
        dimension_candidate_count: 1,
        source_matches: Vec::new(),
    }
}

#[test]
fn 열린_map을_source의_2px_delta로만_이동한다() {
    let report = report_with_cells(3, 3, vec![0; 9]);
    let walk =
        find_direction_robust_quest_map_walk_route_avoiding_rectangles(&report, [40, 40], &[])
            .expect("정상 보행 route");

    assert_eq!(walk.position_step_pixels, 2);
    assert_eq!(walk.route.pixel_steps, 64);
    assert_eq!(walk.movement_steps, 32);
    assert_eq!(walk.direction_input_masks, [0x4000, 0x8000, 0x1000, 0x2000]);
    assert_eq!(
        walk.direction_deltas_fixed,
        [[0, 0x2000], [-0x2000, 0], [0, -0x2000], [0x2000, 0]]
    );
    assert!(walk.route.waypoints.iter().all(|waypoint| {
        (waypoint[0] - walk.route.start_pixel[0]).rem_euclid(2) == 0
            && (waypoint[1] - walk.route.start_pixel[1]).rem_euclid(2) == 0
    }));
}

#[test]
fn 한_pixel만_다른_goal을_재생가능하다고_보고하지_않는다() {
    let report = report_with_cells(3, 3, vec![0; 9]);
    assert_eq!(
        find_direction_robust_quest_map_walk_route_avoiding_rectangles(&report, [9, 8], &[],),
        Err(QuestMapWalkRouteError::GoalOutsideMovementLattice {
            start: [8, 8],
            goal: [9, 8],
            step_pixels: 2,
        })
    );
}

#[test]
fn 장애물을_우회해도_시작점의_xy_parity를_유지한다() {
    let report = report_with_cells(5, 5, vec![0; 25]);
    let obstacle = QuestCollisionRectangle {
        x: 24,
        y: 0,
        horizontal_extent: 15,
        vertical_extent: 31,
    };
    let walk = find_direction_robust_quest_map_walk_route_avoiding_rectangles(
        &report,
        [72, 8],
        &[obstacle],
    )
    .expect("2px 격자의 우회 route");

    assert!(walk.route.pixel_steps > 64);
    assert!(walk
        .route
        .waypoints
        .iter()
        .all(|waypoint| { waypoint[0].rem_euclid(2) == 0 && waypoint[1].rem_euclid(2) == 0 }));
}

#[test]
fn 정상_state와_pixel정렬이_아니면_부분route를_만들지_않는다() {
    let mut wrong_state = report_with_cells(3, 3, vec![0; 9]);
    wrong_state.classifier_context.actor_state = 2;
    assert_eq!(
        find_direction_robust_quest_map_walk_route_avoiding_rectangles(&wrong_state, [40, 8], &[],),
        Err(QuestMapWalkRouteError::UnsupportedActorState {
            expected: 0,
            actual: 2,
        })
    );

    let mut subpixel = report_with_cells(3, 3, vec![0; 9]);
    subpixel.actor_geometry.position_fixed[0] += 1;
    assert_eq!(
        find_direction_robust_quest_map_walk_route_avoiding_rectangles(&subpixel, [40, 8], &[],),
        Err(QuestMapWalkRouteError::StartNotPixelAligned([1, 0]))
    );
}

#[test]
fn 여러_goal중_2px격자에서_먼저_닿는_것을_고른다() {
    let report = report_with_cells(4, 4, vec![0; 16]);
    let walk = find_direction_robust_quest_map_walk_route_to_any_avoiding_rectangles(
        &report,
        &[[40, 40], [24, 8]],
        &[],
    )
    .expect("복수 목표 route");

    assert_eq!(walk.route.goal_pixel, [24, 8]);
    assert_eq!(walk.movement_steps, 8);
}
