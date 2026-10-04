//! scene-local BTX의 canonical cell만 반복 한글로 채우는 진단 경계.
//!
//! 실제 번역문을 만들지 않고 pixel source를 판정하기 위한 PoC 전용이다.
//! 호출자가 확정한 repertoire index 집합만 바꾸며, 다른 pixel은 보존한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::disasm::{
    BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES, BOX_SUBGAME_SEQ_DRAC_CELL_BASE,
    BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS, BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X,
    BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y, BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT,
    BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH,
};
use crate::font::{
    bundled_maplestory_light, rasterize_scene_glyph, SCENE_GLYPH_CELL_HEIGHT,
    SCENE_GLYPH_CELL_WIDTH, SCENE_POC_RASTER_PROFILE,
};

use super::subgame_seq_scene_glyph_patch::SCENE_GLYPH_ATLAS_LAYOUT;
use crate::btx::{decode, encode};

/// 지원 원본 scene family에서 관측한 최대 cell grid 길이.
pub const BOX_SUBGAME_SEQ_SCENE_GLYPH_PROBE_MAX_SLOTS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneGlyphProbePatch {
    pub asset: String,
    pub glyph: char,
    pub repertoire_indices: Vec<usize>,
    pub cell_indices: Vec<usize>,
    pub source_palette_indices: Vec<u8>,
    pub plain: Vec<u8>,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub source_sha256: String,
    pub output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneGlyphProbeError(String);

impl fmt::Display for SubgameSeqSceneGlyphProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqSceneGlyphProbeError {}

fn error(message: impl Into<String>) -> SubgameSeqSceneGlyphProbeError {
    SubgameSeqSceneGlyphProbeError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn cell_origin(index: usize) -> Result<(usize, usize), SubgameSeqSceneGlyphProbeError> {
    if index >= BOX_SUBGAME_SEQ_SCENE_GLYPH_PROBE_MAX_SLOTS {
        return Err(error(format!(
            "scene glyph probe index 범위 이탈: index={index}, slots={BOX_SUBGAME_SEQ_SCENE_GLYPH_PROBE_MAX_SLOTS}"
        )));
    }
    let x = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X)
        + index % BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS
            * usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH);
    let y = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y)
        + index / BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS
            * usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT);
    if x + SCENE_GLYPH_CELL_WIDTH > SCENE_GLYPH_ATLAS_LAYOUT.width
        || y + SCENE_GLYPH_CELL_HEIGHT > SCENE_GLYPH_ATLAS_LAYOUT.height
    {
        return Err(error(format!(
            "scene glyph probe cell이 atlas 밖임: index={index}, origin=({x},{y})"
        )));
    }
    Ok((x, y))
}

/// 지정한 canonical repertoire cell만 같은 한글로 바꾼다.
pub fn patch_box_subgame_seq_scene_glyph_probe_asset(
    asset: &str,
    plain: &[u8],
    repertoire_indices: &[usize],
    glyph: char,
) -> Result<SubgameSeqSceneGlyphProbePatch, SubgameSeqSceneGlyphProbeError> {
    if asset.is_empty() {
        return Err(error("scene glyph probe asset 이름이 비어 있음"));
    }
    if !matches!(glyph, '\u{ac00}'..='\u{d7a3}') {
        return Err(error(format!(
            "scene glyph probe 문자가 한글 음절이 아님: {glyph}"
        )));
    }
    if repertoire_indices.is_empty() {
        return Err(error("scene glyph probe cell 집합이 비어 있음"));
    }
    let indices = repertoire_indices.iter().copied().collect::<BTreeSet<_>>();
    if indices.len() != repertoire_indices.len() {
        return Err(error("scene glyph probe cell index 중복"));
    }
    for &index in &indices {
        cell_origin(index)?;
    }
    if plain.len() != BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES {
        return Err(error(format!(
            "{asset} scene glyph probe atlas 길이 불일치: expected={BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES}, actual={}",
            plain.len()
        )));
    }

    let original = decode(plain, SCENE_GLYPH_ATLAS_LAYOUT)
        .map_err(|source| error(format!("{asset} scene glyph probe decode 실패: {source}")))?;
    let roundtrip = encode(&original, SCENE_GLYPH_ATLAS_LAYOUT)
        .map_err(|source| error(format!("{asset} scene glyph probe encode 실패: {source}")))?;
    if roundtrip != plain {
        return Err(error(format!(
            "{asset} scene glyph probe no-op 왕복 불일치"
        )));
    }

    let font = bundled_maplestory_light().map_err(|source| error(source.to_string()))?;
    let raster = rasterize_scene_glyph(&font, glyph, SCENE_POC_RASTER_PROFILE)
        .map_err(|source| error(source.to_string()))?;
    let allowed_source_palette = BTreeSet::from([0, 1, 15]);
    let mut source_palette = BTreeSet::new();
    let mut output = original.clone();
    let mut allowed = vec![false; original.len()];
    for &index in &indices {
        let (x, y) = cell_origin(index)?;
        let palette = (0..SCENE_GLYPH_CELL_HEIGHT)
            .flat_map(|cell_row| {
                original[(y + cell_row) * SCENE_GLYPH_ATLAS_LAYOUT.width + x
                    ..(y + cell_row) * SCENE_GLYPH_ATLAS_LAYOUT.width + x + SCENE_GLYPH_CELL_WIDTH]
                    .iter()
                    .copied()
            })
            .collect::<BTreeSet<_>>();
        if palette.is_empty() || !palette.is_subset(&allowed_source_palette) {
            return Err(error(format!(
                "{asset} scene glyph probe source palette 역할 불일치: index={index}, palette={palette:?}"
            )));
        }
        source_palette.extend(palette);
        for cell_row in 0..SCENE_GLYPH_CELL_HEIGHT {
            let source_start = cell_row * SCENE_GLYPH_CELL_WIDTH;
            let target_start = (y + cell_row) * SCENE_GLYPH_ATLAS_LAYOUT.width + x;
            output[target_start..target_start + SCENE_GLYPH_CELL_WIDTH]
                .copy_from_slice(&raster[source_start..source_start + SCENE_GLYPH_CELL_WIDTH]);
            allowed[target_start..target_start + SCENE_GLYPH_CELL_WIDTH].fill(true);
        }
    }

    let changed_pixels = original
        .iter()
        .zip(&output)
        .filter(|(before, after)| before != after)
        .count();
    if changed_pixels == 0 {
        return Err(error(format!(
            "{asset} scene glyph probe에서 변경된 pixel이 없음"
        )));
    }
    for (offset, (&before, &after)) in original.iter().zip(&output).enumerate() {
        if before != after && !allowed[offset] {
            return Err(error(format!(
                "{asset} scene glyph probe 비대상 pixel 변경: offset={offset}"
            )));
        }
    }

    let encoded = encode(&output, SCENE_GLYPH_ATLAS_LAYOUT)
        .map_err(|source| error(format!("{asset} scene glyph probe 재encode 실패: {source}")))?;
    if decode(&encoded, SCENE_GLYPH_ATLAS_LAYOUT)
        .map_err(|source| error(format!("{asset} scene glyph probe 재decode 실패: {source}")))?
        != output
    {
        return Err(error(format!("{asset} scene glyph probe 수정 왕복 불일치")));
    }
    let changed_bytes = plain
        .iter()
        .zip(&encoded)
        .filter(|(before, after)| before != after)
        .count();
    let repertoire_indices = indices.into_iter().collect::<Vec<_>>();
    let cell_indices = repertoire_indices
        .iter()
        .map(|index| usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_BASE) + index)
        .collect();

    Ok(SubgameSeqSceneGlyphProbePatch {
        asset: asset.to_string(),
        glyph,
        repertoire_indices,
        cell_indices,
        source_palette_indices: source_palette.into_iter().collect(),
        plain: encoded.clone(),
        changed_pixels,
        changed_bytes,
        source_sha256: digest(plain),
        output_sha256: digest(&encoded),
    })
}

#[cfg(test)]
#[path = "subgame_seq_scene_glyph_probe_tests.rs"]
mod tests;
