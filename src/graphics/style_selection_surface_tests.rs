use super::*;
use crate::graphics::text_patch::render_baked_text_block;

#[test]
fn 스타일이름의_받침과_외곽선이_16픽셀_셀을_넘지_않는다() {
    let fonts = BakedFontBook::with_selected_fonts().unwrap();
    for asset in [LV_MENU2_ASSET, LV_MENU_ASSET] {
        for (i, block) in asset.blocks[..9].iter().enumerate() {
            if i.is_multiple_of(3) {
                continue;
            }
            let word = ["뿌요", "뿌요2", "뿌요4"][i / 3];
            let pixels = render_baked_text_block(
                *block,
                asset.layout.format,
                &fonts,
                word,
                style_profile(i),
            )
            .unwrap();
            let foreground = if i % 3 == 1 { 5 } else { 15 };
            assert!(pixels.contains(&foreground), "{} 본문 누락", block.id);
            assert!(pixels.contains(&1), "{} 불투명 외곽 누락", block.id);
        }
    }
}

#[test]
fn 난이도와_팀_이름이_실제_셀에_들어가고_선택색을_유지한다() {
    let fonts = BakedFontBook::with_selected_fonts().unwrap();
    for (asset, input) in [
        (
            LV_MENU2_ASSET,
            include_str!("../../assets/translations/style-selection/lv_menu2.tsv"),
        ),
        (
            LV_MENU_ASSET,
            include_str!("../../assets/translations/style-selection/lv_menu.tsv"),
        ),
    ] {
        for (block, line) in asset.blocks.iter().zip(input.lines().skip(1)).skip(9) {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields[0], block.id);
            let pixels = render_baked_text_block(
                *block,
                asset.layout.format,
                &fonts,
                fields[2],
                selection_profile(*block),
            )
            .unwrap();
            let foreground = if block.id.ends_with("-selected") {
                9
            } else if block.id.contains("-team-") {
                15
            } else {
                7
            };
            assert!(pixels.contains(&foreground), "{} 본문 누락", block.id);
            if block.bbox.width != 24 {
                assert!(pixels.contains(&1), "{} 검정 외곽 누락", block.id);
            }
        }
    }
}
