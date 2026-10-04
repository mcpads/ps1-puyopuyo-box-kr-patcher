//! PUYO1 scene-local 16×16 glyph의 원천·선택 폰트 profile.
//!
//! 구조 상수인 cell 좌표와 분리해 font 원천·크기·baseline·effect만 소유한다.
//! 원본 scene atlas의 본문 cell이 쓰는 palette index `1` 배경과 `15` 본문을
//! 유지하고, cell 밖으로 나가는 글리프는 잘라내지 않고 실패시킨다.

use std::error::Error;
use std::fmt;

use fontdue::Font;

use crate::font::MAPLESTORY_LIGHT_FONT_ID;

pub const SCENE_GLYPH_CELL_WIDTH: usize = 16;
pub const SCENE_GLYPH_CELL_HEIGHT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneRasterEffect {
    Binary {
        threshold: u8,
        background: u8,
        foreground: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneRasterProfile {
    pub id: &'static str,
    pub font_id: &'static str,
    pub font_size: f32,
    pub baseline: i32,
    pub effect: SceneRasterEffect,
}

/// scene-local 대사의 첫 Maplestory Light 정적 합성 profile.
pub const SCENE_POC_RASTER_PROFILE: SceneRasterProfile = SceneRasterProfile {
    id: "puyo1-scene-local-16x16-binary",
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 14.0,
    baseline: 13,
    effect: SceneRasterEffect::Binary {
        threshold: 96,
        background: 1,
        foreground: 15,
    },
};

/// 수동 scene 번역 glyph의 하단 bearing까지 16×16에 보존하는 미승인 profile.
pub const SCENE_TRANSLATION_DRAFT_RASTER_PROFILE: SceneRasterProfile = SceneRasterProfile {
    id: "puyo1-scene-local-16x16-translation-draft",
    font_id: crate::font::selected::GALMURI11,
    font_size: 12.0,
    baseline: 13,
    effect: SceneRasterEffect::Binary {
        threshold: 128,
        background: 1,
        foreground: 15,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SceneFontError {
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

impl fmt::Display for SceneFontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfile => f.write_str("scene raster profile이 유효하지 않음"),
            Self::EmptyGlyph(glyph) => write!(f, "scene font가 빈 글리프를 반환: {glyph}"),
            Self::GlyphDoesNotFit {
                glyph,
                x,
                y,
                width,
                height,
            } => write!(
                f,
                "scene 16x16 cell에 글리프가 들어가지 않음: {glyph}, origin=({x},{y}), bitmap={width}x{height}"
            ),
        }
    }
}

impl Error for SceneFontError {}

/// 한 글자를 scene-local indexed4 palette cell로 만든다.
pub fn rasterize_scene_glyph(
    font: &Font,
    glyph: char,
    profile: SceneRasterProfile,
) -> Result<Vec<u8>, SceneFontError> {
    let SceneRasterEffect::Binary {
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
        return Err(SceneFontError::InvalidProfile);
    }
    let (metrics, bitmap) = font.rasterize(glyph, profile.font_size);
    if metrics.width == 0 || metrics.height == 0 || bitmap.iter().all(|&coverage| coverage == 0) {
        return Err(SceneFontError::EmptyGlyph(glyph));
    }
    let width = i32::try_from(metrics.width).map_err(|_| SceneFontError::GlyphDoesNotFit {
        glyph,
        x: 0,
        y: 0,
        width: metrics.width,
        height: metrics.height,
    })?;
    let height = i32::try_from(metrics.height).map_err(|_| SceneFontError::GlyphDoesNotFit {
        glyph,
        x: 0,
        y: 0,
        width: metrics.width,
        height: metrics.height,
    })?;
    // 12px 열 간격에서 앞 열의 opaque 16px cell이 다음 열의 왼쪽 4px를 덮는다.
    // 선택 글꼴은 그 겹침 뒤에 배치하고, PoC의 원래 중앙 정렬은 유지한다.
    let x = if profile == SCENE_TRANSLATION_DRAFT_RASTER_PROFILE {
        if width > 12 {
            return Err(SceneFontError::GlyphDoesNotFit {
                glyph,
                x: 0,
                y: 0,
                width: metrics.width,
                height: metrics.height,
            });
        }
        4
    } else {
        (i32::try_from(SCENE_GLYPH_CELL_WIDTH).expect("cell width는 i32 범위") - width) / 2
    };
    let y = profile.baseline - metrics.ymin - height;
    if x < 0
        || y < 0
        || x + width > i32::try_from(SCENE_GLYPH_CELL_WIDTH).expect("cell width는 i32 범위")
        || y + height > i32::try_from(SCENE_GLYPH_CELL_HEIGHT).expect("cell height는 i32 범위")
    {
        return Err(SceneFontError::GlyphDoesNotFit {
            glyph,
            x,
            y,
            width: metrics.width,
            height: metrics.height,
        });
    }

    let mut pixels = vec![background; SCENE_GLYPH_CELL_WIDTH * SCENE_GLYPH_CELL_HEIGHT];
    let x = usize::try_from(x).expect("음수가 아님");
    let y = usize::try_from(y).expect("음수가 아님");
    for row in 0..metrics.height {
        for column in 0..metrics.width {
            if bitmap[row * metrics.width + column] >= threshold {
                pixels[(y + row) * SCENE_GLYPH_CELL_WIDTH + x + column] = foreground;
            }
        }
    }
    Ok(pixels)
}

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;
