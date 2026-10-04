//! DRAC scene-local BTX의 repertoire cell 한글 합성 경계.
//!
//! 지원 원본 두 자산의 1024×256 indexed4 layout과 cell `30..85` 원본
//! palette shape를 먼저 검증한다. 번역 계획이 배정한 repertoire 순번의
//! 16×16 cell만 Maplestory Light raster로 바꾸며, 모든 입력을 검증한 뒤
//! 새 BTX 전체를 반환한다. CLUT·VRAM upload·draw hit는 승인하지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::font::{
    bundled_maplestory_light, rasterize_drac_glyph, DracRasterProfile, DRAC_GLYPH_CELL_HEIGHT,
    DRAC_GLYPH_CELL_WIDTH, DRAC_POC_RASTER_PROFILE,
};
use crate::script::SubgameSeqDracGlyphMapping;

use crate::btx::{decode, encode, Layout, PixelFormat};

pub const DRAC_SCENE_ATLAS_LAYOUT: Layout = Layout {
    width: 1024,
    height: 256,
    format: PixelFormat::Indexed4,
};
pub const DRAC_SCENE_GLYPH_SLOTS: usize = 56;
pub const DRAC_SCENE_GLYPH_GRID_X: usize = 640;
pub const DRAC_SCENE_GLYPH_GRID_Y: usize = 0;
pub const DRAC_SCENE_GLYPH_GRID_COLUMNS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DracSceneGlyphAssetSpec {
    pub scope: &'static str,
    pub archive: &'static str,
    pub asset: &'static str,
    pub original_sha256: &'static str,
    pub uniform_zero_start: usize,
    pub uniform_zero_end: usize,
    pub single_glyph_poc_changed_pixels: usize,
    pub single_glyph_poc_changed_bytes: usize,
    pub single_glyph_poc_sha256: &'static str,
}

pub const BOX_DRAC_SCENE_GLYPH_ASSET_SPECS: [DracSceneGlyphAssetSpec; 2] = [
    DracSceneGlyphAssetSpec {
        scope: "PUYO1",
        archive: "/PUYO1/PP1_GRP.VDD;1",
        asset: "MZ_DRA.BTX",
        original_sha256: "74d42b1d57583f0225ad04e8c5aa03a284742664272b197779450231c35de7b0",
        uniform_zero_start: 27,
        uniform_zero_end: 32,
        single_glyph_poc_changed_pixels: 95,
        single_glyph_poc_changed_bytes: 69,
        single_glyph_poc_sha256: "c037ba53c34e5a227a6d45b37d2ae68fc067374a943f9fee65dc9ce6265fe278",
    },
    DracSceneGlyphAssetSpec {
        scope: "PUYO2",
        archive: "/PUYO2/PP2_GRP.VDD;1",
        asset: "MZ_MUMY.BTX",
        original_sha256: "c0c1ee748a88305c267ddd378d8d260396e0fd4e8ac7719fc2c4b281d3e61185",
        uniform_zero_start: 34,
        uniform_zero_end: 56,
        single_glyph_poc_changed_pixels: 63,
        single_glyph_poc_changed_bytes: 52,
        single_glyph_poc_sha256: "9d1efeae0a4beeeb9c856936a9a819ab213665a12d75c7f9fae5a0a1ec62e8b9",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DracSceneGlyphAssetRow {
    pub scope: &'static str,
    pub archive: &'static str,
    pub asset: &'static str,
    pub binary_cells: usize,
    pub uniform_zero_cells: usize,
    pub source_palette_indices: Vec<u8>,
    pub single_glyph_poc_changed_pixels: usize,
    pub single_glyph_poc_changed_bytes: usize,
    pub single_glyph_poc_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DracSceneGlyphAssetAnalysis {
    pub rows: Vec<DracSceneGlyphAssetRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DracSceneGlyphPatch {
    pub scope: &'static str,
    pub asset: &'static str,
    pub plain: Vec<u8>,
    pub changed_cells: usize,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DracSceneGlyphPatchError(String);

impl fmt::Display for DracSceneGlyphPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for DracSceneGlyphPatchError {}

fn error(message: impl Into<String>) -> DracSceneGlyphPatchError {
    DracSceneGlyphPatchError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn cell_origin(index: usize) -> Result<(usize, usize), DracSceneGlyphPatchError> {
    if index >= DRAC_SCENE_GLYPH_SLOTS {
        return Err(error(format!(
            "DRAC glyph repertoire index 범위 초과: {index}"
        )));
    }
    Ok((
        DRAC_SCENE_GLYPH_GRID_X + (index % DRAC_SCENE_GLYPH_GRID_COLUMNS) * DRAC_GLYPH_CELL_WIDTH,
        DRAC_SCENE_GLYPH_GRID_Y + (index / DRAC_SCENE_GLYPH_GRID_COLUMNS) * DRAC_GLYPH_CELL_HEIGHT,
    ))
}

fn cell_pixels<'a>(
    pixels: &'a [u8],
    index: usize,
) -> Result<impl Iterator<Item = u8> + 'a, DracSceneGlyphPatchError> {
    let (x, y) = cell_origin(index)?;
    Ok((0..DRAC_GLYPH_CELL_HEIGHT).flat_map(move |row| {
        pixels[(y + row) * DRAC_SCENE_ATLAS_LAYOUT.width + x
            ..(y + row) * DRAC_SCENE_ATLAS_LAYOUT.width + x + DRAC_GLYPH_CELL_WIDTH]
            .iter()
            .copied()
    }))
}

fn audit_source_cells(
    pixels: &[u8],
    spec: DracSceneGlyphAssetSpec,
) -> Result<(usize, usize, Vec<u8>), DracSceneGlyphPatchError> {
    let expected_pixels = DRAC_SCENE_ATLAS_LAYOUT
        .width
        .checked_mul(DRAC_SCENE_ATLAS_LAYOUT.height)
        .ok_or_else(|| error("DRAC atlas pixel 수 overflow"))?;
    if pixels.len() != expected_pixels
        || spec.uniform_zero_start >= spec.uniform_zero_end
        || spec.uniform_zero_end > DRAC_SCENE_GLYPH_SLOTS
    {
        return Err(error(format!(
            "{} DRAC glyph source 경계 불일치",
            spec.scope
        )));
    }
    let mut binary_cells = 0usize;
    let mut uniform_zero_cells = 0usize;
    let mut source_palette = BTreeSet::new();
    for index in 0..DRAC_SCENE_GLYPH_SLOTS {
        let palette = cell_pixels(pixels, index)?.collect::<BTreeSet<_>>();
        source_palette.extend(&palette);
        if (spec.uniform_zero_start..spec.uniform_zero_end).contains(&index) {
            if palette != BTreeSet::from([0]) {
                return Err(error(format!(
                    "{} DRAC glyph uniform-zero cell {} drift",
                    spec.scope, index
                )));
            }
            uniform_zero_cells += 1;
        } else {
            if palette != BTreeSet::from([1, 15]) {
                return Err(error(format!(
                    "{} DRAC glyph binary cell {} palette drift: {palette:?}",
                    spec.scope, index
                )));
            }
            binary_cells += 1;
        }
    }
    Ok((
        binary_cells,
        uniform_zero_cells,
        source_palette.into_iter().collect(),
    ))
}

fn validate_mappings(
    mappings: &[SubgameSeqDracGlyphMapping],
) -> Result<(), DracSceneGlyphPatchError> {
    if mappings.is_empty() {
        return Err(error("DRAC glyph mapping이 비어 있음"));
    }
    let mut glyphs = BTreeSet::new();
    let mut indices = BTreeSet::new();
    let mut pairs = BTreeSet::new();
    for mapping in mappings {
        if !matches!(mapping.glyph, '\u{ac00}'..='\u{d7a3}') {
            return Err(error(format!(
                "DRAC glyph mapping이 한글 음절이 아님: {}",
                mapping.glyph
            )));
        }
        cell_origin(mapping.repertoire_index)?;
        if !glyphs.insert(mapping.glyph)
            || !indices.insert(mapping.repertoire_index)
            || !pairs.insert(mapping.pair)
        {
            return Err(error("DRAC glyph mapping glyph/index/pair 중복"));
        }
    }
    Ok(())
}

fn patch_pixels(
    original: &[u8],
    mappings: &[SubgameSeqDracGlyphMapping],
    profile: DracRasterProfile,
) -> Result<(Vec<u8>, usize), DracSceneGlyphPatchError> {
    validate_mappings(mappings)?;
    let font = bundled_maplestory_light().map_err(|source| error(source.to_string()))?;
    let mut output = original.to_vec();
    for mapping in mappings {
        let glyph = rasterize_drac_glyph(&font, mapping.glyph, profile)
            .map_err(|source| error(source.to_string()))?;
        let (x, y) = cell_origin(mapping.repertoire_index)?;
        for row in 0..DRAC_GLYPH_CELL_HEIGHT {
            let source_start = row * DRAC_GLYPH_CELL_WIDTH;
            let target_start = (y + row) * DRAC_SCENE_ATLAS_LAYOUT.width + x;
            output[target_start..target_start + DRAC_GLYPH_CELL_WIDTH]
                .copy_from_slice(&glyph[source_start..source_start + DRAC_GLYPH_CELL_WIDTH]);
        }
    }
    let changed_pixels = original
        .iter()
        .zip(&output)
        .filter(|(before, after)| before != after)
        .count();
    if changed_pixels == 0 {
        return Err(error("DRAC glyph patch에서 변경된 pixel이 없음"));
    }
    let mapped_indices = mappings
        .iter()
        .map(|mapping| mapping.repertoire_index)
        .collect::<BTreeSet<_>>();
    for (offset, (&before, &after)) in original.iter().zip(&output).enumerate() {
        if before == after {
            continue;
        }
        let x = offset % DRAC_SCENE_ATLAS_LAYOUT.width;
        let y = offset / DRAC_SCENE_ATLAS_LAYOUT.width;
        let grid_x = DRAC_SCENE_GLYPH_GRID_X
            ..DRAC_SCENE_GLYPH_GRID_X + DRAC_SCENE_GLYPH_GRID_COLUMNS * DRAC_GLYPH_CELL_WIDTH;
        let grid_y = DRAC_SCENE_GLYPH_GRID_Y
            ..DRAC_SCENE_GLYPH_GRID_Y
                + (DRAC_SCENE_GLYPH_SLOTS / DRAC_SCENE_GLYPH_GRID_COLUMNS) * DRAC_GLYPH_CELL_HEIGHT;
        let in_grid = grid_x.contains(&x) && grid_y.contains(&y);
        let mapped = in_grid.then(|| {
            let column = (x - DRAC_SCENE_GLYPH_GRID_X) / DRAC_GLYPH_CELL_WIDTH;
            let row = (y - DRAC_SCENE_GLYPH_GRID_Y) / DRAC_GLYPH_CELL_HEIGHT;
            row * DRAC_SCENE_GLYPH_GRID_COLUMNS + column
        });
        if mapped.is_none_or(|index| !mapped_indices.contains(&index)) {
            return Err(error(format!(
                "DRAC glyph patch 비대상 pixel 변경: ({x},{y})"
            )));
        }
    }
    Ok((output, changed_pixels))
}

fn verify_source(
    spec: DracSceneGlyphAssetSpec,
    plain: &[u8],
) -> Result<(Vec<u8>, usize, usize, Vec<u8>), DracSceneGlyphPatchError> {
    if digest(plain) != spec.original_sha256 {
        return Err(error(format!(
            "{}:{} DRAC glyph 원본 SHA-256 불일치",
            spec.archive, spec.asset
        )));
    }
    let pixels =
        decode(plain, DRAC_SCENE_ATLAS_LAYOUT).map_err(|source| error(source.to_string()))?;
    let (binary, zero, palette) = audit_source_cells(&pixels, spec)?;
    Ok((pixels, binary, zero, palette))
}

/// 지원 원본 두 BTX의 exact text-cell shape와 단일 한글 합성 경계를 검증한다.
pub fn verify_box_drac_scene_glyph_assets<'a>(
    assets: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
) -> Result<DracSceneGlyphAssetAnalysis, DracSceneGlyphPatchError> {
    let mut assets = assets.into_iter().collect::<Vec<_>>();
    assets.sort_by_key(|(archive, asset, _)| (*archive, *asset));
    if assets.len() != BOX_DRAC_SCENE_GLYPH_ASSET_SPECS.len() {
        return Err(error("DRAC glyph asset 입력 분모 불일치"));
    }
    let mapping = [SubgameSeqDracGlyphMapping {
        glyph: '가',
        repertoire_index: 0,
        pair: [0x82, 0xA0],
    }];
    let mut rows = Vec::new();
    for spec in BOX_DRAC_SCENE_GLYPH_ASSET_SPECS {
        let matches = assets
            .iter()
            .filter(|(archive, asset, _)| *archive == spec.archive && *asset == spec.asset)
            .collect::<Vec<_>>();
        let [(_, _, plain)] = matches.as_slice() else {
            return Err(error(format!("{} DRAC glyph asset 누락/중복", spec.scope)));
        };
        let (pixels, binary_cells, uniform_zero_cells, source_palette_indices) =
            verify_source(spec, plain)?;
        let (poc_pixels, single_glyph_poc_changed_pixels) =
            patch_pixels(&pixels, &mapping, DRAC_POC_RASTER_PROFILE)?;
        let poc_plain = encode(&poc_pixels, DRAC_SCENE_ATLAS_LAYOUT)
            .map_err(|source| error(source.to_string()))?;
        let single_glyph_poc_changed_bytes = plain
            .iter()
            .zip(&poc_plain)
            .filter(|(before, after)| before != after)
            .count();
        let single_glyph_poc_sha256 = digest(&poc_plain);
        if binary_cells + uniform_zero_cells != DRAC_SCENE_GLYPH_SLOTS
            || single_glyph_poc_changed_pixels != spec.single_glyph_poc_changed_pixels
            || single_glyph_poc_changed_bytes != spec.single_glyph_poc_changed_bytes
            || single_glyph_poc_sha256 != spec.single_glyph_poc_sha256
        {
            return Err(error(format!(
                "{} DRAC single-glyph PoC 검산 불일치",
                spec.scope
            )));
        }
        rows.push(DracSceneGlyphAssetRow {
            scope: spec.scope,
            archive: spec.archive,
            asset: spec.asset,
            binary_cells,
            uniform_zero_cells,
            source_palette_indices,
            single_glyph_poc_changed_pixels,
            single_glyph_poc_changed_bytes,
            single_glyph_poc_sha256,
        });
    }
    rows.sort_by_key(|row| row.scope);
    Ok(DracSceneGlyphAssetAnalysis { rows })
}

/// 한 지원 원본 BTX에서 번역 계획이 배정한 cell만 한글로 바꾼다.
pub fn patch_box_drac_scene_glyph_asset(
    scope: &str,
    plain: &[u8],
    mappings: &[SubgameSeqDracGlyphMapping],
) -> Result<DracSceneGlyphPatch, DracSceneGlyphPatchError> {
    patch_box_drac_scene_glyph_asset_with_profile(scope, plain, mappings, DRAC_POC_RASTER_PROFILE)
}

/// 한 지원 원본 BTX에서 지정 profile로 번역 계획의 cell만 바꾼다.
pub fn patch_box_drac_scene_glyph_asset_with_profile(
    scope: &str,
    plain: &[u8],
    mappings: &[SubgameSeqDracGlyphMapping],
    profile: DracRasterProfile,
) -> Result<DracSceneGlyphPatch, DracSceneGlyphPatchError> {
    let spec = BOX_DRAC_SCENE_GLYPH_ASSET_SPECS
        .iter()
        .copied()
        .find(|spec| spec.scope == scope)
        .ok_or_else(|| error(format!("지원하지 않는 DRAC glyph scope: {scope}")))?;
    let (pixels, _, _, _) = verify_source(spec, plain)?;
    let (patched_pixels, changed_pixels) = patch_pixels(&pixels, mappings, profile)?;
    let output = encode(&patched_pixels, DRAC_SCENE_ATLAS_LAYOUT)
        .map_err(|source| error(source.to_string()))?;
    let changed_bytes = plain
        .iter()
        .zip(&output)
        .filter(|(before, after)| before != after)
        .count();
    Ok(DracSceneGlyphPatch {
        scope: spec.scope,
        asset: spec.asset,
        output_sha256: digest(&output),
        plain: output,
        changed_cells: mappings.len(),
        changed_pixels,
        changed_bytes,
    })
}

#[cfg(test)]
#[path = "drac_glyph_patch_tests.rs"]
mod drac_glyph_patch_tests;
