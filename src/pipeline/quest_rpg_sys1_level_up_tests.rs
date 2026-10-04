use super::*;
use crate::coords::LogicalFile;
use crate::isa::encode;

fn fixture_map() -> ExeMap {
    ExeMap {
        name: QUEST_EXE,
        load_addr: 0x8001_0000,
        text_len: 0x000A_E800,
        entry_pc: 0x800A_A6E8,
        file: LogicalFile::new(33_538, 716_800),
    }
}

fn write_instruction(map: &ExeMap, text: &mut [u8], runtime: u32, kind: K) {
    let offset = map.text_offset(runtime).expect("fixture runtime");
    text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
}

fn fixture_source() -> (ExeMap, Vec<u8>) {
    let map = fixture_map();
    let mut text = vec![0u8; map.text_len as usize];
    for (runtime, kind) in level_up_source_instructions() {
        write_instruction(&map, &mut text, runtime, kind);
    }
    (map, text)
}

#[test]
fn runtime_initialized_cell을_원본에_요구하지_않고_selector의_다음_record를_고른다() {
    let (map, text) = fixture_source();
    let route = verify_quest_rpg_sys1_level_up(&map, &text).unwrap();

    assert_eq!(route.resource_id, 2);
    assert_eq!(route.level_up_cell_selector, 95);
    assert_eq!(route.level_up_cell_record_index, 96);
    assert_eq!(route.cell_record_bytes, 12);
    assert_eq!(route.short_renderer_runtime, 0x8002_384C);
    assert_eq!(route.level_up_record_read_runtime, 0x8002_38D4);
}

#[test]
fn 다른_exe와_다른_resource_writer를_승인하지_않는다() {
    let (mut map, text) = fixture_source();
    map.name = "PUYO1.EXE";
    assert!(verify_quest_rpg_sys1_level_up(&map, &text)
        .unwrap_err()
        .to_string()
        .contains("다른 EXE"));

    let (map, mut text) = fixture_source();
    write_instruction(
        &map,
        &mut text,
        0x8007_68F4,
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 3,
        },
    );
    assert!(verify_quest_rpg_sys1_level_up(&map, &text)
        .unwrap_err()
        .to_string()
        .contains("0x800768F4"));
}

#[test]
fn renderer의_다음_record_선택이_바뀌면_승인하지_않는다() {
    let (map, mut text) = fixture_source();
    write_instruction(
        &map,
        &mut text,
        0x8002_38BC,
        K::Addiu {
            rt: Reg::S3,
            rs: Reg::S6,
            imm: 0,
        },
    );
    assert!(verify_quest_rpg_sys1_level_up(&map, &text)
        .unwrap_err()
        .to_string()
        .contains("0x800238BC"));
}
