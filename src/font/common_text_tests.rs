use super::*;

const TEST_RUNTIME_CELLS: [u16; 1] = [3];
const TEST_SPEC: CommonFontTextSurfaceSpec = CommonFontTextSurfaceSpec {
    surface_id: "fixture-surface",
    archive: "/FIXTURE.VDD;1",
    source_text: "あ",
    semantic_category: "fixture",
    x: 1,
    y: 1,
    width: 2,
    height: 2,
    representative_runtime_cell_candidates: &TEST_RUNTIME_CELLS,
    nonzero_pixels: 4,
    pixel_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
};

#[test]
fn crop은_합성_rectangle밖_pixel과_잘린입력을_거부한다() {
    let pixels = (0..12).collect::<Vec<_>>();
    assert_eq!(
        crop_pixels(&pixels, 4, 3, TEST_SPEC).unwrap(),
        [5, 6, 9, 10]
    );

    let outside = CommonFontTextSurfaceSpec { x: 3, ..TEST_SPEC };
    assert!(crop_pixels(&pixels, 4, 3, outside).is_err());
    assert!(crop_pixels(&pixels[..pixels.len() - 1], 4, 3, TEST_SPEC).is_err());
}
