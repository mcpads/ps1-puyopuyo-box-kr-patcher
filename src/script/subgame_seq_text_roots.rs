//! 비-QUEST SEQ 가나 run과 같은 overlay 내부 pointer의 정적 교차.
//!
//! 가나 run은 번역 span이 아니다. 이 모듈은 각 후보의 시작점 또는 그 앞
//! 64바이트 안을 가리키는 정렬 u32 pointer만 찾아, pointer-rooted record를
//! 우선 조사할 수 있게 만든다. pointer가 있어도 실제 text consumer나 record
//! 경계가 확정된 것은 아니므로 모든 행의 쓰기 전략은 계속 보존 전용이다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::subgame_seq_layouts::{SubgameSeqLayoutAnalysis, SubgameSeqLayoutKind};
use super::subgame_seq_text_candidates::SubgameSeqTextCandidateAnalysis;

pub const SUBGAME_SEQ_TEXT_ROOT_SCHEMA_VERSION: &str = "subgame-seq-text-roots-v1";
/// 후보 앞 record header까지 보수적으로 함께 찾는 최대 거리.
pub const BOX_SUBGAME_SEQ_TEXT_ROOT_HEADER_BYTES: usize = 64;

// 지원 원본 기대값은 source gate가 고정한다.
pub const BOX_SUBGAME_SEQ_TEXT_ROOT_CANDIDATES: usize = 738;
pub const BOX_SUBGAME_SEQ_TEXT_ROOT_EXACT_CANDIDATES: usize = 152;
pub const BOX_SUBGAME_SEQ_TEXT_ROOT_HEADER_CANDIDATES: usize = 558;
pub const BOX_SUBGAME_SEQ_TEXT_ROOT_NONE_CANDIDATES: usize = 28;
pub const BOX_SUBGAME_SEQ_TEXT_ROOT_EXACT_RELATIONS: usize = 152;
pub const BOX_SUBGAME_SEQ_TEXT_ROOT_HEADER_RELATIONS: usize = 1_541;
pub const BOX_SUBGAME_SEQ_TEXT_ROOT_REPORT_SHA256: [u8; 32] = [
    0x30, 0x01, 0xF2, 0x29, 0x20, 0xDA, 0xFC, 0x1C, 0x56, 0xAA, 0xB6, 0x7B, 0xA3, 0xE6, 0x62, 0x7A,
    0x6B, 0x58, 0xFA, 0xB5, 0xA6, 0x4A, 0x71, 0xC7, 0x12, 0x8F, 0x52, 0x98, 0x04, 0xA1, 0x68, 0x74,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubgameSeqTextPointerEvidence {
    ExactStart,
    PrecedingHeader,
    None,
}

impl SubgameSeqTextPointerEvidence {
    fn label(self) -> &'static str {
        match self {
            Self::ExactStart => "exact-candidate-start",
            Self::PrecedingHeader => "preceding-record-candidate",
            Self::None => "no-nearby-internal-pointer",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqTextPointerRoot {
    pub pointer_field_offset: usize,
    pub pointer_field_runtime: u32,
    pub target_offset: usize,
    pub target_runtime: u32,
    pub candidate_delta: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextRootRow {
    pub candidate_id: String,
    pub scope: &'static str,
    pub archive: String,
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub candidate_offset: usize,
    pub candidate_bytes: usize,
    pub source: Option<String>,
    pub evidence: SubgameSeqTextPointerEvidence,
    pub roots: Vec<SubgameSeqTextPointerRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextRootAnalysis {
    pub rows: Vec<SubgameSeqTextRootRow>,
    pub exact_candidates: usize,
    pub header_candidates: usize,
    pub none_candidates: usize,
    pub exact_relations: usize,
    pub header_relations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextRootError(String);

impl fmt::Display for SubgameSeqTextRootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqTextRootError {}

fn error(message: impl Into<String>) -> SubgameSeqTextRootError {
    SubgameSeqTextRootError(message.into())
}

/// 해제된 SEQ, 가나 run 하한, overlay footer 계약을 교차한다.
pub fn audit_subgame_seq_text_roots<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    text: &SubgameSeqTextCandidateAnalysis,
    layouts: &SubgameSeqLayoutAnalysis,
) -> Result<SubgameSeqTextRootAnalysis, SubgameSeqTextRootError> {
    let mut input = BTreeMap::<(String, String), &'a [u8]>::new();
    for (archive, sequence, raw) in sources {
        let key = (archive.to_string(), sequence.to_string());
        if input.insert(key.clone(), raw).is_some() {
            return Err(error(format!(
                "subgame SEQ text root 입력 중복: {}:{}",
                key.0, key.1
            )));
        }
    }
    if input.len() != text.summaries.len() || input.len() != layouts.rows.len() {
        return Err(error(format!(
            "subgame SEQ text root 입력 분모 불일치: source={}, text={}, layout={}",
            input.len(),
            text.summaries.len(),
            layouts.rows.len()
        )));
    }

    let mut pointers_by_file = BTreeMap::<(String, String), Vec<(usize, usize, u32, u32)>>::new();
    for layout in &layouts.rows {
        let key = (layout.archive.clone(), layout.sequence.clone());
        let raw = input.get(&key).ok_or_else(|| {
            error(format!(
                "subgame SEQ layout의 root 원본이 없음: {}:{}",
                layout.archive, layout.sequence
            ))
        })?;
        if raw.len() != layout.bytes || Sha256::digest(raw).as_slice() != layout.sequence_sha256 {
            return Err(error(format!(
                "subgame SEQ layout/root 원본 불일치: {}:{}",
                layout.archive, layout.sequence
            )));
        }
        let mut pointers = Vec::new();
        if layout.kind == SubgameSeqLayoutKind::Overlay {
            let footer = layout
                .footer
                .ok_or_else(|| error("overlay 행에 footer가 없음"))?;
            let map = footer
                .runtime_map()
                .ok_or_else(|| error("overlay runtime 사상 overflow"))?;
            let initialized_len = usize::try_from(footer.initialized_len)
                .map_err(|_| error("overlay initialized 길이 변환 실패"))?;
            for field_offset in (0..initialized_len).step_by(4) {
                let Some(field_end) = field_offset.checked_add(4) else {
                    return Err(error("pointer field 범위 overflow"));
                };
                if field_end > initialized_len {
                    break;
                }
                let value = u32::from_le_bytes(
                    raw[field_offset..field_end]
                        .try_into()
                        .expect("4바이트 pointer field"),
                );
                let Some(target_offset) = map.file_offset(value) else {
                    continue;
                };
                let pointer_field_runtime = map
                    .runtime_addr(field_offset)
                    .ok_or_else(|| error("pointer field runtime 변환 실패"))?;
                pointers.push((field_offset, target_offset, pointer_field_runtime, value));
            }
        }
        pointers_by_file.insert(key, pointers);
    }

    let mut rows = Vec::with_capacity(text.candidates.len());
    for candidate in &text.candidates {
        let key = (candidate.archive.clone(), candidate.sequence.clone());
        let raw = input.get(&key).ok_or_else(|| {
            error(format!(
                "subgame SEQ text 후보의 root 원본이 없음: {}:{}",
                candidate.archive, candidate.sequence
            ))
        })?;
        let candidate_end = candidate
            .offset
            .checked_add(candidate.raw.len())
            .ok_or_else(|| error("text 후보 범위 overflow"))?;
        if raw.get(candidate.offset..candidate_end) != Some(candidate.raw.as_slice()) {
            return Err(error(format!(
                "subgame SEQ text 후보/root 원본 불일치: {}",
                candidate.id()
            )));
        }
        let pointers = pointers_by_file
            .get(&key)
            .ok_or_else(|| error("text 후보 파일의 pointer 집합이 없음"))?;
        let mut roots = pointers
            .iter()
            .filter_map(
                |&(pointer_field_offset, target_offset, pointer_field_runtime, target_runtime)| {
                    let delta = candidate.offset.checked_sub(target_offset)?;
                    (delta <= BOX_SUBGAME_SEQ_TEXT_ROOT_HEADER_BYTES).then_some(
                        SubgameSeqTextPointerRoot {
                            pointer_field_offset,
                            pointer_field_runtime,
                            target_offset,
                            target_runtime,
                            candidate_delta: delta,
                        },
                    )
                },
            )
            .collect::<Vec<_>>();
        roots.sort_by_key(|root| (root.candidate_delta, root.pointer_field_offset));
        let evidence = if roots.iter().any(|root| root.candidate_delta == 0) {
            SubgameSeqTextPointerEvidence::ExactStart
        } else if roots.is_empty() {
            SubgameSeqTextPointerEvidence::None
        } else {
            SubgameSeqTextPointerEvidence::PrecedingHeader
        };
        rows.push(SubgameSeqTextRootRow {
            candidate_id: candidate.id(),
            scope: candidate.scope,
            archive: candidate.archive.clone(),
            sequence: candidate.sequence.clone(),
            sequence_sha256: candidate.sequence_sha256,
            candidate_offset: candidate.offset,
            candidate_bytes: candidate.raw.len(),
            source: candidate.decoded.clone(),
            evidence,
            roots,
        });
    }

    let exact_candidates = rows
        .iter()
        .filter(|row| row.evidence == SubgameSeqTextPointerEvidence::ExactStart)
        .count();
    let header_candidates = rows
        .iter()
        .filter(|row| row.evidence == SubgameSeqTextPointerEvidence::PrecedingHeader)
        .count();
    let none_candidates = rows.len() - exact_candidates - header_candidates;
    let exact_relations = rows
        .iter()
        .flat_map(|row| &row.roots)
        .filter(|root| root.candidate_delta == 0)
        .count();
    let header_relations = rows
        .iter()
        .flat_map(|row| &row.roots)
        .filter(|root| root.candidate_delta != 0)
        .count();

    Ok(SubgameSeqTextRootAnalysis {
        rows,
        exact_candidates,
        header_candidates,
        none_candidates,
        exact_relations,
        header_relations,
    })
}

/// 지원 원본의 pointer-rooted 정적 하한을 고정한다.
pub fn verify_box_subgame_seq_text_roots<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    text: &SubgameSeqTextCandidateAnalysis,
    layouts: &SubgameSeqLayoutAnalysis,
) -> Result<SubgameSeqTextRootAnalysis, SubgameSeqTextRootError> {
    let analysis = audit_subgame_seq_text_roots(sources, text, layouts)?;
    let actual = [
        analysis.rows.len(),
        analysis.exact_candidates,
        analysis.header_candidates,
        analysis.none_candidates,
        analysis.exact_relations,
        analysis.header_relations,
    ];
    let expected = [
        BOX_SUBGAME_SEQ_TEXT_ROOT_CANDIDATES,
        BOX_SUBGAME_SEQ_TEXT_ROOT_EXACT_CANDIDATES,
        BOX_SUBGAME_SEQ_TEXT_ROOT_HEADER_CANDIDATES,
        BOX_SUBGAME_SEQ_TEXT_ROOT_NONE_CANDIDATES,
        BOX_SUBGAME_SEQ_TEXT_ROOT_EXACT_RELATIONS,
        BOX_SUBGAME_SEQ_TEXT_ROOT_HEADER_RELATIONS,
    ];
    if actual != expected {
        return Err(error(format!(
            "subgame SEQ text root 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }
    let report = render_subgame_seq_text_roots_tsv(&analysis);
    let report_sha256: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if report_sha256 != BOX_SUBGAME_SEQ_TEXT_ROOT_REPORT_SHA256 {
        return Err(error(format!(
            "subgame SEQ text root report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_TEXT_ROOT_REPORT_SHA256),
            hex(&report_sha256)
        )));
    }
    Ok(analysis)
}

/// 후보별 내부 pointer 근거를 결정적 TSV로 만든다.
pub fn render_subgame_seq_text_roots_tsv(analysis: &SubgameSeqTextRootAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tcandidate_id\tscope\tarchive\tsequence\tsequence_sha256\t\
         candidate_offset\tcandidate_bytes\tpointer_evidence\tpointer_relations\t\
         pointer_field_offsets\tpointer_field_runtimes\ttarget_offsets\ttarget_runtimes\t\
         candidate_deltas\tconsumer_contract\tpatch_strategy\tsource\n",
    );
    for row in &analysis.rows {
        let join = |values: Vec<String>| values.join(",");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
             unknown-subgame-seq-record-consumer\t\
             preserve-no-write-until-record-consumer-and-control-contract\t{}",
            SUBGAME_SEQ_TEXT_ROOT_SCHEMA_VERSION,
            row.candidate_id,
            row.scope,
            row.archive,
            row.sequence,
            hex(&row.sequence_sha256),
            row.candidate_offset,
            row.candidate_bytes,
            row.evidence.label(),
            row.roots.len(),
            join(
                row.roots
                    .iter()
                    .map(|root| format!("0x{:04X}", root.pointer_field_offset))
                    .collect()
            ),
            join(
                row.roots
                    .iter()
                    .map(|root| format!("0x{:08X}", root.pointer_field_runtime))
                    .collect()
            ),
            join(
                row.roots
                    .iter()
                    .map(|root| format!("0x{:04X}", root.target_offset))
                    .collect()
            ),
            join(
                row.roots
                    .iter()
                    .map(|root| format!("0x{:08X}", root.target_runtime))
                    .collect()
            ),
            join(
                row.roots
                    .iter()
                    .map(|root| root.candidate_delta.to_string())
                    .collect()
            ),
            row.source.as_deref().map(escape_tsv).unwrap_or_default(),
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
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
#[path = "subgame_seq_text_roots_tests.rs"]
mod subgame_seq_text_roots_tests;
