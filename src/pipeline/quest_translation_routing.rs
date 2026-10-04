//! QUEST 번역 stream을 원본 SEQ의 고정 위치 또는 보호된 꼬리에 배치하는 계획.
//!
//! 113개 문서 전체를 재직렬화해 뒤 바이트를 미는 대신 canonical OP21의
//! 미소비 3바이트를 `page_id:u8 + target_word_offset:u16`으로 사용한다.
//! `target_word_offset=0`이면 원래 다음 byte를, 0이 아니면 같은 active
//! window의 `base + target_word_offset * 4`를 고른다. 승인된 번역의
//! 실제 stream byte 수요가 원본 범위 안에 들면 control을 보존해 채우고,
//! 넘는 stream만 같은 SEQ의 32 KiB active window 꼬리에 둔다. 이 단계는
//! 초안의 byte 수요와 주소 보존 가능성을 판정하며, 사람 layout 승인이나
//! font page 공급을 대신하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use crate::script::{
    quest_control_spec, QuestSeqAddressAnalysis, QuestSeqAddressEvidence,
    QuestSeqExtractedTextStream, QuestSeqExtractedTokenKind, QuestSeqRuntimeBaseEvidence,
    QuestSeqTextDocumentAnalysis, QuestSeqTextDocumentChunk, QuestTextTerminal,
    QuestTranslationBatchAnalysis, QuestTranslationBatchRole, QuestTranslationWorkStatus,
    QUEST_SEQ_ACTIVE_RUNTIME_BASE, QUEST_SEQ_TEXT_MODE_OPCODE, QUEST_SEQ_TEXT_MODE_RECORD_BYTES,
};

pub const QUEST_TRANSLATION_ROUTING_SCHEMA_VERSION: &str = "quest-translation-routing-v2";
pub const QUEST_SEQ_ACTIVE_WINDOW_BYTES: usize = 32 * 1024;
pub const QUEST_TRANSLATION_RETURN_RECORD_BYTES: usize = 8;

/// 현재 초안으로 판정할 수 있는 저장 위치.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestTranslationPlacement {
    /// 승인 layout이 원본 glyph 수를 넘지 않으면 기존 stream 안에 쓸 수 있다.
    InlineCapacityCandidate,
    /// 원본 stream을 밀지 않고 같은 SEQ의 active-window 꼬리에 둬야 한다.
    RelocatedTailCapacityCandidate,
    /// 안정 placeholder를 사람 결정으로 바꾸기 전에는 byte 수요를 계산하지 않는다.
    PendingHumanDraftDecision,
}

impl QuestTranslationPlacement {
    pub const fn label(self) -> &'static str {
        match self {
            Self::InlineCapacityCandidate => "inline-capacity-candidate",
            Self::RelocatedTailCapacityCandidate => "relocated-tail-capacity-candidate",
            Self::PendingHumanDraftDecision => "pending-human-draft-decision",
        }
    }
}

/// OP21 record 하나와 번역 초안 수요를 결속한 행.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationRoute {
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub stream_id: String,
    pub batch_id: String,
    pub route_id: u16,
    pub page_id: u8,
    pub target_word_offset: u16,
    pub record_offset: usize,
    pub end_offset: usize,
    pub terminal: QuestTextTerminal,
    pub protected_record: [u8; QUEST_SEQ_TEXT_MODE_RECORD_BYTES],
    pub routed_record: [u8; QUEST_SEQ_TEXT_MODE_RECORD_BYTES],
    pub source_glyphs: usize,
    pub draft_glyphs: Option<usize>,
    pub source_stream_bytes: usize,
    pub projected_stream_bytes: Option<usize>,
    pub placement: QuestTranslationPlacement,
    pub relocated_record_offset: Option<usize>,
    pub return_runtime: Option<u32>,
    pub layout_approved: bool,
    pub build_eligible: bool,
}

/// 한 SEQ에서 기존 주소를 보존한 채 사용할 수 있는 꼬리 범위.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationSequenceCapacity {
    pub sequence: String,
    pub runtime_base_evidence: QuestSeqRuntimeBaseEvidence,
    pub source_bytes: usize,
    pub address_fields: usize,
    pub static_or_runtime_pointer_fields: usize,
    pub range_candidate_fields: usize,
    pub routes: usize,
    pub inline_candidates: usize,
    pub relocated_candidates: usize,
    pub pending_draft_decisions: usize,
    pub tail_start: usize,
    pub known_tail_end: usize,
    pub known_relocated_bytes: usize,
    pub active_window_remaining: usize,
    pub known_tail_fits: bool,
}

/// 580개 route와 12개 SEQ의 현재 수용량 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationRoutingAnalysis {
    pub routes: Vec<QuestTranslationRoute>,
    pub sequences: Vec<QuestTranslationSequenceCapacity>,
    pub pages: usize,
    pub resolved_drafts: usize,
    pub inline_candidates: usize,
    pub relocated_candidates: usize,
    pub pending_draft_decisions: usize,
    pub known_relocated_bytes: usize,
    pub minimum_active_window_remaining: usize,
    pub runtime_base_observed_sequences: usize,
    pub static_loader_runtime_base_sequences: usize,
    pub runtime_base_bindings_proven: bool,
    pub build_eligible_routes: usize,
}

/// 원문·배치·주소 원장 또는 active-window 수용량 불일치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestTranslationRoutingError(String);

impl fmt::Display for QuestTranslationRoutingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestTranslationRoutingError {}

fn error(message: impl Into<String>) -> QuestTranslationRoutingError {
    QuestTranslationRoutingError(message.into())
}

pub(super) fn align4(value: usize) -> Result<usize, QuestTranslationRoutingError> {
    value
        .checked_add(3)
        .map(|aligned| aligned & !3)
        .ok_or_else(|| error("QUEST 번역 route 4바이트 정렬 overflow"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn draft_needs_human_decision(status: QuestTranslationWorkStatus, draft: &str) -> bool {
    status == QuestTranslationWorkStatus::NeedsHumanTermDecision
        || draft.contains('{')
        || draft.contains('}')
}

pub(super) fn routed_record(
    stream: &QuestSeqExtractedTextStream,
    page_id: u8,
    target_stream_offset: Option<usize>,
) -> Result<[u8; QUEST_SEQ_TEXT_MODE_RECORD_BYTES], QuestTranslationRoutingError> {
    if stream.record_raw != [QUEST_SEQ_TEXT_MODE_OPCODE, 0, 0, 0] {
        return Err(error(format!(
            "{}: route 입력 OP21 record가 canonical 원본이 아님",
            stream.id()
        )));
    }
    if page_id == 0 {
        return Err(error(format!(
            "{}: page ID 0은 원본 경로용으로 예약됨",
            stream.id()
        )));
    }
    let target_word_offset = match target_stream_offset {
        None => 0,
        Some(offset) if offset == 0 || !offset.is_multiple_of(4) => {
            return Err(error(format!(
                "{}: relocated target은 0이 아닌 4바이트 정렬 offset이어야 함: 0x{offset:X}",
                stream.id()
            )));
        }
        Some(offset) => u16::try_from(offset / 4).map_err(|_| {
            error(format!(
                "{}: relocated target word offset이 u16 범위를 넘음: 0x{offset:X}",
                stream.id()
            ))
        })?,
    };
    let [target_low, target_high] = target_word_offset.to_le_bytes();
    Ok([QUEST_SEQ_TEXT_MODE_OPCODE, page_id, target_low, target_high])
}

pub(super) fn projected_stream_bytes(
    stream: &QuestSeqExtractedTextStream,
    record_offset: usize,
    glyphs: usize,
) -> Result<usize, QuestTranslationRoutingError> {
    if !record_offset.is_multiple_of(4) {
        return Err(error(format!(
            "{}: 배치 record가 4바이트 정렬이 아님: 0x{record_offset:04X}",
            stream.id()
        )));
    }
    let body_offset = record_offset
        .checked_add(QUEST_SEQ_TEXT_MODE_RECORD_BYTES)
        .ok_or_else(|| error(format!("{}: projected body offset overflow", stream.id())))?;
    projected_stream_body_bytes(stream, body_offset, glyphs).and_then(|body_bytes| {
        QUEST_SEQ_TEXT_MODE_RECORD_BYTES
            .checked_add(body_bytes)
            .ok_or_else(|| error(format!("{}: projected stream 길이 overflow", stream.id())))
    })
}

/// OP21 record 없이 실제 text handler가 읽기 시작하는 본문의 byte 수요.
pub(super) fn projected_stream_body_bytes(
    stream: &QuestSeqExtractedTextStream,
    body_offset: usize,
    glyphs: usize,
) -> Result<usize, QuestTranslationRoutingError> {
    if !body_offset.is_multiple_of(4) {
        return Err(error(format!(
            "{}: 배치 body가 4바이트 정렬이 아님: 0x{body_offset:04X}",
            stream.id()
        )));
    }
    let terminal_code = stream.terminal.code();
    let terminal_index = stream
        .tokens
        .iter()
        .rposition(|token| token.kind == QuestSeqExtractedTokenKind::Control)
        .ok_or_else(|| error(format!("{}: terminal control이 없음", stream.id())))?;
    if terminal_index + 1 != stream.tokens.len()
        || stream.tokens[terminal_index].control_code != Some(terminal_code)
    {
        return Err(error(format!(
            "{}: 추출 종단과 마지막 control이 다름",
            stream.id()
        )));
    }

    let nonterminal_control_bytes = stream.tokens[..terminal_index]
        .iter()
        .filter(|token| token.kind == QuestSeqExtractedTokenKind::Control)
        .try_fold(0usize, |total, token| {
            total
                .checked_add(token.raw.len())
                .ok_or_else(|| error(format!("{}: control byte 합계 overflow", stream.id())))
        })?;
    let glyph_bytes = glyphs
        .checked_mul(2)
        .ok_or_else(|| error(format!("{}: draft glyph byte overflow", stream.id())))?;
    let terminal_offset = glyph_bytes
        .checked_add(nonterminal_control_bytes)
        .ok_or_else(|| error(format!("{}: projected body offset overflow", stream.id())))?;
    let terminal_runtime = QUEST_SEQ_ACTIVE_RUNTIME_BASE
        .checked_add(
            u32::try_from(body_offset)
                .map_err(|_| error(format!("{}: body offset이 u32를 넘음", stream.id())))?,
        )
        .and_then(|address| address.checked_add(u32::try_from(terminal_offset).ok()?))
        .ok_or_else(|| error(format!("{}: terminal runtime overflow", stream.id())))?;
    let terminal_bytes = quest_control_spec(terminal_code)
        .ok_or_else(|| error(format!("{}: terminal control spec 누락", stream.id())))?
        .layout
        .encoded_len(terminal_runtime)
        .ok_or_else(|| error(format!("{}: terminal 길이 계산 실패", stream.id())))?;
    terminal_offset
        .checked_add(terminal_bytes)
        .ok_or_else(|| error(format!("{}: projected stream 길이 overflow", stream.id())))
}

fn page_ids(
    batches: &QuestTranslationBatchAnalysis,
) -> Result<BTreeMap<&str, u8>, QuestTranslationRoutingError> {
    let batch_ids = batches
        .entries
        .iter()
        .filter(|entry| entry.role == QuestTranslationBatchRole::Translate)
        .map(|entry| entry.batch_id.as_str())
        .collect::<BTreeSet<_>>();
    if batch_ids.is_empty() || batch_ids.len() > usize::from(u8::MAX) {
        return Err(error(format!(
            "QUEST 번역 page ID 공급 범위 이탈: batches={}",
            batch_ids.len()
        )));
    }
    batch_ids
        .into_iter()
        .enumerate()
        .map(|(index, batch_id)| {
            let page_id = u8::try_from(index + 1)
                .map_err(|_| error("QUEST 번역 page ID가 u8 범위를 넘음"))?;
            Ok((batch_id, page_id))
        })
        .collect()
}

/// 문서·주소·번역 배치를 원본 위치 보존형 route로 결합한다.
pub fn audit_quest_translation_routing(
    documents: &QuestSeqTextDocumentAnalysis,
    addresses: &QuestSeqAddressAnalysis,
    batches: &QuestTranslationBatchAnalysis,
) -> Result<QuestTranslationRoutingAnalysis, QuestTranslationRoutingError> {
    let mut streams = BTreeMap::<String, &QuestSeqExtractedTextStream>::new();
    let mut document_by_sequence = BTreeMap::new();
    for document in &documents.documents {
        if document_by_sequence
            .insert(document.sequence.as_str(), document)
            .is_some()
        {
            return Err(error(format!(
                "QUEST 번역 route 문서 이름 중복: {}",
                document.sequence
            )));
        }
        for chunk in &document.chunks {
            if let QuestSeqTextDocumentChunk::TextStream(stream) = chunk {
                let id = stream.id();
                if streams.insert(id.clone(), stream).is_some() {
                    return Err(error(format!("QUEST 번역 route stream ID 중복: {id}")));
                }
            }
        }
    }
    let summary_by_sequence = addresses
        .summaries
        .iter()
        .map(|summary| (summary.sequence.as_str(), summary))
        .collect::<BTreeMap<_, _>>();
    let pages = page_ids(batches)?;

    let mut translate_entries = batches
        .entries
        .iter()
        .filter(|entry| entry.role == QuestTranslationBatchRole::Translate)
        .collect::<Vec<_>>();
    translate_entries.sort_by(|left, right| {
        left.stream_id
            .cmp(&right.stream_id)
            .then(left.batch_id.cmp(&right.batch_id))
    });
    if translate_entries.len() > usize::from(u16::MAX) {
        return Err(error("QUEST 번역 route ID가 u16 범위를 넘음"));
    }

    let mut seen = BTreeSet::new();
    let mut routes = Vec::with_capacity(translate_entries.len());
    for (index, entry) in translate_entries.into_iter().enumerate() {
        if !seen.insert(entry.stream_id.as_str()) {
            return Err(error(format!(
                "QUEST 번역 route 대상 중복: {}",
                entry.stream_id
            )));
        }
        let stream = streams.get(&entry.stream_id).copied().ok_or_else(|| {
            error(format!(
                "{}: QUEST 번역 route 원본 stream 누락",
                entry.stream_id
            ))
        })?;
        let document = document_by_sequence
            .get(stream.sequence.as_str())
            .copied()
            .ok_or_else(|| error(format!("{}: owner 문서 누락", stream.id())))?;
        if document.source_sha256 != stream.sequence_sha256 {
            return Err(error(format!("{}: owner 문서 digest 불일치", stream.id())));
        }
        let summary = summary_by_sequence
            .get(stream.sequence.as_str())
            .copied()
            .ok_or_else(|| error(format!("{}: 주소 원장 summary 누락", stream.sequence)))?;
        if summary.bytes != document.source_len || summary.fields == 0 {
            return Err(error(format!(
                "{}: 주소 원장과 문서 길이·field 분모 불일치",
                stream.sequence
            )));
        }
        if stream.terminal != QuestTextTerminal::DeactivateAndAlign {
            return Err(error(format!(
                "{}: 현재 꼬리 복귀 계획은 FF03 stream만 지원함: {}",
                stream.id(),
                stream.terminal.label()
            )));
        }

        let source_glyphs = stream
            .tokens
            .iter()
            .filter(|token| token.kind == QuestSeqExtractedTokenKind::Glyph)
            .count();
        let source_stream_bytes = stream
            .byte_len()
            .ok_or_else(|| error(format!("{}: 원본 stream 길이 underflow", stream.id())))?;
        let unresolved = draft_needs_human_decision(entry.work_status, &entry.korean_draft);
        let draft_glyphs = (!unresolved).then(|| entry.korean_draft.chars().count());
        let projected_stream_bytes = draft_glyphs
            .map(|glyphs| projected_stream_bytes(stream, stream.record_offset, glyphs))
            .transpose()?;
        let placement = match projected_stream_bytes {
            None => QuestTranslationPlacement::PendingHumanDraftDecision,
            Some(projected) if projected <= source_stream_bytes => {
                QuestTranslationPlacement::InlineCapacityCandidate
            }
            Some(_) => QuestTranslationPlacement::RelocatedTailCapacityCandidate,
        };
        let route_id =
            u16::try_from(index + 1).map_err(|_| error("QUEST 번역 route ID 변환 실패"))?;
        let page_id = *pages
            .get(entry.batch_id.as_str())
            .ok_or_else(|| error(format!("{}: batch page ID 누락", entry.batch_id)))?;
        routes.push(QuestTranslationRoute {
            sequence: stream.sequence.clone(),
            sequence_sha256: stream.sequence_sha256,
            stream_id: entry.stream_id.clone(),
            batch_id: entry.batch_id.clone(),
            route_id,
            page_id,
            target_word_offset: 0,
            record_offset: stream.record_offset,
            end_offset: stream.end_offset,
            terminal: stream.terminal,
            protected_record: stream.record_raw,
            routed_record: routed_record(stream, page_id, None)?,
            source_glyphs,
            draft_glyphs,
            source_stream_bytes,
            projected_stream_bytes,
            placement,
            relocated_record_offset: None,
            return_runtime: None,
            layout_approved: false,
            build_eligible: false,
        });
    }

    routes.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then(left.record_offset.cmp(&right.record_offset))
    });
    let mut sequences = Vec::new();
    for sequence in routes
        .iter()
        .map(|route| route.sequence.clone())
        .collect::<BTreeSet<_>>()
    {
        let document = document_by_sequence
            .get(sequence.as_str())
            .copied()
            .ok_or_else(|| error(format!("{sequence}: owner 문서 누락")))?;
        let summary = summary_by_sequence
            .get(sequence.as_str())
            .copied()
            .ok_or_else(|| error(format!("{sequence}: 주소 summary 누락")))?;
        let sequence_candidates = addresses
            .candidates
            .iter()
            .filter(|candidate| candidate.sequence == sequence)
            .collect::<Vec<_>>();
        if sequence_candidates.len() != summary.fields {
            return Err(error(format!(
                "{sequence}: 주소 candidate와 summary 분모 불일치"
            )));
        }
        let static_or_runtime_pointer_fields = sequence_candidates
            .iter()
            .filter(|candidate| candidate.evidence != QuestSeqAddressEvidence::RangeCandidate)
            .count();
        let range_candidate_fields = sequence_candidates.len() - static_or_runtime_pointer_fields;
        if static_or_runtime_pointer_fields == 0 {
            return Err(error(format!(
                "{sequence}: same-base pointer record 또는 runtime read 근거가 없음"
            )));
        }
        let tail_start = align4(document.source_len)?;
        let mut cursor = tail_start;
        for route in routes.iter_mut().filter(|route| route.sequence == sequence) {
            if route.placement != QuestTranslationPlacement::RelocatedTailCapacityCandidate {
                continue;
            }
            cursor = align4(cursor)?;
            let relocated_record_offset = cursor;
            let projected = route.projected_stream_bytes.ok_or_else(|| {
                error(format!(
                    "{}: relocated stream byte 수요 누락",
                    route.stream_id
                ))
            })?;
            let relocated_projected = projected_stream_bytes(
                streams
                    .get(&route.stream_id)
                    .copied()
                    .expect("route가 결속한 stream"),
                relocated_record_offset,
                route
                    .draft_glyphs
                    .expect("relocated route는 resolved draft"),
            )?;
            if projected != relocated_projected {
                return Err(error(format!(
                    "{}: 4바이트 정렬 위치에 따른 stream 길이 drift",
                    route.stream_id
                )));
            }
            cursor = cursor
                .checked_add(relocated_projected)
                .and_then(|value| value.checked_add(QUEST_TRANSLATION_RETURN_RECORD_BYTES))
                .ok_or_else(|| error(format!("{}: tail 배치 overflow", route.stream_id)))?;
            let continuation_offset = u32::try_from(route.end_offset).map_err(|_| {
                error(format!(
                    "{}: continuation offset 범위 이탈",
                    route.stream_id
                ))
            })?;
            route.relocated_record_offset = Some(relocated_record_offset);
            route.target_word_offset = u16::try_from(relocated_record_offset / 4)
                .map_err(|_| error(format!("{}: target word offset 범위 이탈", route.stream_id)))?;
            route.routed_record = routed_record(
                streams
                    .get(&route.stream_id)
                    .copied()
                    .expect("route가 결속한 stream"),
                route.page_id,
                Some(relocated_record_offset),
            )?;
            route.return_runtime = Some(
                QUEST_SEQ_ACTIVE_RUNTIME_BASE
                    .checked_add(continuation_offset)
                    .ok_or_else(|| {
                        error(format!("{}: return runtime overflow", route.stream_id))
                    })?,
            );
        }
        let known_tail_fits = cursor <= QUEST_SEQ_ACTIVE_WINDOW_BYTES;
        let active_window_remaining = QUEST_SEQ_ACTIVE_WINDOW_BYTES.saturating_sub(cursor);
        let sequence_routes = routes
            .iter()
            .filter(|route| route.sequence == sequence)
            .collect::<Vec<_>>();
        sequences.push(QuestTranslationSequenceCapacity {
            sequence,
            runtime_base_evidence: summary.runtime_base_evidence,
            source_bytes: document.source_len,
            address_fields: summary.fields,
            static_or_runtime_pointer_fields,
            range_candidate_fields,
            routes: sequence_routes.len(),
            inline_candidates: sequence_routes
                .iter()
                .filter(|route| {
                    route.placement == QuestTranslationPlacement::InlineCapacityCandidate
                })
                .count(),
            relocated_candidates: sequence_routes
                .iter()
                .filter(|route| {
                    route.placement == QuestTranslationPlacement::RelocatedTailCapacityCandidate
                })
                .count(),
            pending_draft_decisions: sequence_routes
                .iter()
                .filter(|route| {
                    route.placement == QuestTranslationPlacement::PendingHumanDraftDecision
                })
                .count(),
            tail_start,
            known_tail_end: cursor,
            known_relocated_bytes: cursor - tail_start,
            active_window_remaining,
            known_tail_fits,
        });
    }

    let resolved_drafts = routes
        .iter()
        .filter(|route| route.draft_glyphs.is_some())
        .count();
    let inline_candidates = routes
        .iter()
        .filter(|route| route.placement == QuestTranslationPlacement::InlineCapacityCandidate)
        .count();
    let relocated_candidates = routes
        .iter()
        .filter(|route| {
            route.placement == QuestTranslationPlacement::RelocatedTailCapacityCandidate
        })
        .count();
    let pending_draft_decisions = routes.len() - resolved_drafts;
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
    let build_eligible_routes = routes.iter().filter(|route| route.build_eligible).count();
    Ok(QuestTranslationRoutingAnalysis {
        routes,
        sequences,
        pages: pages.len(),
        resolved_drafts,
        inline_candidates,
        relocated_candidates,
        pending_draft_decisions,
        known_relocated_bytes,
        minimum_active_window_remaining,
        runtime_base_observed_sequences,
        static_loader_runtime_base_sequences,
        runtime_base_bindings_proven,
        build_eligible_routes,
    })
}

/// 지원 원본의 route 분모·수용량·결정적 report를 검증한다.
pub fn verify_box_quest_translation_routing(
    documents: &QuestSeqTextDocumentAnalysis,
    addresses: &QuestSeqAddressAnalysis,
    batches: &QuestTranslationBatchAnalysis,
) -> Result<QuestTranslationRoutingAnalysis, QuestTranslationRoutingError> {
    let analysis = audit_quest_translation_routing(documents, addresses, batches)?;
    let sequence_routes = analysis
        .sequences
        .iter()
        .map(|sequence| sequence.routes)
        .sum::<usize>();
    let sequence_inline = analysis
        .sequences
        .iter()
        .map(|sequence| sequence.inline_candidates)
        .sum::<usize>();
    let sequence_relocated = analysis
        .sequences
        .iter()
        .map(|sequence| sequence.relocated_candidates)
        .sum::<usize>();
    let sequence_pending = analysis
        .sequences
        .iter()
        .map(|sequence| sequence.pending_draft_decisions)
        .sum::<usize>();
    let sequence_relocated_bytes = analysis
        .sequences
        .iter()
        .map(|sequence| sequence.known_relocated_bytes)
        .sum::<usize>();
    let minimum_remaining = analysis
        .sequences
        .iter()
        .map(|sequence| sequence.active_window_remaining)
        .min()
        .unwrap_or(0);
    let route_pages = analysis
        .routes
        .iter()
        .map(|route| route.page_id)
        .collect::<BTreeSet<_>>();
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
    if analysis.routes.len() != batches.translate_entries
        || analysis.pages != route_pages.len()
        || !route_pages
            .iter()
            .copied()
            .eq(1..=u8::try_from(analysis.pages).map_err(|_| error("QUEST page 수 u8 초과"))?)
        || analysis.resolved_drafts + analysis.pending_draft_decisions != analysis.routes.len()
        || analysis.inline_candidates
            + analysis.relocated_candidates
            + analysis.pending_draft_decisions
            != analysis.routes.len()
        || sequence_routes != analysis.routes.len()
        || sequence_inline != analysis.inline_candidates
        || sequence_relocated != analysis.relocated_candidates
        || sequence_pending != analysis.pending_draft_decisions
        || sequence_relocated_bytes != analysis.known_relocated_bytes
        || minimum_remaining != analysis.minimum_active_window_remaining
        || analysis.runtime_base_observed_sequences != observed_runtime_bases
        || analysis.static_loader_runtime_base_sequences != static_loader_runtime_bases
        || analysis.runtime_base_observed_sequences + analysis.static_loader_runtime_base_sequences
            != analysis.sequences.len()
        || analysis.runtime_base_bindings_proven
            != analysis
                .sequences
                .iter()
                .all(|sequence| sequence.runtime_base_evidence.base_binding_proven())
        || analysis.sequences.iter().any(|sequence| {
            !sequence.known_tail_fits
                || sequence.known_tail_end > QUEST_SEQ_ACTIVE_WINDOW_BYTES
                || sequence.inline_candidates
                    + sequence.relocated_candidates
                    + sequence.pending_draft_decisions
                    != sequence.routes
        })
        || analysis.routes.iter().any(|route| {
            usize::from(route.page_id) > analysis.pages
                || route.page_id == 0
                || route.layout_approved
                || route.build_eligible
        })
        || analysis.build_eligible_routes != 0
    {
        return Err(error("QUEST 번역 route 파생 분모·수용량 불일치"));
    }
    Ok(analysis)
}

/// 원본 record와 route metadata, 초안 수용량을 함께 보여 주는 검토 TSV.
pub fn render_quest_translation_routing_tsv(analysis: &QuestTranslationRoutingAnalysis) -> String {
    let sequence_by_name = analysis
        .sequences
        .iter()
        .map(|sequence| (sequence.sequence.as_str(), sequence))
        .collect::<BTreeMap<_, _>>();
    let mut output = String::from(
        "schema_version\tsequence\tsequence_sha256\truntime_base_evidence\tsource_bytes\taddress_fields\t\
         static_or_runtime_pointer_fields\trange_candidate_fields\tstream_id\tbatch_id\t\
         route_id\tpage_id\ttarget_word_offset\trecord_offset\tend_offset\tterminal\tprotected_record\t\
         routed_record\tsource_glyphs\tdraft_glyphs\tsource_stream_bytes\t\
         projected_stream_bytes\tplacement\trelocated_record_offset\treturn_runtime\t\
         tail_start\tknown_tail_end\tactive_window_remaining\tknown_tail_fits\t\
         layout_approved\tbuild_eligible\n",
    );
    for route in &analysis.routes {
        let sequence = sequence_by_name
            .get(route.sequence.as_str())
            .expect("route의 sequence capacity가 있음");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t0x{:04X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t0x{:04X}\t{}\t{}\tno\tno",
            QUEST_TRANSLATION_ROUTING_SCHEMA_VERSION,
            route.sequence,
            hex(&route.sequence_sha256),
            sequence.runtime_base_evidence.label(),
            sequence.source_bytes,
            sequence.address_fields,
            sequence.static_or_runtime_pointer_fields,
            sequence.range_candidate_fields,
            route.stream_id,
            escape_tsv(&route.batch_id),
            route.route_id,
            route.page_id,
            route.target_word_offset,
            route.record_offset,
            route.end_offset,
            route.terminal.label(),
            hex(&route.protected_record),
            hex(&route.routed_record),
            route.source_glyphs,
            route.draft_glyphs.map_or_else(String::new, |value| value.to_string()),
            route.source_stream_bytes,
            route
                .projected_stream_bytes
                .map_or_else(String::new, |value| value.to_string()),
            route.placement.label(),
            route
                .relocated_record_offset
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

#[cfg(test)]
#[path = "quest_translation_routing_tests.rs"]
mod tests;
