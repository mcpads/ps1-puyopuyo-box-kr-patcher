use std::collections::BTreeMap;

use crate::script::{
    QuestSeqAddressAnalysis, QuestSeqExtractedTextStream, QuestSeqTextDocument,
    QuestSeqTextDocumentAnalysis, QuestSeqTextDocumentChunk,
};

use super::super::quest_translation_selection_routing::{
    QuestTranslationSelectionPlacement, QuestTranslationSelectionRoute,
};
use super::model::{error, QuestManualStreamWriterError};

pub(super) fn write_exact(
    output: &mut [u8],
    claimed: &mut [bool],
    offset: usize,
    expected: &[u8],
    replacement: &[u8],
    label: &str,
) -> Result<(), QuestManualStreamWriterError> {
    if expected.len() != replacement.len() {
        return Err(error(format!("{label}: Expected Write 길이 불일치")));
    }
    let end = offset
        .checked_add(expected.len())
        .ok_or_else(|| error(format!("{label}: Expected Write 범위 overflow")))?;
    let target = output
        .get_mut(offset..end)
        .ok_or_else(|| error(format!("{label}: Expected Write 범위 이탈")))?;
    let claim = claimed
        .get_mut(offset..end)
        .ok_or_else(|| error(format!("{label}: claim 범위 이탈")))?;
    if target != expected {
        return Err(error(format!("{label}: Expected Write 원본 byte 불일치")));
    }
    if claim.iter().any(|claimed| *claimed) {
        return Err(error(format!("{label}: Expected Write 겹침")));
    }
    target.copy_from_slice(replacement);
    claim.fill(true);
    Ok(())
}

pub(super) fn stream_map(
    documents: &QuestSeqTextDocumentAnalysis,
) -> Result<BTreeMap<String, &QuestSeqExtractedTextStream>, QuestManualStreamWriterError> {
    let mut result = BTreeMap::new();
    for document in &documents.documents {
        for chunk in &document.chunks {
            if let QuestSeqTextDocumentChunk::TextStream(stream) = chunk {
                let id = stream.id();
                if result.insert(id.clone(), stream).is_some() {
                    return Err(error(format!("QUEST writer stream ID 중복: {id}")));
                }
            }
        }
    }
    Ok(result)
}

pub(super) fn verify_preserved_ranges(
    document: &QuestSeqTextDocument,
    source: &[u8],
    output: &[u8],
    routes: &[&QuestTranslationSelectionRoute],
    addresses: &QuestSeqAddressAnalysis,
) -> Result<(bool, bool, bool, bool), QuestManualStreamWriterError> {
    let route_by_id = routes
        .iter()
        .map(|route| (route.stream_id.as_str(), *route))
        .collect::<BTreeMap<_, _>>();
    let mut retained_source_exact = true;
    let mut opaque_exact = true;
    let mut non_target_exact = true;
    for chunk in &document.chunks {
        match chunk {
            QuestSeqTextDocumentChunk::Opaque { offset, raw } => {
                let end = offset
                    .checked_add(raw.len())
                    .ok_or_else(|| error("QUEST writer opaque 범위 overflow"))?;
                opaque_exact &= output.get(*offset..end) == Some(raw.as_slice());
            }
            QuestSeqTextDocumentChunk::TextStream(stream) => {
                let route = route_by_id.get(stream.id().as_str()).copied();
                let should_be_exact = route.is_none()
                    || route.is_some_and(|route| {
                        route.placement == QuestTranslationSelectionPlacement::RetainSource
                    });
                if should_be_exact {
                    non_target_exact &= output.get(stream.record_offset..stream.end_offset)
                        == source.get(stream.record_offset..stream.end_offset);
                    retained_source_exact &= route.is_none()
                        || output.get(stream.record_offset..stream.end_offset)
                            == source.get(stream.record_offset..stream.end_offset);
                } else if route.is_some_and(|route| {
                    route.placement
                        == QuestTranslationSelectionPlacement::RelocatedTailCapacityCandidate
                }) {
                    non_target_exact &= output.get(stream.stream_offset..stream.end_offset)
                        == source.get(stream.stream_offset..stream.end_offset);
                }
            }
        }
    }
    let internal_addresses_exact = addresses
        .candidates
        .iter()
        .filter(|candidate| candidate.sequence == document.sequence)
        .all(|candidate| {
            let end = candidate.field_offset.checked_add(4);
            end.is_some_and(|end| {
                output.get(candidate.field_offset..end) == source.get(candidate.field_offset..end)
            })
        });
    Ok((
        retained_source_exact,
        opaque_exact,
        internal_addresses_exact,
        non_target_exact,
    ))
}
