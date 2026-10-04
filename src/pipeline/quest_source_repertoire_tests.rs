use super::*;

#[test]
fn classifies_original_repertoire_roles() {
    assert_eq!(
        classify_code([0x82, 0x4F]).unwrap().2,
        QuestSourceGlyphClass::Digit
    );
    assert_eq!(
        classify_code([0x82, 0xA0]).unwrap().2,
        QuestSourceGlyphClass::Hiragana
    );
    assert_eq!(
        classify_code([0x83, 0x41]).unwrap().2,
        QuestSourceGlyphClass::KatakanaAndLengthMark
    );
    assert_eq!(
        classify_code([0x81, 0x5B]).unwrap().2,
        QuestSourceGlyphClass::KatakanaAndLengthMark
    );
    assert_eq!(
        classify_code([0x81, 0x60]).unwrap().2,
        QuestSourceGlyphClass::KatakanaAndLengthMark
    );
    assert_eq!(
        classify_code([0x8B, 0xAD]).unwrap().2,
        QuestSourceGlyphClass::SelectedKanji
    );
    assert_eq!(
        classify_code([0x81, 0x40]).unwrap().2,
        QuestSourceGlyphClass::SymbolOrSpace
    );
    assert_eq!(
        classify_code([0x82, 0x79]).unwrap().2,
        QuestSourceGlyphClass::FullwidthLatin
    );
}

#[test]
fn custom_ascii_pairs_remain_one_cell_codes() {
    let exclamation = classify_code([0x21, 0x21]).unwrap();
    assert_eq!(exclamation.0, "‼");
    assert_eq!(exclamation.1, "U+203C");
    assert_eq!(exclamation.2, QuestSourceGlyphClass::CustomAsciiPair);

    let question = classify_code([0x3F, 0x3F]).unwrap();
    assert_eq!(question.0, "⁇");
    assert_eq!(question.1, "U+2047");
    assert_eq!(question.2, QuestSourceGlyphClass::CustomAsciiPair);
}

#[test]
fn rejects_unregistered_multi_character_pair() {
    let error = classify_code([0x41, 0x42]).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("41 42"));
    assert!(message.contains("단일 glyph로 decode할 수 없음"));
}
