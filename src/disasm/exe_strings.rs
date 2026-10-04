//! 11개 EXE의 일본어 문자열 후보와 정적 참조 근거.
//!
//! 기존 조사 휴리스틱과 같은 경계를 쓴다. 즉, 연속된 2바이트 Shift-JIS
//! 후보가 3개 이상이고 그 안에 히라가나나 가타카나가 하나 이상 있을 때만
//! 후보로 센다. 이 결과는 **하한이자 후보**다. 정적 주소 참조가 있어도
//! 정상 플레이에서 소비된다는 runtime 증거가 아니며, 참조가 없다고
//! 도달 불가를 뜻하지도 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::{normalize_kseg, ExeMap, EXE_MAPS};
use crate::text::{decode_sjis, scan_kana_runs};

use super::xref::{find_address_loads, DEFAULT_WINDOW};
use super::{linear, trace};

const MIN_CODE_UNITS: usize = 3;

/// 지원 원본의 기존 가나 휴리스틱 분모.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxExeStringExpectation {
    pub exe: &'static str,
    pub candidates: usize,
    pub code_units: usize,
}

pub const BOX_EXE_STRING_EXPECTATIONS: [BoxExeStringExpectation; 11] = [
    BoxExeStringExpectation {
        exe: "SLPS_031.14",
        candidates: 7,
        code_units: 83,
    },
    BoxExeStringExpectation {
        exe: "GOCHA1.EXE",
        candidates: 193,
        code_units: 2_296,
    },
    BoxExeStringExpectation {
        exe: "GOCHA2.EXE",
        candidates: 196,
        code_units: 2_302,
    },
    BoxExeStringExpectation {
        exe: "HORI2.EXE",
        candidates: 189,
        code_units: 2_284,
    },
    BoxExeStringExpectation {
        exe: "HORI4.EXE",
        candidates: 189,
        code_units: 2_284,
    },
    BoxExeStringExpectation {
        exe: "KOUSIKI.EXE",
        candidates: 17,
        code_units: 167,
    },
    BoxExeStringExpectation {
        exe: "MINPUYO.EXE",
        candidates: 192,
        code_units: 2_290,
    },
    BoxExeStringExpectation {
        exe: "PCOMI.EXE",
        candidates: 4,
        code_units: 50,
    },
    BoxExeStringExpectation {
        exe: "PUYO1.EXE",
        candidates: 5,
        code_units: 306,
    },
    BoxExeStringExpectation {
        exe: "PUYO2.EXE",
        candidates: 8,
        code_units: 317,
    },
    BoxExeStringExpectation {
        exe: "QUEST.EXE",
        candidates: 259,
        code_units: 2_253,
    },
];

/// 후보 바이트·주소·정적 근거를 함께 고정한 지원 원본 digest.
pub const BOX_EXE_STRING_ANALYSIS_SHA256: [u8; 32] = [
    0x08, 0x78, 0x89, 0x95, 0xF7, 0x2D, 0xB7, 0xD7, 0xDC, 0x54, 0x2C, 0xDF, 0x18, 0x26, 0xE4, 0xC1,
    0x22, 0x0E, 0x44, 0xCD, 0xD9, 0xA0, 0x6D, 0x9F, 0x67, 0xC7, 0x66, 0x79, 0x3F, 0x02, 0xF9, 0x21,
];

/// 문자열 후보 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExeStringCandidate {
    pub exe: &'static str,
    /// PS-X EXE header를 제외한 text-relative offset.
    pub text_offset: usize,
    pub runtime_addr: u32,
    pub byte_len: usize,
    pub code_units: usize,
    pub hiragana: usize,
    pub katakana: usize,
    /// 엄격한 Shift-JIS decode 결과. `None`이면 heuristic byte pair 중
    /// 실제 mapping이 없는 값이 포함된 것이다.
    pub decoded: Option<String>,
    pub raw_sha256: [u8; 32],
    /// 선형 스윕에서 후보 바이트 범위 안 주소를 만든 `lui` 지점.
    pub static_ref_sites: Vec<u32>,
    /// 위 정적 참조 중 `lui`와 결합 명령이 모두 엔트리 추적에 든 것.
    pub entry_reached_ref_sites: Vec<u32>,
    /// 후보 바이트가 엔트리 추적 명령과 겹치는가. 참조 여부와 독립인
    /// 거짓양성 경고다.
    pub overlaps_entry_code: bool,
}

impl ExeStringCandidate {
    pub fn evidence(&self) -> &'static str {
        if !self.entry_reached_ref_sites.is_empty() {
            "entry_trace_static_ref"
        } else if !self.static_ref_sites.is_empty() {
            "linear_static_ref"
        } else if self.overlaps_entry_code {
            "entry_code_overlap"
        } else {
            "no_static_ref"
        }
    }
}

/// EXE 하나의 후보 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExeStringSummary {
    pub exe: &'static str,
    pub candidates: usize,
    pub code_units: usize,
    pub strict_valid_candidates: usize,
    pub heuristic_only_candidates: usize,
    pub statically_referenced_candidates: usize,
    pub entry_referenced_candidates: usize,
    pub entry_code_overlap_candidates: usize,
}

/// 11개 EXE 후보와 정적 근거 전체.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExeStringAnalysis {
    pub summaries: Vec<ExeStringSummary>,
    pub candidates: Vec<ExeStringCandidate>,
    /// 후보 바이트·주소·정적 근거를 EXE 순서대로 직렬화한 digest.
    pub analysis_sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExeStringError {
    EmptyInput,
    DuplicateExecutable(&'static str),
    TextLengthMismatch {
        exe: &'static str,
        expected: usize,
        actual: usize,
    },
    UnalignedText(&'static str),
    BoxExecutableCount {
        expected: usize,
        actual: usize,
    },
    MissingBoxExecutable(&'static str),
    SourceCountMismatch {
        exe: &'static str,
        expected_candidates: usize,
        actual_candidates: usize,
        expected_code_units: usize,
        actual_code_units: usize,
    },
    AnalysisDigestMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

impl fmt::Display for ExeStringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "문자열 후보를 조사할 EXE가 없음"),
            Self::DuplicateExecutable(exe) => write!(f, "문자열 후보 EXE 중복: {exe}"),
            Self::TextLengthMismatch {
                exe,
                expected,
                actual,
            } => write!(
                f,
                "{exe} text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText(exe) => write!(f, "{exe} text 길이가 4바이트 정렬이 아님"),
            Self::BoxExecutableCount { expected, actual } => write!(
                f,
                "BOX EXE 문자열 후보 분모 불일치: expected={expected}, actual={actual}"
            ),
            Self::MissingBoxExecutable(exe) => {
                write!(f, "BOX EXE 문자열 후보 입력 누락: {exe}")
            }
            Self::SourceCountMismatch {
                exe,
                expected_candidates,
                actual_candidates,
                expected_code_units,
                actual_code_units,
            } => write!(
                f,
                "{exe} 문자열 후보 분모 불일치: candidates expected={expected_candidates}, \
                 actual={actual_candidates}; code_units expected={expected_code_units}, \
                actual={actual_code_units}"
            ),
            Self::AnalysisDigestMismatch { expected, actual } => write!(
                f,
                "EXE 문자열 후보 분석 digest 불일치: expected={}, actual={}",
                hex_digest(expected),
                hex_digest(actual)
            ),
        }
    }
}

impl Error for ExeStringError {}

/// 임의 EXE 집합에서 후보와 정적 근거를 만든다.
pub fn audit_exe_string_candidates<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
) -> Result<ExeStringAnalysis, ExeStringError> {
    let mut seen = BTreeSet::new();
    let mut summaries = Vec::new();
    let mut candidates = Vec::new();

    for (map, text) in executables {
        if !seen.insert(map.name) {
            return Err(ExeStringError::DuplicateExecutable(map.name));
        }
        if text.len() != map.text_len as usize {
            return Err(ExeStringError::TextLengthMismatch {
                exe: map.name,
                expected: map.text_len as usize,
                actual: text.len(),
            });
        }
        let instructions =
            linear::sweep(map, text).ok_or(ExeStringError::UnalignedText(map.name))?;
        let address_loads = find_address_loads(&instructions, DEFAULT_WINDOW);
        let entry_trace = trace(map, text, &[]);
        let raw_candidates = scan_kana_runs(text, MIN_CODE_UNITS);
        let summary_start = candidates.len();

        for raw_candidate in raw_candidates {
            let runtime_addr = map.load_addr + raw_candidate.offset as u32;
            let runtime_end = runtime_addr + raw_candidate.raw.len() as u32;
            let mut static_ref_sites = BTreeSet::new();
            let mut entry_reached_ref_sites = BTreeSet::new();
            for load in &address_loads {
                let value = normalize_kseg(load.value);
                if value < runtime_addr || value >= runtime_end {
                    continue;
                }
                static_ref_sites.insert(load.lui_addr);
                if entry_trace.instructions.contains_key(&load.lui_addr)
                    && entry_trace.instructions.contains_key(&load.combine_addr)
                {
                    entry_reached_ref_sites.insert(load.lui_addr);
                }
            }
            let overlaps_entry_code = entry_trace
                .instructions
                .range(runtime_addr.saturating_sub(3)..runtime_end)
                .any(|(&instruction_addr, _)| instruction_addr < runtime_end);
            candidates.push(ExeStringCandidate {
                exe: map.name,
                text_offset: raw_candidate.offset,
                runtime_addr,
                byte_len: raw_candidate.raw.len(),
                code_units: raw_candidate.raw.len() / 2,
                hiragana: raw_candidate.hiragana,
                katakana: raw_candidate.katakana,
                decoded: decode_sjis(raw_candidate.raw).ok(),
                raw_sha256: Sha256::digest(raw_candidate.raw).into(),
                static_ref_sites: static_ref_sites.into_iter().collect(),
                entry_reached_ref_sites: entry_reached_ref_sites.into_iter().collect(),
                overlaps_entry_code,
            });
        }

        let exe_candidates = &candidates[summary_start..];
        summaries.push(ExeStringSummary {
            exe: map.name,
            candidates: exe_candidates.len(),
            code_units: exe_candidates
                .iter()
                .map(|candidate| candidate.code_units)
                .sum(),
            strict_valid_candidates: exe_candidates
                .iter()
                .filter(|candidate| candidate.decoded.is_some())
                .count(),
            heuristic_only_candidates: exe_candidates
                .iter()
                .filter(|candidate| candidate.decoded.is_none())
                .count(),
            statically_referenced_candidates: exe_candidates
                .iter()
                .filter(|candidate| !candidate.static_ref_sites.is_empty())
                .count(),
            entry_referenced_candidates: exe_candidates
                .iter()
                .filter(|candidate| !candidate.entry_reached_ref_sites.is_empty())
                .count(),
            entry_code_overlap_candidates: exe_candidates
                .iter()
                .filter(|candidate| candidate.overlaps_entry_code)
                .count(),
        });
    }

    if summaries.is_empty() {
        return Err(ExeStringError::EmptyInput);
    }
    let analysis_sha256 = digest_candidates(&candidates);
    Ok(ExeStringAnalysis {
        summaries,
        candidates,
        analysis_sha256,
    })
}

/// 지원 원본 11개와 기존 하한 분모를 고정한다.
pub fn verify_box_exe_string_candidates<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
) -> Result<ExeStringAnalysis, ExeStringError> {
    let analysis = audit_exe_string_candidates(executables)?;
    if analysis.summaries.len() != EXE_MAPS.len() {
        return Err(ExeStringError::BoxExecutableCount {
            expected: EXE_MAPS.len(),
            actual: analysis.summaries.len(),
        });
    }
    for expectation in BOX_EXE_STRING_EXPECTATIONS {
        let summary = analysis
            .summaries
            .iter()
            .find(|summary| summary.exe == expectation.exe)
            .ok_or(ExeStringError::MissingBoxExecutable(expectation.exe))?;
        if summary.candidates != expectation.candidates
            || summary.code_units != expectation.code_units
        {
            return Err(ExeStringError::SourceCountMismatch {
                exe: expectation.exe,
                expected_candidates: expectation.candidates,
                actual_candidates: summary.candidates,
                expected_code_units: expectation.code_units,
                actual_code_units: summary.code_units,
            });
        }
    }
    for map in EXE_MAPS {
        if !analysis
            .summaries
            .iter()
            .any(|summary| summary.exe == map.name)
        {
            return Err(ExeStringError::MissingBoxExecutable(map.name));
        }
    }
    if analysis.analysis_sha256 != BOX_EXE_STRING_ANALYSIS_SHA256 {
        return Err(ExeStringError::AnalysisDigestMismatch {
            expected: BOX_EXE_STRING_ANALYSIS_SHA256,
            actual: analysis.analysis_sha256,
        });
    }
    Ok(analysis)
}

/// 사람이 후보를 분류하고 runtime 증거를 연결할 결정적 TSV.
pub fn render_exe_string_candidates_tsv(analysis: &ExeStringAnalysis) -> String {
    let mut out = String::from(
        "candidate_id\texe\ttext_offset\truntime_addr\tbyte_len\tcode_units\
         \thiragana\tkatakana\tstrict_sjis\tstatic_ref_count\
         \tentry_reached_ref_count\toverlaps_entry_code\tstatic_evidence\
         \truntime_verdict\traw_sha256\tstatic_ref_sites\
         \tentry_reached_ref_sites\tdecoded_text\n",
    );
    for candidate in &analysis.candidates {
        let text = candidate
            .decoded
            .as_deref()
            .map(escape_tsv)
            .unwrap_or_default();
        writeln!(
            out,
            "{}:{:06X}\t{}\t0x{:06X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\
             \t{}\tunverified\t{}\t{}\t{}\t{}",
            candidate.exe,
            candidate.text_offset,
            candidate.exe,
            candidate.text_offset,
            candidate.runtime_addr,
            candidate.byte_len,
            candidate.code_units,
            candidate.hiragana,
            candidate.katakana,
            candidate.decoded.is_some(),
            candidate.static_ref_sites.len(),
            candidate.entry_reached_ref_sites.len(),
            candidate.overlaps_entry_code,
            candidate.evidence(),
            hex_digest(&candidate.raw_sha256),
            hex_addresses(&candidate.static_ref_sites),
            hex_addresses(&candidate.entry_reached_ref_sites),
            text,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    out
}

fn digest_candidates(candidates: &[ExeStringCandidate]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for candidate in candidates {
        digest.update(candidate.exe.as_bytes());
        digest.update([0]);
        digest.update((candidate.text_offset as u64).to_le_bytes());
        digest.update((candidate.byte_len as u64).to_le_bytes());
        digest.update(candidate.raw_sha256);
        digest.update([u8::from(candidate.decoded.is_some())]);
        digest.update([u8::from(candidate.overlaps_entry_code)]);
        for site in &candidate.static_ref_sites {
            digest.update(site.to_le_bytes());
        }
        digest.update([0xFF]);
        for site in &candidate.entry_reached_ref_sites {
            digest.update(site.to_le_bytes());
        }
        digest.update([0xFE]);
    }
    digest.finalize().into()
}

fn escape_tsv(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn hex_addresses(addresses: &[u32]) -> String {
    addresses
        .iter()
        .map(|address| format!("0x{address:08X}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "exe_strings_tests.rs"]
mod exe_strings_tests;
