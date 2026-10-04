//! `PUYO2`의 `DEMOFONT.BTX` 물리 격자.
//!
//! 지원 원본은 runtime에서 512×256 indexed4 texture로 소비된다. 16×16
//! cell 512개는 256-cell짜리 좌우 bank에 놓인다. 이 모듈은 원본 픽셀과
//! runtime cell 경계를 고정한다.
//! 문자 code → cell 소비 규칙은 [`crate::disasm::demo_font`]가 소유한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::btx::{decode, encode, BtxError, Layout, PixelFormat};

pub const DEMO_FONT_LAYOUT: Layout = Layout {
    width: 512,
    height: 256,
    format: PixelFormat::Indexed4,
};
pub const DEMO_FONT_SHA256: &str =
    "ee51306e0d53875b05b3d7004b1d9900379db02862e1d54e888e27658c4970ac";
pub const DEMO_FONT_SLOT_COUNT: usize = 512;
const CELL_WIDTH: usize = 16;
const CELL_HEIGHT: usize = 16;
const BANK_WIDTH: usize = 256;
const COLUMNS_PER_BANK: usize = BANK_WIDTH / CELL_WIDTH;
const CELLS_PER_BANK: usize = COLUMNS_PER_BANK * (DEMO_FONT_LAYOUT.height / CELL_HEIGHT);

/// 한 cell의 물리 사각형.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemoFontSlot {
    pub index: usize,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl DemoFontSlot {
    pub fn for_index(index: usize) -> Result<Self, DemoFontError> {
        if index >= DEMO_FONT_SLOT_COUNT {
            return Err(DemoFontError::SlotIndexOutOfRange(index));
        }
        let bank = index / CELLS_PER_BANK;
        let index_in_bank = index % CELLS_PER_BANK;
        Ok(Self {
            index,
            x: bank * BANK_WIDTH + index_in_bank % COLUMNS_PER_BANK * CELL_WIDTH,
            y: index_in_bank / COLUMNS_PER_BANK * CELL_HEIGHT,
            width: CELL_WIDTH,
            height: CELL_HEIGHT,
        })
    }

    /// 주어진 runtime texture 픽셀이 이 cell에 속하는지 판정한다.
    pub fn contains_pixel(self, x: usize, y: usize) -> bool {
        (self.x..self.x + self.width).contains(&x) && (self.y..self.y + self.height).contains(&y)
    }
}

/// 검증된 DEMOFONT 픽셀.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoFontAtlas {
    pixels: Vec<u8>,
}

impl DemoFontAtlas {
    pub fn parse(raw: &[u8]) -> Result<Self, DemoFontError> {
        let pixels = decode(raw, DEMO_FONT_LAYOUT).map_err(DemoFontError::Btx)?;
        if encode(&pixels, DEMO_FONT_LAYOUT).map_err(DemoFontError::Btx)? != raw {
            return Err(DemoFontError::RoundtripMismatch);
        }
        Ok(Self { pixels })
    }

    pub fn slot_pixels(&self, index: usize) -> Result<Vec<u8>, DemoFontError> {
        let slot = DemoFontSlot::for_index(index)?;
        let mut out = Vec::with_capacity(CELL_WIDTH * CELL_HEIGHT);
        for y in slot.y..slot.y + slot.height {
            let start = y * DEMO_FONT_LAYOUT.width + slot.x;
            out.extend_from_slice(&self.pixels[start..start + slot.width]);
        }
        Ok(out)
    }

    pub fn summarize(&self) -> Result<DemoFontSummary, DemoFontError> {
        let mut empty_slots = Vec::new();
        let mut unique_cells = BTreeSet::new();
        for index in 0..DEMO_FONT_SLOT_COUNT {
            let pixels = self.slot_pixels(index)?;
            if pixels.iter().all(|value| *value == 0) {
                empty_slots.push(index);
            }
            unique_cells.insert(pixels);
        }
        let palette_indices: BTreeSet<_> = self.pixels.iter().copied().collect();
        Ok(DemoFontSummary {
            slots: DEMO_FONT_SLOT_COUNT,
            nonempty_slots: DEMO_FONT_SLOT_COUNT - empty_slots.len(),
            empty_slots,
            unique_cell_pixels: unique_cells.len(),
            palette_indices,
        })
    }
}

/// 지원 원본 DEMOFONT의 물리 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoFontSummary {
    pub slots: usize,
    pub nonempty_slots: usize,
    pub empty_slots: Vec<usize>,
    pub unique_cell_pixels: usize,
    pub palette_indices: BTreeSet<u8>,
}

/// DEMOFONT 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DemoFontError {
    Btx(BtxError),
    RoundtripMismatch,
    SlotIndexOutOfRange(usize),
    DigestMismatch {
        expected: String,
        actual: String,
    },
    SummaryMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    PaletteMismatch {
        expected: Vec<u8>,
        actual: Vec<u8>,
    },
}

impl fmt::Display for DemoFontError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Btx(source) => write!(f, "DEMOFONT BTX: {source}"),
            Self::RoundtripMismatch => write!(f, "DEMOFONT BTX encode가 원본과 다름"),
            Self::SlotIndexOutOfRange(index) => {
                write!(f, "DEMOFONT slot index 범위 초과: {index}")
            }
            Self::DigestMismatch { expected, actual } => write!(
                f,
                "DEMOFONT 해제 원문 SHA-256 불일치: expected={expected}, actual={actual}"
            ),
            Self::SummaryMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "DEMOFONT {field} 불일치: expected={expected}, actual={actual}"
            ),
            Self::PaletteMismatch { expected, actual } => write!(
                f,
                "DEMOFONT palette index 불일치: expected={expected:?}, actual={actual:?}"
            ),
        }
    }
}

impl Error for DemoFontError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Btx(source) => Some(source),
            _ => None,
        }
    }
}

/// 지원 원본 해제 결과의 digest와 전체 cell 분모를 검증한다.
pub fn verify_box_demo_font(raw: &[u8]) -> Result<DemoFontSummary, DemoFontError> {
    let actual_digest = format!("{:x}", Sha256::digest(raw));
    if actual_digest != DEMO_FONT_SHA256 {
        return Err(DemoFontError::DigestMismatch {
            expected: DEMO_FONT_SHA256.to_string(),
            actual: actual_digest,
        });
    }
    let summary = DemoFontAtlas::parse(raw)?.summarize()?;
    require_summary("slot 수", DEMO_FONT_SLOT_COUNT, summary.slots)?;
    require_summary("비어 있지 않은 slot 수", 336, summary.nonempty_slots)?;
    require_summary("고유 cell 픽셀 수", 253, summary.unique_cell_pixels)?;
    let actual_palette: Vec<_> = summary.palette_indices.iter().copied().collect();
    if actual_palette != [0, 1, 15] {
        return Err(DemoFontError::PaletteMismatch {
            expected: vec![0, 1, 15],
            actual: actual_palette,
        });
    }
    Ok(summary)
}

fn require_summary(
    field: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), DemoFontError> {
    if expected != actual {
        return Err(DemoFontError::SummaryMismatch {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "demo_tests.rs"]
mod demo_tests;
