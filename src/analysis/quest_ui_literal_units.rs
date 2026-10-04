//! QUEST world-map 지명과 공용 선택지의 source-bound 번역 단위.
//!
//! 원문 문자열뿐 아니라 각 고정 slot의 NUL padding까지 digest에 넣는다.
//! 따라서 번역 자산은 눈에 보이는 문자열과 실제 쓰기 용량을 함께 검토할
//! 수 있고, 주소나 padding이 달라진 원본에 조용히 재사용되지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::disasm::{QuestUiLiteralRole, QuestUiLiteralSummary, QuestUiLiteralSurface};

pub const BOX_QUEST_UI_LITERAL_UNITS: usize = 14 + crate::disasm::QUEST_MENU_LITERAL_COUNT;
pub const BOX_QUEST_WORLD_MAP_LABEL_UNITS: usize = 12;
pub const BOX_QUEST_BINARY_CHOICE_UNITS: usize = 2;

/// 원본 `QUEST.EXE`의 고정 UI literal slot 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestUiLiteralUnit {
    pub id: String,
    pub role: QuestUiLiteralRole,
    pub surface: QuestUiLiteralSurface,
    pub runtime: u32,
    pub end_runtime: u32,
    pub source_text: String,
    pub source_slot: Vec<u8>,
    pub source_sha256: [u8; 32],
}

/// 수동 composition이 전수 결속해야 하는 UI literal 모집단.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestUiLiteralUnitAnalysis {
    pub units: Vec<QuestUiLiteralUnit>,
    pub source_bytes: usize,
    pub world_map_labels: usize,
    pub binary_choices: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestUiLiteralUnitError(String);

impl fmt::Display for QuestUiLiteralUnitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestUiLiteralUnitError {}

fn error(message: impl Into<String>) -> QuestUiLiteralUnitError {
    QuestUiLiteralUnitError(message.into())
}

/// 검증된 literal 요약에서 안정 ID와 slot digest를 만든다.
pub fn audit_quest_ui_literal_units(
    summary: &QuestUiLiteralSummary,
) -> Result<QuestUiLiteralUnitAnalysis, QuestUiLiteralUnitError> {
    let mut roles = BTreeSet::new();
    let mut runtimes = BTreeSet::new();
    let mut units = Vec::with_capacity(summary.literals.len());
    let mut source_bytes = 0usize;

    for literal in &summary.literals {
        let span = literal
            .end_runtime
            .checked_sub(literal.runtime)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                error(format!(
                    "{}: QUEST UI literal slot 범위 역전",
                    literal.role.label()
                ))
            })?;
        if literal.surface != literal.role.surface()
            || literal.source_text.is_empty()
            || literal.source_slot.len() != span
            || literal.source_slot.last() != Some(&0)
            || !roles.insert(literal.role)
            || !runtimes.insert(literal.runtime)
        {
            return Err(error(format!(
                "{}: QUEST UI literal 원문 단위 계약 불일치",
                literal.role.label()
            )));
        }
        source_bytes = source_bytes
            .checked_add(literal.source_slot.len())
            .ok_or_else(|| error("QUEST UI literal 원문 byte 합계 overflow"))?;
        units.push(QuestUiLiteralUnit {
            id: format!(
                "QUEST-UI-LITERAL:{}:{}:0x{:08X}",
                literal.surface.label(),
                literal.role.label(),
                literal.runtime
            ),
            role: literal.role,
            surface: literal.surface,
            runtime: literal.runtime,
            end_runtime: literal.end_runtime,
            source_text: literal.source_text.clone(),
            source_slot: literal.source_slot.clone(),
            source_sha256: Sha256::digest(&literal.source_slot).into(),
        });
    }

    Ok(QuestUiLiteralUnitAnalysis {
        units,
        source_bytes,
        world_map_labels: summary.world_map_labels,
        binary_choices: summary.binary_choices,
    })
}

/// 지원 원본의 12개 지명과 2개 공용 선택지가 정확히 한 번씩 있는지 검증한다.
pub fn verify_box_quest_ui_literal_units(
    summary: &QuestUiLiteralSummary,
) -> Result<QuestUiLiteralUnitAnalysis, QuestUiLiteralUnitError> {
    let analysis = audit_quest_ui_literal_units(summary)?;
    let expected_roles = [
        QuestUiLiteralRole::PlumberVillage,
        QuestUiLiteralRole::SkeletonTCave,
        QuestUiLiteralRole::NeighborTownBridge,
        QuestUiLiteralRole::CherrybellTown,
        QuestUiLiteralRole::HappinessTower,
        QuestUiLiteralRole::HarpyForest,
        QuestUiLiteralRole::MountainPassCave,
        QuestUiLiteralRole::SevenleaseVillage,
        QuestUiLiteralRole::ZohDaimaohRuins,
        QuestUiLiteralRole::PakistaShop,
        QuestUiLiteralRole::PararaShop,
        QuestUiLiteralRole::SasorimanShop,
        QuestUiLiteralRole::ChoiceYes,
        QuestUiLiteralRole::ChoiceNo,
    ]
    .into_iter()
    .chain(
        crate::disasm::QUEST_MENU_LITERAL_SPECS
            .iter()
            .map(|s| QuestUiLiteralRole::MenuText(s.0)),
    )
    .collect::<BTreeSet<_>>();
    let actual_roles = analysis
        .units
        .iter()
        .map(|unit| unit.role)
        .collect::<BTreeSet<_>>();
    if analysis.units.len() != BOX_QUEST_UI_LITERAL_UNITS
        || analysis.source_bytes != summary.source_bytes
        || analysis.world_map_labels != BOX_QUEST_WORLD_MAP_LABEL_UNITS
        || analysis.binary_choices != BOX_QUEST_BINARY_CHOICE_UNITS
        || actual_roles != expected_roles
    {
        return Err(error(format!(
            "QUEST UI literal 원문 단위 분모 불일치: expected={}, actual={}",
            BOX_QUEST_UI_LITERAL_UNITS,
            analysis.units.len()
        )));
    }
    Ok(analysis)
}

#[cfg(test)]
#[path = "quest_ui_literal_units_tests.rs"]
mod tests;
