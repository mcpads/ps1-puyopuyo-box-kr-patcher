//! PUYO2 `E6` 원문 대조형 번역 manifest.
//!
//! 편집 가능한 한국어 초안 옆에 지원 원본의 SEQ·stream·span identity와
//! 보호 원문을 둔다. 현재 ROM 분석과 전수 대조되지 않는 행은 번역 계획으로
//! 넘기지 않는다. 초안 검산과 제품 승인을 구분하며, 모든 행이 `approved`가
//! 아니면 제품용 번역 집합을 반환하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::disasm::SubgameSeqE6TextAnalysis;

use super::SubgameSeqE6Translation;

pub const PUYO2_E6_TRANSLATION_MANIFEST_SCHEMA_VERSION: &str = "puyo2-e6-translation-manifest-v1";

const PUYO2_SCOPE: &str = "PUYO2";
const PUYO2_ARCHIVE: &str = "/PUYO2/PP2_SEQ.VDD;1";
const MANIFEST: &str = include_str!("../../assets/translations/puyo2_e6.tsv");
const HEADER: &str = "schema_version\tstatus\tarchive\tsequence\tsequence_sha256\tsequence_index\tstream_offset\traw_sha256\tspan_index\tspan_offset\tcandidate_id\tprotected_source\tko\tnotes";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Puyo2E6TranslationStatus {
    DraftNeedsHumanReview,
    Approved,
}

impl Puyo2E6TranslationStatus {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "draft-needs-human-review" => Some(Self::DraftNeedsHumanReview),
            "approved" => Some(Self::Approved),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::DraftNeedsHumanReview => "draft-needs-human-review",
            Self::Approved => "approved",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6TranslationManifestEntry {
    pub status: Puyo2E6TranslationStatus,
    pub archive: String,
    pub sequence: String,
    pub sequence_sha256: String,
    pub sequence_index: usize,
    pub stream_offset: usize,
    pub raw_sha256: String,
    pub span_index: usize,
    pub span_offset: usize,
    pub candidate_id: String,
    pub protected_source: String,
    pub ko: String,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6TranslationManifestAnalysis {
    pub entries: Vec<Puyo2E6TranslationManifestEntry>,
    pub translations: Vec<SubgameSeqE6Translation>,
    pub sequences: usize,
    pub streams: usize,
    pub spans: usize,
    pub protected_source_code_units: usize,
    pub translated_code_units: usize,
    pub draft_needs_human_review: usize,
    pub approved: usize,
    pub product_eligible: bool,
    pub manifest_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2E6TranslationManifestError(String);

impl fmt::Display for Puyo2E6TranslationManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Puyo2E6TranslationManifestError {}

fn error(message: impl Into<String>) -> Puyo2E6TranslationManifestError {
    Puyo2E6TranslationManifestError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn digest_array(raw: &[u8; 32]) -> String {
    raw.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_usize(
    raw: &str,
    field: &str,
    line: usize,
) -> Result<usize, Puyo2E6TranslationManifestError> {
    raw.parse::<usize>()
        .map_err(|_| error(format!("PUYO2 E6 manifest {field} 숫자 아님: line={line}")))
}

fn parse_offset(
    raw: &str,
    field: &str,
    line: usize,
) -> Result<usize, Puyo2E6TranslationManifestError> {
    raw.strip_prefix("0x")
        .and_then(|digits| usize::from_str_radix(digits, 16).ok())
        .ok_or_else(|| {
            error(format!(
                "PUYO2 E6 manifest {field} hex offset 아님: line={line}"
            ))
        })
}

fn contains_japanese(raw: &str) -> bool {
    raw.chars().any(|glyph| {
        matches!(
            glyph,
            '\u{3040}'..='\u{30ff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{4e00}'..='\u{9fff}'
                | '\u{ff65}'..='\u{ff9f}'
        )
    })
}

fn parse_manifest(
    raw: &str,
) -> Result<Vec<Puyo2E6TranslationManifestEntry>, Puyo2E6TranslationManifestError> {
    let mut lines = raw.lines();
    if lines.next().unwrap_or_default() != HEADER {
        return Err(error("PUYO2 E6 translation manifest header 불일치"));
    }
    let mut entries = Vec::new();
    let mut ids = BTreeSet::new();
    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        if line.is_empty() {
            return Err(error(format!(
                "PUYO2 E6 translation manifest 빈 행: line={line_number}"
            )));
        }
        let columns = line.split('\t').collect::<Vec<_>>();
        let [schema_version, status, archive, sequence, sequence_sha256, sequence_index, stream_offset, raw_sha256, span_index, span_offset, candidate_id, protected_source, ko, notes] =
            columns.as_slice()
        else {
            return Err(error(format!(
                "PUYO2 E6 translation manifest 열 수 불일치: line={line_number}"
            )));
        };
        if *schema_version != PUYO2_E6_TRANSLATION_MANIFEST_SCHEMA_VERSION {
            return Err(error(format!(
                "PUYO2 E6 translation manifest schema 불일치: line={line_number}"
            )));
        }
        let status = Puyo2E6TranslationStatus::parse(status).ok_or_else(|| {
            error(format!(
                "PUYO2 E6 translation manifest status 미지원: line={line_number}, value={status}"
            ))
        })?;
        if archive.is_empty()
            || sequence.is_empty()
            || sequence_sha256.len() != 64
            || raw_sha256.len() != 64
            || candidate_id.is_empty()
            || protected_source.is_empty()
            || ko.is_empty()
        {
            return Err(error(format!(
                "PUYO2 E6 translation manifest 필수값 누락: line={line_number}"
            )));
        }
        if contains_japanese(ko) {
            return Err(error(format!(
                "PUYO2 E6 translation manifest 한국어에 일본어 잔존: line={line_number}"
            )));
        }
        let sequence_index = parse_usize(sequence_index, "sequence_index", line_number)?;
        let span_index = parse_usize(span_index, "span_index", line_number)?;
        let stream_offset = parse_offset(stream_offset, "stream_offset", line_number)?;
        let span_offset = parse_offset(span_offset, "span_offset", line_number)?;
        let id = ((*sequence).to_string(), sequence_index, span_index);
        if !ids.insert(id.clone()) {
            return Err(error(format!(
                "PUYO2 E6 translation manifest span ID 중복: {}#{}#{}",
                id.0, id.1, id.2
            )));
        }
        entries.push(Puyo2E6TranslationManifestEntry {
            status,
            archive: (*archive).to_string(),
            sequence: (*sequence).to_string(),
            sequence_sha256: (*sequence_sha256).to_string(),
            sequence_index,
            stream_offset,
            raw_sha256: (*raw_sha256).to_string(),
            span_index,
            span_offset,
            candidate_id: (*candidate_id).to_string(),
            protected_source: (*protected_source).to_string(),
            ko: (*ko).to_string(),
            notes: (*notes).to_string(),
        });
    }
    if entries.is_empty() {
        return Err(error("PUYO2 E6 translation manifest가 비어 있음"));
    }
    Ok(entries)
}

fn verify_manifest(
    raw: &str,
    text: &SubgameSeqE6TextAnalysis,
) -> Result<Puyo2E6TranslationManifestAnalysis, Puyo2E6TranslationManifestError> {
    let actual_sha256 = digest(raw.as_bytes());
    let mut entries = parse_manifest(raw)?;
    entries.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then(left.sequence_index.cmp(&right.sequence_index))
            .then(left.span_index.cmp(&right.span_index))
    });
    let mut rows = text
        .rows
        .iter()
        .filter(|row| row.scope == PUYO2_SCOPE)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| (row.sequence.as_str(), row.sequence_index));
    if rows.is_empty() {
        return Err(error("PUYO2 E6 translation manifest 원본 row가 없음"));
    }
    let expected_entries = rows.iter().map(|row| row.spans.len()).sum::<usize>();
    if entries.len() != expected_entries {
        return Err(error(format!(
            "PUYO2 E6 translation manifest span 분모 불일치: expected={expected_entries}, actual={}",
            entries.len()
        )));
    }

    let by_id = entries
        .iter()
        .map(|entry| {
            (
                (
                    entry.sequence.as_str(),
                    entry.sequence_index,
                    entry.span_index,
                ),
                entry,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut translations = Vec::with_capacity(rows.len());
    for row in &rows {
        let sequence_sha256 = digest_array(&row.sequence_sha256);
        let raw_sha256 = digest_array(&row.raw_sha256);
        let mut spans = Vec::with_capacity(row.spans.len());
        for (index, span) in row.spans.iter().enumerate() {
            let span_index = index + 1;
            let entry = by_id
                .get(&(row.sequence.as_str(), row.sequence_index, span_index))
                .ok_or_else(|| {
                    error(format!(
                        "PUYO2 E6 translation manifest span 누락: {}#{}#{}",
                        row.sequence, row.sequence_index, span_index
                    ))
                })?;
            if entry.archive != PUYO2_ARCHIVE
                || entry.sequence_sha256 != sequence_sha256
                || entry.stream_offset != row.stream_offset
                || entry.raw_sha256 != raw_sha256
                || entry.span_offset != span.offset
                || entry.candidate_id != span.candidate_id
                || entry.protected_source != span.decoded
            {
                return Err(error(format!(
                    "PUYO2 E6 translation manifest 보호 원문/identity drift: {}#{}#{}",
                    row.sequence, row.sequence_index, span_index
                )));
            }
            spans.push(entry.ko.clone());
        }
        translations.push(SubgameSeqE6Translation {
            sequence: row.sequence.clone(),
            sequence_index: row.sequence_index,
            spans,
        });
    }

    let streams = translations.len();
    let sequences = translations
        .iter()
        .map(|entry| entry.sequence.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let draft_needs_human_review = entries
        .iter()
        .filter(|entry| entry.status == Puyo2E6TranslationStatus::DraftNeedsHumanReview)
        .count();
    let approved = entries.len() - draft_needs_human_review;
    Ok(Puyo2E6TranslationManifestAnalysis {
        protected_source_code_units: entries
            .iter()
            .map(|entry| entry.protected_source.chars().count())
            .sum(),
        translated_code_units: entries.iter().map(|entry| entry.ko.chars().count()).sum(),
        product_eligible: approved == entries.len(),
        spans: entries.len(),
        entries,
        translations,
        sequences,
        streams,
        draft_needs_human_review,
        approved,
        manifest_sha256: actual_sha256,
    })
}

/// 지원 원본 분석과 version 관리 E6 manifest를 전수 대조한다.
pub fn verify_box_puyo2_e6_translation_manifest(
    text: &SubgameSeqE6TextAnalysis,
) -> Result<Puyo2E6TranslationManifestAnalysis, Puyo2E6TranslationManifestError> {
    let analysis = verify_manifest(MANIFEST, text)?;
    if render_box_puyo2_e6_translation_manifest_tsv(&analysis) != MANIFEST {
        return Err(error(
            "PUYO2 E6 translation manifest가 canonical 순서/표현이 아님",
        ));
    }
    Ok(analysis)
}

/// 정적 용량·repertoire 검토에만 쓰는 미승인 번역 집합을 반환한다.
pub fn review_box_puyo2_e6_translations(
    text: &SubgameSeqE6TextAnalysis,
) -> Result<Vec<SubgameSeqE6Translation>, Puyo2E6TranslationManifestError> {
    Ok(verify_box_puyo2_e6_translation_manifest(text)?.translations)
}

/// 모든 span이 사람 승인된 경우에만 제품용 번역 집합을 반환한다.
pub fn approved_box_puyo2_e6_translations(
    text: &SubgameSeqE6TextAnalysis,
) -> Result<Vec<SubgameSeqE6Translation>, Puyo2E6TranslationManifestError> {
    let analysis = verify_box_puyo2_e6_translation_manifest(text)?;
    if !analysis.product_eligible {
        return Err(error(format!(
            "PUYO2 E6 translation manifest 사람 승인 미완료: approved={}, total={}",
            analysis.approved, analysis.spans
        )));
    }
    Ok(analysis.translations)
}

/// 보호 원문과 한국어 초안을 같은 행에 둔 결정적 검토 TSV를 만든다.
pub fn render_box_puyo2_e6_translation_manifest_tsv(
    analysis: &Puyo2E6TranslationManifestAnalysis,
) -> String {
    let mut output = format!("{HEADER}\n");
    for entry in &analysis.entries {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t{}\t{}\t0x{:04X}\t{}\t{}\t{}\t{}",
            PUYO2_E6_TRANSLATION_MANIFEST_SCHEMA_VERSION,
            entry.status.label(),
            entry.archive,
            entry.sequence,
            entry.sequence_sha256,
            entry.sequence_index,
            entry.stream_offset,
            entry.raw_sha256,
            entry.span_index,
            entry.span_offset,
            entry.candidate_id,
            entry.protected_source,
            entry.ko,
            entry.notes,
        )
        .expect("String 쓰기는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "subgame_seq_e6_translation_manifest_tests.rs"]
mod tests;
