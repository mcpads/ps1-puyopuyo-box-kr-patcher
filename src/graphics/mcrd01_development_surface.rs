//! `MCRD01.BTX`의 load/save/insert label을 보호형 surface로 합성한다.
//!
//! 세 button의 index 3 내부만 조판 영역으로 열고, 테두리와 insert 오른쪽의
//! slot 숫자는 전역 보호한다. 원문 mask와 profile은 서로 다른 입력이며 둘
//! 중 하나라도 지원 원본 기준선에서 벗어나면 BTX를 만들지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use super::btx_raster_surface_policy::verify_source_text_mask_policy_for_asset;
use super::btx_raster_translation_manifest::{
    general_btx_raster_profile_assignments, general_btx_raster_translation_entries,
    GeneralRasterProfileStatus, GeneralRasterTranslationStatus,
};
use super::general_raster_development_surface::GeneralRasterDevelopmentSurfaceReport;
use super::source_masked_text_surface::{
    compose_source_masked_text_surface, ProtectedPixelMask, SourceMaskedTextReplacement,
    SourceTextRemoval,
};
use super::text_catalog::{is_japanese_translation_target, Rect, MCRD01_TEXT_ASSET};
use super::text_patch::render_baked_text_block;
use super::text_profile::BakedFontBook;
use crate::btx::decode;

const BACKGROUND_INDEX: u8 = 3;
const PROTECTED_PIXELS: usize = 31_508;
const PROTECTED_MASK_SHA256: &str =
    "aae3e1e690276628993489526a2c06d20aa5a4861c2ad9f59ce6108845704020";

#[derive(Debug, Clone, Copy)]
struct SourceMaskSpec {
    block_id: &'static str,
    editable: Rect,
    expected_pixels: usize,
    expected_sha256: &'static str,
}

const SOURCE_MASKS: [SourceMaskSpec; 3] = [
    SourceMaskSpec {
        block_id: "BTXB-0080",
        editable: Rect {
            x: 4,
            y: 2,
            width: 40,
            height: 12,
        },
        expected_pixels: 352,
        expected_sha256: "dd6ac3a26d062e649901f6959b61f5ae1a512fe55bbad363da9966184d1397da",
    },
    SourceMaskSpec {
        block_id: "BTXB-0081",
        editable: Rect {
            x: 4,
            y: 2,
            width: 40,
            height: 12,
        },
        expected_pixels: 368,
        expected_sha256: "2264bdacc766d1b3ff026c0d96a8f4904a6ee93d6bb80910c2c9b84156897092",
    },
    SourceMaskSpec {
        block_id: "BTXB-0082",
        editable: Rect {
            x: 4,
            y: 2,
            width: 25,
            height: 12,
        },
        expected_pixels: 223,
        expected_sha256: "0b5586cbf0db7fc98b616aa8e8ce1c37b9606939fe46b2f7ea383cbc5087d1b4",
    },
];

struct PreparedBlock {
    block_id: &'static str,
    source_mask: Vec<bool>,
    rendered: Vec<u8>,
    profile_status: GeneralRasterProfileStatus,
}

/// MCRD01 보호형 합성 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mcrd01DevelopmentSurfaceError(String);

impl fmt::Display for Mcrd01DevelopmentSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for Mcrd01DevelopmentSurfaceError {}

fn error(message: impl Into<String>) -> Mcrd01DevelopmentSurfaceError {
    Mcrd01DevelopmentSurfaceError(message.into())
}

fn editable_contains(editable: Rect, x: usize, y: usize) -> bool {
    x >= editable.x
        && x < editable.x + editable.width
        && y >= editable.y
        && y < editable.y + editable.height
}

/// 지원 원본의 button 내부만 수동 문구와 profile로 합성한다.
pub fn patch_mcrd01_development_surface(
    original: &[u8],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), Mcrd01DevelopmentSurfaceError> {
    let asset = MCRD01_TEXT_ASSET;
    if verify_source_text_mask_policy_for_asset(asset)
        .map_err(|source| error(source.to_string()))?
        != SOURCE_MASKS.len()
    {
        return Err(error("MCRD01 source mask 정책 분모 불일치"));
    }
    let translations = general_btx_raster_translation_entries()
        .map_err(|source| error(source.to_string()))?
        .into_iter()
        .filter(|entry| entry.asset_name == asset.name)
        .collect::<Vec<_>>();
    let target_blocks = asset
        .blocks
        .iter()
        .filter(|block| is_japanese_translation_target(block.source))
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let translation_blocks = translations
        .iter()
        .map(|entry| entry.block_id.as_str())
        .collect::<BTreeSet<_>>();
    let mask_blocks = SOURCE_MASKS
        .iter()
        .map(|mask| mask.block_id)
        .collect::<BTreeSet<_>>();
    if target_blocks != translation_blocks || target_blocks != mask_blocks {
        return Err(error("MCRD01 catalog·번역·mask 집합 불일치"));
    }
    if translations.iter().any(|entry| {
        !matches!(
            entry.status,
            GeneralRasterTranslationStatus::DraftHumanReviewRequired
                | GeneralRasterTranslationStatus::Approved
        )
    }) {
        return Err(error("MCRD01에 개발 합성 불가 번역 상태가 있음"));
    }

    let assignments =
        general_btx_raster_profile_assignments().map_err(|source| error(source.to_string()))?;
    let pixels = decode(original, asset.layout).map_err(|source| error(source.to_string()))?;
    let fonts = BakedFontBook::with_bundled_maplestory_light()
        .map_err(|source| error(source.to_string()))?;
    let mut prepared = Vec::with_capacity(translations.len());
    for translation in &translations {
        let block = asset
            .blocks
            .iter()
            .copied()
            .find(|block| block.id == translation.block_id)
            .ok_or_else(|| error(format!("{} catalog block 누락", translation.block_id)))?;
        let mask_spec = SOURCE_MASKS
            .iter()
            .copied()
            .find(|mask| mask.block_id == block.id)
            .ok_or_else(|| error(format!("{} source mask identity 누락", block.id)))?;
        let assignment = assignments
            .iter()
            .find(|profile| {
                profile.semantic_family == translation.semantic_family
                    && profile.profile_candidate_index == translation.profile_candidate_index
            })
            .ok_or_else(|| error(format!("{} profile assignment 누락", block.id)))?;
        if !matches!(
            assignment.status,
            GeneralRasterProfileStatus::DevelopmentDraftHumanReviewRequired
                | GeneralRasterProfileStatus::Approved
        ) {
            return Err(error(format!(
                "{} profile은 개발 합성 가능한 상태가 아님",
                block.id
            )));
        }
        let profile = assignment
            .configuration
            .as_ref()
            .ok_or_else(|| error(format!("{} profile 설정 누락", block.id)))?
            .baked_text_profile();
        if profile.background != BACKGROUND_INDEX {
            return Err(error(format!("{} 투명 index 불일치", block.id)));
        }
        let rendered =
            render_baked_text_block(block, asset.layout.format, &fonts, &translation.ko, profile)
                .map_err(|source| error(source.to_string()))?;
        let source_mask = (0..block.bbox.height)
            .flat_map(|local_y| {
                let row = (block.bbox.y + local_y) * asset.layout.width + block.bbox.x;
                pixels[row..row + block.bbox.width].iter().enumerate().map(
                    move |(local_x, value)| {
                        editable_contains(mask_spec.editable, local_x, local_y)
                            && *value != BACKGROUND_INDEX
                    },
                )
            })
            .collect::<Vec<_>>();
        prepared.push((
            mask_spec,
            PreparedBlock {
                block_id: block.id,
                source_mask,
                rendered,
                profile_status: assignment.status,
            },
        ));
    }

    let mut protected = vec![true; pixels.len()];
    for (mask, _) in &prepared {
        let block = asset
            .blocks
            .iter()
            .find(|block| block.id == mask.block_id)
            .expect("검증된 MCRD01 block");
        for local_y in mask.editable.y..mask.editable.y + mask.editable.height {
            for local_x in mask.editable.x..mask.editable.x + mask.editable.width {
                protected[(block.bbox.y + local_y) * asset.layout.width + block.bbox.x + local_x] =
                    false;
            }
        }
    }
    let replacements = prepared
        .iter()
        .map(|(mask, prepared)| SourceMaskedTextReplacement {
            block_id: prepared.block_id,
            source_text_pixels: &prepared.source_mask,
            expected_source_text_pixels: mask.expected_pixels,
            expected_source_text_mask_sha256: mask.expected_sha256,
            removal: SourceTextRemoval::Fill {
                background_index: BACKGROUND_INDEX,
            },
            transparent_index: BACKGROUND_INDEX,
            rendered_pixels: &prepared.rendered,
        })
        .collect::<Vec<_>>();
    let (patched, patch) = compose_source_masked_text_surface(
        original,
        asset,
        ProtectedPixelMask {
            pixels: &protected,
            expected_pixels: PROTECTED_PIXELS,
            expected_sha256: PROTECTED_MASK_SHA256,
        },
        &replacements,
    )
    .map_err(|source| error(source.to_string()))?;
    if patch.replacement_blocks != SOURCE_MASKS.len()
        || patch.source_text_pixels
            != SOURCE_MASKS
                .iter()
                .map(|mask| mask.expected_pixels)
                .sum::<usize>()
        || patch.protected_pixels != PROTECTED_PIXELS
        || !patch.protected_pixels_exact
        || patch.rendered_pixels == 0
    {
        return Err(error("MCRD01 보호형 합성 판정 drift"));
    }

    Ok((
        patched,
        GeneralRasterDevelopmentSurfaceReport {
            asset: asset.name,
            replacement_blocks: patch.replacement_blocks,
            changed_pixels: patch.changed_pixels,
            changed_bytes: patch.changed_bytes,
            output_sha256: patch.output_sha256,
            translation_approved: translations
                .iter()
                .all(|entry| entry.status == GeneralRasterTranslationStatus::Approved),
            profile_approved: prepared
                .iter()
                .all(|(_, block)| block.profile_status == GeneralRasterProfileStatus::Approved),
            runtime_verified: false,
            product_eligible: false,
        },
    ))
}

#[cfg(test)]
#[path = "mcrd01_development_surface_tests.rs"]
mod tests;
