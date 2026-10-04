use super::*;

#[test]
fn pair_table은_종결을_요구하고_opaque를_분리한다() {
    let raw = [0x82, 0xA0, 0x81, 0x49, 0x65, 0x65];
    let parsed = parse_repertoire(&raw, 0, 4).unwrap();
    assert_eq!(parsed.raw, raw[..4]);
    assert_eq!(parsed.strict_sjis_pairs, 2);
    assert_eq!(parsed.opaque_pairs, 0);

    let mut missing_end = raw;
    missing_end[4] = 0;
    assert!(parse_repertoire(&missing_end, 0, 4).is_err());

    let invalid = [0x82, 0x20, 0x65, 0x65];
    let parsed = parse_repertoire(&invalid, 0, 2).unwrap();
    assert_eq!(parsed.strict_sjis_pairs, 0);
    assert_eq!(parsed.opaque_pairs, 1);
}

#[test]
fn compound_message는_control과_first_match_glyph를_분리한다() {
    let prefix = [[0x82, 0xA0], [0x82, 0xA2]];
    let raw = [
        4, 9, 10, 3, 0x82, 0xA0, 0x72, 0x72, 0x77, 0x1A, 0x82, 0xA2, 0x65, 0x65,
    ];
    let parsed = parse_compound_message(&raw, 0, &prefix).unwrap();
    assert_eq!(parsed.end_offset, raw.len());
    assert_eq!(parsed.glyph_tokens, 2);
    assert_eq!(parsed.control_tokens, 3);

    let missing = [[0x82, 0xA0]];
    assert!(parse_compound_message(&raw, 0, &missing).is_err());
}

#[test]
fn cell_비교는_선언한_grid_prefix만_센다() {
    let mut first = vec![0u8; BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES];
    let mut second = first.clone();
    assert_eq!(count_equal_grid_cells(&first, &second, 2).unwrap(), 2);

    let x = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X + BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH);
    let y = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y);
    let byte = y * (usize::from(BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH) / 2) + x / 2;
    second[byte] = 0x0F;
    assert_eq!(count_equal_grid_cells(&first, &second, 2).unwrap(), 1);

    first.pop();
    assert!(count_equal_grid_cells(&first, &second, 1).is_err());
}
