use super::*;

#[test]
fn 여섯픽셀_격자의_slot수는_layout에서_계산한다() {
    let slots = (QUEST_FONT_LAYOUT.width / COMPACT_CELL_WIDTH)
        * (QUEST_FONT_LAYOUT.height / QUEST_FONT_CELL_HEIGHT);
    assert_eq!(slots, 672);
}

#[test]
fn 한글_완성형_catalog의_최소_raw_크기는_글리프당_96바이트다() {
    assert_eq!(GLYPH_BYTES_INDEXED4, 96);
}

#[test]
fn 확장_atlas는_현재_수요의_행수를_올림해_계산한다() {
    assert_eq!(QUEST_FONT_LAYOUT.height, 256);
    assert_eq!(extended_atlas_height(661), 512);
    assert_eq!(extended_atlas_height(663), 528);
}
