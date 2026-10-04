use super::*;

#[test]
fn 현재_layout의_wide_slot_분모는_326개다() {
    let wide = (0..QUEST_CODE_TABLE_COUNT)
        .filter(|index| {
            QuestGlyphSlot::for_index(*index)
                .is_ok_and(|slot| slot.width == QUEST_FONT_WIDE_CELL_WIDTH)
        })
        .count();
    assert_eq!(wide, QUEST_FONT_WIDE_SLOTS);
    assert_eq!(
        crate::font::QUEST_FONT_NARROW_SLOTS + QUEST_FONT_WIDE_SLOTS,
        QUEST_CODE_TABLE_COUNT
    );
}
