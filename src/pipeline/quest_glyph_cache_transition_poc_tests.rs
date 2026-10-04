use super::*;

#[test]
fn 두_batch의_page_id와_record는_분리된다() {
    assert_eq!(QUEST_GLYPH_CACHE_TRANSITION_SECOND_PAGE_ID, 62);
    assert_eq!(QUEST_GLYPH_CACHE_TRANSITION_THIRD_PAGE_ID, 50);
    assert_eq!(QUEST_GLYPH_CACHE_TRANSITION_SECOND_RECORD_OFFSET, 0x22DC);
    assert_eq!(QUEST_GLYPH_CACHE_TRANSITION_THIRD_RECORD_OFFSET, 0x362C);
}

#[test]
fn 초벌문장의_단어분할은_원문순서를_보존한다() {
    let first = "드라코「해냈다~! 내가 이겼네!! 아르르「그럴 수가~ 드라코「역시 귀여운 쪽이 뿌요뿌요도 강하다는 거네!!";
    let second = "드라코는 마지막 한 개의 특제 뿌요만을 손에 넣었다! 몹시 배가 고팠는지 침으로 옷이 흠뻑 젖었다.";
    let third = "드라코「그럼 이 특제 뿌요만은 내가 가져갈게! 아르르「내가 사려고 했는데~!";

    assert_eq!(
        split_words(first, &FIRST_WORDS_PER_LINE).unwrap(),
        [
            "드라코「해냈다~!",
            "내가 이겼네!!",
            "아르르「그럴수가~",
            "드라코「역시",
            "귀여운 쪽이",
            "뿌요뿌요도 강하다는 거네!!",
        ]
    );
    assert_eq!(
        split_words(second, &SECOND_WORDS_PER_LINE)
            .unwrap()
            .join(" "),
        second
    );
    assert_eq!(
        split_words(third, &THIRD_WORDS_PER_LINE).unwrap().join(" "),
        third
    );
}
