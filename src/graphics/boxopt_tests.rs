use super::*;

fn supported_table() -> Vec<u8> {
    let mut raw = vec![0u8; BOXOPT_SELECTOR_TABLE_BYTES + 12];
    raw[4..6].copy_from_slice(&BOXOPT_ATLAS_WIDTH.to_le_bytes());
    raw[6..8].copy_from_slice(&BOXOPT_ATLAS_HEIGHT.to_le_bytes());
    for selector in BOXOPT_PHRASES {
        let offset = 12 + selector.selector * 12;
        raw[offset + 4..offset + 6].copy_from_slice(&8u16.to_le_bytes());
        raw[offset + 6..offset + 8].copy_from_slice(&16u16.to_le_bytes());
    }
    raw
}

#[test]
fn phrase_원장은_selector가_오름차순이고_번역_대상은_46개다() {
    assert!(BOXOPT_PHRASES
        .windows(2)
        .all(|pair| pair[0].selector < pair[1].selector));
    assert_eq!(
        BOXOPT_PHRASES
            .iter()
            .filter(|spec| spec.translation_target)
            .count(),
        BOXOPT_TRANSLATION_TARGET_SELECTORS
    );
}

#[test]
fn overlap_group은_겹치는_최종_쓰기를_명시한다() {
    assert_eq!(
        BOXOPT_OVERLAPS,
        [(25, 27), (35, 45), (37, 57), (40, 48), (51, 52), (60, 69)]
    );
}

#[test]
fn 길이와_header_drift는_table로_승인하지_않는다() {
    let mut raw = supported_table();
    assert!(parse_boxopt_selector_table(&raw).is_err());
    raw[4] ^= 1;
    assert!(parse_boxopt_selector_table(&raw).is_err());
    assert!(parse_boxopt_selector_table(&raw[..BOXOPT_SELECTOR_TABLE_BYTES]).is_err());
}
