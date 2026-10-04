//! 113개 QUEST SEQ의 OP21 중심 lossless 문서 경계.
//!
//! 검증된 canonical OP21 stream은 token 보존형 chunk로, 그 밖의 바이트는
//! opaque chunk로 소유한다. 이 단계는 전체 opcode 의미나 section 소유권을
//! 주장하지 않는다. 대신 각 원본 바이트를 정확히 한 chunk가 소유하고,
//! 문서 재직렬화가 원본과 byte-identical인지 검증한다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::quest_seq_inventory::{BOX_QUEST_SEQ_BYTES, BOX_QUEST_SEQ_SEQUENCES};
use super::quest_seq_text_extraction::{
    serialize_quest_seq_extracted_text_stream, QuestSeqExtractedTextStream,
    QuestSeqTextExtractionAnalysis, BOX_QUEST_SEQ_EXTRACTED_ROUNDTRIP_BYTES,
    BOX_QUEST_SEQ_EXTRACTED_TEXT_STREAMS,
};

pub const QUEST_SEQ_TEXT_DOCUMENT_SCHEMA_VERSION: &str = "quest-seq-text-documents-v1";
pub const BOX_QUEST_SEQ_TEXT_DOCUMENTS: usize = 113;
pub const BOX_QUEST_SEQ_TEXT_DOCUMENT_BYTES: usize = 243_546;
pub const BOX_QUEST_SEQ_TEXT_DOCUMENT_STREAM_CHUNKS: usize = 605;
pub const BOX_QUEST_SEQ_TEXT_DOCUMENT_STREAM_BYTES: usize = 98_516;
pub const BOX_QUEST_SEQ_TEXT_DOCUMENT_OPAQUE_BYTES: usize = 145_030;
pub const BOX_QUEST_SEQ_TEXT_DOCUMENT_OPAQUE_CHUNKS: usize = 711;
pub const BOX_QUEST_SEQ_TEXT_DOCUMENT_REPORT_SHA256: &str =
    "75928d25197756ae992e70645e46a3004fd02ee328f494387a2aa9b18d2f9d6a";

/// 원본 SEQ 전체를 겹침 없이 나누는 chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestSeqTextDocumentChunk {
    /// 아직 구조 의미를 부여하지 않고 원본 그대로 보호하는 범위.
    Opaque { offset: usize, raw: Vec<u8> },
    /// canonical OP21 record부터 lifecycle 종단까지 검증한 범위.
    TextStream(QuestSeqExtractedTextStream),
}

impl QuestSeqTextDocumentChunk {
    pub const fn offset(&self) -> usize {
        match self {
            Self::Opaque { offset, .. } => *offset,
            Self::TextStream(stream) => stream.record_offset,
        }
    }

    pub fn byte_len(&self) -> Result<usize, QuestSeqTextDocumentError> {
        match self {
            Self::Opaque { raw, .. } => Ok(raw.len()),
            Self::TextStream(stream) => stream
                .byte_len()
                .ok_or_else(|| error(format!("{}: stream 범위 underflow", stream.id()))),
        }
    }
}

/// 한 SEQ의 전 바이트 소유권과 원본 digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextDocument {
    pub sequence: String,
    pub source_len: usize,
    pub source_sha256: [u8; 32],
    pub chunks: Vec<QuestSeqTextDocumentChunk>,
}

/// 지원 원본 113개 전체의 lossless 문서 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextDocumentAnalysis {
    pub documents: Vec<QuestSeqTextDocument>,
    pub source_bytes: usize,
    pub chunks: usize,
    pub opaque_chunks: usize,
    pub opaque_bytes: usize,
    pub text_stream_chunks: usize,
    pub text_stream_bytes: usize,
}

/// 파일·chunk·원본 digest 경계 불일치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextDocumentError(String);

impl fmt::Display for QuestSeqTextDocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestSeqTextDocumentError {}

fn error(message: impl Into<String>) -> QuestSeqTextDocumentError {
    QuestSeqTextDocumentError(message.into())
}

fn digest(raw: &[u8]) -> [u8; 32] {
    Sha256::digest(raw).into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 한 문서의 chunk를 순서대로 합성하고 원본 길이·digest를 검증한다.
///
/// 오류가 있으면 최종 `Vec`을 반환하지 않으므로 부분 산출물이 없다.
pub fn serialize_quest_seq_text_document(
    document: &QuestSeqTextDocument,
) -> Result<Vec<u8>, QuestSeqTextDocumentError> {
    if document.sequence.is_empty() {
        return Err(error("QUEST text 문서 이름이 비어 있음"));
    }

    let mut serialized = Vec::with_capacity(document.source_len);
    let mut cursor = 0usize;
    for chunk in &document.chunks {
        if chunk.offset() != cursor {
            return Err(error(format!(
                "{}: chunk 비연속 또는 겹침: expected=0x{cursor:04X}, actual=0x{:04X}",
                document.sequence,
                chunk.offset()
            )));
        }
        match chunk {
            QuestSeqTextDocumentChunk::Opaque { raw, .. } => {
                if raw.is_empty() {
                    return Err(error(format!(
                        "{}: 빈 opaque chunk는 허용하지 않음",
                        document.sequence
                    )));
                }
                serialized.extend_from_slice(raw);
            }
            QuestSeqTextDocumentChunk::TextStream(stream) => {
                if stream.sequence != document.sequence {
                    return Err(error(format!(
                        "{}: 다른 SEQ의 stream chunk {}",
                        document.sequence,
                        stream.id()
                    )));
                }
                let raw = serialize_quest_seq_extracted_text_stream(stream)
                    .map_err(|source| error(format!("{}: {source}", document.sequence)))?;
                serialized.extend_from_slice(&raw);
            }
        }
        cursor = cursor
            .checked_add(chunk.byte_len()?)
            .ok_or_else(|| error(format!("{}: chunk cursor overflow", document.sequence)))?;
    }

    if cursor != document.source_len || serialized.len() != document.source_len {
        return Err(error(format!(
            "{}: 문서 길이 불일치: expected={}, cursor={cursor}, actual={}",
            document.sequence,
            document.source_len,
            serialized.len()
        )));
    }
    let actual_digest = digest(&serialized);
    if actual_digest != document.source_sha256 {
        return Err(error(format!(
            "{}: 문서 SHA-256 불일치: expected={}, actual={}",
            document.sequence,
            hex(&document.source_sha256),
            hex(&actual_digest)
        )));
    }
    Ok(serialized)
}

/// 원본 SEQ와 검증된 OP21 추출 결과를 전 파일 lossless 문서로 결합한다.
pub fn audit_quest_seq_text_documents<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    extraction: &QuestSeqTextExtractionAnalysis,
) -> Result<QuestSeqTextDocumentAnalysis, QuestSeqTextDocumentError> {
    let mut input = BTreeMap::<String, &'a [u8]>::new();
    for (name, raw) in sequences {
        if name.is_empty() {
            return Err(error("QUEST text 문서 입력 이름이 비어 있음"));
        }
        if input.insert(name.to_string(), raw).is_some() {
            return Err(error(format!("QUEST text 문서 입력 중복: {name}")));
        }
    }
    if input.is_empty() {
        return Err(error("QUEST text 문서 입력이 없음"));
    }

    let source_bytes = input.values().map(|raw| raw.len()).sum::<usize>();
    if extraction.sequences != input.len() || extraction.source_bytes != source_bytes {
        return Err(error(format!(
            "QUEST text 추출과 문서 입력 분모 불일치: \
             extraction={}/{}, input={}/{}",
            extraction.sequences,
            extraction.source_bytes,
            input.len(),
            source_bytes
        )));
    }

    let mut streams_by_sequence = BTreeMap::<String, Vec<&QuestSeqExtractedTextStream>>::new();
    for stream in &extraction.streams {
        if !input.contains_key(&stream.sequence) {
            return Err(error(format!(
                "{}: 추출 stream의 원본 SEQ가 문서 입력에 없음",
                stream.id()
            )));
        }
        streams_by_sequence
            .entry(stream.sequence.clone())
            .or_default()
            .push(stream);
    }

    let mut documents = Vec::with_capacity(input.len());
    for (sequence, raw) in input {
        let source_sha256 = digest(raw);
        let mut streams = streams_by_sequence.remove(&sequence).unwrap_or_default();
        streams.sort_by_key(|stream| stream.record_offset);

        let mut cursor = 0usize;
        let mut chunks = Vec::with_capacity(streams.len().saturating_mul(2).saturating_add(1));
        for stream in streams {
            if stream.sequence_sha256 != source_sha256 {
                return Err(error(format!(
                    "{}: stream의 source SHA-256이 문서 원본과 다름",
                    stream.id()
                )));
            }
            if stream.record_offset < cursor {
                return Err(error(format!("{}: 앞 stream과 범위가 겹침", stream.id())));
            }
            if cursor < stream.record_offset {
                let opaque = raw
                    .get(cursor..stream.record_offset)
                    .ok_or_else(|| error(format!("{sequence}: opaque chunk 범위가 원본 밖임")))?;
                chunks.push(QuestSeqTextDocumentChunk::Opaque {
                    offset: cursor,
                    raw: opaque.to_vec(),
                });
            }
            let end_offset = stream.end_offset;
            let protected = raw
                .get(stream.record_offset..end_offset)
                .ok_or_else(|| error(format!("{}: stream 범위가 원본 밖임", stream.id())))?;
            let serialized = serialize_quest_seq_extracted_text_stream(stream)
                .map_err(|source| error(format!("{}: {source}", stream.id())))?;
            if serialized != protected {
                return Err(error(format!(
                    "{}: stream chunk가 원본 범위와 다름",
                    stream.id()
                )));
            }
            chunks.push(QuestSeqTextDocumentChunk::TextStream(stream.clone()));
            cursor = end_offset;
        }
        if cursor < raw.len() {
            chunks.push(QuestSeqTextDocumentChunk::Opaque {
                offset: cursor,
                raw: raw[cursor..].to_vec(),
            });
        }

        let document = QuestSeqTextDocument {
            sequence,
            source_len: raw.len(),
            source_sha256,
            chunks,
        };
        let serialized = serialize_quest_seq_text_document(&document)?;
        if serialized != raw {
            return Err(error(format!(
                "{}: 전 파일 roundtrip 원본 불일치",
                document.sequence
            )));
        }
        documents.push(document);
    }
    if !streams_by_sequence.is_empty() {
        return Err(error("QUEST text 문서에 배치하지 못한 stream이 남음"));
    }

    let chunks = documents.iter().map(|document| document.chunks.len()).sum();
    let opaque_chunks = documents
        .iter()
        .flat_map(|document| &document.chunks)
        .filter(|chunk| matches!(chunk, QuestSeqTextDocumentChunk::Opaque { .. }))
        .count();
    let opaque_bytes = documents
        .iter()
        .flat_map(|document| &document.chunks)
        .filter_map(|chunk| match chunk {
            QuestSeqTextDocumentChunk::Opaque { raw, .. } => Some(raw.len()),
            QuestSeqTextDocumentChunk::TextStream(_) => None,
        })
        .sum();
    let text_stream_chunks = documents
        .iter()
        .flat_map(|document| &document.chunks)
        .filter(|chunk| matches!(chunk, QuestSeqTextDocumentChunk::TextStream(_)))
        .count();
    let text_stream_bytes = documents
        .iter()
        .flat_map(|document| &document.chunks)
        .filter_map(|chunk| match chunk {
            QuestSeqTextDocumentChunk::Opaque { .. } => None,
            QuestSeqTextDocumentChunk::TextStream(stream) => stream.byte_len(),
        })
        .sum();

    Ok(QuestSeqTextDocumentAnalysis {
        documents,
        source_bytes,
        chunks,
        opaque_chunks,
        opaque_bytes,
        text_stream_chunks,
        text_stream_bytes,
    })
}

/// 고정 지원 원본 113개의 문서 분모와 report digest를 검증한다.
pub fn verify_box_quest_seq_text_documents<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    extraction: &QuestSeqTextExtractionAnalysis,
) -> Result<QuestSeqTextDocumentAnalysis, QuestSeqTextDocumentError> {
    let analysis = audit_quest_seq_text_documents(sequences, extraction)?;
    let actual = [
        analysis.documents.len(),
        analysis.source_bytes,
        analysis.chunks,
        analysis.opaque_chunks,
        analysis.opaque_bytes,
        analysis.text_stream_chunks,
        analysis.text_stream_bytes,
    ];
    let expected = [
        BOX_QUEST_SEQ_TEXT_DOCUMENTS,
        BOX_QUEST_SEQ_TEXT_DOCUMENT_BYTES,
        BOX_QUEST_SEQ_TEXT_DOCUMENT_OPAQUE_CHUNKS + BOX_QUEST_SEQ_TEXT_DOCUMENT_STREAM_CHUNKS,
        BOX_QUEST_SEQ_TEXT_DOCUMENT_OPAQUE_CHUNKS,
        BOX_QUEST_SEQ_TEXT_DOCUMENT_OPAQUE_BYTES,
        BOX_QUEST_SEQ_TEXT_DOCUMENT_STREAM_CHUNKS,
        BOX_QUEST_SEQ_TEXT_DOCUMENT_STREAM_BYTES,
    ];
    if actual != expected {
        return Err(error(format!(
            "QUEST text 문서 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }
    if [
        analysis.documents.len(),
        analysis.source_bytes,
        analysis.text_stream_chunks,
        analysis.text_stream_bytes,
    ] != [
        BOX_QUEST_SEQ_SEQUENCES,
        BOX_QUEST_SEQ_BYTES,
        BOX_QUEST_SEQ_EXTRACTED_TEXT_STREAMS,
        BOX_QUEST_SEQ_EXTRACTED_ROUNDTRIP_BYTES,
    ] {
        return Err(error("QUEST text 문서와 기존 추출 계약의 분모가 다름"));
    }

    let report = render_quest_seq_text_document_summary_tsv(&analysis);
    let actual_digest = format!("{:x}", Sha256::digest(report.as_bytes()));
    if actual_digest != BOX_QUEST_SEQ_TEXT_DOCUMENT_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST text 문서 report SHA-256 불일치: \
             expected={BOX_QUEST_SEQ_TEXT_DOCUMENT_REPORT_SHA256}, actual={actual_digest}"
        )));
    }
    Ok(analysis)
}

/// 113개 전 파일의 chunk 소유권과 roundtrip 경계를 요약한다.
pub fn render_quest_seq_text_document_summary_tsv(
    analysis: &QuestSeqTextDocumentAnalysis,
) -> String {
    let mut output = String::from(
        "schema_version\tsequence_name\tsource_bytes\tsource_sha256\tchunks\t\
         opaque_chunks\topaque_bytes\ttext_stream_chunks\ttext_stream_bytes\t\
         roundtrip_state\n",
    );
    for document in &analysis.documents {
        let opaque_chunks = document
            .chunks
            .iter()
            .filter(|chunk| matches!(chunk, QuestSeqTextDocumentChunk::Opaque { .. }))
            .count();
        let opaque_bytes = document
            .chunks
            .iter()
            .filter_map(|chunk| match chunk {
                QuestSeqTextDocumentChunk::Opaque { raw, .. } => Some(raw.len()),
                QuestSeqTextDocumentChunk::TextStream(_) => None,
            })
            .sum::<usize>();
        let text_stream_chunks = document.chunks.len() - opaque_chunks;
        let text_stream_bytes = document.source_len - opaque_bytes;
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tbyte-identical",
            QUEST_SEQ_TEXT_DOCUMENT_SCHEMA_VERSION,
            document.sequence,
            document.source_len,
            hex(&document.source_sha256),
            document.chunks.len(),
            opaque_chunks,
            opaque_bytes,
            text_stream_chunks,
            text_stream_bytes,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_seq_text_document_tests.rs"]
mod tests;
