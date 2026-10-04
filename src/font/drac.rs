//! DRAC scene-local 16×16 glyph용 Maplestory Light raster profile.
//!
//! 원본의 비어 있지 않은 text cell은 palette index `1` 배경과 `15` 본문만
//! 사용한다. 이 모듈은 font/source와 size/baseline/effect를 별도 profile로
//! 유지하고, cell 밖 픽셀을 잘라내는 대신 실패한다.

use std::error::Error;
use std::fmt;

use fontdue::Font;

use crate::font::MAPLESTORY_LIGHT_FONT_ID;

pub const DRAC_GLYPH_CELL_WIDTH: usize = 16;
pub const DRAC_GLYPH_CELL_HEIGHT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DracRasterEffect {
    Binary {
        threshold: u8,
        background: u8,
        foreground: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DracRasterProfile {
    pub id: &'static str,
    pub font_id: &'static str,
    pub font_size: f32,
    pub baseline: i32,
    pub effect: DracRasterEffect,
}

/// 원본의 binary palette 역할을 유지하는 첫 DRAC PoC profile.
pub const DRAC_POC_RASTER_PROFILE: DracRasterProfile = DracRasterProfile {
    id: "drac-scene-message-16x16",
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 14.0,
    baseline: 13,
    effect: DracRasterEffect::Binary {
        threshold: 96,
        background: 1,
        foreground: 15,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DracFontError {
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

impl fmt::Display for DracFontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfile => f.write_str("DRAC raster profile이 유효하지 않음"),
            Self::EmptyGlyph(glyph) => {
                write!(f, "DRAC font가 빈 글리프를 반환: {glyph}")
            }
            Self::GlyphDoesNotFit {
                glyph,
                x,
                y,
                width,
                height,
            } => write!(
                f,
                "DRAC 16x16 cell에 글리프가 들어가지 않음: {glyph}, origin=({x},{y}), bitmap={width}x{height}"
            ),
        }
    }
}

impl Error for DracFontError {}

/// 한 글자를 DRAC의 16×16 indexed4 palette cell로 만든다.
pub fn rasterize_drac_glyph(
    font: &Font,
    glyph: char,
    profile: DracRasterProfile,
) -> Result<Vec<u8>, DracFontError> {
    let DracRasterEffect::Binary {
        threshold,
        background,
        foreground,
    } = profile.effect;
    if profile.id.is_empty()
        || profile.font_id.is_empty()
        || !profile.font_size.is_finite()
        || profile.font_size <= 0.0
        || profile.baseline < 0
        || threshold == 0
        || background > 0x0F
        || foreground > 0x0F
        || background == foreground
    {
        return Err(DracFontError::InvalidProfile);
    }
    let (metrics, bitmap) = font.rasterize(glyph, profile.font_size);
    if metrics.width == 0 || metrics.height == 0 || bitmap.iter().all(|&coverage| coverage == 0) {
        return Err(DracFontError::EmptyGlyph(glyph));
    }
    let width = i32::try_from(metrics.width).map_err(|_| DracFontError::GlyphDoesNotFit {
        glyph,
        x: 0,
        y: 0,
        width: metrics.width,
        height: metrics.height,
    })?;
    let height = i32::try_from(metrics.height).map_err(|_| DracFontError::GlyphDoesNotFit {
        glyph,
        x: 0,
        y: 0,
        width: metrics.width,
        height: metrics.height,
    })?;
    let x = (i32::try_from(DRAC_GLYPH_CELL_WIDTH).expect("cell width는 i32 범위") - width) / 2;
    let y = profile.baseline - metrics.ymin - height;
    if x < 0
        || y < 0
        || x + width > i32::try_from(DRAC_GLYPH_CELL_WIDTH).expect("cell width는 i32 범위")
        || y + height > i32::try_from(DRAC_GLYPH_CELL_HEIGHT).expect("cell height는 i32 범위")
    {
        return Err(DracFontError::GlyphDoesNotFit {
            glyph,
            x,
            y,
            width: metrics.width,
            height: metrics.height,
        });
    }

    let mut pixels = vec![background; DRAC_GLYPH_CELL_WIDTH * DRAC_GLYPH_CELL_HEIGHT];
    let x = usize::try_from(x).expect("음수가 아님");
    let y = usize::try_from(y).expect("음수가 아님");
    for row in 0..metrics.height {
        for column in 0..metrics.width {
            if bitmap[row * metrics.width + column] >= threshold {
                pixels[(y + row) * DRAC_GLYPH_CELL_WIDTH + x + column] = foreground;
            }
        }
    }
    Ok(pixels)
}

#[cfg(test)]
#[path = "drac_tests.rs"]
mod drac_tests;
