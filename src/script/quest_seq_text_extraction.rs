//! OP21 text stream 후보의 token 보존형 추출 계약.
//!
//! `quest_seq_text_streams`가 source shape와 lifecycle 종단을 통과시킨
//! stream만 입력으로 삼는다. 각 OP21 record와 glyph/control token의
//! 원본 좌표·raw byte·의미를 보존하고, 다시 직렬화한 바이트가 원본의
//! 같은 범위와 정확히 같아야 승인한다.
//!
//! 이 계약은 정적 후보의 가역 추출을 증명하지만 정상 실행 도달이나 section
//! 소유권을 증명하지 않는다. 따라서 report는 번역 제품 입력이 아니다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::font::{decode_quest_source_glyph, QuestCodeTable};

use super::quest::{
    parse_quest_token, parse_quest_tokens, quest_control_meaning, QuestControlMeaning, QuestToken,
};
use super::quest_seq_inventory::{BOX_QUEST_SEQ_BYTES, BOX_QUEST_SEQ_SEQUENCES};
use super::quest_seq_text_streams::{
    audit_quest_seq_text_streams, QuestTextTerminal, BOX_QUEST_SEQ_PARSEABLE_TEXT_STREAMS,
    QUEST_SEQ_TEXT_MODE_OPCODE, QUEST_SEQ_TEXT_MODE_RECORD_BYTES,
};

pub const QUEST_SEQ_TEXT_TOKEN_LEDGER_SCHEMA_VERSION: &str = "quest-seq-text-token-ledger-v2";
pub const BOX_QUEST_SEQ_EXTRACTED_TEXT_STREAMS: usize = 605;
pub const BOX_QUEST_SEQ_EXTRACTED_TOKEN_OCCURRENCES: usize = 40_948;
pub const BOX_QUEST_SEQ_EXTRACTED_GLYPH_OCCURRENCES: usize = 31_892;
pub const BOX_QUEST_SEQ_EXTRACTED_CONTROL_OCCURRENCES: usize = 9_056;
pub const BOX_QUEST_SEQ_EXTRACTED_ROUNDTRIP_BYTES: usize = 98_516;
pub const BOX_QUEST_SEQ_TEXT_TOKEN_LEDGER_SHA256: &str =
    "4dff1b32c6ac187c3e2de45011728c8e73ad3f14846a66fa29fae2157083f127";

/// 추출 token의 소비 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqExtractedTokenKind {
    Glyph,
    Control,
}

impl QuestSeqExtractedTokenKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Glyph => "glyph",
            Self::Control => "control",
        }
    }

    pub const fn policy(self) -> &'static str {
        match self {
            Self::Glyph => "translation-candidate",
            Self::Control => "preserve-raw",
        }
    }
}

/// OP21 stream 안에서 원본 좌표와 raw byte를 보호하는 token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqExtractedToken {
    pub index: usize,
    pub offset: usize,
    pub raw: Vec<u8>,
    pub kind: QuestSeqExtractedTokenKind,
    /// 원본 code와 1:1인 단일 의미 문자로 표현되는 glyph만 채운다.
    pub source: Option<String>,
    pub control_code: Option<u8>,
    pub control_meaning: Option<QuestControlMeaning>,
}

/// OP21 record부터 lifecycle 종단까지의 가역 추출 단위.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqExtractedTextStream {
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub record_offset: usize,
    pub record_raw: [u8; QUEST_SEQ_TEXT_MODE_RECORD_BYTES],
    pub stream_offset: usize,
    pub end_offset: usize,
    pub terminal: QuestTextTerminal,
    pub expected_raw_sha256: [u8; 32],
    pub tokens: Vec<QuestSeqExtractedToken>,
}

impl QuestSeqExtractedTextStream {
    pub fn id(&self) -> String {
        format!(
            "QUEST-SEQ-TEXT-STREAM:{}:0x{:04X}",
            self.sequence, self.record_offset
        )
    }

    pub const fn byte_len(&self) -> Option<usize> {
        self.end_offset.checked_sub(self.record_offset)
    }
}

/// 지원 원본의 token 보존형 정적 추출 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextExtractionAnalysis {
    pub sequences: usize,
    pub source_bytes: usize,
    pub streams: Vec<QuestSeqExtractedTextStream>,
    pub token_occurrences: usize,
    pub glyph_occurrences: usize,
    pub control_occurrences: usize,
    pub roundtrip_bytes: usize,
}

/// token 보호 정보·원본 범위·결정적 분모 불일치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextExtractionError(String);

impl fmt::Display for QuestSeqTextExtractionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestSeqTextExtractionError {}

fn error(message: impl Into<String>) -> QuestSeqTextExtractionError {
    QuestSeqTextExtractionError(message.into())
}

fn digest(raw: &[u8]) -> [u8; 32] {
    Sha256::digest(raw).into()
}

fn decode_single_glyph(raw: &[u8]) -> Option<String> {
    let code: [u8; 2] = raw.try_into().ok()?;
    decode_quest_source_glyph(code).map(|character| character.to_string())
}

fn validate_token(token: &QuestSeqExtractedToken) -> Result<(), QuestSeqTextExtractionError> {
    let address = u32::try_from(token.offset)
        .map_err(|_| error("QUEST 추출 token offset이 u32 범위를 넘음"))?;
    let parsed = parse_quest_token(&token.raw, address)
        .map_err(|source| error(format!("QUEST 추출 token 재파싱 실패: {source}")))?;
    if parsed.raw().len() != token.raw.len() {
        return Err(error(format!(
            "QUEST 추출 token raw에 다음 token 바이트가 섞임: offset=0x{:04X}",
            token.offset
        )));
    }
    match parsed {
        QuestToken::Glyph(raw) => {
            if token.kind != QuestSeqExtractedTokenKind::Glyph
                || token.control_code.is_some()
                || token.control_meaning.is_some()
                || token.source != decode_single_glyph(raw)
            {
                return Err(error(format!(
                    "QUEST 추출 glyph 보호 정보 불일치: offset=0x{:04X}",
                    token.offset
                )));
            }
        }
        QuestToken::Control { spec, .. } => {
            let meaning = quest_control_meaning(spec.code).ok_or_else(|| {
                error(format!("QUEST 추출 control 의미 누락: FF{:02X}", spec.code))
            })?;
            if token.kind != QuestSeqExtractedTokenKind::Control
                || token.source.is_some()
                || token.control_code != Some(spec.code)
                || token.control_meaning != Some(meaning)
            {
                return Err(error(format!(
                    "QUEST 추출 control 보호 정보 불일치: offset=0x{:04X}",
                    token.offset
                )));
            }
        }
    }
    Ok(())
}

/// 보호 token을 OP21 record와 함께 다시 직렬화한다.
pub fn serialize_quest_seq_extracted_text_stream(
    stream: &QuestSeqExtractedTextStream,
) -> Result<Vec<u8>, QuestSeqTextExtractionError> {
    if stream.record_raw != [QUEST_SEQ_TEXT_MODE_OPCODE, 0, 0, 0] {
        return Err(error(format!(
            "{}: canonical OP21 record가 아님",
            stream.id()
        )));
    }
    let expected_stream_offset = stream
        .record_offset
        .checked_add(QUEST_SEQ_TEXT_MODE_RECORD_BYTES)
        .ok_or_else(|| error(format!("{}: stream offset overflow", stream.id())))?;
    if stream.stream_offset != expected_stream_offset {
        return Err(error(format!(
            "{}: record와 stream이 인접하지 않음",
            stream.id()
        )));
    }
    if stream.tokens.is_empty() {
        return Err(error(format!("{}: 추출 token이 없음", stream.id())));
    }

    let byte_len = stream
        .byte_len()
        .ok_or_else(|| error(format!("{}: stream 종단이 record보다 앞섬", stream.id())))?;
    let mut serialized = Vec::with_capacity(byte_len);
    serialized.extend_from_slice(&stream.record_raw);
    let mut cursor = stream.stream_offset;
    for (expected_index, token) in stream.tokens.iter().enumerate() {
        if token.index != expected_index || token.offset != cursor {
            return Err(error(format!(
                "{}: token index·offset 비연속: index={}, offset=0x{:04X}",
                stream.id(),
                token.index,
                token.offset
            )));
        }
        validate_token(token)?;
        serialized.extend_from_slice(&token.raw);
        cursor = cursor
            .checked_add(token.raw.len())
            .ok_or_else(|| error(format!("{}: token cursor overflow", stream.id())))?;
    }
    if cursor != stream.end_offset {
        return Err(error(format!(
            "{}: token 종단 불일치: expected=0x{:04X}, actual=0x{cursor:04X}",
            stream.id(),
            stream.end_offset
        )));
    }
    if digest(&serialized) != stream.expected_raw_sha256 {
        return Err(error(format!("{}: roundtrip SHA-256 불일치", stream.id())));
    }
    Ok(serialized)
}

/// 임의 SEQ 집합에서 parseable OP21 stream을 token 보존형으로 추출한다.
pub fn audit_quest_seq_text_extraction<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    code_table: &QuestCodeTable,
) -> Result<QuestSeqTextExtractionAnalysis, QuestSeqTextExtractionError> {
    let mut input = BTreeMap::<String, &'a [u8]>::new();
    for (name, raw) in sequences {
        if name.is_empty() {
            return Err(error("QUEST token 추출 입력 이름이 비어 있음"));
        }
        if input.insert(name.to_string(), raw).is_some() {
            return Err(error(format!("QUEST token 추출 입력 중복: {name}")));
        }
    }
    if input.is_empty() {
        return Err(error("QUEST token 추출 입력이 없음"));
    }

    let source_bytes = input.values().map(|raw| raw.len()).sum();
    let stream_analysis = audit_quest_seq_text_streams(
        input.iter().map(|(name, raw)| (name.as_str(), *raw)),
        code_table,
    )
    .map_err(|source| error(format!("QUEST OP21 stream 분석 실패: {source}")))?;

    let mut streams = Vec::new();
    for marker in stream_analysis
        .markers
        .iter()
        .filter(|marker| marker.is_parseable())
    {
        let raw = input
            .get(&marker.sequence)
            .copied()
            .ok_or_else(|| error(format!("{}: 원본 SEQ 누락", marker.sequence)))?;
        let end_offset = marker
            .end_offset
            .ok_or_else(|| error("parseable OP21 stream에 end offset이 없음"))?;
        let terminal = marker
            .terminal
            .ok_or_else(|| error("parseable OP21 stream에 terminal이 없음"))?;
        let protected = raw
            .get(marker.record_offset..end_offset)
            .ok_or_else(|| error(format!("{}: OP21 stream 원본 범위 이탈", marker.sequence)))?;
        let token_raw = raw
            .get(marker.stream_offset..end_offset)
            .ok_or_else(|| error(format!("{}: token stream 원본 범위 이탈", marker.sequence)))?;
        let stream_address = u32::try_from(marker.stream_offset)
            .map_err(|_| error("QUEST token stream offset이 u32 범위를 넘음"))?;
        let parsed = parse_quest_tokens(token_raw, stream_address).map_err(|source| {
            error(format!(
                "{}: token 전수 파싱 실패: {source}",
                marker.sequence
            ))
        })?;

        let mut cursor = marker.stream_offset;
        let mut tokens = Vec::with_capacity(parsed.len());
        for (index, parsed_token) in parsed.into_iter().enumerate() {
            let raw = parsed_token.raw().to_vec();
            let (kind, source, control_code, control_meaning) = match parsed_token {
                QuestToken::Glyph(raw) => (
                    QuestSeqExtractedTokenKind::Glyph,
                    decode_single_glyph(raw),
                    None,
                    None,
                ),
                QuestToken::Control { spec, .. } => {
                    let meaning = quest_control_meaning(spec.code).ok_or_else(|| {
                        error(format!(
                            "{}: 확인된 control 의미 누락 FF{:02X}",
                            marker.sequence, spec.code
                        ))
                    })?;
                    (
                        QuestSeqExtractedTokenKind::Control,
                        None,
                        Some(spec.code),
                        Some(meaning),
                    )
                }
            };
            tokens.push(QuestSeqExtractedToken {
                index,
                offset: cursor,
                raw,
                kind,
                source,
                control_code,
                control_meaning,
            });
            cursor = cursor
                .checked_add(tokens.last().expect("방금 추가한 token").raw.len())
                .ok_or_else(|| error("QUEST token cursor overflow"))?;
        }

        let stream = QuestSeqExtractedTextStream {
            sequence: marker.sequence.clone(),
            sequence_sha256: marker.sequence_sha256,
            record_offset: marker.record_offset,
            record_raw: marker.record_raw,
            stream_offset: marker.stream_offset,
            end_offset,
            terminal,
            expected_raw_sha256: digest(protected),
            tokens,
        };
        let serialized = serialize_quest_seq_extracted_text_stream(&stream)?;
        if serialized != protected {
            return Err(error(format!(
                "{}: OP21 token roundtrip 원본 불일치",
                stream.id()
            )));
        }
        streams.push(stream);
    }

    let token_occurrences = streams.iter().map(|stream| stream.tokens.len()).sum();
    let glyph_occurrences = streams
        .iter()
        .flat_map(|stream| &stream.tokens)
        .filter(|token| token.kind == QuestSeqExtractedTokenKind::Glyph)
        .count();
    let control_occurrences = token_occurrences - glyph_occurrences;
    let roundtrip_bytes = streams.iter().try_fold(0usize, |total, stream| {
        let byte_len = stream
            .byte_len()
            .ok_or_else(|| error(format!("{}: stream 길이 underflow", stream.id())))?;
        total
            .checked_add(byte_len)
            .ok_or_else(|| error("QUEST token roundtrip byte 합계 overflow"))
    })?;
    Ok(QuestSeqTextExtractionAnalysis {
        sequences: input.len(),
        source_bytes,
        streams,
        token_occurrences,
        glyph_occurrences,
        control_occurrences,
        roundtrip_bytes,
    })
}

/// 고정 지원 원본의 token 보존형 추출 분모와 report digest를 검증한다.
pub fn verify_box_quest_seq_text_extraction<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    code_table: &QuestCodeTable,
) -> Result<QuestSeqTextExtractionAnalysis, QuestSeqTextExtractionError> {
    let analysis = audit_quest_seq_text_extraction(sequences, code_table)?;
    let actual = [
        analysis.sequences,
        analysis.source_bytes,
        analysis.streams.len(),
        analysis.token_occurrences,
        analysis.glyph_occurrences,
        analysis.control_occurrences,
        analysis.roundtrip_bytes,
    ];
    let expected = [
        BOX_QUEST_SEQ_SEQUENCES,
        BOX_QUEST_SEQ_BYTES,
        BOX_QUEST_SEQ_EXTRACTED_TEXT_STREAMS,
        BOX_QUEST_SEQ_EXTRACTED_TOKEN_OCCURRENCES,
        BOX_QUEST_SEQ_EXTRACTED_GLYPH_OCCURRENCES,
        BOX_QUEST_SEQ_EXTRACTED_CONTROL_OCCURRENCES,
        BOX_QUEST_SEQ_EXTRACTED_ROUNDTRIP_BYTES,
    ];
    if actual != expected {
        return Err(error(format!(
            "QUEST token 보존형 추출 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }
    if analysis.streams.len() != BOX_QUEST_SEQ_PARSEABLE_TEXT_STREAMS {
        return Err(error(format!(
            "QUEST parseable stream과 추출 stream 분모 불일치: expected={}, actual={}",
            BOX_QUEST_SEQ_PARSEABLE_TEXT_STREAMS,
            analysis.streams.len()
        )));
    }

    let report = render_quest_seq_text_token_ledger_tsv(&analysis);
    let actual_digest = format!("{:x}", Sha256::digest(report.as_bytes()));
    if actual_digest != BOX_QUEST_SEQ_TEXT_TOKEN_LEDGER_SHA256 {
        return Err(error(format!(
            "QUEST token ledger SHA-256 불일치: \
             expected={BOX_QUEST_SEQ_TEXT_TOKEN_LEDGER_SHA256}, actual={actual_digest}"
        )));
    }
    Ok(analysis)
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

/// 보호 token을 OP21 owner 문맥과 함께 내보내는 결정적 TSV.
pub fn render_quest_seq_text_token_ledger_tsv(analysis: &QuestSeqTextExtractionAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tarchive_path\tarchive_sequence_count\tsequence_name\t\
         sequence_sha256\tstream_id\trecord_offset\trecord_raw_hex\tstream_offset\t\
         end_offset\tstream_sha256\tstream_state\ttoken_index\ttoken_offset\t\
         token_kind\ttoken_policy\traw_hex\tcontrol_code\tcontrol_meaning\tsource\n",
    );
    for stream in &analysis.streams {
        for token in &stream.tokens {
            let control_code = token
                .control_code
                .map(|code| format!("FF{code:02X}"))
                .unwrap_or_default();
            let control_meaning = token
                .control_meaning
                .map(|meaning| format!("{meaning:?}"))
                .unwrap_or_default();
            writeln!(
                output,
                "{}\t/QUEST/RPG_SEQ.VDD;1\t{}\t{}\t{}\t{}\t0x{:04X}\t{}\t\
                 0x{:04X}\t0x{:04X}\t{}\tstatic-op21-candidate\t{}\t0x{:04X}\t\
                 {}\t{}\t{}\t{}\t{}\t{}",
                QUEST_SEQ_TEXT_TOKEN_LEDGER_SCHEMA_VERSION,
                analysis.sequences,
                stream.sequence,
                hex(&stream.sequence_sha256),
                stream.id(),
                stream.record_offset,
                hex(&stream.record_raw),
                stream.stream_offset,
                stream.end_offset,
                hex(&stream.expected_raw_sha256),
                token.index,
                token.offset,
                token.kind.label(),
                token.kind.policy(),
                hex(&token.raw),
                control_code,
                control_meaning,
                escape_tsv(token.source.as_deref().unwrap_or_default()),
            )
            .expect("String write는 실패하지 않음");
        }
    }
    output
}

#[cfg(test)]
#[path = "quest_seq_text_extraction_tests.rs"]
mod tests;
