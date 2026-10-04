use super::*;

fn observed_actor() -> QuestCollisionActorGeometry {
    QuestCollisionActorGeometry {
        position_fixed: [0x0008_9000, 0x0025_4000],
        local_rectangle: [-7, -13, 14, 15],
    }
}

#[test]
fn suke00_actor의_정지사각형과_아홉_tile_probe를_재현한다() {
    let actor_rectangle = quest_collision_actor_rectangle(observed_actor(), [0, 0]).unwrap();
    assert_eq!(
        actor_rectangle,
        QuestCollisionRectangle {
            x: 130,
            y: 583,
            horizontal_extent: 14,
            vertical_extent: 15,
        }
    );
    let probes = quest_collision_tile_probes(
        actor_rectangle,
        QuestCollisionTileBounds {
            min_x: 0,
            min_y: 0,
            max_x_exclusive: 70,
            max_y_exclusive: 64,
        },
    )
    .unwrap();
    assert_eq!(probes.len(), 9);
    assert_eq!((probes[0].x, probes[0].y), (7, 35));
    assert_eq!((probes[8].x, probes[8].y), (9, 37));

    let overlaps = probes
        .iter()
        .filter_map(|probe| {
            quest_collision_rectangle_overlap(actor_rectangle, probe.rectangle)
                .unwrap()
                .map(|overlap| ((probe.x, probe.y), overlap))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        overlaps,
        [
            (
                (8, 36),
                QuestCollisionOverlap {
                    x: 130,
                    y: 583,
                    width_pixels: 14,
                    height_pixels: 9,
                },
            ),
            (
                (8, 37),
                QuestCollisionOverlap {
                    x: 130,
                    y: 593,
                    width_pixels: 14,
                    height_pixels: 6,
                },
            ),
        ]
    );
}

#[test]
fn fixed_point의_음수delta는_0쪽으로_잘라_pixel에_더한다() {
    let actor = QuestCollisionActorGeometry {
        position_fixed: [0, 0],
        local_rectangle: [0, 0, 0, 0],
    };
    assert_eq!(
        quest_collision_actor_rectangle(actor, [-0x1FFF, -0x1000]).unwrap(),
        QuestCollisionRectangle {
            x: -1,
            y: -1,
            horizontal_extent: 0,
            vertical_extent: 0,
        }
    );
}

#[test]
fn 사각형경계가_한_pixel에서_닿아도_overlap이다() {
    let first = QuestCollisionRectangle {
        x: 0,
        y: 0,
        horizontal_extent: 4,
        vertical_extent: 4,
    };
    let second = QuestCollisionRectangle {
        x: 4,
        y: 4,
        horizontal_extent: 2,
        vertical_extent: 2,
    };
    assert_eq!(
        quest_collision_rectangle_overlap(first, second).unwrap(),
        Some(QuestCollisionOverlap {
            x: 4,
            y: 4,
            width_pixels: 1,
            height_pixels: 1,
        })
    );
}

#[test]
fn map밖_probe를_버리지_않고_source의_tile1경계로_남긴다() {
    let probes = quest_collision_tile_probes(
        QuestCollisionRectangle {
            x: 0,
            y: 0,
            horizontal_extent: 14,
            vertical_extent: 14,
        },
        QuestCollisionTileBounds {
            min_x: 0,
            min_y: 0,
            max_x_exclusive: 2,
            max_y_exclusive: 2,
        },
    )
    .unwrap();
    assert_eq!(probes.len(), 9);
    assert_eq!(probes.iter().filter(|probe| probe.in_bounds).count(), 4);
    assert!(probes.iter().any(|probe| (probe.x, probe.y) == (-1, -1)));
}

#[test]
fn 음수extent와_역전tile경계를_부분geometry로_반환하지_않는다() {
    let invalid_rectangle = QuestCollisionRectangle {
        x: 0,
        y: 0,
        horizontal_extent: -1,
        vertical_extent: 0,
    };
    assert!(matches!(
        quest_collision_tile_probes(
            invalid_rectangle,
            QuestCollisionTileBounds {
                min_x: 0,
                min_y: 0,
                max_x_exclusive: 1,
                max_y_exclusive: 1,
            }
        ),
        Err(QuestCollisionGeometryError::InvalidRectangleExtent { .. })
    ));

    let rectangle = QuestCollisionRectangle {
        x: 0,
        y: 0,
        horizontal_extent: 0,
        vertical_extent: 0,
    };
    assert!(matches!(
        quest_collision_tile_probes(
            rectangle,
            QuestCollisionTileBounds {
                min_x: 1,
                min_y: 0,
                max_x_exclusive: 1,
                max_y_exclusive: 1,
            }
        ),
        Err(QuestCollisionGeometryError::InvalidTileBounds(_))
    ));

    let oversized = QuestCollisionRectangle {
        horizontal_extent: 2_048,
        ..rectangle
    };
    assert!(matches!(
        quest_collision_tile_probes(
            oversized,
            QuestCollisionTileBounds {
                min_x: 0,
                min_y: 0,
                max_x_exclusive: 1,
                max_y_exclusive: 1,
            }
        ),
        Err(QuestCollisionGeometryError::ProbeRadiusOutsideSourceByte { .. })
    ));
}
