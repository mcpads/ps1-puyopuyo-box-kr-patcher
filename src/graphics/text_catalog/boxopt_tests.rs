use super::*;
use crate::graphics::boxopt::{BoxoptSelector, BOXOPT_PHRASES};

fn catalog_selectors() -> Vec<BoxoptSelector> {
    let mut selectors = (0..70)
        .map(|index| BoxoptSelector {
            index,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            anchor_x: 0,
            anchor_y: 0,
        })
        .collect::<Vec<_>>();
    for (phrase, block) in BOXOPT_PHRASES.iter().zip(BOXOPT_TEXT_BLOCKS.iter()) {
        selectors[phrase.selector] = BoxoptSelector {
            index: phrase.selector,
            x: block.bbox.x as u16,
            y: block.bbox.y as u16,
            width: block.bbox.width as u16,
            height: block.bbox.height as u16,
            anchor_x: 0,
            anchor_y: 0,
        };
    }
    selectors
}

#[test]
fn phrase_selector와_catalog의_bbox와_원문이_일치한다() {
    verify_boxopt_text_catalog_selectors(&catalog_selectors()).unwrap();
    assert_eq!(BOXOPT_TEXT_BLOCKS.len(), 49);
    assert_eq!(
        BOXOPT_TEXT_BLOCKS
            .iter()
            .filter(|block| is_japanese_translation_target(block.source))
            .count(),
        46
    );
}

#[test]
fn selector_bbox_drift를_거부한다() {
    let mut selectors = catalog_selectors();
    selectors[16].width += 1;
    assert!(verify_boxopt_text_catalog_selectors(&selectors).is_err());
}
