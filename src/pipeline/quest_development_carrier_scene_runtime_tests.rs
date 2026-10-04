use super::*;

const HEADER_LEN: usize = 32;
const PSX_MAIN_RAM_LEN: usize = 2 * 1024 * 1024;

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
    raw.extend_from_slice(
        &u32::try_from(data.len())
            .expect("작은 section")
            .to_le_bytes(),
    );
    raw.extend_from_slice(data);
    raw
}

fn state_with_main_ram(main_ram: &[u8]) -> Vec<u8> {
    let main = variable("MainRAM.data8", main_ram);
    let main_section = section("MAIN", &main);
    let mut raw = vec![0u8; HEADER_LEN];
    raw[..8].copy_from_slice(b"MDFNSVST");
    raw[16..20].copy_from_slice(&0x0010_3201u32.to_le_bytes());
    raw.extend_from_slice(&main_section);
    let len = u32::try_from(raw.len()).expect("작은 fixture");
    raw[20..24].copy_from_slice(&len.to_le_bytes());
    raw
}

fn observation_json(scratch_runtime: u32, scratch_bytes: usize, second_hits: u64) -> Vec<u8> {
    let ranges = expected_protected_ranges(scratch_runtime, scratch_bytes).expect("보호 범위");
    format!(
        r#"{{
  "schema_version":"{OBSERVATION_SCHEMA_VERSION}",
  "scene_id":"post-battle-dialogue",
  "launch_id":"launch-test",
  "tracking_run_id":"run-test",
  "cold_boot":true,
  "start_frame":10,
  "end_frame":25,
  "state_loads":0,
  "memory_writes":0,
  "resets":0,
  "mutable_scratch":{{"start_runtime":{},"bytes":{}}},
  "protected_write_ranges":[
    {{"start_runtime":{},"end_runtime_inclusive":{},"hits":0}},
    {{"start_runtime":{},"end_runtime_inclusive":{},"hits":{second_hits}}}
  ],
  "guest_advance_completed":true
}}"#,
        scratch_runtime,
        scratch_bytes,
        ranges[0].start_runtime,
        ranges[0].end_runtime_inclusive,
        ranges[1].start_runtime,
        ranges[1].end_runtime_inclusive,
    )
    .into_bytes()
}

#[test]
fn 보호범위는_mutable_scratch_양쪽에서_파생한다() {
    let scratch_runtime = QUEST_PAGE_CARRIER_START_RUNTIME + 0x800;
    let scratch_bytes = 0x60;
    let observation = verify_write_observation(
        &observation_json(scratch_runtime, scratch_bytes, 0),
        scratch_runtime,
        scratch_bytes,
    )
    .expect("완전한 cold route observation");

    assert_eq!(observation.end_frame - observation.start_frame, 15);
    assert_eq!(observation.ranges.len(), 2);
    assert_eq!(
        observation.ranges[0].end_runtime_inclusive + 1,
        scratch_runtime
    );
    assert_eq!(observation.ranges[1].start_runtime, scratch_runtime + 0x60);
    assert_eq!(
        observation.ranges[1].end_runtime_inclusive + 1,
        QUEST_PAGE_CARRIER_END_RUNTIME
    );
}

#[test]
fn 보호범위_hit와_빈_실행구간은_거부한다() {
    let scratch_runtime = QUEST_PAGE_CARRIER_START_RUNTIME + 0x800;
    let scratch_bytes = 0x60;
    let hit_error = verify_write_observation(
        &observation_json(scratch_runtime, scratch_bytes, 1),
        scratch_runtime,
        scratch_bytes,
    )
    .expect_err("보호 범위 write hit를 거부해야 함");
    assert!(hit_error.to_string().contains("보호 write 범위 불일치"));

    let empty = String::from_utf8(observation_json(scratch_runtime, scratch_bytes, 0))
        .expect("UTF-8 fixture")
        .replace("\"end_frame\":25", "\"end_frame\":10");
    let frame_error = verify_write_observation(empty.as_bytes(), scratch_runtime, scratch_bytes)
        .expect_err("빈 frame 구간을 거부해야 함");
    assert!(frame_error.to_string().contains("frame 범위 불일치"));
}

#[test]
fn descriptor_event는_typed_entry와_object_page_marker를_함께_요구한다() {
    let object_runtime = 0x800C_1000u32;
    let marker_runtime =
        object_runtime + u32::try_from(QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET).unwrap();
    let mut main_ram = vec![0u8; PSX_MAIN_RAM_LEN];
    main_ram[usize::try_from(marker_runtime - 0x8000_0000).unwrap()] = 33;
    let state_raw = state_with_main_ram(&main_ram);
    let state = MednafenPsxState::parse(&state_raw).expect("PSX state");
    let entry_runtime = 0x8011_2000u32;
    let events = format!(
        r#"{{"dropped":0,"events":[{{"pc":{entry_runtime},"registers":{{"s1":{object_runtime}}}}}]}}"#
    );
    let evidence = verify_descriptor_event(events.as_bytes(), entry_runtime, 33, &state)
        .expect("typed descriptor event");
    assert_eq!(evidence.marker_runtime, marker_runtime);
    assert_eq!(evidence.marker, 33);

    let wrong_entry = verify_descriptor_event(events.as_bytes(), entry_runtime + 4, 33, &state)
        .expect_err("다른 body 위치는 descriptor entry가 아님");
    assert!(wrong_entry.to_string().contains("descriptor entry 불일치"));

    let wrong_page = verify_descriptor_event(events.as_bytes(), entry_runtime, 34, &state)
        .expect_err("다른 object page marker를 거부해야 함");
    assert!(wrong_page.to_string().contains("page marker 불일치"));
}
