use super::*;

#[test]
fn 최소_길이와_가나_포함을_모두_요구한다() {
    let raw = [
        0x00, 0x82, 0xA0, 0x82, 0xA2, 0x82, 0xA4, 0x00, // あいう
        0x83, 0x41, 0x83, 0x43, 0x00, // アイ: 2자는 제외
        0x88, 0xEA, 0x8A, 0xBF, 0x8E, 0x9A, 0x00, // 한자만: 제외
    ];
    let runs = scan_kana_runs(&raw, 3);

    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].offset, 1);
    assert_eq!(runs[0].raw, &raw[1..7]);
    assert_eq!(runs[0].hiragana, 3);
    assert_eq!(runs[0].katakana, 0);
}

#[test]
fn 끝까지_이어진_run과_홀수_tail을_보존한다() {
    let raw = [0x82, 0xA0, 0x82, 0xA2, 0x82, 0xA4, 0xFF];
    let runs = scan_kana_runs(&raw, 3);

    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].raw, &raw[..6]);
}
