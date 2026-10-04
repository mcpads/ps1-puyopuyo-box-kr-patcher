use super::*;

fn place(vram: &mut [u8], data: &[u8], row_bytes: usize, x_words: usize, y: usize) {
    for (row, source) in data.chunks_exact(row_bytes).enumerate() {
        let start = (y + row) * VRAM_ROW_BYTES + x_words * BYTES_PER_WORD;
        vram[start..start + row_bytes].copy_from_slice(source);
    }
}

#[test]
fn indexed8_exact_행간격으로_폭을_확정한다() {
    let data = (0..48).map(|value| value * 5 + 1).collect::<Vec<u8>>();
    let mut vram = vec![0; VRAM_BYTE_LEN];
    place(&mut vram, &data, 16, 9, 7);

    let (layout, placements) =
        infer_unique_vram_layout(&data, PixelFormat::Indexed8, &vram).unwrap();
    assert_eq!(
        layout,
        Layout {
            width: 16,
            height: 3,
            format: PixelFormat::Indexed8,
        }
    );
    assert!(placements.contains(&VramPlacement {
        layout,
        x_words: 9,
        y: 7,
    }));
}

#[test]
fn indexed4의_vram_word를_네_픽셀로_환산한다() {
    let data = (0..32)
        .map(|value| (value * 7 + 3) as u8)
        .collect::<Vec<_>>();
    let mut vram = vec![0; VRAM_BYTE_LEN];
    place(&mut vram, &data, 8, 13, 11);

    let (layout, placements) =
        infer_unique_vram_layout(&data, PixelFormat::Indexed4, &vram).unwrap();
    assert_eq!(
        layout,
        Layout {
            width: 16,
            height: 4,
            format: PixelFormat::Indexed4,
        }
    );
    assert!(placements.contains(&VramPlacement {
        layout,
        x_words: 13,
        y: 11,
    }));
}

#[test]
fn 서로_다른_직사각형이_성립하면_확정을_거부한다() {
    let data = [0x11, 0x22, 0x33, 0x44];
    let mut vram = vec![0; VRAM_BYTE_LEN];
    place(&mut vram, &data, 4, 3, 4);
    place(&mut vram, &data, 2, 8, 9);

    assert_eq!(
        infer_unique_vram_layout(&data, PixelFormat::Indexed8, &vram),
        Err(RuntimeLayoutError::AmbiguousLayouts {
            layouts: vec![(2, 2), (4, 1)],
        })
    );
}

#[test]
fn 전체_일치가_없으면_실패한다() {
    let data = [1, 2, 3, 4, 5, 6];
    let vram = vec![0; VRAM_BYTE_LEN];
    assert_eq!(
        infer_unique_vram_layout(&data, PixelFormat::Indexed8, &vram),
        Err(RuntimeLayoutError::NoExactMatch)
    );
}

#[test]
fn 잘린_vram과_빈_자산을_거부한다() {
    assert_eq!(
        exact_vram_placements(&[1], PixelFormat::Indexed8, &[0; 4]),
        Err(RuntimeLayoutError::InvalidVramLength {
            expected: VRAM_BYTE_LEN,
            actual: 4,
        })
    );
    assert_eq!(
        exact_vram_placements(&[], PixelFormat::Indexed8, &vec![0; VRAM_BYTE_LEN]),
        Err(RuntimeLayoutError::EmptyAsset)
    );
}
