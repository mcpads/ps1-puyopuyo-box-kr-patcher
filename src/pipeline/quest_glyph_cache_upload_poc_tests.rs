use super::*;

#[test]
fn catalog_page는_assignment순서대로_aligned_slot_code에_결속된다() {
    let repertoire = ['가', '나', '다', '라'];
    let mut codes = vec![[0x81, 0x40]; QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX + 2];
    codes[QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX] = [0x82, 0x40];
    codes[QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX + 1] = [0x82, 0x41];

    let rows = map_catalog_page_to_codes(&repertoire, &[0, 2], &codes).unwrap();
    assert_eq!(
        rows,
        [
            QuestGlyphCachePageCode {
                glyph: '가',
                catalog_index: 0,
                slot_index: QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX,
                code: [0x82, 0x40],
            },
            QuestGlyphCachePageCode {
                glyph: '다',
                catalog_index: 2,
                slot_index: QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX + 1,
                code: [0x82, 0x41],
            },
        ]
    );
}

#[test]
fn catalog_page는_중복_범위초과_code부족을_거부한다() {
    let repertoire = ['가', '나'];
    let codes = vec![[0x82, 0x40]; QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX + 1];
    assert!(map_catalog_page_to_codes(&repertoire, &[], &codes).is_err());
    assert!(map_catalog_page_to_codes(&repertoire, &[0, 0], &codes).is_err());
    assert!(map_catalog_page_to_codes(&repertoire, &[2], &codes).is_err());
    assert!(map_catalog_page_to_codes(
        &repertoire,
        &[0, 1],
        &codes[..QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX + 1]
    )
    .is_err());
}
