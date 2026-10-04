use super::*;

fn mapping(glyph: char, cell_index: usize, pair: [u8; 2]) -> SubgameSeqE6GlyphMapping {
    SubgameSeqE6GlyphMapping {
        glyph,
        cell_index,
        pair,
    }
}

#[test]
fn glyph_patch는_배정한_앞256_cell만_바꾼다() {
    let mut pixels = vec![0; DEMO_FONT_LAYOUT.width * DEMO_FONT_LAYOUT.height];
    let target = DemoFontSlot::for_index(37).unwrap();
    for row in target.y..target.y + target.height {
        pixels[row * DEMO_FONT_LAYOUT.width + target.x
            ..row * DEMO_FONT_LAYOUT.width + target.x + target.width]
            .fill(15);
    }
    let (patched, changed) = patch_pixels(&pixels, &[mapping('가', 37, [0x82, 0xA0])]).unwrap();
    assert!(changed > 0);
    for index in 0..crate::font::DEMO_FONT_SLOT_COUNT {
        let slot = DemoFontSlot::for_index(index).unwrap();
        let before = (slot.y..slot.y + slot.height)
            .flat_map(|row| {
                pixels[row * DEMO_FONT_LAYOUT.width + slot.x
                    ..row * DEMO_FONT_LAYOUT.width + slot.x + slot.width]
                    .iter()
                    .copied()
            })
            .collect::<Vec<_>>();
        let after = (slot.y..slot.y + slot.height)
            .flat_map(|row| {
                patched[row * DEMO_FONT_LAYOUT.width + slot.x
                    ..row * DEMO_FONT_LAYOUT.width + slot.x + slot.width]
                    .iter()
                    .copied()
            })
            .collect::<Vec<_>>();
        if index == 37 {
            assert_ne!(before, after);
            assert_eq!(
                after.into_iter().collect::<BTreeSet<_>>(),
                BTreeSet::from([0, 1, 15])
            );
        } else {
            assert_eq!(before, after);
        }
    }
}

#[test]
fn glyph_patch는_중복_소비범위밖_비한글을_거부한다() {
    let duplicate = [
        mapping('가', 0, [0x82, 0xA0]),
        mapping('나', 0, [0x82, 0xA2]),
    ];
    assert!(validate_mappings(&duplicate)
        .unwrap_err()
        .to_string()
        .contains("중복"));
    assert!(
        validate_mappings(&[mapping('가', PUYO2_E6_DEMO_FONT_MAPPED_CELLS, [0x82, 0xA0],)])
            .unwrap_err()
            .to_string()
            .contains("소비 범위 밖")
    );
    assert!(validate_mappings(&[mapping('A', 0, [0x82, 0xA0])]).is_err());
}

#[test]
fn geometry_audit은_넓은한글을_정규화하고_세로형한글은_보존한다() {
    let audit = audit_box_puyo2_e6_demo_font_geometry(&[
        mapping('가', 0, [0x82, 0xA0]),
        mapping('왕', 1, [0x82, 0xA2]),
    ])
    .unwrap();

    assert_eq!(audit.glyphs, 2);
    assert_eq!(audit.normalized_wide_hangul, 1);
    assert_eq!(audit.squat_foreground_glyphs, 0);
    assert_eq!(
        (
            audit.minimum_foreground_width,
            audit.maximum_foreground_width,
            audit.minimum_foreground_height,
            audit.maximum_foreground_height,
        ),
        (11, 12, 12, 14)
    );
}
