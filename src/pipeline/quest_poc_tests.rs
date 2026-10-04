use super::*;

#[test]
fn 모든_한글은_고유한_wide_slot과_2바이트_code를_갖는다() {
    let mut glyphs = BTreeSet::new();
    let mut indices = BTreeSet::new();
    let mut codes = BTreeSet::new();
    for mapping in QUEST_POC_GLYPH_MAPPINGS {
        assert!(glyphs.insert(mapping.glyph));
        assert!(indices.insert(mapping.index));
        assert!(codes.insert(mapping.code));
        let slot = QuestGlyphSlot::for_index(mapping.index).unwrap();
        assert_eq!((slot.width, slot.height), (12, 16));
        assert_eq!(
            encode_quest_poc_character(mapping.glyph).unwrap(),
            mapping.code
        );
    }
}

#[test]
fn mapping_없는_문자는_인코딩_실패다() {
    assert_eq!(
        encode_quest_poc_character('한'),
        Err(error("QUEST 한 문장 인코딩 mapping 누락: 한"))
    );
}

#[test]
fn 초벌_ascii_문장부호는_원본_전각_code로_정규화한다() {
    assert_eq!(encode_quest_poc_character('!').unwrap(), [0x81, 0x49]);
    assert_eq!(encode_quest_poc_character('~').unwrap(), [0x81, 0x60]);
    assert_eq!(encode_quest_poc_character('.').unwrap(), [0x81, 0x42]);
}
