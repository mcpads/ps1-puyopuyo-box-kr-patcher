//! 겹치는 selector block을 하나의 최종 BTX surface로 합성한다.
//!
//! block 적용 순서는 결과를 결정하지 않는다. 겹친 영역은 양쪽 replacement,
//! 또는 replacement와 보존 원본이 같은 픽셀을 제안할 때만 허용한다. 모든
//! 입력을 검증한 뒤 전체 surface를 한 번 encode하므로 부분 출력은 없다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::text_catalog::{
    is_japanese_translation_target, verify_text_asset, PaletteIndexSpec, Rect, TextAssetSpec,
    TextCatalogError, TextStorageSpec,
};
use crate::btx::{decode, encode, BtxError};

/// 한 selector bbox 전체의 행 우선 indexed pixel replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SparseSelectorRegionReplacement<'a> {
    pub block_id: &'a str,
    pub pixels: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseSelectorSurfaceReport {
    pub replacement_blocks: usize,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SparseSelectorSurfaceError {
    Catalog(TextCatalogError),
    Btx(BtxError),
    StorageRequiresSparseSelectorAtlas(String),
    EmptyReplacements,
    EmptyBlockId,
    DuplicateBlockId(String),
    UnknownBlockId(String),
    ReferenceOnlyBlock(String),
    PixelCountMismatch {
        id: String,
        expected: usize,
        actual: usize,
    },
    PaletteIndexNotDeclared {
        id: String,
        value: u8,
    },
    OverlapPixelConflict {
        first: String,
        second: String,
        x: usize,
        y: usize,
    },
    NoChangedPixels,
}

impl fmt::Display for SparseSelectorSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalog(source) => write!(formatter, "sparse catalog 검증 실패: {source}"),
            Self::Btx(source) => write!(formatter, "sparse BTX codec 실패: {source}"),
            Self::StorageRequiresSparseSelectorAtlas(name) => write!(
                formatter,
                "{name}: sparse selector surface composer에는 sparse storage가 필요함"
            ),
            Self::EmptyReplacements => {
                formatter.write_str("sparse selector replacement가 비어 있음")
            }
            Self::EmptyBlockId => {
                formatter.write_str("sparse selector replacement block ID가 비어 있음")
            }
            Self::DuplicateBlockId(id) => {
                write!(formatter, "sparse selector replacement block ID 중복: {id}")
            }
            Self::UnknownBlockId(id) => write!(formatter, "catalog에 없는 sparse block: {id}"),
            Self::ReferenceOnlyBlock(id) => {
                write!(
                    formatter,
                    "{id}: 원형 유지 reference block은 교체할 수 없음"
                )
            }
            Self::PixelCountMismatch {
                id,
                expected,
                actual,
            } => write!(
                formatter,
                "{id}: replacement pixel 수 불일치: expected={expected}, actual={actual}"
            ),
            Self::PaletteIndexNotDeclared { id, value } => {
                write!(
                    formatter,
                    "{id}: 원본 block에 없는 palette index를 요청함: {value}"
                )
            }
            Self::OverlapPixelConflict {
                first,
                second,
                x,
                y,
            } => write!(
                formatter,
                "sparse selector 겹침 pixel 제안 충돌: {first}/{second} at ({x},{y})"
            ),
            Self::NoChangedPixels => {
                formatter.write_str("sparse selector 합성에서 변경된 픽셀이 없음")
            }
        }
    }
}

impl Error for SparseSelectorSurfaceError {}

impl From<TextCatalogError> for SparseSelectorSurfaceError {
    fn from(source: TextCatalogError) -> Self {
        Self::Catalog(source)
    }
}

impl From<BtxError> for SparseSelectorSurfaceError {
    fn from(source: BtxError) -> Self {
        Self::Btx(source)
    }
}

fn palette_indices(spec: PaletteIndexSpec<'_>) -> BTreeSet<u8> {
    match spec {
        PaletteIndexSpec::Values(values) => values.iter().copied().collect(),
        PaletteIndexSpec::Bitset256(words) => (0u16..=255)
            .filter(|value| {
                let value = usize::from(*value);
                words[value / 64] & (1u64 << (value % 64)) != 0
            })
            .map(|value| value as u8)
            .collect(),
    }
}

fn intersection(left: Rect, right: Rect) -> Option<Rect> {
    let x = left.x.max(right.x);
    let y = left.y.max(right.y);
    let right_edge = (left.x + left.width).min(right.x + right.width);
    let bottom_edge = (left.y + left.height).min(right.y + right.height);
    (x < right_edge && y < bottom_edge).then_some(Rect {
        x,
        y,
        width: right_edge - x,
        height: bottom_edge - y,
    })
}

fn proposed_pixel(
    original: &[u8],
    layout_width: usize,
    bbox: Rect,
    replacement: Option<&SparseSelectorRegionReplacement<'_>>,
    x: usize,
    y: usize,
) -> u8 {
    replacement.map_or_else(
        || original[y * layout_width + x],
        |replacement| replacement.pixels[(y - bbox.y) * bbox.width + (x - bbox.x)],
    )
}

/// replacement들을 검증하고 전체 해제 surface를 한 번만 다시 encode한다.
pub fn compose_sparse_selector_surface(
    data: &[u8],
    spec: TextAssetSpec<'_>,
    replacements: &[SparseSelectorRegionReplacement<'_>],
) -> Result<(Vec<u8>, SparseSelectorSurfaceReport), SparseSelectorSurfaceError> {
    verify_text_asset(data, spec)?;
    let TextStorageSpec::SparseSelectorAtlas(sparse) = spec.storage else {
        return Err(
            SparseSelectorSurfaceError::StorageRequiresSparseSelectorAtlas(spec.name.to_string()),
        );
    };
    if replacements.is_empty() {
        return Err(SparseSelectorSurfaceError::EmptyReplacements);
    }

    let blocks = spec
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let mut replacement_by_id = BTreeMap::new();
    for replacement in replacements {
        if replacement.block_id.is_empty() {
            return Err(SparseSelectorSurfaceError::EmptyBlockId);
        }
        if replacement_by_id
            .insert(replacement.block_id, replacement)
            .is_some()
        {
            return Err(SparseSelectorSurfaceError::DuplicateBlockId(
                replacement.block_id.to_string(),
            ));
        }
        let block = blocks.get(replacement.block_id).ok_or_else(|| {
            SparseSelectorSurfaceError::UnknownBlockId(replacement.block_id.to_string())
        })?;
        if !is_japanese_translation_target(block.source) {
            return Err(SparseSelectorSurfaceError::ReferenceOnlyBlock(
                replacement.block_id.to_string(),
            ));
        }
        let expected = block
            .bbox
            .width
            .checked_mul(block.bbox.height)
            .ok_or_else(|| SparseSelectorSurfaceError::PixelCountMismatch {
                id: replacement.block_id.to_string(),
                expected: usize::MAX,
                actual: replacement.pixels.len(),
            })?;
        if replacement.pixels.len() != expected {
            return Err(SparseSelectorSurfaceError::PixelCountMismatch {
                id: replacement.block_id.to_string(),
                expected,
                actual: replacement.pixels.len(),
            });
        }
        let allowed = palette_indices(block.palette_indices);
        if let Some(value) = replacement
            .pixels
            .iter()
            .copied()
            .find(|value| !allowed.contains(value))
        {
            return Err(SparseSelectorSurfaceError::PaletteIndexNotDeclared {
                id: replacement.block_id.to_string(),
                value,
            });
        }
    }

    let original = decode(data, spec.layout)?;
    for &(first_id, second_id) in sparse.allowed_overlap_pairs {
        let first = blocks[first_id];
        let second = blocks[second_id];
        let overlap = intersection(first.bbox, second.bbox)
            .expect("catalog가 exact overlap 쌍을 먼저 검증함");
        let first_replacement = replacement_by_id.get(first_id).copied();
        let second_replacement = replacement_by_id.get(second_id).copied();
        if first_replacement.is_none() && second_replacement.is_none() {
            continue;
        }
        for y in overlap.y..overlap.y + overlap.height {
            for x in overlap.x..overlap.x + overlap.width {
                let first_value = proposed_pixel(
                    &original,
                    spec.layout.width,
                    first.bbox,
                    first_replacement,
                    x,
                    y,
                );
                let second_value = proposed_pixel(
                    &original,
                    spec.layout.width,
                    second.bbox,
                    second_replacement,
                    x,
                    y,
                );
                if first_value != second_value {
                    return Err(SparseSelectorSurfaceError::OverlapPixelConflict {
                        first: first_id.to_string(),
                        second: second_id.to_string(),
                        x,
                        y,
                    });
                }
            }
        }
    }

    let mut output_pixels = original.clone();
    for (block_id, replacement) in &replacement_by_id {
        let block = blocks[block_id];
        for local_y in 0..block.bbox.height {
            let source_start = local_y * block.bbox.width;
            let target_start = (block.bbox.y + local_y) * spec.layout.width + block.bbox.x;
            output_pixels[target_start..target_start + block.bbox.width].copy_from_slice(
                &replacement.pixels[source_start..source_start + block.bbox.width],
            );
        }
    }
    let changed_pixels = original
        .iter()
        .zip(&output_pixels)
        .filter(|(before, after)| before != after)
        .count();
    if changed_pixels == 0 {
        return Err(SparseSelectorSurfaceError::NoChangedPixels);
    }
    let output = encode(&output_pixels, spec.layout)?;
    let changed_bytes = data
        .iter()
        .zip(&output)
        .filter(|(before, after)| before != after)
        .count();
    let output_sha256 = format!("{:x}", Sha256::digest(&output));
    Ok((
        output,
        SparseSelectorSurfaceReport {
            replacement_blocks: replacements.len(),
            changed_pixels,
            changed_bytes,
            output_sha256,
        },
    ))
}

#[cfg(test)]
#[path = "sparse_selector_surface_tests.rs"]
mod tests;
