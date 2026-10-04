//! 선택된 QUEST 번역문 전용 sparse glyph cache 저장 계획.
//!
//! 기본 검토 경계에서는 route의 page ID를 보존하고, 한국어가 선택된 page만
//! assignment를 가진다. page upload가 덮는 원본 문장부호도 동적 글리프로
//! 승격한다. 원문 유지 route는 cache 수요에 넣지 않는다. 이 결과는 저장
//! 용량만 판정하며 실제 package·
//! writer·사람 승인·제품 적격을 대신하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use crate::disc::{Iso9660, VerifiedSourceImage};
use crate::script::QuestTranslationSelectionAnalysis;

use super::quest_glyph_cache_plan::{
    build_quest_glyph_cache_materials_for_ordered_pages, QuestGlyphCacheMaterials,
    QuestGlyphCachePlan, QUEST_GLYPH_CACHE_KOREAN_DESCRIPTOR_BYTES,
    QUEST_GLYPH_CACHE_PAGE_OFFSET_BYTES, QUEST_GLYPH_CACHE_PLAIN_ASSIGNMENT_BYTES,
    QUEST_GLYPH_CACHE_SCRATCH_ALIGNMENT, QUEST_GLYPH_CACHE_SCRATCH_BYTES,
    QUEST_GLYPH_CACHE_SLOT_RECT_BYTES,
};
use super::quest_poc::quest_code_table;
use super::quest_translation_encoding::{
    build_quest_translation_encoding_plan_with_supplemental_pages, QuestTranslationEncodingPlan,
    QuestTranslationEncodingSupplementalPage,
};
use super::quest_translation_selection_routing::{
    QuestTranslationSelectionPlacement, QuestTranslationSelectionRoutingAnalysis,
};

pub const QUEST_TRANSLATION_GLYPH_CACHE_PLAN_SCHEMA_VERSION: &str =
    "quest-translation-glyph-cache-plan-v2";

/// page 하나의 선택문과 동적 repertoire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationGlyphCachePage {
    pub page_id: u8,
    pub selected_routes: usize,
    pub glyphs: String,
}

/// 개발 빌드가 기존 VDD page 뒤에 덧붙이는 독립 glyph page 입력.
///
/// 기존 공개 struct literal의 `glyphs` 필드를 보존한다. encoding 계층의
/// `used_glyphs` 입력으로 바꾸는 일은 이 모듈 내부에서만 한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationGlyphCacheSupplementalPage {
    pub page_id: u8,
    pub glyphs: String,
}

/// 검토문 전용 cache의 page 결속과 저장량.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationGlyphCachePlanAnalysis {
    pub pages: Vec<QuestTranslationGlyphCachePage>,
    pub plan: QuestGlyphCachePlan,
    pub selected_routes: usize,
    pub retained_source_routes: usize,
    pub selected_pages: usize,
    pub translation_approved_routes: usize,
    pub layout_approved_routes: usize,
    pub product_eligible_routes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationGlyphCachePlanError(String);

impl fmt::Display for QuestTranslationGlyphCachePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestTranslationGlyphCachePlanError {}

fn error(message: impl Into<String>) -> QuestTranslationGlyphCachePlanError {
    QuestTranslationGlyphCachePlanError(message.into())
}

struct QuestTranslationGlyphCacheDemand {
    page_glyphs: BTreeMap<u8, Vec<char>>,
    selected_routes_by_page: BTreeMap<u8, usize>,
}

fn translation_glyph_cache_demand_from_encoding(
    encoding: &QuestTranslationEncodingPlan,
    routing: &QuestTranslationSelectionRoutingAnalysis,
) -> Result<QuestTranslationGlyphCacheDemand, QuestTranslationGlyphCachePlanError> {
    if encoding.selected_routes != routing.selected
        || encoding.retained_source_routes != routing.retained_source
    {
        return Err(error("QUEST 번역 glyph cache encoding과 route 분모 불일치"));
    }
    let page_glyphs = encoding
        .pages
        .iter()
        .map(|page| (page.page_id, page.glyphs.chars().collect()))
        .collect::<BTreeMap<u8, Vec<char>>>();
    let mut selected_routes_by_page = BTreeMap::<u8, usize>::new();
    for route in &routing.routes {
        if route.placement != QuestTranslationSelectionPlacement::RetainSource {
            *selected_routes_by_page.entry(route.page_id).or_default() += 1;
        }
    }
    Ok(QuestTranslationGlyphCacheDemand {
        page_glyphs,
        selected_routes_by_page,
    })
}

fn translation_glyph_cache_demand_with_supplemental_pages(
    source: &VerifiedSourceImage,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    supplemental_pages: &[QuestTranslationGlyphCacheSupplementalPage],
) -> Result<QuestTranslationGlyphCacheDemand, QuestTranslationGlyphCachePlanError> {
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let code_table = quest_code_table(&iso).map_err(|source| error(source.to_string()))?;
    let encoding_supplemental_pages = supplemental_pages
        .iter()
        .map(|page| QuestTranslationEncodingSupplementalPage {
            page_id: page.page_id,
            used_glyphs: page.glyphs.clone(),
        })
        .collect::<Vec<_>>();
    let encoding = build_quest_translation_encoding_plan_with_supplemental_pages(
        selection,
        routing,
        &code_table,
        &encoding_supplemental_pages,
    )
    .map_err(|source| error(source.to_string()))?;
    translation_glyph_cache_demand_from_encoding(&encoding, routing)
}

pub(super) fn build_quest_translation_glyph_cache_materials(
    source: &VerifiedSourceImage,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
) -> Result<QuestGlyphCacheMaterials, QuestTranslationGlyphCachePlanError> {
    build_quest_translation_glyph_cache_materials_with_supplemental_pages(
        source,
        selection,
        routing,
        &[],
    )
}

/// 기존 VDD page 뒤에 개발 전용 page를 더한 cache materials를 만든다.
pub(super) fn build_quest_translation_glyph_cache_materials_with_supplemental_pages(
    source: &VerifiedSourceImage,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    supplemental_pages: &[QuestTranslationGlyphCacheSupplementalPage],
) -> Result<QuestGlyphCacheMaterials, QuestTranslationGlyphCachePlanError> {
    let demand = translation_glyph_cache_demand_with_supplemental_pages(
        source,
        selection,
        routing,
        supplemental_pages,
    )?;
    let repertoire = demand
        .page_glyphs
        .values()
        .flat_map(|glyphs| glyphs.iter().copied())
        .collect::<BTreeSet<_>>();
    let materials = build_quest_glyph_cache_materials_for_ordered_pages(
        source,
        QUEST_TRANSLATION_GLYPH_CACHE_PLAN_SCHEMA_VERSION,
        demand.page_glyphs,
        repertoire,
    )
    .map_err(|source| error(source.to_string()))?;
    if !supplemental_pages.is_empty() && !materials.plan.package_fits_carrier {
        return Err(error(format!(
            "QUEST supplemental glyph cache package가 carrier 용량을 초과함: package={}, carrier={}",
            materials.plan.package_bytes, materials.plan.carrier_bytes
        )));
    }
    Ok(materials)
}

/// 이미 검산·정렬된 encoding의 page 순서를 보존해 cache materials를 만든다.
pub(super) fn build_quest_translation_glyph_cache_materials_with_encoding(
    source: &VerifiedSourceImage,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    encoding: &QuestTranslationEncodingPlan,
) -> Result<QuestGlyphCacheMaterials, QuestTranslationGlyphCachePlanError> {
    let demand = translation_glyph_cache_demand_from_encoding(encoding, routing)?;
    let repertoire = demand
        .page_glyphs
        .values()
        .flat_map(|glyphs| glyphs.iter().copied())
        .collect::<BTreeSet<_>>();
    let materials = build_quest_glyph_cache_materials_for_ordered_pages(
        source,
        QUEST_TRANSLATION_GLYPH_CACHE_PLAN_SCHEMA_VERSION,
        demand.page_glyphs,
        repertoire,
    )
    .map_err(|source| error(source.to_string()))?;
    if !materials.plan.package_fits_carrier {
        return Err(error(format!(
            "QUEST ordered glyph cache package가 carrier 용량을 초과함: package={}, carrier={}",
            materials.plan.package_bytes, materials.plan.carrier_bytes
        )));
    }
    Ok(materials)
}

/// 검토문 선택과 route에서 기존 page ID를 보존한 cache 수요를 만든다.
pub fn audit_quest_translation_glyph_cache_plan(
    source: &VerifiedSourceImage,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
) -> Result<QuestTranslationGlyphCachePlanAnalysis, QuestTranslationGlyphCachePlanError> {
    audit_quest_translation_glyph_cache_plan_with_supplemental_pages(
        source,
        selection,
        routing,
        &[],
    )
}

/// 기존 VDD page와 개발 전용 supplemental page를 한 cache 계획으로 계산한다.
pub fn audit_quest_translation_glyph_cache_plan_with_supplemental_pages(
    source: &VerifiedSourceImage,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    supplemental_pages: &[QuestTranslationGlyphCacheSupplementalPage],
) -> Result<QuestTranslationGlyphCachePlanAnalysis, QuestTranslationGlyphCachePlanError> {
    let demand = translation_glyph_cache_demand_with_supplemental_pages(
        source,
        selection,
        routing,
        supplemental_pages,
    )?;
    let repertoire = demand
        .page_glyphs
        .values()
        .flat_map(|glyphs| glyphs.iter().copied())
        .collect::<BTreeSet<_>>();
    let materials = build_quest_glyph_cache_materials_for_ordered_pages(
        source,
        QUEST_TRANSLATION_GLYPH_CACHE_PLAN_SCHEMA_VERSION,
        demand.page_glyphs.clone(),
        repertoire,
    )
    .map_err(|source| error(source.to_string()))?;
    if !supplemental_pages.is_empty() && !materials.plan.package_fits_carrier {
        return Err(error(format!(
            "QUEST supplemental glyph cache package가 carrier 용량을 초과함: package={}, carrier={}",
            materials.plan.package_bytes, materials.plan.carrier_bytes
        )));
    }
    let pages = demand
        .page_glyphs
        .into_iter()
        .map(|(page_id, glyphs)| QuestTranslationGlyphCachePage {
            page_id,
            selected_routes: demand
                .selected_routes_by_page
                .get(&page_id)
                .copied()
                .unwrap_or(0),
            glyphs: glyphs.into_iter().collect(),
        })
        .collect::<Vec<_>>();
    let selected_pages = pages.iter().filter(|page| !page.glyphs.is_empty()).count();
    Ok(QuestTranslationGlyphCachePlanAnalysis {
        pages,
        plan: materials.plan,
        selected_routes: routing.selected,
        retained_source_routes: routing.retained_source,
        selected_pages,
        translation_approved_routes: 0,
        layout_approved_routes: 0,
        product_eligible_routes: 0,
    })
}

/// 검토 cache의 page별 수요와 전역 저장량을 대조하는 TSV.
pub fn render_quest_translation_glyph_cache_plan_tsv(
    analysis: &QuestTranslationGlyphCachePlanAnalysis,
) -> String {
    let mut output = String::from(
        "schema_version\tpage_id\tselected_routes\tglyphs\tglyph_count\tselected_total\t\
         retained_source_total\tdirectory_pages\tselected_pages\trepertoire_glyphs\t\
         max_page_glyphs\tpage_assignments\tpage_directory_bytes\tpage_assignment_bytes\t\
         dictionary_rows\tcatalog_bytes\toriginal_atlas_source_bytes\tslot_rect_bytes\t\
         korean_descriptor_bytes\talignment_padding_bytes\t\
         scratch_bytes\tpackage_bytes\tcarrier_remaining_bytes\t\
         package_fits\ttranslation_approved\tlayout_approved\tproduct_eligible\n",
    );
    for page in &analysis.pages {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tno\tno\tno",
            QUEST_TRANSLATION_GLYPH_CACHE_PLAN_SCHEMA_VERSION,
            page.page_id,
            page.selected_routes,
            page.glyphs,
            page.glyphs.chars().count(),
            analysis.selected_routes,
            analysis.retained_source_routes,
            analysis.plan.pages,
            analysis.selected_pages,
            analysis.plan.glyphs,
            analysis.plan.max_page_glyphs,
            analysis.plan.page_assignments,
            analysis.plan.page_directory_bytes,
            analysis.plan.page_assignment_bytes,
            analysis.plan.catalog.dictionary_rows,
            analysis.plan.catalog.stored_bytes,
            analysis.plan.original_atlas_source.stored_bytes,
            analysis.plan.slot_rect_bytes,
            analysis.plan.korean_descriptor_bytes,
            analysis.plan.alignment_padding_bytes,
            analysis.plan.scratch_bytes,
            analysis.plan.package_bytes,
            analysis.plan.carrier_remaining_bytes,
            analysis.plan.package_fits_carrier,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

/// 지원 원본의 검토 cache 분모·저장량·결정적 TSV를 검증한다.
pub fn verify_box_quest_translation_glyph_cache_plan(
    source: &VerifiedSourceImage,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
) -> Result<QuestTranslationGlyphCachePlanAnalysis, QuestTranslationGlyphCachePlanError> {
    let analysis = audit_quest_translation_glyph_cache_plan(source, selection, routing)?;
    let page_glyph_counts = analysis
        .pages
        .iter()
        .map(|page| page.glyphs.chars().count())
        .collect::<Vec<_>>();
    let expected_glyphs = analysis
        .pages
        .iter()
        .flat_map(|page| page.glyphs.chars())
        .collect::<BTreeSet<_>>()
        .len();
    let expected_assignments = page_glyph_counts.iter().sum::<usize>();
    let expected_max_page_glyphs = page_glyph_counts.iter().copied().max().unwrap_or(0);
    let expected_page_directory_bytes = analysis
        .plan
        .pages
        .checked_add(1)
        .and_then(|entries| entries.checked_mul(QUEST_GLYPH_CACHE_PAGE_OFFSET_BYTES))
        .ok_or_else(|| error("QUEST 번역 glyph cache page directory 크기 overflow"))?;
    let expected_plain_assignment_bytes = expected_assignments
        .checked_mul(QUEST_GLYPH_CACHE_PLAIN_ASSIGNMENT_BYTES)
        .ok_or_else(|| error("QUEST 번역 glyph cache plain assignment 크기 overflow"))?;
    let expected_slot_rect_bytes = expected_max_page_glyphs
        .checked_mul(QUEST_GLYPH_CACHE_SLOT_RECT_BYTES)
        .ok_or_else(|| error("QUEST 번역 glyph cache slot RECT 크기 overflow"))?;
    let expected_catalog_stored_bytes = analysis
        .plan
        .catalog
        .dictionary_bytes
        .checked_add(analysis.plan.catalog.cell_index_bytes)
        .ok_or_else(|| error("QUEST 번역 glyph cache catalog 크기 overflow"))?;
    let expected_original_atlas_source_stored_bytes = analysis
        .plan
        .original_atlas_source
        .index_bytes
        .checked_add(analysis.plan.original_atlas_source.payload_bytes)
        .ok_or_else(|| error("QUEST 번역 glyph cache original atlas source 크기 overflow"))?;
    let expected_package_bytes = analysis
        .plan
        .catalog
        .stored_bytes
        .checked_add(analysis.plan.page_directory_bytes)
        .and_then(|value| value.checked_add(analysis.plan.page_assignment_bytes))
        .and_then(|value| value.checked_add(analysis.plan.slot_rect_bytes))
        .and_then(|value| value.checked_add(analysis.plan.korean_descriptor_bytes))
        .and_then(|value| value.checked_add(analysis.plan.alignment_padding_bytes))
        .and_then(|value| value.checked_add(analysis.plan.scratch_bytes))
        .ok_or_else(|| error("QUEST 번역 glyph cache package 합계 overflow"))?;
    let expected_carrier_remaining = analysis
        .plan
        .carrier_bytes
        .checked_sub(analysis.plan.package_bytes)
        .ok_or_else(|| error("QUEST 번역 glyph cache package가 carrier를 초과함"))?;
    let scratch_offset = analysis
        .plan
        .package_bytes
        .checked_sub(analysis.plan.scratch_bytes)
        .ok_or_else(|| error("QUEST 번역 glyph cache scratch 범위가 package를 초과함"))?;
    if analysis.plan.schema_version != QUEST_TRANSLATION_GLYPH_CACHE_PLAN_SCHEMA_VERSION
        || analysis.plan.glyphs != expected_glyphs
        || analysis.plan.pages != analysis.pages.len()
        || analysis.selected_pages != routing.selected_pages
        || analysis.selected_routes != routing.selected
        || analysis.retained_source_routes != routing.retained_source
        || analysis.plan.max_page_glyphs != expected_max_page_glyphs
        || analysis.plan.page_assignments != expected_assignments
        || analysis.plan.page_directory_bytes != expected_page_directory_bytes
        || analysis.plan.page_assignment_plain_bytes != expected_plain_assignment_bytes
        || analysis.plan.slot_rect_bytes != expected_slot_rect_bytes
        || analysis.plan.korean_descriptor_bytes != QUEST_GLYPH_CACHE_KOREAN_DESCRIPTOR_BYTES
        || analysis.plan.catalog.cells != expected_glyphs
        || analysis.plan.catalog.stored_bytes != expected_catalog_stored_bytes
        || analysis.plan.original_atlas_source.cells != expected_max_page_glyphs
        || analysis.plan.original_atlas_source.stored_bytes
            != expected_original_atlas_source_stored_bytes
        || analysis.plan.package_bytes != expected_package_bytes
        || analysis.plan.carrier_remaining_bytes != expected_carrier_remaining
        || analysis.plan.scratch_bytes != QUEST_GLYPH_CACHE_SCRATCH_BYTES
        || analysis.plan.alignment_padding_bytes >= QUEST_GLYPH_CACHE_SCRATCH_ALIGNMENT
        || !scratch_offset.is_multiple_of(QUEST_GLYPH_CACHE_SCRATCH_ALIGNMENT)
    {
        return Err(error("QUEST 번역 glyph cache 파생 저장량 불일치"));
    }
    if !analysis.plan.package_fits_carrier
        || !analysis.plan.glyph_roundtrip_exact
        || !analysis.plan.original_atlas_source_roundtrip_exact
        || !analysis.plan.page_assignment_roundtrip_exact
        || !analysis.plan.halfword_aligned_slots
        || analysis.plan.runtime_verified
        || analysis.plan.build_eligible
        || analysis.translation_approved_routes != 0
        || analysis.layout_approved_routes != 0
        || analysis.product_eligible_routes != 0
    {
        return Err(error("QUEST 번역 glyph cache 저장 판정 drift"));
    }
    Ok(analysis)
}

#[cfg(test)]
#[path = "quest_translation_glyph_cache_plan_tests.rs"]
mod tests;
