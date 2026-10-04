//! PUYO1 scene-local same-stem BTX의 family별 glyph cell 합성 경계.
//!
//! 검증된 `E1` canonical repertoire의 first-match pair와 `cell = index + 30`을
//! 같은 family의 selected=same-stem BTX에 연결한다. PUYO2는 실제 selected
//! resource가 same-stem과 다르므로 runtime pixel source 판정 전에는 이
//! writer가 항상 거부한다. 수정한 cell 밖의 pixel과 byte는 보존한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::SubgameSeqSceneGlyphMapping;
use crate::analysis::{
    SubgameSeqSceneGlyphAnalysis, SubgameSeqSceneGlyphRow, BOX_SUBGAME_SEQ_SCENE_GLYPH_FAMILIES,
    BOX_SUBGAME_SEQ_SCENE_GLYPH_ROWS,
};
use crate::disasm::{
    BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES, BOX_SUBGAME_SEQ_DRAC_ATLAS_HEIGHT,
    BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH, BOX_SUBGAME_SEQ_DRAC_CELL_BASE,
    BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS, BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X,
    BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y, BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT,
    BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET, BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_RUNTIME,
    BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH,
};
use crate::font::{
    bundled_maplestory_light, rasterize_scene_glyph, SceneRasterEffect, SceneRasterProfile,
    SCENE_GLYPH_CELL_HEIGHT, SCENE_GLYPH_CELL_WIDTH, SCENE_POC_RASTER_PROFILE,
};

use crate::btx::{decode, encode, Layout, PixelFormat};

pub const SUBGAME_SEQ_SCENE_GLYPH_PATCH_SCHEMA_VERSION: &str = "subgame-seq-scene-glyph-patch-v1";
pub const SCENE_GLYPH_ATLAS_LAYOUT: Layout = Layout {
    width: 1024,
    height: 256,
    format: PixelFormat::Indexed4,
};
pub const BOX_SUBGAME_SEQ_SCENE_GLYPH_PATCH_ASSETS: usize = 17;
pub const BOX_SUBGAME_SEQ_SCENE_GLYPH_PATCH_CELLS: usize = 17;
pub const BOX_SUBGAME_SEQ_SCENE_GLYPH_PATCH_CHANGED_PIXELS: usize = 1_420;
pub const BOX_SUBGAME_SEQ_SCENE_GLYPH_PATCH_CHANGED_BYTES: usize = 1_051;
pub const BOX_SUBGAME_SEQ_SCENE_GLYPH_PATCH_REPORT_SHA256: &str =
    "290b99924897b049b73127255ccd13252acf766c9b321a184cded3c2a29749ed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneGlyphPatch {
    pub scope: &'static str,
    pub sequence: &'static str,
    pub archive: &'static str,
    pub asset: &'static str,
    pub plain: Vec<u8>,
    pub cell_indices: Vec<usize>,
    pub source_palette_indices: Vec<u8>,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub source_sha256: String,
    pub output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneGlyphPatchRow {
    pub scope: &'static str,
    pub sequence: &'static str,
    pub archive: &'static str,
    pub asset: &'static str,
    pub cell_indices: Vec<usize>,
    pub source_palette_indices: Vec<u8>,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub source_sha256: String,
    pub output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneGlyphPatchAnalysis {
    pub rows: Vec<SubgameSeqSceneGlyphPatchRow>,
    pub assets: usize,
    pub cells: usize,
    pub source_palette_indices: Vec<u8>,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneGlyphPatchError(String);

impl fmt::Display for SubgameSeqSceneGlyphPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqSceneGlyphPatchError {}

fn error(message: impl Into<String>) -> SubgameSeqSceneGlyphPatchError {
    SubgameSeqSceneGlyphPatchError(message.into())
}

fn digest(raw: &[u8]) -> [u8; 32] {
    Sha256::digest(raw).into()
}

fn hex(raw: &[u8]) -> String {
    raw.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn cell_origin(
    row: &SubgameSeqSceneGlyphRow,
    repertoire_index: usize,
) -> Result<(usize, usize), SubgameSeqSceneGlyphPatchError> {
    if repertoire_index >= row.grid_slots {
        return Err(error(format!(
            "{} scene glyph grid 범위 이탈: index={repertoire_index}, slots={}",
            row.sequence, row.grid_slots
        )));
    }
    let x = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X)
        + repertoire_index % BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS
            * usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH);
    let y = usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y)
        + repertoire_index / BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS
            * usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT);
    if x + SCENE_GLYPH_CELL_WIDTH > SCENE_GLYPH_ATLAS_LAYOUT.width
        || y + SCENE_GLYPH_CELL_HEIGHT > SCENE_GLYPH_ATLAS_LAYOUT.height
    {
        return Err(error(format!(
            "{} scene glyph cell이 atlas 밖임: index={repertoire_index}, origin=({x},{y})",
            row.sequence
        )));
    }
    Ok((x, y))
}

fn verify_row_contract(
    row: &SubgameSeqSceneGlyphRow,
    sequence_raw: &[u8],
    plain: &[u8],
) -> Result<Vec<u8>, SubgameSeqSceneGlyphPatchError> {
    let expected_last = row
        .cell_base
        .checked_add(
            u16::try_from(row.repertoire_slots.saturating_sub(1))
                .map_err(|_| error("scene repertoire slot 변환 실패"))?,
        )
        .ok_or_else(|| error("scene cell last overflow"))?;
    if row.scope != "PUYO1" {
        return Err(error(format!(
            "{}:{} scene glyph patch는 PUYO2 runtime 판정 전 no-write",
            row.scope, row.sequence
        )));
    }
    if !row.selected_is_same_stem
        || row.selected_asset != row.same_stem_asset
        || row.selected_vdd_index != row.same_stem_vdd_index
        || row.selected_plain_sha256 != row.same_stem_plain_sha256
        || row.selected_plain_bytes != row.same_stem_plain_bytes
        || row.selected_cnx_subtype != row.same_stem_cnx_subtype
        || !row.selected_atlas_compatible
        || row.selected_cnx_subtype != *b"bi_"
    {
        return Err(error(format!(
            "{} scene selected/same-stem atlas 계약 불일치",
            row.sequence
        )));
    }
    if row.cell_table_offset != BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET
        || row.cell_table_runtime != BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_RUNTIME
        || row.cell_base != BOX_SUBGAME_SEQ_DRAC_CELL_BASE
        || row.cell_last != expected_last
        || row.canonical_glyph_slots == 0
        || row.canonical_glyph_slots > row.grid_slots
        || row.grid_slots > row.repertoire_slots
        || row.same_stem_plain_bytes != BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES
        || SCENE_GLYPH_ATLAS_LAYOUT.width != usize::from(BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH)
        || SCENE_GLYPH_ATLAS_LAYOUT.height != usize::from(BOX_SUBGAME_SEQ_DRAC_ATLAS_HEIGHT)
        || SCENE_GLYPH_CELL_WIDTH != usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH)
        || SCENE_GLYPH_CELL_HEIGHT != usize::from(BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT)
    {
        return Err(error(format!(
            "{} scene cell/atlas 계약 불일치",
            row.sequence
        )));
    }
    if digest(sequence_raw) != row.sequence_sha256 {
        return Err(error(format!("{} scene SEQ SHA-256 불일치", row.sequence)));
    }
    if plain.len() != BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES
        || digest(plain) != row.same_stem_plain_sha256
    {
        return Err(error(format!(
            "{}:{} scene BTX 길이/SHA-256 불일치",
            row.grp_archive, row.same_stem_asset
        )));
    }
    let pixels = decode(plain, SCENE_GLYPH_ATLAS_LAYOUT)
        .map_err(|source| error(format!("{} scene BTX decode 실패: {source}", row.sequence)))?;
    let roundtrip = encode(&pixels, SCENE_GLYPH_ATLAS_LAYOUT)
        .map_err(|source| error(format!("{} scene BTX encode 실패: {source}", row.sequence)))?;
    if roundtrip != plain {
        return Err(error(format!(
            "{} scene BTX no-op 왕복 불일치",
            row.sequence
        )));
    }
    Ok(pixels)
}

fn validate_mappings(
    row: &SubgameSeqSceneGlyphRow,
    sequence_raw: &[u8],
    mappings: &[SubgameSeqSceneGlyphMapping],
) -> Result<BTreeSet<usize>, SubgameSeqSceneGlyphPatchError> {
    if mappings.is_empty() {
        return Err(error(format!(
            "{} scene glyph mapping이 비어 있음",
            row.sequence
        )));
    }
    let canonical_end = row
        .repertoire_offset
        .checked_add(row.canonical_glyph_slots * 2)
        .ok_or_else(|| error("scene canonical repertoire 끝 overflow"))?;
    let canonical = sequence_raw
        .get(row.repertoire_offset..canonical_end)
        .ok_or_else(|| error(format!("{} canonical repertoire 범위 이탈", row.sequence)))?;
    let mut glyphs = BTreeSet::new();
    let mut indices = BTreeSet::new();
    let mut pairs = BTreeSet::new();
    for mapping in mappings {
        if mapping.sequence != row.sequence {
            return Err(error(format!(
                "{} scene mapping family 불일치: {}",
                row.sequence, mapping.sequence
            )));
        }
        if !matches!(mapping.glyph, '\u{ac00}'..='\u{d7a3}') && mapping.glyph != ' ' {
            return Err(error(format!(
                "{} scene mapping이 한글 음절이나 공백이 아님: {}",
                row.sequence, mapping.glyph
            )));
        }
        if mapping.repertoire_index >= row.canonical_glyph_slots {
            return Err(error(format!(
                "{} scene mapping이 canonical prefix 밖임: {}",
                row.sequence, mapping.repertoire_index
            )));
        }
        cell_origin(row, mapping.repertoire_index)?;
        let expected_cell = usize::from(row.cell_base) + mapping.repertoire_index;
        if mapping.cell_index() != expected_cell || expected_cell > usize::from(row.cell_last) {
            return Err(error(format!(
                "{} scene mapping cell 계약 불일치: index={}, cell={}",
                row.sequence,
                mapping.repertoire_index,
                mapping.cell_index()
            )));
        }
        let first = canonical
            .chunks_exact(2)
            .position(|pair| pair == mapping.pair)
            .ok_or_else(|| {
                error(format!(
                    "{} scene mapping pair가 canonical prefix에 없음: {:02X}{:02X}",
                    row.sequence, mapping.pair[0], mapping.pair[1]
                ))
            })?;
        if first != mapping.repertoire_index {
            return Err(error(format!(
                "{} scene mapping pair first-match 불일치: expected={}, actual={first}",
                row.sequence, mapping.repertoire_index
            )));
        }
        if !glyphs.insert(mapping.glyph)
            || !indices.insert(mapping.repertoire_index)
            || !pairs.insert(mapping.pair)
        {
            return Err(error(format!(
                "{} scene mapping glyph/index/pair 중복",
                row.sequence
            )));
        }
    }
    Ok(indices)
}

fn cell_palette(
    pixels: &[u8],
    row: &SubgameSeqSceneGlyphRow,
    index: usize,
) -> Result<BTreeSet<u8>, SubgameSeqSceneGlyphPatchError> {
    let (x, y) = cell_origin(row, index)?;
    Ok((0..SCENE_GLYPH_CELL_HEIGHT)
        .flat_map(|cell_row| {
            pixels[(y + cell_row) * SCENE_GLYPH_ATLAS_LAYOUT.width + x
                ..(y + cell_row) * SCENE_GLYPH_ATLAS_LAYOUT.width + x + SCENE_GLYPH_CELL_WIDTH]
                .iter()
                .copied()
        })
        .collect())
}

fn patch_pixels(
    row: &SubgameSeqSceneGlyphRow,
    original: &[u8],
    mappings: &[SubgameSeqSceneGlyphMapping],
    mapped_indices: &BTreeSet<usize>,
    profile: SceneRasterProfile,
) -> Result<(Vec<u8>, Vec<u8>, usize), SubgameSeqSceneGlyphPatchError> {
    let font = if profile.font_id == crate::font::selected::GALMURI11 {
        crate::font::selected::load(crate::font::selected::GALMURI11).map_err(error)?
    } else {
        bundled_maplestory_light().map_err(|source| error(source.to_string()))?
    };
    let allowed_source_palette = BTreeSet::from([0, 1, 15]);
    let mut source_palette = BTreeSet::new();
    let mut output = original.to_vec();
    let mut allowed = vec![false; original.len()];
    for mapping in mappings {
        let palette = cell_palette(original, row, mapping.repertoire_index)?;
        if palette.is_empty() || !palette.is_subset(&allowed_source_palette) {
            return Err(error(format!(
                "{} scene source cell palette 역할 불일치: index={}, palette={palette:?}",
                row.sequence, mapping.repertoire_index
            )));
        }
        source_palette.extend(palette);
        let raster = if mapping.glyph == ' ' {
            let SceneRasterEffect::Binary { background, .. } = profile.effect;
            vec![background; SCENE_GLYPH_CELL_WIDTH * SCENE_GLYPH_CELL_HEIGHT]
        } else {
            rasterize_scene_glyph(&font, mapping.glyph, profile)
                .map_err(|source| error(source.to_string()))?
        };
        let (x, y) = cell_origin(row, mapping.repertoire_index)?;
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
            "{} scene glyph patch에서 변경된 pixel이 없음",
            row.sequence
        )));
    }
    for (offset, (&before, &after)) in original.iter().zip(&output).enumerate() {
        if before != after && !allowed[offset] {
            return Err(error(format!(
                "{} scene glyph patch 비대상 pixel 변경: offset={offset}",
                row.sequence
            )));
        }
    }
    if mapped_indices.len() != mappings.len() {
        return Err(error(format!(
            "{} scene mapping cell 분모 불일치",
            row.sequence
        )));
    }
    Ok((output, source_palette.into_iter().collect(), changed_pixels))
}

/// 검증된 PUYO1 family의 배정한 canonical cell만 Maplestory Light로 바꾼다.
pub fn patch_box_subgame_seq_scene_glyph_asset(
    row: &SubgameSeqSceneGlyphRow,
    sequence_raw: &[u8],
    plain: &[u8],
    mappings: &[SubgameSeqSceneGlyphMapping],
) -> Result<SubgameSeqSceneGlyphPatch, SubgameSeqSceneGlyphPatchError> {
    patch_box_subgame_seq_scene_glyph_asset_with_profile(
        row,
        sequence_raw,
        plain,
        mappings,
        SCENE_POC_RASTER_PROFILE,
    )
}

/// 검증된 PUYO1 family의 배정한 cell만 지정한 scene profile로 바꾼다.
pub fn patch_box_subgame_seq_scene_glyph_asset_with_profile(
    row: &SubgameSeqSceneGlyphRow,
    sequence_raw: &[u8],
    plain: &[u8],
    mappings: &[SubgameSeqSceneGlyphMapping],
    profile: SceneRasterProfile,
) -> Result<SubgameSeqSceneGlyphPatch, SubgameSeqSceneGlyphPatchError> {
    let original = verify_row_contract(row, sequence_raw, plain)?;
    let mapped_indices = validate_mappings(row, sequence_raw, mappings)?;
    let (patched_pixels, source_palette_indices, changed_pixels) =
        patch_pixels(row, &original, mappings, &mapped_indices, profile)?;
    let output = encode(&patched_pixels, SCENE_GLYPH_ATLAS_LAYOUT)
        .map_err(|source| error(format!("{} scene BTX encode 실패: {source}", row.sequence)))?;
    if decode(&output, SCENE_GLYPH_ATLAS_LAYOUT).map_err(|source| {
        error(format!(
            "{} scene BTX 재decode 실패: {source}",
            row.sequence
        ))
    })? != patched_pixels
    {
        return Err(error(format!(
            "{} scene BTX 수정 왕복 불일치",
            row.sequence
        )));
    }
    let changed_bytes = plain
        .iter()
        .zip(&output)
        .filter(|(before, after)| before != after)
        .count();
    let mut cell_indices = mapped_indices
        .into_iter()
        .map(|index| usize::from(row.cell_base) + index)
        .collect::<Vec<_>>();
    cell_indices.sort_unstable();
    Ok(SubgameSeqSceneGlyphPatch {
        scope: row.scope,
        sequence: row.sequence,
        archive: row.grp_archive,
        asset: row.same_stem_asset,
        plain: output.clone(),
        cell_indices,
        source_palette_indices,
        changed_pixels,
        changed_bytes,
        source_sha256: hex(&digest(plain)),
        output_sha256: hex(&digest(&output)),
    })
}

/// 지원 원본의 PUYO1 17개 family에 합성 translation mapping을 적용해 검산한다.
pub fn verify_box_subgame_seq_scene_glyph_patches<'a>(
    assets: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    glyphs: &SubgameSeqSceneGlyphAnalysis,
    mappings: &[SubgameSeqSceneGlyphMapping],
) -> Result<SubgameSeqSceneGlyphPatchAnalysis, SubgameSeqSceneGlyphPatchError> {
    if glyphs.rows.len() != BOX_SUBGAME_SEQ_SCENE_GLYPH_ROWS {
        return Err(error("scene glyph patch 분석 분모 불일치"));
    }
    let rows = glyphs
        .rows
        .iter()
        .filter(|row| row.scope == "PUYO1")
        .collect::<Vec<_>>();
    if rows.len() != BOX_SUBGAME_SEQ_SCENE_GLYPH_FAMILIES {
        return Err(error("scene glyph patch PUYO1 family 분모 불일치"));
    }
    let mut asset_map = BTreeMap::new();
    for (archive, asset, plain) in assets {
        if asset_map.insert((archive, asset), plain).is_some() {
            return Err(error("scene glyph patch asset 입력 중복"));
        }
    }
    let mut source_map = BTreeMap::new();
    for (archive, sequence, raw) in sources {
        if source_map.insert((archive, sequence), raw).is_some() {
            return Err(error("scene glyph patch SEQ 입력 중복"));
        }
    }
    if asset_map.len() != rows.len() || source_map.len() != rows.len() {
        return Err(error(format!(
            "scene glyph patch 입력 분모 불일치: assets={}, SEQ={}, expected={}",
            asset_map.len(),
            source_map.len(),
            rows.len()
        )));
    }

    let mut patches = Vec::new();
    for row in rows {
        let plain = asset_map
            .get(&(row.grp_archive, row.same_stem_asset))
            .copied()
            .ok_or_else(|| {
                error(format!(
                    "{}:{} scene glyph patch asset 누락",
                    row.grp_archive, row.same_stem_asset
                ))
            })?;
        let sequence_raw = source_map
            .get(&(row.seq_archive, row.sequence))
            .copied()
            .ok_or_else(|| {
                error(format!(
                    "{}:{} scene glyph patch SEQ 누락",
                    row.seq_archive, row.sequence
                ))
            })?;
        let family_mappings = mappings
            .iter()
            .filter(|mapping| mapping.sequence == row.sequence)
            .cloned()
            .collect::<Vec<_>>();
        patches.push(patch_box_subgame_seq_scene_glyph_asset(
            row,
            sequence_raw,
            plain,
            &family_mappings,
        )?);
    }
    if patches.len() != BOX_SUBGAME_SEQ_SCENE_GLYPH_PATCH_ASSETS
        || patches
            .iter()
            .map(|patch| patch.cell_indices.len())
            .sum::<usize>()
            != mappings.len()
        || mappings
            .iter()
            .map(|mapping| mapping.sequence.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != BOX_SUBGAME_SEQ_SCENE_GLYPH_FAMILIES
    {
        return Err(error("scene glyph patch 합성 mapping 분모 불일치"));
    }
    patches.sort_by_key(|patch| patch.sequence);
    let rows = patches
        .iter()
        .map(|patch| SubgameSeqSceneGlyphPatchRow {
            scope: patch.scope,
            sequence: patch.sequence,
            archive: patch.archive,
            asset: patch.asset,
            cell_indices: patch.cell_indices.clone(),
            source_palette_indices: patch.source_palette_indices.clone(),
            changed_pixels: patch.changed_pixels,
            changed_bytes: patch.changed_bytes,
            source_sha256: patch.source_sha256.clone(),
            output_sha256: patch.output_sha256.clone(),
        })
        .collect::<Vec<_>>();
    let analysis = SubgameSeqSceneGlyphPatchAnalysis {
        assets: rows.len(),
        cells: rows.iter().map(|row| row.cell_indices.len()).sum(),
        source_palette_indices: rows
            .iter()
            .flat_map(|row| row.source_palette_indices.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        changed_pixels: rows.iter().map(|row| row.changed_pixels).sum(),
        changed_bytes: rows.iter().map(|row| row.changed_bytes).sum(),
        rows,
    };
    let report = render_subgame_seq_scene_glyph_patches_tsv(&analysis);
    let report_sha256 = hex(&digest(report.as_bytes()));
    let actual = (
        analysis.assets,
        analysis.cells,
        analysis.changed_pixels,
        analysis.changed_bytes,
    );
    let expected = (
        BOX_SUBGAME_SEQ_SCENE_GLYPH_PATCH_ASSETS,
        BOX_SUBGAME_SEQ_SCENE_GLYPH_PATCH_CELLS,
        BOX_SUBGAME_SEQ_SCENE_GLYPH_PATCH_CHANGED_PIXELS,
        BOX_SUBGAME_SEQ_SCENE_GLYPH_PATCH_CHANGED_BYTES,
    );
    if actual != expected || report_sha256 != BOX_SUBGAME_SEQ_SCENE_GLYPH_PATCH_REPORT_SHA256 {
        return Err(error(format!(
            "scene glyph patch 합성 기준선 불일치: expected={expected:?}/{}, actual={actual:?}/{report_sha256}",
            BOX_SUBGAME_SEQ_SCENE_GLYPH_PATCH_REPORT_SHA256
        )));
    }
    Ok(analysis)
}

pub fn render_subgame_seq_scene_glyph_patches_tsv(
    analysis: &SubgameSeqSceneGlyphPatchAnalysis,
) -> String {
    let mut output = String::from(
        "schema_version\tscope\tsequence\tarchive\tasset\tcell_indices\tsource_palette_indices\tchanged_pixels\tchanged_bytes\tsource_sha256\toutput_sha256\tstatic_status\n",
    );
    for row in &analysis.rows {
        let cells = row
            .cell_indices
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let palette = row
            .source_palette_indices
            .iter()
            .map(|value| format!("{value:02X}"))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tpuyo1-selected-same-stem-first-match-cell-only-maplestory-light-runtime-unverified",
            SUBGAME_SEQ_SCENE_GLYPH_PATCH_SCHEMA_VERSION,
            row.scope,
            row.sequence,
            row.archive,
            row.asset,
            cells,
            palette,
            row.changed_pixels,
            row.changed_bytes,
            row.source_sha256,
            row.output_sha256,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

#[cfg(test)]
#[path = "subgame_seq_scene_glyph_patch_tests.rs"]
mod tests;
