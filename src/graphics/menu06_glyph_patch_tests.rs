use super::*;

#[test]
fn cell0과_16x16_grid를_분리한다() {
    assert_eq!(cell_rect(0).unwrap(), (240, 0, 8, 8));
    assert_eq!(cell_rect(1).unwrap(), (0, 0, 16, 16));
    assert_eq!(cell_rect(15).unwrap(), (224, 0, 16, 16));
    assert_eq!(cell_rect(16).unwrap(), (0, 16, 16, 16));
    assert_eq!(cell_rect(97).unwrap(), (96, 96, 16, 16));
    assert!(cell_rect(98).is_err());
}

#[test]
fn narrow_space와_full_width_blank를_분리한다() {
    let mappings = [
        Menu06GlyphMapping {
            cell: 0,
            glyph: Some(' '),
        },
        Menu06GlyphMapping {
            cell: 1,
            glyph: Some('가'),
        },
        Menu06GlyphMapping {
            cell: 2,
            glyph: None,
        },
    ];
    assert_eq!(validate_mappings(&mappings).unwrap().len(), 3);

    let duplicate_blank = [
        mappings[0],
        mappings[1],
        mappings[2],
        Menu06GlyphMapping {
            cell: 3,
            glyph: None,
        },
    ];
    assert!(validate_mappings(&duplicate_blank).is_err());
}
