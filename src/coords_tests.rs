use super::*;

#[test]
fn runtime_asset_map은_runtime과_파일_offset을_서로_바꾼다() {
    let map = RuntimeAssetMap::new(0x8019_9A70, 0x5764).expect("TOWN00 적재 사상");
    assert_eq!(map.file_offset(0x8019_9BB8), Some(0x148));
    assert_eq!(map.runtime_addr(0x148), Some(0x8019_9BB8));
    assert!(map.contains(0x8019_9A70));
    assert!(!map.contains(0x8019_F1D4));
}

#[test]
fn runtime_asset_map은_overflow와_끝주소를_거부한다() {
    assert_eq!(RuntimeAssetMap::new(u32::MAX, 2), None);
    let map = RuntimeAssetMap::new(0x8000_0000, 4).expect("작은 적재 사상");
    assert_eq!(map.file_offset(0x7FFF_FFFF), None);
    assert_eq!(map.file_offset(0x8000_0004), None);
    assert_eq!(map.runtime_addr(4), None);
}

#[test]
fn logical_offset_zero_lands_on_first_sector_user_data() {
    let f = LogicalFile::new(100, 4096);
    assert_eq!(f.image_offset(0), Some(100 * 2352 + 24));
}

#[test]
fn sector_crossing_skips_the_304_byte_gap() {
    let f = LogicalFile::new(100, 4096);
    // 논리적으로 인접한 2047 → 2048이 이미지에서는 인접하지 않는다.
    let last_of_first = f.image_offset(2047).unwrap();
    let first_of_second = f.image_offset(2048).unwrap();
    assert_eq!(first_of_second - last_of_first, 1 + 304);
    assert_eq!(first_of_second, 101 * 2352 + 24);
}

#[test]
fn offset_past_end_is_rejected() {
    let f = LogicalFile::new(100, 4096);
    assert_eq!(f.image_offset(4096), None);
    assert!(f.image_offset(4095).is_some());
}

#[test]
fn sector_count_rounds_up() {
    assert_eq!(LogicalFile::new(0, 2048).sector_count(), 1);
    assert_eq!(LogicalFile::new(0, 2049).sector_count(), 2);
    assert_eq!(LogicalFile::new(0, 68).sector_count(), 1);
}

#[test]
fn touched_sectors_covers_the_span() {
    let f = LogicalFile::new(100, 8192);
    assert_eq!(f.touched_sectors(0, 1), Some((100, 100)));
    assert_eq!(f.touched_sectors(2047, 2), Some((100, 101)));
    assert_eq!(f.touched_sectors(0, 8192), Some((100, 103)));
    // 범위를 넘기면 거부한다.
    assert_eq!(f.touched_sectors(0, 8193), None);
    assert_eq!(f.touched_sectors(0, 0), None);
}

#[test]
fn quest_entry_point_maps_into_its_own_file() {
    let quest = exe_map("QUEST.EXE").expect("QUEST 사상");
    let off = quest.file_offset(quest.entry_pc).expect("엔트리는 범위 안");
    assert_eq!(off, PSX_EXE_HEADER_LEN + (0x800A_A6E8 - 0x8001_0000));
    assert!(off < quest.file.len as u64);
    assert!(quest.image_offset(quest.entry_pc).is_some());
}

#[test]
fn every_declared_exe_entry_point_is_inside_its_text() {
    for m in EXE_MAPS.iter() {
        assert!(
            m.contains(m.entry_pc),
            "{}: 엔트리 0x{:08X}가 적재 범위 밖",
            m.name,
            m.entry_pc
        );
        let off = m.file_offset(m.entry_pc).expect("엔트리 오프셋");
        assert!(
            off < m.file.len as u64,
            "{}: 엔트리 오프셋 {off}가 파일 크기 {} 밖",
            m.name,
            m.file.len
        );
    }
}

#[test]
fn address_outside_load_range_is_rejected() {
    let quest = exe_map("QUEST.EXE").unwrap();
    assert_eq!(quest.file_offset(0x8000_0000), None);
    assert_eq!(quest.file_offset(0x80FF_FFFF), None);
}

#[test]
fn subgames_share_a_load_address_but_not_a_file() {
    let p1 = exe_map("PUYO1.EXE").unwrap();
    let p2 = exe_map("PUYO2.EXE").unwrap();
    assert_eq!(p1.load_addr, p2.load_addr);
    // 같은 런타임 주소라도 이미지 위치는 완전히 다르다.
    let a = p1.image_offset(0x8001_0000).unwrap();
    let b = p2.image_offset(0x8001_0000).unwrap();
    assert_ne!(a, b);
}

#[test]
fn runtime_addr_is_the_inverse_of_file_offset() {
    let quest = exe_map("QUEST.EXE").unwrap();
    for addr in [0x8001_0000u32, 0x8005_0000, quest.entry_pc] {
        let off = quest.file_offset(addr).unwrap();
        assert_eq!(quest.runtime_addr(off), Some(addr));
    }
}

#[test]
fn text_offset은_psx_exe_header를_제외한_slice좌표다() {
    let quest = exe_map("QUEST.EXE").unwrap();
    assert_eq!(quest.text_offset(quest.load_addr), Some(0));
    assert_eq!(quest.text_offset(0x800B_2998), Some(0xA_2998));
    assert_eq!(quest.text_runtime_addr(0xA_2998), Some(0x800B_2998));
    assert_eq!(quest.text_offset(quest.load_addr - 1), None);
    assert_eq!(quest.text_offset(quest.load_addr + quest.text_len), None);
    assert_eq!(quest.text_runtime_addr(quest.text_len as usize), None);
}

#[test]
fn 확장_text좌표는_원본사상을_바꾸지_않고_post_bss를_계산한다() {
    let quest = exe_map("QUEST.EXE").unwrap();
    let extended_len = 0x0010_9000;
    let runtime = 0x8011_8500;
    assert_eq!(quest.text_offset(runtime), None);
    assert_eq!(
        quest.loaded_text_offset(runtime, extended_len),
        Some(0x0010_8500)
    );
    assert_eq!(
        quest.loaded_file_offset(runtime, extended_len),
        Some(PSX_EXE_HEADER_LEN + 0x0010_8500)
    );
    assert_eq!(
        quest.loaded_text_offset(quest.load_addr + extended_len, extended_len),
        None
    );
}

#[test]
fn kseg1_normalizes_to_the_same_address_as_kseg0() {
    assert_eq!(normalize_kseg(0xA001_0000), 0x8001_0000);
    assert_eq!(normalize_kseg(0x8001_0000), 0x8001_0000);
    // KUSEG는 건드리지 않는다.
    assert_eq!(normalize_kseg(0x0001_0000), 0x0001_0000);
}

#[test]
fn psx_main_ram_offset은_kseg_mirror와_끝경계를_검증한다() {
    assert_eq!(psx_main_ram_offset(0x8001_0000, 4), Some(0x1_0000));
    assert_eq!(psx_main_ram_offset(0xA001_0000, 4), Some(0x1_0000));
    assert_eq!(
        psx_main_ram_offset(0x801F_FFFC, 4),
        Some(PSX_MAIN_RAM_LEN - 4)
    );
    assert_eq!(psx_main_ram_offset(0x801F_FFFC, 5), None);
    assert_eq!(psx_main_ram_offset(0x7FFF_FFFF, 1), None);
}
