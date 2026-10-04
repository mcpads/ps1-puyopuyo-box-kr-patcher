//! QUEST 번역 선택을 원위치·SEQ 꼬리 route에 결속한다.
//!
//! 선택된 검토문만 `[21 page target]` record 후보로 만들고 사람 결정이
//! 남은 행은 canonical 원본 record를 유지한다. 검토안 scalar 수와
//! 원본 control byte를 기준으로 저장 용량만 판정하며 줄 배치·번역 승인·
//! 제품 적격을 대신하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use crate::script::{
    validate_quest_explicit_layout, QuestManualStreamLayout, QuestSeqExtractedTextStream,
    QuestSeqRuntimeBaseEvidence, QuestSeqTextDocumentAnalysis, QuestSeqTextDocumentChunk,
    QuestTranslationIndependentReviewStatus, QuestTranslationSelectionAnalysis,
    QuestTranslationSelectionDisposition, QUEST_SEQ_ACTIVE_RUNTIME_BASE,
};

use super::quest_compact_return::QUEST_COMPACT_RETURN_BYTES;
use super::quest_translation_routing::{
    align4, projected_stream_body_bytes, projected_stream_bytes, routed_record,
    QuestTranslationRoutingAnalysis, QUEST_SEQ_ACTIVE_WINDOW_BYTES,
};

pub const QUEST_TRANSLATION_SELECTION_ROUTING_SCHEMA_VERSION: &str =
    "quest-translation-selection-routing-v1";

/// 검토안 선택 하나의 저장 판정.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestTranslationSelectionPlacement {
    InlineCapacityCandidate,
    RelocatedTailCapacityCandidate,
    RetainSource,
}

impl QuestTranslationSelectionPlacement {
    pub const fn label(self) -> &'static str {
        match self {
            Self::InlineCapacityCandidate => "inline-capacity-candidate",
            Self::RelocatedTailCapacityCandidate => "relocated-tail-capacity-candidate",
            Self::RetainSource => "retain-source",
        }
    }
}

/// 검토안 선택과 원본 OP21 record를 결속한 route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationSelectionRoute {
    pub sequence: String,
    pub stream_id: String,
    pub batch_id: String,
    pub review_status: QuestTranslationIndependentReviewStatus,
    pub placement: QuestTranslationSelectionPlacement,
    pub page_id: u8,
    pub record_offset: usize,
    pub end_offset: usize,
    pub protected_record: [u8; 4],
    pub routed_record: [u8; 4],
    pub source_stream_bytes: usize,
    pub selected_glyphs: Option<usize>,
    pub projected_stream_bytes: Option<usize>,
    pub relocated_stream_offset: Option<usize>,
    pub return_runtime: Option<u32>,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub product_eligible: bool,
}

/// 한 SEQ의 선택된 검토안 꼬리 수용량.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationSelectionSequenceCapacity {
    pub sequence: String,
    pub runtime_base_evidence: QuestSeqRuntimeBaseEvidence,
    pub source_bytes: usize,
    pub routes: usize,
    pub selected: usize,
    pub retained_source: usize,
    pub inline: usize,
    pub relocated: usize,
    pub tail_start: usize,
    pub known_tail_end: usize,
    pub known_relocated_bytes: usize,
    pub active_window_remaining: usize,
    pub known_tail_fits: bool,
}

/// 580개 검토안 선택을 원본 위치 보존형 route에 결속한 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationSelectionRoutingAnalysis {
    pub routes: Vec<QuestTranslationSelectionRoute>,
    pub sequences: Vec<QuestTranslationSelectionSequenceCapacity>,
    pub selected: usize,
    pub retained_source: usize,
    pub selected_pages: usize,
    pub inline: usize,
    pub relocated: usize,
    pub known_relocated_bytes: usize,
    pub minimum_active_window_remaining: usize,
    pub runtime_base_observed_sequences: usize,
    pub static_loader_runtime_base_sequences: usize,
    pub runtime_base_bindings_proven: bool,
    pub translation_approved_routes: usize,
    pub layout_approved_routes: usize,
    pub product_eligible_routes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationSelectionRoutingError(String);

impl fmt::Display for QuestTranslationSelectionRoutingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestTranslationSelectionRoutingError {}

fn error(message: impl Into<String>) -> QuestTranslationSelectionRoutingError {
    QuestTranslationSelectionRoutingError(message.into())
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn selected_translation_placement(
    disposition: QuestTranslationSelectionDisposition,
    projected_bytes: usize,
    source_bytes: usize,
    selected_glyphs: usize,
    source_glyphs: usize,
    manual_layout_fits_inline: bool,
) -> QuestTranslationSelectionPlacement {
    let fits_inline =
        if disposition == QuestTranslationSelectionDisposition::SelectManualComposition {
            manual_layout_fits_inline
        } else {
            projected_bytes <= source_bytes && selected_glyphs <= source_glyphs
        };
    if fits_inline {
        QuestTranslationSelectionPlacement::InlineCapacityCandidate
    } else {
        QuestTranslationSelectionPlacement::RelocatedTailCapacityCandidate
    }
}

fn extracted_streams(
    documents: &QuestSeqTextDocumentAnalysis,
) -> Result<BTreeMap<String, &QuestSeqExtractedTextStream>, QuestTranslationSelectionRoutingError> {
    let mut streams = BTreeMap::new();
    for document in &documents.documents {
        for chunk in &document.chunks {
            if let QuestSeqTextDocumentChunk::TextStream(stream) = chunk {
                let id = stream.id();
                if streams.insert(id.clone(), stream).is_some() {
                    return Err(error(format!(
                        "QUEST 검토안 route 추출 stream ID 중복: {id}"
                    )));
                }
            }
        }
    }
    Ok(streams)
}

/// 번역 선택과 수동 run layout을 기존 원본 route·문서에 결합해 저장 수요를 센다.
pub fn audit_quest_translation_selection_routing_with_layouts(
    documents: &QuestSeqTextDocumentAnalysis,
    source_routing: &QuestTranslationRoutingAnalysis,
    selection: &QuestTranslationSelectionAnalysis,
    explicit_layouts: &[QuestManualStreamLayout],
) -> Result<QuestTranslationSelectionRoutingAnalysis, QuestTranslationSelectionRoutingError> {
    if source_routing.routes.len() != selection.entries.len()
        || selection.selected_manual_compositions != explicit_layouts.len()
    {
        return Err(error(
            "QUEST 검토안 선택·원본 route·수동 layout 분모 불일치",
        ));
    }
    let streams = extracted_streams(documents)?;
    let selected_by_id = selection
        .entries
        .iter()
        .map(|entry| (entry.stream_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    if selected_by_id.len() != selection.entries.len() {
        return Err(error("QUEST 검토안 선택 stream ID 중복"));
    }
    let mut explicit_layout_by_stream = BTreeMap::new();
    for layout in explicit_layouts {
        if explicit_layout_by_stream
            .insert(layout.stream_id.as_str(), layout)
            .is_some()
        {
            return Err(error(format!(
                "QUEST 검토안 route 수동 layout stream ID 중복: {}",
                layout.stream_id
            )));
        }
    }

    let mut routes = Vec::with_capacity(source_routing.routes.len());
    for source_route in &source_routing.routes {
        let selected = selected_by_id
            .get(source_route.stream_id.as_str())
            .copied()
            .ok_or_else(|| {
                error(format!(
                    "{}: QUEST 검토안 선택 누락",
                    source_route.stream_id
                ))
            })?;
        let stream = streams
            .get(&source_route.stream_id)
            .copied()
            .ok_or_else(|| {
                error(format!(
                    "{}: QUEST 검토안 route 추출 stream 누락",
                    source_route.stream_id
                ))
            })?;
        if selected.batch_id != source_route.batch_id
            || stream.sequence != source_route.sequence
            || stream.record_offset != source_route.record_offset
            || stream.end_offset != source_route.end_offset
            || stream.record_raw != source_route.protected_record
        {
            return Err(error(format!(
                "{}: QUEST 검토안 선택·원본 route·문서 결속 drift",
                source_route.stream_id
            )));
        }
        let expected_disposition = match selected.review_status {
            QuestTranslationIndependentReviewStatus::ReviewAgrees
            | QuestTranslationIndependentReviewStatus::ReviewProposesChange => {
                QuestTranslationSelectionDisposition::SelectReviewedKorean
            }
            QuestTranslationIndependentReviewStatus::NeedsHumanDecision => {
                QuestTranslationSelectionDisposition::RetainSource
            }
        };
        if selected.disposition != expected_disposition
            && selected.disposition != QuestTranslationSelectionDisposition::SelectManualComposition
            && selected.disposition != QuestTranslationSelectionDisposition::RetainSource
        {
            return Err(error(format!(
                "{}: QUEST 검토 상태와 번역 선택 정책 불일치",
                source_route.stream_id
            )));
        }

        let (placement, selected_glyphs, projected, routed) =
            match (selected.disposition, selected.selected_korean.as_deref()) {
                (
                    QuestTranslationSelectionDisposition::SelectReviewedKorean
                    | QuestTranslationSelectionDisposition::SelectManualComposition,
                    Some(korean),
                ) => {
                    let glyphs = korean.chars().count();
                    let source_glyphs = stream
                        .tokens
                        .iter()
                        .filter(|token| {
                            token.kind == crate::script::QuestSeqExtractedTokenKind::Glyph
                        })
                        .count();
                    let projected = projected_stream_bytes(stream, stream.record_offset, glyphs)
                        .map_err(|source| error(source.to_string()))?;
                    let manual_layout_fits_inline = if selected.disposition
                        == QuestTranslationSelectionDisposition::SelectManualComposition
                    {
                        let layout = explicit_layout_by_stream
                            .remove(selected.stream_id.as_str())
                            .ok_or_else(|| {
                                error(format!(
                                    "{}: 수동 선택문 route layout 누락",
                                    selected.stream_id
                                ))
                            })?;
                        if layout.selected_korean != korean {
                            return Err(error(format!(
                                "{}: 수동 선택문 route text 결속 drift",
                                selected.stream_id
                            )));
                        }
                        let plan = validate_quest_explicit_layout(
                            stream,
                            &layout.candidate_glyph_run_lengths,
                        )
                        .map_err(|source| error(source.to_string()))?;
                        plan.candidate_glyph_run_lengths
                            .iter()
                            .zip(&plan.source_glyph_run_lengths)
                            .all(|(candidate, source)| candidate <= source)
                    } else {
                        false
                    };
                    (
                        selected_translation_placement(
                            selected.disposition,
                            projected,
                            source_route.source_stream_bytes,
                            glyphs,
                            source_glyphs,
                            manual_layout_fits_inline,
                        ),
                        Some(glyphs),
                        Some(projected),
                        routed_record(stream, source_route.page_id, None)
                            .map_err(|source| error(source.to_string()))?,
                    )
                }
                (QuestTranslationSelectionDisposition::RetainSource, None) => (
                    QuestTranslationSelectionPlacement::RetainSource,
                    None,
                    None,
                    source_route.protected_record,
                ),
                _ => {
                    return Err(error(format!(
                        "{}: QUEST 검토안 disposition과 선택문 불일치",
                        source_route.stream_id
                    )));
                }
            };
        routes.push(QuestTranslationSelectionRoute {
            sequence: source_route.sequence.clone(),
            stream_id: source_route.stream_id.clone(),
            batch_id: source_route.batch_id.clone(),
            review_status: selected.review_status,
            placement,
            page_id: source_route.page_id,
            record_offset: source_route.record_offset,
            end_offset: source_route.end_offset,
            protected_record: source_route.protected_record,
            routed_record: routed,
            source_stream_bytes: source_route.source_stream_bytes,
            selected_glyphs,
            projected_stream_bytes: projected,
            relocated_stream_offset: None,
            return_runtime: None,
            translation_approved: false,
            layout_approved: false,
            product_eligible: false,
        });
    }
    if !explicit_layout_by_stream.is_empty() {
        return Err(error(format!(
            "QUEST 검토안 route 수동 layout 소비 누락: {}",
            explicit_layout_by_stream
                .keys()
                .copied()
                .collect::<Vec<_>>()
                .join(",")
        )));
    }
    routes.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then(left.record_offset.cmp(&right.record_offset))
    });

    let source_sequence_by_name = source_routing
        .sequences
        .iter()
        .map(|sequence| (sequence.sequence.as_str(), sequence))
        .collect::<BTreeMap<_, _>>();
    let mut sequences = Vec::with_capacity(source_routing.sequences.len());
    for sequence_name in routes
        .iter()
        .map(|route| route.sequence.clone())
        .collect::<BTreeSet<_>>()
    {
        let source_sequence = source_sequence_by_name
            .get(sequence_name.as_str())
            .copied()
            .ok_or_else(|| error(format!("{sequence_name}: 원본 route sequence 누락")))?;
        let tail_start =
            align4(source_sequence.source_bytes).map_err(|source| error(source.to_string()))?;
        if tail_start != source_sequence.tail_start {
            return Err(error(format!(
                "{sequence_name}: 원본 route tail 시작점 drift"
            )));
        }
        let mut cursor = tail_start;
        for route in routes
            .iter_mut()
            .filter(|route| route.sequence == sequence_name)
            .filter(|route| {
                route.placement
                    == QuestTranslationSelectionPlacement::RelocatedTailCapacityCandidate
            })
        {
            cursor = align4(cursor).map_err(|source| error(source.to_string()))?;
            let stream = streams
                .get(&route.stream_id)
                .copied()
                .expect("검증한 route의 추출 stream");
            let relocated_bytes = projected_stream_body_bytes(
                stream,
                cursor,
                route
                    .selected_glyphs
                    .expect("relocated route에는 선택한 glyph 수가 있음"),
            )
            .map_err(|source| error(source.to_string()))?;
            if Some(
                relocated_bytes
                    .checked_add(4)
                    .ok_or_else(|| error(format!("{}: route 길이 overflow", route.stream_id)))?,
            ) != route.projected_stream_bytes
            {
                return Err(error(format!(
                    "{}: QUEST 검토안 route 정렬 위치에 따른 길이 drift",
                    route.stream_id
                )));
            }
            route.relocated_stream_offset = Some(cursor);
            route.routed_record = routed_record(stream, route.page_id, Some(cursor))
                .map_err(|source| error(source.to_string()))?;
            route.return_runtime = Some(
                QUEST_SEQ_ACTIVE_RUNTIME_BASE
                    .checked_add(u32::try_from(route.end_offset).map_err(|_| {
                        error(format!("{}: 복귀 offset이 u32를 넘음", route.stream_id))
                    })?)
                    .ok_or_else(|| error(format!("{}: 복귀 주소 overflow", route.stream_id)))?,
            );
            cursor = cursor
                .checked_add(relocated_bytes)
                .and_then(|value| value.checked_add(QUEST_COMPACT_RETURN_BYTES))
                .ok_or_else(|| error(format!("{}: tail 배치 overflow", route.stream_id)))?;
        }

        let sequence_routes = routes
            .iter()
            .filter(|route| route.sequence == sequence_name)
            .collect::<Vec<_>>();
        let selected = sequence_routes
            .iter()
            .filter(|route| route.selected_glyphs.is_some())
            .count();
        let retained_source = sequence_routes.len() - selected;
        let inline = sequence_routes
            .iter()
            .filter(|route| {
                route.placement == QuestTranslationSelectionPlacement::InlineCapacityCandidate
            })
            .count();
        let relocated = sequence_routes
            .iter()
            .filter(|route| {
                route.placement
                    == QuestTranslationSelectionPlacement::RelocatedTailCapacityCandidate
            })
            .count();
        sequences.push(QuestTranslationSelectionSequenceCapacity {
            sequence: sequence_name,
            runtime_base_evidence: source_sequence.runtime_base_evidence,
            source_bytes: source_sequence.source_bytes,
            routes: sequence_routes.len(),
            selected,
            retained_source,
            inline,
            relocated,
            tail_start,
            known_tail_end: cursor,
            known_relocated_bytes: cursor - tail_start,
            active_window_remaining: QUEST_SEQ_ACTIVE_WINDOW_BYTES.saturating_sub(cursor),
            known_tail_fits: cursor <= QUEST_SEQ_ACTIVE_WINDOW_BYTES,
        });
    }

    let selected = routes
        .iter()
        .filter(|route| route.selected_glyphs.is_some())
        .count();
    let retained_source = routes.len() - selected;
    let selected_pages = routes
        .iter()
        .filter(|route| route.selected_glyphs.is_some())
        .map(|route| route.page_id)
        .collect::<BTreeSet<_>>()
        .len();
    let inline = routes
        .iter()
        .filter(|route| {
            route.placement == QuestTranslationSelectionPlacement::InlineCapacityCandidate
        })
        .count();
    let relocated = routes
        .iter()
        .filter(|route| {
            route.placement == QuestTranslationSelectionPlacement::RelocatedTailCapacityCandidate
        })
        .count();
    let known_relocated_bytes = sequences
        .iter()
        .map(|sequence| sequence.known_relocated_bytes)
        .sum();
    let minimum_active_window_remaining = sequences
        .iter()
        .map(|sequence| sequence.active_window_remaining)
        .min()
        .unwrap_or(0);
    let runtime_base_observed_sequences = sequences
        .iter()
        .filter(|sequence| sequence.runtime_base_evidence.runtime_observed())
        .count();
    let static_loader_runtime_base_sequences = sequences
        .iter()
        .filter(|sequence| {
            sequence.runtime_base_evidence == QuestSeqRuntimeBaseEvidence::StaticCommonLoader
        })
        .count();
    let runtime_base_bindings_proven = sequences
        .iter()
        .all(|sequence| sequence.runtime_base_evidence.base_binding_proven());
    Ok(QuestTranslationSelectionRoutingAnalysis {
        routes,
        sequences,
        selected,
        retained_source,
        selected_pages,
        inline,
        relocated,
        known_relocated_bytes,
        minimum_active_window_remaining,
        runtime_base_observed_sequences,
        static_loader_runtime_base_sequences,
        runtime_base_bindings_proven,
        translation_approved_routes: 0,
        layout_approved_routes: 0,
        product_eligible_routes: 0,
    })
}

/// 수동 선택이 없는 번역 검토 상태의 저장 수요를 센다.
pub fn audit_quest_translation_selection_routing(
    documents: &QuestSeqTextDocumentAnalysis,
    source_routing: &QuestTranslationRoutingAnalysis,
    selection: &QuestTranslationSelectionAnalysis,
) -> Result<QuestTranslationSelectionRoutingAnalysis, QuestTranslationSelectionRoutingError> {
    audit_quest_translation_selection_routing_with_layouts(
        documents,
        source_routing,
        selection,
        &[],
    )
}

/// 검토안 선택과 실제 route metadata를 전수 대조하는 TSV.
pub fn render_quest_translation_selection_routing_tsv(
    analysis: &QuestTranslationSelectionRoutingAnalysis,
) -> String {
    let sequence_by_name = analysis
        .sequences
        .iter()
        .map(|sequence| (sequence.sequence.as_str(), sequence))
        .collect::<BTreeMap<_, _>>();
    let mut output = String::from(
        "schema_version\tsequence\truntime_base_evidence\tstream_id\tbatch_id\treview_status\tplacement\tpage_id\t\
         record_offset\tend_offset\tprotected_record\trouted_record\tsource_stream_bytes\t\
         selected_glyphs\tprojected_stream_bytes\trelocated_stream_offset\treturn_runtime\t\
         tail_start\tknown_tail_end\tactive_window_remaining\tknown_tail_fits\t\
         translation_approved\tlayout_approved\tproduct_eligible\n",
    );
    for route in &analysis.routes {
        let sequence = sequence_by_name
            .get(route.sequence.as_str())
            .expect("route의 sequence capacity가 있음");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t0x{:04X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t0x{:04X}\t{}\t{}\tno\tno\tno",
            QUEST_TRANSLATION_SELECTION_ROUTING_SCHEMA_VERSION,
            route.sequence,
            sequence.runtime_base_evidence.label(),
            route.stream_id,
            escape_tsv(&route.batch_id),
            route.review_status.label(),
            route.placement.label(),
            route.page_id,
            route.record_offset,
            route.end_offset,
            hex(&route.protected_record),
            hex(&route.routed_record),
            route.source_stream_bytes,
            route
                .selected_glyphs
                .map_or_else(String::new, |value| value.to_string()),
            route
                .projected_stream_bytes
                .map_or_else(String::new, |value| value.to_string()),
            route
                .relocated_stream_offset
                .map_or_else(String::new, |value| format!("0x{value:04X}")),
            route
                .return_runtime
                .map_or_else(String::new, |value| format!("0x{value:08X}")),
            sequence.tail_start,
            sequence.known_tail_end,
            sequence.active_window_remaining,
            sequence.known_tail_fits,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

/// 지원 원본의 검토안 route 분모·수용량과 파생 합계를 검증한다.
pub fn verify_box_quest_translation_selection_routing(
    documents: &QuestSeqTextDocumentAnalysis,
    source_routing: &QuestTranslationRoutingAnalysis,
    selection: &QuestTranslationSelectionAnalysis,
) -> Result<QuestTranslationSelectionRoutingAnalysis, QuestTranslationSelectionRoutingError> {
    verify_box_quest_translation_selection_routing_with_layouts(
        documents,
        source_routing,
        selection,
        &[],
    )
}

/// 지원 원본의 route 분모·수용량과 수동 run별 inline 경계를 검증한다.
pub fn verify_box_quest_translation_selection_routing_with_layouts(
    documents: &QuestSeqTextDocumentAnalysis,
    source_routing: &QuestTranslationRoutingAnalysis,
    selection: &QuestTranslationSelectionAnalysis,
    explicit_layouts: &[QuestManualStreamLayout],
) -> Result<QuestTranslationSelectionRoutingAnalysis, QuestTranslationSelectionRoutingError> {
    let analysis = audit_quest_translation_selection_routing_with_layouts(
        documents,
        source_routing,
        selection,
        explicit_layouts,
    )?;
    let sequence_routes = analysis
        .sequences
        .iter()
        .map(|sequence| sequence.routes)
        .sum::<usize>();
    let sequence_selected = analysis
        .sequences
        .iter()
        .map(|sequence| sequence.selected)
        .sum::<usize>();
    let sequence_retained = analysis
        .sequences
        .iter()
        .map(|sequence| sequence.retained_source)
        .sum::<usize>();
    let sequence_inline = analysis
        .sequences
        .iter()
        .map(|sequence| sequence.inline)
        .sum::<usize>();
    let sequence_relocated = analysis
        .sequences
        .iter()
        .map(|sequence| sequence.relocated)
        .sum::<usize>();
    let sequence_relocated_bytes = analysis
        .sequences
        .iter()
        .map(|sequence| sequence.known_relocated_bytes)
        .sum::<usize>();
    let observed_runtime_bases = analysis
        .sequences
        .iter()
        .filter(|sequence| sequence.runtime_base_evidence.runtime_observed())
        .count();
    let static_loader_runtime_bases = analysis
        .sequences
        .iter()
        .filter(|sequence| {
            sequence.runtime_base_evidence == QuestSeqRuntimeBaseEvidence::StaticCommonLoader
        })
        .count();
    if analysis.routes.len() != source_routing.routes.len()
        || analysis.routes.len() != selection.entries.len()
        || analysis.selected != selection.selected_entries
        || analysis.retained_source != selection.retained_source_entries
        || analysis.selected_pages != selection.selected_batches
        || analysis.inline + analysis.relocated != analysis.selected
        || analysis.sequences.len() != source_routing.sequences.len()
        || sequence_routes != analysis.routes.len()
        || sequence_selected != analysis.selected
        || sequence_retained != analysis.retained_source
        || sequence_inline != analysis.inline
        || sequence_relocated != analysis.relocated
        || sequence_relocated_bytes != analysis.known_relocated_bytes
        || analysis.runtime_base_observed_sequences != observed_runtime_bases
        || analysis.static_loader_runtime_base_sequences != static_loader_runtime_bases
        || analysis.runtime_base_observed_sequences + analysis.static_loader_runtime_base_sequences
            != analysis.sequences.len()
        || analysis.runtime_base_bindings_proven
            != analysis
                .sequences
                .iter()
                .all(|sequence| sequence.runtime_base_evidence.base_binding_proven())
        || analysis.runtime_base_observed_sequences
            != source_routing.runtime_base_observed_sequences
        || analysis.static_loader_runtime_base_sequences
            != source_routing.static_loader_runtime_base_sequences
        || analysis.runtime_base_bindings_proven != source_routing.runtime_base_bindings_proven
        || analysis.sequences.iter().any(|sequence| {
            !sequence.known_tail_fits
                || sequence.known_tail_end > QUEST_SEQ_ACTIVE_WINDOW_BYTES
                || sequence.selected + sequence.retained_source != sequence.routes
                || sequence.inline + sequence.relocated != sequence.selected
        })
        || analysis.routes.iter().any(|route| {
            route.placement == QuestTranslationSelectionPlacement::RetainSource
                && route.routed_record != route.protected_record
        })
        || analysis.translation_approved_routes != 0
        || analysis.layout_approved_routes != 0
        || analysis.product_eligible_routes != 0
    {
        let invalid_sequences = analysis
            .sequences
            .iter()
            .filter(|sequence| {
                !sequence.known_tail_fits
                    || sequence.known_tail_end > QUEST_SEQ_ACTIVE_WINDOW_BYTES
                    || sequence.selected + sequence.retained_source != sequence.routes
                    || sequence.inline + sequence.relocated != sequence.selected
            })
            .map(|sequence| {
                format!(
                    "{}: routes={}, selected={}, retained={}, inline={}, relocated={}, tail_end={}, tail_fits={}",
                    sequence.sequence,
                    sequence.routes,
                    sequence.selected,
                    sequence.retained_source,
                    sequence.inline,
                    sequence.relocated,
                    sequence.known_tail_end,
                    sequence.known_tail_fits
                )
            })
            .collect::<Vec<_>>();
        return Err(error(format!(
            "QUEST 검토안 route 파생 분모·수용량 불일치: routes={}/{}/{}, selected={}/{}, retained={}/{}, pages={}/{}, sequences=[{}]",
            analysis.routes.len(),
            source_routing.routes.len(),
            selection.entries.len(),
            analysis.selected,
            selection.selected_entries,
            analysis.retained_source,
            selection.retained_source_entries,
            analysis.selected_pages,
            selection.selected_batches,
            invalid_sequences.join("; ")
        )));
    }
    Ok(analysis)
}

#[cfg(test)]
#[path = "quest_translation_selection_routing_tests.rs"]
mod tests;
