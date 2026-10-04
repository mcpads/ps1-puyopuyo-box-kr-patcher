//! `MM_MES.BTX`의 선언된 16×16 synthetic storage cell만 바꾸는 stress 경계.
//!
//! 실제 자산은 baked-text surface다. 이 grid는 renderer token이나 제품
//! patch 단위가 아니며, 원본 사본별 digest와 synthetic stress 변경 범위만
//! 검증한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::font::{
    bundled_maplestory_light, rasterize_mm_mes_storage_stress_glyph,
    MmMesStorageStressRasterEffect, MmMesStorageStressRasterProfile, MAPLESTORY_LIGHT_FONT_ID,
    MM_MES_STRESS_CELL_HEIGHT, MM_MES_STRESS_CELL_WIDTH,
};

use super::mm_mes_family::{
    verify_mm_mes_plain, MmMesVariant, MM_MES_LAYOUT, MM_MES_STORAGE_CELLS,
    MM_MES_STORAGE_CELL_HEIGHT, MM_MES_STORAGE_CELL_WIDTH, MM_MES_STORAGE_COLUMNS,
};
use crate::btx::{decode, encode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MmMesGridStressMapping {
    pub cell: u16,
    /// `None`은 명시적으로 투명 cell을 만든다. 공백 문자는 받지 않는다.
    pub glyph: Option<char>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmMesGridStressPatch {
    pub plain: Vec<u8>,
    pub mapped_cells: usize,
    pub changed_cells: usize,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmMesGridStressError(String);

impl fmt::Display for MmMesGridStressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for MmMesGridStressError {}

fn error(message: impl Into<String>) -> MmMesGridStressError {
    MmMesGridStressError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn cell_origin(cell: usize) -> Result<(usize, usize), MmMesGridStressError> {
    if cell >= MM_MES_STORAGE_CELLS {
        return Err(error(format!("MM_MES cell 범위 이탈: {cell}")));
    }
    Ok((
        (cell % MM_MES_STORAGE_COLUMNS) * MM_MES_STORAGE_CELL_WIDTH,
        (cell / MM_MES_STORAGE_COLUMNS) * MM_MES_STORAGE_CELL_HEIGHT,
    ))
}

fn validate_mappings(
    mappings: &[MmMesGridStressMapping],
) -> Result<BTreeSet<usize>, MmMesGridStressError> {
    if mappings.is_empty() || mappings.len() > MM_MES_STORAGE_CELLS {
        return Err(error(format!(
            "MM_MES cell mapping 분모 오류: {}",
            mappings.len()
        )));
    }
    let mut cells = BTreeSet::new();
    for mapping in mappings {
        let cell = usize::from(mapping.cell);
        cell_origin(cell)?;
        if !cells.insert(cell) {
            return Err(error(format!("MM_MES cell mapping 중복: {cell}")));
        }
        if mapping.glyph.is_some_and(char::is_whitespace) {
            return Err(error(format!(
                "MM_MES cell {cell}은 공백 문자 대신 None을 써야 함"
            )));
        }
    }
    Ok(cells)
}

fn cell_changed(before: &[u8], after: &[u8], cell: usize) -> bool {
    let x = (cell % MM_MES_STORAGE_COLUMNS) * MM_MES_STORAGE_CELL_WIDTH;
    let y = (cell / MM_MES_STORAGE_COLUMNS) * MM_MES_STORAGE_CELL_HEIGHT;
    (0..MM_MES_STORAGE_CELL_HEIGHT).any(|row| {
        let start = (y + row) * MM_MES_LAYOUT.width + x;
        before[start..start + MM_MES_STORAGE_CELL_WIDTH]
            != after[start..start + MM_MES_STORAGE_CELL_WIDTH]
    })
}

fn patch_pixels(
    original: &[u8],
    mappings: &[MmMesGridStressMapping],
    profile: MmMesStorageStressRasterProfile,
) -> Result<(Vec<u8>, usize, usize), MmMesGridStressError> {
    if original.len() != MM_MES_LAYOUT.width * MM_MES_LAYOUT.height {
        return Err(error(format!(
            "MM_MES decoded pixel 길이 불일치: {}",
            original.len()
        )));
    }
    let mapped_cells = validate_mappings(mappings)?;
    if profile.font_id != MAPLESTORY_LIGHT_FONT_ID {
        return Err(error(format!(
            "MM_MES 미지원 font profile: {}",
            profile.font_id
        )));
    }
    let MmMesStorageStressRasterEffect::CoverageRamp { transparent, .. } = profile.effect;
    let font = bundled_maplestory_light().map_err(|source| error(source.to_string()))?;
    // `None`만 있는 mapping도 잘못된 profile을 통과시키지 않는다.
    rasterize_mm_mes_storage_stress_glyph(&font, '가', profile)
        .map_err(|source| error(source.to_string()))?;

    let mut output = original.to_vec();
    for mapping in mappings {
        let cell = usize::from(mapping.cell);
        let (x, y) = cell_origin(cell)?;
        let raster = match mapping.glyph {
            Some(glyph) => rasterize_mm_mes_storage_stress_glyph(&font, glyph, profile)
                .map_err(|source| error(source.to_string()))?,
            None => vec![transparent; MM_MES_STRESS_CELL_WIDTH * MM_MES_STRESS_CELL_HEIGHT],
        };
        if raster.len() != MM_MES_STORAGE_CELL_WIDTH * MM_MES_STORAGE_CELL_HEIGHT
            || MM_MES_STRESS_CELL_WIDTH != MM_MES_STORAGE_CELL_WIDTH
            || MM_MES_STRESS_CELL_HEIGHT != MM_MES_STORAGE_CELL_HEIGHT
        {
            return Err(error("MM_MES stress font와 storage partition 크기 불일치"));
        }
        for row in 0..MM_MES_STORAGE_CELL_HEIGHT {
            let source_start = row * MM_MES_STORAGE_CELL_WIDTH;
            let target_start = (y + row) * MM_MES_LAYOUT.width + x;
            output[target_start..target_start + MM_MES_STORAGE_CELL_WIDTH]
                .copy_from_slice(&raster[source_start..source_start + MM_MES_STORAGE_CELL_WIDTH]);
        }
    }

    let changed_pixels = original
        .iter()
        .zip(&output)
        .filter(|(before, after)| before != after)
        .count();
    if changed_pixels == 0 {
        return Err(error("MM_MES grid stress에서 변경된 pixel이 없음"));
    }
    let changed_cells = mapped_cells
        .iter()
        .filter(|&&cell| cell_changed(original, &output, cell))
        .count();

    let mut allowed = vec![false; original.len()];
    for cell in mapped_cells {
        let (x, y) = cell_origin(cell)?;
        for row in 0..MM_MES_STORAGE_CELL_HEIGHT {
            let start = (y + row) * MM_MES_LAYOUT.width + x;
            allowed[start..start + MM_MES_STORAGE_CELL_WIDTH].fill(true);
        }
    }
    for (offset, (&before, &after)) in original.iter().zip(&output).enumerate() {
        if before != after && !allowed[offset] {
            return Err(error(format!(
                "MM_MES grid stress 비대상 pixel 변경: offset={offset}"
            )));
        }
    }
    Ok((output, changed_cells, changed_pixels))
}

/// 지원 원본의 배정한 cell만 전용 Maplestory Light profile 또는 투명으로 바꾼다.
pub fn stress_patch_mm_mes_grid(
    variant: MmMesVariant,
    plain: &[u8],
    mappings: &[MmMesGridStressMapping],
    profile: MmMesStorageStressRasterProfile,
) -> Result<MmMesGridStressPatch, MmMesGridStressError> {
    verify_mm_mes_plain(variant, plain).map_err(|source| error(source.to_string()))?;
    let original = decode(plain, MM_MES_LAYOUT).map_err(|source| error(source.to_string()))?;
    let (patched, changed_cells, changed_pixels) = patch_pixels(&original, mappings, profile)?;
    let output = encode(&patched, MM_MES_LAYOUT).map_err(|source| error(source.to_string()))?;
    let readback = decode(&output, MM_MES_LAYOUT).map_err(|source| error(source.to_string()))?;
    if readback != patched {
        return Err(error("MM_MES grid stress BTX roundtrip 불일치"));
    }
    let changed_bytes = plain
        .iter()
        .zip(&output)
        .filter(|(before, after)| before != after)
        .count();
    Ok(MmMesGridStressPatch {
        output_sha256: digest(&output),
        plain: output,
        mapped_cells: mappings.len(),
        changed_cells,
        changed_pixels,
        changed_bytes,
    })
}

#[cfg(test)]
#[path = "mm_mes_grid_stress_tests.rs"]
mod tests;
