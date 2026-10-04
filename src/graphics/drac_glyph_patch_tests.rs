use super::*;

fn synthetic_pixels(spec: DracSceneGlyphAssetSpec) -> Vec<u8> {
    let mut pixels = vec![0; DRAC_SCENE_ATLAS_LAYOUT.width * DRAC_SCENE_ATLAS_LAYOUT.height];
    for index in 0..DRAC_SCENE_GLYPH_SLOTS {
        if (spec.uniform_zero_start..spec.uniform_zero_end).contains(&index) {
            continue;
        }
        let (x, y) = cell_origin(index).unwrap();
        for row in 0..DRAC_GLYPH_CELL_HEIGHT {
            pixels[(y + row) * DRAC_SCENE_ATLAS_LAYOUT.width + x
                ..(y + row) * DRAC_SCENE_ATLAS_LAYOUT.width + x + DRAC_GLYPH_CELL_WIDTH]
                .fill(1);
        }
        pixels[y * DRAC_SCENE_ATLAS_LAYOUT.width + x] = 15;
    }
    pixels
}

#[test]
fn 두_원본cell_shape의_binary와_uniform_zero분모를_구분한다() {
    for spec in BOX_DRAC_SCENE_GLYPH_ASSET_SPECS {
        let pixels = synthetic_pixels(spec);
        let (binary, zero, palette) = audit_source_cells(&pixels, spec).unwrap();
        assert_eq!(zero, spec.uniform_zero_end - spec.uniform_zero_start);
        assert_eq!(binary + zero, DRAC_SCENE_GLYPH_SLOTS);
        assert_eq!(palette, vec![0, 1, 15]);
    }
}

#[test]
fn glyph_patch는_배정한_한_cell밖을_바꾸지_않는다() {
    let spec = BOX_DRAC_SCENE_GLYPH_ASSET_SPECS[0];
    let pixels = synthetic_pixels(spec);
    let mapping = [SubgameSeqDracGlyphMapping {
        glyph: '가',
        repertoire_index: 3,
        pair: [0x82, 0xA4],
    }];
    let (patched, changed) = patch_pixels(&pixels, &mapping, DRAC_POC_RASTER_PROFILE).unwrap();
    assert!(changed > 0);
    for index in 0..DRAC_SCENE_GLYPH_SLOTS {
        let before = cell_pixels(&pixels, index).unwrap().collect::<Vec<_>>();
        let after = cell_pixels(&patched, index).unwrap().collect::<Vec<_>>();
        if index == 3 {
            assert_ne!(before, after);
            assert_eq!(
                after.into_iter().collect::<BTreeSet<_>>(),
                BTreeSet::from([1, 15])
            );
        } else {
            assert_eq!(before, after);
        }
    }
}

#[test]
fn glyph_patch는_중복과_범위밖과_비한글_mapping을_거부한다() {
    let duplicate = [
        SubgameSeqDracGlyphMapping {
            glyph: '가',
            repertoire_index: 0,
            pair: [1, 2],
        },
        SubgameSeqDracGlyphMapping {
            glyph: '나',
            repertoire_index: 0,
            pair: [3, 4],
        },
    ];
    assert!(validate_mappings(&duplicate)
        .unwrap_err()
        .to_string()
        .contains("중복"));
    let invalid = [SubgameSeqDracGlyphMapping {
        glyph: 'A',
        repertoire_index: DRAC_SCENE_GLYPH_SLOTS,
        pair: [1, 2],
    }];
    assert!(validate_mappings(&invalid).is_err());
}
