//! 정적 SEQ path의 `OPAA` record를 아이템 획득 consumer에 결속한다.
//!
//! `disasm::quest_item_acquisition`이 검증한 record layout과 handler 의미를
//! 이미 계산된 SEQ path에 적용한다. 이 모듈은 path를 다시 걷거나 실행
//! 도달을 주장하지 않고, source record와 consumer 사이의 정적 결속만 소유한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::disasm::QuestItemAcquisitionSummary;

use super::quest_seq_text_paths::{QuestSeqTextPathAnalysis, QuestSeqTextPathTransition};

pub const BOX_QUEST_ITEM_ACQUISITION_SEQUENCE: &str = "SHEZ.SEQ";
pub const BOX_QUEST_ITEM_ACQUISITION_SEQUENCE_SHA256: [u8; 32] = [
    0x2C, 0x87, 0x85, 0x82, 0x5B, 0x4D, 0xA1, 0xEB, 0xB7, 0x65, 0x5B, 0xA2, 0x23, 0xFA, 0x25, 0x8E,
    0xDC, 0x97, 0x8A, 0x3F, 0x8A, 0xE8, 0x48, 0x3F, 0x1E, 0xED, 0xFF, 0xB9, 0x03, 0x8C, 0xBF, 0x0E,
];
pub const BOX_QUEST_ITEM_ACQUISITION_SEED_OFFSET: usize = 0x1944;
pub const BOX_QUEST_ITEM_ACQUISITION_RECORD_OFFSET: usize = 0x194C;
pub const BOX_QUEST_ITEM_ACQUISITION_RECORD_RAW: [u8; 8] =
    [0xAA, 0x05, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00];
pub const BOX_QUEST_ITEM_ACQUISITION_SUBMODE: u8 = 5;
pub const BOX_QUEST_ITEM_ACQUISITION_ITEM_ID: u8 = 5;
pub const BOX_QUEST_ITEM_ACQUISITION_PARAMETER: u16 = 0;
pub const BOX_QUEST_ITEM_ACQUISITION_PATH_OCCURRENCES: usize = 1;

/// 한 정적 seed path에서 확인한 `OPAA` record occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemAcquisitionRecordOccurrence {
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub seed_offset: usize,
    pub record_offset: usize,
    pub raw: Vec<u8>,
    pub submode: u8,
    pub item_id: u8,
    pub parameter: u16,
    pub uses_whole_string_name: bool,
}

/// 이미 계산된 SEQ path에 나타난 아이템 획득 record 원장.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemAcquisitionRecordAnalysis {
    pub occurrences: Vec<QuestItemAcquisitionRecordOccurrence>,
}

/// `OPAA` source record 결속 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemAcquisitionRecordError(String);

impl fmt::Display for QuestItemAcquisitionRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemAcquisitionRecordError {}

fn error(message: impl Into<String>) -> QuestItemAcquisitionRecordError {
    QuestItemAcquisitionRecordError(message.into())
}

fn validate_record_layout(
    route: &QuestItemAcquisitionSummary,
) -> Result<(), QuestItemAcquisitionRecordError> {
    if route.record_bytes == 0
        || route.record_submode_offset >= route.record_bytes
        || route.record_item_id_offset >= route.record_bytes
        || route
            .record_parameter_offset
            .checked_add(2)
            .is_none_or(|end| end > route.record_bytes)
    {
        return Err(error(format!(
            "QUEST item acquisition record layout 범위 불일치: bytes={}, submode=+{}, item=+{}, parameter=+{}",
            route.record_bytes,
            route.record_submode_offset,
            route.record_item_id_offset,
            route.record_parameter_offset,
        )));
    }
    Ok(())
}

fn collect_sequences<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Result<BTreeMap<&'a str, &'a [u8]>, QuestItemAcquisitionRecordError> {
    let mut input = BTreeMap::new();
    for (name, raw) in sequences {
        if name.is_empty() || raw.is_empty() {
            return Err(error(
                "QUEST item acquisition sequence 이름이나 원문이 비어 있음",
            ));
        }
        if input.insert(name, raw).is_some() {
            return Err(error(format!(
                "QUEST item acquisition sequence 입력 중복: {name}"
            )));
        }
    }
    if input.is_empty() {
        return Err(error("QUEST item acquisition sequence 입력이 없음"));
    }
    Ok(input)
}

/// 기존 정적 path에서 `OPAA` record occurrence를 추출해 consumer 의미에 결속한다.
pub fn audit_quest_item_acquisition_records<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    paths: &QuestSeqTextPathAnalysis,
    route: &QuestItemAcquisitionSummary,
) -> Result<QuestItemAcquisitionRecordAnalysis, QuestItemAcquisitionRecordError> {
    validate_record_layout(route)?;
    let input = collect_sequences(sequences)?;
    let mut seen = BTreeSet::new();
    let mut occurrences = Vec::new();

    for path in &paths.paths {
        let raw = input.get(path.sequence.as_str()).ok_or_else(|| {
            error(format!(
                "QUEST item acquisition path의 sequence 원문이 없음: {}",
                path.sequence
            ))
        })?;
        let source_sha256: [u8; 32] = Sha256::digest(raw).into();
        if source_sha256 != path.sequence_sha256 {
            return Err(error(format!(
                "QUEST item acquisition path/source digest 불일치: {} seed=0x{:04X}",
                path.sequence, path.seed_offset
            )));
        }

        for node in path.nodes.iter().filter(|node| node.opcode == route.opcode) {
            if node.transition != QuestSeqTextPathTransition::DirectAdvance
                || node.direct_advance_bytes != Some(route.record_bytes)
                || node.next_offset != node.offset.checked_add(route.record_bytes)
            {
                return Err(error(format!(
                    "QUEST item acquisition path advance 불일치: {} seed=0x{:04X} record=0x{:04X}",
                    path.sequence, path.seed_offset, node.offset
                )));
            }
            let record = raw
                .get(node.offset..node.offset + route.record_bytes)
                .ok_or_else(|| {
                    error(format!(
                        "QUEST item acquisition record 범위 이탈: {} 0x{:04X}",
                        path.sequence, node.offset
                    ))
                })?;
            if record[0] != route.opcode {
                return Err(error(format!(
                    "QUEST item acquisition record opcode 불일치: {} 0x{:04X}",
                    path.sequence, node.offset
                )));
            }
            let identity = (path.sequence.as_str(), path.seed_offset, node.offset);
            if !seen.insert(identity) {
                return Err(error(format!(
                    "QUEST item acquisition path occurrence 중복: {} seed=0x{:04X} record=0x{:04X}",
                    path.sequence, path.seed_offset, node.offset
                )));
            }
            let submode = record[route.record_submode_offset];
            let item_id = record[route.record_item_id_offset];
            let parameter = u16::from_le_bytes([
                record[route.record_parameter_offset],
                record[route.record_parameter_offset + 1],
            ]);
            occurrences.push(QuestItemAcquisitionRecordOccurrence {
                sequence: path.sequence.clone(),
                sequence_sha256: source_sha256,
                seed_offset: path.seed_offset,
                record_offset: node.offset,
                raw: record.to_vec(),
                submode,
                item_id,
                parameter,
                uses_whole_string_name: route.whole_string_submodes.contains(&submode),
            });
        }
    }

    Ok(QuestItemAcquisitionRecordAnalysis { occurrences })
}

/// 지원 원본에서 `SHEZ.SEQ`의 단일 획득 record와 whole-string 경로를 검증한다.
pub fn verify_box_quest_item_acquisition_records<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    paths: &QuestSeqTextPathAnalysis,
    route: &QuestItemAcquisitionSummary,
) -> Result<QuestItemAcquisitionRecordAnalysis, QuestItemAcquisitionRecordError> {
    let analysis = audit_quest_item_acquisition_records(sequences, paths, route)?;
    if analysis.occurrences.len() != BOX_QUEST_ITEM_ACQUISITION_PATH_OCCURRENCES {
        return Err(error(format!(
            "QUEST item acquisition path occurrence 수 불일치: expected={}, actual={}",
            BOX_QUEST_ITEM_ACQUISITION_PATH_OCCURRENCES,
            analysis.occurrences.len()
        )));
    }
    let occurrence = &analysis.occurrences[0];
    if occurrence.sequence != BOX_QUEST_ITEM_ACQUISITION_SEQUENCE
        || occurrence.sequence_sha256 != BOX_QUEST_ITEM_ACQUISITION_SEQUENCE_SHA256
        || occurrence.seed_offset != BOX_QUEST_ITEM_ACQUISITION_SEED_OFFSET
        || occurrence.record_offset != BOX_QUEST_ITEM_ACQUISITION_RECORD_OFFSET
        || occurrence.raw.as_slice() != BOX_QUEST_ITEM_ACQUISITION_RECORD_RAW
        || occurrence.submode != BOX_QUEST_ITEM_ACQUISITION_SUBMODE
        || occurrence.item_id != BOX_QUEST_ITEM_ACQUISITION_ITEM_ID
        || occurrence.parameter != BOX_QUEST_ITEM_ACQUISITION_PARAMETER
        || !occurrence.uses_whole_string_name
    {
        return Err(error(format!(
            "QUEST item acquisition 지원 원본 record 불일치: {occurrence:?}"
        )));
    }
    Ok(analysis)
}

#[cfg(test)]
#[path = "quest_item_acquisition_records_tests.rs"]
mod tests;
