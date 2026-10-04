use super::*;

// 모든 문자 셀은 서로 겹치지 않고 y=96 배지·숫자 행을 침범하지 않아야 한다.
#[test]
fn text_cells_do_not_overlap_or_touch_badge_row() {
    let mut pixels = BTreeSet::new();
    for cell in 24..128 {
        let (x, y) = cell_origin(cell).unwrap();
        for row in y..y + 16 {
            for column in x..x + 16 {
                assert!(!(96..112).contains(&row));
                assert!(column < 256 && row < 128 && pixels.insert((column, row)));
            }
        }
    }
    assert!(cell_origin(23).is_err());
    assert!(cell_origin(128).is_err());
}

// 공백이 이전 일본어를 남기지 않고, 없는 문자를 대체 글자로 성공시키지 않는다.
#[test]
fn blank_is_transparent_and_missing_glyph_fails() {
    let font = bundled_maplestory_light().unwrap();
    assert_eq!(rasterize(&font, ' ').unwrap(), vec![0; 256]);
    assert!(rasterize(&font, '\u{10ffff}').is_err());
    let pixels = rasterize(&font, '꽂').unwrap();
    assert!(pixels.iter().all(|&v| v <= 13));
    assert!(pixels.iter().any(|&v| v != 0));
}
