use super::*;

const HEADER_LEN: usize = 32;
const MAIN_RAM_LEN: usize = 2 * 1024 * 1024;
const PRIMARY_ACTOR: u32 = 0x800C_1000;

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
    raw.extend_from_slice(&u32::try_from(value.len()).expect("작은 값").to_le_bytes());
    raw.extend_from_slice(value);
    raw
}

fn section(name: &str, data: &[u8]) -> Vec<u8> {
    let mut raw = vec![0u8; 32];
    raw[..name.len()].copy_from_slice(name.as_bytes());
    raw.extend_from_slice(&u32::try_from(data.len()).expect("작은 값").to_le_bytes());
    raw.extend_from_slice(data);
    raw
}

fn state_fixture(active_slot: u8, width: u32, height: u32, cells: &[u16]) -> Vec<u8> {
    let mut ram = vec![0u8; MAIN_RAM_LEN];
    write_u8(&mut ram, QUEST_MAP_ACTIVE_SLOT_RUNTIME, active_slot);
    if usize::from(active_slot) < QUEST_MAP_SLOT_COUNT {
        let slot_offset = u32::from(active_slot) * 4;
        write_u32(&mut ram, QUEST_MAP_SLOT_WIDTHS_RUNTIME + slot_offset, width);
        write_u32(
            &mut ram,
            QUEST_MAP_SLOT_HEIGHTS_RUNTIME + slot_offset,
            height,
        );
    }
    for (index, &cell) in cells.iter().enumerate() {
        write_u16(
            &mut ram,
            QUEST_MAP_COLLISION_BUFFER_RUNTIME
                + u32::try_from(index * 2).expect("작은 cell offset"),
            cell,
        );
    }
    write_u32(
        &mut ram,
        QUEST_COLLISION_PRIMARY_ACTOR_RUNTIME,
        PRIMARY_ACTOR,
    );
    write_u32(
        &mut ram,
        PRIMARY_ACTOR + u32::try_from(QUEST_COLLISION_ACTOR_FIXED_X_OFFSET).unwrap(),
        0x0008_9000,
    );
    write_u32(
        &mut ram,
        PRIMARY_ACTOR + u32::try_from(QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET).unwrap(),
        0x0025_4000,
    );
    for (offset, value) in QUEST_COLLISION_ACTOR_RECTANGLE_OFFSETS
        .into_iter()
        .zip([-7i16, -13, 14, 15])
    {
        write_u16(
            &mut ram,
            PRIMARY_ACTOR + u32::try_from(offset).unwrap(),
            value as u16,
        );
    }
    write_u8(
        &mut ram,
        PRIMARY_ACTOR + u32::try_from(QUEST_COLLISION_ACTOR_MEMBER_FLAGS_OFFSET).unwrap(),
        1,
    );
    write_u32(
        &mut ram,
        PRIMARY_ACTOR + u32::try_from(QUEST_COLLISION_ACTOR_LAYER_OFFSET).unwrap(),
        0,
    );
    write_u8(
        &mut ram,
        PRIMARY_ACTOR + u32::try_from(QUEST_COLLISION_ACTOR_DIRECTION_OFFSET).unwrap(),
        0,
    );
    write_u16(
        &mut ram,
        PRIMARY_ACTOR + u32::try_from(QUEST_COLLISION_ACTOR_STATE_OFFSET).unwrap(),
        0,
    );
    write_u16(&mut ram, QUEST_COLLISION_INPUT_STATE_RUNTIME, 0x0004);
    for (runtime, value) in
        QUEST_COLLISION_MAP_BOUNDS_RUNTIMES
            .into_iter()
            .zip([0u16, 0, width as u16, height as u16])
    {
        write_u16(&mut ram, runtime, value);
    }

    let main = variable("MainRAM.data8", &ram);
    let mut raw = vec![0u8; HEADER_LEN];
    raw[..8].copy_from_slice(b"MDFNSVST");
    raw[16..20].copy_from_slice(&0x0010_3201u32.to_le_bytes());
    raw.extend_from_slice(&section("MAIN", &main));
    let len = u32::try_from(raw.len()).expect("작은 fixture");
    raw[20..24].copy_from_slice(&len.to_le_bytes());
    raw
}

fn qmp(width: u32, height: u32, cells: &[u16]) -> QmpMap {
    let mut raw = b"PBXQMAP\0".to_vec();
    raw.extend_from_slice(&20u32.to_le_bytes());
    raw.extend_from_slice(&width.to_le_bytes());
    raw.extend_from_slice(&height.to_le_bytes());
    for &cell in cells {
        raw.extend_from_slice(&cell.to_le_bytes());
    }
    QmpMap::parse(&raw).expect("QMP fixture")
}

#[test]
fn active_plane을_유일한_원본과_zero_clearing으로_결합한다() {
    let live = [0, 1, 0, 0x4001, 6, 97, 112, 7];
    let expected = qmp(4, 2, &[0, 1, 5, 0x4001, 6, 97, 112, 7]);
    let incompatible = qmp(4, 2, &[0, 1, 5, 0x4001, 6, 97, 112, 9]);
    let state_raw = state_fixture(1, 4, 2, &live);
    let state = MednafenPsxState::parse(&state_raw).unwrap();
    let sources = [
        QuestMapCollisionSource {
            resource_id: 6,
            name: "SUKE00_2.QMP",
            map: &expected,
        },
        QuestMapCollisionSource {
            resource_id: 9,
            name: "OTHER_2.QMP",
            map: &incompatible,
        },
    ];

    let report = inspect_quest_map_collision_state(&state, &sources).unwrap();
    assert_eq!(report.active_slot, 1);
    assert_eq!((report.width, report.height), (4, 2));
    assert_eq!(report.primary_actor_runtime, PRIMARY_ACTOR);
    assert_eq!(report.actor_member_flags, 1);
    assert!(report.actor_map_collision_enabled);
    assert_eq!(
        report.actor_geometry,
        QuestCollisionActorGeometry {
            position_fixed: [0x0008_9000, 0x0025_4000],
            local_rectangle: [-7, -13, 14, 15],
        }
    );
    assert_eq!(
        report.stationary_actor_rectangle,
        QuestCollisionRectangle {
            x: 130,
            y: 583,
            horizontal_extent: 14,
            vertical_extent: 15,
        }
    );
    assert_eq!(report.stationary_probes.len(), 9);
    assert!(report
        .stationary_probes
        .iter()
        .all(|probe| !probe.in_bounds));
    assert_eq!(report.live_cells, live);
    assert_eq!(report.live_tail_nonzero_cells, 0);
    assert_eq!(report.class_counts, [3, 2, 3]);
    assert_eq!(report.dimension_candidate_count, 2);
    let source_match = report.unique_source_match().expect("유일한 QMP 후보");
    assert_eq!(source_match.resource_id, 6);
    assert_eq!(source_match.name, "SUKE00_2.QMP");
    assert_eq!(
        source_match.zero_cleared_cells,
        [QuestMapCollisionClearedCell {
            index: 2,
            x: 2,
            y: 0,
            source_tile: 5,
            runtime_tile: 0,
        }]
    );
}

#[test]
fn 같은_runtime_plane과_양립하는_원본은_임의로_하나를_고르지_않는다() {
    let map_a = qmp(2, 1, &[1, 0]);
    let map_b = qmp(2, 1, &[1, 0]);
    let state_raw = state_fixture(0, 2, 1, &[1, 0]);
    let state = MednafenPsxState::parse(&state_raw).unwrap();
    let sources = [
        QuestMapCollisionSource {
            resource_id: 1,
            name: "A.QMP",
            map: &map_a,
        },
        QuestMapCollisionSource {
            resource_id: 2,
            name: "B.QMP",
            map: &map_b,
        },
    ];

    let report = inspect_quest_map_collision_state(&state, &sources).unwrap();
    assert_eq!(report.source_matches.len(), 2);
    assert_eq!(report.unique_source_match(), None);
}

#[test]
fn live_map_밖의_published_tail은_진단값으로만_센다() {
    let map = qmp(2, 1, &[1, 0]);
    let state_raw = state_fixture(0, 2, 1, &[1, 0, 7]);
    let state = MednafenPsxState::parse(&state_raw).unwrap();
    let source = [QuestMapCollisionSource {
        resource_id: 1,
        name: "A.QMP",
        map: &map,
    }];

    let report = inspect_quest_map_collision_state(&state, &source).unwrap();
    assert_eq!(report.live_cells, [1, 0]);
    assert_eq!(report.live_tail_nonzero_cells, 1);
    assert_eq!(report.unique_source_match().unwrap().name, "A.QMP");
}

#[test]
fn active_slot과_map_capacity_drift를_부분관측으로_반환하지_않는다() {
    let map = qmp(1, 1, &[0]);
    let source = [QuestMapCollisionSource {
        resource_id: 1,
        name: "A.QMP",
        map: &map,
    }];

    let invalid_slot_raw = state_fixture(2, 1, 1, &[0]);
    let invalid_slot = MednafenPsxState::parse(&invalid_slot_raw).unwrap();
    assert_eq!(
        inspect_quest_map_collision_state(&invalid_slot, &source),
        Err(QuestMapCollisionStateError::InvalidActiveSlot(2))
    );

    let oversized_raw = state_fixture(0, 97, 96, &[]);
    let oversized = MednafenPsxState::parse(&oversized_raw).unwrap();
    assert_eq!(
        inspect_quest_map_collision_state(&oversized, &source),
        Err(QuestMapCollisionStateError::MapCapacityExceeded {
            cells: 9_312,
            capacity: QUEST_MAP_PLANE_CAPACITY_CELLS,
        })
    );
}

#[test]
fn source_resource_id중복을_후보_matching전에_거부한다() {
    let map = qmp(1, 1, &[0]);
    let state_raw = state_fixture(0, 1, 1, &[0]);
    let state = MednafenPsxState::parse(&state_raw).unwrap();
    let sources = [
        QuestMapCollisionSource {
            resource_id: 1,
            name: "A.QMP",
            map: &map,
        },
        QuestMapCollisionSource {
            resource_id: 1,
            name: "B.QMP",
            map: &map,
        },
    ];
    assert_eq!(
        inspect_quest_map_collision_state(&state, &sources),
        Err(QuestMapCollisionStateError::DuplicateSourceResourceId(1))
    );
}
