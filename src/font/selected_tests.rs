use super::*;

#[test]
fn 선택_원천과_필수_글리프를_검증한다() {
    for id in [GALMURI11, GALMURI11_BOLD, DENKI, MULMARU] {
        let font = load(id).unwrap();
        for ch in "상쇄장비최하특기".chars() {
            assert_ne!(font.lookup_glyph_index(ch), 0);
        }
    }
    assert!(load("unknown").is_err());
}

#[test]
fn 메뉴와_대사의_같은_글자는_다른_identity이며_원문을_복원한다() {
    let text = "장비3　！";
    let encoded = menu_catalog_text(text);
    assert_ne!(encoded, text);
    assert_eq!(
        encoded
            .chars()
            .map(|c| menu_glyph(c).unwrap_or(c))
            .collect::<String>(),
        text
    );
    assert_eq!(menu_glyph('장'), None);
}

#[test]
fn 원본_공통_슬롯의_기호와_숫자_폭을_보존한다() {
    let font = load(DENKI).unwrap();
    for glyph in "０１２３４５６７８９／。、‼⁇（）「：".chars() {
        assert_eq!(quest_cell(&font, glyph, 6).unwrap().len(), 6 * 16);
    }
    for glyph in "Ｚ〜！？♪●ＧＮＥＸＢＯＴ□　＊하트회전".chars() {
        assert_eq!(quest_cell(&font, glyph, 12).unwrap().len(), 12 * 16);
    }
}

#[test]
fn 그림_기호는_일반_텍스트_재생성에_포함하지_않는다() {
    for glyph in ['●', '□', '♪', '＊'] {
        assert!(!quest_common_glyph(glyph));
    }
    for glyph in ['0', 'Ｇ', '！', '⁇', '*'] {
        assert!(quest_common_glyph(glyph));
    }
}

#[test]
fn 원본_문장부호_슬롯에도_한국어_마침표와_쉼표를_그린다() {
    let font = load(DENKI).unwrap();
    for width in [6, 12] {
        for (slot, punctuation) in [('。', '.'), ('、', ',')] {
            assert_eq!(
                quest_cell(&font, slot, width).unwrap(),
                quest_cell(&font, punctuation, width).unwrap()
            );
        }
    }
}
