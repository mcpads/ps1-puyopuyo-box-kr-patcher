//! `OPAA` 아이템 획득 상태기가 직접 그리는 고정 문자열 단위.
//!
//! 원문 주소·NUL span·역할은 `disasm::quest_item_acquisition`의 검증 결과만
//! 소비한다. 이 모듈은 번역문이나 page 배치를 정하지 않고, 번역 자산이
//! 결속할 안정 ID와 원문 digest를 만든다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::disasm::{QuestItemAcquisitionMessageRole, QuestItemAcquisitionSummary};

pub const BOX_QUEST_ITEM_ACQUISITION_TEXT_UNITS: usize = 7;

/// 원본 `QUEST.EXE`의 고정 NUL 문자열 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemAcquisitionTextUnit {
    pub id: String,
    pub role: QuestItemAcquisitionMessageRole,
    pub runtime: u32,
    pub source_text: String,
    pub source: Vec<u8>,
    pub source_sha256: [u8; 32],
}

/// 번역 자산이 반드시 전수 결속해야 하는 고정 문자열 모집단.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemAcquisitionTextUnitAnalysis {
    pub units: Vec<QuestItemAcquisitionTextUnit>,
    pub source_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemAcquisitionTextUnitError(String);

impl fmt::Display for QuestItemAcquisitionTextUnitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemAcquisitionTextUnitError {}

fn error(message: impl Into<String>) -> QuestItemAcquisitionTextUnitError {
    QuestItemAcquisitionTextUnitError(message.into())
}

/// 검증된 상태기 요약에서 source-bound 번역 단위를 만든다.
pub fn audit_quest_item_acquisition_text_units(
    summary: &QuestItemAcquisitionSummary,
) -> Result<QuestItemAcquisitionTextUnitAnalysis, QuestItemAcquisitionTextUnitError> {
    let mut roles = BTreeSet::new();
    let mut runtimes = BTreeSet::new();
    let mut units = Vec::with_capacity(summary.fixed_messages.len());
    let mut source_bytes = 0usize;

    for message in &summary.fixed_messages {
        let source = message.source_bytes_with_nul;
        if !roles.insert(message.role)
            || !runtimes.insert(message.runtime)
            || message.source_text.is_empty()
            || source.len() < 3
            || source.last() != Some(&0)
            || source[..source.len() - 1].contains(&0)
        {
            return Err(error(format!(
                "QUEST item acquisition 고정 문자열 원문 계약 불일치: {} 0x{:08X}",
                message.role.label(),
                message.runtime
            )));
        }
        source_bytes = source_bytes
            .checked_add(source.len())
            .ok_or_else(|| error("QUEST item acquisition 고정 문자열 byte 합계 overflow"))?;
        units.push(QuestItemAcquisitionTextUnit {
            id: format!(
                "QUEST-ITEM-ACQUISITION:{}:0x{:08X}",
                message.role.label(),
                message.runtime
            ),
            role: message.role,
            runtime: message.runtime,
            source_text: message.source_text.to_owned(),
            source: source.to_vec(),
            source_sha256: Sha256::digest(source).into(),
        });
    }

    if units.is_empty()
        || summary.fixed_message_call_runtimes.is_empty()
        || summary
            .fixed_message_call_runtimes
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != summary.fixed_message_call_runtimes.len()
    {
        return Err(error(
            "QUEST item acquisition 고정 문자열 또는 draw call 분모가 비었거나 중복됨",
        ));
    }

    Ok(QuestItemAcquisitionTextUnitAnalysis {
        units,
        source_bytes,
    })
}

/// 지원 원본의 일곱 역할이 정확히 한 번씩 있는지 검증한다.
pub fn verify_box_quest_item_acquisition_text_units(
    summary: &QuestItemAcquisitionSummary,
) -> Result<QuestItemAcquisitionTextUnitAnalysis, QuestItemAcquisitionTextUnitError> {
    let analysis = audit_quest_item_acquisition_text_units(summary)?;
    let expected_roles = [
        QuestItemAcquisitionMessageRole::CurrencyAcquiredSuffix,
        QuestItemAcquisitionMessageRole::ItemAcquiredSuffix,
        QuestItemAcquisitionMessageRole::InventoryFullNotice,
        QuestItemAcquisitionMessageRole::DiscardPrompt,
        QuestItemAcquisitionMessageRole::AbandonSubject,
        QuestItemAcquisitionMessageRole::AbandonParticle,
        QuestItemAcquisitionMessageRole::AbandonPredicate,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let actual_roles = analysis
        .units
        .iter()
        .map(|unit| unit.role)
        .collect::<BTreeSet<_>>();
    if analysis.units.len() != BOX_QUEST_ITEM_ACQUISITION_TEXT_UNITS
        || actual_roles != expected_roles
    {
        return Err(error(format!(
            "QUEST item acquisition 고정 문자열 역할 분모 불일치: expected={}, actual={}",
            BOX_QUEST_ITEM_ACQUISITION_TEXT_UNITS,
            analysis.units.len()
        )));
    }
    Ok(analysis)
}

#[cfg(test)]
#[path = "quest_item_acquisition_text_units_tests.rs"]
mod tests;
