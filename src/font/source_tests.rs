use super::*;

#[test]
fn bundled_maplestory_light_matches_the_pinned_source() {
    let font = bundled_maplestory_light().expect("고정 Maplestory Light");
    let (metrics, raster) = font.rasterize('가', 12.0);
    assert!(metrics.width > 0);
    assert!(metrics.height > 0);
    assert!(raster.iter().any(|value| *value != 0));
}

#[test]
fn wrong_length_is_rejected_before_font_parsing() {
    let error = load_maplestory_light(&[0; 16]).unwrap_err();
    assert_eq!(
        error,
        FontSourceError::WrongLength {
            expected: MAPLESTORY_LIGHT_LEN,
            actual: 16,
        }
    );
}

#[test]
fn modified_font_bytes_are_rejected() {
    let mut bytes = MAPLESTORY_LIGHT_BYTES.to_vec();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    let error = load_maplestory_light(&bytes).unwrap_err();
    assert!(matches!(error, FontSourceError::Sha1Mismatch { .. }));
}
