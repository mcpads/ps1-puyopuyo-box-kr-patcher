use std::collections::BTreeMap;

use crate::font::{QuestCodeTable, QUEST_CODE_TABLE_COUNT};

use super::encode_quest_fixed_slot_text;

const PAGE_ID: u8 = 7;

fn code_table() -> QuestCodeTable {
    let mut codes = (0..QUEST_CODE_TABLE_COUNT)
        .map(|index| u16::try_from(index).expect("fixture code").to_le_bytes())
        .collect::<Vec<_>>();
    codes[0] = [0x81, 0x40];
    QuestCodeTable::parse(&codes.into_iter().flatten().collect::<Vec<_>>())
        .expect("fixture QUEST code table")
}

fn page_codes() -> BTreeMap<u8, BTreeMap<char, [u8; 2]>> {
    BTreeMap::from([(
        PAGE_ID,
        BTreeMap::from([('가', 30u16.to_le_bytes()), ('나', 31u16.to_le_bytes())]),
    )])
}

#[test]
fn 수동_고정문은_원본_slot을_nul로_채운다() {
    let encoded = encode_quest_fixed_slot_text("가나", 7, PAGE_ID, &page_codes(), &code_table())
        .expect("encode");
    assert_eq!(encoded.normalized_text, "가나");
    assert_eq!(encoded.glyph_cells, 2);
    assert_eq!(encoded.output, [30, 0, 31, 0, 0, 0, 0]);
}

#[test]
fn 수동_생략은_nul만_남기고_용량_초과는_거부한다() {
    let encoded =
        encode_quest_fixed_slot_text("", 3, PAGE_ID, &page_codes(), &code_table()).expect("empty");
    assert_eq!(encoded.glyph_cells, 0);
    assert_eq!(encoded.output, [0, 0, 0]);
    assert!(
        encode_quest_fixed_slot_text("가나", 4, PAGE_ID, &page_codes(), &code_table()).is_err()
    );
}
