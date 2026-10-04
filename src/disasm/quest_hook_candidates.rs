//! QUEST 내부 hook 설치 공간의 정적 후보 인벤토리.
//!
//! startup clear와 겹치지 않는 initialized text에서 긴 0 run을 찾고,
//! 엔트리 재귀 추적·직접 control target·주소 조립·정렬 raw pointer를 각각
//! 센다. 정적 미참조는 free-space 증명이 아니므로 모든 미기각 행은 runtime
//! write/exec 검증 대기로 남긴다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::{normalize_kseg, ExeMap};
use crate::isa::FlowKind;

use super::linear;
use super::quest_hook_space::{
    verify_box_quest_hook_space, QuestHookSpaceError, QUEST_HOOK_INITIALIZED_END_RUNTIME,
};
use super::trace;
use super::xref::{find_address_loads, DEFAULT_WINDOW};

pub const QUEST_HOOK_CANDIDATE_MIN_BYTES: usize = 64;

/// 지원 원본의 후보 행 digest. 주소는 runtime, 길이와 각 참조 수는 u64 LE로
/// 순서대로 넣는다.
pub const BOX_QUEST_HOOK_CANDIDATE_ANALYSIS_SHA256: [u8; 32] = [
    0xFA, 0x41, 0x45, 0x55, 0xF1, 0x33, 0xE5, 0x9F, 0x51, 0x72, 0x33, 0x39, 0x18, 0x62, 0x29, 0x41,
    0x7B, 0x4C, 0x25, 0x0D, 0xED, 0x0E, 0x1A, 0xFF, 0x1D, 0xCD, 0xE7, 0xAF, 0xA9, 0x59, 0xDC, 0xAE,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestHookCandidateStaticVerdict {
    /// 엔트리 추적 명령과 겹치므로 설치 후보에서 정적으로 기각한다.
    RejectedEntryReached,
    /// 정적 참조가 있어 소비 의미 판정 전에는 설치할 수 없다.
    ReferencedNeedsConsumer,
    /// 현재 정적 패턴에는 참조가 없지만 runtime write/exec 증거가 필요하다.
    UnreferencedNeedsRuntime,
}

impl QuestHookCandidateStaticVerdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RejectedEntryReached => "rejected-entry-reached",
            Self::ReferencedNeedsConsumer => "referenced-needs-consumer",
            Self::UnreferencedNeedsRuntime => "unreferenced-needs-runtime",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestHookSpaceCandidate {
    pub start_runtime: u32,
    pub end_runtime: u32,
    pub bytes: usize,
    pub entry_reached_instructions: usize,
    pub direct_control_targets: usize,
    pub address_loads: usize,
    pub aligned_pointer_words: usize,
    pub static_verdict: QuestHookCandidateStaticVerdict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestHookCandidateAnalysis {
    pub candidates: Vec<QuestHookSpaceCandidate>,
    pub total_zero_bytes: usize,
    pub entry_reached_candidates: usize,
    pub referenced_candidates: usize,
    pub unreferenced_candidates: usize,
    pub indirect_sites: usize,
    pub analysis_sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestHookCandidateError {
    HookSpace(QuestHookSpaceError),
    UnalignedText(usize),
    CoordinateOverflow,
    NoCandidates,
    AnalysisDigestMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

impl fmt::Display for QuestHookCandidateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HookSpace(source) => write!(f, "QUEST hook 후보 원본 경계 실패: {source}"),
            Self::UnalignedText(actual) => {
                write!(
                    f,
                    "QUEST hook 후보 text 길이가 4바이트 정렬이 아님: {actual}"
                )
            }
            Self::CoordinateOverflow => f.write_str("QUEST hook 후보 좌표 overflow"),
            Self::NoCandidates => f.write_str("QUEST hook 64B 이상 0-run 후보가 없음"),
            Self::AnalysisDigestMismatch { expected, actual } => write!(
                f,
                "QUEST hook 후보 분석 digest 불일치: expected={}, actual={}",
                hex_digest(expected),
                hex_digest(actual)
            ),
        }
    }
}

impl Error for QuestHookCandidateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::HookSpace(source) => Some(source),
            _ => None,
        }
    }
}

impl From<QuestHookSpaceError> for QuestHookCandidateError {
    fn from(value: QuestHookSpaceError) -> Self {
        Self::HookSpace(value)
    }
}

fn aligned_zero_runs(
    map: &ExeMap,
    text: &[u8],
    end_runtime: u32,
) -> Result<Vec<(u32, u32)>, QuestHookCandidateError> {
    let end_offset = map
        .text_offset(
            end_runtime
                .checked_sub(1)
                .ok_or(QuestHookCandidateError::CoordinateOverflow)?,
        )
        .ok_or(QuestHookCandidateError::CoordinateOverflow)?
        .checked_add(1)
        .ok_or(QuestHookCandidateError::CoordinateOverflow)?;
    let initialized = text
        .get(..end_offset)
        .ok_or(QuestHookCandidateError::CoordinateOverflow)?;
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while cursor < initialized.len() {
        if initialized[cursor] != 0 {
            cursor += 1;
            continue;
        }
        let raw_start = cursor;
        while cursor < initialized.len() && initialized[cursor] == 0 {
            cursor += 1;
        }
        let raw_end = cursor;
        let aligned_start = raw_start
            .checked_add(3)
            .ok_or(QuestHookCandidateError::CoordinateOverflow)?
            & !3;
        let aligned_end = raw_end & !3;
        if aligned_end.saturating_sub(aligned_start) < QUEST_HOOK_CANDIDATE_MIN_BYTES {
            continue;
        }
        let start_runtime = map
            .load_addr
            .checked_add(
                u32::try_from(aligned_start)
                    .map_err(|_| QuestHookCandidateError::CoordinateOverflow)?,
            )
            .ok_or(QuestHookCandidateError::CoordinateOverflow)?;
        let end_runtime = map
            .load_addr
            .checked_add(
                u32::try_from(aligned_end)
                    .map_err(|_| QuestHookCandidateError::CoordinateOverflow)?,
            )
            .ok_or(QuestHookCandidateError::CoordinateOverflow)?;
        out.push((start_runtime, end_runtime));
    }
    Ok(out)
}

fn in_range(value: u32, start: u32, end: u32) -> bool {
    let normalized = normalize_kseg(value);
    normalized >= start && normalized < end
}

fn digest_candidates(candidates: &[QuestHookSpaceCandidate]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for candidate in candidates {
        digest.update(candidate.start_runtime.to_le_bytes());
        digest.update(candidate.end_runtime.to_le_bytes());
        digest.update((candidate.bytes as u64).to_le_bytes());
        digest.update((candidate.entry_reached_instructions as u64).to_le_bytes());
        digest.update((candidate.direct_control_targets as u64).to_le_bytes());
        digest.update((candidate.address_loads as u64).to_le_bytes());
        digest.update((candidate.aligned_pointer_words as u64).to_le_bytes());
        digest.update(candidate.static_verdict.as_str().as_bytes());
        digest.update([0]);
    }
    digest.finalize().into()
}

/// QUEST initialized text의 64B 이상 정렬 0-run과 정적 근거를 전수화한다.
pub fn audit_quest_hook_space_candidates(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestHookCandidateAnalysis, QuestHookCandidateError> {
    verify_box_quest_hook_space(map, text)?;
    let instructions =
        linear::sweep(map, text).ok_or(QuestHookCandidateError::UnalignedText(text.len()))?;
    let address_loads = find_address_loads(&instructions, DEFAULT_WINDOW);
    let entry_trace = trace(map, text, &[]);
    let runs = aligned_zero_runs(map, text, QUEST_HOOK_INITIALIZED_END_RUNTIME)?;
    if runs.is_empty() {
        return Err(QuestHookCandidateError::NoCandidates);
    }

    let mut candidates = Vec::with_capacity(runs.len());
    for (start_runtime, end_runtime) in runs {
        let bytes = usize::try_from(
            end_runtime
                .checked_sub(start_runtime)
                .ok_or(QuestHookCandidateError::CoordinateOverflow)?,
        )
        .map_err(|_| QuestHookCandidateError::CoordinateOverflow)?;
        let entry_reached_instructions = entry_trace
            .instructions
            .range(start_runtime..end_runtime)
            .count();
        let direct_control_targets = instructions
            .iter()
            .filter_map(|instruction| match instruction.flow_kind() {
                FlowKind::Branch { taken } => Some(taken),
                FlowKind::Call { target } | FlowKind::Jump { target } => Some(target),
                _ => None,
            })
            .filter(|&target| in_range(target, start_runtime, end_runtime))
            .count();
        let address_load_count = address_loads
            .iter()
            .filter(|load| in_range(load.value, start_runtime, end_runtime))
            .count();
        let aligned_pointer_words = text
            .chunks_exact(4)
            .filter(|raw| {
                let value = u32::from_le_bytes(
                    (*raw).try_into().expect("4바이트 정렬 chunk 범위를 확인함"),
                );
                in_range(value, start_runtime, end_runtime)
            })
            .count();
        let static_verdict = if entry_reached_instructions != 0 {
            QuestHookCandidateStaticVerdict::RejectedEntryReached
        } else if direct_control_targets != 0
            || address_load_count != 0
            || aligned_pointer_words != 0
        {
            QuestHookCandidateStaticVerdict::ReferencedNeedsConsumer
        } else {
            QuestHookCandidateStaticVerdict::UnreferencedNeedsRuntime
        };
        candidates.push(QuestHookSpaceCandidate {
            start_runtime,
            end_runtime,
            bytes,
            entry_reached_instructions,
            direct_control_targets,
            address_loads: address_load_count,
            aligned_pointer_words,
            static_verdict,
        });
    }

    let total_zero_bytes = candidates.iter().map(|candidate| candidate.bytes).sum();
    let entry_reached_candidates = candidates
        .iter()
        .filter(|candidate| {
            candidate.static_verdict == QuestHookCandidateStaticVerdict::RejectedEntryReached
        })
        .count();
    let referenced_candidates = candidates
        .iter()
        .filter(|candidate| {
            candidate.static_verdict == QuestHookCandidateStaticVerdict::ReferencedNeedsConsumer
        })
        .count();
    let unreferenced_candidates =
        candidates.len() - entry_reached_candidates - referenced_candidates;
    let analysis_sha256 = digest_candidates(&candidates);
    Ok(QuestHookCandidateAnalysis {
        candidates,
        total_zero_bytes,
        entry_reached_candidates,
        referenced_candidates,
        unreferenced_candidates,
        indirect_sites: entry_trace.indirect_sites.len(),
        analysis_sha256,
    })
}

/// 지원 원본에서 후보 집합과 정적 판정 digest까지 고정한다.
pub fn verify_box_quest_hook_space_candidates(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestHookCandidateAnalysis, QuestHookCandidateError> {
    let analysis = audit_quest_hook_space_candidates(map, text)?;
    if analysis.analysis_sha256 != BOX_QUEST_HOOK_CANDIDATE_ANALYSIS_SHA256 {
        return Err(QuestHookCandidateError::AnalysisDigestMismatch {
            expected: BOX_QUEST_HOOK_CANDIDATE_ANALYSIS_SHA256,
            actual: analysis.analysis_sha256,
        });
    }
    Ok(analysis)
}

pub fn render_quest_hook_space_candidates_tsv(analysis: &QuestHookCandidateAnalysis) -> String {
    let mut out = String::from(
        "start_runtime\tend_runtime\tbytes\tentry_reached_instructions\
         \tdirect_control_targets\taddress_loads\taligned_pointer_words\tstatic_verdict\
         \truntime_verdict\n",
    );
    for candidate in &analysis.candidates {
        writeln!(
            out,
            "0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\tneeds-write-exec-observation",
            candidate.start_runtime,
            candidate.end_runtime,
            candidate.bytes,
            candidate.entry_reached_instructions,
            candidate.direct_control_targets,
            candidate.address_loads,
            candidate.aligned_pointer_words,
            candidate.static_verdict.as_str(),
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    out
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "quest_hook_candidates_tests.rs"]
mod tests;
