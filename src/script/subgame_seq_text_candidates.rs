//! QUEST 밖 여섯 SEQ archive의 일본어 run 정적 하한.
//!
//! `PUYO1`·`PUYO2`·`PCOMI`의 실제 text opcode, control, font consumer는
//! 아직 확정하지 않았다. 따라서 이 모듈은 기존 가나 휴리스틱을 원본
//! 직결 Rust gate와 안정 ID로 옮길 뿐이며, 후보를 번역 단위나 쓰기 경계로
//! 승격하지 않는다. 후보가 없는 파일도 summary에 남겨 입력 모집단을
//! 보호한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::text::{decode_sjis, scan_kana_runs};

pub const SUBGAME_SEQ_TEXT_CANDIDATE_SCHEMA_VERSION: &str = "subgame-seq-text-candidates-v1";
pub const SUBGAME_SEQ_TEXT_SUMMARY_SCHEMA_VERSION: &str = "subgame-seq-text-summary-v1";
pub const SUBGAME_SEQ_TEXT_MIN_CODE_UNITS: usize = 3;

/// 지원 원본 archive별 가나 run 하한.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxSubgameSeqTextExpectation {
    pub scope: &'static str,
    pub archive: &'static str,
    pub sequences: usize,
    pub files_with_candidates: usize,
    pub candidates: usize,
    pub code_units: usize,
}

pub const BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS: [BoxSubgameSeqTextExpectation; 6] = [
    expectation("SHELL", "/FIL_SEQ.VDD;1", 1, 0, 0, 0),
    expectation("GOCHA1", "/GOCHA1/TUU_SEQ.VDD;1", 49, 0, 0, 0),
    expectation("KOUSIKI", "/KOUSIKI/KOU_SEQ.VDD;1", 2, 0, 0, 0),
    expectation("PCOMI", "/PCOMI/PC1_SEQ.VDD;1", 2, 1, 4, 14),
    expectation("PUYO1", "/PUYO1/PP1_SEQ.VDD;1", 119, 60, 373, 3_448),
    expectation("PUYO2", "/PUYO2/PP2_SEQ.VDD;1", 151, 58, 361, 3_404),
];

const fn expectation(
    scope: &'static str,
    archive: &'static str,
    sequences: usize,
    files_with_candidates: usize,
    candidates: usize,
    code_units: usize,
) -> BoxSubgameSeqTextExpectation {
    BoxSubgameSeqTextExpectation {
        scope,
        archive,
        sequences,
        files_with_candidates,
        candidates,
        code_units,
    }
}

pub const BOX_SUBGAME_SEQ_TEXT_ANALYSIS_SHA256: [u8; 32] = [
    0x5f, 0x33, 0x16, 0xbc, 0x63, 0x29, 0xba, 0x60, 0x34, 0x1a, 0x81, 0x04, 0xcc, 0xfe, 0x15, 0x8c,
    0x3b, 0x06, 0xae, 0x9f, 0x14, 0x02, 0x46, 0x63, 0x14, 0x35, 0x2a, 0x1a, 0xd7, 0x36, 0x4e, 0x41,
];
pub const BOX_SUBGAME_SEQ_TEXT_CANDIDATE_REPORT_SHA256: [u8; 32] = [
    0xc0, 0x5b, 0x39, 0x9c, 0xa7, 0xb4, 0xdb, 0x59, 0xb4, 0x11, 0xea, 0xb8, 0x8d, 0xc4, 0xec, 0x3d,
    0x29, 0xee, 0xf1, 0x67, 0xf5, 0x40, 0x69, 0xd9, 0xb0, 0xec, 0xe6, 0xa7, 0xdc, 0x10, 0xe2, 0x7a,
];
pub const BOX_SUBGAME_SEQ_TEXT_SUMMARY_REPORT_SHA256: [u8; 32] = [
    0x8e, 0x88, 0xe6, 0xf2, 0xa3, 0x56, 0x3a, 0x71, 0x8c, 0xc3, 0xe6, 0x99, 0xde, 0xce, 0x70, 0x46,
    0xe5, 0xbc, 0x3e, 0x46, 0xdd, 0xa9, 0x5b, 0xf9, 0x8b, 0x61, 0x92, 0x84, 0x23, 0x04, 0x67, 0x63,
];

/// SEQ 하나 안의 가나 포함 run 후보.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextCandidate {
    pub scope: &'static str,
    pub archive: String,
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub offset: usize,
    pub raw: Vec<u8>,
    pub hiragana: usize,
    pub katakana: usize,
    pub decoded: Option<String>,
}

impl SubgameSeqTextCandidate {
    pub fn id(&self) -> String {
        format!(
            "SUBGAME-SEQ-TEXT:{}:{}:0x{:04X}",
            self.scope, self.sequence, self.offset
        )
    }

    pub fn code_units(&self) -> usize {
        self.raw.len() / 2
    }
}

/// 후보가 없는 SEQ도 포함한 원본 파일 계약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextSummary {
    pub scope: &'static str,
    pub archive: String,
    pub sequence: String,
    pub bytes: usize,
    pub sequence_sha256: [u8; 32],
    pub candidates: usize,
    pub code_units: usize,
    pub strict_candidates: usize,
    pub heuristic_only_candidates: usize,
}

/// 여섯 archive의 전체 정적 후보 하한.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextCandidateAnalysis {
    pub summaries: Vec<SubgameSeqTextSummary>,
    pub candidates: Vec<SubgameSeqTextCandidate>,
    pub archives: usize,
    pub files_with_candidates: usize,
    pub total_bytes: usize,
    pub code_units: usize,
    pub strict_candidates: usize,
    pub heuristic_only_candidates: usize,
    pub unique_decoded_characters: usize,
    pub analysis_sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextCandidateError(String);

impl fmt::Display for SubgameSeqTextCandidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqTextCandidateError {}

fn error(message: impl Into<String>) -> SubgameSeqTextCandidateError {
    SubgameSeqTextCandidateError(message.into())
}

fn scope_for_archive(archive: &str) -> Option<&'static str> {
    BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS
        .iter()
        .find(|expectation| expectation.archive == archive)
        .map(|expectation| expectation.scope)
}

/// 해제된 SEQ 입력을 순서와 무관하게 결정적으로 분석한다.
pub fn audit_subgame_seq_text_candidates<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
) -> Result<SubgameSeqTextCandidateAnalysis, SubgameSeqTextCandidateError> {
    let mut input = BTreeMap::<(String, String), (&'static str, &'a [u8])>::new();
    for (archive, sequence, raw) in sources {
        if archive.is_empty() || sequence.is_empty() {
            return Err(error(
                "subgame SEQ text 후보의 archive/sequence 이름이 비어 있음",
            ));
        }
        let scope = scope_for_archive(archive)
            .ok_or_else(|| error(format!("subgame SEQ text 후보의 미등록 archive: {archive}")))?;
        let key = (archive.to_string(), sequence.to_string());
        if input.insert(key.clone(), (scope, raw)).is_some() {
            return Err(error(format!(
                "subgame SEQ text 후보 입력 중복: {}:{}",
                key.0, key.1
            )));
        }
    }
    if input.is_empty() {
        return Err(error("subgame SEQ text 후보 입력이 없음"));
    }

    let mut summaries = Vec::with_capacity(input.len());
    let mut candidates = Vec::new();
    for ((archive, sequence), (scope, raw)) in input {
        let sequence_sha256: [u8; 32] = Sha256::digest(raw).into();
        let start = candidates.len();
        candidates.extend(
            scan_kana_runs(raw, SUBGAME_SEQ_TEXT_MIN_CODE_UNITS)
                .into_iter()
                .map(|run| SubgameSeqTextCandidate {
                    scope,
                    archive: archive.clone(),
                    sequence: sequence.clone(),
                    sequence_sha256,
                    offset: run.offset,
                    raw: run.raw.to_vec(),
                    hiragana: run.hiragana,
                    katakana: run.katakana,
                    decoded: decode_sjis(run.raw).ok(),
                }),
        );
        let file_candidates = &candidates[start..];
        summaries.push(SubgameSeqTextSummary {
            scope,
            archive,
            sequence,
            bytes: raw.len(),
            sequence_sha256,
            candidates: file_candidates.len(),
            code_units: file_candidates
                .iter()
                .map(SubgameSeqTextCandidate::code_units)
                .sum(),
            strict_candidates: file_candidates
                .iter()
                .filter(|candidate| candidate.decoded.is_some())
                .count(),
            heuristic_only_candidates: file_candidates
                .iter()
                .filter(|candidate| candidate.decoded.is_none())
                .count(),
        });
    }

    let files_with_candidates = summaries
        .iter()
        .filter(|summary| summary.candidates > 0)
        .count();
    let total_bytes = summaries.iter().map(|summary| summary.bytes).sum();
    let code_units = candidates
        .iter()
        .map(SubgameSeqTextCandidate::code_units)
        .sum();
    let strict_candidates = candidates
        .iter()
        .filter(|candidate| candidate.decoded.is_some())
        .count();
    let heuristic_only_candidates = candidates.len() - strict_candidates;
    let unique_decoded_characters = candidates
        .iter()
        .filter_map(|candidate| candidate.decoded.as_deref())
        .flat_map(str::chars)
        .collect::<BTreeSet<_>>()
        .len();
    let archives = summaries
        .iter()
        .map(|summary| summary.archive.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let analysis_sha256 = digest_analysis(&summaries, &candidates);

    Ok(SubgameSeqTextCandidateAnalysis {
        summaries,
        candidates,
        archives,
        files_with_candidates,
        total_bytes,
        code_units,
        strict_candidates,
        heuristic_only_candidates,
        unique_decoded_characters,
        analysis_sha256,
    })
}

/// 지원 원본 여섯 archive의 분모와 결정적 보고서를 고정한다.
pub fn verify_box_subgame_seq_text_candidates<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
) -> Result<SubgameSeqTextCandidateAnalysis, SubgameSeqTextCandidateError> {
    let analysis = audit_subgame_seq_text_candidates(sources)?;
    for expectation in BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS {
        let summaries = analysis
            .summaries
            .iter()
            .filter(|summary| summary.archive == expectation.archive)
            .collect::<Vec<_>>();
        let actual = [
            summaries.len(),
            summaries
                .iter()
                .filter(|summary| summary.candidates > 0)
                .count(),
            summaries.iter().map(|summary| summary.candidates).sum(),
            summaries.iter().map(|summary| summary.code_units).sum(),
        ];
        let expected = [
            expectation.sequences,
            expectation.files_with_candidates,
            expectation.candidates,
            expectation.code_units,
        ];
        if actual != expected {
            return Err(error(format!(
                "{} subgame SEQ text 분모 불일치: expected={expected:?}, actual={actual:?}",
                expectation.archive
            )));
        }
        if summaries
            .iter()
            .any(|summary| summary.scope != expectation.scope)
        {
            return Err(error(format!(
                "{} subgame SEQ scope 불일치",
                expectation.archive
            )));
        }
    }
    if analysis.archives != BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS.len() {
        return Err(error(format!(
            "subgame SEQ archive 수 불일치: expected={}, actual={}",
            BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS.len(),
            analysis.archives
        )));
    }
    if analysis.analysis_sha256 != BOX_SUBGAME_SEQ_TEXT_ANALYSIS_SHA256 {
        return Err(error(format!(
            "subgame SEQ text 분석 SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_TEXT_ANALYSIS_SHA256),
            hex(&analysis.analysis_sha256)
        )));
    }
    let candidate_report = render_subgame_seq_text_candidates_tsv(&analysis);
    let actual_candidate_report: [u8; 32] = Sha256::digest(candidate_report.as_bytes()).into();
    if actual_candidate_report != BOX_SUBGAME_SEQ_TEXT_CANDIDATE_REPORT_SHA256 {
        return Err(error(format!(
            "subgame SEQ text 후보 report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_TEXT_CANDIDATE_REPORT_SHA256),
            hex(&actual_candidate_report)
        )));
    }
    let summary_report = render_subgame_seq_text_summary_tsv(&analysis);
    let actual_summary_report: [u8; 32] = Sha256::digest(summary_report.as_bytes()).into();
    if actual_summary_report != BOX_SUBGAME_SEQ_TEXT_SUMMARY_REPORT_SHA256 {
        return Err(error(format!(
            "subgame SEQ text summary report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_TEXT_SUMMARY_REPORT_SHA256),
            hex(&actual_summary_report)
        )));
    }
    Ok(analysis)
}

/// 가나 run 후보만 내보내는 검토 TSV.
pub fn render_subgame_seq_text_candidates_tsv(
    analysis: &SubgameSeqTextCandidateAnalysis,
) -> String {
    let mut output = String::from(
        "schema_version\tcandidate_id\tscope\tarchive\tsequence\tsequence_sha256\t\
         offset\traw_hex\tbyte_len\tcode_units\thiragana\tkatakana\tstrict_sjis\t\
         runtime_verdict\tconsumer_contract\tpatch_strategy\tsource\n",
    );
    for candidate in &analysis.candidates {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t{}\t{}\t{}\t{}\t{}\t{}\t\
             unverified\tunknown-subgame-seq-text-consumer\t\
             preserve-no-write-until-consumer-and-control-contract\t{}",
            SUBGAME_SEQ_TEXT_CANDIDATE_SCHEMA_VERSION,
            candidate.id(),
            candidate.scope,
            candidate.archive,
            candidate.sequence,
            hex(&candidate.sequence_sha256),
            candidate.offset,
            hex(&candidate.raw),
            candidate.raw.len(),
            candidate.code_units(),
            candidate.hiragana,
            candidate.katakana,
            candidate.decoded.is_some(),
            candidate
                .decoded
                .as_deref()
                .map(escape_tsv)
                .unwrap_or_default(),
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

/// 후보가 없는 파일까지 포함하는 결정적 요약 TSV.
pub fn render_subgame_seq_text_summary_tsv(analysis: &SubgameSeqTextCandidateAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tscope\tarchive\tsequence\tsequence_sha256\tbytes\tcandidates\t\
         code_units\tstrict_candidates\theuristic_only_candidates\tstatic_status\n",
    );
    for summary in &analysis.summaries {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            SUBGAME_SEQ_TEXT_SUMMARY_SCHEMA_VERSION,
            summary.scope,
            summary.archive,
            summary.sequence,
            hex(&summary.sequence_sha256),
            summary.bytes,
            summary.candidates,
            summary.code_units,
            summary.strict_candidates,
            summary.heuristic_only_candidates,
            if summary.candidates == 0 {
                "no-kana-run-candidate"
            } else {
                "static-candidates-present"
            },
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn digest_analysis(
    summaries: &[SubgameSeqTextSummary],
    candidates: &[SubgameSeqTextCandidate],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    for summary in summaries {
        digest.update(summary.archive.as_bytes());
        digest.update([0]);
        digest.update(summary.sequence.as_bytes());
        digest.update([0]);
        digest.update((summary.bytes as u64).to_le_bytes());
        digest.update(summary.sequence_sha256);
        digest.update((summary.candidates as u64).to_le_bytes());
        digest.update((summary.code_units as u64).to_le_bytes());
    }
    digest.update([0xFF]);
    for candidate in candidates {
        digest.update(candidate.archive.as_bytes());
        digest.update([0]);
        digest.update(candidate.sequence.as_bytes());
        digest.update([0]);
        digest.update((candidate.offset as u64).to_le_bytes());
        digest.update((candidate.raw.len() as u64).to_le_bytes());
        digest.update(&candidate.raw);
        digest.update((candidate.hiragana as u64).to_le_bytes());
        digest.update((candidate.katakana as u64).to_le_bytes());
        digest.update([u8::from(candidate.decoded.is_some())]);
    }
    digest.finalize().into()
}

fn escape_tsv(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_text_candidates_tests.rs"]
mod subgame_seq_text_candidates_tests;
