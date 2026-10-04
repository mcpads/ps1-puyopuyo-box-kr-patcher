//! PUYO2 E6 `DEMOFONT.BTX` 16×16 glyph용 Maplestory Light profile.
//!
//! DRAC의 불투명 binary cell과 달리 DEMOFONT는 palette index `0`을
//! 투명 배경, `1`을 오른쪽 아래 그림자, `15`를 본문으로 사용한다. 폰트
//! 원천·크기·baseline·effect를 별도 profile로 고정하고 잘림은 실패시킨다.

use std::error::Error;
use std::fmt;

use fontdue::Font;

use crate::font::MAPLESTORY_LIGHT_FONT_ID;

pub const E6_DEMO_GLYPH_CELL_WIDTH: usize = 16;
pub const E6_DEMO_GLYPH_CELL_HEIGHT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum E6DemoRasterEffect {
    DropShadow {
        threshold: u8,
        transparent: u8,
        shadow: u8,
        foreground: u8,
        offset_x: usize,
        offset_y: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct E6DemoRasterProfile {
    pub id: &'static str,
    pub font_id: &'static str,
    pub font_size: f32,
    pub baseline: i32,
    /// 본문 폭이 높이보다 큰 한글만 아래 기준선을 유지하며 정사각형까지 늘린다.
    pub normalize_wide_hangul: bool,
    pub effect: E6DemoRasterEffect,
}

/// E6 direct-text의 첫 Maplestory Light 합성 profile.
pub const E6_DEMO_POC_RASTER_PROFILE: E6DemoRasterProfile = E6DemoRasterProfile {
    id: "puyo2-e6-demofont-16x16-shadow",
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 14.0,
    baseline: 12,
    normalize_wide_hangul: true,
    effect: E6DemoRasterEffect::DropShadow {
        threshold: 96,
        transparent: 0,
        shadow: 1,
        foreground: 15,
        offset_x: 1,
        offset_y: 1,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum E6DemoFontError {
    InvalidProfile,
    EmptyGlyph(char),
    GlyphDoesNotFit {
        glyph: char,
        x: i32,
        y: i32,
        width: usize,
        height: usize,
        effect_width: usize,
        effect_height: usize,
    },
}

impl fmt::Display for E6DemoFontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfile => f.write_str("E6 DEMOFONT raster profile이 유효하지 않음"),
            Self::EmptyGlyph(glyph) => {
                write!(f, "E6 DEMOFONT font가 빈 글리프를 반환: {glyph}")
            }
            Self::GlyphDoesNotFit {
                glyph,
                x,
                y,
                width,
                height,
                effect_width,
                effect_height,
            } => write!(
                f,
                "E6 DEMOFONT 16x16 cell에 글리프/effect가 들어가지 않음: {glyph}, \
                 origin=({x},{y}), bitmap={width}x{height}, effect={effect_width}x{effect_height}"
            ),
        }
    }
}

impl Error for E6DemoFontError {}

/// 한 셀의 원본·정규화 본문과 effect 점유 영역.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct E6DemoGlyphGeometry {
    pub source_foreground_width: usize,
    pub source_foreground_height: usize,
    pub foreground_width: usize,
    pub foreground_height: usize,
    pub effect_width: usize,
    pub effect_height: usize,
    pub normalized_wide_hangul: bool,
}

/// E6 DEMOFONT 한 셀의 indexed4 pixel과 검토 가능한 기하 정보.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct E6DemoGlyphRaster {
    pub pixels: Vec<u8>,
    pub geometry: E6DemoGlyphGeometry,
}

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

fn foreground_bounds(layer: &[bool]) -> Option<PixelBounds> {
    let points = layer
        .iter()
        .enumerate()
        .filter(|(_, occupied)| **occupied)
        .map(|(offset, _)| {
            (
                offset % E6_DEMO_GLYPH_CELL_WIDTH,
                offset / E6_DEMO_GLYPH_CELL_WIDTH,
            )
        })
        .collect::<Vec<_>>();
    Some(PixelBounds {
        min_x: points.iter().map(|(x, _)| *x).min()?,
        min_y: points.iter().map(|(_, y)| *y).min()?,
        max_x: points.iter().map(|(x, _)| *x).max()?,
        max_y: points.iter().map(|(_, y)| *y).max()?,
    })
}

fn normalize_wide_hangul_foreground(
    glyph: char,
    layer: &mut Vec<bool>,
) -> Result<(), E6DemoFontError> {
    if !matches!(glyph, '\u{ac00}'..='\u{d7a3}') {
        return Ok(());
    }
    let bounds = foreground_bounds(layer).ok_or(E6DemoFontError::EmptyGlyph(glyph))?;
    if bounds.width() <= bounds.height() {
        return Ok(());
    }
    let target_height = bounds.width();
    let target_min_y = bounds
        .max_y
        .checked_add(1)
        .and_then(|bottom| bottom.checked_sub(target_height))
        .ok_or(E6DemoFontError::GlyphDoesNotFit {
            glyph,
            x: i32::try_from(bounds.min_x).unwrap_or(i32::MAX),
            y: -1,
            width: bounds.width(),
            height: bounds.height(),
            effect_width: bounds.width(),
            effect_height: target_height,
        })?;
    let mut normalized = vec![false; layer.len()];
    for target_row in 0..target_height {
        let source_row = bounds.min_y
            + ((target_row * 2 + 1) * bounds.height() / (target_height * 2))
                .min(bounds.height() - 1);
        for x in bounds.min_x..=bounds.max_x {
            if layer[source_row * E6_DEMO_GLYPH_CELL_WIDTH + x] {
                normalized[(target_min_y + target_row) * E6_DEMO_GLYPH_CELL_WIDTH + x] = true;
            }
        }
    }
    *layer = normalized;
    Ok(())
}

/// 한 글자를 DEMOFONT의 투명+그림자 indexed4 cell로 만들고 점유 영역을 보고한다.
pub fn rasterize_e6_demo_glyph_with_geometry(
    font: &Font,
    glyph: char,
    profile: E6DemoRasterProfile,
) -> Result<E6DemoGlyphRaster, E6DemoFontError> {
    let E6DemoRasterEffect::DropShadow {
        threshold,
        transparent,
        shadow,
        foreground,
        offset_x,
        offset_y,
    } = profile.effect;
    if profile.id.is_empty()
        || profile.font_id.is_empty()
        || !profile.font_size.is_finite()
        || profile.font_size <= 0.0
        || profile.baseline < 0
        || threshold == 0
        || [transparent, shadow, foreground]
            .iter()
            .any(|value| *value > 0x0F)
        || transparent == shadow
        || transparent == foreground
        || shadow == foreground
        || offset_x == 0
        || offset_y == 0
    {
        return Err(E6DemoFontError::InvalidProfile);
    }
    let (metrics, bitmap) = font.rasterize(glyph, profile.font_size);
    if metrics.width == 0 || metrics.height == 0 || bitmap.iter().all(|&coverage| coverage == 0) {
        return Err(E6DemoFontError::EmptyGlyph(glyph));
    }
    let effect_width =
        metrics
            .width
            .checked_add(offset_x)
            .ok_or(E6DemoFontError::GlyphDoesNotFit {
                glyph,
                x: 0,
                y: 0,
                width: metrics.width,
                height: metrics.height,
                effect_width: usize::MAX,
                effect_height: metrics.height,
            })?;
    let effect_height =
        metrics
            .height
            .checked_add(offset_y)
            .ok_or(E6DemoFontError::GlyphDoesNotFit {
                glyph,
                x: 0,
                y: 0,
                width: metrics.width,
                height: metrics.height,
                effect_width,
                effect_height: usize::MAX,
            })?;
    let x = i32::try_from((E6_DEMO_GLYPH_CELL_WIDTH.saturating_sub(effect_width)) / 2)
        .expect("cell width는 i32 범위");
    let height = i32::try_from(metrics.height).map_err(|_| E6DemoFontError::GlyphDoesNotFit {
        glyph,
        x,
        y: 0,
        width: metrics.width,
        height: metrics.height,
        effect_width,
        effect_height,
    })?;
    let y = profile.baseline - metrics.ymin - height;
    let fits = effect_width <= E6_DEMO_GLYPH_CELL_WIDTH
        && effect_height <= E6_DEMO_GLYPH_CELL_HEIGHT
        && y >= 0
        && usize::try_from(y)
            .ok()
            .and_then(|origin| origin.checked_add(effect_height))
            .is_some_and(|end| end <= E6_DEMO_GLYPH_CELL_HEIGHT);
    if !fits {
        return Err(E6DemoFontError::GlyphDoesNotFit {
            glyph,
            x,
            y,
            width: metrics.width,
            height: metrics.height,
            effect_width,
            effect_height,
        });
    }

    let x = usize::try_from(x).expect("음수가 아님");
    let y = usize::try_from(y).expect("음수가 아님");
    let mut foreground_layer = vec![false; E6_DEMO_GLYPH_CELL_WIDTH * E6_DEMO_GLYPH_CELL_HEIGHT];
    for row in 0..metrics.height {
        for column in 0..metrics.width {
            if bitmap[row * metrics.width + column] >= threshold {
                foreground_layer[(y + row) * E6_DEMO_GLYPH_CELL_WIDTH + x + column] = true;
            }
        }
    }
    let source_bounds =
        foreground_bounds(&foreground_layer).ok_or(E6DemoFontError::EmptyGlyph(glyph))?;
    if profile.normalize_wide_hangul {
        normalize_wide_hangul_foreground(glyph, &mut foreground_layer)?;
    }
    let bounds = foreground_bounds(&foreground_layer).ok_or(E6DemoFontError::EmptyGlyph(glyph))?;
    let normalized_effect_width =
        bounds
            .width()
            .checked_add(offset_x)
            .ok_or(E6DemoFontError::GlyphDoesNotFit {
                glyph,
                x: i32::try_from(bounds.min_x).unwrap_or(i32::MAX),
                y: i32::try_from(bounds.min_y).unwrap_or(i32::MAX),
                width: bounds.width(),
                height: bounds.height(),
                effect_width: usize::MAX,
                effect_height: bounds.height(),
            })?;
    let normalized_effect_height =
        bounds
            .height()
            .checked_add(offset_y)
            .ok_or(E6DemoFontError::GlyphDoesNotFit {
                glyph,
                x: i32::try_from(bounds.min_x).unwrap_or(i32::MAX),
                y: i32::try_from(bounds.min_y).unwrap_or(i32::MAX),
                width: bounds.width(),
                height: bounds.height(),
                effect_width: normalized_effect_width,
                effect_height: usize::MAX,
            })?;
    let effect_max_x = bounds.max_x.checked_add(offset_x);
    let effect_max_y = bounds.max_y.checked_add(offset_y);
    if effect_max_x.is_none_or(|right| right >= E6_DEMO_GLYPH_CELL_WIDTH)
        || effect_max_y.is_none_or(|bottom| bottom >= E6_DEMO_GLYPH_CELL_HEIGHT)
    {
        return Err(E6DemoFontError::GlyphDoesNotFit {
            glyph,
            x: i32::try_from(bounds.min_x).unwrap_or(i32::MAX),
            y: i32::try_from(bounds.min_y).unwrap_or(i32::MAX),
            width: bounds.width(),
            height: bounds.height(),
            effect_width: normalized_effect_width,
            effect_height: normalized_effect_height,
        });
    }

    let mut pixels = vec![transparent; foreground_layer.len()];
    for (offset, occupied) in foreground_layer.iter().copied().enumerate() {
        if occupied {
            let x = offset % E6_DEMO_GLYPH_CELL_WIDTH;
            let y = offset / E6_DEMO_GLYPH_CELL_WIDTH;
            pixels[(y + offset_y) * E6_DEMO_GLYPH_CELL_WIDTH + x + offset_x] = shadow;
        }
    }
    for (offset, occupied) in foreground_layer.into_iter().enumerate() {
        if occupied {
            pixels[offset] = foreground;
        }
    }
    let effect_bounds = pixels
        .iter()
        .map(|value| *value != transparent)
        .collect::<Vec<_>>();
    let effect_bounds =
        foreground_bounds(&effect_bounds).ok_or(E6DemoFontError::EmptyGlyph(glyph))?;
    Ok(E6DemoGlyphRaster {
        pixels,
        geometry: E6DemoGlyphGeometry {
            source_foreground_width: source_bounds.width(),
            source_foreground_height: source_bounds.height(),
            foreground_width: bounds.width(),
            foreground_height: bounds.height(),
            effect_width: effect_bounds.width(),
            effect_height: effect_bounds.height(),
            normalized_wide_hangul: source_bounds.width() > source_bounds.height()
                && bounds.height() > source_bounds.height(),
        },
    })
}

/// 한 글자를 DEMOFONT의 투명+그림자 indexed4 cell로 만든다.
pub fn rasterize_e6_demo_glyph(
    font: &Font,
    glyph: char,
    profile: E6DemoRasterProfile,
) -> Result<Vec<u8>, E6DemoFontError> {
    Ok(rasterize_e6_demo_glyph_with_geometry(font, glyph, profile)?.pixels)
}

#[cfg(test)]
#[path = "e6_demo_tests.rs"]
mod tests;
