use super::*;

const HEADER_LEN: usize = 32;
const MAIN_RAM_LEN: usize = 2 * 1024 * 1024;

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

fn state_fixture(object_runtime: u32, stream_offset: usize) -> Vec<u8> {
    let mut ram = vec![0u8; MAIN_RAM_LEN];
    let callback_offset =
        usize::try_from((object_runtime + QUEST_SEQ_INTERPRETER_CALLBACK_FIELD_OFFSET) & 0x1F_FFFF)
            .expect("RAM offset");
    ram[callback_offset..callback_offset + 4]
        .copy_from_slice(&QUEST_SEQ_INTERPRETER_RUNTIME.to_le_bytes());
    let stream_field_offset = usize::try_from(
        (object_runtime + QUEST_SEQ_STREAM_POINTER_STATE_OFFSET as u32) & 0x1F_FFFF,
    )
    .expect("RAM offset");
    let stream_runtime =
        TOWN00_SEQ_RUNTIME_BASE + u32::try_from(stream_offset).expect("작은 offset");
    ram[stream_field_offset..stream_field_offset + 4]
        .copy_from_slice(&stream_runtime.to_le_bytes());

    let main = variable("MainRAM.data8", &ram);
    let mut raw = vec![0u8; HEADER_LEN];
    raw[..8].copy_from_slice(b"MDFNSVST");
    raw[16..20].copy_from_slice(&0x0010_3201u32.to_le_bytes());
    raw.extend_from_slice(&section("MAIN", &main));
    let len = u32::try_from(raw.len()).expect("작은 fixture");
    raw[20..24].copy_from_slice(&len.to_le_bytes());
    raw
}

fn spec(start: usize, end: usize, batch: Option<&str>) -> QuestFontSceneSpec {
    QuestFontSceneSpec {
        record_offset: start,
        end_offset: end,
        stream_id: format!("QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x{start:04X}"),
        scene_id: batch.map(|_| "town00-test-001".to_string()),
        batch_id: batch.map(str::to_string),
    }
}

#[test]
fn 현재_pointer를_소유_stream과_batch에_결합한다() {
    let raw = state_fixture(0x800C_4000, 0x1210);
    let state = MednafenPsxState::parse(&raw).unwrap();
    let census = inspect_with_specs(&state, &[spec(0x1200, 0x1240, Some("batch-a"))]).unwrap();
    assert_eq!(census.candidates.len(), 1);
    assert_eq!(census.canonical_stream_candidates, 1);
    assert_eq!(census.batch_bound_candidates, 1);
    assert_eq!(census.unique_bound_batch_id(), Some("batch-a"));
    assert_eq!(census.candidates[0].record_offset, Some(0x1200));
}

#[test]
fn canonical_밖_pointer는_batch로_승격하지_않는다() {
    let raw = state_fixture(0x800C_5000, 0x1300);
    let state = MednafenPsxState::parse(&raw).unwrap();
    let census = inspect_with_specs(&state, &[spec(0x1200, 0x1240, Some("batch-a"))]).unwrap();
    assert_eq!(census.canonical_stream_candidates, 0);
    assert_eq!(census.batch_bound_candidates, 0);
    assert_eq!(census.unique_bound_batch_id(), None);
}

#[test]
fn manifest_미결합_stream은_page_binding이_아니다() {
    let raw = state_fixture(0x800C_6000, 0x1210);
    let state = MednafenPsxState::parse(&raw).unwrap();
    let census = inspect_with_specs(&state, &[spec(0x1200, 0x1240, None)]).unwrap();
    assert_eq!(census.canonical_stream_candidates, 1);
    assert_eq!(census.batch_bound_candidates, 0);
    assert_eq!(census.unique_bound_batch_id(), None);
}
