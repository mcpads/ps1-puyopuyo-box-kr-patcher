use std::collections::BTreeSet;

use super::*;
use crate::font::bundled_maplestory_light;

#[test]
fn scene_profile은_독립된_maplestory_binary_cell을_만든다() {
    let font = bundled_maplestory_light().unwrap();
    let pixels = rasterize_scene_glyph(&font, '가', SCENE_POC_RASTER_PROFILE).unwrap();
    assert_eq!(
        pixels.len(),
        SCENE_GLYPH_CELL_WIDTH * SCENE_GLYPH_CELL_HEIGHT
    );
    assert_eq!(pixels.into_iter().collect::<BTreeSet<_>>(), [1, 15].into());
}

#[test]
fn scene_profile은_잘못된_effect와_큰_글리프를_거부한다() {
    let font = bundled_maplestory_light().unwrap();
    let invalid = SceneRasterProfile {
        effect: SceneRasterEffect::Binary {
            threshold: 0,
            background: 1,
            foreground: 15,
        },
        ..SCENE_POC_RASTER_PROFILE
    };
    assert_eq!(
        rasterize_scene_glyph(&font, '가', invalid),
        Err(SceneFontError::InvalidProfile)
    );
    let oversized = SceneRasterProfile {
        font_size: 32.0,
        ..SCENE_POC_RASTER_PROFILE
    };
    assert!(matches!(
        rasterize_scene_glyph(&font, '가', oversized),
        Err(SceneFontError::GlyphDoesNotFit { .. })
    ));
}

#[test]
fn 번역draft_profile은_하단bearing이_큰_흥을_압축없이_수용한다() {
    let font = crate::font::selected::load(crate::font::selected::GALMURI11).unwrap();
    let pixels =
        rasterize_scene_glyph(&font, '흥', SCENE_TRANSLATION_DRAFT_RASTER_PROFILE).unwrap();

    assert_eq!(
        pixels.len(),
        SCENE_GLYPH_CELL_WIDTH * SCENE_GLYPH_CELL_HEIGHT
    );
    assert!(pixels.contains(&15));
}

#[test]
fn 앞_opaque_cell이_다음_글자의_획을_덮지_않는다() {
    let font = crate::font::selected::load(crate::font::selected::GALMURI11).unwrap();
    for ch in "아잖뿌요지옥나승부길흥".chars() {
        let cell =
            rasterize_scene_glyph(&font, ch, SCENE_TRANSLATION_DRAFT_RASTER_PROFILE).unwrap();
        for row in cell.chunks_exact(16) {
            assert!(
                row[..4].iter().all(|&pixel| pixel == 1),
                "{ch}의 잉크가 앞 cell 배경에 가려짐"
            );
        }
    }
}
