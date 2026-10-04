use super::*;
use crate::font::{QuestCodeTable, QUEST_CODE_TABLE_COUNT};

const PAGE_ID: u8 = 7;

fn code_table() -> QuestCodeTable {
    let mut codes = (0..QUEST_CODE_TABLE_COUNT)
        .map(|index| u16::try_from(index).expect("fixture code").to_le_bytes())
        .collect::<Vec<_>>();
    codes[0] = [0x81, 0x40];
    let raw = codes.into_iter().flatten().collect::<Vec<_>>();
    QuestCodeTable::parse(&raw).expect("fixture QUEST code table")
}

fn page_codes() -> BTreeMap<u8, BTreeMap<char, [u8; 2]>> {
    BTreeMap::from([(
        PAGE_ID,
        BTreeMap::from([
            ('가', 30u16.to_le_bytes()),
            ('나', 31u16.to_le_bytes()),
            ('！', 32u16.to_le_bytes()),
        ]),
    )])
}

#[test]
fn 수동이름을_page_code와_마지막nul로_직렬화한다() {
    let (normalized, cells, output) =
        encode_item_name("가나!", PAGE_ID, &page_codes(), &code_table()).unwrap();

    assert_eq!(normalized, "가나！");
    assert_eq!(cells, 3);
    assert_eq!(output, [30, 0, 31, 0, 32, 0, 0, 0]);
    assert!(!output[..output.len() - 2]
        .chunks_exact(2)
        .any(|pair| pair == [0, 0]));
}

#[test]
fn 일본어와_미공급glyph와_page0을_거부한다() {
    assert!(encode_item_name("あ", PAGE_ID, &page_codes(), &code_table()).is_err());
    assert!(encode_item_name("다", PAGE_ID, &page_codes(), &code_table()).is_err());
    assert!(encode_item_name("가", 0, &page_codes(), &code_table()).is_err());
}
