//! `P2_INFO4/5.BTX` 16×16 설명 page용 Maplestory Light profile.
//!
//! 실제 문장 조립은 `graphics::fixed_cell_text_patch`가 소유한다. 이 모듈은
//! 한 셀의 font·크기·baseline·palette와 넓은 한글의 기하 보정만 분리한다.

use std::error::Error;
use std::fmt;

use fontdue::Font;

use crate::font::MAPLESTORY_LIGHT_FONT_ID;

pub const INFO_PAGE_GLYPH_CELL_WIDTH: usize = 16;
pub const INFO_PAGE_GLYPH_CELL_HEIGHT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoPageRasterEffect {
    Binary {
        threshold: u8,
        background: u8,
        foreground: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InfoPageRasterProfile {
    pub id: &'static str,
    pub font_id: &'static str,
    pub font_size: f32,
    pub baseline: i32,
    /// 본문 폭이 높이보다 큰 한글만 아래 기준선을 유지하며 정사각형까지 늘린다.
    pub normalize_wide_hangul: bool,
    pub empty: u8,
    pub effect: InfoPageRasterEffect,
}

/// PUYO classic 설명 page의 첫 Maplestory Light 합성 profile.
pub const P2_INFO_PAGE_DRAFT_RASTER_PROFILE: InfoPageRasterProfile = InfoPageRasterProfile {
    id: "puyo-info-page-16x16-binary",
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 14.0,
    baseline: 13,
    normalize_wide_hangul: true,
    empty: 0,
    effect: InfoPageRasterEffect::Binary {
        threshold: 96,
        background: 1,
        foreground: 15,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoPageFontError {
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

impl fmt::Display for InfoPageFontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfile => f.write_str("INFO page raster profile이 유효하지 않음"),
            Self::EmptyGlyph(glyph) => write!(f, "INFO page font가 빈 글리프를 반환: {glyph}"),
            Self::GlyphDoesNotFit {
                glyph,
                x,
                y,
                width,
                height,
            } => write!(
                f,
                "INFO page 16x16 cell에 글리프가 들어가지 않음: {glyph}, \
                 origin=({x},{y}), bitmap={width}x{height}"
            ),
        }
    }
}

impl Error for InfoPageFontError {}

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
    let mut bounds: Option<PixelBounds> = None;
    for (offset, occupied) in layer.iter().copied().enumerate() {
        if !occupied {
            continue;
        }
        let x = offset % INFO_PAGE_GLYPH_CELL_WIDTH;
        let y = offset / INFO_PAGE_GLYPH_CELL_WIDTH;
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

fn normalize_wide_hangul_foreground(
    glyph: char,
    layer: &mut Vec<bool>,
) -> Result<(), InfoPageFontError> {
    if !matches!(glyph, '\u{ac00}'..='\u{d7a3}') {
        return Ok(());
    }
    let bounds = foreground_bounds(layer).ok_or(InfoPageFontError::EmptyGlyph(glyph))?;
    if bounds.width() <= bounds.height() {
        return Ok(());
    }
    let target_height = bounds.width();
    let target_min_y = bounds
        .max_y
        .checked_add(1)
        .and_then(|bottom| bottom.checked_sub(target_height))
        .ok_or(InfoPageFontError::GlyphDoesNotFit {
            glyph,
            x: i32::try_from(bounds.min_x).unwrap_or(i32::MAX),
            y: -1,
            width: bounds.width(),
            height: bounds.height(),
        })?;
    let mut normalized = vec![false; layer.len()];
    for target_row in 0..target_height {
        let source_row = bounds.min_y
            + ((target_row * 2 + 1) * bounds.height() / (target_height * 2))
                .min(bounds.height() - 1);
        for x in bounds.min_x..=bounds.max_x {
            if layer[source_row * INFO_PAGE_GLYPH_CELL_WIDTH + x] {
                normalized[(target_min_y + target_row) * INFO_PAGE_GLYPH_CELL_WIDTH + x] = true;
            }
        }
    }
    *layer = normalized;
    Ok(())
}

/// 한 글자를 설명 page의 background+본문 indexed4 cell로 만든다.
pub fn rasterize_info_page_glyph(
    font: &Font,
    glyph: char,
    profile: InfoPageRasterProfile,
) -> Result<Vec<u8>, InfoPageFontError> {
    let InfoPageRasterEffect::Binary {
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
        || [profile.empty, background, foreground]
            .iter()
            .any(|value| *value > 0x0F)
        || profile.empty == background
        || profile.empty == foreground
        || background == foreground
    {
        return Err(InfoPageFontError::InvalidProfile);
    }
    if glyph.is_whitespace() {
        return Ok(vec![
            background;
            INFO_PAGE_GLYPH_CELL_WIDTH * INFO_PAGE_GLYPH_CELL_HEIGHT
        ]);
    }
    let (metrics, bitmap) = font.rasterize(glyph, profile.font_size);
    if metrics.width == 0 || metrics.height == 0 || bitmap.iter().all(|&value| value == 0) {
        return Err(InfoPageFontError::EmptyGlyph(glyph));
    }
    let width = i32::try_from(metrics.width).map_err(|_| InfoPageFontError::GlyphDoesNotFit {
        glyph,
        x: 0,
        y: 0,
        width: metrics.width,
        height: metrics.height,
    })?;
    let height = i32::try_from(metrics.height).map_err(|_| InfoPageFontError::GlyphDoesNotFit {
        glyph,
        x: 0,
        y: 0,
        width: metrics.width,
        height: metrics.height,
    })?;
    let x = (i32::try_from(INFO_PAGE_GLYPH_CELL_WIDTH).expect("cell width는 i32 범위") - width) / 2;
    let y = profile.baseline - metrics.ymin - height;
    if x < 0
        || y < 0
        || x + width > i32::try_from(INFO_PAGE_GLYPH_CELL_WIDTH).expect("cell width는 i32 범위")
        || y + height > i32::try_from(INFO_PAGE_GLYPH_CELL_HEIGHT).expect("cell height는 i32 범위")
    {
        return Err(InfoPageFontError::GlyphDoesNotFit {
            glyph,
            x,
            y,
            width: metrics.width,
            height: metrics.height,
        });
    }

    let x = usize::try_from(x).expect("음수가 아님");
    let y = usize::try_from(y).expect("음수가 아님");
    let mut foreground_layer =
        vec![false; INFO_PAGE_GLYPH_CELL_WIDTH * INFO_PAGE_GLYPH_CELL_HEIGHT];
    for row in 0..metrics.height {
        for column in 0..metrics.width {
            if bitmap[row * metrics.width + column] >= threshold {
                foreground_layer[(y + row) * INFO_PAGE_GLYPH_CELL_WIDTH + x + column] = true;
            }
        }
    }
    if foreground_bounds(&foreground_layer).is_none() {
        return Err(InfoPageFontError::EmptyGlyph(glyph));
    }
    if profile.normalize_wide_hangul {
        normalize_wide_hangul_foreground(glyph, &mut foreground_layer)?;
    }

    Ok(foreground_layer
        .into_iter()
        .map(|occupied| if occupied { foreground } else { background })
        .collect())
}

#[cfg(test)]
#[path = "info_page_tests.rs"]
mod tests;
