//! Baked BTX 일본어 block의 한글 픽셀 합성.
//!
//! 원본 계약을 먼저 검증하고, 승인한 block bbox 안에서만 category별
//! font profile의 글리프와 효과를 합성한다. 영문·숫자 참고 block,
//! 잘리는 문자열, 원본 block에 없던 palette index는 실패한다. 모든
//! replacement를 먼저 검증·합성한 뒤 새 BTX를 반환하므로 실패 시 부분
//! 출력은 없다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use fontdue::{Font, Metrics};
use sha2::{Digest, Sha256};

use super::text_catalog::{
    is_japanese_translation_target, verify_text_asset, PaletteIndexSpec, TextAssetSpec,
    TextBlockSpec, TextCatalogError, TextStorageSpec,
};
use super::text_profile::{BakedFontBook, BakedTextEffect, BakedTextProfile, HorizontalAlign};
use crate::btx::{decode, encode, BtxError, PixelFormat};

/// 한 catalog block의 한글 replacement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BakedTextReplacement<'a> {
    pub block_id: &'a str,
    pub text: &'a str,
    pub profile: &'a BakedTextProfile<'a>,
}

/// baked text patch 결과 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BakedTextPatchReport {
    pub replacement_blocks: usize,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub output_sha256: String,
}

/// baked text patch 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BakedTextPatchError {
    Catalog(TextCatalogError),
    Btx(BtxError),
    EmptyReplacements,
    FixedCellGridRequiresAssembler(String),
    SparseSelectorAtlasRequiresSurfaceComposer(String),
    EmptyBlockId,
    DuplicateBlockId(String),
    UnknownBlockId(String),
    ReferenceOnlyBlock(String),
    EmptyTranslation(String),
    CarriageReturn(String),
    LineCountMismatch {
        id: String,
        expected: usize,
        actual: usize,
    },
    MissingHangul(String),
    EmptyProfileId(String),
    EmptyFontId(String),
    UnknownFont {
        id: String,
        font_id: String,
    },
    InvalidRasterProfile(String),
    EmptyPaletteRamp(String),
    DuplicatePaletteIndex {
        id: String,
        value: u8,
    },
    BackgroundInBodyRamp {
        id: String,
        value: u8,
    },
    PaletteIndexNotDeclared {
        id: String,
        value: u8,
    },
    EmptyGlyph {
        id: String,
        glyph: char,
    },
    TextDoesNotFit {
        id: String,
        width: usize,
        height: usize,
        needed_width: usize,
        top: i32,
        bottom: i32,
    },
    NoChangedPixels,
}

impl fmt::Display for BakedTextPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalog(source) => write!(f, "BTX 원본 계약 실패: {source}"),
            Self::Btx(source) => write!(f, "BTX codec 실패: {source}"),
            Self::EmptyReplacements => f.write_str("baked text replacement가 비어 있음"),
            Self::FixedCellGridRequiresAssembler(name) => write!(
                f,
                "{name}: fixed cell grid는 일반 bbox rasterizer가 아니라 cell assembler가 필요함"
            ),
            Self::SparseSelectorAtlasRequiresSurfaceComposer(name) => write!(
                f,
                "{name}: sparse selector atlas는 일반 bbox rasterizer가 아니라 전체 surface composer가 필요함"
            ),
            Self::EmptyBlockId => f.write_str("baked text replacement block ID가 비어 있음"),
            Self::DuplicateBlockId(id) => write!(f, "baked text replacement block ID 중복: {id}"),
            Self::UnknownBlockId(id) => write!(f, "catalog에 없는 baked text block: {id}"),
            Self::ReferenceOnlyBlock(id) => {
                write!(f, "{id}: 영문·숫자 원형 유지 block은 번역할 수 없음")
            }
            Self::EmptyTranslation(id) => write!(f, "{id}: 한글 번역문이 비어 있음"),
            Self::CarriageReturn(id) => write!(f, "{id}: 번역문에 CR 문자가 있음"),
            Self::LineCountMismatch {
                id,
                expected,
                actual,
            } => write!(
                f,
                "{id}: 번역문 행 수 불일치: expected={expected}, actual={actual}"
            ),
            Self::MissingHangul(id) => write!(f, "{id}: 번역문에 한글 음절이 없음"),
            Self::EmptyProfileId(id) => write!(f, "{id}: baked text profile ID가 비어 있음"),
            Self::EmptyFontId(id) => write!(f, "{id}: baked text font ID가 비어 있음"),
            Self::UnknownFont { id, font_id } => {
                write!(f, "{id}: font book에 없는 baked font ID: {font_id}")
            }
            Self::InvalidRasterProfile(id) => {
                write!(f, "{id}: font size·baseline·tracking 설정이 유효하지 않음")
            }
            Self::EmptyPaletteRamp(id) => write!(f, "{id}: body palette ramp가 비어 있음"),
            Self::DuplicatePaletteIndex { id, value } => {
                write!(f, "{id}: body palette ramp index 중복: {value}")
            }
            Self::BackgroundInBodyRamp { id, value } => {
                write!(f, "{id}: background와 body palette index가 같음: {value}")
            }
            Self::PaletteIndexNotDeclared { id, value } => {
                write!(f, "{id}: 원본 block에 없는 palette index를 요청함: {value}")
            }
            Self::EmptyGlyph { id, glyph } => {
                write!(f, "{id}: 선택한 font가 빈 글리프를 반환: {glyph}")
            }
            Self::TextDoesNotFit {
                id,
                width,
                height,
                needed_width,
                top,
                bottom,
            } => write!(
                f,
                "{id}: 번역문이 bbox 안에 들어가지 않음: bbox={width}x{height}, \
                 needed_width={needed_width}, vertical={top}..{bottom}"
            ),
            Self::NoChangedPixels => f.write_str("baked text patch에서 변경된 픽셀이 없음"),
        }
    }
}

impl Error for BakedTextPatchError {}

impl From<TextCatalogError> for BakedTextPatchError {
    fn from(source: TextCatalogError) -> Self {
        Self::Catalog(source)
    }
}

impl From<BtxError> for BakedTextPatchError {
    fn from(source: BtxError) -> Self {
        Self::Btx(source)
    }
}

#[derive(Debug)]
struct RasterGlyph {
    metrics: Metrics,
    bitmap: Vec<u8>,
    pen_x: i32,
}

fn contains_hangul_syllable(text: &str) -> bool {
    text.chars()
        .any(|character| matches!(character, '\u{ac00}'..='\u{d7a3}'))
}

fn palette_contains(spec: PaletteIndexSpec<'_>, value: u8) -> bool {
    match spec {
        PaletteIndexSpec::Values(values) => values.contains(&value),
        PaletteIndexSpec::Bitset256(words) => {
            let word = usize::from(value / 64);
            let bit = u32::from(value % 64);
            words[word] & (1u64 << bit) != 0
        }
    }
}

fn validate_profile(
    block: TextBlockSpec<'_>,
    format: PixelFormat,
    profile: BakedTextProfile<'_>,
) -> Result<(), BakedTextPatchError> {
    if profile.id.is_empty() {
        return Err(BakedTextPatchError::EmptyProfileId(block.id.to_string()));
    }
    if profile.font_id.is_empty() {
        return Err(BakedTextPatchError::EmptyFontId(block.id.to_string()));
    }
    if !profile.font_size.is_finite()
        || profile.font_size <= 0.0
        || profile.baseline < 0
        || profile.line_count == 0
        || profile.line_height == 0
        || usize::try_from(profile.baseline).map_or(true, |baseline| baseline > block.bbox.height)
    {
        return Err(BakedTextPatchError::InvalidRasterProfile(
            block.id.to_string(),
        ));
    }
    let last_baseline = profile
        .line_count
        .checked_sub(1)
        .and_then(|lines| lines.checked_mul(profile.line_height))
        .and_then(|offset| usize::try_from(profile.baseline).ok()?.checked_add(offset));
    if last_baseline.is_none_or(|baseline| baseline > block.bbox.height) {
        return Err(BakedTextPatchError::InvalidRasterProfile(
            block.id.to_string(),
        ));
    }
    if profile.body_ramp.is_empty() {
        return Err(BakedTextPatchError::EmptyPaletteRamp(block.id.to_string()));
    }
    if matches!(
        profile.effect,
        BakedTextEffect::Shadow {
            offset_x: 0,
            offset_y: 0,
            ..
        }
    ) {
        return Err(BakedTextPatchError::InvalidRasterProfile(
            block.id.to_string(),
        ));
    }

    let limit = match format {
        PixelFormat::Indexed4 => 15,
        PixelFormat::Indexed8 => u8::MAX,
    };
    let mut seen = BTreeSet::new();
    for value in std::iter::once(profile.background).chain(profile.body_ramp.iter().copied()) {
        if value > limit || !palette_contains(block.palette_indices, value) {
            return Err(BakedTextPatchError::PaletteIndexNotDeclared {
                id: block.id.to_string(),
                value,
            });
        }
        if value == profile.background && seen.contains(&value) {
            return Err(BakedTextPatchError::BackgroundInBodyRamp {
                id: block.id.to_string(),
                value,
            });
        }
        if !seen.insert(value) {
            return Err(BakedTextPatchError::DuplicatePaletteIndex {
                id: block.id.to_string(),
                value,
            });
        }
    }
    let effect_index = match profile.effect {
        BakedTextEffect::None => None,
        BakedTextEffect::Shadow { palette_index, .. }
        | BakedTextEffect::Outline { palette_index } => Some(palette_index),
    };
    if let Some(value) = effect_index {
        if value > limit || !palette_contains(block.palette_indices, value) {
            return Err(BakedTextPatchError::PaletteIndexNotDeclared {
                id: block.id.to_string(),
                value,
            });
        }
        if value == profile.background {
            return Err(BakedTextPatchError::BackgroundInBodyRamp {
                id: block.id.to_string(),
                value,
            });
        }
    }
    Ok(())
}

fn render_single_line(
    font: &Font,
    block: TextBlockSpec<'_>,
    text: &str,
    profile: BakedTextProfile<'_>,
) -> Result<Vec<u8>, BakedTextPatchError> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut glyphs = Vec::with_capacity(chars.len());
    let mut pen_x = 0i32;
    let mut min_x = 0i32;
    let mut max_x = 0i32;
    let tracking = i32::try_from(profile.tracking)
        .map_err(|_| BakedTextPatchError::InvalidRasterProfile(block.id.to_string()))?;

    for (index, glyph) in chars.iter().copied().enumerate() {
        let (metrics, bitmap) = font.rasterize(glyph, profile.font_size);
        if bitmap.iter().all(|value| *value == 0) && !glyph.is_whitespace() {
            return Err(BakedTextPatchError::EmptyGlyph {
                id: block.id.to_string(),
                glyph,
            });
        }
        let ink_left = pen_x
            .checked_add(metrics.xmin)
            .ok_or_else(|| BakedTextPatchError::InvalidRasterProfile(block.id.to_string()))?;
        let ink_right = ink_left
            .checked_add(
                i32::try_from(metrics.width)
                    .map_err(|_| BakedTextPatchError::InvalidRasterProfile(block.id.to_string()))?,
            )
            .ok_or_else(|| BakedTextPatchError::InvalidRasterProfile(block.id.to_string()))?;
        min_x = min_x.min(ink_left);
        max_x = max_x.max(ink_right);
        glyphs.push(RasterGlyph {
            metrics,
            bitmap,
            pen_x,
        });

        let advance = metrics.advance_width.round() as i32;
        pen_x = pen_x
            .checked_add(advance)
            .and_then(|value| {
                if index + 1 == chars.len() {
                    Some(value)
                } else {
                    value.checked_add(tracking)
                }
            })
            .ok_or_else(|| BakedTextPatchError::InvalidRasterProfile(block.id.to_string()))?;
        max_x = max_x.max(pen_x);
    }

    let needed_width = usize::try_from(max_x - min_x)
        .map_err(|_| BakedTextPatchError::InvalidRasterProfile(block.id.to_string()))?;
    let start_x = match profile.align {
        HorizontalAlign::Left { padding } => padding,
        HorizontalAlign::Center => block.bbox.width.saturating_sub(needed_width) / 2,
        HorizontalAlign::Right { padding } => block
            .bbox
            .width
            .checked_sub(padding)
            .and_then(|right| right.checked_sub(needed_width))
            .ok_or_else(|| BakedTextPatchError::TextDoesNotFit {
                id: block.id.to_string(),
                width: block.bbox.width,
                height: block.bbox.height,
                needed_width,
                top: 0,
                bottom: block.bbox.height as i32,
            })?,
    };
    if start_x
        .checked_add(needed_width)
        .is_none_or(|right| right > block.bbox.width)
    {
        return Err(BakedTextPatchError::TextDoesNotFit {
            id: block.id.to_string(),
            width: block.bbox.width,
            height: block.bbox.height,
            needed_width,
            top: 0,
            bottom: block.bbox.height as i32,
        });
    }

    let mut coverage_layer = vec![0u8; block.bbox.width * block.bbox.height];
    for glyph in glyphs {
        let x = i32::try_from(start_x)
            .ok()
            .and_then(|start| start.checked_add(glyph.pen_x))
            .and_then(|value| value.checked_add(glyph.metrics.xmin))
            .and_then(|value| value.checked_sub(min_x))
            .ok_or_else(|| BakedTextPatchError::InvalidRasterProfile(block.id.to_string()))?;
        let y = profile
            .baseline
            .checked_sub(glyph.metrics.ymin)
            .and_then(|value| value.checked_sub(glyph.metrics.height as i32))
            .ok_or_else(|| BakedTextPatchError::InvalidRasterProfile(block.id.to_string()))?;
        let right = x + glyph.metrics.width as i32;
        let bottom = y + glyph.metrics.height as i32;
        if x < 0 || y < 0 || right > block.bbox.width as i32 || bottom > block.bbox.height as i32 {
            return Err(BakedTextPatchError::TextDoesNotFit {
                id: block.id.to_string(),
                width: block.bbox.width,
                height: block.bbox.height,
                needed_width,
                top: y,
                bottom,
            });
        }

        for row in 0..glyph.metrics.height {
            for column in 0..glyph.metrics.width {
                let coverage = glyph.bitmap[row * glyph.metrics.width + column];
                if coverage == 0 {
                    continue;
                }
                let target = (y as usize + row) * block.bbox.width + x as usize + column;
                coverage_layer[target] = coverage_layer[target].max(coverage);
            }
        }
    }

    let mut output = vec![profile.background; block.bbox.width * block.bbox.height];
    match profile.effect {
        BakedTextEffect::None => {}
        BakedTextEffect::Shadow {
            offset_x,
            offset_y,
            palette_index,
        } => {
            for (source, coverage) in coverage_layer.iter().copied().enumerate() {
                if coverage == 0 {
                    continue;
                }
                let source_x = (source % block.bbox.width) as i32;
                let source_y = (source / block.bbox.width) as i32;
                let target_x = source_x + offset_x;
                let target_y = source_y + offset_y;
                if target_x < 0
                    || target_y < 0
                    || target_x >= block.bbox.width as i32
                    || target_y >= block.bbox.height as i32
                {
                    return Err(BakedTextPatchError::TextDoesNotFit {
                        id: block.id.to_string(),
                        width: block.bbox.width,
                        height: block.bbox.height,
                        needed_width,
                        top: target_y,
                        bottom: target_y + 1,
                    });
                }
                output[target_y as usize * block.bbox.width + target_x as usize] = palette_index;
            }
        }
        BakedTextEffect::Outline { palette_index } => {
            for (source, coverage) in coverage_layer.iter().copied().enumerate() {
                if coverage == 0 {
                    continue;
                }
                let source_x = (source % block.bbox.width) as i32;
                let source_y = (source / block.bbox.width) as i32;
                for offset_y in -1..=1 {
                    for offset_x in -1..=1 {
                        if offset_x == 0 && offset_y == 0 {
                            continue;
                        }
                        let target_x = source_x + offset_x;
                        let target_y = source_y + offset_y;
                        if target_x < 0
                            || target_y < 0
                            || target_x >= block.bbox.width as i32
                            || target_y >= block.bbox.height as i32
                        {
                            return Err(BakedTextPatchError::TextDoesNotFit {
                                id: block.id.to_string(),
                                width: block.bbox.width,
                                height: block.bbox.height,
                                needed_width,
                                top: target_y,
                                bottom: target_y + 1,
                            });
                        }
                        output[target_y as usize * block.bbox.width + target_x as usize] =
                            palette_index;
                    }
                }
            }
        }
    }
    for (target, coverage) in coverage_layer.into_iter().enumerate() {
        if coverage == 0 {
            continue;
        }
        let ramp_index = (usize::from(coverage) * (profile.body_ramp.len() - 1) + 127) / 255;
        output[target] = profile.body_ramp[ramp_index];
    }
    Ok(output)
}

fn render_text(
    font: &Font,
    block: TextBlockSpec<'_>,
    text: &str,
    profile: BakedTextProfile<'_>,
) -> Result<Vec<u8>, BakedTextPatchError> {
    if profile.line_count == 1 {
        return render_single_line(font, block, text, profile);
    }

    let used_height = profile
        .line_count
        .checked_mul(profile.line_height)
        .ok_or_else(|| BakedTextPatchError::InvalidRasterProfile(block.id.to_string()))?;
    if used_height > block.bbox.height {
        return Err(BakedTextPatchError::InvalidRasterProfile(
            block.id.to_string(),
        ));
    }

    let mut output = vec![profile.background; block.bbox.width * block.bbox.height];
    let line_profile = BakedTextProfile {
        line_count: 1,
        ..profile
    };
    let line_block = TextBlockSpec {
        bbox: super::text_catalog::Rect {
            x: 0,
            y: 0,
            width: block.bbox.width,
            height: profile.line_height,
        },
        ..block
    };
    for (line_index, line) in text.split('\n').enumerate() {
        let rendered = render_single_line(font, line_block, line, line_profile)?;
        let target_start = line_index * profile.line_height * block.bbox.width;
        output[target_start..target_start + rendered.len()].copy_from_slice(&rendered);
    }
    Ok(output)
}

/// 검증된 catalog block 크기의 배경 포함 indexed text layer를 만든다.
///
/// 일반 bbox 교체와 자산 전용 surface composer가 같은 font rasterization을
/// 공유하도록 제공한다. 호출자는 `profile.background` 픽셀을 투명한 곳으로
/// 해석할 수 있지만, profile과 palette 계약 자체는 일반 교체와 똑같이
/// 검증된다.
pub(crate) fn render_baked_text_block(
    block: TextBlockSpec<'_>,
    format: PixelFormat,
    fonts: &BakedFontBook,
    text: &str,
    profile: BakedTextProfile<'_>,
) -> Result<Vec<u8>, BakedTextPatchError> {
    if text.is_empty() {
        return Err(BakedTextPatchError::EmptyTranslation(block.id.to_string()));
    }
    if text.contains('\r') {
        return Err(BakedTextPatchError::CarriageReturn(block.id.to_string()));
    }
    let actual_lines = text.split('\n').count();
    if actual_lines != profile.line_count {
        return Err(BakedTextPatchError::LineCountMismatch {
            id: block.id.to_string(),
            expected: profile.line_count,
            actual: actual_lines,
        });
    }
    if !contains_hangul_syllable(text) {
        return Err(BakedTextPatchError::MissingHangul(block.id.to_string()));
    }
    validate_profile(block, format, profile)?;
    let font = fonts
        .get(profile.font_id)
        .ok_or_else(|| BakedTextPatchError::UnknownFont {
            id: block.id.to_string(),
            font_id: profile.font_id.to_string(),
        })?;
    render_text(font, block, text, profile)
}

/// 원본 BTX를 검증하고 일본어 catalog block의 한글 replacement를 합성한다.
pub fn patch_baked_text(
    original: &[u8],
    spec: TextAssetSpec<'_>,
    fonts: &BakedFontBook,
    replacements: &[BakedTextReplacement<'_>],
) -> Result<(Vec<u8>, BakedTextPatchReport), BakedTextPatchError> {
    verify_text_asset(original, spec)?;
    if matches!(spec.storage, TextStorageSpec::FixedCellGrid(_)) {
        return Err(BakedTextPatchError::FixedCellGridRequiresAssembler(
            spec.name.to_string(),
        ));
    }
    if matches!(spec.storage, TextStorageSpec::SparseSelectorAtlas(_)) {
        return Err(
            BakedTextPatchError::SparseSelectorAtlasRequiresSurfaceComposer(spec.name.to_string()),
        );
    }
    if replacements.is_empty() {
        return Err(BakedTextPatchError::EmptyReplacements);
    }

    let mut ids = BTreeSet::new();
    let mut rendered = Vec::with_capacity(replacements.len());
    for replacement in replacements {
        if replacement.block_id.is_empty() {
            return Err(BakedTextPatchError::EmptyBlockId);
        }
        if !ids.insert(replacement.block_id) {
            return Err(BakedTextPatchError::DuplicateBlockId(
                replacement.block_id.to_string(),
            ));
        }
        let block = spec
            .blocks
            .iter()
            .copied()
            .find(|block| block.id == replacement.block_id)
            .ok_or_else(|| BakedTextPatchError::UnknownBlockId(replacement.block_id.to_string()))?;
        if !is_japanese_translation_target(block.source) {
            return Err(BakedTextPatchError::ReferenceOnlyBlock(
                block.id.to_string(),
            ));
        }
        rendered.push((
            block,
            render_baked_text_block(
                block,
                spec.layout.format,
                fonts,
                replacement.text,
                *replacement.profile,
            )?,
        ));
    }

    let original_pixels = decode(original, spec.layout)?;
    let mut patched_pixels = original_pixels.clone();
    for (block, replacement_pixels) in rendered {
        for row in 0..block.bbox.height {
            let source = row * block.bbox.width;
            let target = (block.bbox.y + row) * spec.layout.width + block.bbox.x;
            patched_pixels[target..target + block.bbox.width]
                .copy_from_slice(&replacement_pixels[source..source + block.bbox.width]);
        }
    }

    let changed_pixels = original_pixels
        .iter()
        .zip(&patched_pixels)
        .filter(|(before, after)| before != after)
        .count();
    if changed_pixels == 0 {
        return Err(BakedTextPatchError::NoChangedPixels);
    }
    let patched = encode(&patched_pixels, spec.layout)?;
    let changed_bytes = original
        .iter()
        .zip(&patched)
        .filter(|(before, after)| before != after)
        .count();
    let output_sha256 = format!("{:x}", Sha256::digest(&patched));

    Ok((
        patched,
        BakedTextPatchReport {
            replacement_blocks: replacements.len(),
            changed_pixels,
            changed_bytes,
            output_sha256,
        },
    ))
}

#[cfg(test)]
#[path = "text_patch_tests.rs"]
mod tests;
