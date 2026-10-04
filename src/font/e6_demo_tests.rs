use std::collections::BTreeSet;

use super::*;
use crate::font::bundled_maplestory_light;

#[test]
fn maplestory_light_대표한글은_16x16_투명그림자cell에_들어간다() {
    let font = bundled_maplestory_light().unwrap();
    for glyph in "가나다라마바사아자차카타파하드라코해냈어".chars() {
        let pixels = rasterize_e6_demo_glyph(&font, glyph, E6_DEMO_POC_RASTER_PROFILE).unwrap();
        assert_eq!(
            pixels.len(),
            E6_DEMO_GLYPH_CELL_WIDTH * E6_DEMO_GLYPH_CELL_HEIGHT
        );
        assert_eq!(
            pixels.into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([0, 1, 15])
        );
    }
}

#[test]
fn e6_가_본문은_정사각형보다_납작해지지_않는다() {
    let font = bundled_maplestory_light().unwrap();
    let raster =
        rasterize_e6_demo_glyph_with_geometry(&font, '가', E6_DEMO_POC_RASTER_PROFILE).unwrap();

    assert_eq!(
        (
            raster.geometry.source_foreground_width,
            raster.geometry.source_foreground_height,
        ),
        (12, 11)
    );
    assert_eq!(
        (
            raster.geometry.foreground_width,
            raster.geometry.foreground_height,
        ),
        (12, 12)
    );
    assert_eq!(
        (raster.geometry.effect_width, raster.geometry.effect_height),
        (13, 13)
    );
    assert!(raster.geometry.normalized_wide_hangul);
}

#[test]
fn e6_넓은한글_정규화는_profile에서_분리된다() {
    let font = bundled_maplestory_light().unwrap();
    let mut profile = E6_DEMO_POC_RASTER_PROFILE;
    profile.normalize_wide_hangul = false;
    let raster = rasterize_e6_demo_glyph_with_geometry(&font, '가', profile).unwrap();

    assert_eq!(
        (
            raster.geometry.foreground_width,
            raster.geometry.foreground_height,
        ),
        (12, 11)
    );
    assert!(!raster.geometry.normalized_wide_hangul);
}

#[test]
fn e6_profile은_폰트크기와_palette_effect오류를_거부한다() {
    let font = bundled_maplestory_light().unwrap();
    let mut invalid = E6_DEMO_POC_RASTER_PROFILE;
    invalid.font_size = 0.0;
    assert_eq!(
        rasterize_e6_demo_glyph(&font, '가', invalid),
        Err(E6DemoFontError::InvalidProfile)
    );

    let mut oversized = E6_DEMO_POC_RASTER_PROFILE;
    oversized.font_size = 32.0;
    assert!(matches!(
        rasterize_e6_demo_glyph(&font, '가', oversized),
        Err(E6DemoFontError::GlyphDoesNotFit { .. })
    ));
}
