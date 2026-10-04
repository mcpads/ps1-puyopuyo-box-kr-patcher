//! `RPG_SEQ.VDD` 113개 전체의 일본어 text run 정적 후보.
//!
//! 각 SEQ의 section·token 시작과 runtime 도달이 아직 전수 확정되지
//! 않았으므로 이 inventory는 번역 모집단이 아니다. 지원 원본 전체를 같은
//! 코드표 기반 조건으로 훑어 사람이 검토할 하한과 미확정 범위를 안정 ID,
//! 원본 digest·raw byte와 함께 보존한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::RuntimeAssetMap;
use crate::font::QuestCodeTable;
use crate::text::encode_sjis;

use super::quest_text_candidates::scan_quest_code_table_text_runs;
use super::{
    BOX_QUEST_SEQ_BYTES, BOX_QUEST_SEQ_SEQUENCES, QUEST_DIALOGUE_TRANSLATIONS, TOWN00_SEQ_LEN,
    TOWN00_SEQ_RUNTIME_BASE, TOWN00_SEQ_SHA256,
};

pub const QUEST_SEQ_TEXT_CANDIDATE_SCHEMA_VERSION: &str = "quest-seq-text-candidates-v1";
pub const QUEST_SEQ_TEXT_SUMMARY_SCHEMA_VERSION: &str = "quest-seq-text-summary-v1";
pub const BOX_QUEST_SEQ_TEXT_SEQUENCES: usize = BOX_QUEST_SEQ_SEQUENCES;
pub const BOX_QUEST_SEQ_TEXT_BYTES: usize = BOX_QUEST_SEQ_BYTES;
pub const BOX_QUEST_SEQ_TEXT_FILES_WITH_CANDIDATES: usize = 13;
pub const BOX_QUEST_SEQ_TEXT_FILES_WITHOUT_CANDIDATES: usize = 100;
pub const BOX_QUEST_SEQ_TEXT_CANDIDATES: usize = 3_720;
pub const BOX_QUEST_SEQ_RUNTIME_OBSERVED_TEXT_SPANS: usize = 8;
pub const BOX_QUEST_SEQ_STATIC_ONLY_TEXT_CANDIDATES: usize = 3_712;
pub const BOX_QUEST_SEQ_TEXT_CANDIDATE_CODE_UNITS: usize = 34_295;
pub const BOX_QUEST_SEQ_TEXT_CANDIDATE_UNIQUE_CHARACTERS: usize = 307;
pub const BOX_QUEST_SEQ_TEXT_CANDIDATE_REPORT_SHA256: &str =
    "4a7441341c5840b9e89cb1287943710be0b004397b4a080b3842a7034fbc5087";
pub const BOX_QUEST_SEQ_TEXT_SUMMARY_REPORT_SHA256: &str =
    "9365cb835ecc517a1c393caa03f9ff8733e72bda027046881dda46addc1ab067";
pub const BOX_QUEST_SEQ_TEXT_CANDIDATE_FILES: [(&str, usize, usize); 13] = [
    ("HASI.SEQ", 109, 875),
    ("ISEKI.SEQ", 215, 1_879),
    ("ITEM_DAT.SEQ", 416, 3_155),
    ("KAWA.SEQ", 638, 5_643),
    ("MITI.SEQ", 117, 1_053),
    ("MORI.SEQ", 188, 1_536),
    ("SHEZ.SEQ", 155, 1_619),
    ("SUKE.SEQ", 80, 727),
    ("TOU.SEQ", 342, 3_112),
    ("TOWN00.SEQ", 465, 4_848),
    ("TOWN10.SEQ", 568, 5_746),
    ("TOWN20.SEQ", 288, 2_928),
    ("TOWN30.SEQ", 139, 1_174),
];

/// 전체 SEQ 후보의 증거 수준.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqTextCandidateEvidence {
    /// runtime 화면·state ledger와 정확히 같은 `TOWN00` span.
    RuntimeObservedSpan,
    /// 코드표·정렬·엄격 decode만 통과한 정적 후보.
    StaticCodeTableRunCandidate,
}

impl QuestSeqTextCandidateEvidence {
    pub const fn label(self) -> &'static str {
        match self {
            Self::RuntimeObservedSpan => "runtime-observed-span",
            Self::StaticCodeTableRunCandidate => "static-code-table-run-candidate",
        }
    }
}

/// SEQ 하나 안의 연속 일본어 run 후보.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextCandidate {
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub offset: usize,
    pub raw: Vec<u8>,
    pub source: String,
    pub japanese_code_units: usize,
    pub evidence: QuestSeqTextCandidateEvidence,
}

impl QuestSeqTextCandidate {
    pub fn byte_len(&self) -> usize {
        self.raw.len()
    }

    pub fn code_units(&self) -> usize {
        self.raw.len() / 2
    }

    pub fn id(&self) -> String {
        format!("QUEST-SEQ-TEXT:{}:0x{:04X}", self.sequence, self.offset)
    }
}

/// 후보가 없는 SEQ도 보존하는 파일별 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextSummary {
    pub sequence: String,
    pub bytes: usize,
    pub sha256: [u8; 32],
    pub candidates: usize,
    pub source_code_units: usize,
}

/// `RPG_SEQ.VDD` 전체 후보 분석.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextCandidateAnalysis {
    pub summaries: Vec<QuestSeqTextSummary>,
    pub candidates: Vec<QuestSeqTextCandidate>,
    pub sequences_with_candidates: usize,
    pub runtime_observed_spans: usize,
    pub static_only_candidates: usize,
    pub source_code_units: usize,
    pub unique_source_characters: usize,
}

/// 입력 집합·runtime ledger·결정적 분모 불일치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextCandidateError(String);

impl fmt::Display for QuestSeqTextCandidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestSeqTextCandidateError {}

fn error(message: impl Into<String>) -> QuestSeqTextCandidateError {
    QuestSeqTextCandidateError(message.into())
}

fn runtime_observed(sequence: &str, offset: usize, source: &str, raw: &[u8]) -> bool {
    sequence == "TOWN00.SEQ"
        && QUEST_DIALOGUE_TRANSLATIONS
            .iter()
            .flat_map(|entry| entry.spans)
            .any(|span| {
                span.offset == offset
                    && span.protected_source == source
                    && encode_sjis(span.protected_source).is_ok_and(|encoded| encoded == raw)
            })
}

/// 임의 SEQ 집합을 이름 순서와 무관하게 결정적으로 분석한다.
pub fn audit_quest_seq_text_candidates<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    code_table: &QuestCodeTable,
) -> Result<QuestSeqTextCandidateAnalysis, QuestSeqTextCandidateError> {
    let mut input = BTreeMap::<String, &'a [u8]>::new();
    for (name, raw) in sequences {
        if name.is_empty() {
            return Err(error("QUEST SEQ text 후보 입력 이름이 비어 있음"));
        }
        if input.insert(name.to_string(), raw).is_some() {
            return Err(error(format!("QUEST SEQ text 후보 입력 중복: {name}")));
        }
    }
    if input.is_empty() {
        return Err(error("QUEST SEQ text 후보 입력이 없음"));
    }

    let mut summaries = Vec::with_capacity(input.len());
    let mut candidates = Vec::new();
    for (sequence, raw) in input {
        let sequence_sha256: [u8; 32] = Sha256::digest(raw).into();
        let runs = scan_quest_code_table_text_runs(raw, code_table);
        let start = candidates.len();
        candidates.extend(runs.into_iter().map(|run| {
            let evidence = if runtime_observed(&sequence, run.offset, &run.source, &run.raw) {
                QuestSeqTextCandidateEvidence::RuntimeObservedSpan
            } else {
                QuestSeqTextCandidateEvidence::StaticCodeTableRunCandidate
            };
            QuestSeqTextCandidate {
                sequence: sequence.clone(),
                sequence_sha256,
                offset: run.offset,
                raw: run.raw,
                source: run.source,
                japanese_code_units: run.japanese_code_units,
                evidence,
            }
        }));
        let sequence_candidates = &candidates[start..];
        summaries.push(QuestSeqTextSummary {
            sequence,
            bytes: raw.len(),
            sha256: sequence_sha256,
            candidates: sequence_candidates.len(),
            source_code_units: sequence_candidates
                .iter()
                .map(QuestSeqTextCandidate::code_units)
                .sum(),
        });
    }

    let runtime_observed_spans = candidates
        .iter()
        .filter(|candidate| {
            candidate.evidence == QuestSeqTextCandidateEvidence::RuntimeObservedSpan
        })
        .count();
    let source_code_units = candidates
        .iter()
        .map(QuestSeqTextCandidate::code_units)
        .sum();
    let unique_source_characters = candidates
        .iter()
        .flat_map(|candidate| candidate.source.chars())
        .collect::<BTreeSet<_>>()
        .len();

    Ok(QuestSeqTextCandidateAnalysis {
        sequences_with_candidates: summaries
            .iter()
            .filter(|summary| summary.candidates > 0)
            .count(),
        static_only_candidates: candidates.len() - runtime_observed_spans,
        summaries,
        candidates,
        runtime_observed_spans,
        source_code_units,
        unique_source_characters,
    })
}

/// 지원 원본 113개 SEQ의 정적 후보 분모와 두 report digest를 고정한다.
pub fn verify_box_quest_seq_text_candidates<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    code_table: &QuestCodeTable,
) -> Result<QuestSeqTextCandidateAnalysis, QuestSeqTextCandidateError> {
    let analysis = audit_quest_seq_text_candidates(sequences, code_table)?;
    let actual = [
        analysis.summaries.len(),
        analysis.summaries.iter().map(|summary| summary.bytes).sum(),
        analysis.sequences_with_candidates,
        analysis
            .summaries
            .iter()
            .filter(|summary| summary.candidates == 0)
            .count(),
        analysis.candidates.len(),
        analysis.runtime_observed_spans,
        analysis.static_only_candidates,
        analysis.source_code_units,
        analysis.unique_source_characters,
    ];
    let expected = [
        BOX_QUEST_SEQ_TEXT_SEQUENCES,
        BOX_QUEST_SEQ_TEXT_BYTES,
        BOX_QUEST_SEQ_TEXT_FILES_WITH_CANDIDATES,
        BOX_QUEST_SEQ_TEXT_FILES_WITHOUT_CANDIDATES,
        BOX_QUEST_SEQ_TEXT_CANDIDATES,
        BOX_QUEST_SEQ_RUNTIME_OBSERVED_TEXT_SPANS,
        BOX_QUEST_SEQ_STATIC_ONLY_TEXT_CANDIDATES,
        BOX_QUEST_SEQ_TEXT_CANDIDATE_CODE_UNITS,
        BOX_QUEST_SEQ_TEXT_CANDIDATE_UNIQUE_CHARACTERS,
    ];
    if actual != expected {
        return Err(error(format!(
            "QUEST SEQ text 후보 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }

    let actual_candidate_files = analysis
        .summaries
        .iter()
        .filter(|summary| summary.candidates > 0)
        .map(|summary| {
            (
                summary.sequence.as_str(),
                summary.candidates,
                summary.source_code_units,
            )
        })
        .collect::<Vec<_>>();
    if actual_candidate_files != BOX_QUEST_SEQ_TEXT_CANDIDATE_FILES {
        return Err(error(format!(
            "QUEST SEQ text 후보 파일별 분모 불일치: expected={:?}, actual={actual_candidate_files:?}",
            BOX_QUEST_SEQ_TEXT_CANDIDATE_FILES
        )));
    }

    let candidate_report = render_quest_seq_text_candidates_tsv(&analysis);
    let candidate_digest = format!("{:x}", Sha256::digest(candidate_report.as_bytes()));
    if candidate_digest != BOX_QUEST_SEQ_TEXT_CANDIDATE_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST SEQ text 후보 report SHA-256 불일치: expected={}, actual={candidate_digest}",
            BOX_QUEST_SEQ_TEXT_CANDIDATE_REPORT_SHA256
        )));
    }

    let summary_report = render_quest_seq_text_summary_tsv(&analysis);
    let summary_digest = format!("{:x}", Sha256::digest(summary_report.as_bytes()));
    if summary_digest != BOX_QUEST_SEQ_TEXT_SUMMARY_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST SEQ text 요약 report SHA-256 불일치: expected={}, actual={summary_digest}",
            BOX_QUEST_SEQ_TEXT_SUMMARY_REPORT_SHA256
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

fn supported_town00_runtime(candidate: &QuestSeqTextCandidate) -> Option<u32> {
    if candidate.sequence != "TOWN00.SEQ" || hex(&candidate.sequence_sha256) != TOWN00_SEQ_SHA256 {
        return None;
    }
    RuntimeAssetMap::new(TOWN00_SEQ_RUNTIME_BASE, TOWN00_SEQ_LEN as u32)?
        .runtime_addr(candidate.offset)
}

/// 전체 후보를 원본 보호 필드와 함께 결정적 TSV로 렌더링한다.
pub fn render_quest_seq_text_candidates_tsv(analysis: &QuestSeqTextCandidateAnalysis) -> String {
    let sequence_candidate_counts = analysis
        .summaries
        .iter()
        .map(|summary| (summary.sequence.as_str(), summary.candidates))
        .collect::<BTreeMap<_, _>>();
    let mut output = String::from(
        "schema_version\tarchive_path\tarchive_sequence_count\tsequence_name\t\
         sequence_sha256\tsequence_candidate_count\tcandidate_id\toffset\truntime_address\t\
         raw_hex\tbyte_len\tcode_units\tjapanese_code_units\tevidence\tsource\n",
    );
    for candidate in &analysis.candidates {
        let runtime_address = supported_town00_runtime(candidate)
            .map(|address| format!("0x{address:08X}"))
            .unwrap_or_default();
        output.push_str(&format!(
            "{}\t/QUEST/RPG_SEQ.VDD;1\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            QUEST_SEQ_TEXT_CANDIDATE_SCHEMA_VERSION,
            analysis.summaries.len(),
            candidate.sequence,
            hex(&candidate.sequence_sha256),
            sequence_candidate_counts[candidate.sequence.as_str()],
            candidate.id(),
            candidate.offset,
            runtime_address,
            hex(&candidate.raw),
            candidate.byte_len(),
            candidate.code_units(),
            candidate.japanese_code_units,
            candidate.evidence.label(),
            escape_tsv(&candidate.source),
        ));
    }
    output
}

/// 후보가 없는 SEQ를 포함한 전체 파일 분모를 결정적 TSV로 렌더링한다.
pub fn render_quest_seq_text_summary_tsv(analysis: &QuestSeqTextCandidateAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tarchive_path\tarchive_sequence_count\tsequence_name\t\
         sequence_sha256\tbytes\ttext_candidate_count\tsource_code_units\tstatus\n",
    );
    for summary in &analysis.summaries {
        let status = if summary.candidates == 0 {
            "no-code-table-run-candidate"
        } else {
            "static-candidates-present"
        };
        output.push_str(&format!(
            "{}\t/QUEST/RPG_SEQ.VDD;1\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            QUEST_SEQ_TEXT_SUMMARY_SCHEMA_VERSION,
            analysis.summaries.len(),
            summary.sequence,
            hex(&summary.sha256),
            summary.bytes,
            summary.candidates,
            summary.source_code_units,
            status,
        ));
    }
    output
}

#[cfg(test)]
#[path = "quest_seq_text_candidates_tests.rs"]
mod tests;
