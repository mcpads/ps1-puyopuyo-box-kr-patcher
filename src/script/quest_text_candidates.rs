//! `TOWN00.SEQ`의 일본어 text run 정적 후보.
//!
//! 전체 SEQ의 token 시작·section 경계가 아직 닫히지 않았으므로 이 결과는
//! 번역 모집단이 아니다. u16 정렬, `QUEST.EXE` 코드표 포함, 엄격한
//! Shift-JIS 단일문자 decode를 모두 만족하는 연속 run만 사람이 검토할
//! 후보로 내보내며, 별도 runtime ledger와 정확히 일치하는 span만 관측
//! 증거로 구분한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::coords::RuntimeAssetMap;
use crate::font::QuestCodeTable;
use crate::text::decode_sjis;
use sha2::{Digest, Sha256};

use super::{
    verify_town00_seq_roundtrip, QuestDialogueSpan, QUEST_DIALOGUE_TRANSLATIONS, TOWN00_SEQ_LEN,
    TOWN00_SEQ_RUNTIME_BASE, TOWN00_SEQ_SHA256,
};

pub const TOWN00_TEXT_CANDIDATE_SCHEMA_VERSION: &str = "town00-text-candidates-v1";
pub const TOWN00_TEXT_CANDIDATE_MIN_CODE_UNITS: usize = 3;
pub const BOX_TOWN00_TEXT_CANDIDATES: usize = 465;
pub const BOX_TOWN00_RUNTIME_OBSERVED_TEXT_SPANS: usize = 8;
pub const BOX_TOWN00_STATIC_ONLY_TEXT_CANDIDATES: usize = 457;
pub const BOX_TOWN00_TEXT_CANDIDATE_CODE_UNITS: usize = 4_848;
pub const BOX_TOWN00_TEXT_CANDIDATE_UNIQUE_CHARACTERS: usize = 199;
pub const BOX_TOWN00_TEXT_CANDIDATE_REPORT_SHA256: &str =
    "1738fd8d7f70bb2b49ed4740a0f25cf33396ff65368f5accd74733ea91400b40";

/// 정적 후보에 붙일 수 있는 증거 수준.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Town00TextCandidateEvidence {
    /// runtime 화면과 state를 가진 typed ledger의 정확한 보호 span.
    RuntimeObservedSpan,
    /// 코드표와 decode 조건만 만족한 정적 run 후보.
    StaticCodeTableRunCandidate,
}

impl Town00TextCandidateEvidence {
    pub const fn label(self) -> &'static str {
        match self {
            Self::RuntimeObservedSpan => "runtime-observed-span",
            Self::StaticCodeTableRunCandidate => "static-code-table-run-candidate",
        }
    }
}

/// `TOWN00.SEQ` 안의 연속 text run 후보.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Town00TextCandidate {
    pub offset: usize,
    pub byte_len: usize,
    pub source: String,
    pub japanese_code_units: usize,
    pub evidence: Town00TextCandidateEvidence,
}

impl Town00TextCandidate {
    pub fn code_units(&self) -> usize {
        self.byte_len / 2
    }

    pub fn id(&self) -> String {
        format!("TOWN00-TEXT-0x{:04X}", self.offset)
    }
}

/// 정적 후보 report의 현재 분모.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Town00TextCandidateAnalysis {
    pub candidates: Vec<Town00TextCandidate>,
    pub runtime_observed_spans: usize,
    pub static_only_candidates: usize,
    pub source_code_units: usize,
    pub unique_source_characters: usize,
}

/// 원본·코드표·runtime ledger 사이의 불일치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Town00TextCandidateError(String);

impl fmt::Display for Town00TextCandidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Town00TextCandidateError {}

fn error(message: impl Into<String>) -> Town00TextCandidateError {
    Town00TextCandidateError(message.into())
}

fn is_japanese_character(character: char) -> bool {
    matches!(
        character,
        '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'
    )
}

fn decoded_code_table_pair(code_table: &QuestCodeTable, raw: &[u8]) -> Option<char> {
    let pair = [*raw.first()?, *raw.get(1)?];
    code_table.index_of(pair)?;
    let decoded = decode_sjis(&pair).ok()?;
    let mut characters = decoded.chars();
    let character = characters.next()?;
    characters.next().is_none().then_some(character)
}

fn runtime_spans() -> Vec<QuestDialogueSpan> {
    QUEST_DIALOGUE_TRANSLATIONS
        .iter()
        .flat_map(|entry| entry.spans.iter().copied())
        .collect()
}

/// QUEST 코드표로 decode할 수 있는 연속 일본어 run.
///
/// 아직 section·token 시작을 확정하지 않은 정적 후보이므로 이 타입 자체는
/// runtime 증거를 갖지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuestCodeTableTextRun {
    pub offset: usize,
    pub byte_len: usize,
    pub raw: Vec<u8>,
    pub source: String,
    pub japanese_code_units: usize,
}

pub(crate) fn scan_quest_code_table_text_runs(
    raw: &[u8],
    code_table: &QuestCodeTable,
) -> Vec<QuestCodeTableTextRun> {
    let mut runs = Vec::new();
    let mut offset = 0usize;

    while offset + 2 <= raw.len() {
        let start = offset;
        let mut source = String::new();
        let mut japanese_code_units = 0usize;

        while offset + 2 <= raw.len() {
            let Some(character) = decoded_code_table_pair(code_table, &raw[offset..offset + 2])
            else {
                break;
            };
            japanese_code_units += usize::from(is_japanese_character(character));
            source.push(character);
            offset += 2;
        }

        let byte_len = offset - start;
        if byte_len / 2 >= TOWN00_TEXT_CANDIDATE_MIN_CODE_UNITS && japanese_code_units > 0 {
            runs.push(QuestCodeTableTextRun {
                offset: start,
                byte_len,
                raw: raw[start..offset].to_vec(),
                source,
                japanese_code_units,
            });
        }

        if offset == start {
            offset += 2;
        }
    }

    runs
}

/// 지원 원본 `TOWN00`에서 코드표 기반 일본어 run 후보를 만든다.
pub fn audit_town00_text_candidates(
    raw: &[u8],
    code_table: &QuestCodeTable,
) -> Result<Town00TextCandidateAnalysis, Town00TextCandidateError> {
    verify_town00_seq_roundtrip(raw).map_err(|source| error(source.to_string()))?;
    let observed_spans = runtime_spans();
    let candidates = scan_quest_code_table_text_runs(raw, code_table)
        .into_iter()
        .map(|run| {
            let evidence = if observed_spans.iter().any(|span| {
                span.offset == run.offset
                    && span.protected_source == run.source
                    && crate::text::encode_sjis(span.protected_source)
                        .is_ok_and(|encoded| encoded == run.raw)
            }) {
                Town00TextCandidateEvidence::RuntimeObservedSpan
            } else {
                Town00TextCandidateEvidence::StaticCodeTableRunCandidate
            };
            Town00TextCandidate {
                offset: run.offset,
                byte_len: run.byte_len,
                source: run.source,
                japanese_code_units: run.japanese_code_units,
                evidence,
            }
        })
        .collect::<Vec<_>>();

    for span in &observed_spans {
        let found = candidates.iter().any(|candidate| {
            candidate.offset == span.offset
                && candidate.source == span.protected_source
                && candidate.evidence == Town00TextCandidateEvidence::RuntimeObservedSpan
        });
        if !found {
            return Err(error(format!(
                "runtime 확인 span이 정적 text 후보에서 사라짐: offset=0x{:04X}, source={}",
                span.offset, span.protected_source
            )));
        }
    }

    let runtime_observed_spans = candidates
        .iter()
        .filter(|candidate| candidate.evidence == Town00TextCandidateEvidence::RuntimeObservedSpan)
        .count();
    if runtime_observed_spans != observed_spans.len() {
        return Err(error(format!(
            "runtime 확인 span 분모 불일치: expected={}, actual={runtime_observed_spans}",
            observed_spans.len()
        )));
    }

    let source_code_units = candidates.iter().map(Town00TextCandidate::code_units).sum();
    let unique_source_characters = candidates
        .iter()
        .flat_map(|candidate| candidate.source.chars())
        .collect::<BTreeSet<_>>()
        .len();

    Ok(Town00TextCandidateAnalysis {
        static_only_candidates: candidates.len() - runtime_observed_spans,
        candidates,
        runtime_observed_spans,
        source_code_units,
        unique_source_characters,
    })
}

/// 지원 원본의 후보 분모와 결정적 report digest를 고정한다.
pub fn verify_box_town00_text_candidates(
    raw: &[u8],
    code_table: &QuestCodeTable,
) -> Result<Town00TextCandidateAnalysis, Town00TextCandidateError> {
    let analysis = audit_town00_text_candidates(raw, code_table)?;
    let actual = [
        analysis.candidates.len(),
        analysis.runtime_observed_spans,
        analysis.static_only_candidates,
        analysis.source_code_units,
        analysis.unique_source_characters,
    ];
    let expected = [
        BOX_TOWN00_TEXT_CANDIDATES,
        BOX_TOWN00_RUNTIME_OBSERVED_TEXT_SPANS,
        BOX_TOWN00_STATIC_ONLY_TEXT_CANDIDATES,
        BOX_TOWN00_TEXT_CANDIDATE_CODE_UNITS,
        BOX_TOWN00_TEXT_CANDIDATE_UNIQUE_CHARACTERS,
    ];
    if actual != expected {
        return Err(error(format!(
            "TOWN00 text 후보 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }

    let report = render_town00_text_candidates_tsv(&analysis);
    let digest = format!("{:x}", Sha256::digest(report.as_bytes()));
    if digest != BOX_TOWN00_TEXT_CANDIDATE_REPORT_SHA256 {
        return Err(error(format!(
            "TOWN00 text 후보 report SHA-256 불일치: expected={}, actual={digest}",
            BOX_TOWN00_TEXT_CANDIDATE_REPORT_SHA256
        )));
    }
    Ok(analysis)
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

/// 후보를 사람이 검토할 결정적 TSV로 렌더링한다.
pub fn render_town00_text_candidates_tsv(analysis: &Town00TextCandidateAnalysis) -> String {
    let map = RuntimeAssetMap::new(TOWN00_SEQ_RUNTIME_BASE, TOWN00_SEQ_LEN as u32)
        .expect("TOWN00 runtime 사상");
    let mut output = String::from(
        "schema_version\tasset_name\tasset_sha256\tcandidate_id\toffset\truntime_address\t\
         byte_len\tcode_units\tjapanese_code_units\tevidence\tsource\n",
    );
    for candidate in &analysis.candidates {
        let runtime_address = map
            .runtime_addr(candidate.offset)
            .expect("검증된 TOWN00 후보 offset");
        output.push_str(&format!(
            "{}\tTOWN00.SEQ\t{}\t{}\t0x{:04X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\n",
            TOWN00_TEXT_CANDIDATE_SCHEMA_VERSION,
            TOWN00_SEQ_SHA256,
            candidate.id(),
            candidate.offset,
            runtime_address,
            candidate.byte_len,
            candidate.code_units(),
            candidate.japanese_code_units,
            candidate.evidence.label(),
            escape_tsv(&candidate.source),
        ));
    }
    output
}

#[cfg(test)]
#[path = "quest_text_candidates_tests.rs"]
mod tests;
