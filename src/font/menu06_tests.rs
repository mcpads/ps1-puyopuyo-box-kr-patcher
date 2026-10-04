use super::*;
use crate::font::bundled_maplestory_light;

#[test]
fn maplestory_light_대표문자는_16x16_binary_cell에_들어간다() {
    let font = bundled_maplestory_light().unwrap();
    for glyph in ['가', '뿌', '2', '.', '!'] {
        let pixels = rasterize_menu06_glyph(&font, glyph, MENU06_DRAFT_RASTER_PROFILE).unwrap();
        assert_eq!(pixels.len(), 256);
        assert!(pixels.contains(&15));
        assert!(pixels.iter().all(|pixel| matches!(pixel, 0 | 15)));
    }
}
