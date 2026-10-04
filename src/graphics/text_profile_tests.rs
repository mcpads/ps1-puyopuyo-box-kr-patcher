use super::*;

#[test]
fn bundled_font는_안정된_id로_등록한다() {
    let fonts = BakedFontBook::with_bundled_maplestory_light().unwrap();
    assert!(fonts.get(MAPLESTORY_LIGHT_FONT_ID).is_some());
    assert!(fonts.get("없는-font").is_none());
}

#[test]
fn font_id의_조용한_덮어쓰기를_거부한다() {
    let mut fonts = BakedFontBook::with_bundled_maplestory_light().unwrap();
    let duplicate = bundled_maplestory_light().unwrap();
    assert_eq!(
        fonts.insert(MAPLESTORY_LIGHT_FONT_ID, duplicate),
        Err(BakedFontBookError::DuplicateId(
            MAPLESTORY_LIGHT_FONT_ID.to_string()
        ))
    );
}
