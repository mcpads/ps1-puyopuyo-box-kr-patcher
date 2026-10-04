//! 평탄한 배경의 일반 raster BTX를 수동 번역·명시 profile로 합성한다.
//! BATAN2의 생성 배지 세 셀은 별도 indexed 입력으로 합성한다.
//!
//! 이 경로는 block bbox 전체를 profile background로 다시 그려도 되는 자산만
//! 받는다. 버튼 질감이나 삽화가 bbox 안에 남아야 하는 자산은 등록하지 않고
//! 별도 배경 보존형 surface composer에 위임한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use super::btx_raster_surface_policy::flat_background_indices_for_asset;
use super::btx_raster_surface_policy::verify_source_text_mask_policy_for_asset;
use super::btx_raster_translation_manifest::{
    general_btx_raster_profile_assignments, general_btx_raster_translation_entries,
    GeneralRasterProfileStatus, GeneralRasterTranslationStatus,
};
use super::source_masked_text_surface::{
    compose_source_masked_text_surface, ProtectedPixelMask, SourceMaskedTextReplacement,
    SourceTextRemoval,
};
use super::text_catalog::{is_japanese_translation_target, TextAssetSpec, P2_END16_TEXT_ASSET};
use super::text_patch::{
    patch_baked_text, render_baked_text_block, BakedTextPatchReport, BakedTextReplacement,
};
use super::text_profile::{BakedFontBook, HorizontalAlign};
use crate::btx::decode;

/// bbox 안이 단색 배경인 것으로 판정한 일반 raster surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlatBackgroundGeneralRasterSurfaceSpec {
    pub asset: TextAssetSpec<'static>,
}

const P2_END16_BACKGROUND_INDEX: u8 = 7;
const P2_END16_SOURCE_TEXT_PIXELS: usize = 17_307;
const P2_END16_SOURCE_TEXT_MASK_SHA256: &str =
    "0f834a4437758828eaeee1468f84492fa50ca72a28d8065035cab601017f7dbc";
const P2_END16_PROTECTED_PIXELS: usize = 24_576;
const P2_END16_PROTECTED_MASK_SHA256: &str =
    "62b5a68d4fadc7c9e0fe7e68c61e7bce44f0a198a370f7a554be9e841c0519eb";

/// 한 논리 BTX의 개발 합성 결과와 승인 경계.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralRasterDevelopmentSurfaceReport {
    pub asset: &'static str,
    pub replacement_blocks: usize,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub output_sha256: String,
    pub translation_approved: bool,
    pub profile_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// 일반 raster 개발 합성 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralRasterDevelopmentSurfaceError(String);

impl fmt::Display for GeneralRasterDevelopmentSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for GeneralRasterDevelopmentSurfaceError {}

fn error(message: impl Into<String>) -> GeneralRasterDevelopmentSurfaceError {
    GeneralRasterDevelopmentSurfaceError(message.into())
}

/// 등록된 평탄 배경 자산에 번역 profile과 지정된 생성 배지를 적용한다.
pub fn patch_flat_background_general_raster_surface(
    original: &[u8],
    spec: FlatBackgroundGeneralRasterSurfaceSpec,
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), GeneralRasterDevelopmentSurfaceError>
{
    if matches!(spec.asset.name, "TITLEA.BTX" | "P2_OP13.BTX") {
        return super::generated_title_surface::patch(original, spec.asset).map_err(error);
    }
    let surface_backgrounds = flat_background_indices_for_asset(spec.asset)
        .map_err(|source| error(source.to_string()))?;
    let translations = general_btx_raster_translation_entries()
        .map_err(|source| error(source.to_string()))?
        .into_iter()
        .filter(|entry| entry.asset_name == spec.asset.name)
        .collect::<Vec<_>>();
    let target_blocks = spec
        .asset
        .blocks
        .iter()
        .filter(|block| is_japanese_translation_target(block.source))
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let translation_blocks = translations
        .iter()
        .map(|entry| entry.block_id.as_str())
        .collect::<BTreeSet<_>>();
    if translations.is_empty() || target_blocks != translation_blocks {
        return Err(error(format!(
            "{}의 일본어 block과 번역 manifest 집합 불일치",
            spec.asset.name
        )));
    }
    if translations.iter().any(|entry| {
        !matches!(
            entry.status,
            GeneralRasterTranslationStatus::DraftHumanReviewRequired
                | GeneralRasterTranslationStatus::Approved
        )
    }) {
        return Err(error(format!(
            "{}에 개발 합성 불가 번역 상태가 있음",
            spec.asset.name
        )));
    }
    let profile_keys = translations
        .iter()
        .map(|entry| {
            (
                entry.semantic_family.as_str(),
                entry.profile_candidate_index,
            )
        })
        .collect::<BTreeSet<_>>();
    let profile_assignments =
        general_btx_raster_profile_assignments().map_err(|source| error(source.to_string()))?;
    let configured_profiles = profile_keys
        .iter()
        .map(|key| {
            let assignment = profile_assignments
                .iter()
                .find(|profile| {
                    profile.semantic_family == key.0 && profile.profile_candidate_index == key.1
                })
                .ok_or_else(|| {
                    error(format!(
                        "{}의 {}:{} profile 누락",
                        spec.asset.name, key.0, key.1
                    ))
                })?;
            if !matches!(
                assignment.status,
                GeneralRasterProfileStatus::DevelopmentDraftHumanReviewRequired
                    | GeneralRasterProfileStatus::Approved
            ) {
                return Err(error(format!(
                    "{}의 {}:{} profile은 아직 개발 합성 설정이 없음",
                    spec.asset.name, key.0, key.1
                )));
            }
            let configuration = assignment.configuration.as_ref().ok_or_else(|| {
                error(format!(
                    "{}의 {}:{} profile 설정 누락",
                    spec.asset.name, key.0, key.1
                ))
            })?;
            Ok((*key, configuration.baked_text_profile(), assignment.status))
        })
        .collect::<Result<Vec<_>, GeneralRasterDevelopmentSurfaceError>>()?;
    let block_profiles = translations
        .iter()
        .map(|entry| {
            let key = (
                entry.semantic_family.as_str(),
                entry.profile_candidate_index,
            );
            let profile = configured_profiles
                .iter()
                .find(|(configured_key, _, _)| *configured_key == key)
                .map(|(_, profile, _)| profile)
                .ok_or_else(|| {
                    error(format!(
                        "{}의 {}:{} configured profile 누락",
                        entry.block_id, key.0, key.1
                    ))
                })?;
            let expected_background =
                surface_backgrounds.get(&entry.block_id).ok_or_else(|| {
                    error(format!("{}의 flat-background 정책이 없음", entry.block_id))
                })?;
            if profile.background != *expected_background {
                return Err(error(format!(
                    "{}의 profile background {}와 surface 정책 {}가 다름",
                    entry.block_id, profile.background, expected_background
                )));
            }
            let mut profile = *profile;
            // MENU01 안내는 서로 붙은 두 sprite다. 양쪽을 각각 중앙 정렬하면
            // 단어와 조사가 떨어지므로 cell 접점으로 맞춘다.
            if spec.asset == super::text_catalog::MENU01_TEXT_ASSET {
                profile.align = match entry.block_id.as_str() {
                    "BTXB-0177" | "BTXB-0178" => HorizontalAlign::Right { padding: 0 },
                    "BTXB-0179" => HorizontalAlign::Left { padding: 0 },
                    _ => profile.align,
                };
            }
            if spec.asset == super::text_catalog::MENU201_TEXT_ASSET {
                profile.align = if entry.block_id == "BTXB-0214" {
                    HorizontalAlign::Left { padding: 0 }
                } else {
                    HorizontalAlign::Right { padding: 0 }
                };
            }
            if spec.asset == super::text_catalog::MENU203_TEXT_ASSET {
                profile.align = match entry.block_id.as_str() {
                    "BTXB-0192" => HorizontalAlign::Right { padding: 1 },
                    "BTXB-0193" => HorizontalAlign::Left { padding: 1 },
                    _ => profile.align,
                };
            }
            Ok(profile)
        })
        .collect::<Result<Vec<_>, GeneralRasterDevelopmentSurfaceError>>()?;
    let replacements = translations
        .iter()
        .zip(&block_profiles)
        .map(|(entry, profile)| BakedTextReplacement {
            block_id: &entry.block_id,
            text: &entry.ko,
            profile,
        })
        .collect::<Vec<_>>();
    let fonts = BakedFontBook::with_selected_fonts().map_err(|source| error(source.to_string()))?;
    let (patched, patch) = patch_baked_text(original, spec.asset, &fonts, &replacements)
        .map_err(|source| error(source.to_string()))?;
    // 생성 배지는 같은 메모리 surface에서 합성하고 최종 쓰기는 한 번만 한다.
    let (patched, patch) = if spec.asset == super::text_catalog::BATAN2_TEXT_ASSET
        || spec.asset == super::text_catalog::MENU201_TEXT_ASSET
    {
        let output = if spec.asset == super::text_catalog::MENU201_TEXT_ASSET {
            super::mode_help_surface::compose(&patched).map_err(error)?
        } else {
            let output = super::battle_badge_surface::compose(&patched).map_err(error)?;
            super::battle_badge_surface::restore_offset_background(original, &output)
                .map_err(error)?
        };
        let source_pixels =
            decode(original, spec.asset.layout).map_err(|e| error(e.to_string()))?;
        let output_pixels = decode(&output, spec.asset.layout).map_err(|e| error(e.to_string()))?;
        use sha2::{Digest, Sha256};
        let report = BakedTextPatchReport {
            replacement_blocks: patch.replacement_blocks,
            changed_pixels: source_pixels
                .iter()
                .zip(&output_pixels)
                .filter(|(a, b)| a != b)
                .count(),
            changed_bytes: original.iter().zip(&output).filter(|(a, b)| a != b).count(),
            output_sha256: format!("{:x}", Sha256::digest(&output)),
        };
        (output, report)
    } else {
        (patched, patch)
    };
    let BakedTextPatchReport {
        replacement_blocks,
        changed_pixels,
        changed_bytes,
        output_sha256,
    } = patch;
    let report = GeneralRasterDevelopmentSurfaceReport {
        asset: spec.asset.name,
        replacement_blocks,
        changed_pixels,
        changed_bytes,
        output_sha256,
        translation_approved: translations
            .iter()
            .all(|entry| entry.status == GeneralRasterTranslationStatus::Approved),
        profile_approved: spec.asset != super::text_catalog::BATAN2_TEXT_ASSET
            && configured_profiles
                .iter()
                .all(|(_, _, status)| *status == GeneralRasterProfileStatus::Approved),
        runtime_verified: false,
        product_eligible: false,
    };
    if report.replacement_blocks != translations.len()
        || report.changed_pixels == 0
        || report.changed_bytes == 0
        || report.runtime_verified
        || report.product_eligible
    {
        return Err(error(format!(
            "{} 일반 raster 개발 합성 판정 drift",
            spec.asset.name
        )));
    }
    Ok((patched, report))
}

/// `P2_END16` 왼쪽 문구 cell만 지우고 오른쪽 탑 graphic을 보호해 합성한다.
pub fn patch_p2_end16_development_surface(
    original: &[u8],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), GeneralRasterDevelopmentSurfaceError>
{
    let asset = P2_END16_TEXT_ASSET;
    let policy_blocks = verify_source_text_mask_policy_for_asset(asset)
        .map_err(|source| error(source.to_string()))?;
    if policy_blocks != 1 {
        return Err(error(format!(
            "P2_END16 source-text-mask 정책 분모 불일치: {policy_blocks}"
        )));
    }
    let translations = general_btx_raster_translation_entries()
        .map_err(|source| error(source.to_string()))?
        .into_iter()
        .filter(|entry| entry.asset_name == asset.name)
        .collect::<Vec<_>>();
    if translations.len() != 1 {
        return Err(error(format!(
            "P2_END16 번역 분모 불일치: {}",
            translations.len()
        )));
    }
    let translation = &translations[0];
    if !matches!(
        translation.status,
        GeneralRasterTranslationStatus::DraftHumanReviewRequired
            | GeneralRasterTranslationStatus::Approved
    ) {
        return Err(error("P2_END16 번역은 개발 합성 가능한 상태가 아님"));
    }
    let block = asset
        .blocks
        .iter()
        .copied()
        .find(|block| block.id == translation.block_id)
        .ok_or_else(|| error("P2_END16 번역 block이 catalog에 없음"))?;
    let assignments =
        general_btx_raster_profile_assignments().map_err(|source| error(source.to_string()))?;
    let assignment = assignments
        .iter()
        .find(|profile| {
            profile.semantic_family == translation.semantic_family
                && profile.profile_candidate_index == translation.profile_candidate_index
        })
        .ok_or_else(|| error("P2_END16 profile assignment 누락"))?;
    if !matches!(
        assignment.status,
        GeneralRasterProfileStatus::DevelopmentDraftHumanReviewRequired
            | GeneralRasterProfileStatus::Approved
    ) {
        return Err(error("P2_END16 profile은 개발 합성 가능한 상태가 아님"));
    }
    let configuration = assignment
        .configuration
        .as_ref()
        .ok_or_else(|| error("P2_END16 profile 설정 누락"))?;
    let profile = configuration.baked_text_profile();
    if profile.background != P2_END16_BACKGROUND_INDEX {
        return Err(error(format!(
            "P2_END16 profile background 불일치: {}",
            profile.background
        )));
    }

    let fonts = BakedFontBook::with_selected_fonts().map_err(|source| error(source.to_string()))?;
    let rendered =
        render_baked_text_block(block, asset.layout.format, &fonts, &translation.ko, profile)
            .map_err(|source| error(source.to_string()))?;
    let original_pixels =
        decode(original, asset.layout).map_err(|source| error(source.to_string()))?;
    let source_text_mask = (0..block.bbox.height)
        .flat_map(|local_y| {
            let row = (block.bbox.y + local_y) * asset.layout.width + block.bbox.x;
            original_pixels[row..row + block.bbox.width]
                .iter()
                .map(|value| *value != P2_END16_BACKGROUND_INDEX)
        })
        .collect::<Vec<_>>();
    let protected_pixels = (0..asset.layout.height)
        .flat_map(|y| {
            (0..asset.layout.width).map(move |x| {
                x < block.bbox.x
                    || x >= block.bbox.x + block.bbox.width
                    || y < block.bbox.y
                    || y >= block.bbox.y + block.bbox.height
            })
        })
        .collect::<Vec<_>>();
    let replacement = SourceMaskedTextReplacement {
        block_id: block.id,
        source_text_pixels: &source_text_mask,
        expected_source_text_pixels: P2_END16_SOURCE_TEXT_PIXELS,
        expected_source_text_mask_sha256: P2_END16_SOURCE_TEXT_MASK_SHA256,
        removal: SourceTextRemoval::Fill {
            background_index: P2_END16_BACKGROUND_INDEX,
        },
        transparent_index: P2_END16_BACKGROUND_INDEX,
        rendered_pixels: &rendered,
    };
    let (patched, patch) = compose_source_masked_text_surface(
        original,
        asset,
        ProtectedPixelMask {
            pixels: &protected_pixels,
            expected_pixels: P2_END16_PROTECTED_PIXELS,
            expected_sha256: P2_END16_PROTECTED_MASK_SHA256,
        },
        &[replacement],
    )
    .map_err(|source| error(source.to_string()))?;
    if patch.replacement_blocks != translations.len()
        || patch.source_text_pixels != P2_END16_SOURCE_TEXT_PIXELS
        || patch.protected_pixels != P2_END16_PROTECTED_PIXELS
        || !patch.protected_pixels_exact
        || patch.rendered_pixels == 0
        || patch.changed_pixels == 0
        || patch.changed_bytes == 0
    {
        return Err(error("P2_END16 source mask 합성 판정 drift"));
    }
    Ok((
        patched,
        GeneralRasterDevelopmentSurfaceReport {
            asset: asset.name,
            replacement_blocks: patch.replacement_blocks,
            changed_pixels: patch.changed_pixels,
            changed_bytes: patch.changed_bytes,
            output_sha256: patch.output_sha256,
            translation_approved: translation.status == GeneralRasterTranslationStatus::Approved,
            profile_approved: assignment.status == GeneralRasterProfileStatus::Approved,
            runtime_verified: false,
            product_eligible: false,
        },
    ))
}

#[cfg(test)]
#[path = "general_raster_development_surface_tests.rs"]
mod tests;
