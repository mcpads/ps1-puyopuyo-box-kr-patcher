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
    for (runtime, kind) in field_layout_source_instructions()
        .into_iter()
        .chain(pending_garbage_source_instructions())
    {
        write_instruction(&map, &mut text, runtime, kind);
    }
    (map, text)
}

#[test]
fn 검증된_경로는_상대_대기량과_셀_주소를_같은_좌표계로_계산한다() {
    let (map, text) = fixture_source();
    let route = verify_gocha1_garbage_route(&map, &text).unwrap();

    assert_eq!(route.player_field_runtime, 0x800D_1B20);
    assert_eq!(route.opponent_field_runtime, 0x800D_2F38);
    assert_eq!(route.player_pending_garbage_runtime, 0x800D_2E64);
    assert_eq!(route.opponent_pending_garbage_runtime, 0x800D_427C);
    assert_eq!(route.opponent_pending_garbage_ram_offset, 0x000D_427C);
    assert_eq!(route.garbage_drop_batch_limit, 30);
    assert_eq!(route.cell_type_runtime(1, 1, 12), Some(0x800D_3D18));
    assert_eq!(route.cell_type_runtime(1, 1, 13), Some(0x800D_3E38));
    assert_eq!(route.cell_type_runtime(4, 1, 12), None);
    assert_eq!(route.cell_type_runtime(1, 8, 12), None);
    assert_eq!(route.cell_type_runtime(1, 1, 16), None);
}

#[test]
fn 대기량_소비_명령이_바뀌면_경로를_승인하지_않는다() {
    let (map, mut text) = fixture_source();
    write_instruction(
        &map,
        &mut text,
        0x8003_73EC,
        K::Lw {
            rt: Reg::V0,
            base: Reg::S1,
            offset: 0x1348,
        },
    );

    let error = verify_gocha1_garbage_route(&map, &text).unwrap_err();
    assert!(error.to_string().contains("0x800373EC"));
}
