use super::*;

#[test]
fn indexed4_palette_index를_모두_보존한_bottom_up_bmp를_만든다() {
    let bmp = render_btx_family_preview_bmp(
        &[0xF0, 0x21],
        Layout {
            width: 2,
            height: 2,
            format: PixelFormat::Indexed4,
        },
    )
    .unwrap();

    assert_eq!(&bmp[..2], b"BM");
    assert_eq!(u32::from_le_bytes(bmp[2..6].try_into().unwrap()), 70);
    assert_eq!(u32::from_le_bytes(bmp[10..14].try_into().unwrap()), 54);
    assert_eq!(i32::from_le_bytes(bmp[18..22].try_into().unwrap()), 2);
    assert_eq!(i32::from_le_bytes(bmp[22..26].try_into().unwrap()), 2);
    assert_eq!(u16::from_le_bytes(bmp[28..30].try_into().unwrap()), 24);
    assert_eq!(&bmp[54..62], &[17, 17, 17, 34, 34, 34, 0, 0]);
    assert_eq!(&bmp[62..70], &[0, 0, 0, 255, 255, 255, 0, 0]);
}

#[test]
fn layout과_원시_크기가_다르면_부분_preview를_반환하지_않는다() {
    let result = render_btx_family_preview_bmp(
        &[0x10],
        Layout {
            width: 2,
            height: 2,
            format: PixelFormat::Indexed4,
        },
    );

    assert!(matches!(
        result,
        Err(BtxFamilyPreviewError::Btx(
            BtxError::DataLengthMismatch { .. }
        ))
    ));
}
