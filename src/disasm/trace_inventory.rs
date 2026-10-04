//! 11개 EXE의 엔트리 기반 재귀 추적 인벤토리.
//!
//! 각 EXE는 자기 [`ExeMap`]과 text에서 독립적으로 추적한다. 결과는
//! 엔트리에서 정적으로 확인한 **도달 하한**이며, 간접 분기 너머와
//! 런타임 생성 타깃은 포함하지 않는다. text에는 데이터도 섞일 수 있으므로
//! 전체 text word 대비 비율을 코드 커버리지로 계산하지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::{ExeMap, EXE_MAPS};

use super::trace;

/// EXE 하나의 엔트리 기반 재귀 추적 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableTraceSummary {
    pub exe: &'static str,
    pub text_bytes: usize,
    pub reached_instructions: usize,
    pub decoded_instructions: usize,
    pub unknown_instructions: usize,
    pub call_targets: usize,
    pub internal_call_targets: usize,
    pub external_call_targets: usize,
    pub indirect_sites: usize,
    pub out_of_range_edges: usize,
    /// 주소를 text 선두 기준 offset으로 정규화한 `(offset, word)` 열의 digest.
    pub trace_sha256: [u8; 32],
}

impl ExecutableTraceSummary {
    /// 간접 분기 때문에 정적 추적이 닫히지 않았는가.
    pub const fn has_open_indirect_flow(&self) -> bool {
        self.indirect_sites != 0
    }
}

/// 추적 인벤토리 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceInventoryError {
    EmptyInput,
    DuplicateExecutable(&'static str),
    TextLengthMismatch {
        exe: &'static str,
        expected: usize,
        actual: usize,
    },
    EntryOutsideText {
        exe: &'static str,
        entry_pc: u32,
    },
    EmptyTrace(&'static str),
    BoxExecutableCount {
        expected: usize,
        actual: usize,
    },
    MissingBoxExecutable(&'static str),
}

impl fmt::Display for TraceInventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "재귀 추적할 EXE가 없음"),
            Self::DuplicateExecutable(exe) => write!(f, "재귀 추적 EXE 중복: {exe}"),
            Self::TextLengthMismatch {
                exe,
                expected,
                actual,
            } => write!(
                f,
                "{exe} text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::EntryOutsideText { exe, entry_pc } => {
                write!(f, "{exe} entry PC가 text 범위 밖임: 0x{entry_pc:08X}")
            }
            Self::EmptyTrace(exe) => write!(f, "{exe} 엔트리 재귀 추적 결과가 비어 있음"),
            Self::BoxExecutableCount { expected, actual } => write!(
                f,
                "BOX EXE 재귀 추적 분모 불일치: expected={expected}, actual={actual}"
            ),
            Self::MissingBoxExecutable(exe) => {
                write!(f, "BOX EXE 재귀 추적 입력 누락: {exe}")
            }
        }
    }
}

impl Error for TraceInventoryError {}

/// 각 EXE를 자기 엔트리에서 독립적으로 재귀 추적한다.
pub fn audit_executable_traces<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
) -> Result<Vec<ExecutableTraceSummary>, TraceInventoryError> {
    let mut seen = BTreeSet::new();
    let mut summaries = Vec::new();

    for (map, text) in executables {
        if !seen.insert(map.name) {
            return Err(TraceInventoryError::DuplicateExecutable(map.name));
        }
        if text.len() != map.text_len as usize {
            return Err(TraceInventoryError::TextLengthMismatch {
                exe: map.name,
                expected: map.text_len as usize,
                actual: text.len(),
            });
        }
        if map.text_offset(map.entry_pc).is_none() || !map.entry_pc.is_multiple_of(4) {
            return Err(TraceInventoryError::EntryOutsideText {
                exe: map.name,
                entry_pc: map.entry_pc,
            });
        }

        let trace = trace(map, text, &[]);
        if trace.instructions.is_empty() {
            return Err(TraceInventoryError::EmptyTrace(map.name));
        }

        let mut digest = Sha256::new();
        let mut decoded_instructions = 0usize;
        for (&addr, instruction) in &trace.instructions {
            let offset = map
                .text_offset(addr)
                .expect("trace는 ExeMap text 범위 안의 주소만 보존한다");
            digest.update((offset as u64).to_le_bytes());
            digest.update(instruction.word.to_le_bytes());
            decoded_instructions += usize::from(instruction.kind.is_decoded());
        }
        let internal_call_targets = trace
            .call_targets
            .iter()
            .filter(|&&target| map.contains(target))
            .count();
        let trace_sha256 = digest.finalize().into();

        summaries.push(ExecutableTraceSummary {
            exe: map.name,
            text_bytes: text.len(),
            reached_instructions: trace.instructions.len(),
            decoded_instructions,
            unknown_instructions: trace.instructions.len() - decoded_instructions,
            call_targets: trace.call_targets.len(),
            internal_call_targets,
            external_call_targets: trace.call_targets.len() - internal_call_targets,
            indirect_sites: trace.indirect_sites.len(),
            out_of_range_edges: trace.out_of_range.len(),
            trace_sha256,
        });
    }

    if summaries.is_empty() {
        return Err(TraceInventoryError::EmptyInput);
    }
    Ok(summaries)
}

/// 고정 지원 원본의 11개 EXE가 모두 한 번씩 추적되었는지 검증한다.
pub fn verify_box_executable_traces<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
) -> Result<Vec<ExecutableTraceSummary>, TraceInventoryError> {
    let summaries = audit_executable_traces(executables)?;
    if summaries.len() != EXE_MAPS.len() {
        return Err(TraceInventoryError::BoxExecutableCount {
            expected: EXE_MAPS.len(),
            actual: summaries.len(),
        });
    }
    for map in EXE_MAPS {
        if !summaries.iter().any(|summary| summary.exe == map.name) {
            return Err(TraceInventoryError::MissingBoxExecutable(map.name));
        }
    }
    Ok(summaries)
}

/// 사람이 비교하고 diff할 수 있는 결정적 TSV를 만든다.
pub fn render_executable_trace_tsv(summaries: &[ExecutableTraceSummary]) -> String {
    let mut out = String::from(
        "exe\ttext_bytes\treached_instructions\tdecoded_instructions\
         \tunknown_instructions\tcall_targets\tinternal_call_targets\
         \texternal_call_targets\tindirect_sites\tout_of_range_edges\
         \topen_indirect_flow\ttrace_sha256\n",
    );
    for summary in summaries {
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            summary.exe,
            summary.text_bytes,
            summary.reached_instructions,
            summary.decoded_instructions,
            summary.unknown_instructions,
            summary.call_targets,
            summary.internal_call_targets,
            summary.external_call_targets,
            summary.indirect_sites,
            summary.out_of_range_edges,
            summary.has_open_indirect_flow(),
            hex_digest(&summary.trace_sha256),
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    out
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "trace_inventory_tests.rs"]
mod trace_inventory_tests;
