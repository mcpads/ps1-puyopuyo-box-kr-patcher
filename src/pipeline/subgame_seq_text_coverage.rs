//! 비-QUEST 가나 후보 원장을 확인된 text surface와 보호 경계로 분해한다.
//!
//! 가나 scanner의 한 행은 번역 단위가 아니다. 이 모듈은 E6 span, E0 message
//! block, E1 repertoire, MIN의 비참조 보호 복사와 custom byte record에 후보를
//! 귀속한다. 어느 구조에도 속하지 않는 후보는 확인된 u16 consumer가 읽을 수
//! 있는 정렬인지 따로 판정한다. 홀수 주소 후보를 게임 전체의 비텍스트로
//! 일반화하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::analysis::SubgameSeqSceneGlyphAnalysis;
use crate::disasm::SubgameSeqE6TextAnalysis;
use crate::script::{
    SubgameSeqCustomByteRecordAnalysis, SubgameSeqSceneTextAnalysis, SubgameSeqSceneTextKind,
    SubgameSeqTextCandidateAnalysis, SubgameSeqTextPointerEvidence, SubgameSeqTextRootAnalysis,
    BOX_SUBGAME_SEQ_CUSTOM_BYTE_CANDIDATE_ID,
};

use super::SubgameSeqSceneRelinkAnalysis;

pub const SUBGAME_SEQ_TEXT_COVERAGE_SCHEMA_VERSION: &str = "subgame-seq-text-coverage-v2";

pub const BOX_SUBGAME_SEQ_TEXT_COVERAGE_ROWS: usize = 738;
pub const BOX_SUBGAME_SEQ_TEXT_COVERAGE_E6: usize = 266;
pub const BOX_SUBGAME_SEQ_TEXT_COVERAGE_SCENE_MESSAGE: usize = 370;
pub const BOX_SUBGAME_SEQ_TEXT_COVERAGE_SCENE_MESSAGE_REPERTOIRE: usize = 16;
pub const BOX_SUBGAME_SEQ_TEXT_COVERAGE_SCENE_REPERTOIRE: usize = 34;
pub const BOX_SUBGAME_SEQ_TEXT_COVERAGE_PROTECTED_COPY: usize = 2;
pub const BOX_SUBGAME_SEQ_TEXT_COVERAGE_CUSTOM_BYTE_RECORD: usize = 1;
pub const BOX_SUBGAME_SEQ_TEXT_COVERAGE_MISALIGNED: usize = 49;
pub const BOX_SUBGAME_SEQ_TEXT_COVERAGE_ALIGNED_UNRESOLVED: usize = 0;

pub const BOX_SUBGAME_SEQ_TEXT_COVERAGE_REPORT_SHA256: [u8; 32] = [
    0xE9, 0xA9, 0x42, 0x79, 0x76, 0xCF, 0x34, 0x26, 0x36, 0x82, 0x14, 0x82, 0x50, 0x13, 0x53, 0x59,
    0xD3, 0x7B, 0xB3, 0xDE, 0xCB, 0x60, 0xA6, 0x38, 0x2A, 0x3A, 0x73, 0xDA, 0x7B, 0xB0, 0x42, 0x8D,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SubgameSeqTextCoverageDisposition {
    E6Text,
    SceneMessage,
    SceneMessageInsideRepertoire,
    SceneRepertoire,
    ProtectedUnreferencedCopy,
    CustomByteRecord,
    MisalignedForConfirmedU16Consumers,
    AlignedUnresolved,
}

impl SubgameSeqTextCoverageDisposition {
    pub const fn label(self) -> &'static str {
        match self {
            Self::E6Text => "e6-text",
            Self::SceneMessage => "scene-message",
            Self::SceneMessageInsideRepertoire => "scene-message-inside-repertoire",
            Self::SceneRepertoire => "scene-repertoire",
            Self::ProtectedUnreferencedCopy => "protected-unreferenced-copy",
            Self::CustomByteRecord => "custom-byte-record",
            Self::MisalignedForConfirmedU16Consumers => "misaligned-for-confirmed-u16-consumers",
            Self::AlignedUnresolved => "aligned-unresolved",
        }
    }

    pub const fn consumer_contract(self) -> &'static str {
        match self {
            Self::E6Text => "e6-direct-pair-parser",
            Self::SceneMessage => "e0-pair-token-consumer",
            Self::SceneMessageInsideRepertoire => "e0-message-overlaps-e1-protected-repertoire",
            Self::SceneRepertoire => "e1-pair-table-structural-source",
            Self::ProtectedUnreferencedCopy => "no-direct-e0-pointer-protected-source-gap",
            Self::CustomByteRecord => "pointer-delimited-custom-byte-record-consumer-unverified",
            Self::MisalignedForConfirmedU16Consumers => {
                "all-confirmed-subgame-text-consumers-require-even-u16-address"
            }
            Self::AlignedUnresolved => "no-confirmed-record-or-consumer-owner",
        }
    }

    pub const fn patch_strategy(self) -> &'static str {
        match self {
            Self::E6Text => "e6-bounded-relink-and-glyph-bank",
            Self::SceneMessage | Self::SceneMessageInsideRepertoire => {
                "scene-e0-bounded-relink-and-family-glyph-bank"
            }
            Self::SceneRepertoire => "not-standalone-follow-scene-glyph-family",
            Self::ProtectedUnreferencedCopy
            | Self::CustomByteRecord
            | Self::MisalignedForConfirmedU16Consumers
            | Self::AlignedUnresolved => "protected-no-write",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextCoverageRow {
    pub candidate_id: String,
    pub scope: &'static str,
    pub archive: String,
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub offset: usize,
    pub raw: Vec<u8>,
    pub strict_sjis: bool,
    pub source: Option<String>,
    pub pointer_evidence: SubgameSeqTextPointerEvidence,
    pub disposition: SubgameSeqTextCoverageDisposition,
    pub owners: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextCoverageAnalysis {
    pub rows: Vec<SubgameSeqTextCoverageRow>,
    pub disposition_counts: BTreeMap<SubgameSeqTextCoverageDisposition, usize>,
}

impl SubgameSeqTextCoverageAnalysis {
    pub fn count(&self, disposition: SubgameSeqTextCoverageDisposition) -> usize {
        self.disposition_counts
            .get(&disposition)
            .copied()
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextCoverageError(String);

impl fmt::Display for SubgameSeqTextCoverageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqTextCoverageError {}

fn error(message: impl Into<String>) -> SubgameSeqTextCoverageError {
    SubgameSeqTextCoverageError(message.into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OwnerInterval {
    start: usize,
    end: usize,
    owner: String,
}

#[derive(Debug, Default)]
struct OwnerIndex {
    e6_by_candidate: BTreeMap<String, Vec<String>>,
    custom_by_candidate: BTreeMap<String, Vec<String>>,
    messages: BTreeMap<(&'static str, &'static str), Vec<OwnerInterval>>,
    repertoires: BTreeMap<(&'static str, &'static str), Vec<OwnerInterval>>,
    min_messages: BTreeMap<(&'static str, &'static str), Vec<OwnerInterval>>,
}

fn interval_owners(
    intervals: &BTreeMap<(&'static str, &'static str), Vec<OwnerInterval>>,
    scope: &str,
    sequence: &str,
    offset: usize,
) -> Vec<String> {
    intervals
        .iter()
        .filter(|((candidate_scope, candidate_sequence), _)| {
            *candidate_scope == scope && *candidate_sequence == sequence
        })
        .flat_map(|(_, ranges)| ranges)
        .filter(|range| range.start <= offset && offset < range.end)
        .map(|range| range.owner.clone())
        .collect()
}

fn classify(
    offset: usize,
    e6_owners: &[String],
    custom_owners: &[String],
    message_owners: &[String],
    repertoire_owners: &[String],
    min_message_owners: &[String],
) -> Result<SubgameSeqTextCoverageDisposition, SubgameSeqTextCoverageError> {
    if e6_owners.len() > 1
        || custom_owners.len() > 1
        || message_owners.len() > 1
        || repertoire_owners.len() > 1
        || min_message_owners.len() > 1
    {
        return Err(error("subgame SEQ 후보의 같은 종류 owner가 중복됨"));
    }
    if (!e6_owners.is_empty()
        && (!custom_owners.is_empty()
            || !message_owners.is_empty()
            || !repertoire_owners.is_empty()
            || !min_message_owners.is_empty()))
        || (!custom_owners.is_empty()
            && (!message_owners.is_empty()
                || !repertoire_owners.is_empty()
                || !min_message_owners.is_empty()))
    {
        return Err(error("subgame SEQ 후보가 배타적인 owner 종류와 겹침"));
    }
    if !message_owners.is_empty() {
        return Ok(if repertoire_owners.is_empty() {
            SubgameSeqTextCoverageDisposition::SceneMessage
        } else {
            SubgameSeqTextCoverageDisposition::SceneMessageInsideRepertoire
        });
    }
    if !e6_owners.is_empty() {
        return Ok(SubgameSeqTextCoverageDisposition::E6Text);
    }
    if !custom_owners.is_empty() {
        return Ok(SubgameSeqTextCoverageDisposition::CustomByteRecord);
    }
    if !min_message_owners.is_empty() {
        return Ok(SubgameSeqTextCoverageDisposition::ProtectedUnreferencedCopy);
    }
    if !repertoire_owners.is_empty() {
        return Ok(SubgameSeqTextCoverageDisposition::SceneRepertoire);
    }
    Ok(if offset.is_multiple_of(2) {
        SubgameSeqTextCoverageDisposition::AlignedUnresolved
    } else {
        SubgameSeqTextCoverageDisposition::MisalignedForConfirmedU16Consumers
    })
}

fn build_owner_index(
    e6: &SubgameSeqE6TextAnalysis,
    glyphs: &SubgameSeqSceneGlyphAnalysis,
    relinks: &SubgameSeqSceneRelinkAnalysis,
    min_text: &SubgameSeqSceneTextAnalysis,
    custom_records: &SubgameSeqCustomByteRecordAnalysis,
) -> Result<OwnerIndex, SubgameSeqTextCoverageError> {
    let mut index = OwnerIndex::default();
    for row in &e6.rows {
        for span in &row.spans {
            index
                .e6_by_candidate
                .entry(span.candidate_id.clone())
                .or_default()
                .push(format!(
                    "{}:{}:E6@0x{:04X}",
                    row.scope, row.sequence, row.command_offset
                ));
        }
    }
    for row in &custom_records.rows {
        index
            .custom_by_candidate
            .entry(row.candidate_id.clone())
            .or_default()
            .push(row.owner_id());
    }
    for row in &relinks.rows {
        let ranges = index.messages.entry((row.scope, row.sequence)).or_default();
        for block in &row.blocks {
            if block.start_offset >= block.end_offset {
                return Err(error(format!(
                    "{}:{} scene block 범위가 비어 있음",
                    row.scope, row.sequence
                )));
            }
            ranges.push(OwnerInterval {
                start: block.start_offset,
                end: block.end_offset,
                owner: format!("{}:{}:E0-BLOCK-{}", row.scope, row.sequence, block.index),
            });
        }
    }
    for row in &glyphs.rows {
        let end = row
            .repertoire_offset
            .checked_add(row.repertoire_bytes)
            .ok_or_else(|| error("scene repertoire 범위 overflow"))?;
        index
            .repertoires
            .entry((row.scope, row.sequence))
            .or_default()
            .push(OwnerInterval {
                start: row.repertoire_offset,
                end,
                owner: format!(
                    "{}:{}:E1@0x{:04X}",
                    row.scope, row.sequence, row.e1_record_offset
                ),
            });
    }
    for row in &min_text.rows {
        if row.kind != SubgameSeqSceneTextKind::Message {
            continue;
        }
        let end = row
            .record_offset
            .checked_add(row.raw.len())
            .ok_or_else(|| error("MIN message 보호 범위 overflow"))?;
        index
            .min_messages
            .entry((row.scope, row.sequence))
            .or_default()
            .push(OwnerInterval {
                start: row.record_offset,
                end,
                owner: format!("{}:{}:MIN-MESSAGE-{}", row.scope, row.sequence, row.index),
            });
    }
    Ok(index)
}

/// 가나 후보 전량을 확인된 surface·보호 경계·미확정 정렬로 분해한다.
pub fn audit_subgame_seq_text_coverage(
    candidates: &SubgameSeqTextCandidateAnalysis,
    roots: &SubgameSeqTextRootAnalysis,
    e6: &SubgameSeqE6TextAnalysis,
    glyphs: &SubgameSeqSceneGlyphAnalysis,
    relinks: &SubgameSeqSceneRelinkAnalysis,
    min_text: &SubgameSeqSceneTextAnalysis,
    custom_records: &SubgameSeqCustomByteRecordAnalysis,
) -> Result<SubgameSeqTextCoverageAnalysis, SubgameSeqTextCoverageError> {
    let index = build_owner_index(e6, glyphs, relinks, min_text, custom_records)?;
    let roots_by_id = roots
        .rows
        .iter()
        .map(|row| (row.candidate_id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    if roots_by_id.len() != candidates.candidates.len() {
        return Err(error("subgame SEQ candidate/root 분모 불일치"));
    }

    let mut rows = Vec::with_capacity(candidates.candidates.len());
    for candidate in &candidates.candidates {
        let candidate_id = candidate.id();
        let root = roots_by_id
            .get(candidate_id.as_str())
            .ok_or_else(|| error(format!("후보의 root 행이 없음: {candidate_id}")))?;
        if root.scope != candidate.scope
            || root.archive != candidate.archive
            || root.sequence != candidate.sequence
            || root.sequence_sha256 != candidate.sequence_sha256
            || root.candidate_offset != candidate.offset
            || root.candidate_bytes != candidate.raw.len()
        {
            return Err(error(format!("후보/root 보호 필드 불일치: {candidate_id}")));
        }

        let e6_owners = index
            .e6_by_candidate
            .get(&candidate_id)
            .cloned()
            .unwrap_or_default();
        let custom_owners = index
            .custom_by_candidate
            .get(&candidate_id)
            .cloned()
            .unwrap_or_default();
        let message_owners = interval_owners(
            &index.messages,
            candidate.scope,
            &candidate.sequence,
            candidate.offset,
        );
        let repertoire_owners = interval_owners(
            &index.repertoires,
            candidate.scope,
            &candidate.sequence,
            candidate.offset,
        );
        let min_message_owners = interval_owners(
            &index.min_messages,
            candidate.scope,
            &candidate.sequence,
            candidate.offset,
        );
        let disposition = classify(
            candidate.offset,
            &e6_owners,
            &custom_owners,
            &message_owners,
            &repertoire_owners,
            &min_message_owners,
        )?;
        let mut owners = e6_owners;
        owners.extend(custom_owners);
        owners.extend(message_owners);
        owners.extend(repertoire_owners);
        if disposition == SubgameSeqTextCoverageDisposition::ProtectedUnreferencedCopy {
            owners.extend(min_message_owners);
        }
        owners.sort();
        owners.dedup();
        rows.push(SubgameSeqTextCoverageRow {
            candidate_id,
            scope: candidate.scope,
            archive: candidate.archive.clone(),
            sequence: candidate.sequence.clone(),
            sequence_sha256: candidate.sequence_sha256,
            offset: candidate.offset,
            raw: candidate.raw.clone(),
            strict_sjis: candidate.decoded.is_some(),
            source: candidate.decoded.clone(),
            pointer_evidence: root.evidence,
            disposition,
            owners,
        });
    }
    rows.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let mut disposition_counts = BTreeMap::new();
    for row in &rows {
        *disposition_counts.entry(row.disposition).or_insert(0) += 1;
    }
    Ok(SubgameSeqTextCoverageAnalysis {
        rows,
        disposition_counts,
    })
}

/// 지원 원본의 738개 후보 분해와 custom record owner를 고정한다.
pub fn verify_box_subgame_seq_text_coverage(
    candidates: &SubgameSeqTextCandidateAnalysis,
    roots: &SubgameSeqTextRootAnalysis,
    e6: &SubgameSeqE6TextAnalysis,
    glyphs: &SubgameSeqSceneGlyphAnalysis,
    relinks: &SubgameSeqSceneRelinkAnalysis,
    min_text: &SubgameSeqSceneTextAnalysis,
    custom_records: &SubgameSeqCustomByteRecordAnalysis,
) -> Result<SubgameSeqTextCoverageAnalysis, SubgameSeqTextCoverageError> {
    let analysis = audit_subgame_seq_text_coverage(
        candidates,
        roots,
        e6,
        glyphs,
        relinks,
        min_text,
        custom_records,
    )?;
    let expected = [
        (
            SubgameSeqTextCoverageDisposition::E6Text,
            BOX_SUBGAME_SEQ_TEXT_COVERAGE_E6,
        ),
        (
            SubgameSeqTextCoverageDisposition::SceneMessage,
            BOX_SUBGAME_SEQ_TEXT_COVERAGE_SCENE_MESSAGE,
        ),
        (
            SubgameSeqTextCoverageDisposition::SceneMessageInsideRepertoire,
            BOX_SUBGAME_SEQ_TEXT_COVERAGE_SCENE_MESSAGE_REPERTOIRE,
        ),
        (
            SubgameSeqTextCoverageDisposition::SceneRepertoire,
            BOX_SUBGAME_SEQ_TEXT_COVERAGE_SCENE_REPERTOIRE,
        ),
        (
            SubgameSeqTextCoverageDisposition::ProtectedUnreferencedCopy,
            BOX_SUBGAME_SEQ_TEXT_COVERAGE_PROTECTED_COPY,
        ),
        (
            SubgameSeqTextCoverageDisposition::CustomByteRecord,
            BOX_SUBGAME_SEQ_TEXT_COVERAGE_CUSTOM_BYTE_RECORD,
        ),
        (
            SubgameSeqTextCoverageDisposition::MisalignedForConfirmedU16Consumers,
            BOX_SUBGAME_SEQ_TEXT_COVERAGE_MISALIGNED,
        ),
        (
            SubgameSeqTextCoverageDisposition::AlignedUnresolved,
            BOX_SUBGAME_SEQ_TEXT_COVERAGE_ALIGNED_UNRESOLVED,
        ),
    ];
    if analysis.rows.len() != BOX_SUBGAME_SEQ_TEXT_COVERAGE_ROWS {
        return Err(error(format!(
            "지원 원본 subgame 후보 분모 불일치: expected={}, actual={}",
            BOX_SUBGAME_SEQ_TEXT_COVERAGE_ROWS,
            analysis.rows.len()
        )));
    }
    for (disposition, count) in expected {
        let actual = analysis.count(disposition);
        if actual != count {
            return Err(error(format!(
                "지원 원본 subgame 후보 {} 수 불일치: expected={count}, actual={actual}",
                disposition.label()
            )));
        }
    }
    let aligned = analysis
        .rows
        .iter()
        .filter(|row| row.disposition == SubgameSeqTextCoverageDisposition::AlignedUnresolved)
        .map(|row| row.candidate_id.as_str())
        .collect::<BTreeSet<_>>();
    if !aligned.is_empty() {
        return Err(error(format!(
            "지원 원본 정렬 미확정 후보 불일치: {aligned:?}"
        )));
    }
    let custom = analysis
        .rows
        .iter()
        .filter(|row| row.disposition == SubgameSeqTextCoverageDisposition::CustomByteRecord)
        .map(|row| row.candidate_id.as_str())
        .collect::<BTreeSet<_>>();
    if custom != BTreeSet::from([BOX_SUBGAME_SEQ_CUSTOM_BYTE_CANDIDATE_ID]) {
        return Err(error(format!(
            "지원 원본 custom byte record 후보 불일치: {custom:?}"
        )));
    }
    let report = render_subgame_seq_text_coverage_tsv(&analysis);
    let actual_digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if actual_digest != BOX_SUBGAME_SEQ_TEXT_COVERAGE_REPORT_SHA256 {
        return Err(error(format!(
            "지원 원본 subgame 후보 coverage report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_TEXT_COVERAGE_REPORT_SHA256),
            hex(&actual_digest)
        )));
    }
    Ok(analysis)
}

fn pointer_evidence_label(evidence: SubgameSeqTextPointerEvidence) -> &'static str {
    match evidence {
        SubgameSeqTextPointerEvidence::ExactStart => "exact-candidate-start",
        SubgameSeqTextPointerEvidence::PrecedingHeader => "preceding-record-candidate",
        SubgameSeqTextPointerEvidence::None => "no-nearby-internal-pointer",
    }
}

/// 후보의 보호 원문과 patch family 귀속을 같은 행에서 검토하는 TSV.
pub fn render_subgame_seq_text_coverage_tsv(analysis: &SubgameSeqTextCoverageAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tcandidate_id\tscope\tarchive\tsequence\tsequence_sha256\toffset\t\
         raw_hex\tbyte_len\tstrict_sjis\tpointer_evidence\tdisposition\towners\t\
         consumer_contract\tpatch_strategy\tstrategy_status\tsource\n",
    );
    for row in &analysis.rows {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            SUBGAME_SEQ_TEXT_COVERAGE_SCHEMA_VERSION,
            row.candidate_id,
            row.scope,
            row.archive,
            row.sequence,
            hex(&row.sequence_sha256),
            row.offset,
            hex(&row.raw),
            row.raw.len(),
            row.strict_sjis,
            pointer_evidence_label(row.pointer_evidence),
            row.disposition.label(),
            row.owners.join(","),
            row.disposition.consumer_contract(),
            row.disposition.patch_strategy(),
            if matches!(
                row.disposition,
                SubgameSeqTextCoverageDisposition::CustomByteRecord
                    | SubgameSeqTextCoverageDisposition::AlignedUnresolved
            ) {
                "analysis-required"
            } else {
                "mechanism-or-protection-selected"
            },
            escape_tsv(row.source.as_deref().unwrap_or("")),
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_text_coverage_tests.rs"]
mod tests;
