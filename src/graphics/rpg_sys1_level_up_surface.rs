//! QUEST `RPG_SYS1.BTX`의 level-up baked 문구 개발 합성.
//!
//! 원문·bbox·번역 초안은 일반 raster manifest와 catalog에서 다시 결속한다.
//! profile은 사람 승인이 끝나지 않은 명시적 개발 후보이며, 이 모듈은 다른
//! icon·숫자·panel 픽셀을 건드리지 않는다.

use std::error::Error;
use std::fmt;

use super::btx_patch_strategy::verify_box_btx_patch_strategy;
use super::btx_raster_surface_policy::flat_background_indices_for_asset;
use super::btx_raster_translation_manifest::{
    general_btx_raster_translation_entries, GeneralRasterTranslationEntry,
    GeneralRasterTranslationStatus,
};
use super::text_catalog::RPG_SYS1_TEXT_ASSET;
use super::text_patch::{patch_baked_text, BakedTextReplacement};
use super::text_profile::{
    BakedFontBook, BakedTextEffect, BakedTextProfile, HorizontalAlign, MAPLESTORY_LIGHT_FONT_ID,
};
use crate::btx::decode;

const LEVEL_UP_BLOCK_ID: &str = "BTXB-0105";
const LEVEL_UP_SOURCE: &str = "レベルアップ！";
const LEVEL_UP_DRAFT: &str = "레벨 업!";
const LEVEL_UP_FAMILY: &str = "puyo-status-branding";
const LEVEL_UP_BODY_RAMP: [u8; 14] = [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

/// 16px보다 원문 높이에 가깝고, 20px 후보처럼 24px bbox를 침범하지 않는
/// 현재 검토용 후보다. 승인 profile로 승격하지 않는다.
const LEVEL_UP_DEVELOPMENT_PROFILE: BakedTextProfile<'static> = BakedTextProfile {
    id: "quest-rpg-sys1-level-up-development",
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 18.0,
    baseline: 19,
    line_count: 1,
    line_height: 24,
    tracking: 0,
    align: HorizontalAlign::Center,
    background: 0,
    body_ramp: &LEVEL_UP_BODY_RAMP,
    effect: BakedTextEffect::Outline { palette_index: 1 },
};

/// level-up 한 block 합성의 정적 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgSys1LevelUpSurfaceReport {
    pub replacement_blocks: usize,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub output_sha256: String,
    pub draft_manifest_exact: bool,
    pub outside_target_block_exact: bool,
    pub translation_approved: bool,
    pub profile_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// 원문·초안·profile·보호 픽셀 경계 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgSys1LevelUpSurfaceError(String);

impl fmt::Display for RpgSys1LevelUpSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for RpgSys1LevelUpSurfaceError {}

fn error(message: impl Into<String>) -> RpgSys1LevelUpSurfaceError {
    RpgSys1LevelUpSurfaceError(message.into())
}

fn level_up_profile_candidate_index() -> Result<usize, RpgSys1LevelUpSurfaceError> {
    verify_box_btx_patch_strategy()
        .map_err(|source| error(source.to_string()))?
        .profile_candidate_members
        .into_iter()
        .find(|member| {
            member.block_id == LEVEL_UP_BLOCK_ID && member.asset_name == RPG_SYS1_TEXT_ASSET.name
        })
        .map(|member| member.candidate_index)
        .ok_or_else(|| error("level-up profile 후보 결속 누락"))
}

fn verify_translation(
    entry: &GeneralRasterTranslationEntry,
) -> Result<(), RpgSys1LevelUpSurfaceError> {
    if entry.semantic_family != LEVEL_UP_FAMILY
        || entry.profile_candidate_index != level_up_profile_candidate_index()?
        || entry.archive_paths != RPG_SYS1_TEXT_ASSET.archive_paths.join("\n")
        || entry.asset_name != RPG_SYS1_TEXT_ASSET.name
        || entry.block_id != LEVEL_UP_BLOCK_ID
        || entry.protected_source != LEVEL_UP_SOURCE
        || entry.ko != LEVEL_UP_DRAFT
        || entry.status != GeneralRasterTranslationStatus::DraftHumanReviewRequired
        || entry.category_id.is_some()
        || entry.profile_id.is_some()
    {
        return Err(error("RPG_SYS1 level-up 번역 초안 계약 drift"));
    }
    Ok(())
}

fn outside_target_block_exact(original: &[u8], patched: &[u8]) -> Result<bool, String> {
    let original =
        decode(original, RPG_SYS1_TEXT_ASSET.layout).map_err(|source| source.to_string())?;
    let patched =
        decode(patched, RPG_SYS1_TEXT_ASSET.layout).map_err(|source| source.to_string())?;
    let target = RPG_SYS1_TEXT_ASSET
        .blocks
        .iter()
        .find(|block| block.id == LEVEL_UP_BLOCK_ID)
        .ok_or_else(|| "RPG_SYS1 catalog에 level-up block이 없음".to_string())?;
    let target_right = target
        .bbox
        .x
        .checked_add(target.bbox.width)
        .ok_or_else(|| "RPG_SYS1 level-up bbox x overflow".to_string())?;
    let target_bottom = target
        .bbox
        .y
        .checked_add(target.bbox.height)
        .ok_or_else(|| "RPG_SYS1 level-up bbox y overflow".to_string())?;
    Ok(original
        .iter()
        .zip(&patched)
        .enumerate()
        .all(|(offset, (before, after))| {
            let x = offset % RPG_SYS1_TEXT_ASSET.layout.width;
            let y = offset / RPG_SYS1_TEXT_ASSET.layout.width;
            (x >= target.bbox.x && x < target_right && y >= target.bbox.y && y < target_bottom)
                || before == after
        }))
}

/// exact `RPG_SYS1.BTX`의 level-up bbox 하나만 현재 개발 후보로 합성한다.
pub fn patch_rpg_sys1_level_up_development_surface(
    original: &[u8],
) -> Result<(Vec<u8>, RpgSys1LevelUpSurfaceReport), RpgSys1LevelUpSurfaceError> {
    let backgrounds = flat_background_indices_for_asset(RPG_SYS1_TEXT_ASSET)
        .map_err(|source| error(source.to_string()))?;
    if backgrounds.get(LEVEL_UP_BLOCK_ID) != Some(&LEVEL_UP_DEVELOPMENT_PROFILE.background) {
        return Err(error(
            "RPG_SYS1 level-up profile background와 surface 정책 불일치",
        ));
    }
    let entries = general_btx_raster_translation_entries()
        .map_err(|source| error(source.to_string()))?
        .into_iter()
        .filter(|entry| {
            entry.asset_name == RPG_SYS1_TEXT_ASSET.name && entry.block_id == LEVEL_UP_BLOCK_ID
        })
        .collect::<Vec<_>>();
    let [entry] = entries.as_slice() else {
        return Err(error(format!(
            "RPG_SYS1 level-up 번역 행 분모 불일치: {}",
            entries.len()
        )));
    };
    verify_translation(entry)?;
    let fonts = BakedFontBook::with_bundled_maplestory_light()
        .map_err(|source| error(source.to_string()))?;
    let (patched, patch) = patch_baked_text(
        original,
        RPG_SYS1_TEXT_ASSET,
        &fonts,
        &[BakedTextReplacement {
            block_id: LEVEL_UP_BLOCK_ID,
            text: &entry.ko,
            profile: &LEVEL_UP_DEVELOPMENT_PROFILE,
        }],
    )
    .map_err(|source| error(source.to_string()))?;
    let outside_target_block_exact =
        outside_target_block_exact(original, &patched).map_err(error)?;
    let report = RpgSys1LevelUpSurfaceReport {
        replacement_blocks: patch.replacement_blocks,
        changed_pixels: patch.changed_pixels,
        changed_bytes: patch.changed_bytes,
        output_sha256: patch.output_sha256,
        draft_manifest_exact: true,
        outside_target_block_exact,
        translation_approved: false,
        profile_approved: false,
        runtime_verified: false,
        product_eligible: false,
    };
    if report.replacement_blocks != 1
        || report.changed_pixels == 0
        || report.changed_bytes == 0
        || report.output_sha256.len() != 64
        || !report.draft_manifest_exact
        || !report.outside_target_block_exact
        || report.translation_approved
        || report.profile_approved
        || report.runtime_verified
        || report.product_eligible
    {
        return Err(error("RPG_SYS1 level-up 개발 surface 판정 drift"));
    }
    Ok((patched, report))
}

#[cfg(test)]
#[path = "rpg_sys1_level_up_surface_tests.rs"]
mod tests;
