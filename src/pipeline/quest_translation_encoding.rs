//! 선택된 QUEST 한국어를 원본 코드와 page별 동적 글리프로 결속한다.
//!
//! 검토 원장의 ASCII 문장부호는 게임이 이미 쓰는 전각 기호로 정규화한다.
//! 선택 page는 원본 atlas와 다른 독립 bank를 쓰므로, 표시문이 쓰는
//! 비공백 글리프를 원본 code table 존재 여부와 관계없이 page
//! assignment에 모두 넣는다. 전각 공백은 원본 slot이 page upload에
//! 덮이지 않을 때만 원본 code를 재사용하고, 겹치면 page가 원본 공백 cell을
//! 명시적으로 공급한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::font::{
    canonicalize_quest_display_glyph, decode_quest_source_glyph, normalize_quest_display_text,
    QuestCodeTable,
};
use crate::script::{QuestTranslationSelectionAnalysis, QuestTranslationSelectionDisposition};

use super::quest_glyph_cache_plan::QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX;
use super::quest_translation_selection_routing::{
    QuestTranslationSelectionPlacement, QuestTranslationSelectionRoutingAnalysis,
};

/// 원본 QUEST code table에서 단일 Unicode 문자로 해석되는 항목.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestTranslationSourceGlyphCode {
    pub index: usize,
    pub code: [u8; 2],
}

/// page 하나가 cache에 올려야 하는 동적 글리프.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationEncodingPage {
    pub page_id: u8,
    pub glyphs: String,
}

/// 기존 VDD page 뒤에 더할 개발 전용 page의 사용 문자 집합.
///
/// `used_glyphs`는 중복 없는 집합 표현이어야 한다. 정규화한 집합에서
/// 비공백 문자는 항상 독립 bank의 동적 glyph가 된다. 전각 공백은 원본
/// slot이 그 page의 upload 범위와 겹칠 때만 동적 glyph로 승격한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationEncodingSupplementalPage {
    pub page_id: u8,
    pub used_glyphs: String,
}

/// 선택문 전체의 source/dynamic code 공급 계획.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationEncodingPlan {
    pub pages: Vec<QuestTranslationEncodingPage>,
    pub repertoire: String,
    pub selected_routes: usize,
    pub retained_source_routes: usize,
    source_codes: BTreeMap<char, QuestTranslationSourceGlyphCode>,
}

impl QuestTranslationEncodingPlan {
    /// page에 설치되는 동적 글리프 집합을 반환한다.
    pub fn page_glyphs(&self, page_id: u8) -> Option<&str> {
        self.pages
            .iter()
            .find(|page| page.page_id == page_id)
            .map(|page| page.glyphs.as_str())
    }

    /// 원본 atlas에 같은 의미의 glyph가 있는 문자 code를 반환한다.
    pub fn source_code(&self, glyph: char) -> Option<QuestTranslationSourceGlyphCode> {
        self.source_codes.get(&glyph).copied()
    }
}

/// 여러 page가 공통 glyph를 같은 code slot에 두도록 assignment 순서를 맞춘다.
///
/// 모든 대상 page에서 동적으로 공급되는 공통 glyph 집합이 같아야 한다.
/// 원본 code를 그대로 쓰는 glyph는 assignment prefix에 넣지 않는다.
pub(super) fn align_quest_page_code_prefix(
    plan: &mut QuestTranslationEncodingPlan,
    page_ids: &[u8],
    shared_used_glyphs: &str,
) -> Result<String, QuestTranslationEncodingError> {
    if page_ids.len() < 2 || page_ids.contains(&0) {
        return Err(error(
            "QUEST 공통 code prefix 대상 page가 둘 미만이거나 page 0을 포함함",
        ));
    }
    let page_ids = page_ids.iter().copied().collect::<BTreeSet<_>>();
    if page_ids.len() < 2 {
        return Err(error("QUEST 공통 code prefix 대상 page ID가 중복됨"));
    }
    let shared = normalize_quest_translation_text(shared_used_glyphs)?
        .chars()
        .collect::<BTreeSet<_>>();
    if shared.is_empty() {
        return Err(error("QUEST 공통 code prefix glyph가 비었음"));
    }

    let mut dynamic_shared = None;
    for page_id in &page_ids {
        let page = plan
            .pages
            .iter()
            .find(|page| page.page_id == *page_id)
            .ok_or_else(|| error(format!("QUEST 공통 code prefix page 누락: {page_id}")))?;
        let page_glyphs = page.glyphs.chars().collect::<BTreeSet<_>>();
        let current = shared
            .intersection(&page_glyphs)
            .copied()
            .collect::<BTreeSet<_>>();
        if dynamic_shared
            .as_ref()
            .is_some_and(|expected| expected != &current)
        {
            return Err(error(
                "QUEST 공통 glyph의 동적/source code 공급이 page마다 다름",
            ));
        }
        dynamic_shared = Some(current);
    }
    let prefix = dynamic_shared
        .ok_or_else(|| error("QUEST 공통 code prefix page가 비었음"))?
        .into_iter()
        .collect::<String>();

    for page in plan
        .pages
        .iter_mut()
        .filter(|page| page_ids.contains(&page.page_id))
    {
        let suffix = page
            .glyphs
            .chars()
            .filter(|glyph| !prefix.contains(*glyph))
            .collect::<String>();
        let ordered = format!("{prefix}{suffix}");
        if ordered.chars().collect::<BTreeSet<_>>() != page.glyphs.chars().collect::<BTreeSet<_>>()
        {
            return Err(error(format!(
                "QUEST 공통 code prefix가 page {} glyph 집합을 바꿈",
                page.page_id
            )));
        }
        page.glyphs = ordered;
    }
    Ok(prefix)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationEncodingError(String);

impl fmt::Display for QuestTranslationEncodingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestTranslationEncodingError {}

fn error(message: impl Into<String>) -> QuestTranslationEncodingError {
    QuestTranslationEncodingError(message.into())
}

pub(crate) fn canonicalize_quest_translation_glyph(character: char) -> char {
    canonicalize_quest_display_glyph(character).expect("단일 비-control 문자 정규화")
}

/// 검토 원장 표기를 게임 표시용 단일-cell 문자로 정규화한다.
pub fn normalize_quest_translation_text(
    text: &str,
) -> Result<String, QuestTranslationEncodingError> {
    normalize_quest_display_text(text).map_err(|source| error(source.to_string()))
}

fn source_glyph_codes(
    code_table: &QuestCodeTable,
) -> Result<BTreeMap<char, QuestTranslationSourceGlyphCode>, QuestTranslationEncodingError> {
    let mut result = BTreeMap::new();
    for (index, code) in code_table.codes().iter().copied().enumerate() {
        let Some(glyph) = decode_quest_source_glyph(code) else {
            continue;
        };
        let glyph = canonicalize_quest_translation_glyph(glyph);
        let row = QuestTranslationSourceGlyphCode { index, code };
        if let Some(existing) = result.insert(glyph, row) {
            return Err(error(format!(
                "QUEST 원본 code table의 Unicode 의미가 중복됨: {glyph:?}, index {} / {index}",
                existing.index
            )));
        }
    }
    Ok(result)
}

fn page_dynamic_glyphs(used_glyphs: &BTreeSet<char>) -> BTreeSet<char> {
    used_glyphs
        .iter()
        .filter(|glyph| **glyph != '\u{3000}')
        .copied()
        .collect()
}

fn page_storage_glyphs(
    used_glyphs: &BTreeSet<char>,
    source_codes: &BTreeMap<char, QuestTranslationSourceGlyphCode>,
) -> Result<BTreeSet<char>, QuestTranslationEncodingError> {
    let mut glyphs = page_dynamic_glyphs(used_glyphs);
    if !used_glyphs.contains(&'\u{3000}') {
        return Ok(glyphs);
    }
    let source_index = source_codes
        .get(&'\u{3000}')
        .ok_or_else(|| error("QUEST 전각 공백 source code가 없음"))?
        .index;
    let overwritten_end = QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX
        .checked_add(glyphs.len())
        .ok_or_else(|| error("QUEST text encoding 동적 slot 범위 overflow"))?;
    if (QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX..overwritten_end).contains(&source_index) {
        glyphs.insert('\u{3000}');
    }
    Ok(glyphs)
}

/// 표시문 집합이 sparse cache에 실제로 요구하는 동적 글리프를 계산한다.
///
/// page 재배치는 독립 bank가 자체 공급해야 할 비공백 사용 문자
/// 전체를 working set으로 삼는다.
pub(crate) fn quest_translation_dynamic_glyphs(used_glyphs: &BTreeSet<char>) -> BTreeSet<char> {
    page_dynamic_glyphs(used_glyphs)
}

/// 독립 page가 실제 package에 저장할 glyph 집합을 계산한다.
pub(crate) fn quest_translation_page_storage_glyphs(
    used_glyphs: &BTreeSet<char>,
    code_table: &QuestCodeTable,
) -> Result<BTreeSet<char>, QuestTranslationEncodingError> {
    page_storage_glyphs(used_glyphs, &source_glyph_codes(code_table)?)
}

fn page_encodings_with_supplemental_pages(
    mut used_by_page: BTreeMap<u8, BTreeSet<char>>,
    supplemental_pages: &[QuestTranslationEncodingSupplementalPage],
    source_codes: &BTreeMap<char, QuestTranslationSourceGlyphCode>,
    code_table_slots: usize,
) -> Result<Vec<QuestTranslationEncodingPage>, QuestTranslationEncodingError> {
    let mut additions = BTreeMap::<u8, BTreeSet<char>>::new();
    let mut supplemental_ids = BTreeSet::new();
    for page in supplemental_pages {
        if page.page_id == 0 {
            return Err(error("QUEST supplemental text encoding page ID 0은 예약됨"));
        }
        if used_by_page.contains_key(&page.page_id) {
            return Err(error(format!(
                "QUEST supplemental text encoding page가 기존 VDD page와 충돌함: {}",
                page.page_id
            )));
        }
        if !supplemental_ids.insert(page.page_id) {
            return Err(error(format!(
                "QUEST supplemental text encoding page ID가 중복됨: {}",
                page.page_id
            )));
        }

        let normalized = normalize_quest_translation_text(&page.used_glyphs)?;
        let glyph_count = normalized.chars().count();
        let glyphs = normalized.chars().collect::<BTreeSet<_>>();
        if glyphs.is_empty() || glyphs.iter().all(|glyph| glyph.is_whitespace()) {
            return Err(error(format!(
                "QUEST supplemental text encoding page의 사용 glyph가 비었거나 공백뿐임: {}",
                page.page_id
            )));
        }
        if glyphs.len() != glyph_count {
            return Err(error(format!(
                "QUEST supplemental text encoding page의 사용 glyph가 중복됨: {}",
                page.page_id
            )));
        }
        additions.insert(page.page_id, glyphs);
    }

    used_by_page.extend(additions);
    let last_page_id = u8::try_from(used_by_page.len())
        .map_err(|_| error("QUEST supplemental text encoding page 수가 u8 범위를 초과함"))?;
    if !used_by_page.keys().copied().eq(1..=last_page_id) {
        return Err(error(
            "QUEST supplemental text encoding page ID가 기존 VDD 뒤에서 연속하지 않음",
        ));
    }

    let dynamic_slot_capacity = code_table_slots
        .checked_sub(QUEST_GLYPH_CACHE_FIRST_ALIGNED_SLOT_INDEX)
        .ok_or_else(|| error("QUEST text encoding 동적 slot 시작이 code table 밖임"))?;
    used_by_page
        .into_iter()
        .map(|(page_id, used)| {
            let glyphs = page_storage_glyphs(&used, source_codes)?;
            if glyphs.len() > dynamic_slot_capacity {
                return Err(error(format!(
                    "QUEST text encoding page가 독립 bank slot 용량을 초과함: page={page_id}, demand={}, slots={dynamic_slot_capacity}",
                    glyphs.len()
                )));
            }
            Ok(QuestTranslationEncodingPage {
                page_id,
                glyphs: glyphs.into_iter().collect(),
            })
        })
        .collect()
}

/// 검토 선택과 route를 page별 충돌 없는 문자 공급 계획으로 만든다.
pub fn build_quest_translation_encoding_plan(
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    code_table: &QuestCodeTable,
) -> Result<QuestTranslationEncodingPlan, QuestTranslationEncodingError> {
    build_quest_translation_encoding_plan_with_supplemental_pages(
        selection,
        routing,
        code_table,
        &[],
    )
}

/// 기존 VDD page와 개발 전용 사용 문자 page를 하나의 encoding 계획으로 만든다.
pub fn build_quest_translation_encoding_plan_with_supplemental_pages(
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    code_table: &QuestCodeTable,
    supplemental_pages: &[QuestTranslationEncodingSupplementalPage],
) -> Result<QuestTranslationEncodingPlan, QuestTranslationEncodingError> {
    if selection.entries.len() != routing.routes.len() {
        return Err(error("QUEST 검토 text encoding 선택과 route 분모 불일치"));
    }
    let source_codes = source_glyph_codes(code_table)?;
    let route_by_stream = routing
        .routes
        .iter()
        .map(|route| (route.stream_id.as_str(), route))
        .collect::<BTreeMap<_, _>>();
    if route_by_stream.len() != routing.routes.len() {
        return Err(error("QUEST 검토 text encoding route stream ID 중복"));
    }
    let last_page_id = routing
        .routes
        .iter()
        .map(|route| route.page_id)
        .max()
        .ok_or_else(|| error("QUEST 검토 text encoding route가 비었음"))?;
    let mut used_by_page = (1..=last_page_id)
        .map(|page_id| (page_id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();

    for entry in &selection.entries {
        let route = route_by_stream
            .get(entry.stream_id.as_str())
            .copied()
            .ok_or_else(|| error(format!("{}: QUEST text route 누락", entry.stream_id)))?;
        if entry.batch_id != route.batch_id {
            return Err(error(format!(
                "{}: QUEST text encoding batch 결속 drift",
                entry.stream_id
            )));
        }
        match (entry.disposition, entry.selected_korean.as_deref()) {
            (
                QuestTranslationSelectionDisposition::SelectReviewedKorean
                | QuestTranslationSelectionDisposition::SelectManualComposition,
                Some(text),
            ) => {
                if route.placement == QuestTranslationSelectionPlacement::RetainSource {
                    return Err(error(format!(
                        "{}: 선택문 route가 원문 유지로 분류됨",
                        entry.stream_id
                    )));
                }
                let normalized = normalize_quest_translation_text(text)?;
                if route.selected_glyphs != Some(normalized.chars().count()) {
                    return Err(error(format!(
                        "{}: 정규화한 선택문 glyph 수와 route 불일치",
                        entry.stream_id
                    )));
                }
                used_by_page
                    .get_mut(&route.page_id)
                    .ok_or_else(|| error(format!("page {} directory 누락", route.page_id)))?
                    .extend(normalized.chars());
            }
            (QuestTranslationSelectionDisposition::RetainSource, None) => {
                if route.placement != QuestTranslationSelectionPlacement::RetainSource
                    || route.routed_record != route.protected_record
                {
                    return Err(error(format!(
                        "{}: 원문 유지 route 불일치",
                        entry.stream_id
                    )));
                }
            }
            _ => {
                return Err(error(format!(
                    "{}: disposition과 선택문 불일치",
                    entry.stream_id
                )));
            }
        }
    }

    let pages = page_encodings_with_supplemental_pages(
        used_by_page,
        supplemental_pages,
        &source_codes,
        code_table.codes().len(),
    )?;
    let repertoire = pages
        .iter()
        .flat_map(|page| page.glyphs.chars())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(QuestTranslationEncodingPlan {
        pages,
        repertoire,
        selected_routes: routing.selected,
        retained_source_routes: routing.retained_source,
        source_codes,
    })
}

#[cfg(test)]
#[path = "quest_translation_encoding_tests.rs"]
mod tests;
