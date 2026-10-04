use super::*;

#[test]
fn 타이틀은_저장_크기와_실제_표시_경계를_구분한다() {
    let mut pixels = vec![0; 256 * 80];
    pixels[55 * 256 + 254] = 15;
    assert!(verify_logo_visible_pixels(&pixels).is_ok());
    pixels[56 * 256] = 15;
    assert!(verify_logo_visible_pixels(&pixels).is_err());
    pixels[56 * 256] = 0;
    pixels[255] = 15;
    assert!(verify_logo_visible_pixels(&pixels).is_err());
}

#[test]
fn 옵션_여덟픽셀_글리프가_셀_안에_표시된다() {
    let font = super::super::common_chain_surface::bundled_dalmoori().unwrap();
    for text in ["규칙 변경", "상쇄", "연쇄 제한"] {
        let rect = Rect {
            x: 0,
            y: 0,
            width: 96,
            height: 8,
        };
        let mut pixels = vec![0; 96 * 8];
        paint_label(&mut pixels, 96, rect, text, &font, 8.0, 7).unwrap();
        assert!(pixels.contains(&15));
        assert!(pixels.iter().all(|p| [0, 15].contains(p)));
    }
}

#[test]
fn 작은_셀에_긴_문구를_자르지_않는다() {
    let font = super::super::common_chain_surface::bundled_dalmoori().unwrap();
    let mut pixels = vec![0; 64];
    assert!(paint_label(
        &mut pixels,
        8,
        Rect {
            x: 0,
            y: 0,
            width: 8,
            height: 8
        },
        "규칙 변경",
        &font,
        8.0,
        7
    )
    .is_err());
}
