//! OP21 text stream과 파일 내부주소 후보의 직접 교차 원장.
//!
//! `quest_seq_text_streams`가 확정한 parseable OP21 record를
//! `quest_seq_inventory`의 같은-file target과 정확히 대조한다. pointer
//! consumer 형태가 확인된 record는 별도로 보존하지만, `TOWN00.SEQ` 이외
//! 파일에 `0x80199A70` base를 적용하는 일과 source record의 정상 실행
//! 도달은 여전히 후보다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::font::QuestCodeTable;

use super::quest_seq::QUEST_SEQ_ACTIVE_RUNTIME_BASE;
#[cfg(test)]
use super::quest_seq::TOWN00_SEQ_RUNTIME_BASE;
use super::quest_seq_inventory::{
    audit_quest_seq_addresses, verify_box_quest_seq_addresses, QuestSeqAddressAnalysis,
    QuestSeqAddressCandidate, QuestSeqAddressEvidence, QuestSeqPointerBehavior,
};
use super::quest_seq_text_streams::{
    verify_box_quest_seq_text_streams, QuestSeqTextStreamAnalysis,
    BOX_QUEST_SEQ_PARSEABLE_TEXT_STREAMS,
};

pub const QUEST_SEQ_TEXT_ROOT_SCHEMA_VERSION: &str = "quest-seq-text-roots-v1";
const TOWN00_NAME: &str = "TOWN00.SEQ";
pub const BOX_QUEST_SEQ_TEXT_ROOT_MARKERS: usize = BOX_QUEST_SEQ_PARSEABLE_TEXT_STREAMS;
pub const BOX_QUEST_SEQ_TEXT_ROOT_DIRECT_WORD_MARKERS: usize = 179;
pub const BOX_QUEST_SEQ_TEXT_ROOT_DIRECT_WORDS: usize = 260;
pub const BOX_QUEST_SEQ_TEXT_ROOT_CONSUMER_MARKERS: usize = 57;
pub const BOX_QUEST_SEQ_TEXT_ROOT_CONSUMER_EDGES: usize = 60;
pub const BOX_QUEST_SEQ_TEXT_ROOT_RANGE_ONLY_MARKERS: usize = 122;
pub const BOX_QUEST_SEQ_TEXT_ROOT_NO_DIRECT_WORD_MARKERS: usize = 426;
pub const BOX_QUEST_SEQ_TEXT_ROOT_RANGE_WORDS: usize = 200;
pub const BOX_QUEST_SEQ_TEXT_ROOT_OTHER_POINTER_WORDS: usize = 0;
pub const BOX_QUEST_SEQ_TEXT_ROOT_TOWN00_CONSUMER_MARKERS: usize = 11;
pub const BOX_QUEST_SEQ_TEXT_ROOT_TRANSFERRED_BASE_CONSUMER_MARKERS: usize = 46;
pub const BOX_QUEST_SEQ_TEXT_ROOT_RUNS: usize = 244;
pub const BOX_QUEST_SEQ_TEXT_ROOT_CODE_UNITS: usize = 2_420;
pub const BOX_QUEST_SEQ_TEXT_ROOT_JAPANESE_CODE_UNITS: usize = 1_861;
pub const BOX_QUEST_SEQ_TEXT_ROOT_REPORT_SHA256: &str =
    "6c3bdfae243663330cc8d1c3bab18a528b1e1b0a8f72d18d0ece1688043f109e";

/// `(파일, parseable marker, consumer marker, edge, rooted run, range-only, no-word)`.
pub const BOX_QUEST_SEQ_TEXT_ROOT_FILE_EXPECTATIONS: [(
    &str,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
); 12] = [
    ("HASI.SEQ", 18, 1, 1, 8, 0, 17),
    ("ISEKI.SEQ", 37, 1, 1, 4, 2, 34),
    ("KAWA.SEQ", 66, 9, 9, 59, 6, 51),
    ("MITI.SEQ", 11, 1, 1, 3, 1, 9),
    ("MORI.SEQ", 43, 3, 3, 14, 1, 39),
    ("SHEZ.SEQ", 29, 2, 5, 3, 0, 27),
    ("SUKE.SEQ", 16, 1, 1, 5, 1, 14),
    ("TOU.SEQ", 51, 1, 1, 5, 4, 46),
    ("TOWN00.SEQ", 110, 11, 11, 42, 28, 71),
    ("TOWN10.SEQ", 109, 12, 12, 49, 41, 56),
    ("TOWN20.SEQ", 81, 11, 11, 39, 24, 46),
    ("TOWN30.SEQ", 34, 4, 4, 13, 14, 16),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqTextRootEvidence {
    /// 검증된 stream-pointer 소비 record 형태가 OP21 record를 가리킨다.
    ConsumerBackedPointerShape,
    /// 같은-base 범위 u32만 OP21 record를 가리킨다.
    SameBaseRangeWordOnly,
    /// OP21 record를 직접 가리키는 정렬 u32가 없다.
    NoDirectIncomingWord,
}

impl QuestSeqTextRootEvidence {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ConsumerBackedPointerShape => "consumer-backed-pointer-shape-candidate",
            Self::SameBaseRangeWordOnly => "same-base-range-word-only-candidate",
            Self::NoDirectIncomingWord => "no-direct-incoming-word",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqTextRootBaseEvidence {
    /// `TOWN00.SEQ`는 loader와 active RAM에서 base를 확인했다.
    RuntimeConfirmedTown00,
    /// 다른 SEQ는 지원 원본의 공용 loader와 active copy에서 base를 확인했다.
    StaticCommonLoader,
}

impl QuestSeqTextRootBaseEvidence {
    pub const fn label(self) -> &'static str {
        match self {
            Self::RuntimeConfirmedTown00 => "runtime-confirmed-town00-base",
            Self::StaticCommonLoader => "static-common-loader-base",
        }
    }
}

/// source에서 소비 형태를 확인한 직접 incoming pointer 후보.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqTextRootEdge {
    pub source_record_offset: usize,
    pub pointer_field_offset: usize,
    pub opcode: u8,
    pub pointer_offset: usize,
    pub behavior: QuestSeqPointerBehavior,
    pub pointer_consumer_runtime: u32,
}

/// parseable OP21 marker 하나의 incoming word 교차 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextRootMarker {
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub record_offset: usize,
    pub stream_offset: usize,
    pub end_offset: usize,
    pub rooted_run_count: usize,
    pub base_evidence: QuestSeqTextRootBaseEvidence,
    pub evidence: QuestSeqTextRootEvidence,
    pub consumer_edges: Vec<QuestSeqTextRootEdge>,
    pub range_word_fields: Vec<usize>,
    pub other_pointer_fields: Vec<usize>,
}

impl QuestSeqTextRootMarker {
    pub fn direct_word_count(&self) -> usize {
        self.consumer_edges.len() + self.range_word_fields.len() + self.other_pointer_fields.len()
    }
}

/// 파일별 교차 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextRootSummary {
    pub sequence: String,
    pub parseable_markers: usize,
    pub consumer_markers: usize,
    pub consumer_edges: usize,
    pub rooted_runs: usize,
    pub range_only_markers: usize,
    pub no_direct_word_markers: usize,
}

/// 전체 OP21 marker의 직접 incoming word 교차 분석.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextRootAnalysis {
    pub summaries: Vec<QuestSeqTextRootSummary>,
    pub markers: Vec<QuestSeqTextRootMarker>,
    pub rooted_runs: usize,
    pub rooted_code_units: usize,
    pub rooted_japanese_code_units: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextRootError(String);

impl fmt::Display for QuestSeqTextRootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestSeqTextRootError {}

fn error(message: impl Into<String>) -> QuestSeqTextRootError {
    QuestSeqTextRootError(message.into())
}

fn is_stream_pointer_behavior(behavior: QuestSeqPointerBehavior) -> bool {
    matches!(
        behavior,
        QuestSeqPointerBehavior::SpawnStream
            | QuestSeqPointerBehavior::ScheduledStream
            | QuestSeqPointerBehavior::SavedStream
            | QuestSeqPointerBehavior::Branch
            | QuestSeqPointerBehavior::StackBranch
    )
}

fn consumer_edge(candidate: &QuestSeqAddressCandidate) -> Option<QuestSeqTextRootEdge> {
    let spec = candidate.pointer_spec?;
    if !is_stream_pointer_behavior(spec.behavior) {
        return None;
    }
    Some(QuestSeqTextRootEdge {
        source_record_offset: candidate.field_offset.checked_sub(spec.pointer_offset)?,
        pointer_field_offset: candidate.field_offset,
        opcode: spec.opcode,
        pointer_offset: spec.pointer_offset,
        behavior: spec.behavior,
        pointer_consumer_runtime: spec.pointer_consumer_runtime,
    })
}

/// 이미 같은 원본에서 만든 두 분석을 exact `(sequence, target_offset)`으로 교차한다.
pub fn audit_quest_seq_text_roots(
    text: &QuestSeqTextStreamAnalysis,
    addresses: &QuestSeqAddressAnalysis,
) -> Result<QuestSeqTextRootAnalysis, QuestSeqTextRootError> {
    let text_files = text
        .summaries
        .iter()
        .map(|summary| (summary.sequence.as_str(), summary.bytes))
        .collect::<BTreeMap<_, _>>();
    let address_files = addresses
        .summaries
        .iter()
        .map(|summary| (summary.sequence.as_str(), summary.bytes))
        .collect::<BTreeMap<_, _>>();
    if text_files != address_files {
        return Err(error(
            "QUEST SEQ text root 교차 입력의 파일 이름·길이 집합이 다름",
        ));
    }

    let mut incoming = BTreeMap::<(&str, usize), Vec<&QuestSeqAddressCandidate>>::new();
    for candidate in &addresses.candidates {
        incoming
            .entry((candidate.sequence.as_str(), candidate.target_offset))
            .or_default()
            .push(candidate);
    }

    let mut markers = Vec::new();
    let mut consumer_owner_records = BTreeSet::<(String, usize)>::new();
    for marker in text.markers.iter().filter(|marker| marker.is_parseable()) {
        let mut consumer_edges = Vec::new();
        let mut range_word_fields = Vec::new();
        let mut other_pointer_fields = Vec::new();
        for candidate in incoming
            .get(&(marker.sequence.as_str(), marker.record_offset))
            .into_iter()
            .flatten()
        {
            if let Some(edge) = consumer_edge(candidate) {
                consumer_edges.push(edge);
            } else if candidate.evidence == QuestSeqAddressEvidence::RangeCandidate {
                range_word_fields.push(candidate.field_offset);
            } else {
                other_pointer_fields.push(candidate.field_offset);
            }
        }
        consumer_edges.sort_unstable_by_key(|edge| {
            (
                edge.source_record_offset,
                edge.pointer_field_offset,
                edge.opcode,
                edge.pointer_consumer_runtime,
            )
        });
        consumer_edges.dedup();
        range_word_fields.sort_unstable();
        range_word_fields.dedup();
        other_pointer_fields.sort_unstable();
        other_pointer_fields.dedup();

        let evidence = if consumer_edges.is_empty() {
            if range_word_fields.is_empty() && other_pointer_fields.is_empty() {
                QuestSeqTextRootEvidence::NoDirectIncomingWord
            } else {
                QuestSeqTextRootEvidence::SameBaseRangeWordOnly
            }
        } else {
            consumer_owner_records.insert((marker.sequence.clone(), marker.record_offset));
            QuestSeqTextRootEvidence::ConsumerBackedPointerShape
        };
        markers.push(QuestSeqTextRootMarker {
            sequence: marker.sequence.clone(),
            sequence_sha256: marker.sequence_sha256,
            record_offset: marker.record_offset,
            stream_offset: marker.stream_offset,
            end_offset: marker
                .end_offset
                .expect("parseable OP21 marker에는 end offset이 있음"),
            rooted_run_count: marker.rooted_run_count,
            base_evidence: if marker.sequence == TOWN00_NAME {
                QuestSeqTextRootBaseEvidence::RuntimeConfirmedTown00
            } else {
                QuestSeqTextRootBaseEvidence::StaticCommonLoader
            },
            evidence,
            consumer_edges,
            range_word_fields,
            other_pointer_fields,
        });
    }

    let rooted = text
        .rooted_runs
        .iter()
        .filter(|run| {
            run.owner_record_offsets
                .iter()
                .any(|offset| consumer_owner_records.contains(&(run.sequence.clone(), *offset)))
        })
        .collect::<Vec<_>>();
    let rooted_code_units = rooted.iter().map(|run| run.code_units()).sum();
    let rooted_japanese_code_units = rooted.iter().map(|run| run.japanese_code_units).sum();

    let mut summaries = Vec::new();
    for text_summary in text
        .summaries
        .iter()
        .filter(|summary| summary.parseable_streams > 0)
    {
        let file_markers = markers
            .iter()
            .filter(|marker| marker.sequence == text_summary.sequence)
            .collect::<Vec<_>>();
        summaries.push(QuestSeqTextRootSummary {
            sequence: text_summary.sequence.clone(),
            parseable_markers: file_markers.len(),
            consumer_markers: file_markers
                .iter()
                .filter(|marker| {
                    marker.evidence == QuestSeqTextRootEvidence::ConsumerBackedPointerShape
                })
                .count(),
            consumer_edges: file_markers
                .iter()
                .map(|marker| marker.consumer_edges.len())
                .sum(),
            rooted_runs: file_markers
                .iter()
                .filter(|marker| {
                    marker.evidence == QuestSeqTextRootEvidence::ConsumerBackedPointerShape
                })
                .map(|marker| marker.rooted_run_count)
                .sum(),
            range_only_markers: file_markers
                .iter()
                .filter(|marker| marker.evidence == QuestSeqTextRootEvidence::SameBaseRangeWordOnly)
                .count(),
            no_direct_word_markers: file_markers
                .iter()
                .filter(|marker| marker.evidence == QuestSeqTextRootEvidence::NoDirectIncomingWord)
                .count(),
        });
    }

    Ok(QuestSeqTextRootAnalysis {
        summaries,
        markers,
        rooted_runs: rooted.len(),
        rooted_code_units,
        rooted_japanese_code_units,
    })
}

/// 지원 원본의 직접 incoming word 분모와 report digest를 검증한다.
pub fn verify_box_quest_seq_text_roots<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    code_table: &QuestCodeTable,
) -> Result<QuestSeqTextRootAnalysis, QuestSeqTextRootError> {
    let input = sequences.into_iter().collect::<Vec<_>>();
    let text = verify_box_quest_seq_text_streams(input.iter().copied(), code_table)
        .map_err(|source| error(source.to_string()))?;
    let addresses = audit_quest_seq_addresses(input.iter().copied())
        .map_err(|source| error(source.to_string()))?;
    verify_box_quest_seq_addresses(&addresses).map_err(|source| error(source.to_string()))?;
    let analysis = audit_quest_seq_text_roots(&text, &addresses)?;

    let direct_word_markers = analysis
        .markers
        .iter()
        .filter(|marker| marker.direct_word_count() > 0)
        .count();
    let direct_words = analysis
        .markers
        .iter()
        .map(QuestSeqTextRootMarker::direct_word_count)
        .sum();
    let consumer_markers = analysis
        .markers
        .iter()
        .filter(|marker| marker.evidence == QuestSeqTextRootEvidence::ConsumerBackedPointerShape)
        .count();
    let consumer_edges = analysis
        .markers
        .iter()
        .map(|marker| marker.consumer_edges.len())
        .sum();
    let range_only_markers = analysis
        .markers
        .iter()
        .filter(|marker| marker.evidence == QuestSeqTextRootEvidence::SameBaseRangeWordOnly)
        .count();
    let no_direct_word_markers = analysis
        .markers
        .iter()
        .filter(|marker| marker.evidence == QuestSeqTextRootEvidence::NoDirectIncomingWord)
        .count();
    let range_words = analysis
        .markers
        .iter()
        .map(|marker| marker.range_word_fields.len())
        .sum();
    let other_pointer_words = analysis
        .markers
        .iter()
        .map(|marker| marker.other_pointer_fields.len())
        .sum();
    let town00_consumer_markers = analysis
        .markers
        .iter()
        .filter(|marker| {
            marker.base_evidence == QuestSeqTextRootBaseEvidence::RuntimeConfirmedTown00
                && marker.evidence == QuestSeqTextRootEvidence::ConsumerBackedPointerShape
        })
        .count();
    let transferred_base_consumer_markers = consumer_markers - town00_consumer_markers;
    let actual = [
        analysis.markers.len(),
        direct_word_markers,
        direct_words,
        consumer_markers,
        consumer_edges,
        range_only_markers,
        no_direct_word_markers,
        range_words,
        other_pointer_words,
        town00_consumer_markers,
        transferred_base_consumer_markers,
        analysis.rooted_runs,
        analysis.rooted_code_units,
        analysis.rooted_japanese_code_units,
    ];
    let expected = [
        BOX_QUEST_SEQ_TEXT_ROOT_MARKERS,
        BOX_QUEST_SEQ_TEXT_ROOT_DIRECT_WORD_MARKERS,
        BOX_QUEST_SEQ_TEXT_ROOT_DIRECT_WORDS,
        BOX_QUEST_SEQ_TEXT_ROOT_CONSUMER_MARKERS,
        BOX_QUEST_SEQ_TEXT_ROOT_CONSUMER_EDGES,
        BOX_QUEST_SEQ_TEXT_ROOT_RANGE_ONLY_MARKERS,
        BOX_QUEST_SEQ_TEXT_ROOT_NO_DIRECT_WORD_MARKERS,
        BOX_QUEST_SEQ_TEXT_ROOT_RANGE_WORDS,
        BOX_QUEST_SEQ_TEXT_ROOT_OTHER_POINTER_WORDS,
        BOX_QUEST_SEQ_TEXT_ROOT_TOWN00_CONSUMER_MARKERS,
        BOX_QUEST_SEQ_TEXT_ROOT_TRANSFERRED_BASE_CONSUMER_MARKERS,
        BOX_QUEST_SEQ_TEXT_ROOT_RUNS,
        BOX_QUEST_SEQ_TEXT_ROOT_CODE_UNITS,
        BOX_QUEST_SEQ_TEXT_ROOT_JAPANESE_CODE_UNITS,
    ];
    if actual != expected {
        return Err(error(format!(
            "QUEST SEQ text root 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }

    let opcode_counts = [0xA1, 0xA2, 0xA8].map(|opcode| {
        analysis
            .markers
            .iter()
            .flat_map(|marker| &marker.consumer_edges)
            .filter(|edge| edge.opcode == opcode)
            .count()
    });
    if opcode_counts != [48, 1, 11] {
        return Err(error(format!(
            "QUEST SEQ text root opcode edge 분모 불일치: \
             expected=[48, 1, 11], actual={opcode_counts:?}"
        )));
    }

    let actual_files = analysis
        .summaries
        .iter()
        .map(|summary| {
            (
                summary.sequence.as_str(),
                summary.parseable_markers,
                summary.consumer_markers,
                summary.consumer_edges,
                summary.rooted_runs,
                summary.range_only_markers,
                summary.no_direct_word_markers,
            )
        })
        .collect::<Vec<_>>();
    if actual_files != BOX_QUEST_SEQ_TEXT_ROOT_FILE_EXPECTATIONS {
        return Err(error(format!(
            "QUEST SEQ text root 파일별 분모 불일치: \
             expected={BOX_QUEST_SEQ_TEXT_ROOT_FILE_EXPECTATIONS:?}, actual={actual_files:?}"
        )));
    }

    let report = render_quest_seq_text_roots_tsv(&analysis);
    let digest = format!("{:x}", Sha256::digest(report.as_bytes()));
    if digest != BOX_QUEST_SEQ_TEXT_ROOT_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST SEQ text root report SHA-256 불일치: \
             expected={BOX_QUEST_SEQ_TEXT_ROOT_REPORT_SHA256}, actual={digest}"
        )));
    }
    Ok(analysis)
}

fn join<T>(values: impl IntoIterator<Item = T>, render: impl Fn(T) -> String) -> String {
    values.into_iter().map(render).collect::<Vec<_>>().join(",")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// parseable OP21 marker 605개를 모두 보존하는 결정적 TSV.
pub fn render_quest_seq_text_roots_tsv(analysis: &QuestSeqTextRootAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tarchive_path\tarchive_sequence_count\tsequence_name\t\
         sequence_sha256\tsequence_runtime_base\tbase_evidence\top21_record_offset\t\
         stream_offset\tend_offset\trooted_run_count\tdirect_word_count\trange_word_count\t\
         range_word_fields\tother_pointer_word_count\tother_pointer_fields\t\
         consumer_edge_count\tconsumer_opcodes\tconsumer_record_starts\t\
         consumer_pointer_fields\tconsumer_behaviors\tconsumer_runtimes\tevidence\n",
    );
    for marker in &analysis.markers {
        writeln!(
            output,
            "{}\t/QUEST/RPG_SEQ.VDD;1\t113\t{}\t{}\t0x{:08X}\t{}\t0x{:04X}\t\
             0x{:04X}\t0x{:04X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            QUEST_SEQ_TEXT_ROOT_SCHEMA_VERSION,
            marker.sequence,
            hex(&marker.sequence_sha256),
            QUEST_SEQ_ACTIVE_RUNTIME_BASE,
            marker.base_evidence.label(),
            marker.record_offset,
            marker.stream_offset,
            marker.end_offset,
            marker.rooted_run_count,
            marker.direct_word_count(),
            marker.range_word_fields.len(),
            join(&marker.range_word_fields, |offset| format!(
                "0x{offset:04X}"
            )),
            marker.other_pointer_fields.len(),
            join(&marker.other_pointer_fields, |offset| format!(
                "0x{offset:04X}"
            )),
            marker.consumer_edges.len(),
            join(&marker.consumer_edges, |edge| format!(
                "0x{:02X}",
                edge.opcode
            )),
            join(&marker.consumer_edges, |edge| {
                format!("0x{:04X}", edge.source_record_offset)
            }),
            join(&marker.consumer_edges, |edge| {
                format!("0x{:04X}", edge.pointer_field_offset)
            }),
            join(&marker.consumer_edges, |edge| edge
                .behavior
                .label()
                .to_string()),
            join(&marker.consumer_edges, |edge| {
                format!("0x{:08X}", edge.pointer_consumer_runtime)
            }),
            marker.evidence.label(),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_seq_text_roots_tests.rs"]
mod tests;
