use super::*;
use crate::font::bundled_maplestory_light;
use std::collections::BTreeSet;

#[test]
fn maplestory_light_한글을_16x16_binary_cell로_만든다() {
    let font = bundled_maplestory_light().unwrap();
    let pixels = rasterize_info_page_glyph(&font, '가', P2_INFO_PAGE_DRAFT_RASTER_PROFILE).unwrap();
    assert_eq!(pixels.len(), 16 * 16);
    assert_eq!(
        pixels.iter().copied().collect::<BTreeSet<_>>(),
        [1, 15].into()
    );

    let foreground = pixels
        .iter()
        .enumerate()
        .filter(|(_, pixel)| **pixel == 15)
        .map(|(index, _)| (index % 16, index / 16))
        .collect::<Vec<_>>();
    let width = foreground.iter().map(|(x, _)| *x).max().unwrap()
        - foreground.iter().map(|(x, _)| *x).min().unwrap()
        + 1;
    let height = foreground.iter().map(|(_, y)| *y).max().unwrap()
        - foreground.iter().map(|(_, y)| *y).min().unwrap()
        + 1;
    assert_eq!((width, height), (12, 12));
}

#[test]
fn 공백은_active_background_cell로_만든다() {
    let font = bundled_maplestory_light().unwrap();
    let pixels = rasterize_info_page_glyph(&font, ' ', P2_INFO_PAGE_DRAFT_RASTER_PROFILE).unwrap();
    assert!(pixels.iter().all(|pixel| *pixel == 1));
}

#[test]
fn 잘못된_palette_profile을_거부한다() {
    let font = bundled_maplestory_light().unwrap();
    let invalid = InfoPageRasterProfile {
        empty: 1,
        ..P2_INFO_PAGE_DRAFT_RASTER_PROFILE
    };
    assert_eq!(
        rasterize_info_page_glyph(&font, '가', invalid),
        Err(InfoPageFontError::InvalidProfile)
    );
}
