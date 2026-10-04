use super::*;

#[test]
fn translation_hook_schema는package나runtime승인을_암시하지않는다() {
    assert!(QUEST_TRANSLATION_GLYPH_CACHE_HOOK_SCHEMA_VERSION.contains("translation"));
    assert!(QUEST_TRANSLATION_GLYPH_CACHE_HOOK_SCHEMA_VERSION.contains("hook-plan"));
    assert!(!QUEST_TRANSLATION_GLYPH_CACHE_HOOK_SCHEMA_VERSION.contains("approved"));
    assert!(!QUEST_TRANSLATION_GLYPH_CACHE_HOOK_SCHEMA_VERSION.contains("runtime-verified"));
}
