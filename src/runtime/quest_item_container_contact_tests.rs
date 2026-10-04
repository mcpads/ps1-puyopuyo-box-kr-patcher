use super::*;
use crate::disasm::quest_map_collision::{
    classify_quest_collision_tile, QuestCollisionClassifierContext, QuestCollisionTileBounds,
};
use crate::runtime::{QuestMapCollisionSourceMatch, QuestMapCollisionStateReport};

const HEADER_LEN: usize = 32;
const MAIN_RAM_LEN: usize = 2 * 1024 * 1024;
const PLAYER: u32 = 0x800C_09F8;

fn ram_offset(runtime: u32) -> usize {
    usize::try_from(runtime & 0x1F_FFFF).expect("PS1 RAM offset")
}

fn write_u8(ram: &mut [u8], runtime: u32, value: u8) {
    ram[ram_offset(runtime)] = value;
}

fn write_u16(ram: &mut [u8], runtime: u32, value: u16) {
    let offset = ram_offset(runtime);
    ram[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(ram: &mut [u8], runtime: u32, value: u32) {
    let offset = ram_offset(runtime);
    ram[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn variable(name: &str, value: &[u8]) -> Vec<u8> {
    let mut raw = Vec::new();
    raw.push(u8::try_from(name.len()).expect("짧은 이름"));
    raw.extend_from_slice(name.as_bytes());
    raw.extend_from_slice(&u32::try_from(value.len()).unwrap().to_le_bytes());
    raw.extend_from_slice(value);
    raw
}

fn section(name: &str, data: &[u8]) -> Vec<u8> {
    let mut raw = vec![0u8; 32];
    raw[..name.len()].copy_from_slice(name.as_bytes());
    raw.extend_from_slice(&u32::try_from(data.len()).unwrap().to_le_bytes());
    raw.extend_from_slice(data);
    raw
}

fn state_raw(mut customize: impl FnMut(&mut [u8])) -> Vec<u8> {
    let mut ram = vec![0u8; MAIN_RAM_LEN];
    write_u32(&mut ram, QUEST_COLLISION_PRIMARY_ACTOR_RUNTIME, PLAYER);
    write_u32(
        &mut ram,
        PLAYER + QUEST_OBJECT_CALLBACK_FIELD_OFFSET as u32,
        QUEST_PLAYER_UPDATE_CALLBACK_RUNTIME,
    );
    write_u32(
        &mut ram,
        PLAYER + QUEST_COLLISION_ACTOR_FIXED_X_OFFSET as u32,
        16 << 12,
    );
    write_u32(
        &mut ram,
        PLAYER + QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET as u32,
        16 << 12,
    );
    write_u32(
        &mut ram,
        PLAYER + QUEST_COLLISION_ACTOR_LAYER_OFFSET as u32,
        0,
    );
    for (offset, value) in QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS
        .into_iter()
        .zip([-7_i16, -13, 14, 15])
    {
        write_u16(&mut ram, PLAYER + offset as u32, value as u16);
    }
    write_u8(
        &mut ram,
        PLAYER + QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET as u32,
        1,
    );
    put_container(&mut ram, 82, [80, 80], 5);
    put_container(&mut ram, 107, [48, 48], 2);
    customize(&mut ram);

    let main = variable("MainRAM.data8", &ram);
    let mut raw = vec![0u8; HEADER_LEN];
    raw[..8].copy_from_slice(b"MDFNSVST");
    raw[16..20].copy_from_slice(&0x0010_3201u32.to_le_bytes());
    raw.extend_from_slice(&section("MAIN", &main));
    let len = u32::try_from(raw.len()).unwrap();
    raw[20..24].copy_from_slice(&len.to_le_bytes());
    raw
}

fn put_container(ram: &mut [u8], index: usize, anchor: [i32; 2], item_id: u32) {
    let object =
        QUEST_OBJECT_POOL_RUNTIME + u32::try_from(index * QUEST_OBJECT_POOL_RECORD_BYTES).unwrap();
    write_u32(
        ram,
        object + QUEST_OBJECT_CALLBACK_FIELD_OFFSET as u32,
        QUEST_ITEM_CONTAINER_CALLBACK_RUNTIME,
    );
    write_u32(
        ram,
        object + QUEST_COLLISION_ACTOR_FIXED_X_OFFSET as u32,
        u32::try_from(anchor[0] << 12).unwrap(),
    );
    write_u32(
        ram,
        object + QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET as u32,
        u32::try_from(anchor[1] << 12).unwrap(),
    );
    write_u32(ram, object + QUEST_COLLISION_ACTOR_LAYER_OFFSET as u32, 0);
    for (offset, value) in QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS
        .into_iter()
        .zip(QUEST_ITEM_CONTAINER_LOCAL_RECTANGLE)
    {
        write_u16(ram, object + offset as u32, value as u16);
    }
    write_u8(
        ram,
        object + QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET as u32,
        2,
    );
    write_u16(ram, object + QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET as u32, 0);
    write_u16(ram, object + QUEST_SEQ_CHILD_STATE_OFFSET as u32, 0);
    write_u8(
        ram,
        object + QUEST_ITEM_CONTAINER_PERSISTENCE_FIELD_OFFSET as u32,
        0x42,
    );
    write_u16(
        ram,
        object + QUEST_ITEM_CONTAINER_PARAMETER_FIELD_OFFSET as u32,
        3,
    );
    write_u32(
        ram,
        object + QUEST_ITEM_CONTAINER_ITEM_ID_FIELD_OFFSET as u32,
        item_id,
    );
}

fn map_report() -> QuestMapCollisionStateReport {
    let actor_geometry = QuestCollisionActorGeometry {
        position_fixed: [16 << 12, 16 << 12],
        local_rectangle: [-7, -13, 14, 15],
    };
    let live_cells = vec![0; 8 * 8];
    let class_counts = live_cells.iter().fold([0usize; 3], |mut counts, &tile| {
        let class = classify_quest_collision_tile(
            tile,
            QuestCollisionClassifierContext {
                actor_is_primary: true,
                actor_layer: 0,
                actor_direction: 2,
                actor_state: 0,
                input_state: 0,
                op92_conditions: [0, 0],
            },
        );
        counts[class as usize] += 1;
        counts
    });
    QuestMapCollisionStateReport {
        active_slot: 0,
        width: 8,
        height: 8,
        primary_actor_runtime: PLAYER,
        actor_member_flags: 1,
        actor_map_collision_enabled: true,
        actor_geometry,
        stationary_actor_rectangle: quest_collision_actor_rectangle(actor_geometry, [0, 0])
            .unwrap(),
        tile_bounds: QuestCollisionTileBounds {
            min_x: 0,
            min_y: 0,
            max_x_exclusive: 8,
            max_y_exclusive: 8,
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
        live_cells,
        live_tail_nonzero_cells: 0,
        class_counts,
        dimension_candidate_count: 1,
        source_matches: vec![QuestMapCollisionSourceMatch {
            resource_id: 6,
            name: "FIXTURE.QMP".to_string(),
            zero_cleared_cells: Vec::new(),
        }],
    }
}

#[test]
fn exact_container의_남쪽_접촉면과_target_우회_route를_만든다() {
    let raw = state_raw(|_| {});
    let state = MednafenPsxState::parse(&raw).unwrap();
    let report = inspect_quest_item_container_contact_state(&state, &map_report(), [80, 80])
        .expect("container contact");

    assert_eq!(report.container_object_count, 2);
    assert_eq!(report.target.pool_index, 82);
    assert_eq!(report.target.item_id, 5);
    assert_eq!(
        report.target.world_rectangle,
        QuestCollisionRectangle {
            x: 65,
            y: 65,
            horizontal_extent: 30,
            vertical_extent: 15,
        }
    );
    assert_eq!(report.interaction_direction, 2);
    assert_eq!(report.interaction_contact_mask, 4);
    assert_eq!(report.interaction_input_mask, 0x20);
    assert_eq!(report.centered_contact_anchor_pixel, [80, 94]);
    assert!(report.contact_anchor_candidate_count > 1);
    assert_eq!(report.contact_anchor_pixel, [72, 94]);
    assert_eq!(
        report.player_rectangle_at_contact,
        QuestCollisionRectangle {
            x: 65,
            y: 81,
            horizontal_extent: 14,
            vertical_extent: 15,
        }
    );
    assert_eq!(
        report.interaction_probe_at_contact,
        QuestCollisionRectangle {
            x: 66,
            y: 77,
            horizontal_extent: 12,
            vertical_extent: 4,
        }
    );
    assert_eq!(
        report.interaction_overlap,
        QuestCollisionOverlap {
            x: 66,
            y: 77,
            width_pixels: 13,
            height_pixels: 4,
        }
    );
    assert_eq!(report.route.route.goal_pixel, [72, 94]);
    assert_eq!(report.route.position_step_pixels, 2);
    for segment in report.route.route.waypoints.windows(2) {
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
                    local_rectangle: map_report().actor_geometry.local_rectangle,
                },
                [0, 0],
            )
            .unwrap();
            assert_eq!(
                quest_collision_rectangle_overlap(actor, report.target.world_rectangle).unwrap(),
                None,
                "route position={position:?}",
            );
        }
    }
}

#[test]
fn target_callback_중복과_geometry_drift를_부분결과없이_거부한다() {
    let duplicate_raw = state_raw(|ram| put_container(ram, 108, [80, 80], 7));
    let duplicate = MednafenPsxState::parse(&duplicate_raw).unwrap();
    assert_eq!(
        inspect_quest_item_container_contact_state(&duplicate, &map_report(), [80, 80]),
        Err(QuestItemContainerContactError::MultipleContainersAtAnchor {
            anchor: [80, 80],
            count: 2,
        })
    );

    let drift_raw = state_raw(|ram| {
        let object =
            QUEST_OBJECT_POOL_RUNTIME + u32::try_from(82 * QUEST_OBJECT_POOL_RECORD_BYTES).unwrap();
        write_u16(
            ram,
            object + QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS[2] as u32,
            32,
        );
    });
    let drift = MednafenPsxState::parse(&drift_raw).unwrap();
    assert_eq!(
        inspect_quest_item_container_contact_state(&drift, &map_report(), [80, 80]),
        Err(QuestItemContainerContactError::TargetGeometryDrift {
            actual: [-15, -15, 32, 15],
        })
    );
}

#[test]
fn state와_map_report의_player_geometry가_다르면_거부한다() {
    let raw = state_raw(|_| {});
    let state = MednafenPsxState::parse(&raw).unwrap();
    let mut report = map_report();
    report.actor_geometry.position_fixed[0] += 1 << 12;

    assert_eq!(
        inspect_quest_item_container_contact_state(&state, &report, [80, 80]),
        Err(QuestItemContainerContactError::PlayerGeometryMismatch {
            report: report.actor_geometry,
            state: QuestCollisionActorGeometry {
                position_fixed: [16 << 12, 16 << 12],
                local_rectangle: [-7, -13, 14, 15],
            },
        })
    );
}

#[test]
fn 다른_player_callback과_이미_열리는중인_container를_거부한다() {
    let callback_raw = state_raw(|ram| {
        write_u32(
            ram,
            PLAYER + QUEST_OBJECT_CALLBACK_FIELD_OFFSET as u32,
            0x8005_4744,
        );
    });
    let callback = MednafenPsxState::parse(&callback_raw).unwrap();
    assert_eq!(
        inspect_quest_item_container_contact_state(&callback, &map_report(), [80, 80]),
        Err(QuestItemContainerContactError::UnsupportedPlayerCallback(
            0x8005_4744
        ))
    );

    let active_raw = state_raw(|ram| {
        let object =
            QUEST_OBJECT_POOL_RUNTIME + u32::try_from(82 * QUEST_OBJECT_POOL_RECORD_BYTES).unwrap();
        write_u16(ram, object + QUEST_SEQ_CHILD_STATE_OFFSET as u32, 1);
    });
    let active = MednafenPsxState::parse(&active_raw).unwrap();
    assert_eq!(
        inspect_quest_item_container_contact_state(&active, &map_report(), [80, 80]),
        Err(QuestItemContainerContactError::TargetNotIdle(1))
    );

    let contact_raw = state_raw(|ram| {
        let object =
            QUEST_OBJECT_POOL_RUNTIME + u32::try_from(82 * QUEST_OBJECT_POOL_RECORD_BYTES).unwrap();
        write_u16(
            ram,
            object + QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET as u32,
            QUEST_ITEM_CONTAINER_OPEN_CONTACT_MASK,
        );
    });
    let contact = MednafenPsxState::parse(&contact_raw).unwrap();
    assert_eq!(
        inspect_quest_item_container_contact_state(&contact, &map_report(), [80, 80]),
        Err(QuestItemContainerContactError::TargetContactFlagsNotClear(
            QUEST_ITEM_CONTAINER_OPEN_CONTACT_MASK,
        ))
    );
}
