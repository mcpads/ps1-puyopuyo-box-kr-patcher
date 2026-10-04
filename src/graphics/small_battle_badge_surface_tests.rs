use super::*;

#[test]
fn 작은_배지_합성은_장식과_효과를_보존하고_같은_크기_상쇄를_옮긴다() {
    let mut target = vec![14; 168 * 112];
    let mut translated = vec![0; 256 * 126];
    for block in BATAN2_TEXT_ASSET.blocks {
        let r = block.bbox;
        let PaletteIndexSpec::Values(indices) = block.palette_indices else {
            panic!()
        };
        for y in 0..r.height {
            for x in 0..r.width {
                translated[(r.y + y) * 256 + r.x + x] = indices[(x + y) % indices.len()];
            }
        }
    }
    compose_pixels(&mut target, &translated).unwrap();
    for y in 0..112 {
        for x in 0..168 {
            if !BLOCKS.iter().any(|b| {
                let r = b.bbox;
                x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
            }) {
                assert_eq!(target[y * 168 + x], 14, "보호 픽셀 {x},{y}");
            }
        }
    }
    for y in 0..16 {
        assert_eq!(
            &target[(64 + y) * 168..(64 + y) * 168 + 32],
            &translated[(88 + y) * 256 + 48..(88 + y) * 256 + 80]
        );
    }
}

#[test]
fn 작은_배지는_크기와_미등록_색을_거부한다() {
    assert!(compose_pixels(&mut [0; 1], &vec![0; 256 * 126]).is_err());
    assert!(compose_pixels(&mut vec![0; 168 * 112], &vec![14; 256 * 126]).is_err());
    assert!(patch_small_badges(&vec![0; 9408], SMALL_BADGE_ASSETS[0].0, &vec![0; 16128]).is_err());
}
