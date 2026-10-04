//! `BOXOPT.BTX` 번역 selector와 설명 glyph의 개발 합성.
//!
//! 서로 겹치는 selector는 한 완성 surface에서 수동 조판한 뒤 각 bbox로 다시
//! 잘라 공용 sparse composer에 넘긴다. 따라서 block 적용 순서로 충돌을
//! 숨기지 않는다. 설명 glyph는 별도 bank의 비중첩 slot에 함께 합성하고,
//! 나머지 참고 문구와 미대상 pixel은 원본 그대로 남는다.

#[path = "boxopt_development_surface/layout.rs"]
mod layout;

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::btx::{decode, PixelFormat};

use self::layout::{SharedSelectorLayout, TranslationSlice, SHARED_SELECTOR_LAYOUTS};
use super::btx_raster_surface_policy::verify_sparse_selector_policy_for_asset;
use super::btx_raster_translation_manifest::{
    general_btx_raster_profile_assignments, general_btx_raster_translation_entries,
    GeneralRasterProfileStatus, GeneralRasterTranslationEntry, GeneralRasterTranslationStatus,
};
use super::general_raster_development_surface::GeneralRasterDevelopmentSurfaceReport;
use super::sparse_selector_surface::{
    compose_sparse_selector_surface, SparseSelectorRegionReplacement,
};
use super::text_catalog::{
    is_japanese_translation_target, Rect, TextAssetSpec, TextBlockSpec, TextStorageSpec,
    BOXOPT_TEXT_ASSET,
};
use super::text_patch::render_baked_text_block;
use super::text_profile::{BakedFontBook, BakedTextProfile};

const BACKGROUND_INDEX: u8 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxoptDevelopmentSurfaceError(String);

impl fmt::Display for BoxoptDevelopmentSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "BOXOPT 개발 합성 실패: {}", self.0)
    }
}

impl Error for BoxoptDevelopmentSurfaceError {}

fn error(message: impl Into<String>) -> BoxoptDevelopmentSurfaceError {
    BoxoptDevelopmentSurfaceError(message.into())
}

fn translation_slice(
    translations: &BTreeMap<&str, &GeneralRasterTranslationEntry>,
    slice: TranslationSlice,
) -> Result<String, BoxoptDevelopmentSurfaceError> {
    let translation = translations
        .get(slice.block_id)
        .ok_or_else(|| error(format!("{} 번역 누락", slice.block_id)))?;
    let characters = translation.ko.chars().collect::<Vec<_>>();
    if slice.start >= slice.end || slice.end > characters.len() {
        return Err(error(format!(
            "{} 수동 조판 문자 범위 불일치: {}..{} / {}",
            slice.block_id,
            slice.start,
            slice.end,
            characters.len()
        )));
    }
    Ok(characters[slice.start..slice.end].iter().collect())
}

fn rect_is_inside_surface(rect: Rect, surface: Rect) -> bool {
    rect.x >= surface.x
        && rect.y >= surface.y
        && rect
            .x
            .checked_add(rect.width)
            .is_some_and(|right| right <= surface.x + surface.width)
        && rect
            .y
            .checked_add(rect.height)
            .is_some_and(|bottom| bottom <= surface.y + surface.height)
}

fn rect_is_covered_by_blocks(rect: Rect, blocks: &[TextBlockSpec<'_>]) -> bool {
    (rect.y..rect.y + rect.height).all(|y| {
        (rect.x..rect.x + rect.width).all(|x| {
            blocks.iter().any(|block| {
                x >= block.bbox.x
                    && x < block.bbox.x + block.bbox.width
                    && y >= block.bbox.y
                    && y < block.bbox.y + block.bbox.height
            })
        })
    })
}

fn clear_rect(pixels: &mut [u8], width: usize, rect: Rect) {
    for y in rect.y..rect.y + rect.height {
        pixels[y * width + rect.x..y * width + rect.x + rect.width].fill(BACKGROUND_INDEX);
    }
}

fn copy_rect(source: &[u8], target: &mut [u8], width: usize, rect: Rect) {
    for y in rect.y..rect.y + rect.height {
        let start = y * width + rect.x;
        target[start..start + rect.width].copy_from_slice(&source[start..start + rect.width]);
    }
}

fn paste_rect(target: &mut [u8], width: usize, rect: Rect, layer: &[u8]) {
    for y in 0..rect.height {
        let source_start = y * rect.width;
        let target_start = (rect.y + y) * width + rect.x;
        target[target_start..target_start + rect.width]
            .copy_from_slice(&layer[source_start..source_start + rect.width]);
    }
}

fn crop_rect(source: &[u8], width: usize, rect: Rect) -> Vec<u8> {
    let mut output = Vec::with_capacity(rect.width * rect.height);
    for y in rect.y..rect.y + rect.height {
        output.extend_from_slice(&source[y * width + rect.x..y * width + rect.x + rect.width]);
    }
    output
}

fn validate_layouts(
    asset: TextAssetSpec<'static>,
    translations: &BTreeMap<&str, &GeneralRasterTranslationEntry>,
) -> Result<BTreeSet<&'static str>, BoxoptDevelopmentSurfaceError> {
    let TextStorageSpec::SparseSelectorAtlas(sparse) = asset.storage else {
        return Err(error("BOXOPT catalog가 sparse selector atlas가 아님"));
    };
    let blocks = asset
        .blocks
        .iter()
        .map(|block| (block.id, *block))
        .collect::<BTreeMap<_, _>>();
    let mut expected_targets = BTreeSet::new();
    let mut expected_preserved = BTreeSet::new();
    for (left, right) in sparse.allowed_overlap_pairs {
        for block_id in [*left, *right] {
            let block = blocks
                .get(block_id)
                .ok_or_else(|| error(format!("겹침 catalog block 누락: {block_id}")))?;
            if is_japanese_translation_target(block.source) {
                expected_targets.insert(block_id);
            } else {
                expected_preserved.insert(block_id);
            }
        }
    }

    let surface = Rect {
        x: 0,
        y: 0,
        width: asset.layout.width,
        height: asset.layout.height,
    };
    let mut actual_targets = BTreeSet::new();
    let mut actual_preserved = BTreeSet::new();
    for group in SHARED_SELECTOR_LAYOUTS {
        if group.target_block_ids.is_empty() || group.runs.is_empty() {
            return Err(error("빈 BOXOPT 공용 selector 조판 그룹"));
        }
        let group_blocks = group
            .target_block_ids
            .iter()
            .chain(group.preserved_block_ids)
            .map(|block_id| {
                blocks
                    .get(block_id)
                    .copied()
                    .ok_or_else(|| error(format!("수동 조판 catalog block 누락: {block_id}")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for block_id in group.target_block_ids {
            if !actual_targets.insert(*block_id) || !translations.contains_key(block_id) {
                return Err(error(format!(
                    "수동 조판 target 중복 또는 번역 누락: {block_id}"
                )));
            }
        }
        for block_id in group.preserved_block_ids {
            if !actual_preserved.insert(*block_id)
                || translations.contains_key(block_id)
                || is_japanese_translation_target(blocks[block_id].source)
            {
                return Err(error(format!(
                    "수동 조판 보존 block 경계 불일치: {block_id}"
                )));
            }
        }

        let mut run_rects = Vec::new();
        for run in group.runs {
            if !rect_is_inside_surface(run.rect, surface)
                || !rect_is_covered_by_blocks(run.rect, &group_blocks)
            {
                return Err(error(format!(
                    "{} 수동 조판 run이 selector 공용 영역 밖임",
                    run.text.block_id
                )));
            }
            if run_rects.iter().any(|previous| {
                let previous: &Rect = previous;
                run.rect.x < previous.x + previous.width
                    && previous.x < run.rect.x + run.rect.width
                    && run.rect.y < previous.y + previous.height
                    && previous.y < run.rect.y + run.rect.height
            }) {
                return Err(error("BOXOPT 수동 조판 run이 서로 겹침"));
            }
            run_rects.push(run.rect);
            translation_slice(translations, run.text)?;
        }
        for equality in group.equalities {
            if translation_slice(translations, equality.left)?
                != translation_slice(translations, equality.right)?
            {
                return Err(error("BOXOPT 공용 pixel 번역 구간 불일치"));
            }
        }
        for literal in group.literals {
            if translation_slice(translations, literal.slice)? != literal.expected {
                return Err(error(format!(
                    "{} 보존 literal 결속 불일치",
                    literal.slice.block_id
                )));
            }
        }

        for block_id in group.target_block_ids {
            let translation = translations[block_id];
            let mut covered = BTreeSet::new();
            for slice in group
                .runs
                .iter()
                .map(|run| run.text)
                .chain(
                    group
                        .equalities
                        .iter()
                        .flat_map(|item| [item.left, item.right]),
                )
                .chain(group.literals.iter().map(|item| item.slice))
                .filter(|slice| slice.block_id == *block_id)
            {
                covered.extend(slice.start..slice.end);
            }
            if translation
                .ko
                .chars()
                .enumerate()
                .any(|(index, character)| !character.is_whitespace() && !covered.contains(&index))
            {
                return Err(error(format!(
                    "{block_id} 번역의 비공백 문자가 수동 조판에 포함되지 않음"
                )));
            }
        }
    }
    if actual_targets != expected_targets || actual_preserved != expected_preserved {
        return Err(error(format!(
            "겹침 catalog와 수동 조판 분모 불일치: target={actual_targets:?}/{expected_targets:?}, preserved={actual_preserved:?}/{expected_preserved:?}"
        )));
    }
    Ok(actual_targets)
}

fn render_shared_layout(
    layout: SharedSelectorLayout,
    asset: TextAssetSpec<'static>,
    translations: &BTreeMap<&str, &GeneralRasterTranslationEntry>,
    fonts: &BakedFontBook,
    profile: BakedTextProfile<'_>,
    original_pixels: &[u8],
    output_pixels: &mut [u8],
) -> Result<(), BoxoptDevelopmentSurfaceError> {
    let blocks = asset
        .blocks
        .iter()
        .map(|block| (block.id, *block))
        .collect::<BTreeMap<_, _>>();
    for block_id in layout.target_block_ids {
        clear_rect(output_pixels, asset.layout.width, blocks[block_id].bbox);
    }
    for block_id in layout.preserved_block_ids {
        copy_rect(
            original_pixels,
            output_pixels,
            asset.layout.width,
            blocks[block_id].bbox,
        );
    }
    for run in layout.runs {
        let source_block = blocks
            .get(run.text.block_id)
            .copied()
            .ok_or_else(|| error(format!("{} 조판 source block 누락", run.text.block_id)))?;
        let run_block = TextBlockSpec {
            bbox: run.rect,
            ..source_block
        };
        let run_profile = BakedTextProfile {
            align: run.align,
            ..profile
        };
        let text = translation_slice(translations, run.text)?;
        let layer =
            render_baked_text_block(run_block, PixelFormat::Indexed4, fonts, &text, run_profile)
                .map_err(|source| error(source.to_string()))?;
        paste_rect(output_pixels, asset.layout.width, run.rect, &layer);
    }
    Ok(())
}

/// 수동 번역과 명시 profile을 46개 selector에 한 번에 합성한다.
pub fn patch_boxopt_development_surface(
    original: &[u8],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), BoxoptDevelopmentSurfaceError> {
    let asset = BOXOPT_TEXT_ASSET;
    let policy_blocks = verify_sparse_selector_policy_for_asset(asset)
        .map_err(|source| error(source.to_string()))?;
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
    if policy_blocks != target_blocks.len()
        || target_blocks != translation_blocks
        || translations.iter().any(|entry| {
            !matches!(
                entry.status,
                GeneralRasterTranslationStatus::DraftHumanReviewRequired
                    | GeneralRasterTranslationStatus::Approved
            )
        })
    {
        return Err(error("BOXOPT 정책·catalog·개발 번역 분모 불일치"));
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
    if profile_keys.len() != 1 {
        return Err(error("BOXOPT category profile 후보가 하나가 아님"));
    }
    let profile_key = *profile_keys
        .first()
        .ok_or_else(|| error("BOXOPT profile 후보 누락"))?;
    let assignments =
        general_btx_raster_profile_assignments().map_err(|source| error(source.to_string()))?;
    let assignment = assignments
        .iter()
        .find(|assignment| {
            assignment.semantic_family == profile_key.0
                && assignment.profile_candidate_index == profile_key.1
        })
        .ok_or_else(|| error("BOXOPT profile assignment 누락"))?;
    if !matches!(
        assignment.status,
        GeneralRasterProfileStatus::DevelopmentDraftHumanReviewRequired
            | GeneralRasterProfileStatus::Approved
    ) {
        return Err(error("BOXOPT profile은 개발 합성 가능한 상태가 아님"));
    }
    let configuration = assignment
        .configuration
        .as_ref()
        .ok_or_else(|| error("BOXOPT profile 설정 누락"))?;
    let profile = configuration.baked_text_profile();
    if profile.background != BACKGROUND_INDEX || profile.line_count != 1 {
        return Err(error("BOXOPT profile의 배경 또는 수동 행 수 불일치"));
    }

    let translations_by_id = translations
        .iter()
        .map(|entry| (entry.block_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let shared_targets = validate_layouts(asset, &translations_by_id)?;
    let original_pixels =
        decode(original, asset.layout).map_err(|source| error(source.to_string()))?;
    let mut shared_surface = original_pixels.clone();
    let fonts = BakedFontBook::with_bundled_maplestory_light()
        .map_err(|source| error(source.to_string()))?;
    for layout in SHARED_SELECTOR_LAYOUTS {
        render_shared_layout(
            layout,
            asset,
            &translations_by_id,
            &fonts,
            profile,
            &original_pixels,
            &mut shared_surface,
        )?;
    }

    let blocks_by_id = asset
        .blocks
        .iter()
        .map(|block| (block.id, *block))
        .collect::<BTreeMap<_, _>>();
    let mut replacement_pixels = Vec::with_capacity(translations.len());
    for translation in &translations {
        let block = blocks_by_id[translation.block_id.as_str()];
        let pixels = if shared_targets.contains(block.id) {
            crop_rect(&shared_surface, asset.layout.width, block.bbox)
        } else {
            render_baked_text_block(block, asset.layout.format, &fonts, &translation.ko, profile)
                .map_err(|source| error(source.to_string()))?
        };
        replacement_pixels.push((translation.block_id.as_str(), pixels));
    }
    let replacements = replacement_pixels
        .iter()
        .map(|(block_id, pixels)| SparseSelectorRegionReplacement { block_id, pixels })
        .collect::<Vec<_>>();
    let (patched, sparse_report) = compose_sparse_selector_surface(original, asset, &replacements)
        .map_err(|source| error(source.to_string()))?;
    let patched = super::subgame_help::compose_help_glyphs(original, &patched).map_err(error)?;
    let report = GeneralRasterDevelopmentSurfaceReport {
        asset: asset.name,
        replacement_blocks: sparse_report.replacement_blocks,
        changed_pixels: {
            let final_pixels = decode(&patched, asset.layout).map_err(|e| error(e.to_string()))?;
            original_pixels
                .iter()
                .zip(final_pixels)
                .filter(|(a, b)| **a != *b)
                .count()
        },
        changed_bytes: original
            .iter()
            .zip(&patched)
            .filter(|(a, b)| a != b)
            .count(),
        output_sha256: {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(&patched))
        },
        // selector의 승인만으로 새 설명 번역·profile까지 승인하지 않는다.
        translation_approved: false,
        profile_approved: false,
        runtime_verified: false,
        product_eligible: false,
    };
    if report.replacement_blocks != target_blocks.len()
        || report.changed_pixels == 0
        || report.changed_bytes == 0
        || report.runtime_verified
        || report.product_eligible
    {
        return Err(error("BOXOPT 개발 합성 판정 drift"));
    }
    Ok((patched, report))
}

#[cfg(test)]
#[path = "boxopt_development_surface_tests.rs"]
mod tests;
