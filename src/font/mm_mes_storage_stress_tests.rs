use super::*;
use crate::font::bundled_maplestory_light;

#[test]
fn storage_stress_한글은_16x16_정사각_coverage_cell에_들어간다() {
    let font = bundled_maplestory_light().unwrap();
    for glyph in ['가', '뿌', '쀍', '한'] {
        let pixels = rasterize_mm_mes_storage_stress_glyph(
            &font,
            glyph,
            MM_MES_STORAGE_STRESS_RASTER_PROFILE,
        )
        .unwrap();
        assert_eq!(pixels.len(), 256);
        assert!(pixels.iter().any(|&pixel| pixel != 0));
        assert!(pixels.iter().all(|&pixel| pixel <= 13));
        let bounds = coverage_bounds(&pixels).unwrap();
        assert!(bounds.height() >= bounds.width());
    }
}

#[test]
fn 공백은_투명_cell이고_잘못된_ramp는_거부한다() {
    let font = bundled_maplestory_light().unwrap();
    assert_eq!(
        rasterize_mm_mes_storage_stress_glyph(&font, ' ', MM_MES_STORAGE_STRESS_RASTER_PROFILE)
            .unwrap(),
        vec![0; 256]
    );
    let invalid = MmMesStorageStressRasterProfile {
        effect: MmMesStorageStressRasterEffect::CoverageRamp {
            transparent: 0,
            ramp: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 12],
        },
        ..MM_MES_STORAGE_STRESS_RASTER_PROFILE
    };
    assert_eq!(
        rasterize_mm_mes_storage_stress_glyph(&font, '가', invalid),
        Err(MmMesStorageStressFontError::InvalidProfile)
    );
}

#[test]
fn cell보다_큰_profile은_잘림_대신_실패한다() {
    let font = bundled_maplestory_light().unwrap();
    let too_large = MmMesStorageStressRasterProfile {
        font_size: 40.0,
        ..MM_MES_STORAGE_STRESS_RASTER_PROFILE
    };
    assert!(matches!(
        rasterize_mm_mes_storage_stress_glyph(&font, '가', too_large),
        Err(MmMesStorageStressFontError::GlyphDoesNotFit { .. })
    ));
}
