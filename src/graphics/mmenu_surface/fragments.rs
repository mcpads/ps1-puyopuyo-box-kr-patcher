//! 선택 sprite에 포함된 이웃 판은 기본 메뉴에서 파생한다.
//! 생성 시트의 반복 조각은 공급하지 않는다. 원본의 그림자 농도만 분리해 적용한다.
use super::{error, MmenuSurfaceError};

const WIDTH: usize = 640;

// (선택 시트 x/y, 폭/높이, 기본 메뉴 x/y).
// SPRT의 XY·UV·tpage와 기본 메뉴의 XY=(0,18)로 확인한 대응이다.
const NEIGHBORS: [(usize, usize, usize, usize, usize, usize); 7] = [
    (400, 0, 8, 70, 120, 25),
    (320, 70, 88, 26, 40, 95),
    (400, 96, 8, 68, 200, 33),
    (320, 164, 88, 20, 120, 101),
    (424, 0, 3, 64, 64, 97),
    (512, 0, 6, 64, 136, 97),
    (592, 12, 16, 48, 216, 109),
];

fn palette() -> Result<Vec<[u16; 3]>, MmenuSurfaceError> {
    let hex =
        include_str!("../../../assets/translations/btx/main-menu-lettering/palette.hex").trim_end();
    if hex.len() != 256 * 6 || !hex.is_ascii() {
        return Err(error("MMENU 파생 조각 팔레트 크기 불일치"));
    }
    hex.as_bytes()
        .chunks_exact(6)
        .map(|color| {
            let mut rgb = [0; 3];
            for (channel, pair) in color.chunks_exact(2).enumerate() {
                let text = std::str::from_utf8(pair).map_err(|e| error(e.to_string()))?;
                rgb[channel] = u16::from_str_radix(text, 16).map_err(|e| error(e.to_string()))?;
            }
            Ok(rgb)
        })
        .collect()
}

pub(super) fn compose(original: &[u8], atlas: &mut [u8]) -> Result<(), MmenuSurfaceError> {
    let palette = palette()?;
    // 모든 조각이 동일한 기준 그림을 읽도록 출력과 분리한다.
    let main = atlas.to_vec();
    for (dx, dy, width, height, sx, sy) in NEIGHBORS {
        let mut shade = vec![256_u16; width * height];
        for y in 0..height {
            for x in 0..width {
                let a = palette[usize::from(original[(sy + y) * WIDTH + sx + x])];
                let b = palette[usize::from(original[(dy + y) * WIDTH + dx + x])];
                let bright_a = *a.iter().max().unwrap_or(&0);
                let bright_b = *b.iter().max().unwrap_or(&0);
                if bright_a >= 80 {
                    shade[y * width + x] = (bright_b * 256 / bright_a).clamp(166, 256);
                }
            }
        }
        for y in 0..height {
            for x in 0..width {
                // 원문 획의 palette 양자화 차이를 그림자 무늬로 옮기지 않는다.
                let mut neighborhood = Vec::with_capacity(25);
                for yy in y.saturating_sub(2)..=(y + 2).min(height - 1) {
                    for xx in x.saturating_sub(2)..=(x + 2).min(width - 1) {
                        neighborhood.push(shade[yy * width + xx]);
                    }
                }
                neighborhood.sort_unstable();
                let factor = neighborhood[neighborhood.len() / 2];
                let index = main[(sy + y) * WIDTH + sx + x];
                let color = palette[usize::from(index)];
                let best = if factor == 256 || index == 0 {
                    index
                } else {
                    (1..256)
                        .min_by_key(|&candidate| {
                            palette[candidate]
                                .iter()
                                .zip(color)
                                .map(|(&a, b)| {
                                    let delta =
                                        i32::from(a) * 256 - i32::from(b) * i32::from(factor);
                                    i64::from(delta).pow(2)
                                })
                                .sum::<i64>()
                        })
                        .unwrap_or(1) as u8
                };
                atlas[(dy + y) * WIDTH + dx + x] = best;
            }
        }
    }
    Ok(())
}
