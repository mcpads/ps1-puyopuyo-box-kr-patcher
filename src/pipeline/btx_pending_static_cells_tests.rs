use super::*;

#[test]
fn setup_record는_resource와_overlay_pointer를_검증한다() {
    let mut raw = vec![0u8; 0x80];
    raw[0x10..0x14].copy_from_slice(&SETUP_PREFIX);
    raw[0x14..0x16].copy_from_slice(&7u16.to_le_bytes());
    raw[0x1C..0x20].copy_from_slice(&0x8010_0040u32.to_le_bytes());
    raw[0x44..0x46].copy_from_slice(&256u16.to_le_bytes());
    raw[0x46..0x48].copy_from_slice(&176u16.to_le_bytes());

    let rows = scan_setup_records(
        "TEST.SEQ",
        &raw,
        SetupScanContext {
            initialized_len: raw.len(),
            load_base: 0x8010_0000,
            resource_id: 7,
            review_width: 256,
            review_height: 176,
            format: PixelFormat::Indexed4,
            descriptor_plain_bytes: 22_528,
        },
    )
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].record_runtime, 0x8010_0010);
    assert_eq!(rows[0].table_offset, 0x40);
    assert!(rows[0].review_layout_matches);
    assert!(rows[0].descriptor_plain_layout_matches);
}

#[test]
fn descriptor_plain_layout은_pixel_format별_바이트수를_사용한다() {
    let mut raw = vec![0u8; 0x80];
    raw[0x10..0x14].copy_from_slice(&SETUP_PREFIX);
    raw[0x14..0x16].copy_from_slice(&7u16.to_le_bytes());
    raw[0x1C..0x20].copy_from_slice(&0x8010_0040u32.to_le_bytes());
    raw[0x44..0x46].copy_from_slice(&64u16.to_le_bytes());
    raw[0x46..0x48].copy_from_slice(&32u16.to_le_bytes());

    let indexed8 = scan_setup_records(
        "TEST.SEQ",
        &raw,
        SetupScanContext {
            initialized_len: raw.len(),
            load_base: 0x8010_0000,
            resource_id: 7,
            review_width: 64,
            review_height: 32,
            format: PixelFormat::Indexed8,
            descriptor_plain_bytes: 2_048,
        },
    )
    .unwrap();
    let indexed4 = scan_setup_records(
        "TEST.SEQ",
        &raw,
        SetupScanContext {
            initialized_len: raw.len(),
            load_base: 0x8010_0000,
            resource_id: 7,
            review_width: 64,
            review_height: 32,
            format: PixelFormat::Indexed4,
            descriptor_plain_bytes: 2_048,
        },
    )
    .unwrap();

    assert!(indexed8[0].descriptor_plain_layout_matches);
    assert!(!indexed4[0].descriptor_plain_layout_matches);
}

#[test]
fn 다른_resource와_비정렬_record는_제외한다() {
    let mut raw = vec![0u8; 0x80];
    raw[0x11..0x15].copy_from_slice(&SETUP_PREFIX);
    raw[0x15..0x17].copy_from_slice(&7u16.to_le_bytes());
    raw[0x1D..0x21].copy_from_slice(&0x8010_0040u32.to_le_bytes());
    raw[0x20..0x24].copy_from_slice(&SETUP_PREFIX);
    raw[0x24..0x26].copy_from_slice(&8u16.to_le_bytes());
    raw[0x2C..0x30].copy_from_slice(&0x8010_0040u32.to_le_bytes());

    assert!(scan_setup_records(
        "TEST.SEQ",
        &raw,
        SetupScanContext {
            initialized_len: raw.len(),
            load_base: 0x8010_0000,
            resource_id: 7,
            review_width: 256,
            review_height: 176,
            format: PixelFormat::Indexed4,
            descriptor_plain_bytes: 22_528,
        }
    )
    .unwrap()
    .is_empty());
}

#[test]
fn overlay_밖_pointer와_reserved를_거부한다() {
    let mut outside = vec![0u8; 0x40];
    outside[0x10..0x14].copy_from_slice(&SETUP_PREFIX);
    outside[0x14..0x16].copy_from_slice(&7u16.to_le_bytes());
    outside[0x1C..0x20].copy_from_slice(&0x8010_1000u32.to_le_bytes());
    assert!(scan_setup_records(
        "TEST.SEQ",
        &outside,
        SetupScanContext {
            initialized_len: outside.len(),
            load_base: 0x8010_0000,
            resource_id: 7,
            review_width: 256,
            review_height: 176,
            format: PixelFormat::Indexed4,
            descriptor_plain_bytes: 22_528,
        },
    )
    .is_err());

    let mut reserved = vec![0u8; 0x80];
    reserved[0x10..0x14].copy_from_slice(&SETUP_PREFIX);
    reserved[0x14..0x16].copy_from_slice(&7u16.to_le_bytes());
    reserved[0x18..0x1C].copy_from_slice(&1u32.to_le_bytes());
    reserved[0x1C..0x20].copy_from_slice(&0x8010_0040u32.to_le_bytes());
    assert!(scan_setup_records(
        "TEST.SEQ",
        &reserved,
        SetupScanContext {
            initialized_len: reserved.len(),
            load_base: 0x8010_0000,
            resource_id: 7,
            review_width: 256,
            review_height: 176,
            format: PixelFormat::Indexed4,
            descriptor_plain_bytes: 22_528,
        },
    )
    .is_err());
}

#[test]
fn 다음_table_경계까지_cell과_tail후보를_가역적으로_보존한다() {
    let mut raw = vec![0u8; 0x60];
    raw[0x24..0x26].copy_from_slice(&32u16.to_le_bytes());
    raw[0x26..0x28].copy_from_slice(&16u16.to_le_bytes());
    raw[0x2C..0x2E].copy_from_slice(&0u16.to_le_bytes());
    raw[0x2E..0x30].copy_from_slice(&0u16.to_le_bytes());
    raw[0x30..0x32].copy_from_slice(&16u16.to_le_bytes());
    raw[0x32..0x34].copy_from_slice(&16u16.to_le_bytes());
    raw[0x38..0x3A].copy_from_slice(&24u16.to_le_bytes());
    raw[0x3A..0x3C].copy_from_slice(&0u16.to_le_bytes());
    raw[0x3C..0x3E].copy_from_slice(&16u16.to_le_bytes());
    raw[0x3E..0x40].copy_from_slice(&16u16.to_le_bytes());

    let span = infer_table_span(&raw, 0x20, 0x44, 32, 16).unwrap();

    assert_eq!(span.records, 2);
    assert_eq!(span.padding_bytes, 0);
    assert_eq!(span.valid_cells, 1);
    assert_eq!(span.empty_cells, 0);
    assert_eq!(span.outside_cells, 1);
    assert_eq!(span.cells[0].index, 0);
    assert!(!span.cells[0].outside_atlas);
    assert_eq!(span.cells[1].index, 1);
    assert!(span.cells[1].outside_atlas);
    assert_eq!(
        render_cells(&span.cells),
        "0:0,0,16,16,0,0,valid;1:24,0,16,16,0,0,outside"
    );
}

#[test]
fn full_span에_atlas_밖_record가_섞이면_선두_cell_prefix도_보존한다() {
    let mut raw = vec![0u8; 0xA0];
    raw[0x10..0x14].copy_from_slice(&SETUP_PREFIX);
    raw[0x14..0x16].copy_from_slice(&7u16.to_le_bytes());
    raw[0x1C..0x20].copy_from_slice(&0x8010_0040u32.to_le_bytes());
    raw[0x20..0x24].copy_from_slice(&SETUP_PREFIX);
    raw[0x24..0x26].copy_from_slice(&8u16.to_le_bytes());
    raw[0x2C..0x30].copy_from_slice(&0x8010_0088u32.to_le_bytes());
    raw[0x8C..0x8E].copy_from_slice(&32u16.to_le_bytes());
    raw[0x8E..0x90].copy_from_slice(&16u16.to_le_bytes());

    raw[0x44..0x46].copy_from_slice(&32u16.to_le_bytes());
    raw[0x46..0x48].copy_from_slice(&16u16.to_le_bytes());
    raw[0x50..0x52].copy_from_slice(&16u16.to_le_bytes());
    raw[0x52..0x54].copy_from_slice(&16u16.to_le_bytes());
    raw[0x5C..0x5E].copy_from_slice(&16u16.to_le_bytes());
    raw[0x5E..0x60].copy_from_slice(&16u16.to_le_bytes());
    raw[0x64..0x66].copy_from_slice(&0u16.to_le_bytes());
    raw[0x66..0x68].copy_from_slice(&0u16.to_le_bytes());
    raw[0x68..0x6A].copy_from_slice(&0u16.to_le_bytes());
    raw[0x6A..0x6C].copy_from_slice(&0x8000u16.to_le_bytes());
    raw[0x6C..0x6E].copy_from_slice(&0x8000u16.to_le_bytes());
    raw[0x6E..0x70].copy_from_slice(&0x8020u16.to_le_bytes());
    raw[0x70..0x72].copy_from_slice(&24u16.to_le_bytes());
    raw[0x74..0x76].copy_from_slice(&16u16.to_le_bytes());
    raw[0x76..0x78].copy_from_slice(&16u16.to_le_bytes());

    let setups = scan_setup_records(
        "TEST.SEQ",
        &raw,
        SetupScanContext {
            initialized_len: raw.len(),
            load_base: 0x8010_0000,
            resource_id: 7,
            review_width: 32,
            review_height: 16,
            format: PixelFormat::Indexed4,
            descriptor_plain_bytes: 256,
        },
    )
    .unwrap();

    assert_eq!(setups.len(), 1);
    assert_eq!(setups[0].outside_cells, Some(1));
    let prefix = setups[0].leading_cell_prefix.as_ref().unwrap();
    assert_eq!(prefix.valid_cells.len(), 2);
    assert_eq!(prefix.boundary_record.index, 2);
    assert!(prefix.boundary_record.empty);
    assert_eq!(prefix.protected_tail_bytes, 0x18);
}

#[test]
fn nonzero_tail_앞의_empty_boundary_cell_prefix만_분리한다() {
    let mut raw = vec![0xA5; 0x60];
    raw[0x20..0x2C].fill(0);
    raw[0x24..0x26].copy_from_slice(&32u16.to_le_bytes());
    raw[0x26..0x28].copy_from_slice(&16u16.to_le_bytes());
    raw[0x2C..0x2E].copy_from_slice(&0u16.to_le_bytes());
    raw[0x2E..0x30].copy_from_slice(&0u16.to_le_bytes());
    raw[0x30..0x32].copy_from_slice(&16u16.to_le_bytes());
    raw[0x32..0x34].copy_from_slice(&16u16.to_le_bytes());
    raw[0x38..0x3A].copy_from_slice(&16u16.to_le_bytes());
    raw[0x3A..0x3C].copy_from_slice(&16u16.to_le_bytes());
    raw[0x3C..0x3E].copy_from_slice(&16u16.to_le_bytes());
    raw[0x3E..0x40].copy_from_slice(&0u16.to_le_bytes());

    assert!(infer_table_span(&raw, 0x20, 0x54, 32, 16).is_none());
    let prefix = infer_leading_cell_prefix(&raw, 0x20, 0x54, 32, 16).unwrap();

    assert_eq!(prefix.valid_cells.len(), 1);
    assert_eq!(prefix.valid_cells[0].index, 0);
    assert_eq!(prefix.boundary_record.index, 1);
    assert!(prefix.boundary_record.empty);
    assert_eq!(prefix.protected_tail_bytes, 0x10);
}

#[test]
fn 마지막_cell_table은_다음_overlay참조로_경계를_닫는다() {
    let mut raw = vec![0u8; 0x80];
    raw[0x10..0x14].copy_from_slice(&SETUP_PREFIX);
    raw[0x14..0x16].copy_from_slice(&7u16.to_le_bytes());
    raw[0x1C..0x20].copy_from_slice(&0x8010_0040u32.to_le_bytes());
    raw[0x20..0x24].copy_from_slice(&0x8010_0060u32.to_le_bytes());
    raw[0x44..0x46].copy_from_slice(&32u16.to_le_bytes());
    raw[0x46..0x48].copy_from_slice(&16u16.to_le_bytes());
    raw[0x50..0x52].copy_from_slice(&32u16.to_le_bytes());
    raw[0x52..0x54].copy_from_slice(&16u16.to_le_bytes());

    let setups = scan_setup_records(
        "TEST.SEQ",
        &raw,
        SetupScanContext {
            initialized_len: raw.len(),
            load_base: 0x8010_0000,
            resource_id: 7,
            review_width: 32,
            review_height: 16,
            format: PixelFormat::Indexed4,
            descriptor_plain_bytes: 256,
        },
    )
    .unwrap();

    assert_eq!(setups.len(), 1);
    assert_eq!(setups[0].next_referenced_table_offset, None);
    assert_eq!(setups[0].next_referenced_overlay_offset, Some(0x60));
    assert_eq!(setups[0].inferred_cell_records, None);
    let span = setups[0].next_overlay_cell_span.as_ref().unwrap();
    assert_eq!(span.records, 1);
    assert_eq!(span.padding_bytes, 8);
    assert_eq!(span.valid_cells, 1);
}
