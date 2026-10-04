use super::*;
use crate::font::{QuestCodeTable, QUEST_CODE_TABLE_COUNT};

const PAGE_ID: u8 = 8;

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
        BTreeMap::from([
            ('가', 30u16.to_le_bytes()),
            ('나', 31u16.to_le_bytes()),
            ('！', 32u16.to_le_bytes()),
        ]),
    )])
}

fn parts() -> Vec<QuestItemDescriptionDraftPart> {
    vec![
        QuestItemDescriptionDraftPart::Control(vec![0x0B, 0xFF, 0, 0]),
        QuestItemDescriptionDraftPart::Literal("가나!".to_owned()),
        QuestItemDescriptionDraftPart::Control(vec![0x04, 0xFF]),
        QuestItemDescriptionDraftPart::Literal("가".to_owned()),
        QuestItemDescriptionDraftPart::Control(vec![0x1A, 0xFF, 0, 0, 0, 0]),
    ]
}

#[test]
fn 수동literal과control위치를그대로직렬화한다() {
    let (normalized, glyphs, controls, output) =
        encode_description("가나!가", &parts(), PAGE_ID, &page_codes(), &code_table()).unwrap();

    assert_eq!(normalized, "가나！가");
    assert_eq!(glyphs, 4);
    assert_eq!(controls, 3);
    assert_eq!(
        output,
        [0x0B, 0xFF, 0, 0, 30, 0, 31, 0, 32, 0, 0x04, 0xFF, 30, 0, 0x1A, 0xFF, 0, 0, 0, 0,]
    );
}

#[test]
fn 일본어와page0와종단누락을거부한다() {
    assert!(encode_description("あ", &parts(), PAGE_ID, &page_codes(), &code_table()).is_err());
    assert!(encode_description("가나!가", &parts(), 0, &page_codes(), &code_table()).is_err());
    let mut missing_terminal = parts();
    missing_terminal.pop();
    assert!(encode_description(
        "가나!가",
        &missing_terminal,
        PAGE_ID,
        &page_codes(),
        &code_table()
    )
    .is_err());
}
