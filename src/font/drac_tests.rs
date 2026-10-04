use std::collections::BTreeSet;

use super::*;
use crate::font::bundled_maplestory_light;

#[test]
fn maplestory_light_대표한글은_16x16_binary_cell에_들어간다() {
    let font = bundled_maplestory_light().unwrap();
    for glyph in "가나다라마바사아자차카타파하드라코해냈어".chars() {
        let pixels = rasterize_drac_glyph(&font, glyph, DRAC_POC_RASTER_PROFILE).unwrap();
        assert_eq!(pixels.len(), DRAC_GLYPH_CELL_WIDTH * DRAC_GLYPH_CELL_HEIGHT);
        assert_eq!(
            pixels.into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([1, 15])
        );
    }
}

#[test]
fn raster_profile은_font_size와_palette_effect오류를_거부한다() {
    let font = bundled_maplestory_light().unwrap();
    let mut invalid = DRAC_POC_RASTER_PROFILE;
    invalid.font_size = 0.0;
    assert_eq!(
        rasterize_drac_glyph(&font, '가', invalid),
        Err(DracFontError::InvalidProfile)
    );

    let mut oversized = DRAC_POC_RASTER_PROFILE;
    oversized.font_size = 32.0;
    assert!(matches!(
        rasterize_drac_glyph(&font, '가', oversized),
        Err(DracFontError::GlyphDoesNotFit { .. })
    ));
}
