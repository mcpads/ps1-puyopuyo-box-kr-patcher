//! PUYO2 E6 전역 mapping에 맞춘 `DEMOFONT.BTX` cell 합성 경계.
//!
//! 지원 원본의 512×256 indexed4 runtime layout과 전체 digest를 먼저 검증한다. 번역 계획이
//! 배정한 앞 256개 cell만 E6 전용 Maplestory Light profile로 바꾸고, 나머지
//! cell과 모든 비대상 pixel은 보존한다. CNX·VDD·디스크 쓰기는 상위
//! pipeline의 책임이다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::font::{
    bundled_maplestory_light, rasterize_e6_demo_glyph, rasterize_e6_demo_glyph_with_geometry,
    verify_box_demo_font, DemoFontSlot, DEMO_FONT_LAYOUT, E6_DEMO_GLYPH_CELL_HEIGHT,
    E6_DEMO_GLYPH_CELL_WIDTH, E6_DEMO_POC_RASTER_PROFILE,
};

use crate::btx::{decode, encode};

/// 한글 음절에 배정한 원본 code pair와 first-match cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqE6GlyphMapping {
    pub glyph: char,
    pub cell_index: usize,
    pub pair: [u8; 2],
}

pub const PUYO2_E6_DEMO_FONT_MAPPED_CELLS: usize = 256;
pub const BOX_PUYO2_E6_DEMO_FONT_POC_CHANGED_PIXELS: usize = 96;
pub const BOX_PUYO2_E6_DEMO_FONT_POC_CHANGED_BYTES: usize = 64;
pub const BOX_PUYO2_E6_DEMO_FONT_POC_SHA256: &str =
    "f2a6d95a431586caf6f0107d31d74e8edb162ab9894fd8f4405239e873141aa8";

/// E6 번역 글리프 집합의 정규화 뒤 기하 분모.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Puyo2E6DemoFontGeometryAudit {
    pub glyphs: usize,
    pub normalized_wide_hangul: usize,
    pub squat_foreground_glyphs: usize,
    pub minimum_foreground_width: usize,
    pub maximum_foreground_width: usize,
    pub minimum_foreground_height: usize,
    pub maximum_foreground_height: usize,
}

/// 한 DEMOFONT 합성 결과와 허용 변경 분모.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6DemoFontPatch {
    pub plain: Vec<u8>,
    pub changed_cells: usize,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6DemoFontPatchError(String);

impl fmt::Display for Puyo2E6DemoFontPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Puyo2E6DemoFontPatchError {}

fn error(message: impl Into<String>) -> Puyo2E6DemoFontPatchError {
    Puyo2E6DemoFontPatchError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn validate_mappings(
    mappings: &[SubgameSeqE6GlyphMapping],
) -> Result<(), Puyo2E6DemoFontPatchError> {
    if mappings.is_empty() {
        return Err(error("PUYO2 E6 DEMOFONT glyph mapping이 비어 있음"));
    }
    let mut glyphs = BTreeSet::new();
    let mut indices = BTreeSet::new();
    let mut pairs = BTreeSet::new();
    for mapping in mappings {
        if !matches!(mapping.glyph, '\u{ac00}'..='\u{d7a3}') {
            return Err(error(format!(
                "PUYO2 E6 DEMOFONT mapping이 한글 음절이 아님: {}",
                mapping.glyph
            )));
        }
        if mapping.cell_index >= PUYO2_E6_DEMO_FONT_MAPPED_CELLS {
            return Err(error(format!(
                "PUYO2 E6 DEMOFONT mapping cell이 소비 범위 밖임: {}",
                mapping.cell_index
            )));
        }
        if !glyphs.insert(mapping.glyph)
            || !indices.insert(mapping.cell_index)
            || !pairs.insert(mapping.pair)
        {
            return Err(error("PUYO2 E6 DEMOFONT mapping glyph/cell/pair 중복"));
        }
    }
    Ok(())
}

/// E6 번역 글리프 전부가 같은 profile로 잘림 없이 생성되고 납작하지 않은지 판정한다.
pub fn audit_box_puyo2_e6_demo_font_geometry(
    mappings: &[SubgameSeqE6GlyphMapping],
) -> Result<Puyo2E6DemoFontGeometryAudit, Puyo2E6DemoFontPatchError> {
    validate_mappings(mappings)?;
    let font = bundled_maplestory_light().map_err(|source| error(source.to_string()))?;
    let geometries = mappings
        .iter()
        .map(|mapping| {
            rasterize_e6_demo_glyph_with_geometry(&font, mapping.glyph, E6_DEMO_POC_RASTER_PROFILE)
                .map(|raster| raster.geometry)
                .map_err(|source| error(source.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let minimum_foreground_width = geometries
        .iter()
        .map(|geometry| geometry.foreground_width)
        .min()
        .ok_or_else(|| error("PUYO2 E6 DEMOFONT geometry 분모가 비어 있음"))?;
    let maximum_foreground_width = geometries
        .iter()
        .map(|geometry| geometry.foreground_width)
        .max()
        .ok_or_else(|| error("PUYO2 E6 DEMOFONT geometry 분모가 비어 있음"))?;
    let minimum_foreground_height = geometries
        .iter()
        .map(|geometry| geometry.foreground_height)
        .min()
        .ok_or_else(|| error("PUYO2 E6 DEMOFONT geometry 분모가 비어 있음"))?;
    let maximum_foreground_height = geometries
        .iter()
        .map(|geometry| geometry.foreground_height)
        .max()
        .ok_or_else(|| error("PUYO2 E6 DEMOFONT geometry 분모가 비어 있음"))?;
    let audit = Puyo2E6DemoFontGeometryAudit {
        glyphs: geometries.len(),
        normalized_wide_hangul: geometries
            .iter()
            .filter(|geometry| geometry.normalized_wide_hangul)
            .count(),
        squat_foreground_glyphs: geometries
            .iter()
            .filter(|geometry| geometry.foreground_width > geometry.foreground_height)
            .count(),
        minimum_foreground_width,
        maximum_foreground_width,
        minimum_foreground_height,
        maximum_foreground_height,
    };
    if audit.squat_foreground_glyphs != 0 {
        return Err(error(format!(
            "PUYO2 E6 DEMOFONT profile에 세로로 납작한 한글이 남음: {}",
            audit.squat_foreground_glyphs
        )));
    }
    Ok(audit)
}

fn patch_pixels(
    original: &[u8],
    mappings: &[SubgameSeqE6GlyphMapping],
) -> Result<(Vec<u8>, usize), Puyo2E6DemoFontPatchError> {
    validate_mappings(mappings)?;
    let expected_pixels = DEMO_FONT_LAYOUT
        .width
        .checked_mul(DEMO_FONT_LAYOUT.height)
        .ok_or_else(|| error("DEMOFONT pixel 수 overflow"))?;
    if original.len() != expected_pixels {
        return Err(error(format!(
            "DEMOFONT pixel 길이 불일치: expected={expected_pixels}, actual={}",
            original.len()
        )));
    }
    let font = bundled_maplestory_light().map_err(|source| error(source.to_string()))?;
    let mut output = original.to_vec();
    for mapping in mappings {
        let glyph = rasterize_e6_demo_glyph(&font, mapping.glyph, E6_DEMO_POC_RASTER_PROFILE)
            .map_err(|source| error(source.to_string()))?;
        let slot = DemoFontSlot::for_index(mapping.cell_index)
            .map_err(|source| error(source.to_string()))?;
        for row in 0..E6_DEMO_GLYPH_CELL_HEIGHT {
            let source_start = row * E6_DEMO_GLYPH_CELL_WIDTH;
            let target_start = (slot.y + row) * DEMO_FONT_LAYOUT.width + slot.x;
            output[target_start..target_start + E6_DEMO_GLYPH_CELL_WIDTH]
                .copy_from_slice(&glyph[source_start..source_start + E6_DEMO_GLYPH_CELL_WIDTH]);
        }
    }
    let changed_pixels = original
        .iter()
        .zip(&output)
        .filter(|(before, after)| before != after)
        .count();
    if changed_pixels == 0 {
        return Err(error("PUYO2 E6 DEMOFONT patch에서 변경된 pixel이 없음"));
    }
    let mapped_slots = mappings
        .iter()
        .map(|mapping| {
            DemoFontSlot::for_index(mapping.cell_index).map_err(|source| error(source.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (offset, (&before, &after)) in original.iter().zip(&output).enumerate() {
        if before == after {
            continue;
        }
        let x = offset % DEMO_FONT_LAYOUT.width;
        let y = offset / DEMO_FONT_LAYOUT.width;
        if !mapped_slots.iter().any(|slot| slot.contains_pixel(x, y)) {
            return Err(error(format!(
                "PUYO2 E6 DEMOFONT 비대상 pixel 변경: ({x},{y})"
            )));
        }
    }
    Ok((output, changed_pixels))
}

/// 지원 원본 DEMOFONT에서 번역 계획이 배정한 cell만 바꾼다.
pub fn patch_box_puyo2_e6_demo_font(
    plain: &[u8],
    mappings: &[SubgameSeqE6GlyphMapping],
) -> Result<Puyo2E6DemoFontPatch, Puyo2E6DemoFontPatchError> {
    verify_box_demo_font(plain).map_err(|source| error(source.to_string()))?;
    let pixels = decode(plain, DEMO_FONT_LAYOUT).map_err(|source| error(source.to_string()))?;
    let (patched_pixels, changed_pixels) = patch_pixels(&pixels, mappings)?;
    let output =
        encode(&patched_pixels, DEMO_FONT_LAYOUT).map_err(|source| error(source.to_string()))?;
    let changed_bytes = plain
        .iter()
        .zip(&output)
        .filter(|(before, after)| before != after)
        .count();
    Ok(Puyo2E6DemoFontPatch {
        output_sha256: digest(&output),
        plain: output,
        changed_cells: mappings.len(),
        changed_pixels,
        changed_bytes,
    })
}

/// 합성 `가` 번역 계획의 실제 DEMOFONT 출력 기준선을 검증한다.
pub fn verify_box_puyo2_e6_demo_font_single_glyph_poc(
    plain: &[u8],
    mappings: &[SubgameSeqE6GlyphMapping],
) -> Result<Puyo2E6DemoFontPatch, Puyo2E6DemoFontPatchError> {
    let patch = patch_box_puyo2_e6_demo_font(plain, mappings)?;
    if patch.changed_cells != 1
        || mappings.first().map(|mapping| mapping.glyph) != Some('가')
        || patch.changed_pixels != BOX_PUYO2_E6_DEMO_FONT_POC_CHANGED_PIXELS
        || patch.changed_bytes != BOX_PUYO2_E6_DEMO_FONT_POC_CHANGED_BYTES
        || patch.output_sha256 != BOX_PUYO2_E6_DEMO_FONT_POC_SHA256
    {
        return Err(error(format!(
            "PUYO2 E6 DEMOFONT 합성 PoC 기준선 불일치: cells={}, pixels={}, bytes={}, sha256={}",
            patch.changed_cells, patch.changed_pixels, patch.changed_bytes, patch.output_sha256
        )));
    }
    Ok(patch)
}

#[cfg(test)]
#[path = "e6_demo_font_patch_tests.rs"]
mod tests;
