use super::*;
use crate::coords::LogicalFile;
use crate::isa::encode;

fn fixture_map() -> ExeMap {
    ExeMap {
        name: GOCHA1_EXE,
        load_addr: 0x8001_0000,
        text_len: 0x0009_0000,
        entry_pc: 0x8008_E870,
        file: LogicalFile::new(24, 591_872),
    }
}

fn write_instruction(map: &ExeMap, text: &mut [u8], runtime: u32, kind: K) {
    let offset = map.text_offset(runtime).expect("fixture runtime");
    text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
}

fn fixture_source() -> (ExeMap, Vec<u8>) {
    let map = fixture_map();
    let mut text = vec![0u8; map.text_len as usize];
    for (runtime, kind) in result_name_entry_source_instructions() {
        write_instruction(&map, &mut text, runtime, kind);
    }
    (map, text)
}

#[test]
fn 세_글자_확정과_취소가_같은_이름_위치를_갱신한다() {
    let (map, text) = fixture_source();
    let entry = verify_gocha1_result_name_entry(&map, &text).unwrap();

    assert_eq!(entry.name_slots, 3);
    assert_eq!(entry.confirm_event_mask, 0x0020);
    assert_eq!(entry.erase_event_mask, 0x0040);
    assert_eq!(entry.name_characters_field_offset, 0x0050);
    assert_eq!(entry.position_field_offset, 0x00D4);
    assert_eq!(entry.state_field_offset, 0x00D0);
    assert_eq!(entry.confirm_position_store_runtime, 0x8003_0C8C);
    assert_eq!(entry.state_increment_store_runtime, 0x8003_0CA8);
    assert_eq!(entry.erase_position_store_runtime, 0x8003_0CE4);
}

#[test]
fn 세번째_확정의_상태_전이_명령이_바뀌면_승인하지_않는다() {
    let (map, mut text) = fixture_source();
    write_instruction(
        &map,
        &mut text,
        STATE_INCREMENT_STORE_RUNTIME,
        K::Sh {
            rt: Reg::V0,
            base: Reg::S1,
            offset: 0x00D2,
        },
    );

    let error = verify_gocha1_result_name_entry(&map, &text).unwrap_err();
    assert!(error.to_string().contains("0x80030CA8"));
}
