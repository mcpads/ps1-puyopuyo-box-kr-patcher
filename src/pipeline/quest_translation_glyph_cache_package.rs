//! 선택된 QUEST 번역문용 sparse glyph cache package 직렬화.
//!
//! plan에서 section offset을 파생하고 실제 bytes를 만든 뒤 모든 section과
//! glyph/page assignment와 package 밖 original atlas source를 다시 읽는다.
//! 아직 hook 소비 ABI가 아니므로
//! 파생 offset과 digest를 중복 상수로 동결하지 않는다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use crate::disc::VerifiedSourceImage;
use crate::script::QuestTranslationSelectionAnalysis;

use super::quest_glyph_cache_package::{
    serialize_quest_glyph_cache_materials, QuestGlyphCachePackage,
};
use super::quest_translation_encoding::QuestTranslationEncodingPlan;
use super::quest_translation_glyph_cache_plan::{
    build_quest_translation_glyph_cache_materials,
    build_quest_translation_glyph_cache_materials_with_encoding,
    build_quest_translation_glyph_cache_materials_with_supplemental_pages,
    QuestTranslationGlyphCacheSupplementalPage, QUEST_TRANSLATION_GLYPH_CACHE_PLAN_SCHEMA_VERSION,
};
use super::quest_translation_selection_routing::QuestTranslationSelectionRoutingAnalysis;

pub const QUEST_TRANSLATION_GLYPH_CACHE_PACKAGE_SCHEMA_VERSION: &str =
    "quest-translation-glyph-cache-package-v2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationGlyphCachePackageError(String);

impl fmt::Display for QuestTranslationGlyphCachePackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestTranslationGlyphCachePackageError {}

fn error(message: impl Into<String>) -> QuestTranslationGlyphCachePackageError {
    QuestTranslationGlyphCachePackageError(message.into())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

/// 검토문 plan에서 offset과 bytes를 파생한 package를 만든다.
pub fn audit_quest_translation_glyph_cache_package(
    source: &VerifiedSourceImage,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
) -> Result<QuestGlyphCachePackage, QuestTranslationGlyphCachePackageError> {
    audit_quest_translation_glyph_cache_package_with_supplemental_pages(
        source,
        selection,
        routing,
        &[],
    )
}

/// 기존 VDD page 뒤에 supplemental page를 포함한 개발 package를 만든다.
pub fn audit_quest_translation_glyph_cache_package_with_supplemental_pages(
    source: &VerifiedSourceImage,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    supplemental_pages: &[QuestTranslationGlyphCacheSupplementalPage],
) -> Result<QuestGlyphCachePackage, QuestTranslationGlyphCachePackageError> {
    let materials = if supplemental_pages.is_empty() {
        build_quest_translation_glyph_cache_materials(source, selection, routing)
    } else {
        build_quest_translation_glyph_cache_materials_with_supplemental_pages(
            source,
            selection,
            routing,
            supplemental_pages,
        )
    }
    .map_err(|source| error(source.to_string()))?;
    serialize_quest_glyph_cache_materials(
        materials,
        QUEST_TRANSLATION_GLYPH_CACHE_PACKAGE_SCHEMA_VERSION,
    )
    .map_err(|source| error(source.to_string()))
}

/// 검산된 encoding의 page assignment 순서를 보존해 package를 만든다.
pub(super) fn audit_quest_translation_glyph_cache_package_with_encoding(
    source: &VerifiedSourceImage,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    encoding: &QuestTranslationEncodingPlan,
) -> Result<QuestGlyphCachePackage, QuestTranslationGlyphCachePackageError> {
    let materials =
        build_quest_translation_glyph_cache_materials_with_encoding(source, routing, encoding)
            .map_err(|source| error(source.to_string()))?;
    serialize_quest_glyph_cache_materials(
        materials,
        QUEST_TRANSLATION_GLYPH_CACHE_PACKAGE_SCHEMA_VERSION,
    )
    .map_err(|source| error(source.to_string()))
}

/// 검토문 package의 파생 layout과 모든 readback을 검증한다.
pub fn verify_box_quest_translation_glyph_cache_package(
    source: &VerifiedSourceImage,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
) -> Result<QuestGlyphCachePackage, QuestTranslationGlyphCachePackageError> {
    verify_box_quest_translation_glyph_cache_package_with_supplemental_pages(
        source,
        selection,
        routing,
        &[],
    )
}

/// supplemental page까지 직렬화한 package의 모든 page assignment를 검증한다.
pub fn verify_box_quest_translation_glyph_cache_package_with_supplemental_pages(
    source: &VerifiedSourceImage,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    supplemental_pages: &[QuestTranslationGlyphCacheSupplementalPage],
) -> Result<QuestGlyphCachePackage, QuestTranslationGlyphCachePackageError> {
    let package = audit_quest_translation_glyph_cache_package_with_supplemental_pages(
        source,
        selection,
        routing,
        supplemental_pages,
    )?;
    let assigned = (1..=package.plan.pages)
        .map(|page| {
            let page_id = u8::try_from(page)
                .map_err(|_| error("QUEST 번역 glyph package page ID가 u8을 넘음"))?;
            package
                .page_catalog_indices(page_id)
                .map(|indices| indices.len())
                .map_err(|source| error(source.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if package.schema_version != QUEST_TRANSLATION_GLYPH_CACHE_PACKAGE_SCHEMA_VERSION
        || package.plan.schema_version != QUEST_TRANSLATION_GLYPH_CACHE_PLAN_SCHEMA_VERSION
        || package.layout.package_bytes != package.plan.package_bytes
        || package.bytes.len() != package.layout.package_bytes
        || assigned.iter().sum::<usize>() != package.plan.page_assignments
        || assigned.iter().copied().max().unwrap_or(0) != package.plan.max_page_glyphs
        || !is_sha256_hex(&package.sha256)
        || !package.sections_exact
        || !package.catalog_readback_exact
        || !package.original_atlas_source_roundtrip_exact
        || !package.page_assignment_readback_exact
        || !package.slot_rect_readback_exact
        || !package.korean_descriptor_readback_exact
        || !package.scratch_zero
        || package.runtime_verified
        || package.build_eligible
    {
        return Err(error("QUEST 번역 glyph cache package 판정 drift"));
    }
    Ok(package)
}

/// ordered encoding과 package의 모든 section·assignment readback을 검증한다.
pub(super) fn verify_box_quest_translation_glyph_cache_package_with_encoding(
    source: &VerifiedSourceImage,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    encoding: &QuestTranslationEncodingPlan,
) -> Result<QuestGlyphCachePackage, QuestTranslationGlyphCachePackageError> {
    let package =
        audit_quest_translation_glyph_cache_package_with_encoding(source, routing, encoding)?;
    let assigned = (1..=package.plan.pages)
        .map(|page| {
            let page_id = u8::try_from(page)
                .map_err(|_| error("QUEST ordered glyph package page ID가 u8을 넘음"))?;
            package
                .page_catalog_indices(page_id)
                .map(|indices| indices.len())
                .map_err(|source| error(source.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if package.schema_version != QUEST_TRANSLATION_GLYPH_CACHE_PACKAGE_SCHEMA_VERSION
        || package.plan.schema_version != QUEST_TRANSLATION_GLYPH_CACHE_PLAN_SCHEMA_VERSION
        || package.layout.package_bytes != package.plan.package_bytes
        || package.bytes.len() != package.layout.package_bytes
        || assigned.iter().sum::<usize>() != package.plan.page_assignments
        || assigned.iter().copied().max().unwrap_or(0) != package.plan.max_page_glyphs
        || !is_sha256_hex(&package.sha256)
        || !package.sections_exact
        || !package.catalog_readback_exact
        || !package.original_atlas_source_roundtrip_exact
        || !package.page_assignment_readback_exact
        || !package.slot_rect_readback_exact
        || !package.korean_descriptor_readback_exact
        || !package.scratch_zero
        || package.runtime_verified
        || package.build_eligible
    {
        return Err(error("QUEST ordered glyph cache package 판정 drift"));
    }
    Ok(package)
}

/// 검토문 package의 현재 파생 layout과 digest를 기록하는 TSV.
pub fn render_quest_translation_glyph_cache_package_tsv(
    package: &QuestGlyphCachePackage,
) -> String {
    let layout = package.layout;
    let mut output = String::from(
        "schema_version\tglyphs\tpages\tmax_page_glyphs\tpage_assignments\t\
         slot_rects_offset\tslot_rects_bytes\tkorean_descriptor_offset\t\
         korean_descriptor_bytes\tglyph_dictionary_offset\tglyph_dictionary_bytes\t\
         glyph_indices_offset\tglyph_indices_bytes\tpage_directory_offset\t\
         page_directory_bytes\tpage_assignments_offset\t\
         page_assignments_bytes\talignment_padding_offset\talignment_padding_bytes\t\
         scratch_offset\tscratch_bytes\tpackage_bytes\tpackage_sha256\tsections_exact\t\
         original_atlas_source_roundtrip_exact\truntime_verified\tbuild_eligible\n",
    );
    writeln!(
        output,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        package.schema_version,
        package.plan.glyphs,
        package.plan.pages,
        package.plan.max_page_glyphs,
        package.plan.page_assignments,
        layout.slot_rects_offset,
        layout.slot_rects_bytes,
        layout.korean_descriptor_offset,
        layout.korean_descriptor_bytes,
        layout.glyph_row_dictionary_offset,
        layout.glyph_row_dictionary_bytes,
        layout.glyph_cell_indices_offset,
        layout.glyph_cell_indices_bytes,
        layout.page_directory_offset,
        layout.page_directory_bytes,
        layout.page_assignments_offset,
        layout.page_assignments_bytes,
        layout.alignment_padding_offset,
        layout.alignment_padding_bytes,
        layout.scratch_offset,
        layout.scratch_bytes,
        layout.package_bytes,
        package.sha256,
        package.sections_exact,
        package.original_atlas_source_roundtrip_exact,
        package.runtime_verified,
        package.build_eligible,
    )
    .expect("String write는 실패하지 않음");
    output
}

#[cfg(test)]
#[path = "quest_translation_glyph_cache_package_tests.rs"]
mod tests;
