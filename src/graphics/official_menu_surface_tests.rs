use super::*;
#[test]
fn 공식전_안내가_소비되는_폭과_두줄_경계에_들어간다() {
    for pixels in guide_pixels().unwrap() {
        assert!(pixels.contains(&13));
        assert!(pixels.iter().all(|v| [0, 1, 13].contains(v)));
    }
}
#[test]
fn 생성판의_색범위와_셀_결속_오류는_거부한다() {
    assert!(panel_pixels(PANELS).is_ok());
    assert!(panel_pixels(&PANELS.replacen("battle\t64", "records\t64", 1)).is_err());
    let mut rows: Vec<_> = PANELS.lines().map(str::to_owned).collect();
    let at = rows[1].rfind('\t').unwrap() + 1;
    rows[1].replace_range(at..at + 2, "10");
    assert!(panel_pixels(&rows.join("\n")).is_err());
}
