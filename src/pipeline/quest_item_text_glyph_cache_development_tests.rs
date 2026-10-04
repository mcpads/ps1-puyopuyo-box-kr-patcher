use super::*;
use crate::font::{QuestCodeTable, QUEST_CODE_TABLE_COUNT};

fn code_table() -> QuestCodeTable {
    let mut codes = (0..QUEST_CODE_TABLE_COUNT)
        .map(|index| u16::try_from(index).expect("fixture code").to_le_bytes())
        .collect::<Vec<_>>();
    codes[0] = [0x81, 0x40];
    QuestCodeTable::parse(&codes.into_iter().flatten().collect::<Vec<_>>())
        .expect("fixture QUEST code table")
}

fn glyphs(start: u32, count: u32) -> BTreeSet<char> {
    (start..start + count)
        .map(|offset| char::from_u32(0xAC00 + offset).expect("fixture Hangul"))
        .collect()
}

fn entry(
    unit_id: &str,
    names: &BTreeSet<char>,
    exclusive: BTreeSet<char>,
) -> DescriptionGlyphEntry {
    (
        unit_id.to_owned(),
        names.union(&exclusive).copied().collect(),
        exclusive,
    )
}

#[test]
fn 설명glyph는_최소page수부터_결정적으로_분할한다() {
    let names = glyphs(0, 300);
    let entries = vec![
        entry("description-a", &names, glyphs(300, 10)),
        entry("description-b", &names, glyphs(310, 10)),
    ];
    let mut reversed = entries.clone();
    reversed.reverse();

    let groups = partition_description_glyph_entries(&names, entries, &code_table()).unwrap();
    let reversed_groups =
        partition_description_glyph_entries(&names, reversed, &code_table()).unwrap();

    assert_eq!(groups.len(), 2);
    assert_eq!(
        groups
            .iter()
            .map(|group| group.unit_ids.clone())
            .collect::<Vec<_>>(),
        reversed_groups
            .iter()
            .map(|group| group.unit_ids.clone())
            .collect::<Vec<_>>()
    );
    assert!(groups
        .iter()
        .all(|group| names.is_subset(&group.used_glyphs)));
    assert_eq!(
        groups
            .iter()
            .flat_map(|group| group.unit_ids.iter().map(String::as_str))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["description-a", "description-b"])
    );
}

#[test]
fn 카드전용_glyph는_첫page에만_두고_동시표시_공통glyph는_보존한다() {
    let shared = glyphs(0, 260);
    let ui = glyphs(260, 40).into_iter().collect::<String>();
    let entries = vec![
        entry("a", &shared, BTreeSet::new()),
        entry("b", &shared, glyphs(300, 20)),
        entry("c", &shared, glyphs(320, 20)),
    ];
    assert!(partition_description_glyph_entries(
        &shared.iter().copied().chain(ui.chars()).collect(),
        entries.clone(),
        &code_table()
    )
    .is_err());
    let groups = partition_with_ui_page(&shared, &ui, entries, &code_table()).unwrap();
    assert!(ui.chars().all(|ch| groups[0].used_glyphs.contains(&ch)));
    assert!(
        groups[0].unit_ids.is_empty(),
        "Denki 메뉴 page에 갈무리 설명을 넣지 않는다"
    );
    assert!(groups.iter().all(|g| shared.is_subset(&g.used_glyphs)));
    assert!(groups
        .iter()
        .skip(1)
        .all(|g| ui.chars().all(|ch| !g.used_glyphs.contains(&ch))));
    assert_eq!(
        groups
            .iter()
            .flat_map(|g| g.unit_ids.iter().map(String::as_str))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["a", "b", "c"])
    );
}
