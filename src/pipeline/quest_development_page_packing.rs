//! QUEST development 선택문을 실제 glyph working set 기준 page에 묶는다.
//!
//! 번역 batch ID는 검토 단위이고, OP21의 page ID는 sparse cache 선택자다.
//! 수동 조판을 넣을 때에는 두 식별자를 같은 번호로 유지할 이유가 없다.
//! 가장 큰 단일 stream 수요를 하한으로 삼아, 수동 조판 stream이 모두
//! 들어가면서 동적 glyph 중복 증가가 가장 작은 page를 결정적으로 고른다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::font::QuestCodeTable;
use crate::script::{
    QuestTranslationSelectionAnalysis, QuestTranslationSelectionDisposition,
    QUEST_SEQ_TEXT_MODE_OPCODE,
};

use super::quest_translation_encoding::{
    build_quest_translation_encoding_plan, normalize_quest_translation_text,
    quest_translation_dynamic_glyphs,
};
use super::quest_translation_selection_routing::{
    QuestTranslationSelectionPlacement, QuestTranslationSelectionRoutingAnalysis,
};

/// resident supplemental page가 뒤에 하나 더 붙을 수 있도록 남기는 최대 ID.
const QUEST_DEVELOPMENT_LAST_VDD_PAGE_ID: u8 = u8::MAX - 1;

// 지원 원본 HASI에서 두 stream은 FF03으로 끝나지만 glyph를 해제하지 않는다.
// 다음 0x0B14의 FF05 전에 page를 요청하므로 두 stream이 같은 bank를 써야 한다.
// 자연 차 전달의 all-pinned 재현과 원본 control 근거: docs/quest_playthrough.md.
const SHARED_WINDOW_STREAM_GROUPS: &[&[&str]] = &[
    // 모모모 상점 이용 뒤에는 item glyph도 살아 있다. 제안문을 닫는 응답의
    // FF05보다 page 요청이 먼저이므로 예·아니요 양쪽을 제안과 함께 둔다.
    // 자연 선택의 all-pinned 재현: docs/rom_quest_translation_runtime.md.
    &[
        "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x39C8",
        "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x3A90",
        "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x3B10",
    ],
    &[
        "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x3CE4",
        "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x3D6C",
        "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x3DB0",
    ],
    // 체리벨의 네 stream은 FF1C 선택 항목을 5·2·1·2개씩 누적한다.
    &[
        "QUEST-SEQ-TEXT-STREAM:TOWN10.SEQ:0x5290",
        "QUEST-SEQ-TEXT-STREAM:TOWN10.SEQ:0x530C",
        "QUEST-SEQ-TEXT-STREAM:TOWN10.SEQ:0x5348",
        "QUEST-SEQ-TEXT-STREAM:TOWN10.SEQ:0x5374",
    ],
    // 플럼버의 일곱 제목도 FF1D 선택 처리 전까지 함께 남는다.
    // 4C20과 4C70은 각각 두 개의 FF1C 선택 항목을 공급한다.
    &[
        "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x4C20",
        "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x4C70",
        "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x4CA8",
        "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x4CD4",
        "QUEST-SEQ-TEXT-STREAM:TOWN00.SEQ:0x4D00",
    ],
    &[
        "QUEST-SEQ-TEXT-STREAM:HASI.SEQ:0x0AA4",
        "QUEST-SEQ-TEXT-STREAM:HASI.SEQ:0x0ADC",
    ],
    // 메일 제목은 같은 선택 목록에 누적된다. 세 번째 page 대기는 앞선 제목을
    // 해제할 선택 처리에 도달하지 못한다. 원문 FF1C와 자연 접촉 재현은
    // docs/rom_analysis.md의 라그나스 포스트 실행 근거를 따른다.
    &[
        "QUEST-SEQ-TEXT-STREAM:TOWN20.SEQ:0x31B4",
        "QUEST-SEQ-TEXT-STREAM:TOWN20.SEQ:0x3208",
        "QUEST-SEQ-TEXT-STREAM:TOWN20.SEQ:0x3234",
        "QUEST-SEQ-TEXT-STREAM:TOWN20.SEQ:0x3264",
    ],
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentPagePackingAnalysis {
    pub routing: QuestTranslationSelectionRoutingAnalysis,
    pub source_pages: usize,
    pub packed_pages: usize,
    pub max_stream_dynamic_glyphs: usize,
    pub source_max_page_dynamic_glyphs: usize,
    pub packed_max_page_dynamic_glyphs: usize,
    pub source_page_assignments: usize,
    pub packed_page_assignments: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDevelopmentPagePackingError(String);

impl fmt::Display for QuestDevelopmentPagePackingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestDevelopmentPagePackingError {}

fn error(message: impl Into<String>) -> QuestDevelopmentPagePackingError {
    QuestDevelopmentPagePackingError(message.into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StreamGlyphDemand {
    route_index: usize,
    stream_id: String,
    used_glyphs: BTreeSet<char>,
    dynamic_glyphs: BTreeSet<char>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackedPage {
    used_glyphs: BTreeSet<char>,
    dynamic_glyphs: BTreeSet<char>,
    route_indices: Vec<usize>,
}

fn encoding_counts(
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    code_table: &QuestCodeTable,
) -> Result<(usize, usize), QuestDevelopmentPagePackingError> {
    let encoding = build_quest_translation_encoding_plan(selection, routing, code_table)
        .map_err(|source| error(source.to_string()))?;
    let maximum = encoding
        .pages
        .iter()
        .map(|page| page.glyphs.chars().count())
        .max()
        .unwrap_or(0);
    let assignments = encoding
        .pages
        .iter()
        .map(|page| page.glyphs.chars().count())
        .sum();
    Ok((maximum, assignments))
}

fn collect_stream_demands(
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
) -> Result<Vec<StreamGlyphDemand>, QuestDevelopmentPagePackingError> {
    if selection.entries.len() != routing.routes.len()
        || selection.selected_entries != selection.selected_manual_compositions
        || selection.selected_entries != routing.selected
        || selection.retained_source_entries != routing.retained_source
        || selection.selected_entries + selection.retained_source_entries != selection.entries.len()
        || routing.selected + routing.retained_source != routing.routes.len()
        || routing.translation_approved_routes != 0
        || routing.layout_approved_routes != 0
        || routing.product_eligible_routes != 0
    {
        return Err(error(
            "QUEST development page packing은 수동 composition과 원문 유지 route만 받음",
        ));
    }

    let route_by_stream = routing
        .routes
        .iter()
        .enumerate()
        .map(|(index, route)| (route.stream_id.as_str(), (index, route)))
        .collect::<BTreeMap<_, _>>();
    if route_by_stream.len() != routing.routes.len() {
        return Err(error("QUEST development page packing route ID 중복"));
    }

    let mut demands = Vec::with_capacity(selection.entries.len());
    for entry in &selection.entries {
        let (route_index, route) = route_by_stream
            .get(entry.stream_id.as_str())
            .copied()
            .ok_or_else(|| error(format!("{}: page packing route 누락", entry.stream_id)))?;
        let text = match (entry.disposition, entry.selected_korean.as_deref()) {
            (QuestTranslationSelectionDisposition::SelectManualComposition, Some(text)) => text,
            (QuestTranslationSelectionDisposition::RetainSource, None) => {
                if route.placement != QuestTranslationSelectionPlacement::RetainSource
                    || route.routed_record != route.protected_record
                {
                    return Err(error(format!(
                        "{}: development 원문 유지 route 결속 drift",
                        entry.stream_id
                    )));
                }
                continue;
            }
            _ => {
                return Err(error(format!(
                    "{}: development page packing 선택 경계 불일치",
                    entry.stream_id
                )));
            }
        };
        if entry.batch_id != route.batch_id
            || route.placement == QuestTranslationSelectionPlacement::RetainSource
            || route.routed_record[0] != QUEST_SEQ_TEXT_MODE_OPCODE
            || route.routed_record[1] != route.page_id
        {
            return Err(error(format!(
                "{}: development page packing route 결속 drift",
                entry.stream_id
            )));
        }
        let normalized =
            normalize_quest_translation_text(text).map_err(|source| error(source.to_string()))?;
        if route.selected_glyphs != Some(normalized.chars().count()) {
            return Err(error(format!(
                "{}: development page packing glyph 수 drift",
                entry.stream_id
            )));
        }
        let used_glyphs = normalized.chars().collect::<BTreeSet<_>>();
        let dynamic_glyphs = quest_translation_dynamic_glyphs(&used_glyphs);
        demands.push(StreamGlyphDemand {
            route_index,
            stream_id: entry.stream_id.clone(),
            used_glyphs,
            dynamic_glyphs,
        });
    }
    demands.sort_by(|left, right| {
        right
            .dynamic_glyphs
            .len()
            .cmp(&left.dynamic_glyphs.len())
            .then(right.used_glyphs.len().cmp(&left.used_glyphs.len()))
            .then(left.stream_id.cmp(&right.stream_id))
    });
    Ok(demands)
}

fn pack_stream_demands(
    demands: &[StreamGlyphDemand],
) -> Result<Vec<PackedPage>, QuestDevelopmentPagePackingError> {
    let limit = demands
        .iter()
        .map(|demand| demand.dynamic_glyphs.len())
        .max()
        .unwrap_or(0);
    let mut pages = Vec::<PackedPage>::new();
    let mut units = Vec::<PackedPage>::new();
    let mut grouped_routes = BTreeSet::new();
    for group in SHARED_WINDOW_STREAM_GROUPS {
        let members = demands
            .iter()
            .filter(|demand| group.contains(&demand.stream_id.as_str()))
            .collect::<Vec<_>>();
        if members.len() < 2 {
            continue;
        }
        let used_glyphs = members
            .iter()
            .flat_map(|demand| demand.used_glyphs.iter().copied())
            .collect::<BTreeSet<_>>();
        let dynamic_glyphs = quest_translation_dynamic_glyphs(&used_glyphs);
        if dynamic_glyphs.len() > limit {
            return Err(error(
                "QUEST 동시 표시 stream의 glyph 수요가 page 한도를 넘음",
            ));
        }
        let route_indices = members
            .iter()
            .map(|demand| demand.route_index)
            .collect::<Vec<_>>();
        for &index in &route_indices {
            if !grouped_routes.insert(index) {
                return Err(error("QUEST 동시 표시 stream group 중복"));
            }
        }
        units.push(PackedPage {
            used_glyphs,
            dynamic_glyphs,
            route_indices,
        });
    }
    units.extend(
        demands
            .iter()
            .filter(|demand| !grouped_routes.contains(&demand.route_index))
            .map(|demand| PackedPage {
                used_glyphs: demand.used_glyphs.clone(),
                dynamic_glyphs: demand.dynamic_glyphs.clone(),
                route_indices: vec![demand.route_index],
            }),
    );

    for demand in &units {
        let mut best = None::<(usize, usize, usize, BTreeSet<char>, BTreeSet<char>)>;
        for (page_index, page) in pages.iter().enumerate() {
            let mut merged_used = page.used_glyphs.clone();
            merged_used.extend(demand.used_glyphs.iter().copied());
            let merged_dynamic = quest_translation_dynamic_glyphs(&merged_used);
            if merged_dynamic.len() > limit {
                continue;
            }
            let score = (
                merged_dynamic.len() - page.dynamic_glyphs.len(),
                merged_dynamic.len(),
                page_index,
            );
            if best
                .as_ref()
                .is_none_or(|current| score < (current.0, current.1, current.2))
            {
                best = Some((score.0, score.1, page_index, merged_used, merged_dynamic));
            }
        }

        if let Some((_, _, page_index, merged_used, merged_dynamic)) = best {
            let page = &mut pages[page_index];
            page.used_glyphs = merged_used;
            page.dynamic_glyphs = merged_dynamic;
            page.route_indices.extend_from_slice(&demand.route_indices);
        } else {
            pages.push(PackedPage {
                used_glyphs: demand.used_glyphs.clone(),
                dynamic_glyphs: demand.dynamic_glyphs.clone(),
                route_indices: demand.route_indices.clone(),
            });
        }
    }
    Ok(pages)
}

/// 수동 development 선택문의 route page를 단일 stream 최대 수요 안에서 묶는다.
pub fn pack_quest_development_translation_pages(
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    code_table: &QuestCodeTable,
) -> Result<QuestDevelopmentPagePackingAnalysis, QuestDevelopmentPagePackingError> {
    let (source_maximum, source_assignments) = encoding_counts(selection, routing, code_table)?;
    let demands = collect_stream_demands(selection, routing)?;
    let max_stream_dynamic_glyphs = demands
        .iter()
        .map(|demand| demand.dynamic_glyphs.len())
        .max()
        .unwrap_or(0);
    let pages = pack_stream_demands(&demands)?;
    if pages.is_empty()
        || pages.len() > usize::from(QUEST_DEVELOPMENT_LAST_VDD_PAGE_ID)
        || pages.iter().any(|page| page.route_indices.is_empty())
    {
        return Err(error(
            "QUEST development page packing이 resident용 ID를 남기지 못함",
        ));
    }

    let mut packed_routing = routing.clone();
    let mut assigned_routes = BTreeSet::new();
    for (page_index, page) in pages.iter().enumerate() {
        let page_id = u8::try_from(page_index + 1)
            .map_err(|_| error("QUEST development packed page ID 변환 실패"))?;
        for &route_index in &page.route_indices {
            if !assigned_routes.insert(route_index) {
                return Err(error(
                    "QUEST development packed route가 둘 이상 page에 배정됨",
                ));
            }
            let route = packed_routing
                .routes
                .get_mut(route_index)
                .ok_or_else(|| error("QUEST development packed route index 이탈"))?;
            route.page_id = page_id;
            route.routed_record[1] = page_id;
        }
    }
    if assigned_routes.len() != routing.selected {
        return Err(error("QUEST development packed route 배정 누락"));
    }
    packed_routing.selected_pages = pages.len();

    let (packed_maximum, packed_assignments) =
        encoding_counts(selection, &packed_routing, code_table)?;
    let route_pages = packed_routing
        .routes
        .iter()
        .filter(|route| route.selected_glyphs.is_some())
        .map(|route| route.page_id)
        .collect::<BTreeSet<_>>();
    let last_page_id = u8::try_from(pages.len())
        .map_err(|_| error("QUEST development packed page 수 변환 실패"))?;
    if !route_pages.iter().copied().eq(1..=last_page_id)
        || packed_maximum > max_stream_dynamic_glyphs
        || packed_routing.selected != routing.selected
        || packed_routing.retained_source != routing.retained_source
        || packed_routing.inline != routing.inline
        || packed_routing.relocated != routing.relocated
        || packed_routing.known_relocated_bytes != routing.known_relocated_bytes
        || packed_routing.sequences != routing.sequences
    {
        return Err(error("QUEST development packed route 최종 결속 drift"));
    }

    Ok(QuestDevelopmentPagePackingAnalysis {
        routing: packed_routing,
        source_pages: routing.selected_pages,
        packed_pages: pages.len(),
        max_stream_dynamic_glyphs,
        source_max_page_dynamic_glyphs: source_maximum,
        packed_max_page_dynamic_glyphs: packed_maximum,
        source_page_assignments: source_assignments,
        packed_page_assignments: packed_assignments,
    })
}

#[cfg(test)]
#[path = "quest_development_page_packing_tests.rs"]
mod tests;
