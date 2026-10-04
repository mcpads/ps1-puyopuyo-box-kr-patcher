//! classic mode `MENU06.BTX` 98-cell glyph atlas 경계.
//!
//! 두 물리 사본의 exact 512×256 indexed4 원본과 cell 0–97 영역을 검증한다.
//! 실제 Maplestory Light 합성은 원본 palette 역할을 이 분석으로 고정한 뒤
//! 같은 모듈에서만 제공한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::font::{bundled_maplestory_light, rasterize_menu06_glyph, MENU06_DRAFT_RASTER_PROFILE};
use crate::script::Menu06GlyphMapping;

use crate::btx::{decode, encode, Layout, PixelFormat};

pub const MENU06_GLYPH_ATLAS_LAYOUT: Layout = Layout {
    width: 512,
    height: 256,
    format: PixelFormat::Indexed4,
};
pub const MENU06_GLYPH_SLOTS: usize = 98;
const NARROW_SPACE_CELL: u16 = 0;
pub const MENU06_GLYPH_ASSET_SHA256: &str =
    "7a621068d7341bbb6a09f4905fe5650fe228f5a6d908fbaab495376e656f01d5";
pub const MENU06_SOURCE_TRANSPARENT_PIXELS: usize = 20_175;
pub const MENU06_SOURCE_FOREGROUND_PIXELS: usize = 4_657;
pub const BOX_MENU06_GLYPH_ASSETS: [(&str, &str); 2] = [
    ("/PUYO1/PP1_GRP.VDD;1", "MENU06.BTX"),
    ("/PUYO2/PP2_GRP.VDD;1", "MENU06.BTX"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06GlyphAssetAnalysis {
    pub physical_assets: usize,
    pub source_sha256: String,
    pub narrow_space_palette_indices: Vec<u8>,
    pub glyph_palette_indices: Vec<u8>,
    pub glyph_palette_histogram: Vec<(u8, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06GlyphPatch {
    pub plain: Vec<u8>,
    pub changed_cells: usize,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06GlyphPatchError(String);

impl fmt::Display for Menu06GlyphPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Menu06GlyphPatchError {}

fn error(message: impl Into<String>) -> Menu06GlyphPatchError {
    Menu06GlyphPatchError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn cell_rect(index: usize) -> Result<(usize, usize, usize, usize), Menu06GlyphPatchError> {
    if index >= MENU06_GLYPH_SLOTS {
        return Err(error(format!("MENU06 glyph cell 범위 이탈: {index}")));
    }
    if index == 0 {
        return Ok((240, 0, 8, 8));
    }
    let glyph = index - 1;
    Ok(((glyph % 15) * 16, (glyph / 15) * 16, 16, 16))
}

fn cell_pixels<'a>(
    pixels: &'a [u8],
    index: usize,
) -> Result<impl Iterator<Item = u8> + 'a, Menu06GlyphPatchError> {
    let (x, y, width, height) = cell_rect(index)?;
    Ok((0..height).flat_map(move |row| {
        pixels[(y + row) * MENU06_GLYPH_ATLAS_LAYOUT.width + x
            ..(y + row) * MENU06_GLYPH_ATLAS_LAYOUT.width + x + width]
            .iter()
            .copied()
    }))
}

/// 두 물리 MENU06 atlas와 원문 glyph cell palette를 검증한다.
pub fn verify_box_menu06_glyph_assets<'a>(
    assets: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
) -> Result<Menu06GlyphAssetAnalysis, Menu06GlyphPatchError> {
    let assets = assets.into_iter().collect::<Vec<_>>();
    if assets.len() != BOX_MENU06_GLYPH_ASSETS.len() {
        return Err(error(format!(
            "MENU06 glyph asset 입력 분모 불일치: expected={}, actual={}",
            BOX_MENU06_GLYPH_ASSETS.len(),
            assets.len()
        )));
    }
    let mut canonical = None;
    for (expected_archive, expected_asset) in BOX_MENU06_GLYPH_ASSETS {
        let matches = assets
            .iter()
            .filter(|(archive, asset, _)| *archive == expected_archive && *asset == expected_asset)
            .collect::<Vec<_>>();
        let [(_, _, plain)] = matches.as_slice() else {
            return Err(error(format!(
                "MENU06 glyph asset 누락/중복: {expected_archive}:{expected_asset}"
            )));
        };
        if digest(plain) != MENU06_GLYPH_ASSET_SHA256 {
            return Err(error(format!(
                "MENU06 glyph asset SHA-256 불일치: {expected_archive}:{expected_asset}"
            )));
        }
        if let Some(previous) = canonical {
            if previous != *plain {
                return Err(error("MENU06 두 물리 atlas byte drift"));
            }
        } else {
            canonical = Some(*plain);
        }
    }
    let plain = canonical.ok_or_else(|| error("MENU06 canonical atlas 누락"))?;
    let pixels =
        decode(plain, MENU06_GLYPH_ATLAS_LAYOUT).map_err(|source| error(source.to_string()))?;
    let narrow_space_palette_indices = cell_pixels(&pixels, 0)?
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut histogram = BTreeMap::<u8, usize>::new();
    for index in 1..MENU06_GLYPH_SLOTS {
        for pixel in cell_pixels(&pixels, index)? {
            *histogram.entry(pixel).or_default() += 1;
        }
    }
    let glyph_palette_indices = histogram.keys().copied().collect::<Vec<_>>();
    let glyph_palette_histogram = histogram.into_iter().collect::<Vec<_>>();
    if narrow_space_palette_indices != [0]
        || glyph_palette_indices != [0, 15]
        || glyph_palette_histogram
            != [
                (0, MENU06_SOURCE_TRANSPARENT_PIXELS),
                (15, MENU06_SOURCE_FOREGROUND_PIXELS),
            ]
    {
        return Err(error(format!(
            "MENU06 source glyph palette drift: space={narrow_space_palette_indices:?}, glyph={glyph_palette_histogram:?}"
        )));
    }
    Ok(Menu06GlyphAssetAnalysis {
        physical_assets: assets.len(),
        source_sha256: digest(plain),
        narrow_space_palette_indices,
        glyph_palette_indices,
        glyph_palette_histogram,
    })
}

fn validate_mappings(
    mappings: &[Menu06GlyphMapping],
) -> Result<BTreeSet<usize>, Menu06GlyphPatchError> {
    if mappings.is_empty() || mappings.len() > MENU06_GLYPH_SLOTS {
        return Err(error(format!(
            "MENU06 glyph mapping 분모 오류: {}",
            mappings.len()
        )));
    }
    let mut cells = BTreeSet::new();
    let mut glyphs = BTreeSet::new();
    let mut blank_cells = 0usize;
    for mapping in mappings {
        let cell = usize::from(mapping.cell);
        cell_rect(cell)?;
        if !cells.insert(cell) {
            return Err(error(format!("MENU06 glyph cell 중복: {cell}")));
        }
        match mapping.glyph {
            Some(' ') if cell == usize::from(NARROW_SPACE_CELL) => {}
            Some(glyph) if cell != usize::from(NARROW_SPACE_CELL) && !glyph.is_whitespace() => {
                if !glyphs.insert(glyph) {
                    return Err(error(format!("MENU06 glyph 문자 중복: {glyph}")));
                }
            }
            None if cell != usize::from(NARROW_SPACE_CELL) => blank_cells += 1,
            _ => {
                return Err(error(format!(
                    "MENU06 glyph mapping 역할 오류: cell={cell}"
                )))
            }
        }
    }
    if !cells.contains(&usize::from(NARROW_SPACE_CELL)) || blank_cells != 1 {
        return Err(error(format!(
            "MENU06 narrow-space/full-width-blank 분모 오류: cell0={}, blanks={blank_cells}",
            cells.contains(&usize::from(NARROW_SPACE_CELL))
        )));
    }
    Ok(cells)
}

fn patch_pixels(
    original: &[u8],
    mappings: &[Menu06GlyphMapping],
) -> Result<(Vec<u8>, usize), Menu06GlyphPatchError> {
    let mapped_cells = validate_mappings(mappings)?;
    let font = bundled_maplestory_light().map_err(|source| error(source.to_string()))?;
    let mut output = original.to_vec();
    for mapping in mappings {
        let cell = usize::from(mapping.cell);
        if cell == usize::from(NARROW_SPACE_CELL) {
            continue;
        }
        let (x, y, width, height) = cell_rect(cell)?;
        if width != 16 || height != 16 {
            return Err(error(format!("MENU06 patch 대상 cell 크기 오류: {cell}")));
        }
        let raster = match mapping.glyph {
            Some(glyph) => rasterize_menu06_glyph(&font, glyph, MENU06_DRAFT_RASTER_PROFILE)
                .map_err(|source| error(source.to_string()))?,
            None => vec![0; 16 * 16],
        };
        for row in 0..16 {
            let source_start = row * 16;
            let target_start = (y + row) * MENU06_GLYPH_ATLAS_LAYOUT.width + x;
            output[target_start..target_start + 16]
                .copy_from_slice(&raster[source_start..source_start + 16]);
        }
    }
    let changed_pixels = original
        .iter()
        .zip(&output)
        .filter(|(before, after)| before != after)
        .count();
    if changed_pixels == 0 {
        return Err(error("MENU06 glyph patch에서 변경된 pixel이 없음"));
    }
    let mut allowed = vec![false; original.len()];
    for cell in mapped_cells {
        if cell == usize::from(NARROW_SPACE_CELL) {
            continue;
        }
        let (x, y, width, height) = cell_rect(cell)?;
        for row in 0..height {
            let start = (y + row) * MENU06_GLYPH_ATLAS_LAYOUT.width + x;
            allowed[start..start + width].fill(true);
        }
    }
    for (offset, (&before, &after)) in original.iter().zip(&output).enumerate() {
        if before != after && !allowed[offset] {
            return Err(error(format!(
                "MENU06 glyph patch 비대상 pixel 변경: offset={offset}"
            )));
        }
    }
    Ok((output, changed_pixels))
}

/// 지원 원본 atlas의 배정한 cell만 Maplestory Light/blank로 바꾼다.
pub fn patch_box_menu06_glyph_asset(
    plain: &[u8],
    mappings: &[Menu06GlyphMapping],
) -> Result<Menu06GlyphPatch, Menu06GlyphPatchError> {
    if digest(plain) != MENU06_GLYPH_ASSET_SHA256 {
        return Err(error("MENU06 glyph patch 원본 SHA-256 불일치"));
    }
    let original =
        decode(plain, MENU06_GLYPH_ATLAS_LAYOUT).map_err(|source| error(source.to_string()))?;
    let (patched, changed_pixels) = patch_pixels(&original, mappings)?;
    let output =
        encode(&patched, MENU06_GLYPH_ATLAS_LAYOUT).map_err(|source| error(source.to_string()))?;
    let changed_bytes = plain
        .iter()
        .zip(&output)
        .filter(|(before, after)| before != after)
        .count();
    Ok(Menu06GlyphPatch {
        output_sha256: digest(&output),
        plain: output,
        changed_cells: mappings.len() - 1,
        changed_pixels,
        changed_bytes,
    })
}

#[cfg(test)]
#[path = "menu06_glyph_patch_tests.rs"]
mod tests;
