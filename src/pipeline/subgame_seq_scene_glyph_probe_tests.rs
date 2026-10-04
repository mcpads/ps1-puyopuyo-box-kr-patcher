use super::*;

#[test]
fn probe는_지정한_cell만_반복_한글로_바꾼다() {
    let plain = vec![0x11; BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES];
    let patch =
        patch_box_subgame_seq_scene_glyph_probe_asset("MZ_TEST.BTX", &plain, &[0, 79], '가')
            .unwrap();
    assert_eq!(patch.repertoire_indices, [0, 79]);
    assert_eq!(patch.cell_indices, [30, 109]);
    assert_eq!(patch.source_palette_indices, [1]);
    assert!(patch.changed_pixels > 0);
    assert!(patch.changed_bytes > 0);

    let before = decode(&plain, SCENE_GLYPH_ATLAS_LAYOUT).unwrap();
    let after = decode(&patch.plain, SCENE_GLYPH_ATLAS_LAYOUT).unwrap();
    for (offset, (&left, &right)) in before.iter().zip(&after).enumerate() {
        if left == right {
            continue;
        }
        let x = offset % SCENE_GLYPH_ATLAS_LAYOUT.width;
        let y = offset / SCENE_GLYPH_ATLAS_LAYOUT.width;
        let in_first = (640..656).contains(&x) && (0..16).contains(&y);
        let in_last = (752..768).contains(&x) && (144..160).contains(&y);
        assert!(in_first || in_last, "비대상 pixel 변경: ({x},{y})");
    }
}

#[test]
fn probe는_범위_중복_비한글_형상_오류를_거부한다() {
    let plain = vec![0x11; BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES];
    assert!(patch_box_subgame_seq_scene_glyph_probe_asset(
        "MZ_TEST.BTX",
        &plain,
        &[BOX_SUBGAME_SEQ_SCENE_GLYPH_PROBE_MAX_SLOTS],
        '가'
    )
    .is_err());
    assert!(
        patch_box_subgame_seq_scene_glyph_probe_asset("MZ_TEST.BTX", &plain, &[0, 0], '가')
            .is_err()
    );
    assert!(
        patch_box_subgame_seq_scene_glyph_probe_asset("MZ_TEST.BTX", &plain, &[0], 'あ').is_err()
    );
    assert!(patch_box_subgame_seq_scene_glyph_probe_asset(
        "MZ_TEST.BTX",
        &plain[..plain.len() - 1],
        &[0],
        '가'
    )
    .is_err());
}
