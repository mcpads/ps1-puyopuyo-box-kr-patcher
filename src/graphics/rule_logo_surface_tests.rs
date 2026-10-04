use super::*;
use crate::graphics::style_selection_surface::{LV_MENU2_ASSET, LV_MENU_ASSET};

#[test]
fn 로고만_합성하고_sun과_이름_난이도_셀은_보존한다() {
    for asset in [LV_MENU2_ASSET, LV_MENU_ASSET] {
        let original_pixels: Vec<_> = (0..asset.layout.width * asset.layout.height)
            .map(|i| (i % 16) as u8)
            .collect();
        let before = encode(&original_pixels, asset.layout).unwrap();
        let after = compose(&before, asset).unwrap();
        let actual = decode(&after, asset.layout).unwrap();
        let cells: Vec<_> = asset
            .blocks
            .iter()
            .take(9)
            .step_by(3)
            .map(|b| b.bbox)
            .collect();
        for (i, (&old, &new)) in original_pixels.iter().zip(&actual).enumerate() {
            let (x, y) = (i % asset.layout.width, i / asset.layout.width);
            if !cells
                .iter()
                .any(|r| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height)
            {
                assert_eq!(new, old, "보호 셀 변경: {x},{y}");
            }
        }
        for r in cells {
            let cell: Vec<_> = (r.y..r.y + r.height)
                .flat_map(|y| {
                    actual[y * asset.layout.width + r.x..y * asset.layout.width + r.x + r.width]
                        .iter()
                        .copied()
                })
                .collect();
            assert!(cell.contains(&0), "투명 배경 누락");
            assert!(cell.contains(&1), "검정 외곽 누락");
        }
    }
}

#[test]
fn 임시_배경색과_누락_중복_셀을_반입하지_않는다() {
    let asset = LV_MENU2_ASSET;
    let before = vec![0; asset.layout.width * asset.layout.height / 2];
    let mut lines: Vec<_> = INPUT.lines().map(str::to_owned).collect();
    let start = lines[1].rfind('\t').unwrap() + 1;
    lines[1].replace_range(start..start + 2, "10");
    assert!(compose_cells(&before, asset, &lines.join("\n")).is_err());
    assert!(compose_cells(
        &before,
        asset,
        &INPUT.lines().take(3).collect::<Vec<_>>().join("\n")
    )
    .is_err());
    assert!(compose_cells(
        &before,
        asset,
        &format!("{INPUT}{}\n", INPUT.lines().nth(1).unwrap())
    )
    .is_err());
    assert!(compose_cells(&before, asset, &INPUT.replace("puyo2\t", "puyo\t")).is_err());
}
