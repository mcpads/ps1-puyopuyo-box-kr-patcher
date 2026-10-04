use super::*;
use crate::font::MM_MES_STORAGE_STRESS_RASTER_PROFILE;

#[test]
fn 선언한_stress_cell만_바꾸고_나머지는_보존한다() {
    let original = vec![0u8; MM_MES_LAYOUT.width * MM_MES_LAYOUT.height];
    let mappings = [MmMesGridStressMapping {
        cell: 33,
        glyph: Some('가'),
    }];
    let (patched, changed_cells, changed_pixels) =
        patch_pixels(&original, &mappings, MM_MES_STORAGE_STRESS_RASTER_PROFILE).unwrap();
    assert_eq!(changed_cells, 1);
    assert!(changed_pixels > 0);
    assert!(cell_changed(&original, &patched, 33));
    for cell in 0..MM_MES_STORAGE_CELLS {
        if cell != 33 {
            assert!(!cell_changed(&original, &patched, cell));
        }
    }
}

#[test]
fn 중복_범위이탈_공백_mapping은_거부한다() {
    assert!(validate_mappings(&[
        MmMesGridStressMapping {
            cell: 1,
            glyph: Some('가'),
        },
        MmMesGridStressMapping {
            cell: 1,
            glyph: Some('나'),
        },
    ])
    .is_err());
    assert!(validate_mappings(&[MmMesGridStressMapping {
        cell: MM_MES_STORAGE_CELLS as u16,
        glyph: Some('가'),
    }])
    .is_err());
    assert!(validate_mappings(&[MmMesGridStressMapping {
        cell: 0,
        glyph: Some(' '),
    }])
    .is_err());
}

#[test]
fn 빈_mapping과_변경없는_clear는_정상산출물로_보지_않는다() {
    let original = vec![0u8; MM_MES_LAYOUT.width * MM_MES_LAYOUT.height];
    assert!(patch_pixels(&original, &[], MM_MES_STORAGE_STRESS_RASTER_PROFILE).is_err());
    assert!(patch_pixels(
        &original,
        &[MmMesGridStressMapping {
            cell: 0,
            glyph: None,
        }],
        MM_MES_STORAGE_STRESS_RASTER_PROFILE,
    )
    .is_err());
}
