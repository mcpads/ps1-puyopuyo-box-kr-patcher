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
    for (runtime, kind) in field_layout_source_instructions()
        .into_iter()
        .chain(garbage_consumer_source_instructions())
    {
        write_instruction(&map, &mut text, runtime, kind);
    }
    (map, text)
}

#[test]
fn 검증된_경로는_quest의_u32_상대_대기량과_셀_주소를_계산한다() {
    let (map, text) = fixture_source();
    let route = verify_quest_garbage_route(&map, &text).unwrap();

    assert_eq!(route.field_objects_runtime, 0x800F_0D60);
    assert_eq!(route.field_object_stride, 0x13A0);
    assert_eq!(route.allocated_field_objects, 4);
    assert_eq!(route.controlled_battle_fields, 2);
    assert_eq!(route.player_field_runtime, 0x800F_0D60);
    assert_eq!(route.opponent_field_runtime, 0x800F_2100);
    assert_eq!(route.player_pending_garbage_runtime, 0x800F_20BC);
    assert_eq!(route.opponent_pending_garbage_runtime, 0x800F_345C);
    assert_eq!(route.opponent_pending_garbage_ram_offset, 0x000F_345C);
    assert_eq!(route.pending_garbage_bytes, 4);
    assert_eq!(route.garbage_drop_batch_limit, 30);
    assert_eq!(route.cell_type_runtime(1, 1, 13), Some(0x800F_3010));
    assert_eq!(route.cell_type_runtime(4, 1, 13), None);
    assert_eq!(route.cell_type_runtime(1, 8, 13), None);
    assert_eq!(route.cell_type_runtime(1, 1, 16), None);
}

#[test]
fn 다른_exe와_u16_대기량_store는_quest_경로로_승인하지_않는다() {
    let (mut map, text) = fixture_source();
    map.name = "PUYO1.EXE";
    assert!(verify_quest_garbage_route(&map, &text)
        .unwrap_err()
        .to_string()
        .contains("다른 EXE"));

    let (map, mut text) = fixture_source();
    write_instruction(
        &map,
        &mut text,
        0x8004_48EC,
        K::Sh {
            rt: Reg::V0,
            base: Reg::S3,
            offset: 0x135C,
        },
    );
    assert!(verify_quest_garbage_route(&map, &text)
        .unwrap_err()
        .to_string()
        .contains("0x800448EC"));
}

#[test]
fn callback_offset와_네_field_stride가_바뀌면_승인하지_않는다() {
    let (map, mut text) = fixture_source();
    write_instruction(
        &map,
        &mut text,
        0x8004_47E8,
        K::Lw {
            rt: Reg::S3,
            base: Reg::A0,
            offset: 0x00E0,
        },
    );
    assert!(verify_quest_garbage_route(&map, &text)
        .unwrap_err()
        .to_string()
        .contains("0x800447E8"));

    let (map, mut text) = fixture_source();
    write_instruction(
        &map,
        &mut text,
        0x8004_063C,
        K::Addiu {
            rt: Reg::A2,
            rs: Reg::A2,
            imm: 0x139C,
        },
    );
    assert!(verify_quest_garbage_route(&map, &text)
        .unwrap_err()
        .to_string()
        .contains("0x8004063C"));
}

#[test]
fn empty_type과_batch_상한이_바뀌면_승인하지_않는다() {
    let (map, mut text) = fixture_source();
    write_instruction(
        &map,
        &mut text,
        0x8004_4804,
        K::Addiu {
            rt: Reg::A2,
            rs: Reg::ZERO,
            imm: 0x000D,
        },
    );
    assert!(verify_quest_garbage_route(&map, &text)
        .unwrap_err()
        .to_string()
        .contains("0x80044804"));

    let (map, mut text) = fixture_source();
    write_instruction(
        &map,
        &mut text,
        0x8004_48DC,
        K::Addiu {
            rt: Reg::S7,
            rs: Reg::ZERO,
            imm: 0x001F,
        },
    );
    assert!(verify_quest_garbage_route(&map, &text)
        .unwrap_err()
        .to_string()
        .contains("0x800448DC"));
}
