use super::*;

#[test]
fn 검토_후보를_단어_손실없이_네_행으로_나눈다() {
    let reviewed = "드라코는 마지막 한 개의 특제 뿌요만을 손에 넣었다! 몹시 배가 고팠는지 침 때문에 옷이 끈적끈적하다.";
    let lines = split_words(reviewed, &QUEST_SECOND_DIALOGUE_LAYOUT_WORDS).unwrap();
    assert_eq!(lines.join(" "), reviewed);
    assert_eq!(
        lines
            .iter()
            .map(|line| line.chars().count())
            .collect::<Vec<_>>(),
        [13, 15, 16, 10]
    );
}

#[test]
fn 일반_공백과_패딩_공백은_mapping_수요에서_제외한다() {
    let lines = vec!["드라코 !.".to_string(), "　".to_string()];
    assert_eq!(
        mapping_characters(&lines),
        BTreeSet::from(['!', '.', '드', '라', '코'])
    );
}
