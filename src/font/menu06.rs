//! `MENU06.BTX` 16×16 glyph용 Maplestory Light profile.
//!
//! 정적 원본 cell은 palette index `0` 투명 배경과 `15` 본문만 사용한다.
//! font 원천·크기·baseline·effect를 다른 자산과 분리하고, 잘림은 실패한다.

use std::error::Error;
use std::fmt;

use fontdue::Font;

use crate::font::MAPLESTORY_LIGHT_FONT_ID;

pub const MENU06_GLYPH_CELL_WIDTH: usize = 16;
pub const MENU06_GLYPH_CELL_HEIGHT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu06RasterEffect {
    Binary {
        threshold: u8,
        transparent: u8,
        foreground: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Menu06RasterProfile {
    pub id: &'static str,
    pub font_id: &'static str,
    pub font_size: f32,
    pub baseline: i32,
    pub effect: Menu06RasterEffect,
}

pub const MENU06_DRAFT_RASTER_PROFILE: Menu06RasterProfile = Menu06RasterProfile {
    id: "classic-menu06-16x16-binary",
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 14.0,
    baseline: 13,
    effect: Menu06RasterEffect::Binary {
        threshold: 96,
        transparent: 0,
        foreground: 15,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Menu06FontError {
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

impl fmt::Display for Menu06FontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfile => f.write_str("MENU06 raster profile이 유효하지 않음"),
            Self::EmptyGlyph(glyph) => write!(f, "MENU06 font가 빈 글리프를 반환: {glyph}"),
            Self::GlyphDoesNotFit {
                glyph,
                x,
                y,
                width,
                height,
            } => write!(
                f,
                "MENU06 16x16 cell에 글리프가 들어가지 않음: {glyph}, origin=({x},{y}), bitmap={width}x{height}"
            ),
        }
    }
}

impl Error for Menu06FontError {}

pub fn rasterize_menu06_glyph(
    font: &Font,
    glyph: char,
    profile: Menu06RasterProfile,
) -> Result<Vec<u8>, Menu06FontError> {
    let Menu06RasterEffect::Binary {
        threshold,
        transparent,
        foreground,
    } = profile.effect;
    if profile.id.is_empty()
        || profile.font_id.is_empty()
        || !profile.font_size.is_finite()
        || profile.font_size <= 0.0
        || profile.baseline < 0
        || threshold == 0
        || transparent > 0x0F
        || foreground > 0x0F
        || transparent == foreground
    {
        return Err(Menu06FontError::InvalidProfile);
    }
    let (metrics, bitmap) = font.rasterize(glyph, profile.font_size);
    if metrics.width == 0 || metrics.height == 0 || bitmap.iter().all(|value| *value == 0) {
        return Err(Menu06FontError::EmptyGlyph(glyph));
    }
    let width = i32::try_from(metrics.width).map_err(|_| Menu06FontError::GlyphDoesNotFit {
        glyph,
        x: 0,
        y: 0,
        width: metrics.width,
        height: metrics.height,
    })?;
    let height = i32::try_from(metrics.height).map_err(|_| Menu06FontError::GlyphDoesNotFit {
        glyph,
        x: 0,
        y: 0,
        width: metrics.width,
        height: metrics.height,
    })?;
    let x = (i32::try_from(MENU06_GLYPH_CELL_WIDTH).expect("cell width는 i32 범위") - width) / 2;
    let y = profile.baseline - metrics.ymin - height;
    if x < 0
        || y < 0
        || x + width > i32::try_from(MENU06_GLYPH_CELL_WIDTH).expect("cell width는 i32 범위")
        || y + height > i32::try_from(MENU06_GLYPH_CELL_HEIGHT).expect("cell height는 i32 범위")
    {
        return Err(Menu06FontError::GlyphDoesNotFit {
            glyph,
            x,
            y,
            width: metrics.width,
            height: metrics.height,
        });
    }

    let mut pixels = vec![transparent; MENU06_GLYPH_CELL_WIDTH * MENU06_GLYPH_CELL_HEIGHT];
    let x = usize::try_from(x).expect("음수가 아님");
    let y = usize::try_from(y).expect("음수가 아님");
    for row in 0..metrics.height {
        for column in 0..metrics.width {
            if bitmap[row * metrics.width + column] >= threshold {
                pixels[(y + row) * MENU06_GLYPH_CELL_WIDTH + x + column] = foreground;
            }
        }
    }
    Ok(pixels)
}

#[cfg(test)]
#[path = "menu06_tests.rs"]
mod tests;
