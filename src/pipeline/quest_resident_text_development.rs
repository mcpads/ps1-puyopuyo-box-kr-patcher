//! `QUEST.EXE` 상주 문구를 기존 번역 page 공급에 합치는 개발 입력.
//!
//! VDD route의 마지막 page 뒤에서 ID를 파생하고, version 관리 composition의
//! 실제 사용 문자와 package, 고정 span writer를 한 번에 결속한다. 이 결과는
//! 번역·geometry·runtime·제품 승인을 올리지 않는다.

use std::error::Error;
use std::fmt;

use crate::analysis::QuestResidentTextUnitAnalysis;
use crate::disc::VerifiedSourceImage;
use crate::font::QuestCodeTable;
use crate::script::QuestTranslationSelectionAnalysis;

use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_resident_text_compositions::{
    audit_quest_resident_text_compositions, quest_resident_text_composition_used_glyphs,
};
use super::quest_resident_text_writer::{
    audit_quest_resident_text_writer, QuestResidentTextPageAssignment,
    QuestResidentTextWriterAnalysis,
};
use super::quest_translation_encoding::{
    align_quest_page_code_prefix, build_quest_translation_encoding_plan_with_supplemental_pages,
    QuestTranslationEncodingPlan, QuestTranslationEncodingSupplementalPage,
};
use super::quest_translation_glyph_cache_package::verify_box_quest_translation_glyph_cache_package_with_encoding;
use super::quest_translation_glyph_cache_package::verify_box_quest_translation_glyph_cache_package_with_supplemental_pages;
use super::quest_translation_glyph_cache_plan::QuestTranslationGlyphCacheSupplementalPage;
use super::quest_translation_selection_routing::QuestTranslationSelectionRoutingAnalysis;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextDevelopmentInputs {
    pub page_id: u8,
    pub used_glyphs: String,
    pub encoding: QuestTranslationEncodingPlan,
    pub package: QuestGlyphCachePackage,
    pub writer: QuestResidentTextWriterAnalysis,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextDevelopmentError(String);

impl fmt::Display for QuestResidentTextDevelopmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentTextDevelopmentError {}

fn error(message: impl Into<String>) -> QuestResidentTextDevelopmentError {
    QuestResidentTextDevelopmentError(message.into())
}

/// source-bound unit와 수동 composition을 다음 supplemental page에 결속한다.
pub fn build_quest_resident_text_development_inputs(
    source: &VerifiedSourceImage,
    units: &QuestResidentTextUnitAnalysis,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    code_table: &QuestCodeTable,
) -> Result<QuestResidentTextDevelopmentInputs, QuestResidentTextDevelopmentError> {
    build_quest_resident_text_development_inputs_with_following_pages(
        source,
        units,
        selection,
        routing,
        code_table,
        &[],
    )
}

pub(super) fn build_quest_resident_text_development_inputs_with_following_pages(
    source: &VerifiedSourceImage,
    units: &QuestResidentTextUnitAnalysis,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    code_table: &QuestCodeTable,
    following_page_glyphs: &[String],
) -> Result<QuestResidentTextDevelopmentInputs, QuestResidentTextDevelopmentError> {
    build_quest_resident_text_development_inputs_with_page_code_prefix(
        source,
        units,
        selection,
        routing,
        code_table,
        following_page_glyphs,
        None,
    )
}

/// 뒤따르는 page들이 공통 glyph를 같은 code slot에 두도록 결속한다.
pub(super) fn build_quest_resident_text_development_inputs_with_aligned_following_pages(
    source: &VerifiedSourceImage,
    units: &QuestResidentTextUnitAnalysis,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    code_table: &QuestCodeTable,
    following_page_glyphs: &[String],
    shared_code_glyphs: &str,
) -> Result<QuestResidentTextDevelopmentInputs, QuestResidentTextDevelopmentError> {
    build_quest_resident_text_development_inputs_with_page_code_prefix(
        source,
        units,
        selection,
        routing,
        code_table,
        following_page_glyphs,
        Some(shared_code_glyphs),
    )
}

#[allow(clippy::too_many_arguments)]
fn build_quest_resident_text_development_inputs_with_page_code_prefix(
    source: &VerifiedSourceImage,
    units: &QuestResidentTextUnitAnalysis,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    code_table: &QuestCodeTable,
    following_page_glyphs: &[String],
    shared_code_glyphs: Option<&str>,
) -> Result<QuestResidentTextDevelopmentInputs, QuestResidentTextDevelopmentError> {
    let page_id = routing
        .routes
        .iter()
        .map(|route| route.page_id)
        .max()
        .and_then(|page_id| page_id.checked_add(1))
        .ok_or_else(|| error("QUEST resident supplemental page ID 파생 실패"))?;
    let compositions = audit_quest_resident_text_compositions(units)
        .map_err(|source| error(source.to_string()))?;
    let used_glyphs = quest_resident_text_composition_used_glyphs(&compositions)
        .map_err(|source| error(source.to_string()))?;
    let mut supplemental_pages = vec![QuestTranslationGlyphCacheSupplementalPage {
        page_id,
        glyphs: used_glyphs.clone(),
    }];
    let mut encoding_pages = vec![QuestTranslationEncodingSupplementalPage {
        page_id,
        used_glyphs: used_glyphs.clone(),
    }];
    for (index, glyphs) in following_page_glyphs.iter().enumerate() {
        let offset = u8::try_from(index + 1)
            .map_err(|_| error("QUEST following supplemental page 수가 u8을 넘음"))?;
        let following_page_id = page_id
            .checked_add(offset)
            .ok_or_else(|| error("QUEST following supplemental page ID overflow"))?;
        supplemental_pages.push(QuestTranslationGlyphCacheSupplementalPage {
            page_id: following_page_id,
            glyphs: glyphs.clone(),
        });
        encoding_pages.push(QuestTranslationEncodingSupplementalPage {
            page_id: following_page_id,
            used_glyphs: glyphs.clone(),
        });
    }
    let mut encoding = build_quest_translation_encoding_plan_with_supplemental_pages(
        selection,
        routing,
        code_table,
        &encoding_pages,
    )
    .map_err(|source| error(source.to_string()))?;
    let package = if let Some(shared_code_glyphs) = shared_code_glyphs {
        let following_page_ids = encoding_pages
            .iter()
            .skip(1)
            .map(|page| page.page_id)
            .collect::<Vec<_>>();
        if following_page_ids.len() >= 2 {
            align_quest_page_code_prefix(&mut encoding, &following_page_ids, shared_code_glyphs)
                .map_err(|source| error(source.to_string()))?;
        }
        verify_box_quest_translation_glyph_cache_package_with_encoding(source, routing, &encoding)
            .map_err(|source| error(source.to_string()))?
    } else {
        verify_box_quest_translation_glyph_cache_package_with_supplemental_pages(
            source,
            selection,
            routing,
            &supplemental_pages,
        )
        .map_err(|source| error(source.to_string()))?
    };
    let package = if shared_code_glyphs.is_some() && !following_page_glyphs.is_empty() {
        let materials =
            super::quest_glyph_cache_plan::build_quest_glyph_cache_materials_with_menu_font(
                source,
                package.plan.schema_version,
                encoding
                    .pages
                    .iter()
                    .map(|p| (p.page_id, p.glyphs.chars().collect()))
                    .collect(),
                encoding.repertoire.chars().collect(),
                Some(page_id + 1),
            )
            .map_err(|e| error(e.to_string()))?;
        super::quest_glyph_cache_package::serialize_quest_glyph_cache_materials(
            materials,
            package.schema_version,
        )
        .map_err(|e| error(e.to_string()))?
    } else {
        package
    };
    let assignments = units
        .units
        .iter()
        .map(|unit| QuestResidentTextPageAssignment {
            unit_id: unit.id.clone(),
            page_id,
        })
        .collect::<Vec<_>>();
    let writer = audit_quest_resident_text_writer(
        units,
        &compositions,
        &assignments,
        &encoding,
        &package,
        code_table,
    )
    .map_err(|source| error(source.to_string()))?;

    let final_page_id = page_id
        .checked_add(
            u8::try_from(following_page_glyphs.len())
                .map_err(|_| error("QUEST following supplemental page 수가 u8을 넘음"))?,
        )
        .ok_or_else(|| error("QUEST following supplemental 최종 page ID overflow"))?;
    if encoding.pages.last().map(|page| page.page_id) != Some(final_page_id)
        || package.plan.pages != usize::from(final_page_id)
        || writer.units != units.units.len()
        || writer.writes.iter().any(|write| write.page_id != page_id)
        || writer.translation_approved
        || writer.layout_approved
        || writer.runtime_verified
        || writer.product_eligible
    {
        return Err(error("QUEST resident development 입력 최종 결속 drift"));
    }

    Ok(QuestResidentTextDevelopmentInputs {
        page_id,
        used_glyphs,
        encoding,
        package,
        writer,
    })
}
