//! `RANK_S.BTX`의 일본어 ranking label만 source mask로 교체한다.
//!
//! 상단 일본어 bbox는 글자 픽셀만 제거하고, 영문 heading과 하단 glyph bank를
//! 포함한 나머지 surface는 전역 보호한다. 문구와 profile은 manifest가,
//! 원본 글자·보호 mask identity는 이 모듈이 소유한다.

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
use super::text_catalog::{is_japanese_translation_target, RANK_S_TEXT_ASSET};
use super::text_patch::render_baked_text_block;
use super::text_profile::BakedFontBook;
use crate::btx::decode;

const BACKGROUND_INDEX: u8 = 0;
const SOURCE_TEXT_PIXELS: usize = 14098;
const PROTECTED_PIXELS: usize = 40192;
const PROTECTED_MASK_SHA256: &str =
    "ee747b9c198f08d2d0fff4f4ed816e97ab0277c70c54791ecc253f6fb0bb6192";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceTextMaskSpec {
    block_id: &'static str,
    expected_pixels: usize,
    expected_sha256: &'static str,
}

const SOURCE_TEXT_MASKS: [SourceTextMaskSpec; 10] = [
    SourceTextMaskSpec {
        block_id: "BTXB-0085",
        expected_pixels: 1194,
        expected_sha256: "f78abef790d4842d56ec7c576de7e3234bd903170b1e3a4b5ca03b69b2b025ec",
    },
    SourceTextMaskSpec {
        block_id: "BTXB-0086",
        expected_pixels: 1183,
        expected_sha256: "8d8d897e6f9889634737db38c23b93220d1e95da2371277d9ed9793babac95ce",
    },
    SourceTextMaskSpec {
        block_id: "BTXB-0087",
        expected_pixels: 1188,
        expected_sha256: "6dc4681808d298141f3efefd07c965a507fbf18d0c360b19f86eec9579df1670",
    },
    SourceTextMaskSpec {
        block_id: "BTXB-0088",
        expected_pixels: 948,
        expected_sha256: "875ea0376314be97d52584b0c53bdf67319fb0778cf9ec24e4b528562a43d8fa",
    },
    SourceTextMaskSpec {
        block_id: "BTXB-0089",
        expected_pixels: 1343,
        expected_sha256: "15fe97a89fe2874cff0fffaf8c4b4eb2afb91b5a11fe06ac4886755e7f02cc3c",
    },
    SourceTextMaskSpec {
        block_id: "BTXB-0090",
        expected_pixels: 788,
        expected_sha256: "354e4df85dd67f81e8dd9a5c872ef3b885d43d5082f7abe5788671cd32c8abd5",
    },
    SourceTextMaskSpec {
        block_id: "BTXB-0091",
        expected_pixels: 1343,
        expected_sha256: "15fe97a89fe2874cff0fffaf8c4b4eb2afb91b5a11fe06ac4886755e7f02cc3c",
    },
    SourceTextMaskSpec {
        block_id: "BTXB-0092",
        expected_pixels: 686,
        expected_sha256: "d7a48cc8c0d8ad343b4b58ade90ed640a54d8e69983ab15a575f406c5703bb10",
    },
    SourceTextMaskSpec {
        block_id: "BTXB-0093",
        expected_pixels: 2761,
        expected_sha256: "568ae53ef076f4773d2b6547c8a6a00cd653ed549b670fb8784890f6248a159a",
    },
    SourceTextMaskSpec {
        block_id: "BTXB-0094",
        expected_pixels: 2664,
        expected_sha256: "8edc26935880553fdde60ae77b56fcff0b17ba0b060c104a41af26916fc76ba8",
    },
];

/// ranking label source mask·manifest·원본 검증 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankSDevelopmentSurfaceError(String);

impl fmt::Display for RankSDevelopmentSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for RankSDevelopmentSurfaceError {}

fn error(message: impl Into<String>) -> RankSDevelopmentSurfaceError {
    RankSDevelopmentSurfaceError(message.into())
}

/// 지원 원본의 일본어 label 10개만 수동 문구·profile로 합성한다.
pub fn patch_rank_s_development_surface(
    original: &[u8],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), RankSDevelopmentSurfaceError> {
    let asset = RANK_S_TEXT_ASSET;
    let policy_blocks = verify_source_text_mask_policy_for_asset(asset)
        .map_err(|source| error(source.to_string()))?;
    if policy_blocks != SOURCE_TEXT_MASKS.len() {
        return Err(error(format!(
            "RANK_S source-text-mask 정책 분모 불일치: {policy_blocks}"
        )));
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
    let mask_blocks = SOURCE_TEXT_MASKS
        .iter()
        .map(|mask| mask.block_id)
        .collect::<BTreeSet<_>>();
    if target_blocks != translation_blocks || target_blocks != mask_blocks {
        return Err(error("RANK_S catalog·번역·source mask 집합 불일치"));
    }
    if translations.iter().any(|entry| {
        !matches!(
            entry.status,
            GeneralRasterTranslationStatus::DraftHumanReviewRequired
                | GeneralRasterTranslationStatus::Approved
        )
    }) {
        return Err(error("RANK_S에 개발 합성 불가 번역 상태가 있음"));
    }

    let assignments =
        general_btx_raster_profile_assignments().map_err(|source| error(source.to_string()))?;
    let original_pixels =
        decode(original, asset.layout).map_err(|source| error(source.to_string()))?;
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
        let mask_spec = SOURCE_TEXT_MASKS
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
        let configuration = assignment
            .configuration
            .as_ref()
            .ok_or_else(|| error(format!("{} profile 설정 누락", block.id)))?;
        let profile = configuration.baked_text_profile();
        if profile.background != BACKGROUND_INDEX {
            return Err(error(format!(
                "{} profile background 불일치: {}",
                block.id, profile.background
            )));
        }
        let rendered =
            render_baked_text_block(block, asset.layout.format, &fonts, &translation.ko, profile)
                .map_err(|source| error(source.to_string()))?;
        let source_text_mask = (0..block.bbox.height)
            .flat_map(|local_y| {
                let row = (block.bbox.y + local_y) * asset.layout.width + block.bbox.x;
                original_pixels[row..row + block.bbox.width]
                    .iter()
                    .map(|value| *value != BACKGROUND_INDEX)
            })
            .collect::<Vec<_>>();
        prepared.push((
            block,
            mask_spec,
            source_text_mask,
            rendered,
            assignment.status,
        ));
    }

    let mut replacement_area = vec![false; original_pixels.len()];
    for (block, _, _, _, _) in &prepared {
        for y in block.bbox.y..block.bbox.y + block.bbox.height {
            for x in block.bbox.x..block.bbox.x + block.bbox.width {
                replacement_area[y * asset.layout.width + x] = true;
            }
        }
    }
    let protected_pixels = replacement_area
        .iter()
        .map(|selected| !selected)
        .collect::<Vec<_>>();
    let replacements = prepared
        .iter()
        .map(
            |(block, mask_spec, source_text_mask, rendered, _)| SourceMaskedTextReplacement {
                block_id: block.id,
                source_text_pixels: source_text_mask,
                expected_source_text_pixels: mask_spec.expected_pixels,
                expected_source_text_mask_sha256: mask_spec.expected_sha256,
                removal: SourceTextRemoval::Fill {
                    background_index: BACKGROUND_INDEX,
                },
                transparent_index: BACKGROUND_INDEX,
                rendered_pixels: rendered,
            },
        )
        .collect::<Vec<_>>();
    let (patched, patch) = compose_source_masked_text_surface(
        original,
        asset,
        ProtectedPixelMask {
            pixels: &protected_pixels,
            expected_pixels: PROTECTED_PIXELS,
            expected_sha256: PROTECTED_MASK_SHA256,
        },
        &replacements,
    )
    .map_err(|source| error(source.to_string()))?;
    if patch.replacement_blocks != SOURCE_TEXT_MASKS.len()
        || patch.source_text_pixels != SOURCE_TEXT_PIXELS
        || patch.protected_pixels != PROTECTED_PIXELS
        || !patch.protected_pixels_exact
        || patch.rendered_pixels == 0
        || patch.changed_pixels == 0
        || patch.changed_bytes == 0
    {
        return Err(error("RANK_S source mask 합성 판정 drift"));
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
                .all(|(_, _, _, _, status)| *status == GeneralRasterProfileStatus::Approved),
            runtime_verified: false,
            product_eligible: false,
        },
    ))
}

#[cfg(test)]
#[path = "rank_s_development_surface_tests.rs"]
mod tests;
