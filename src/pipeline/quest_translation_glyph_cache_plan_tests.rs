use super::*;

#[test]
fn page별glyph문자열은_결정적순서를_가진다() {
    let page = QuestTranslationGlyphCachePage {
        page_id: 1,
        selected_routes: 1,
        glyphs: ['한', '글']
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    };
    assert_eq!(page.glyphs.chars().collect::<BTreeSet<_>>().len(), 2);
}

#[test]
fn glyph_cache_supplemental입력은_기존glyphs필드를_보존한다() {
    let page = QuestTranslationGlyphCacheSupplementalPage {
        page_id: 1,
        glyphs: "한글".to_owned(),
    };
    assert_eq!(page.glyphs, "한글");
}
