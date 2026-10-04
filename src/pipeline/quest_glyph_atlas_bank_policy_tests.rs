use super::*;

fn state(logical_pages: [u8; 2], pinned: [bool; 2]) -> QuestGlyphAtlasBankState {
    QuestGlyphAtlasBankState {
        logical_pages,
        pinned,
    }
}

#[test]
fn 이전_page_object가살아있어도_빈bank로다음page를고른다() {
    assert_eq!(
        select_quest_glyph_atlas_bank(18, state([51, 0], [true, false])).unwrap(),
        QuestGlyphAtlasBankSelection::Upload {
            bank: QuestGlyphAtlasBank::Secondary,
            marker: 0x80 | 18,
        }
    );
}

#[test]
fn 요청page가resident이고_live_object가있을때만재사용한다() {
    assert_eq!(
        select_quest_glyph_atlas_bank(51, state([51, 18], [true, true])).unwrap(),
        QuestGlyphAtlasBankSelection::Resident {
            bank: QuestGlyphAtlasBank::Primary,
            marker: 51,
        }
    );
    assert_eq!(
        select_quest_glyph_atlas_bank(18, state([51, 18], [true, true])).unwrap(),
        QuestGlyphAtlasBankSelection::Resident {
            bank: QuestGlyphAtlasBank::Secondary,
            marker: 0x80 | 18,
        }
    );
}

#[test]
fn page_state만남은_unpinned_bank는같은bank에다시올린다() {
    assert_eq!(
        select_quest_glyph_atlas_bank(51, state([51, 18], [false, true])).unwrap(),
        QuestGlyphAtlasBankSelection::Upload {
            bank: QuestGlyphAtlasBank::Primary,
            marker: 51,
        }
    );
    assert_eq!(
        select_quest_glyph_atlas_bank(18, state([51, 18], [true, false])).unwrap(),
        QuestGlyphAtlasBankSelection::Upload {
            bank: QuestGlyphAtlasBank::Secondary,
            marker: 0x80 | 18,
        }
    );
}

#[test]
fn 모든bank가_pinned이고요청page가없을때만_대기한다() {
    assert_eq!(
        select_quest_glyph_atlas_bank(7, state([51, 18], [true, true])).unwrap(),
        QuestGlyphAtlasBankSelection::WaitAllBanksPinned
    );
}

#[test]
fn page0은원본이고_marker의상위bit가bank를구분한다() {
    assert_eq!(
        select_quest_glyph_atlas_bank(0, state([0, 0], [false, false])).unwrap(),
        QuestGlyphAtlasBankSelection::Original
    );
    assert_eq!(quest_glyph_atlas_bank_from_marker(0), None);
    assert_eq!(
        quest_glyph_atlas_bank_from_marker(56),
        Some(QuestGlyphAtlasBank::Primary)
    );
    assert_eq!(
        quest_glyph_atlas_bank_from_marker(0x80 | 18),
        Some(QuestGlyphAtlasBank::Secondary)
    );
}

#[test]
fn marker에들지않는_page는실패한다() {
    assert!(encode_quest_glyph_atlas_marker(0, QuestGlyphAtlasBank::Primary).is_err());
    assert!(select_quest_glyph_atlas_bank(0x80, state([0, 0], [false, false])).is_err());
}
