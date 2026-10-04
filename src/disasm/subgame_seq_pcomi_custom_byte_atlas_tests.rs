use super::*;

fn cell(index: usize, x: u16, y: u16, width: u16, height: u16) -> PcomiCell {
    let empty = width == 0 || height == 0;
    let outside_atlas = !empty
        && (usize::from(x) + usize::from(width) > PCOMI_CUSTOM_BYTE_ATLAS_WIDTH
            || usize::from(y) + usize::from(height) > PCOMI_CUSTOM_BYTE_ATLAS_HEIGHT);
    PcomiCell {
        index,
        x,
        y,
        width,
        height,
        empty,
        outside_atlas,
    }
}

#[test]
fn resource_4는_descriptor_격자의_pcomi03을_고른다() {
    assert_eq!(PCOMI_CUSTOM_BYTE_DESCRIPTOR_RUNTIME, 0x8002_F3E8 + 4 * 36);
    assert_eq!(PCOMI03_NAME_RUNTIME, 0x8002_F6E7);
}

#[test]
fn cell_범위와_empty를_서로_다르게_분류한다() {
    assert!(!cell(1, 752, 240, 16, 16).outside_atlas);
    assert!(cell(2, 753, 240, 16, 16).outside_atlas);
    assert!(cell(3, 0, 0, 0, 0).empty);
    assert!(!cell(3, 0, 0, 0, 0).outside_atlas);
}

#[test]
fn cell_직사각형_안의_nonzero_pixel만_content로_센다() {
    let target = cell(1, 8, 4, 3, 2);
    let mut pixels = vec![0; PCOMI_CUSTOM_BYTE_ATLAS_WIDTH * PCOMI_CUSTOM_BYTE_ATLAS_HEIGHT];
    pixels[5 * PCOMI_CUSTOM_BYTE_ATLAS_WIDTH + 10] = 7;
    assert!(cell_has_nonzero_pixel(&target, &pixels));
    pixels[5 * PCOMI_CUSTOM_BYTE_ATLAS_WIDTH + 10] = 0;
    assert!(!cell_has_nonzero_pixel(&target, &pixels));
}

#[test]
fn code는_offset없이_같은_index의_cell을_요구한다() {
    let cells = vec![cell(0, 0, 0, 0, 0), cell(1, 0, 0, 1, 1)];
    let codes = BTreeSet::from([1]);
    let mut pixels = vec![0; PCOMI_CUSTOM_BYTE_ATLAS_WIDTH * PCOMI_CUSTOM_BYTE_ATLAS_HEIGHT];
    pixels[0] = 1;
    assert_eq!(validate_codes("test", &codes, &cells, &pixels).unwrap(), 1);

    let missing = BTreeSet::from([2]);
    assert!(validate_codes("test", &missing, &cells, &pixels)
        .unwrap_err()
        .to_string()
        .contains("cell 없음"));
}
