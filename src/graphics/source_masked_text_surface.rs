//! 원문 글자와 보호 pixel을 명시한 raster block을 한 surface로 합성한다.
//!
//! source text mask는 지울 원문 픽셀만, 전역 protected mask는 절대로 바꾸지
//! 않을 픽셀만 소유한다. 나머지 배경 영역에는 이미 수동 조판한 text layer를
//! 합성할 수 있다. mask 검출과 문장·줄바꿈 작성은 이 모듈의 책임이 아니다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use super::text_catalog::{
    is_japanese_translation_target, verify_text_asset, PaletteIndexSpec, TextAssetSpec,
    TextCatalogError, TextStorageSpec,
};
use crate::btx::{decode, encode, BtxError};

/// 전체 surface에서 변경을 금지할 pixel 집합과 수동 identity.
#[derive(Debug, Clone, Copy)]
pub struct ProtectedPixelMask<'a> {
    pub pixels: &'a [bool],
    pub expected_pixels: usize,
    pub expected_sha256: &'a str,
}

/// source text mask가 선택한 픽셀을 어떤 배경으로 되돌릴지 지정한다.
///
/// 단색 surface는 `Fill`을, 문구 아래에 질감이나 도형이 남는 surface는
/// 지원 원본에서 결정적으로 재생성한 block 크기의 `Restore`를 사용한다.
#[derive(Debug, Clone, Copy)]
pub enum SourceTextRemoval<'a> {
    Fill {
        background_index: u8,
    },
    Restore {
        pixels: &'a [u8],
        expected_pixels_sha256: &'a str,
    },
}

/// 한 block의 원문 글자 제거와 수동 조판 layer.
#[derive(Debug, Clone, Copy)]
pub struct SourceMaskedTextReplacement<'a> {
    pub block_id: &'a str,
    pub source_text_pixels: &'a [bool],
    pub expected_source_text_pixels: usize,
    pub expected_source_text_mask_sha256: &'a str,
    pub removal: SourceTextRemoval<'a>,
    /// 이 index를 투명 픽셀로 쓰는 block 크기의 수동 조판 layer.
    pub transparent_index: u8,
    pub rendered_pixels: &'a [u8],
}

/// source mask 합성의 정적 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMaskedTextSurfaceReport {
    pub replacement_blocks: usize,
    pub source_text_pixels: usize,
    pub rendered_pixels: usize,
    pub protected_pixels: usize,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub protected_pixels_exact: bool,
    pub output_sha256: String,
}

/// source mask·보호 경계·원본 identity 검증 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceMaskedTextSurfaceError {
    Catalog(TextCatalogError),
    Btx(BtxError),
    StorageRequiresRasterBlocks(String),
    EmptyReplacements,
    DuplicateBlockId(String),
    UnknownBlockId(String),
    ReferenceOnlyBlock(String),
    ProtectedMaskLengthMismatch { expected: usize, actual: usize },
    ProtectedMaskIdentityMismatch,
    BlockInputLengthMismatch(String),
    RestoredBackgroundIdentityMismatch(String),
    SourceTextMaskIdentityMismatch(String),
    SourceTextMaskIsEmpty(String),
    SourceTextMaskSelectsUnchangedPixel { id: String, x: usize, y: usize },
    SourceTextOverlapsProtectedPixel { id: String, x: usize, y: usize },
    UnclassifiedSourceDifference { id: String, x: usize, y: usize },
    PaletteIndexNotDeclared { id: String, value: u8 },
    RenderTouchesProtectedPixel { id: String, x: usize, y: usize },
    UnprotectedPixelOutsideReplacement { x: usize, y: usize },
    ProtectedPixelChanged { x: usize, y: usize },
    NoChangedPixels,
}

impl fmt::Display for SourceMaskedTextSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalog(source) => write!(formatter, "source mask catalog 실패: {source}"),
            Self::Btx(source) => write!(formatter, "source mask BTX codec 실패: {source}"),
            Self::StorageRequiresRasterBlocks(name) => {
                write!(
                    formatter,
                    "{name}: source mask composer에는 raster block storage가 필요함"
                )
            }
            Self::EmptyReplacements => formatter.write_str("source mask replacement가 비어 있음"),
            Self::DuplicateBlockId(id) => write!(formatter, "source mask block ID 중복: {id}"),
            Self::UnknownBlockId(id) => write!(formatter, "catalog에 없는 source mask block: {id}"),
            Self::ReferenceOnlyBlock(id) => write!(formatter, "{id}: 보존 block은 교체할 수 없음"),
            Self::ProtectedMaskLengthMismatch { expected, actual } => write!(
                formatter,
                "전역 보호 mask 크기 불일치: expected={expected}, actual={actual}"
            ),
            Self::ProtectedMaskIdentityMismatch => {
                formatter.write_str("전역 보호 mask count 또는 SHA-256 불일치")
            }
            Self::BlockInputLengthMismatch(id) => {
                write!(formatter, "{id}: block mask 또는 layer 크기 불일치")
            }
            Self::RestoredBackgroundIdentityMismatch(id) => {
                write!(formatter, "{id}: 복원 배경 pixel SHA-256 불일치")
            }
            Self::SourceTextMaskIdentityMismatch(id) => {
                write!(
                    formatter,
                    "{id}: source text mask count 또는 SHA-256 불일치"
                )
            }
            Self::SourceTextMaskIsEmpty(id) => {
                write!(formatter, "{id}: source text mask가 비어 있음")
            }
            Self::SourceTextMaskSelectsUnchangedPixel { id, x, y } => {
                write!(
                    formatter,
                    "{id}: source text mask가 제거 전후 같은 pixel을 선택함: ({x},{y})"
                )
            }
            Self::SourceTextOverlapsProtectedPixel { id, x, y } => {
                write!(
                    formatter,
                    "{id}: source text와 보호 pixel이 겹침: ({x},{y})"
                )
            }
            Self::UnclassifiedSourceDifference { id, x, y } => write!(
                formatter,
                "{id}: 원본과 제거 배경의 차이가 글자 또는 보호 영역으로 분류되지 않음: ({x},{y})"
            ),
            Self::PaletteIndexNotDeclared { id, value } => {
                write!(
                    formatter,
                    "{id}: 선언되지 않은 palette index를 합성함: {value}"
                )
            }
            Self::RenderTouchesProtectedPixel { id, x, y } => {
                write!(
                    formatter,
                    "{id}: 새 text layer가 보호 pixel을 침범함: ({x},{y})"
                )
            }
            Self::UnprotectedPixelOutsideReplacement { x, y } => write!(
                formatter,
                "replacement 밖 pixel이 보호 mask에서 빠짐: ({x},{y})"
            ),
            Self::ProtectedPixelChanged { x, y } => {
                write!(formatter, "보호 pixel이 변경됨: ({x},{y})")
            }
            Self::NoChangedPixels => {
                formatter.write_str("source mask 합성에서 변경된 pixel이 없음")
            }
        }
    }
}

impl Error for SourceMaskedTextSurfaceError {}

impl From<TextCatalogError> for SourceMaskedTextSurfaceError {
    fn from(source: TextCatalogError) -> Self {
        Self::Catalog(source)
    }
}

impl From<BtxError> for SourceMaskedTextSurfaceError {
    fn from(source: BtxError) -> Self {
        Self::Btx(source)
    }
}

fn mask_digest(mask: &[bool]) -> String {
    let mut digest = Sha256::new();
    for &selected in mask {
        digest.update([u8::from(selected)]);
    }
    format!("{:x}", digest.finalize())
}

fn pixels_digest(pixels: &[u8]) -> String {
    format!("{:x}", Sha256::digest(pixels))
}

fn palette_contains(spec: PaletteIndexSpec<'_>, value: u8) -> bool {
    match spec {
        PaletteIndexSpec::Values(values) => values.contains(&value),
        PaletteIndexSpec::Bitset256(words) => {
            let value = usize::from(value);
            words[value / 64] & (1u64 << (value % 64)) != 0
        }
    }
}

/// 모든 입력과 보호 경계를 검증한 뒤 전체 surface를 한 번 encode한다.
pub fn compose_source_masked_text_surface(
    original: &[u8],
    asset: TextAssetSpec<'_>,
    protected: ProtectedPixelMask<'_>,
    replacements: &[SourceMaskedTextReplacement<'_>],
) -> Result<(Vec<u8>, SourceMaskedTextSurfaceReport), SourceMaskedTextSurfaceError> {
    verify_text_asset(original, asset)?;
    if asset.storage != TextStorageSpec::RasterBlocks {
        return Err(SourceMaskedTextSurfaceError::StorageRequiresRasterBlocks(
            asset.name.to_string(),
        ));
    }
    if replacements.is_empty() {
        return Err(SourceMaskedTextSurfaceError::EmptyReplacements);
    }

    let original_pixels = decode(original, asset.layout)?;
    if protected.pixels.len() != original_pixels.len() {
        return Err(SourceMaskedTextSurfaceError::ProtectedMaskLengthMismatch {
            expected: original_pixels.len(),
            actual: protected.pixels.len(),
        });
    }
    let protected_pixels = protected.pixels.iter().filter(|value| **value).count();
    if protected_pixels != protected.expected_pixels
        || mask_digest(protected.pixels) != protected.expected_sha256
    {
        return Err(SourceMaskedTextSurfaceError::ProtectedMaskIdentityMismatch);
    }

    let mut ids = BTreeSet::new();
    let mut target_pixels = vec![false; original_pixels.len()];
    let mut output_pixels = original_pixels.clone();
    let mut source_text_pixels = 0usize;
    let mut rendered_pixels = 0usize;

    for replacement in replacements {
        if !ids.insert(replacement.block_id) {
            return Err(SourceMaskedTextSurfaceError::DuplicateBlockId(
                replacement.block_id.to_string(),
            ));
        }
        let block = asset
            .blocks
            .iter()
            .copied()
            .find(|block| block.id == replacement.block_id)
            .ok_or_else(|| {
                SourceMaskedTextSurfaceError::UnknownBlockId(replacement.block_id.to_string())
            })?;
        if !is_japanese_translation_target(block.source) {
            return Err(SourceMaskedTextSurfaceError::ReferenceOnlyBlock(
                block.id.to_string(),
            ));
        }
        let block_pixels = block
            .bbox
            .width
            .checked_mul(block.bbox.height)
            .ok_or_else(|| {
                SourceMaskedTextSurfaceError::BlockInputLengthMismatch(block.id.to_string())
            })?;
        if replacement.source_text_pixels.len() != block_pixels
            || replacement.rendered_pixels.len() != block_pixels
        {
            return Err(SourceMaskedTextSurfaceError::BlockInputLengthMismatch(
                block.id.to_string(),
            ));
        }
        let selected = replacement
            .source_text_pixels
            .iter()
            .filter(|value| **value)
            .count();
        if selected == 0 {
            return Err(SourceMaskedTextSurfaceError::SourceTextMaskIsEmpty(
                block.id.to_string(),
            ));
        }
        if selected != replacement.expected_source_text_pixels
            || mask_digest(replacement.source_text_pixels)
                != replacement.expected_source_text_mask_sha256
        {
            return Err(
                SourceMaskedTextSurfaceError::SourceTextMaskIdentityMismatch(block.id.to_string()),
            );
        }
        if !palette_contains(block.palette_indices, replacement.transparent_index) {
            return Err(SourceMaskedTextSurfaceError::PaletteIndexNotDeclared {
                id: block.id.to_string(),
                value: replacement.transparent_index,
            });
        }
        let restored_pixels = match replacement.removal {
            SourceTextRemoval::Fill { background_index } => {
                if !palette_contains(block.palette_indices, background_index) {
                    return Err(SourceMaskedTextSurfaceError::PaletteIndexNotDeclared {
                        id: block.id.to_string(),
                        value: background_index,
                    });
                }
                None
            }
            SourceTextRemoval::Restore {
                pixels,
                expected_pixels_sha256,
            } => {
                if pixels.len() != block_pixels {
                    return Err(SourceMaskedTextSurfaceError::BlockInputLengthMismatch(
                        block.id.to_string(),
                    ));
                }
                if pixels_digest(pixels) != expected_pixels_sha256 {
                    return Err(
                        SourceMaskedTextSurfaceError::RestoredBackgroundIdentityMismatch(
                            block.id.to_string(),
                        ),
                    );
                }
                if let Some(&value) = pixels
                    .iter()
                    .find(|value| !palette_contains(block.palette_indices, **value))
                {
                    return Err(SourceMaskedTextSurfaceError::PaletteIndexNotDeclared {
                        id: block.id.to_string(),
                        value,
                    });
                }
                Some(pixels)
            }
        };

        for local_y in 0..block.bbox.height {
            for local_x in 0..block.bbox.width {
                let local = local_y * block.bbox.width + local_x;
                let x = block.bbox.x + local_x;
                let y = block.bbox.y + local_y;
                let global = y * asset.layout.width + x;
                target_pixels[global] = true;
                let source_text = replacement.source_text_pixels[local];
                let is_protected = protected.pixels[global];
                let original_value = original_pixels[global];
                let removed_value = match replacement.removal {
                    SourceTextRemoval::Fill { background_index } => background_index,
                    SourceTextRemoval::Restore { .. } => {
                        restored_pixels.expect("Restore이면 검증된 복원 배경이 있음")[local]
                    }
                };
                if source_text && original_value == removed_value {
                    return Err(
                        SourceMaskedTextSurfaceError::SourceTextMaskSelectsUnchangedPixel {
                            id: block.id.to_string(),
                            x,
                            y,
                        },
                    );
                }
                if source_text && is_protected {
                    return Err(
                        SourceMaskedTextSurfaceError::SourceTextOverlapsProtectedPixel {
                            id: block.id.to_string(),
                            x,
                            y,
                        },
                    );
                }
                if original_value != removed_value && !source_text && !is_protected {
                    return Err(SourceMaskedTextSurfaceError::UnclassifiedSourceDifference {
                        id: block.id.to_string(),
                        x,
                        y,
                    });
                }
                if source_text {
                    output_pixels[global] = removed_value;
                }
                let rendered = replacement.rendered_pixels[local];
                if !palette_contains(block.palette_indices, rendered) {
                    return Err(SourceMaskedTextSurfaceError::PaletteIndexNotDeclared {
                        id: block.id.to_string(),
                        value: rendered,
                    });
                }
                if rendered != replacement.transparent_index {
                    if is_protected {
                        return Err(SourceMaskedTextSurfaceError::RenderTouchesProtectedPixel {
                            id: block.id.to_string(),
                            x,
                            y,
                        });
                    }
                    output_pixels[global] = rendered;
                    rendered_pixels += 1;
                }
            }
        }
        source_text_pixels += selected;
    }

    if let Some(offset) = target_pixels
        .iter()
        .zip(protected.pixels)
        .position(|(target, protected)| !target && !protected)
    {
        return Err(
            SourceMaskedTextSurfaceError::UnprotectedPixelOutsideReplacement {
                x: offset % asset.layout.width,
                y: offset / asset.layout.width,
            },
        );
    }
    if let Some(offset) = original_pixels
        .iter()
        .zip(&output_pixels)
        .zip(protected.pixels)
        .position(|((before, after), protected)| *protected && before != after)
    {
        return Err(SourceMaskedTextSurfaceError::ProtectedPixelChanged {
            x: offset % asset.layout.width,
            y: offset / asset.layout.width,
        });
    }

    let changed_pixels = original_pixels
        .iter()
        .zip(&output_pixels)
        .filter(|(before, after)| before != after)
        .count();
    if changed_pixels == 0 {
        return Err(SourceMaskedTextSurfaceError::NoChangedPixels);
    }
    let output = encode(&output_pixels, asset.layout)?;
    let changed_bytes = original
        .iter()
        .zip(&output)
        .filter(|(before, after)| before != after)
        .count();
    let output_sha256 = format!("{:x}", Sha256::digest(&output));
    Ok((
        output,
        SourceMaskedTextSurfaceReport {
            replacement_blocks: replacements.len(),
            source_text_pixels,
            rendered_pixels,
            protected_pixels,
            changed_pixels,
            changed_bytes,
            protected_pixels_exact: true,
            output_sha256,
        },
    ))
}

#[cfg(test)]
#[path = "source_masked_text_surface_tests.rs"]
mod tests;
