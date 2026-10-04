use super::*;

fn fixture() -> (SubgameSeqSceneGlyphRow, Vec<u8>, Vec<u8>) {
    let mut sequence = vec![0u8; 0x80];
    sequence[0x20..0x24].copy_from_slice(&[0x82, 0xA0, 0x82, 0xA2]);
    let mut plain = vec![0x11u8; BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES];
    let x = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X);
    plain[x / 2] = 0xF1;
    let sequence_sha256 = digest(&sequence);
    let plain_sha256 = digest(&plain);
    let row = SubgameSeqSceneGlyphRow {
        scope: "PUYO1",
        exe: "PUYO1.EXE",
        seq_archive: "/PUYO1/PP1_SEQ.VDD;1",
        grp_archive: "/PUYO1/PP1_GRP.VDD;1",
        sequence: "TEST_SEQ.SEQ",
        sequence_sha256,
        e2_record_offset: 0,
        e1_record_offset: 8,
        repertoire_offset: 0x20,
        repertoire_runtime: 0,
        repertoire_bytes: 4,
        repertoire_slots: 2,
        strict_sjis_pairs: 2,
        opaque_pairs: 0,
        repertoire_unique_pairs: 2,
        repertoire_sha256: digest(&sequence[0x20..0x24]),
        repertoire_copy_equal: true,
        canonical_glyph_slots: 2,
        protected_message_slots: 0,
        compound_message_records: 0,
        compound_glyph_tokens: 0,
        compound_control_tokens: 0,
        compound_variant_records: 0,
        embedded_message_record_offset: None,
        resource_id: 1,
        auxiliary_id: 2,
        auxiliary_asset: "AUX.BTX".to_string(),
        cell_setup_record_offset: 0,
        cell_table_offset: BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET,
        cell_table_runtime: BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_RUNTIME,
        cell_base: BOX_SUBGAME_SEQ_DRAC_CELL_BASE,
        cell_last: BOX_SUBGAME_SEQ_DRAC_CELL_BASE + 1,
        grid_slots: 2,
        selected_asset: "MZ_TEST.BTX".to_string(),
        selected_vdd_index: 3,
        selected_stored_bytes: 0,
        selected_cnx_subtype: *b"bi_",
        selected_plain_bytes: plain.len(),
        selected_plain_sha256: plain_sha256,
        selected_atlas_compatible: true,
        same_stem_asset: "MZ_TEST.BTX",
        same_stem_resource_id: 1,
        same_stem_vdd_index: 3,
        same_stem_stored_bytes: 0,
        same_stem_cnx_subtype: *b"bi_",
        same_stem_plain_bytes: plain.len(),
        same_stem_plain_sha256: plain_sha256,
        selected_is_same_stem: true,
        selected_same_stem_equal_grid_cells: 2,
        same_stem_copy_equal: true,
        e1_handler_runtime: 0,
        e2_handler_runtime: 0,
        pair_lookup_runtime: 0,
        renderer_runtime: 0,
    };
    (row, sequence, plain)
}

fn mapping_for(glyph: char, index: usize, pair: [u8; 2]) -> SubgameSeqSceneGlyphMapping {
    SubgameSeqSceneGlyphMapping {
        sequence: "TEST_SEQ.SEQ".to_string(),
        glyph,
        repertoire_index: index,
        pair,
    }
}

fn mapping(index: usize, pair: [u8; 2]) -> SubgameSeqSceneGlyphMapping {
    mapping_for('가', index, pair)
}

#[test]
fn scene_writer는_first_match_cell만_바꾼다() {
    let (row, sequence, plain) = fixture();
    let patch = patch_box_subgame_seq_scene_glyph_asset(
        &row,
        &sequence,
        &plain,
        &[mapping(0, [0x82, 0xA0])],
    )
    .unwrap();
    assert_eq!(patch.cell_indices, [30]);
    assert_eq!(patch.source_palette_indices, [1, 15]);
    assert!(patch.changed_pixels > 0);
    assert!(patch.changed_bytes > 0);

    let before = decode(&plain, SCENE_GLYPH_ATLAS_LAYOUT).unwrap();
    let after = decode(&patch.plain, SCENE_GLYPH_ATLAS_LAYOUT).unwrap();
    let (x, y) = cell_origin(&row, 0).unwrap();
    for (offset, (&left, &right)) in before.iter().zip(&after).enumerate() {
        let px = offset % SCENE_GLYPH_ATLAS_LAYOUT.width;
        let py = offset / SCENE_GLYPH_ATLAS_LAYOUT.width;
        if left != right {
            assert!((x..x + 16).contains(&px));
            assert!((y..y + 16).contains(&py));
        }
    }
}

#[test]
fn scene_writer는_공백에_배정한_cell을_background로_비운다() {
    let (row, sequence, plain) = fixture();
    let patch = patch_box_subgame_seq_scene_glyph_asset(
        &row,
        &sequence,
        &plain,
        &[
            mapping_for(' ', 0, [0x82, 0xA0]),
            mapping_for('가', 1, [0x82, 0xA2]),
        ],
    )
    .unwrap();
    let after = decode(&patch.plain, SCENE_GLYPH_ATLAS_LAYOUT).unwrap();
    let (x, y) = cell_origin(&row, 0).unwrap();
    assert!(
        (0..SCENE_GLYPH_CELL_HEIGHT).all(|row_offset| after[(y + row_offset)
            * SCENE_GLYPH_ATLAS_LAYOUT.width
            + x
            ..(y + row_offset) * SCENE_GLYPH_ATLAS_LAYOUT.width + x + SCENE_GLYPH_CELL_WIDTH]
            .iter()
            .all(|pixel| *pixel == 1))
    );
}

#[test]
fn scene_writer는_puyo2와_pair_sha_경계_이탈을_거부한다() {
    let (row, sequence, plain) = fixture();
    let mut puyo2 = row.clone();
    puyo2.scope = "PUYO2";
    assert!(patch_box_subgame_seq_scene_glyph_asset(
        &puyo2,
        &sequence,
        &plain,
        &[mapping(0, [0x82, 0xA0])]
    )
    .is_err());
    assert!(patch_box_subgame_seq_scene_glyph_asset(
        &row,
        &sequence,
        &plain,
        &[mapping(1, [0x82, 0xA0])]
    )
    .is_err());

    let mut drift = plain.clone();
    drift[0] ^= 1;
    assert!(patch_box_subgame_seq_scene_glyph_asset(
        &row,
        &sequence,
        &drift,
        &[mapping(0, [0x82, 0xA0])]
    )
    .is_err());
}
