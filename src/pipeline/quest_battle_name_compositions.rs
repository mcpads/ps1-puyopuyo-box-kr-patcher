//! QUEST 자연 조우의 59개 전투 이름을 위한 수동 composition 입력.
//!
//! 각 행은 source consumer에서 유도한 물리 slot의 ID·순서·원문·용량·digest에
//! 결속한다. `development_text`는 개발 빌드용 수동 선택이며 자동 음역·축약·
//! 개행을 만들지 않는다. 자연 조우의 주격 조사 역할은 고정하되 완성형 한글
//! 받침만 기계적으로 판정하고, 비한글로 끝나는 표기의 발음 끝음절은 별도
//! 수동 원장만 사용한다. 사람 선택이 남은 행도 세 후보와 임시 개발안을
//! 명시하고 번역·geometry·runtime·제품 승인을 올리지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::analysis::QuestBattleNameTableAnalysis;
use crate::text::select_subject_particle;

use super::quest_translation_encoding::normalize_quest_translation_text;

const COMPOSITIONS: &str =
    include_str!("../../assets/translations/quest_battle_name_compositions.tsv");
const PARTICLE_PRONUNCIATIONS: &str =
    include_str!("../../assets/translations/quest_battle_name_particle_pronunciations.tsv");
const HEADER: &str = "unit_id\tindex\tprotected_source\tprotected_slot_sha256\t\
                     slot_capacity_glyphs\tdevelopment_text\tcandidate_a\tcandidate_b\t\
                     candidate_c\tdecision_status\tstatus\tnotes";
const PARTICLE_PRONUNCIATION_HEADER: &str = "unit_id\tdevelopment_text\tpronunciation_tail\tnotes";

pub const QUEST_BATTLE_NAME_COMPOSITION_STATUS: &str = "manual-draft-needs-runtime-geometry";
pub const QUEST_BATTLE_NAME_SELECTED_DRAFT_STATUS: &str = "draft-needs-independent-review";
pub const QUEST_BATTLE_NAME_HUMAN_CHOICE_STATUS: &str = "draft-needs-human-choice";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestBattleNameDecisionStatus {
    DraftNeedsIndependentReview,
    DraftNeedsHumanChoice,
}

impl QuestBattleNameDecisionStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::DraftNeedsIndependentReview => QUEST_BATTLE_NAME_SELECTED_DRAFT_STATUS,
            Self::DraftNeedsHumanChoice => QUEST_BATTLE_NAME_HUMAN_CHOICE_STATUS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestBattleNameComposition {
    pub unit_id: String,
    pub index: usize,
    pub protected_source: String,
    pub protected_slot_sha256: [u8; 32],
    pub slot_capacity_glyphs: usize,
    pub development_text: String,
    /// 표기의 마지막 문자가 한글이 아닐 때만 수동으로 정한 발음 끝음절.
    pub pronunciation_tail: char,
    /// 자연 조우 문장에서 이름 뒤에 붙일 검증된 주격 조사.
    pub subject_particle: char,
    pub candidates: Vec<String>,
    pub decision_status: QuestBattleNameDecisionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestBattleNameCompositionAnalysis {
    pub compositions: Vec<QuestBattleNameComposition>,
    pub used_glyphs: String,
    pub selected_drafts: usize,
    pub needs_human_choice: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompositionRow {
    unit_id: String,
    index: usize,
    protected_source: String,
    protected_slot_sha256: [u8; 32],
    slot_capacity_glyphs: usize,
    development_text: String,
    candidates: Vec<String>,
    decision_status: QuestBattleNameDecisionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParticlePronunciationRow {
    unit_id: String,
    development_text: String,
    pronunciation_tail: char,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestBattleNameCompositionError(String);

impl fmt::Display for QuestBattleNameCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestBattleNameCompositionError {}

fn error(message: impl Into<String>) -> QuestBattleNameCompositionError {
    QuestBattleNameCompositionError(message.into())
}

fn parse_usize(
    value: &str,
    field: &str,
    line_number: usize,
) -> Result<usize, QuestBattleNameCompositionError> {
    value.parse::<usize>().map_err(|_| {
        error(format!(
            "QUEST battle name composition {line_number}행 {field} 형식 오류: {value:?}"
        ))
    })
}

fn parse_sha256(
    value: &str,
    line_number: usize,
) -> Result<[u8; 32], QuestBattleNameCompositionError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(error(format!(
            "QUEST battle name composition {line_number}행 slot SHA-256 형식 오류"
        )));
    }
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| {
            error(format!(
                "QUEST battle name composition {line_number}행 slot SHA-256 해석 실패"
            ))
        })?;
    }
    Ok(digest)
}

fn parse_decision_status(
    value: &str,
    line_number: usize,
) -> Result<QuestBattleNameDecisionStatus, QuestBattleNameCompositionError> {
    match value {
        QUEST_BATTLE_NAME_SELECTED_DRAFT_STATUS => {
            Ok(QuestBattleNameDecisionStatus::DraftNeedsIndependentReview)
        }
        QUEST_BATTLE_NAME_HUMAN_CHOICE_STATUS => {
            Ok(QuestBattleNameDecisionStatus::DraftNeedsHumanChoice)
        }
        _ => Err(error(format!(
            "QUEST battle name composition {line_number}행 decision status 오류: {value:?}"
        ))),
    }
}

fn contains_japanese(text: &str) -> bool {
    text.chars().any(|glyph| {
        matches!(
            glyph,
            '\u{3040}'..='\u{30ff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{4e00}'..='\u{9fff}'
                | '\u{ff65}'..='\u{ff9f}'
        )
    })
}

fn normalize_candidate(
    value: &str,
    line_number: usize,
    role: &str,
) -> Result<String, QuestBattleNameCompositionError> {
    if value.is_empty() || contains_japanese(value) {
        return Err(error(format!(
            "QUEST battle name composition {line_number}행 {role}가 비었거나 일본어를 포함함"
        )));
    }
    normalize_quest_translation_text(value).map_err(|source| error(source.to_string()))
}

fn parse_rows(input: &str) -> Result<Vec<CompositionRow>, QuestBattleNameCompositionError> {
    let mut lines = input.lines();
    if lines.next() != Some(HEADER) {
        return Err(error("QUEST battle name composition TSV header 불일치"));
    }
    let mut rows = Vec::new();
    for (line_index, line) in lines.enumerate() {
        let line_number = line_index + 2;
        if line.is_empty() {
            return Err(error(format!(
                "QUEST battle name composition {line_number}행이 비었음"
            )));
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        let [unit_id, index, protected_source, slot_sha256, capacity, development_text, candidate_a, candidate_b, candidate_c, decision_status, status, notes] =
            fields.as_slice()
        else {
            return Err(error(format!(
                "QUEST battle name composition {line_number}행 열 수 불일치: {}",
                fields.len()
            )));
        };
        if unit_id.is_empty()
            || protected_source.is_empty()
            || development_text.is_empty()
            || candidate_a.is_empty()
            || notes.is_empty()
            || *status != QUEST_BATTLE_NAME_COMPOSITION_STATUS
        {
            return Err(error(format!(
                "QUEST battle name composition {line_number}행 필수 field/status 불일치"
            )));
        }
        let decision_status = parse_decision_status(decision_status, line_number)?;
        let development_text = normalize_candidate(development_text, line_number, "개발안")?;
        let mut candidates = vec![normalize_candidate(candidate_a, line_number, "후보 A")?];
        for (role, candidate) in [("후보 B", candidate_b), ("후보 C", candidate_c)] {
            if !candidate.is_empty() {
                candidates.push(normalize_candidate(candidate, line_number, role)?);
            }
        }
        if candidates.iter().collect::<BTreeSet<_>>().len() != candidates.len()
            || !candidates.contains(&development_text)
            || (decision_status == QuestBattleNameDecisionStatus::DraftNeedsHumanChoice
                && candidates.len() != 3)
        {
            return Err(error(format!(
                "QUEST battle name composition {line_number}행 후보/개발안 계약 불일치"
            )));
        }
        rows.push(CompositionRow {
            unit_id: (*unit_id).to_owned(),
            index: parse_usize(index, "index", line_number)?,
            protected_source: (*protected_source).to_owned(),
            protected_slot_sha256: parse_sha256(slot_sha256, line_number)?,
            slot_capacity_glyphs: parse_usize(capacity, "slot_capacity_glyphs", line_number)?,
            development_text,
            candidates,
            decision_status,
        });
    }
    if rows.is_empty() {
        return Err(error("QUEST battle name composition TSV가 비었음"));
    }
    Ok(rows)
}

fn parse_particle_pronunciations(
    input: &str,
) -> Result<Vec<ParticlePronunciationRow>, QuestBattleNameCompositionError> {
    let mut lines = input.lines();
    if lines.next() != Some(PARTICLE_PRONUNCIATION_HEADER) {
        return Err(error("QUEST battle name 조사 발음 TSV header 불일치"));
    }
    let mut rows = Vec::new();
    for (line_index, line) in lines.enumerate() {
        let line_number = line_index + 2;
        let fields = line.split('\t').collect::<Vec<_>>();
        let [unit_id, development_text, pronunciation_tail, notes] = fields.as_slice() else {
            return Err(error(format!(
                "QUEST battle name 조사 발음 {line_number}행 열 수 불일치: {}",
                fields.len()
            )));
        };
        if unit_id.is_empty()
            || development_text.is_empty()
            || pronunciation_tail.is_empty()
            || notes.is_empty()
        {
            return Err(error(format!(
                "QUEST battle name 조사 발음 {line_number}행 필수 field 불일치"
            )));
        }
        let development_text =
            normalize_candidate(development_text, line_number, "조사 발음 개발안")?;
        let mut tail_characters = pronunciation_tail.chars();
        let pronunciation_tail = tail_characters.next().ok_or_else(|| {
            error(format!(
                "QUEST battle name 조사 발음 {line_number}행 끝음절이 비었음"
            ))
        })?;
        if tail_characters.next().is_some() || select_subject_particle(pronunciation_tail).is_err()
        {
            return Err(error(format!(
                "QUEST battle name 조사 발음 {line_number}행 끝음절이 현대 한글 한 글자가 아님"
            )));
        }
        rows.push(ParticlePronunciationRow {
            unit_id: (*unit_id).to_owned(),
            development_text,
            pronunciation_tail,
        });
    }
    Ok(rows)
}

fn bind_rows(
    rows: Vec<CompositionRow>,
    particle_pronunciations: Vec<ParticlePronunciationRow>,
    table: &QuestBattleNameTableAnalysis,
) -> Result<QuestBattleNameCompositionAnalysis, QuestBattleNameCompositionError> {
    let mut rows_by_id = BTreeMap::new();
    for row in rows {
        let row_id = row.unit_id.clone();
        if rows_by_id.insert(row_id.clone(), row).is_some() {
            return Err(error(format!(
                "QUEST battle name composition unit ID 중복: {row_id}"
            )));
        }
    }
    let mut particle_pronunciations_by_id = BTreeMap::new();
    for row in particle_pronunciations {
        let row_id = row.unit_id.clone();
        if particle_pronunciations_by_id
            .insert(row_id.clone(), row)
            .is_some()
        {
            return Err(error(format!(
                "QUEST battle name 조사 발음 unit ID 중복: {row_id}"
            )));
        }
    }

    let mut source_selections = BTreeMap::<String, String>::new();
    let mut compositions = Vec::with_capacity(table.entries.len());
    let mut used_glyphs = BTreeSet::new();
    let mut selected_drafts = 0usize;
    let mut needs_human_choice = 0usize;
    for entry in &table.entries {
        let row = rows_by_id
            .remove(entry.id.as_str())
            .ok_or_else(|| error(format!("{}: QUEST battle name composition 누락", entry.id)))?;
        if row.index != entry.index
            || row.protected_source != entry.source
            || row.protected_slot_sha256 != entry.protected_slot_sha256
            || row.slot_capacity_glyphs != entry.slot_capacity_glyphs
        {
            return Err(error(format!(
                "{}: QUEST battle name composition source/index/slot drift",
                entry.id
            )));
        }
        let last_character = row
            .development_text
            .chars()
            .last()
            .ok_or_else(|| error(format!("{}: QUEST battle name 개발안이 비었음", entry.id)))?;
        let pronunciation_tail = if select_subject_particle(last_character).is_ok() {
            if particle_pronunciations_by_id.contains_key(entry.id.as_str()) {
                return Err(error(format!(
                    "{}: 한글로 끝나는 QUEST battle name에 불필요한 조사 발음 예외가 있음",
                    entry.id
                )));
            }
            last_character
        } else {
            let pronunciation = particle_pronunciations_by_id
                .remove(entry.id.as_str())
                .ok_or_else(|| {
                    error(format!(
                        "{}: 한글로 끝나지 않는 QUEST battle name의 조사 발음 예외가 없음",
                        entry.id
                    ))
                })?;
            if pronunciation.development_text != row.development_text {
                return Err(error(format!(
                    "{}: QUEST battle name 조사 발음 개발안 drift",
                    entry.id
                )));
            }
            pronunciation.pronunciation_tail
        };
        let subject_particle = select_subject_particle(pronunciation_tail)
            .map_err(|source| error(format!("{}: {source}", entry.id)))?;
        let glyphs = row
            .development_text
            .chars()
            .count()
            .checked_add(1)
            .ok_or_else(|| error("QUEST battle name 조사 포함 glyph 수 overflow"))?;
        if glyphs > entry.slot_capacity_glyphs {
            return Err(error(format!(
                "{}: QUEST battle name 개발안과 주격 조사 {} glyph가 slot 용량 {}을 넘음",
                entry.id, glyphs, entry.slot_capacity_glyphs
            )));
        }
        if let Some(previous) =
            source_selections.insert(row.protected_source.clone(), row.development_text.clone())
        {
            if previous != row.development_text {
                return Err(error(format!(
                    "{}: 같은 QUEST battle name 원문에 다른 개발안을 선택함",
                    entry.id
                )));
            }
        }
        used_glyphs.extend(row.development_text.chars());
        used_glyphs.insert(subject_particle);
        match row.decision_status {
            QuestBattleNameDecisionStatus::DraftNeedsIndependentReview => selected_drafts += 1,
            QuestBattleNameDecisionStatus::DraftNeedsHumanChoice => needs_human_choice += 1,
        }
        compositions.push(QuestBattleNameComposition {
            unit_id: row.unit_id,
            index: row.index,
            protected_source: row.protected_source,
            protected_slot_sha256: row.protected_slot_sha256,
            slot_capacity_glyphs: row.slot_capacity_glyphs,
            development_text: row.development_text,
            pronunciation_tail,
            subject_particle,
            candidates: row.candidates,
            decision_status: row.decision_status,
        });
    }
    if !rows_by_id.is_empty() || !particle_pronunciations_by_id.is_empty() || used_glyphs.is_empty()
    {
        return Err(error(
            "QUEST battle name composition/조사 발음에 미소비 행이 남거나 glyph 집합이 비었음",
        ));
    }

    Ok(QuestBattleNameCompositionAnalysis {
        compositions,
        used_glyphs: used_glyphs.into_iter().collect(),
        selected_drafts,
        needs_human_choice,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    })
}

/// 내장 수동 composition을 지원 원본의 59개 물리 이름 slot에 결속한다.
pub fn audit_quest_battle_name_compositions(
    table: &QuestBattleNameTableAnalysis,
) -> Result<QuestBattleNameCompositionAnalysis, QuestBattleNameCompositionError> {
    bind_rows(
        parse_rows(COMPOSITIONS)?,
        parse_particle_pronunciations(PARTICLE_PRONUNCIATIONS)?,
        table,
    )
}

#[cfg(test)]
#[path = "quest_battle_name_compositions_tests.rs"]
mod tests;
