use crate::disasm::quest_map_collision::{
    classify_quest_collision_tile, quest_collision_actor_rectangle,
    quest_collision_rectangle_overlap, QuestCollisionActorGeometry,
    QuestCollisionClassifierContext, QuestCollisionRectangle, QuestCollisionTileBounds,
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
fn open_plane_route_keeps_only_axis_turns() {
    let report = report_with_cells(3, 3, vec![0; 9]);
    let route = find_direction_robust_quest_map_route(&report, [40, 40]).expect("open route");

    assert_eq!(route.start_pixel, [8, 8]);
    assert_eq!(route.goal_pixel, [40, 40]);
    assert_eq!(route.pixel_steps, 64);
    assert_eq!(route.waypoints.first(), Some(&[8, 8]));
    assert_eq!(route.waypoints.last(), Some(&[40, 40]));
    assert!(route.waypoints.windows(2).all(|pair| {
        let horizontal = pair[0][1] == pair[1][1];
        let vertical = pair[0][0] == pair[1][0];
        horizontal ^ vertical
    }));
    assert_eq!(
        route
            .waypoints
            .windows(2)
            .map(|pair| {
                (pair[1][0] - pair[0][0]).unsigned_abs() as usize
                    + (pair[1][1] - pair[0][1]).unsigned_abs() as usize
            })
            .sum::<usize>(),
        route.pixel_steps,
    );
    assert_eq!(route.direction_branch_representatives, [0, 1]);
}

#[test]
fn live_zero_clearing_is_used_instead_of_the_source_tile() {
    let report = report_with_cells(3, 1, vec![0, 0, 0]);
    let route = find_direction_robust_quest_map_route(&report, [40, 8])
        .expect("live plane의 열린 cell을 사용");

    assert_eq!(route.pixel_steps, 32);
    assert_eq!(route.waypoints, vec![[8, 8], [40, 8]]);
}

#[test]
fn solid_column_makes_the_goal_unreachable() {
    let mut cells = vec![0; 9];
    cells[1] = 1;
    cells[4] = 1;
    cells[7] = 1;
    let report = report_with_cells(3, 3, cells);

    assert!(matches!(
        find_direction_robust_quest_map_route(&report, [40, 8]),
        Err(QuestMapRouteError::GoalUnreachable {
            start: [8, 8],
            goal: [40, 8],
            ..
        })
    ));
}

#[test]
fn blocked_goal_fails_before_search() {
    let mut cells = vec![0; 9];
    cells[8] = 1;
    let report = report_with_cells(3, 3, cells);

    assert_eq!(
        find_direction_robust_quest_map_route(&report, [40, 40]),
        Err(QuestMapRouteError::GoalBlocked([40, 40]))
    );
}

#[test]
fn disabled_actor_map_membership_rejects_the_model() {
    let mut report = report_with_cells(3, 3, vec![0; 9]);
    report.actor_map_collision_enabled = false;

    assert_eq!(
        find_direction_robust_quest_map_route(&report, [40, 40]),
        Err(QuestMapRouteError::MapCollisionDisabled)
    );
}

#[test]
fn malformed_live_plane_is_rejected() {
    let report = report_with_cells(3, 3, vec![0; 8]);

    assert_eq!(
        find_direction_robust_quest_map_route(&report, [40, 40]),
        Err(QuestMapRouteError::InvalidLivePlaneLength {
            expected: 9,
            actual: 8,
        })
    );
}

#[test]
fn one_nonzero_direction_represents_every_nonzero_classifier_branch() {
    for actor_is_primary in [false, true] {
        for actor_layer in [0, 1] {
            for actor_state in [0, 5] {
                for input_state in [0, 4] {
                    let base = QuestCollisionClassifierContext {
                        actor_is_primary,
                        actor_layer,
                        actor_direction: 1,
                        actor_state,
                        input_state,
                        op92_conditions: [0, 1],
                    };
                    for tile in u16::MIN..=u16::MAX {
                        let expected = classify_quest_collision_tile(tile, base);
                        for actor_direction in [2, 3, u8::MAX] {
                            let mut context = base;
                            context.actor_direction = actor_direction;
                            assert_eq!(
                                classify_quest_collision_tile(tile, context),
                                expected,
                                "tile=0x{tile:04X}, context={context:?}",
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn 명시한_object_사각형을_통과하지_않고_우회한다() {
    let report = report_with_cells(5, 5, vec![0; 25]);
    let obstacle = QuestCollisionRectangle {
        x: 24,
        y: 0,
        horizontal_extent: 15,
        vertical_extent: 31,
    };
    let route =
        find_direction_robust_quest_map_route_avoiding_rectangles(&report, [72, 8], &[obstacle])
            .expect("아래쪽으로 우회하는 경로");

    assert!(route.pixel_steps > 64);
    assert_eq!(route.waypoints.first(), Some(&[8, 8]));
    assert_eq!(route.waypoints.last(), Some(&[72, 8]));
    for segment in route.waypoints.windows(2) {
        let delta = [
            (segment[1][0] - segment[0][0]).signum(),
            (segment[1][1] - segment[0][1]).signum(),
        ];
        let steps = (segment[1][0] - segment[0][0]).unsigned_abs()
            + (segment[1][1] - segment[0][1]).unsigned_abs();
        for step in 0..=steps {
            let step = i32::try_from(step).expect("fixture route 길이");
            let position = [
                segment[0][0] + delta[0] * step,
                segment[0][1] + delta[1] * step,
            ];
            let actor = quest_collision_actor_rectangle(
                QuestCollisionActorGeometry {
                    position_fixed: [position[0] << 12, position[1] << 12],
                    local_rectangle: report.actor_geometry.local_rectangle,
                },
                [0, 0],
            )
            .unwrap();
            assert_eq!(
                quest_collision_rectangle_overlap(actor, obstacle).unwrap(),
                None,
                "route position={position:?}",
            );
        }
    }
}

#[test]
fn 목표_actor가_object와_겹치면_search전에_거부한다() {
    let report = report_with_cells(3, 3, vec![0; 9]);
    let obstacle = QuestCollisionRectangle {
        x: 32,
        y: 32,
        horizontal_extent: 15,
        vertical_extent: 15,
    };
    assert_eq!(
        find_direction_robust_quest_map_route_avoiding_rectangles(&report, [40, 40], &[obstacle]),
        Err(QuestMapRouteError::GoalBlocked([40, 40]))
    );
}
