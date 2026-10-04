use super::*;

#[test]
fn object_pool은_500개_e0바이트_record를_word단위로_지운다() {
    assert_eq!(QUEST_OBJECT_POOL_RECORD_BYTES, 0xE0);
    assert_eq!(QUEST_OBJECT_POOL_RECORD_COUNT, 500);
    assert_eq!(QUEST_OBJECT_POOL_ZERO_WORDS, 0x6D60);
    assert_eq!(QUEST_OBJECT_POOL_RECORD_WORDS * 4, 0xE0);
}

#[test]
fn page_marker는_state_index_바로_앞의_별도_byte다() {
    assert_eq!(QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET, 0xDD);
    assert_eq!(QUEST_FONT_OBJECT_STATE_INDEX_OFFSET, 0xDE);
    assert_eq!(QUEST_FONT_OBJECT_SCALE_X_OFFSET, 0x58);
    assert_eq!(QUEST_FONT_OBJECT_NEXT_OFFSET, 0x08);
}

#[test]
fn type14_draw_list의_head와_sentinel은_서로_다르다() {
    assert_eq!(QUEST_FONT_OBJECT_TYPE, 14);
    assert_eq!(QUEST_FONT_OBJECT_LIST_HEAD_RUNTIME, 0x800D_A0B0);
    assert_eq!(QUEST_FONT_OBJECT_LIST_SENTINEL_RUNTIME, 0x800D_A0B4);
    assert_ne!(
        QUEST_FONT_OBJECT_LIST_HEAD_RUNTIME,
        QUEST_FONT_OBJECT_LIST_SENTINEL_RUNTIME
    );
}
