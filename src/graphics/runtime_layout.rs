//! 해제 BTX와 PlayStation VRAM dump의 exact layout 대조.
//!
//! BTX에는 폭·높이가 없다. 이 모듈은 파일 크기나 시각적 인상으로 폭을
//! 고르지 않고, 해제 원문 전체가 VRAM의 같은 X 위치와 연속 Y 행에 정확히
//! 놓인 경우만 열거한다. 서로 다른 layout이 동시에 성립하면 판정을
//! 거부한다.

use std::error::Error;
use std::fmt;

use crate::btx::{Layout, PixelFormat};

/// PlayStation VRAM 한 행의 16-bit word 수.
pub const VRAM_WIDTH_WORDS: usize = 1024;
/// PlayStation VRAM의 행 수.
pub const VRAM_HEIGHT: usize = 512;
const BYTES_PER_WORD: usize = 2;
const VRAM_ROW_BYTES: usize = VRAM_WIDTH_WORDS * BYTES_PER_WORD;
/// PlayStation VRAM dump의 정확한 바이트 수.
pub const VRAM_BYTE_LEN: usize = VRAM_ROW_BYTES * VRAM_HEIGHT;
const MAX_ANCHOR_LEN: usize = 32;

/// 한 exact BTX 배치.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VramPlacement {
    pub layout: Layout,
    /// VRAM 16-bit word 좌표.
    pub x_words: usize,
    pub y: usize,
}

/// runtime layout 판정 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeLayoutError {
    InvalidVramLength { expected: usize, actual: usize },
    EmptyAsset,
    NoExactMatch,
    AmbiguousLayouts { layouts: Vec<(usize, usize)> },
}

impl fmt::Display for RuntimeLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidVramLength { expected, actual } => write!(
                f,
                "PlayStation VRAM dump 크기 불일치: expected={expected}, actual={actual}"
            ),
            Self::EmptyAsset => write!(f, "빈 BTX는 runtime layout을 판정할 수 없음"),
            Self::NoExactMatch => {
                write!(f, "BTX 해제 원문 전체와 일치하는 VRAM 직사각형이 없음")
            }
            Self::AmbiguousLayouts { layouts } => {
                write!(f, "서로 다른 BTX runtime layout이 동시에 성립: ")?;
                for (index, (width, height)) in layouts.iter().enumerate() {
                    if index != 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{width}x{height}")?;
                }
                Ok(())
            }
        }
    }
}

impl Error for RuntimeLayoutError {}

fn pixel_width(row_bytes: usize, format: PixelFormat) -> usize {
    match format {
        PixelFormat::Indexed4 => row_bytes * 2,
        PixelFormat::Indexed8 => row_bytes,
    }
}

fn anchor(data: &[u8], row_bytes: usize) -> (usize, &[u8]) {
    let anchor_len = row_bytes.min(MAX_ANCHOR_LEN);
    let mut best_offset = 0;
    let mut best_score = (0usize, 0usize);

    for row_start in (0..data.len()).step_by(row_bytes) {
        for within in 0..=row_bytes - anchor_len {
            let offset = row_start + within;
            let candidate = &data[offset..offset + anchor_len];
            let mut seen = [false; 256];
            let mut distinct = 0;
            let mut nonzero = 0;
            for &byte in candidate {
                if !seen[usize::from(byte)] {
                    seen[usize::from(byte)] = true;
                    distinct += 1;
                }
                nonzero += usize::from(byte != 0);
            }
            let score = (distinct, nonzero);
            if score > best_score {
                best_score = score;
                best_offset = offset;
            }
        }
    }

    (best_offset, &data[best_offset..best_offset + anchor_len])
}

fn rectangle_matches(
    data: &[u8],
    vram: &[u8],
    row_bytes: usize,
    height: usize,
    base_x: usize,
    base_y: usize,
) -> bool {
    (0..height).all(|row| {
        let source_start = row * row_bytes;
        let vram_start = (base_y + row) * VRAM_ROW_BYTES + base_x;
        data[source_start..source_start + row_bytes] == vram[vram_start..vram_start + row_bytes]
    })
}

/// 해제 BTX 전체와 exact 일치하는 모든 VRAM 배치를 열거한다.
///
/// 폭 후보는 GPU `LoadImage`의 16-bit word 경계와 VRAM 크기에서만
/// 파생한다. 즉 4bpp는 4px, 8bpp는 2px 단위이며 파일명·시각 후보 폭을
/// 입력으로 받지 않는다.
pub fn exact_vram_placements(
    data: &[u8],
    format: PixelFormat,
    vram: &[u8],
) -> Result<Vec<VramPlacement>, RuntimeLayoutError> {
    if vram.len() != VRAM_BYTE_LEN {
        return Err(RuntimeLayoutError::InvalidVramLength {
            expected: VRAM_BYTE_LEN,
            actual: vram.len(),
        });
    }
    if data.is_empty() {
        return Err(RuntimeLayoutError::EmptyAsset);
    }

    let mut placements = Vec::new();
    let max_row_bytes = data.len().min(VRAM_ROW_BYTES);
    for row_bytes in (BYTES_PER_WORD..=max_row_bytes).step_by(BYTES_PER_WORD) {
        if !data.len().is_multiple_of(row_bytes) {
            continue;
        }
        let height = data.len() / row_bytes;
        if height > VRAM_HEIGHT {
            continue;
        }
        let width = pixel_width(row_bytes, format);
        let layout = Layout {
            width,
            height,
            format,
        };
        let (source_anchor_offset, source_anchor) = anchor(data, row_bytes);
        let source_anchor_row = source_anchor_offset / row_bytes;
        let source_anchor_x = source_anchor_offset % row_bytes;

        for (hit, window) in vram.windows(source_anchor.len()).enumerate() {
            if window != source_anchor {
                continue;
            }
            let hit_y = hit / VRAM_ROW_BYTES;
            let hit_x = hit % VRAM_ROW_BYTES;
            if hit_x + source_anchor.len() > VRAM_ROW_BYTES
                || hit_x < source_anchor_x
                || hit_y < source_anchor_row
            {
                continue;
            }
            let base_x = hit_x - source_anchor_x;
            let base_y = hit_y - source_anchor_row;
            if !base_x.is_multiple_of(BYTES_PER_WORD)
                || base_x + row_bytes > VRAM_ROW_BYTES
                || base_y + height > VRAM_HEIGHT
            {
                continue;
            }
            if rectangle_matches(data, vram, row_bytes, height, base_x, base_y) {
                placements.push(VramPlacement {
                    layout,
                    x_words: base_x / BYTES_PER_WORD,
                    y: base_y,
                });
            }
        }
    }
    Ok(placements)
}

/// exact 배치들의 폭·높이가 하나로 수렴할 때만 runtime layout을 확정한다.
pub fn infer_unique_vram_layout(
    data: &[u8],
    format: PixelFormat,
    vram: &[u8],
) -> Result<(Layout, Vec<VramPlacement>), RuntimeLayoutError> {
    let placements = exact_vram_placements(data, format, vram)?;
    let Some(first) = placements.first() else {
        return Err(RuntimeLayoutError::NoExactMatch);
    };
    let mut layouts = Vec::new();
    for placement in &placements {
        let dimensions = (placement.layout.width, placement.layout.height);
        if !layouts.contains(&dimensions) {
            layouts.push(dimensions);
        }
    }
    layouts.sort_unstable();
    if layouts.len() != 1 {
        return Err(RuntimeLayoutError::AmbiguousLayouts { layouts });
    }
    Ok((first.layout, placements))
}

#[cfg(test)]
#[path = "runtime_layout_tests.rs"]
mod tests;
