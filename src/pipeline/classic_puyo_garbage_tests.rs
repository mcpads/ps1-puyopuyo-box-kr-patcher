use super::*;
use crate::coords::LogicalFile;
use crate::isa::encode;

fn fixture_map(spec: ClassicPuyoGarbageSpec) -> ExeMap {
    match spec.exe {
        PUYO1_EXE => ExeMap {
            name: PUYO1_EXE,
            load_addr: 0x8001_0000,
            text_len: 0x0008_6000,
            entry_pc: 0x8001_5274,
            file: LogicalFile::new(23_315, 550_912),
        },
        PUYO2_EXE => ExeMap {
            name: PUYO2_EXE,
            load_addr: 0x8001_0000,
            text_len: 0x0008_8000,
            entry_pc: 0x8001_4A3C,
            file: LogicalFile::new(28_257, 559_104),
        },
        _ => unreachable!("test spec"),
    }
}

fn write_instruction(map: &ExeMap, text: &mut [u8], runtime: u32, kind: K) {
    let offset = map.text_offset(runtime).expect("fixture runtime");
    text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
}

fn fixture_source(spec: ClassicPuyoGarbageSpec) -> (ExeMap, Vec<u8>) {
    let map = fixture_map(spec);
    let mut text = vec![0u8; map.text_len as usize];
    for (runtime, kind) in field_layout_source_instructions(spec)
        .into_iter()
        .chain(garbage_consumer_source_instructions(spec))
    {
        write_instruction(&map, &mut text, runtime, kind);
    }
    (map, text)
}

#[test]
fn 검증된_경로는_puyo1의_상대_대기량과_셀_주소를_계산한다() {
    let (map, text) = fixture_source(PUYO1_SPEC);
    let route = verify_puyo1_garbage_route(&map, &text).unwrap();

    assert_eq!(route.field_objects_runtime, 0x800C_9DB8);
    assert_eq!(route.field_object_stride, 0x139C);
    assert_eq!(route.player_field_runtime, 0x800C_9DB8);
    assert_eq!(route.opponent_field_runtime, 0x800C_B154);
    assert_eq!(route.player_pending_garbage_runtime, 0x800C_B114);
    assert_eq!(route.opponent_pending_garbage_runtime, 0x800C_C4B0);
    assert_eq!(route.opponent_pending_garbage_ram_offset, 0x000C_C4B0);
    assert_eq!(route.pending_garbage_bytes, 2);
    assert_eq!(route.garbage_drop_batch_limit, 30);
    assert_eq!(route.cell_type_runtime(1, 1, 13), Some(0x800C_C064));
    assert_eq!(route.cell_type_runtime(2, 1, 13), None);
    assert_eq!(route.cell_type_runtime(1, 8, 13), None);
    assert_eq!(route.cell_type_runtime(1, 1, 16), None);
}

#[test]
fn 검증된_경로는_puyo2의_상대_대기량과_셀_주소를_별도로_계산한다() {
    let (map, text) = fixture_source(PUYO2_SPEC);
    let route = verify_puyo2_garbage_route(&map, &text).unwrap();

    assert_eq!(route.exe, PUYO2_EXE);
    assert_eq!(route.field_objects_runtime, 0x800C_AB08);
    assert_eq!(route.field_object_stride, 0x139C);
    assert_eq!(route.player_field_runtime, 0x800C_AB08);
    assert_eq!(route.opponent_field_runtime, 0x800C_BEA4);
    assert_eq!(route.player_pending_garbage_runtime, 0x800C_BE64);
    assert_eq!(route.opponent_pending_garbage_runtime, 0x800C_D200);
    assert_eq!(route.opponent_pending_garbage_ram_offset, 0x000C_D200);
    assert_eq!(route.pending_garbage_bytes, 2);
    assert_eq!(route.garbage_drop_batch_limit, 30);
    assert_eq!(route.cell_type_runtime(1, 1, 13), Some(0x800C_CDB4));
}

#[test]
fn 대기량_감산_store가_바뀌면_puyo1_경로를_승인하지_않는다() {
    let (map, mut text) = fixture_source(PUYO1_SPEC);
    write_instruction(
        &map,
        &mut text,
        0x8004_9A84,
        K::Sh {
            rt: Reg::V0,
            base: Reg::S2,
            offset: 0x135E,
        },
    );

    let error = verify_puyo1_garbage_route(&map, &text).unwrap_err();
    assert!(error.to_string().contains("0x80049A84"));
}

#[test]
fn 열_탐색_상한은_마스킹한_byte를_비교해야_한다() {
    let (map, mut text) = fixture_source(PUYO1_SPEC);
    write_instruction(
        &map,
        &mut text,
        0x8004_99E0,
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::A2,
            imm: 0x000E,
        },
    );

    let error = verify_puyo1_garbage_route(&map, &text).unwrap_err();
    assert!(error.to_string().contains("0x800499E0"));
}

#[test]
fn 행_탐색_상한과_방해뿌요_type의_delay_slot을_보존한다() {
    let (map, mut text) = fixture_source(PUYO1_SPEC);
    write_instruction(
        &map,
        &mut text,
        0x8004_9A00,
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::S4,
            imm: 0x0007,
        },
    );

    let error = verify_puyo1_garbage_route(&map, &text).unwrap_err();
    assert!(error.to_string().contains("0x80049A00"));

    let (map, mut text) = fixture_source(PUYO1_SPEC);
    write_instruction(
        &map,
        &mut text,
        0x8004_9A28,
        K::Addiu {
            rt: Reg::T0,
            rs: Reg::ZERO,
            imm: 0x0006,
        },
    );

    let error = verify_puyo1_garbage_route(&map, &text).unwrap_err();
    assert!(error.to_string().contains("0x80049A28"));
}

#[test]
fn puyo1의_consumer_address를_puyo2에_이식하면_승인하지_않는다() {
    let (map, mut text) = fixture_source(PUYO2_SPEC);
    write_instruction(
        &map,
        &mut text,
        PUYO2_SPEC.incoming_garbage_consumer_runtime + 0x138,
        K::Sh {
            rt: Reg::V0,
            base: Reg::S2,
            offset: 0x135E,
        },
    );

    let error = verify_puyo2_garbage_route(&map, &text).unwrap_err();
    assert!(error.to_string().contains("0x800402BC"));
}
