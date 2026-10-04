//! Baked 텍스트 category별 font·raster·effect profile.
//!
//! 자산 patch는 이 profile을 참조하고 font bytes를 직접 고르지 않는다.
//! 한 번의 합성에서도 category마다 서로 다른 font ID와 효과를 선택할 수
//! 있도록 font book과 profile을 분리한다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use fontdue::Font;

use crate::font::bundled_maplestory_light;
pub use crate::font::MAPLESTORY_LIGHT_FONT_ID;

/// 한 줄 텍스트의 수평 정렬.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalAlign {
    Left { padding: usize },
    Center,
    Right { padding: usize },
}

/// body 글리프보다 먼저 그릴 category별 효과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BakedTextEffect {
    None,
    Shadow {
        offset_x: i32,
        offset_y: i32,
        palette_index: u8,
    },
    Outline {
        palette_index: u8,
    },
}

/// 한 baked 텍스트 category의 version 관리 raster 설정.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BakedTextProfile<'a> {
    /// 보고서와 오류에서 category를 식별하는 안정된 ID.
    pub id: &'a str,
    /// [`BakedFontBook`]에 등록된 font ID.
    pub font_id: &'a str,
    pub font_size: f32,
    /// block 위쪽을 0으로 한 첫 행 baseline.
    pub baseline: i32,
    /// 이 profile이 정확히 요구하는 행 수.
    pub line_count: usize,
    /// 연속한 행 baseline 사이의 픽셀 거리.
    pub line_height: usize,
    pub tracking: usize,
    pub align: HorizontalAlign,
    pub background: u8,
    /// coverage가 0보다 큰 픽셀에 낮은 값부터 차례로 쓸 palette index.
    pub body_ramp: &'a [u8],
    pub effect: BakedTextEffect,
}

/// profile이 참조하는 font ID의 실제 font 집합.
#[derive(Default)]
pub struct BakedFontBook {
    fonts: BTreeMap<String, Font>,
}

impl BakedFontBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// 수정하지 않은 bundled Maplestory Light만 등록한 기본 font book.
    pub fn with_bundled_maplestory_light() -> Result<Self, BakedFontBookError> {
        let mut book = Self::new();
        let font = bundled_maplestory_light()
            .map_err(|source| BakedFontBookError::Load(source.to_string()))?;
        book.insert(MAPLESTORY_LIGHT_FONT_ID, font)?;
        Ok(book)
    }

    /// 화면별로 선택한 폰트와 기존 Maple을 함께 등록한다.
    pub fn with_selected_fonts() -> Result<Self, BakedFontBookError> {
        let mut book = Self::with_bundled_maplestory_light()?;
        for id in [
            crate::font::selected::MULMARU,
            crate::font::selected::GALMURI11_BOLD,
        ] {
            book.insert(
                id,
                crate::font::selected::load(id).map_err(BakedFontBookError::Load)?,
            )?;
        }
        Ok(book)
    }

    /// 새 font를 ID로 등록한다. 같은 ID의 조용한 덮어쓰기는 허용하지 않는다.
    pub fn insert(&mut self, id: &str, font: Font) -> Result<(), BakedFontBookError> {
        if id.is_empty() {
            return Err(BakedFontBookError::EmptyId);
        }
        if self.fonts.contains_key(id) {
            return Err(BakedFontBookError::DuplicateId(id.to_string()));
        }
        self.fonts.insert(id.to_string(), font);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Font> {
        self.fonts.get(id)
    }
}

/// font book 구성 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BakedFontBookError {
    EmptyId,
    DuplicateId(String),
    Load(String),
}

impl fmt::Display for BakedFontBookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => f.write_str("baked font ID가 비어 있음"),
            Self::DuplicateId(id) => write!(f, "baked font ID 중복: {id}"),
            Self::Load(source) => write!(f, "bundled baked font 로드 실패: {source}"),
        }
    }
}

impl Error for BakedFontBookError {}

#[cfg(test)]
#[path = "text_profile_tests.rs"]
mod tests;
