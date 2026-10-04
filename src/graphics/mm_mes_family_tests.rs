use super::*;
use crate::btx::encode;

#[test]
fn pixel_difference는_최소_bbox와_변경수를_함께_센다() {
    let layout = Layout {
        width: 8,
        height: 4,
        format: PixelFormat::Indexed4,
    };
    let first = vec![0u8; 32];
    let mut second = first.clone();
    for (x, y) in [(2, 1), (5, 1), (3, 2)] {
        second[y * layout.width + x] = 7;
    }
    let first = encode(&first, layout).unwrap();
    let second = encode(&second, layout).unwrap();

    assert_eq!(
        pixel_difference(&first, &second, layout),
        Ok((
            3,
            MmMesDifferenceRect {
                x: 2,
                y: 1,
                width: 4,
                height: 2,
            }
        ))
    );
}

#[test]
fn 같은_두_surface를_variant로_승격하지_않는다() {
    let layout = Layout {
        width: 4,
        height: 2,
        format: PixelFormat::Indexed4,
    };
    let raw = [0u8; 4];
    assert!(pixel_difference(&raw, &raw, layout).is_err());
}

#[test]
fn cell은_16x16_row_major로_잘라낸다() {
    let mut pixels = vec![0u8; MM_MES_LAYOUT.width * MM_MES_LAYOUT.height];
    let cell = 35;
    let x = (cell % MM_MES_STORAGE_COLUMNS) * MM_MES_STORAGE_CELL_WIDTH;
    let y = (cell / MM_MES_STORAGE_COLUMNS) * MM_MES_STORAGE_CELL_HEIGHT;
    for row in 0..MM_MES_STORAGE_CELL_HEIGHT {
        pixels[(y + row) * MM_MES_LAYOUT.width + x] = (row + 1) as u8;
    }

    let actual = cell_pixels(&pixels, cell).unwrap();
    assert_eq!(actual.len(), 256);
    for row in 0..MM_MES_STORAGE_CELL_HEIGHT {
        assert_eq!(actual[row * MM_MES_STORAGE_CELL_WIDTH], (row + 1) as u8);
        assert!(
            actual[row * MM_MES_STORAGE_CELL_WIDTH + 1..(row + 1) * MM_MES_STORAGE_CELL_WIDTH]
                .iter()
                .all(|&value| value == 0)
        );
    }
}

#[test]
fn cell_범위와_pixel_길이는_fail_closed다() {
    let pixels = vec![0u8; MM_MES_LAYOUT.width * MM_MES_LAYOUT.height];
    assert!(cell_pixels(&pixels, MM_MES_STORAGE_CELLS).is_err());
    assert!(cell_pixels(&pixels[..pixels.len() - 1], 0).is_err());
}

#[test]
fn palette_histogram은_16개_index의_픽셀수를_센다() {
    let mut pixels = vec![0u8; MM_MES_LAYOUT.width * MM_MES_LAYOUT.height];
    pixels[0] = 3;
    pixels[1] = 3;
    pixels[2] = 15;
    let plain = encode(&pixels, MM_MES_LAYOUT).unwrap();
    let histogram = palette_histogram(&plain).unwrap();
    assert_eq!(histogram[0], pixels.len() - 3);
    assert_eq!(histogram[3], 2);
    assert_eq!(histogram[15], 1);
    assert_eq!(histogram.iter().sum::<usize>(), pixels.len());
}

#[test]
fn baked_band_점유구간은_5px_이하_내부공백만_합친다() {
    let layout = Layout {
        width: 24,
        height: 4,
        format: PixelFormat::Indexed4,
    };
    let mut pixels = vec![0u8; layout.width * layout.height];
    pixels[1..4].fill(1);
    pixels[layout.width + 9..layout.width + 12].fill(1);
    pixels[18..21].fill(1);

    assert_eq!(
        occupied_runs(&pixels, layout, 2, 6).unwrap(),
        vec![
            vec![
                MmMesHorizontalRun {
                    start_x: 1,
                    end_x: 12,
                },
                MmMesHorizontalRun {
                    start_x: 18,
                    end_x: 21,
                },
            ],
            vec![],
        ]
    );
}

#[test]
fn baked_band_입력경계는_fail_closed다() {
    let layout = Layout {
        width: 8,
        height: 4,
        format: PixelFormat::Indexed4,
    };
    let pixels = vec![0u8; layout.width * layout.height];
    assert!(occupied_runs(&pixels[..pixels.len() - 1], layout, 2, 6).is_err());
    assert!(occupied_runs(&pixels, layout, 0, 6).is_err());
    assert!(occupied_runs(&pixels, layout, 3, 6).is_err());
    assert!(occupied_runs(&pixels, layout, 2, 0).is_err());
}
