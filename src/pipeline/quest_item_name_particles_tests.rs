use super::*;
#[test]
fn 이름_발음은_기존_받침과_명시한_숫자_예외를_구분한다() {
    assert_eq!(pronunciation_tail("book", "탈출 책").unwrap(), '책');
    assert_eq!(pronunciation_tail("curry", "카레").unwrap(), '레');
    let id = "QUEST-ITEM-TEXT:ITEM_DAT.SEQ:record-0x0280:name";
    assert_eq!(pronunciation_tail(id, "아이템0").unwrap(), '영');
    assert_eq!(pronunciation_tail(id, "아이템０").unwrap(), '영');
    assert!(pronunciation_tail(id, "다른0").is_err());
    assert!(pronunciation_tail("unknown", "아이템0").is_err());
    assert!(pronunciation_tail(id, "아이템영").is_err());
    assert_eq!(non_hangul_tails().unwrap().get(&'０'), Some(&'영'));
    assert_eq!(non_hangul_tails().unwrap().get(&'0'), Some(&'영'));
}
