use super::*;

#[test]
fn runtime_texture의_좌우_bank에_256개씩_배치한다() {
    assert_eq!(
        (DEMO_FONT_LAYOUT.width, DEMO_FONT_LAYOUT.height),
        (512, 256)
    );
    let cases = [
        (0, (0, 0)),
        (15, (240, 0)),
        (16, (0, 16)),
        (255, (240, 240)),
        (256, (256, 0)),
        (511, (496, 240)),
    ];
    for (index, (x, y)) in cases {
        let slot = DemoFontSlot::for_index(index).unwrap();
        assert_eq!((slot.x, slot.y, slot.width, slot.height), (x, y, 16, 16));
        assert!(slot.contains_pixel(x, y));
        assert!(slot.contains_pixel(x + 15, y + 15));
        assert!(slot.x + slot.width <= DEMO_FONT_LAYOUT.width);
        assert!(slot.y + slot.height <= DEMO_FONT_LAYOUT.height);
    }
    assert_eq!(
        DemoFontSlot::for_index(DEMO_FONT_SLOT_COUNT),
        Err(DemoFontError::SlotIndexOutOfRange(DEMO_FONT_SLOT_COUNT))
    );
}

#[test]
fn layout_전체를_4bpp로_바이트_동일하게_왕복한다() {
    let mut raw = vec![0u8; DEMO_FONT_LAYOUT.byte_len().unwrap()];
    for (index, byte) in raw.iter_mut().enumerate() {
        *byte = ((index % 16) as u8) | (((index / 16) % 16) as u8) << 4;
    }
    let atlas = DemoFontAtlas::parse(&raw).unwrap();

    assert_eq!(atlas.slot_pixels(0).unwrap().len(), 16 * 16);
    let encoded = encode(&atlas.pixels, DEMO_FONT_LAYOUT).unwrap();
    assert_eq!(encoded, raw);
}

#[test]
fn 길이와_지원_원본_digest가_다르면_실패한다() {
    assert!(matches!(
        DemoFontAtlas::parse(&[]),
        Err(DemoFontError::Btx(BtxError::DataLengthMismatch { .. }))
    ));
    let raw = vec![0u8; DEMO_FONT_LAYOUT.byte_len().unwrap()];
    assert!(matches!(
        verify_box_demo_font(&raw),
        Err(DemoFontError::DigestMismatch { .. })
    ));
}
