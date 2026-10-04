//! 사람이 조판한 QUEST 번역을 실제 SEQ stream byte로 만드는 개발 writer.
//!
//! 사람이 작성한 명시적 glyph run만 소비한다. 수동 조판이 없는 stream은
//! 원문을 유지하며, 검토문이나 번역 후보를 자동으로 행·page에 배분하지
//! 않는다. in-place 본문은 원본 control offset과 전체 span을 보존하고, tail로
//! 보낸 원본 본문, 원문 유지 stream, opaque chunk와 기존 내부주소 field는
//! 모두 byte-identical이어야 한다.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::font::QuestCodeTable;
use crate::script::{
    serialize_quest_seq_text_document, QuestSeqAddressAnalysis, QuestSeqTextDocumentAnalysis,
    QuestTranslationSelectionAnalysis, QuestTranslationSelectionDisposition,
};

use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_translation_encoding::build_quest_translation_encoding_plan;
use super::quest_translation_routing::QUEST_SEQ_ACTIVE_WINDOW_BYTES;
use super::quest_translation_selection_routing::{
    QuestTranslationSelectionPlacement, QuestTranslationSelectionRoutingAnalysis,
};

mod body_serialization;
mod glyph_code_supply;
mod model;
mod report;
mod source_preservation;

pub use crate::script::QuestManualStreamLayout;
use body_serialization::{
    return_record, serialize_explicit_body,
    serialize_explicit_inline_body_preserving_source_offsets,
};
pub(super) use glyph_code_supply::{encode_quest_page_text, quest_page_code_maps};
#[cfg(test)]
use glyph_code_supply::{verify_quest_repertoire_cells, verify_source_atlas_cell};
use model::error;
pub use model::{
    QuestManualSequenceWrite, QuestManualStreamCodeSupply, QuestManualStreamWriterAnalysis,
    QuestManualStreamWriterError, QUEST_MANUAL_STREAM_WRITER_SCHEMA_VERSION,
};
pub use report::render_quest_manual_stream_writer_tsv;
use source_preservation::{stream_map, verify_preserved_ranges, write_exact};

fn sha256(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

/// 외부에서 결속한 encoding과 수동 조판을 실제 SEQ byte로 직렬화한다.
pub fn audit_quest_manual_stream_writer_with_code_supply_and_layouts(
    documents: &QuestSeqTextDocumentAnalysis,
    addresses: &QuestSeqAddressAnalysis,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    code_supply: QuestManualStreamCodeSupply<'_>,
    explicit_layouts: &[QuestManualStreamLayout],
) -> Result<QuestManualStreamWriterAnalysis, QuestManualStreamWriterError> {
    if documents.documents.len() != addresses.summaries.len()
        || selection.entries.len() != routing.routes.len()
        || selection.selected_manual_compositions != explicit_layouts.len()
    {
        return Err(error(
            "QUEST writer 문서·주소·선택·route·수동 layout 분모 불일치",
        ));
    }
    let streams = stream_map(documents)?;
    let selections = selection
        .entries
        .iter()
        .map(|entry| (entry.stream_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    if selections.len() != selection.entries.len() {
        return Err(error("QUEST writer 선택 stream ID 중복"));
    }
    let mut explicit_layout_by_stream = BTreeMap::new();
    for layout in explicit_layouts {
        if explicit_layout_by_stream
            .insert(layout.stream_id.as_str(), layout)
            .is_some()
        {
            return Err(error(format!(
                "QUEST writer 명시적 layout stream ID 중복: {}",
                layout.stream_id
            )));
        }
        let selected = selections.get(layout.stream_id.as_str()).ok_or_else(|| {
            error(format!(
                "QUEST writer 명시적 layout의 선택 stream이 없음: {}",
                layout.stream_id
            ))
        })?;
        if selected.disposition != QuestTranslationSelectionDisposition::SelectManualComposition
            || selected.selected_korean.as_deref() != Some(layout.selected_korean.as_str())
        {
            return Err(error(format!(
                "QUEST writer 명시적 layout과 수동 선택문 결속 불일치: {}",
                layout.stream_id
            )));
        }
    }
    let code_table = code_supply.code_table;
    let page_codes = quest_page_code_maps(
        code_supply.encoding,
        code_supply.package,
        code_supply.code_table,
    )?;
    let sequence_capacity = routing
        .sequences
        .iter()
        .map(|sequence| (sequence.sequence.as_str(), sequence))
        .collect::<BTreeMap<_, _>>();

    let mut sequences = Vec::with_capacity(documents.documents.len());
    let mut retained_source_exact = true;
    let mut opaque_exact = true;
    let mut internal_addresses_exact = true;
    let mut non_target_exact = true;
    let mut explicit_layout_routes = 0usize;
    for document in &documents.documents {
        let source = serialize_quest_seq_text_document(document)
            .map_err(|source| error(source.to_string()))?;
        let routes = routing
            .routes
            .iter()
            .filter(|route| route.sequence == document.sequence)
            .collect::<Vec<_>>();
        let mut output = source.clone();
        let mut claimed = vec![false; source.len()];
        let mut selected_routes = 0usize;
        let mut retained_source_routes = 0usize;
        let mut inline_routes = 0usize;
        let mut relocated_routes = 0usize;

        for route in &routes {
            let selected = selections
                .get(route.stream_id.as_str())
                .copied()
                .ok_or_else(|| error(format!("{}: writer 선택 누락", route.stream_id)))?;
            let stream = streams
                .get(&route.stream_id)
                .copied()
                .ok_or_else(|| error(format!("{}: writer stream 누락", route.stream_id)))?;
            if selected.batch_id != route.batch_id
                || stream.sequence != route.sequence
                || stream.record_offset != route.record_offset
                || stream.end_offset != route.end_offset
            {
                return Err(error(format!(
                    "{}: writer 입력 결속 drift",
                    route.stream_id
                )));
            }
            match (selected.disposition, selected.selected_korean.as_deref()) {
                (QuestTranslationSelectionDisposition::RetainSource, None) => {
                    if route.placement != QuestTranslationSelectionPlacement::RetainSource
                        || route.routed_record != route.protected_record
                    {
                        return Err(error(format!(
                            "{}: writer 원문 유지 route drift",
                            route.stream_id
                        )));
                    }
                    retained_source_routes += 1;
                }
                (QuestTranslationSelectionDisposition::SelectManualComposition, Some(text)) => {
                    selected_routes += 1;
                    let explicit_layout = explicit_layout_by_stream
                        .remove(route.stream_id.as_str())
                        .ok_or_else(|| {
                            error(format!(
                                "{}: 수동 선택문에 명시적 layout이 없음",
                                route.stream_id
                            ))
                        })?;
                    explicit_layout_routes += 1;
                    let glyph_codes =
                        encode_quest_page_text(text, route.page_id, &page_codes, code_table)?;
                    if route.selected_glyphs != Some(glyph_codes.len()) {
                        return Err(error(format!(
                            "{}: writer 선택문 glyph 수 drift",
                            route.stream_id
                        )));
                    }
                    match route.placement {
                        QuestTranslationSelectionPlacement::InlineCapacityCandidate => {
                            inline_routes += 1;
                            let padding_codes = encode_quest_page_text(
                                "　",
                                route.page_id,
                                &page_codes,
                                code_table,
                            )?;
                            let padding_code = match padding_codes.as_slice() {
                                [padding_code] => *padding_code,
                                _ => {
                                    return Err(error(format!(
                                        "{}: inline padding code 분모 불일치",
                                        route.stream_id
                                    )))
                                }
                            };
                            let body = serialize_explicit_inline_body_preserving_source_offsets(
                                stream,
                                stream.stream_offset,
                                &glyph_codes,
                                &explicit_layout.candidate_glyph_run_lengths,
                                padding_code,
                            )?;
                            let expected = source
                                .get(stream.stream_offset..stream.end_offset)
                                .ok_or_else(|| {
                                    error(format!(
                                        "{}: inline 원문 body 범위 이탈",
                                        route.stream_id
                                    ))
                                })?;
                            write_exact(
                                &mut output[..source.len()],
                                &mut claimed,
                                stream.stream_offset,
                                expected,
                                &body,
                                &route.stream_id,
                            )?;
                        }
                        QuestTranslationSelectionPlacement::RelocatedTailCapacityCandidate => {
                            relocated_routes += 1;
                            let body_offset = route.relocated_stream_offset.ok_or_else(|| {
                                error(format!("{}: tail offset 누락", route.stream_id))
                            })?;
                            if output.len() > body_offset {
                                return Err(error(format!(
                                    "{}: tail이 기존 또는 앞 tail과 겹침",
                                    route.stream_id
                                )));
                            }
                            output.resize(body_offset, 0);
                            let body = serialize_explicit_body(
                                stream,
                                body_offset,
                                &glyph_codes,
                                &explicit_layout.candidate_glyph_run_lengths,
                            )?;
                            output.extend_from_slice(&body);
                            let return_runtime = route.return_runtime.ok_or_else(|| {
                                error(format!("{}: 복귀 runtime 누락", route.stream_id))
                            })?;
                            output.extend_from_slice(&return_record(return_runtime)?);
                        }
                        QuestTranslationSelectionPlacement::RetainSource => {
                            return Err(error(format!(
                                "{}: 수동 조판 route가 원문 유지로 분류됨",
                                route.stream_id
                            )));
                        }
                    }
                    write_exact(
                        &mut output[..source.len()],
                        &mut claimed,
                        route.record_offset,
                        &route.protected_record,
                        &route.routed_record,
                        &route.stream_id,
                    )?;
                }
                _ => {
                    return Err(error(format!(
                        "{}: writer는 수동 composition 또는 원문 유지 선택만 허용함",
                        route.stream_id
                    )));
                }
            }
        }

        let capacity = sequence_capacity.get(document.sequence.as_str()).copied();
        let expected_output_bytes = match capacity {
            Some(capacity) if capacity.relocated > 0 => capacity.known_tail_end,
            _ => source.len(),
        };
        if output.len() != expected_output_bytes || output.len() > QUEST_SEQ_ACTIVE_WINDOW_BYTES {
            return Err(error(format!(
                "{}: writer 최종 길이 불일치: expected={expected_output_bytes}, actual={}",
                document.sequence,
                output.len()
            )));
        }
        let (retained, opaque, addresses_exact, non_target) =
            verify_preserved_ranges(document, &source, &output, &routes, addresses)?;
        retained_source_exact &= retained;
        opaque_exact &= opaque;
        internal_addresses_exact &= addresses_exact;
        non_target_exact &= non_target;
        let changed_bytes = source
            .iter()
            .zip(&output)
            .filter(|(before, after)| before != after)
            .count()
            + output.len().saturating_sub(source.len());
        sequences.push(QuestManualSequenceWrite {
            sequence: document.sequence.clone(),
            source_bytes: source.len(),
            output_sha256: sha256(&output),
            source_sha256: sha256(&source),
            selected_routes,
            retained_source_routes,
            inline_routes,
            relocated_routes,
            tail_bytes: output.len().saturating_sub(source.len()),
            changed_bytes,
            output,
        });
    }

    if !explicit_layout_by_stream.is_empty() {
        return Err(error(format!(
            "QUEST writer 명시적 layout 소비 누락: {}",
            explicit_layout_by_stream
                .keys()
                .copied()
                .collect::<Vec<_>>()
                .join(",")
        )));
    }

    let selected_routes = sequences
        .iter()
        .map(|sequence| sequence.selected_routes)
        .sum();
    let retained_source_routes = sequences
        .iter()
        .map(|sequence| sequence.retained_source_routes)
        .sum();
    let inline_routes = sequences
        .iter()
        .map(|sequence| sequence.inline_routes)
        .sum();
    let relocated_routes = sequences
        .iter()
        .map(|sequence| sequence.relocated_routes)
        .sum();
    let source_bytes = sequences.iter().map(|sequence| sequence.source_bytes).sum();
    let output_bytes = sequences.iter().map(|sequence| sequence.output.len()).sum();
    let changed_bytes = sequences
        .iter()
        .map(|sequence| sequence.changed_bytes)
        .sum();
    Ok(QuestManualStreamWriterAnalysis {
        sequences,
        selected_routes,
        retained_source_routes,
        inline_routes,
        relocated_routes,
        return_records: relocated_routes,
        source_bytes,
        output_bytes,
        changed_bytes,
        retained_source_exact,
        opaque_exact,
        internal_addresses_exact,
        non_target_exact,
        control_order_exact: true,
        explicit_layout_routes,
        translation_approved_routes: 0,
        layout_approved_routes: 0,
        runtime_verified: false,
        product_eligible: false,
    })
}

/// 검토 선택과 명시적 수동 조판을 기본 VDD encoding으로 직렬화한다.
pub fn audit_quest_manual_stream_writer_with_layouts(
    documents: &QuestSeqTextDocumentAnalysis,
    addresses: &QuestSeqAddressAnalysis,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    code_table: &QuestCodeTable,
    package: &QuestGlyphCachePackage,
    explicit_layouts: &[QuestManualStreamLayout],
) -> Result<QuestManualStreamWriterAnalysis, QuestManualStreamWriterError> {
    let encoding = build_quest_translation_encoding_plan(selection, routing, code_table)
        .map_err(|source| error(source.to_string()))?;
    audit_quest_manual_stream_writer_with_code_supply_and_layouts(
        documents,
        addresses,
        selection,
        routing,
        QuestManualStreamCodeSupply {
            code_table,
            package,
            encoding: &encoding,
        },
        explicit_layouts,
    )
}

/// 외부 encoding에 결속한 전수 분모, 명시적 layout과 보호 경계를 검증한다.
pub fn verify_box_quest_manual_stream_writer_with_code_supply_and_layouts(
    documents: &QuestSeqTextDocumentAnalysis,
    addresses: &QuestSeqAddressAnalysis,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    code_supply: QuestManualStreamCodeSupply<'_>,
    explicit_layouts: &[QuestManualStreamLayout],
) -> Result<QuestManualStreamWriterAnalysis, QuestManualStreamWriterError> {
    let analysis = audit_quest_manual_stream_writer_with_code_supply_and_layouts(
        documents,
        addresses,
        selection,
        routing,
        code_supply,
        explicit_layouts,
    )?;
    let target_sequences = analysis
        .sequences
        .iter()
        .filter(|sequence| sequence.selected_routes + sequence.retained_source_routes > 0)
        .count();
    if analysis.sequences.len() != documents.documents.len()
        || target_sequences != routing.sequences.len()
        || analysis.selected_routes != routing.selected
        || analysis.retained_source_routes != routing.retained_source
        || analysis.inline_routes != routing.inline
        || analysis.relocated_routes != routing.relocated
        || analysis.return_records != routing.relocated
        || analysis.source_bytes != documents.source_bytes
        || analysis.output_bytes
            != analysis
                .source_bytes
                .checked_add(routing.known_relocated_bytes)
                .ok_or_else(|| error("QUEST writer 전역 출력 크기 overflow"))?
        || analysis.changed_bytes == 0
        || !analysis.retained_source_exact
        || !analysis.opaque_exact
        || !analysis.internal_addresses_exact
        || !analysis.non_target_exact
        || !analysis.control_order_exact
        || analysis.explicit_layout_routes != explicit_layouts.len()
        || analysis.explicit_layout_routes != selection.selected_manual_compositions
        || analysis.explicit_layout_routes != routing.selected
        || analysis.translation_approved_routes != 0
        || analysis.layout_approved_routes != 0
        || analysis.runtime_verified
        || analysis.product_eligible
    {
        return Err(error("QUEST manual stream writer 전수 판정 drift"));
    }
    Ok(analysis)
}

/// 지원 원본을 기본 VDD encoding으로 검증한다.
pub fn verify_box_quest_manual_stream_writer_with_layouts(
    documents: &QuestSeqTextDocumentAnalysis,
    addresses: &QuestSeqAddressAnalysis,
    selection: &QuestTranslationSelectionAnalysis,
    routing: &QuestTranslationSelectionRoutingAnalysis,
    code_table: &QuestCodeTable,
    package: &QuestGlyphCachePackage,
    explicit_layouts: &[QuestManualStreamLayout],
) -> Result<QuestManualStreamWriterAnalysis, QuestManualStreamWriterError> {
    let encoding = build_quest_translation_encoding_plan(selection, routing, code_table)
        .map_err(|source| error(source.to_string()))?;
    verify_box_quest_manual_stream_writer_with_code_supply_and_layouts(
        documents,
        addresses,
        selection,
        routing,
        QuestManualStreamCodeSupply {
            code_table,
            package,
            encoding: &encoding,
        },
        explicit_layouts,
    )
}

#[cfg(test)]
#[path = "quest_manual_stream_writer_tests.rs"]
mod tests;
