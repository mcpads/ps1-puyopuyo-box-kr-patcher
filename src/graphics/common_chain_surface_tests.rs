use super::*;

#[test]
fn 연쇄_두_글자가_sfc_채택_타일의_픽셀과_같다() {
    // SFC gameplay_rensa의 검증된 두 4bpp 타일. 주소·descriptor는 비교하지 않는다.
    let fixtures = [
        (
            '연',
            "00ff00ff00ff00ff00ff00e000ff00ff76008a008e0072000000400040007e00",
        ),
        (
            '쇄',
            "007f00ff00ff00ff00ff00ff00ff00ff2500250055008d0027002500fd000500",
        ),
    ];
    let font = load_dalmoori(DALMOORI_BYTES).unwrap();
    for (character, hex) in fixtures {
        let tile = (0..32)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
            .collect::<Vec<_>>();
        let pixels = render_chain_glyph(&font, character).unwrap();
        for y in 0..8 {
            for x in 0..8 {
                let shift = 7 - x;
                let expected = ((tile[y * 2] >> shift) & 1)
                    | (((tile[y * 2 + 1] >> shift) & 1) << 1)
                    | (((tile[16 + y * 2] >> shift) & 1) << 2)
                    | (((tile[17 + y * 2] >> shift) & 1) << 3);
                assert_eq!(pixels[y * 8 + x], expected, "{character} ({x},{y})");
            }
        }
    }
}

#[test]
fn 바뀐_글꼴과_없는_글리프를_실패시킨다() {
    let mut bytes = DALMOORI_BYTES.to_vec();
    bytes[100] ^= 1;
    assert!(load_dalmoori(&bytes).is_err());
    let font = load_dalmoori(DALMOORI_BYTES).unwrap();
    assert!(render_chain_glyph(&font, '\u{10ffff}').is_err());
}
