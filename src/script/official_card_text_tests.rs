use super::*;

fn bytes(words: &[u16]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn glyphs() -> BTreeMap<char, u16> {
    BTreeMap::from([(' ', 24), ('가', 25), ('나', 26), ('예', 27), ('아', 28)])
}

// 긴 번역 줄을 다른 줄의 여유 공간으로 수용해도 선택지 좌표와 종료는 보존한다.
#[test]
fn reflow_preserves_choice_coordinates_and_terminal_offsets() {
    let source = bytes(&[
        0xff11, 16, 16, 0xff16, 1, 24, 24, 24, 0xff02, 25, 0xff02, 0xff13, 64, 16, 24, 25, 0xff13,
        48, 0, 24, 0xff00,
    ]);
    let result = rewrite(
        &source,
        "あああ\\nい\\n<FF13:64,16>あい<FF13:48,0>あ",
        "가\\n가나\\n<FF13:64,16>예<FF13:48,0>아",
        &glyphs(),
    )
    .unwrap();
    assert_eq!(
        result,
        bytes(&[
            0xff11, 16, 16, 0xff16, 1, 25, 24, 0xff02, 25, 26, 0xff02, 0xff13, 64, 16, 27, 24,
            0xff13, 48, 0, 28, 0xff00
        ])
    );
}

// 번역 공간·문자·제어를 충족하지 못하면 대체문자나 잘린 출력으로 성공하지 않는다.
#[test]
fn rejects_overflow_missing_glyph_and_changed_control() {
    let original = bytes(&[0xff11, 16, 16, 24, 0xff02, 0xff00]);
    for ko in ["가나\\n", "힣\\n", "가<FF13:64,0>"] {
        assert!(rewrite(&original, "あ\\n", ko, &glyphs()).is_err());
    }
    assert!(rewrite(&original, "い\\n", "가\\n", &glyphs()).is_err());
    assert!(compile_official_card_streams(&original, &glyphs(), &[], &[]).is_err());
}

#[test]
fn rejects_truncated_unknown_and_unterminated_source() {
    for words in [
        &[0xff11, 16][..],
        &[0xff99, 0xff00],
        &[24],
        &[0xff00, 24],
        &[128, 0xff00],
    ] {
        assert!(source_parts(&bytes(words)).is_err());
    }
}

// 문장과 셀 배치를 대조한 PCOMI 참조의 부호·종료 글자를 분리해서 읽는다.
#[test]
fn pocket_reference_repertoire_keeps_punctuation_and_exit_glyphs_distinct() {
    let parsed = source_parts(&bytes(&[112, 113, 114, 115, 116, 117, 122, 0xff00])).unwrap();
    assert_eq!(visible(&parsed), "ポケテション８");
    assert_eq!(
        visible(&source_parts(&bytes(&[123, 124, 125, 0xff00])).unwrap()),
        "「！」"
    );
    assert_eq!(
        visible(&source_parts(&bytes(&[126, 127, 0xff00])).unwrap()),
        "終了"
    );
}
