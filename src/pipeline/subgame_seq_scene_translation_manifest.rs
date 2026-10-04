//! scene-local E0 논리 payload 원문 대조형 번역 manifest.
//!
//! 물리 record 194개를 raw digest가 같은 논리 payload 100개로 묶되,
//! PUYO1/PUYO2의 실제 message ID와 record offset은 각 행에 함께 보존한다.
//! 보호 원문과 번역문의 control placeholder 순서가 다르면 compiler 전
//! 단계에서 실패한다. 미번역 행은 명시적인 pending 상태와 빈 `ko`만
//! 허용하고, 모든 행이 승인되기 전에는 제품 입력으로 승격하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::analysis::{SubgameSeqSceneMessageAnalysis, SubgameSeqSceneMessageRow};

pub const SUBGAME_SEQ_SCENE_TRANSLATION_MANIFEST_SCHEMA_VERSION: &str =
    "scene-e0-translation-manifest-v1";

const MANIFEST: &str = include_str!("../../assets/translations/scene_e0.tsv");
const HEADER: &str = "schema_version\tstatus\tlogical_id\tsequence\traw_sha256\tcopy_count\tpuyo1_message_id\tpuyo2_message_id\tpuyo1_record_offset\tpuyo2_record_offset\tprotected_source\tko\treference\tnotes";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubgameSeqSceneTranslationStatus {
    PendingHumanTranslation,
    DraftNeedsHumanReview,
    Approved,
}

impl SubgameSeqSceneTranslationStatus {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "pending-human-translation" => Some(Self::PendingHumanTranslation),
            "draft-needs-human-review" => Some(Self::DraftNeedsHumanReview),
            "approved" => Some(Self::Approved),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::PendingHumanTranslation => "pending-human-translation",
            Self::DraftNeedsHumanReview => "draft-needs-human-review",
            Self::Approved => "approved",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneTranslationManifestEntry {
    pub status: SubgameSeqSceneTranslationStatus,
    pub logical_id: String,
    pub sequence: String,
    pub raw_sha256: String,
    pub copy_count: usize,
    pub puyo1_message_id: Option<String>,
    pub puyo2_message_id: Option<String>,
    pub puyo1_record_offset: Option<usize>,
    pub puyo2_record_offset: Option<usize>,
    pub protected_source: String,
    pub ko: String,
    pub reference: String,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneTranslationManifestAnalysis {
    pub entries: Vec<SubgameSeqSceneTranslationManifestEntry>,
    pub sequences: usize,
    pub logical_payloads: usize,
    pub physical_messages: usize,
    pub paired_payloads: usize,
    pub singleton_payloads: usize,
    pub protected_source_code_units: usize,
    pub translated_code_units: usize,
    pub pending_human_translation: usize,
    pub draft_needs_human_review: usize,
    pub approved: usize,
    pub product_eligible: bool,
    pub manifest_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneTranslationManifestError(String);

impl fmt::Display for SubgameSeqSceneTranslationManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqSceneTranslationManifestError {}

fn error(message: impl Into<String>) -> SubgameSeqSceneTranslationManifestError {
    SubgameSeqSceneTranslationManifestError(message.into())
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
) -> Result<usize, SubgameSeqSceneTranslationManifestError> {
    raw.parse::<usize>().map_err(|_| {
        error(format!(
            "scene E0 translation manifest {field} 숫자 아님: line={line}"
        ))
    })
}

fn parse_optional_offset(
    raw: &str,
    field: &str,
    line: usize,
) -> Result<Option<usize>, SubgameSeqSceneTranslationManifestError> {
    if raw.is_empty() {
        return Ok(None);
    }
    raw.strip_prefix("0x")
        .and_then(|digits| usize::from_str_radix(digits, 16).ok())
        .map(Some)
        .ok_or_else(|| {
            error(format!(
                "scene E0 translation manifest {field} hex offset 아님: line={line}"
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

fn is_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'A'..=b'F')
}

fn control_placeholders(text: &str) -> Result<Vec<&str>, SubgameSeqSceneTranslationManifestError> {
    let mut controls = Vec::new();
    let mut rest = text;
    loop {
        let Some(start) = rest.find('<') else {
            if rest.contains('>') {
                return Err(error("scene E0 translation placeholder 닫힘 괄호만 있음"));
            }
            break;
        };
        if rest[..start].contains('>') {
            return Err(error("scene E0 translation placeholder 괄호 순서 불일치"));
        }
        let after_start = &rest[start + 1..];
        let end = after_start
            .find('>')
            .ok_or_else(|| error("scene E0 translation placeholder 종결 누락"))?;
        let body = &after_start[..end];
        let bytes = body.as_bytes();
        let valid = (bytes.len() == 2 && bytes.iter().all(|byte| is_hex(*byte)))
            || (bytes.len() == 4 && bytes.iter().all(|byte| is_hex(*byte)))
            || (bytes.len() == 5
                && bytes[2] == b':'
                && bytes[..2].iter().all(|byte| is_hex(*byte))
                && bytes[3..].iter().all(|byte| is_hex(*byte)));
        if !valid {
            return Err(error(format!(
                "scene E0 translation placeholder 형식 불일치: <{body}>"
            )));
        }
        controls.push(&rest[start..start + end + 2]);
        rest = &after_start[end + 1..];
    }
    if controls.last().copied() != Some("<6565>") {
        return Err(error("scene E0 translation <6565> 종결 placeholder 누락"));
    }
    Ok(controls)
}

fn parse_manifest(
    raw: &str,
) -> Result<Vec<SubgameSeqSceneTranslationManifestEntry>, SubgameSeqSceneTranslationManifestError> {
    let mut lines = raw.lines();
    if lines.next().unwrap_or_default() != HEADER {
        return Err(error("scene E0 translation manifest header 불일치"));
    }
    let mut entries = Vec::new();
    let mut logical_ids = BTreeSet::new();
    let mut raw_digests = BTreeSet::new();
    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        if line.is_empty() {
            return Err(error(format!(
                "scene E0 translation manifest 빈 행: line={line_number}"
            )));
        }
        let columns = line.split('\t').collect::<Vec<_>>();
        let [schema, status, logical_id, sequence, raw_sha256, copy_count, puyo1_message_id, puyo2_message_id, puyo1_record_offset, puyo2_record_offset, protected_source, ko, reference, notes] =
            columns.as_slice()
        else {
            return Err(error(format!(
                "scene E0 translation manifest 열 수 불일치: line={line_number}"
            )));
        };
        if *schema != SUBGAME_SEQ_SCENE_TRANSLATION_MANIFEST_SCHEMA_VERSION {
            return Err(error(format!(
                "scene E0 translation manifest schema 불일치: line={line_number}"
            )));
        }
        let status = SubgameSeqSceneTranslationStatus::parse(status).ok_or_else(|| {
            error(format!(
                "scene E0 translation manifest status 미지원: line={line_number}, value={status}"
            ))
        })?;
        let copy_count = parse_usize(copy_count, "copy_count", line_number)?;
        if logical_id.is_empty()
            || sequence.is_empty()
            || raw_sha256.len() != 64
            || !matches!(copy_count, 1 | 2)
            || protected_source.is_empty()
            || reference.is_empty()
            || notes.is_empty()
        {
            return Err(error(format!(
                "scene E0 translation manifest 필수값 불일치: line={line_number}"
            )));
        }
        if !logical_ids.insert(*logical_id) || !raw_digests.insert(*raw_sha256) {
            return Err(error(format!(
                "scene E0 translation manifest 논리 ID/raw digest 중복: line={line_number}"
            )));
        }
        let source_controls = control_placeholders(protected_source)?;
        match status {
            SubgameSeqSceneTranslationStatus::PendingHumanTranslation if !ko.is_empty() => {
                return Err(error(format!(
                    "scene E0 pending 행에 번역문이 있음: line={line_number}"
                )));
            }
            SubgameSeqSceneTranslationStatus::PendingHumanTranslation => {}
            _ if ko.is_empty() || contains_japanese(ko) => {
                return Err(error(format!(
                    "scene E0 draft/approved 번역문 누락 또는 일본어 잔존: line={line_number}"
                )));
            }
            _ if control_placeholders(ko)? != source_controls => {
                return Err(error(format!(
                    "scene E0 translation control placeholder drift: line={line_number}"
                )));
            }
            _ => {}
        }
        let puyo1_message_id =
            (!puyo1_message_id.is_empty()).then(|| (*puyo1_message_id).to_string());
        let puyo2_message_id =
            (!puyo2_message_id.is_empty()).then(|| (*puyo2_message_id).to_string());
        let puyo1_record_offset =
            parse_optional_offset(puyo1_record_offset, "puyo1_record_offset", line_number)?;
        let puyo2_record_offset =
            parse_optional_offset(puyo2_record_offset, "puyo2_record_offset", line_number)?;
        if puyo1_message_id.is_some() != puyo1_record_offset.is_some()
            || puyo2_message_id.is_some() != puyo2_record_offset.is_some()
            || usize::from(puyo1_message_id.is_some()) + usize::from(puyo2_message_id.is_some())
                != copy_count
        {
            return Err(error(format!(
                "scene E0 translation physical copy 열 불일치: line={line_number}"
            )));
        }
        entries.push(SubgameSeqSceneTranslationManifestEntry {
            status,
            logical_id: (*logical_id).to_string(),
            sequence: (*sequence).to_string(),
            raw_sha256: (*raw_sha256).to_string(),
            copy_count,
            puyo1_message_id,
            puyo2_message_id,
            puyo1_record_offset,
            puyo2_record_offset,
            protected_source: (*protected_source).to_string(),
            ko: (*ko).to_string(),
            reference: (*reference).to_string(),
            notes: (*notes).to_string(),
        });
    }
    if entries.is_empty() {
        return Err(error("scene E0 translation manifest가 비어 있음"));
    }
    Ok(entries)
}

fn physical_id(row: &SubgameSeqSceneMessageRow) -> String {
    format!(
        "{}:{}:0x{:04X}",
        row.scope, row.sequence, row.command_offset
    )
}

struct ExpectedPayload<'a> {
    logical_id: String,
    rows: Vec<&'a SubgameSeqSceneMessageRow>,
}

fn expected_payloads(
    messages: &SubgameSeqSceneMessageAnalysis,
) -> Result<Vec<ExpectedPayload<'_>>, SubgameSeqSceneTranslationManifestError> {
    let mut groups = BTreeMap::<[u8; 32], Vec<&SubgameSeqSceneMessageRow>>::new();
    for row in &messages.rows {
        groups.entry(row.raw_sha256).or_default().push(row);
    }
    let mut ordered = groups
        .into_values()
        .map(|mut rows| {
            rows.sort_by_key(|row| row.scope);
            let sequence = rows[0].sequence;
            if !matches!(rows.len(), 1 | 2)
                || rows.iter().any(|row| {
                    row.sequence != sequence
                        || row.source_text != rows[0].source_text
                        || row.raw != rows[0].raw
                })
                || rows
                    .iter()
                    .map(|row| row.scope)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != rows.len()
            {
                return Err(error("scene E0 logical payload 원본 그룹 계약 불일치"));
            }
            let minimum_index = rows
                .iter()
                .map(|row| row.index)
                .min()
                .ok_or_else(|| error("scene E0 logical payload 빈 그룹"))?;
            Ok((sequence, minimum_index, rows[0].raw_sha256, rows))
        })
        .collect::<Result<Vec<_>, SubgameSeqSceneTranslationManifestError>>()?;
    ordered
        .sort_by_key(|(sequence, minimum_index, digest, _)| (*sequence, *minimum_index, *digest));
    let mut sequence_ordinals = BTreeMap::<&str, usize>::new();
    Ok(ordered
        .into_iter()
        .map(|(sequence, _, _, rows)| {
            let ordinal = sequence_ordinals.entry(sequence).or_default();
            *ordinal += 1;
            ExpectedPayload {
                logical_id: format!("SCENE-E0:{sequence}:L{ordinal:02}"),
                rows,
            }
        })
        .collect())
}

/// 편집 중 manifest를 현재 물리 message 분석과 전수 대조한다.
pub fn audit_subgame_seq_scene_translation_manifest(
    raw: &str,
    messages: &SubgameSeqSceneMessageAnalysis,
) -> Result<SubgameSeqSceneTranslationManifestAnalysis, SubgameSeqSceneTranslationManifestError> {
    let entries = parse_manifest(raw)?;
    let expected = expected_payloads(messages)?;
    if entries.len() != expected.len() {
        return Err(error(format!(
            "scene E0 translation logical payload 분모 불일치: expected={}, actual={}",
            expected.len(),
            entries.len()
        )));
    }
    for (entry, expected) in entries.iter().zip(&expected) {
        let p1 = expected.rows.iter().find(|row| row.scope == "PUYO1");
        let p2 = expected.rows.iter().find(|row| row.scope == "PUYO2");
        if entry.logical_id != expected.logical_id
            || entry.sequence != expected.rows[0].sequence
            || entry.raw_sha256 != digest_array(&expected.rows[0].raw_sha256)
            || entry.copy_count != expected.rows.len()
            || entry.puyo1_message_id.as_deref() != p1.map(|row| physical_id(row)).as_deref()
            || entry.puyo2_message_id.as_deref() != p2.map(|row| physical_id(row)).as_deref()
            || entry.puyo1_record_offset != p1.map(|row| row.record_offset)
            || entry.puyo2_record_offset != p2.map(|row| row.record_offset)
            || entry.protected_source != expected.rows[0].source_text
        {
            return Err(error(format!(
                "scene E0 translation 원본 대조 불일치: {}",
                entry.logical_id
            )));
        }
    }
    let pending_human_translation = entries
        .iter()
        .filter(|entry| entry.status == SubgameSeqSceneTranslationStatus::PendingHumanTranslation)
        .count();
    let draft_needs_human_review = entries
        .iter()
        .filter(|entry| entry.status == SubgameSeqSceneTranslationStatus::DraftNeedsHumanReview)
        .count();
    let approved = entries
        .iter()
        .filter(|entry| entry.status == SubgameSeqSceneTranslationStatus::Approved)
        .count();
    let sequences = entries
        .iter()
        .map(|entry| entry.sequence.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let physical_messages = entries.iter().map(|entry| entry.copy_count).sum();
    let paired_payloads = entries.iter().filter(|entry| entry.copy_count == 2).count();
    let singleton_payloads = entries.len() - paired_payloads;
    Ok(SubgameSeqSceneTranslationManifestAnalysis {
        protected_source_code_units: entries
            .iter()
            .map(|entry| entry.protected_source.chars().count())
            .sum(),
        translated_code_units: entries.iter().map(|entry| entry.ko.chars().count()).sum(),
        product_eligible: approved == entries.len(),
        logical_payloads: entries.len(),
        entries,
        sequences,
        physical_messages,
        paired_payloads,
        singleton_payloads,
        pending_human_translation,
        draft_needs_human_review,
        approved,
        manifest_sha256: digest(raw.as_bytes()),
    })
}

/// 지원 원본 분석과 version 관리 manifest를 전수 대조한다.
pub fn verify_box_subgame_seq_scene_translation_manifest(
    messages: &SubgameSeqSceneMessageAnalysis,
) -> Result<SubgameSeqSceneTranslationManifestAnalysis, SubgameSeqSceneTranslationManifestError> {
    let analysis = audit_subgame_seq_scene_translation_manifest(MANIFEST, messages)?;
    if render_box_subgame_seq_scene_translation_manifest_tsv(&analysis) != MANIFEST {
        return Err(error(
            "scene E0 translation manifest가 canonical 순서/표현이 아님",
        ));
    }
    Ok(analysis)
}

/// 보호 원문과 한국어를 같은 행에 둔 canonical TSV를 만든다.
pub fn render_box_subgame_seq_scene_translation_manifest_tsv(
    analysis: &SubgameSeqSceneTranslationManifestAnalysis,
) -> String {
    let mut output = format!("{HEADER}\n");
    for entry in &analysis.entries {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            SUBGAME_SEQ_SCENE_TRANSLATION_MANIFEST_SCHEMA_VERSION,
            entry.status.label(),
            entry.logical_id,
            entry.sequence,
            entry.raw_sha256,
            entry.copy_count,
            entry.puyo1_message_id.as_deref().unwrap_or_default(),
            entry.puyo2_message_id.as_deref().unwrap_or_default(),
            entry
                .puyo1_record_offset
                .map_or_else(String::new, |offset| format!("0x{offset:04X}")),
            entry
                .puyo2_record_offset
                .map_or_else(String::new, |offset| format!("0x{offset:04X}")),
            entry.protected_source,
            entry.ko,
            entry.reference,
            entry.notes,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

#[cfg(test)]
#[path = "subgame_seq_scene_translation_manifest_tests.rs"]
mod tests;
