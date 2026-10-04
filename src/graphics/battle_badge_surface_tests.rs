use super::*;

#[test]
fn 생성_세_셀_밖의_휴식_상쇄_장식은_보존한다() {
    let layout = BATAN2_TEXT_ASSET.layout;
    let source = encode(&vec![7; layout.width * layout.height], layout).unwrap();
    let output = decode(&compose(&source).unwrap(), layout).unwrap();
    let mut changes = 0;
    for (i, &pixel) in output.iter().enumerate() {
        let inside = BATAN2_TEXT_ASSET.blocks[..3].iter().any(|block| {
            let r = block.bbox;
            (r.x..r.x + r.width).contains(&(i % layout.width))
                && (r.y..r.y + r.height).contains(&(i / layout.width))
        });
        if !inside {
            assert_eq!(pixel, 7);
        } else {
            changes += usize::from(pixel != 7);
        }
    }
    assert!(changes > 0);
}

#[test]
fn 원본에_없는_palette와_누락_초과_셀은_실패한다() {
    let source = vec![0; BATAN2_TEXT_ASSET.layout.byte_len().unwrap()];
    let mut invalid = PIXELS.to_owned();
    let start = invalid.lines().next().unwrap().len() + 1 + "BTXB-0107\t96\t48\t".len();
    invalid.replace_range(start..start + 2, "07");
    assert!(compose_cells(&source, &invalid)
        .unwrap_err()
        .contains("palette"));
    assert!(compose_cells(
        &source,
        &PIXELS.lines().take(3).collect::<Vec<_>>().join("\n")
    )
    .is_err());
    assert!(compose_cells(&source, &format!("{PIXELS}extra\n")).is_err());
}

#[test]
fn 상쇄는_흰_말풍선과_검은_글자이며_투명_윤곽을_보존한다() {
    let layout = BATAN2_TEXT_ASSET.layout;
    let r = BATAN2_TEXT_ASSET.blocks[4].bbox;
    let mut original = vec![7; layout.width * layout.height];
    let mut translated = original.clone();
    for y in r.y..r.y + r.height {
        for x in r.x..r.x + r.width {
            let i = y * layout.width + x;
            original[i] = 15;
            translated[i] = 15;
        }
    }
    let corner = r.y * layout.width + r.x;
    original[corner] = 0;
    translated[corner + 1] = 0;
    translated[corner + layout.width * 3 + 3] = 1;
    let raw = encode(&original, layout).unwrap();
    let out = restore_offset_background(&raw, &encode(&translated, layout).unwrap()).unwrap();
    let pixels = decode(&out, layout).unwrap();
    assert_eq!(pixels[corner], 0);
    assert_eq!(pixels[corner + 1], 15);
    assert_eq!(pixels[corner + layout.width * 3 + 3], 1);
    assert_eq!(pixels[0], 7);
    translated[corner] = 1;
    assert!(restore_offset_background(&raw, &encode(&translated, layout).unwrap()).is_err());
}

#[test]
fn 상쇄_profile은_흰_바탕에_검은_글자를_외곽선_없이_그린다() {
    use crate::graphics::btx_raster_translation_manifest::general_btx_raster_profile_assignments;
    use crate::graphics::text_profile::BakedTextEffect;
    let assignments = general_btx_raster_profile_assignments().unwrap();
    let config = assignments
        .iter()
        .find(|a| a.semantic_family == "puyo-status-branding" && a.profile_candidate_index == 9)
        .unwrap()
        .configuration
        .as_ref()
        .unwrap();
    let profile = config.baked_text_profile();
    assert_eq!(profile.background, 15);
    assert_eq!(profile.body_ramp, &[0, 1]);
    assert_eq!(profile.effect, BakedTextEffect::None);
}
