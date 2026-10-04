//! `MM_MES.BTX` 16×16 synthetic storage stress용 Maplestory Light profile.
//!
//! 다른 16×16 surface의 binary 효과를 빌려 쓰지 않는다. 원본 `MM_MES`는
//! 여러 palette index를 쓰므로 coverage를 전용 ramp로 양자화하고, 넓은
//! 한글은 아래 기준선을 유지한 채 정사각형으로 보정한다. 실제 자산은
//! baked-text surface이며 이 profile은 제품의 문구 layout을 나타내지 않는다.

use std::error::Error;
use std::fmt;

use fontdue::Font;

use crate::font::MAPLESTORY_LIGHT_FONT_ID;

pub const MM_MES_STRESS_CELL_WIDTH: usize = 16;
pub const MM_MES_STRESS_CELL_HEIGHT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmMesStorageStressRasterEffect {
    CoverageRamp { transparent: u8, ramp: [u8; 13] },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MmMesStorageStressRasterProfile {
    pub id: &'static str,
    pub font_id: &'static str,
    pub font_size: f32,
    pub baseline: i32,
    /// 넓은 한글의 세로 압축을 막되 아래 기준선은 유지한다.
    pub normalize_wide_hangul: bool,
    pub effect: MmMesStorageStressRasterEffect,
}

/// 승인 전 `MM_MES` 전용 합성 profile.
pub const MM_MES_STORAGE_STRESS_RASTER_PROFILE: MmMesStorageStressRasterProfile =
    MmMesStorageStressRasterProfile {
        id: "classic-mm-mes-16x16-coverage-ramp",
        font_id: MAPLESTORY_LIGHT_FONT_ID,
        font_size: 14.0,
        baseline: 13,
        normalize_wide_hangul: true,
        effect: MmMesStorageStressRasterEffect::CoverageRamp {
            transparent: 0,
            ramp: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13],
        },
    };

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MmMesStorageStressFontError {
    InvalidProfile,
    EmptyGlyph(char),
    GlyphDoesNotFit {
        glyph: char,
        x: i32,
        y: i32,
        width: usize,
        height: usize,
    },
}

impl fmt::Display for MmMesStorageStressFontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfile => f.write_str("MM_MES raster profile이 유효하지 않음"),
            Self::EmptyGlyph(glyph) => write!(f, "MM_MES font가 빈 글리프를 반환: {glyph}"),
            Self::GlyphDoesNotFit {
                glyph,
                x,
                y,
                width,
                height,
            } => write!(
                f,
                "MM_MES 16x16 cell에 글리프가 들어가지 않음: {glyph}, \
                 origin=({x},{y}), bitmap={width}x{height}"
            ),
        }
    }
}

impl Error for MmMesStorageStressFontError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PixelBounds {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
}

impl PixelBounds {
    fn width(self) -> usize {
        self.max_x - self.min_x + 1
    }

    fn height(self) -> usize {
        self.max_y - self.min_y + 1
    }
}

fn coverage_bounds(layer: &[u8]) -> Option<PixelBounds> {
    let mut bounds: Option<PixelBounds> = None;
    for (offset, coverage) in layer.iter().copied().enumerate() {
        if coverage == 0 {
            continue;
        }
        let x = offset % MM_MES_STRESS_CELL_WIDTH;
        let y = offset / MM_MES_STRESS_CELL_WIDTH;
        bounds = Some(match bounds {
            None => PixelBounds {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            },
            Some(previous) => PixelBounds {
                min_x: previous.min_x.min(x),
                min_y: previous.min_y.min(y),
                max_x: previous.max_x.max(x),
                max_y: previous.max_y.max(y),
            },
        });
    }
    bounds
}

fn normalize_wide_hangul_coverage(
    glyph: char,
    layer: &mut Vec<u8>,
) -> Result<(), MmMesStorageStressFontError> {
    if !matches!(glyph, '\u{ac00}'..='\u{d7a3}') {
        return Ok(());
    }
    let bounds = coverage_bounds(layer).ok_or(MmMesStorageStressFontError::EmptyGlyph(glyph))?;
    if bounds.width() <= bounds.height() {
        return Ok(());
    }
    let target_height = bounds.width();
    let target_min_y = bounds
        .max_y
        .checked_add(1)
        .and_then(|bottom| bottom.checked_sub(target_height))
        .ok_or(MmMesStorageStressFontError::GlyphDoesNotFit {
            glyph,
            x: i32::try_from(bounds.min_x).unwrap_or(i32::MAX),
            y: -1,
            width: bounds.width(),
            height: bounds.height(),
        })?;
    let mut normalized = vec![0; layer.len()];
    for target_row in 0..target_height {
        let source_row = bounds.min_y
            + ((target_row * 2 + 1) * bounds.height() / (target_height * 2))
                .min(bounds.height() - 1);
        for x in bounds.min_x..=bounds.max_x {
            normalized[(target_min_y + target_row) * MM_MES_STRESS_CELL_WIDTH + x] =
                layer[source_row * MM_MES_STRESS_CELL_WIDTH + x];
        }
    }
    *layer = normalized;
    Ok(())
}

fn validate_profile(
    profile: MmMesStorageStressRasterProfile,
) -> Result<(u8, [u8; 13]), MmMesStorageStressFontError> {
    let MmMesStorageStressRasterEffect::CoverageRamp { transparent, ramp } = profile.effect;
    let ramp_is_strictly_ascending = ramp.windows(2).all(|pair| pair[0] < pair[1]);
    if profile.id.is_empty()
        || profile.font_id.is_empty()
        || !profile.font_size.is_finite()
        || profile.font_size <= 0.0
        || profile.baseline < 0
        || transparent > 0x0F
        || ramp
            .iter()
            .any(|&value| value > 0x0F || value == transparent)
        || !ramp_is_strictly_ascending
    {
        return Err(MmMesStorageStressFontError::InvalidProfile);
    }
    Ok((transparent, ramp))
}

/// 한 글자를 `MM_MES` 전용 16×16 coverage-ramp cell로 만든다.
pub fn rasterize_mm_mes_storage_stress_glyph(
    font: &Font,
    glyph: char,
    profile: MmMesStorageStressRasterProfile,
) -> Result<Vec<u8>, MmMesStorageStressFontError> {
    let (transparent, ramp) = validate_profile(profile)?;
    if glyph.is_whitespace() {
        return Ok(vec![
            transparent;
            MM_MES_STRESS_CELL_WIDTH * MM_MES_STRESS_CELL_HEIGHT
        ]);
    }
    let (metrics, bitmap) = font.rasterize(glyph, profile.font_size);
    if metrics.width == 0 || metrics.height == 0 || bitmap.iter().all(|&value| value == 0) {
        return Err(MmMesStorageStressFontError::EmptyGlyph(glyph));
    }
    let width =
        i32::try_from(metrics.width).map_err(|_| MmMesStorageStressFontError::GlyphDoesNotFit {
            glyph,
            x: 0,
            y: 0,
            width: metrics.width,
            height: metrics.height,
        })?;
    let height = i32::try_from(metrics.height).map_err(|_| {
        MmMesStorageStressFontError::GlyphDoesNotFit {
            glyph,
            x: 0,
            y: 0,
            width: metrics.width,
            height: metrics.height,
        }
    })?;
    let x = (i32::try_from(MM_MES_STRESS_CELL_WIDTH).expect("cell width는 i32 범위") - width) / 2;
    let y = profile.baseline - metrics.ymin - height;
    if x < 0
        || y < 0
        || x + width > i32::try_from(MM_MES_STRESS_CELL_WIDTH).expect("cell width는 i32 범위")
        || y + height > i32::try_from(MM_MES_STRESS_CELL_HEIGHT).expect("cell height는 i32 범위")
    {
        return Err(MmMesStorageStressFontError::GlyphDoesNotFit {
            glyph,
            x,
            y,
            width: metrics.width,
            height: metrics.height,
        });
    }

    let x = usize::try_from(x).expect("음수가 아님");
    let y = usize::try_from(y).expect("음수가 아님");
    let mut coverage = vec![0; MM_MES_STRESS_CELL_WIDTH * MM_MES_STRESS_CELL_HEIGHT];
    for row in 0..metrics.height {
        for column in 0..metrics.width {
            coverage[(y + row) * MM_MES_STRESS_CELL_WIDTH + x + column] =
                bitmap[row * metrics.width + column];
        }
    }
    if coverage_bounds(&coverage).is_none() {
        return Err(MmMesStorageStressFontError::EmptyGlyph(glyph));
    }
    if profile.normalize_wide_hangul {
        normalize_wide_hangul_coverage(glyph, &mut coverage)?;
    }

    Ok(coverage
        .into_iter()
        .map(|value| {
            if value == 0 {
                transparent
            } else {
                let index = usize::from(value - 1) * ramp.len() / 255;
                ramp[index.min(ramp.len() - 1)]
            }
        })
        .collect())
}

#[cfg(test)]
#[path = "mm_mes_storage_stress_tests.rs"]
mod tests;
