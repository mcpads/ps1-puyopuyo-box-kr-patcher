//! QUEST item 이름·설명을 ITEM_DAT 전용 적재 범위 안에 재배치한다.
//!
//! 이름은 NUL 종단 원문을 그대로 옮기고, 설명은 canonical OP21 record를
//! 앞에 붙인 뒤 원문 stream을 옮긴다. 설명 constructor의 원본 active=1
//! store는 새 interpreter를 scheduler에 남기므로 검산만 하고 바꾸지 않는다.
//! page 0 control은 원본 glyph route를 유지하며 번역이나 동적 page 공급을
//! 승인하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::disasm::{
    verify_box_quest_item_text_consumer, QuestItemTextConsumerError,
    QUEST_ITEM_DESCRIPTION_ACTIVE_STORE_RUNTIME,
};
use crate::isa::{
    verify_bytes, InstructionKind as K, IsaProfile, Reg, VerifiedMachineCode, VerifyError,
};
use crate::script::{
    QuestItemTextRole, QuestItemTextUnitAnalysis, QUEST_SEQ_TEXT_MODE_OPCODE,
    QUEST_SEQ_TEXT_MODE_RECORD_BYTES,
};

// 초기화 record 0x80089380/0x80089398은 ITEM_DAT와 MCRD1을 각각
// 0x801A1A70/0x801A7A70에 적재한다. 일반 SEQ active window와 다르다.
pub const QUEST_ITEM_TEXT_WINDOW_BYTES: usize = 0x6000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemTextRelocatedUnit {
    pub id: String,
    pub role: QuestItemTextRole,
    pub pointer_field_offset: usize,
    pub original_text_offset: usize,
    pub relocated_record_offset: Option<usize>,
    pub relocated_text_offset: usize,
    pub relocated_pointer: u32,
    pub source_bytes: usize,
    pub protected_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemTextRelinkControl {
    pub output: Vec<u8>,
    pub source_bytes: usize,
    pub output_bytes: usize,
    pub appended_bytes: usize,
    pub active_window_remaining: usize,
    pub pointer_writes: usize,
    pub name_units: usize,
    pub description_units: usize,
    pub description_op21_records: usize,
    pub source_exact_outside_relinked_fields: bool,
    pub relocated_units: Vec<QuestItemTextRelocatedUnit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemTextRelinkPlan {
    pub control: QuestItemTextRelinkControl,
    pub verified_active_store: VerifiedMachineCode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct QuestItemDescriptionRelinkReplacement {
    pub page_id: u8,
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub enum QuestItemTextRelinkError {
    Message(String),
    Consumer(QuestItemTextConsumerError),
    MachineCode(VerifyError),
}

impl fmt::Display for QuestItemTextRelinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => formatter.write_str(message),
            Self::Consumer(source) => write!(formatter, "QUEST item consumer 검산 실패: {source}"),
            Self::MachineCode(source) => {
                write!(formatter, "QUEST item constructor ISA 검산 실패: {source}")
            }
        }
    }
}

impl Error for QuestItemTextRelinkError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Consumer(source) => Some(source),
            Self::MachineCode(source) => Some(source),
            Self::Message(_) => None,
        }
    }
}

impl From<QuestItemTextConsumerError> for QuestItemTextRelinkError {
    fn from(value: QuestItemTextConsumerError) -> Self {
        Self::Consumer(value)
    }
}

impl From<VerifyError> for QuestItemTextRelinkError {
    fn from(value: VerifyError) -> Self {
        Self::MachineCode(value)
    }
}

fn error(message: impl Into<String>) -> QuestItemTextRelinkError {
    QuestItemTextRelinkError::Message(message.into())
}

fn runtime_base(analysis: &QuestItemTextUnitAnalysis) -> Result<u32, QuestItemTextRelinkError> {
    let first = analysis
        .units
        .first()
        .ok_or_else(|| error("QUEST item relink unit이 비어 있음"))?;
    let base = first
        .record_runtime
        .checked_sub(
            u32::try_from(first.record_offset)
                .map_err(|_| error("QUEST item record offset가 u32를 넘음"))?,
        )
        .ok_or_else(|| error("QUEST item runtime base 계산 실패"))?;
    for unit in &analysis.units {
        let record_offset = u32::try_from(unit.record_offset)
            .map_err(|_| error("QUEST item record offset가 u32를 넘음"))?;
        let text_offset = u32::try_from(unit.text_offset)
            .map_err(|_| error("QUEST item text offset가 u32를 넘음"))?;
        if base.checked_add(record_offset) != Some(unit.record_runtime)
            || base.checked_add(text_offset) != Some(unit.text_runtime)
        {
            return Err(error(format!(
                "QUEST item unit 좌표계가 한 runtime base에 결속되지 않음: {}",
                unit.id
            )));
        }
    }
    Ok(base)
}

fn align_output(output: &mut Vec<u8>) {
    while !output.len().is_multiple_of(4) {
        output.push(0);
    }
}

fn plan_quest_item_text_relink_control_with_replacements(
    raw: &[u8],
    analysis: &QuestItemTextUnitAnalysis,
    name_replacements: &BTreeMap<String, Vec<u8>>,
    description_replacements: &BTreeMap<String, QuestItemDescriptionRelinkReplacement>,
) -> Result<QuestItemTextRelinkControl, QuestItemTextRelinkError> {
    if raw.is_empty() || raw.len() > QUEST_ITEM_TEXT_WINDOW_BYTES || analysis.units.is_empty() {
        return Err(error("QUEST item relink 입력이 비어 있음"));
    }
    let base = runtime_base(analysis)?;
    let mut units = analysis.units.iter().collect::<Vec<_>>();
    units.sort_by_key(|unit| (unit.record_offset, unit.role));
    let mut record_roles = BTreeMap::<usize, (usize, usize)>::new();
    let mut unit_ids = BTreeSet::new();
    for unit in &units {
        if !unit_ids.insert(unit.id.as_str()) {
            return Err(error(format!(
                "QUEST item relink unit ID가 중복됨: {}",
                unit.id
            )));
        }
        let counts = record_roles.entry(unit.record_offset).or_default();
        match unit.role {
            QuestItemTextRole::Name => counts.0 += 1,
            QuestItemTextRole::Description => counts.1 += 1,
        }
    }
    if record_roles.values().any(|counts| *counts != (1, 1)) {
        return Err(error(format!(
            "QUEST item relink record별 이름·설명 분모 불일치: {record_roles:?}"
        )));
    }
    let source_name_ids = units
        .iter()
        .filter(|unit| unit.role == QuestItemTextRole::Name)
        .map(|unit| unit.id.as_str())
        .collect::<BTreeSet<_>>();
    let replacement_ids = name_replacements
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if !name_replacements.is_empty() && replacement_ids != source_name_ids {
        return Err(error(
            "QUEST item name replacement가 이름 unit 전수와 일치하지 않음",
        ));
    }
    let source_description_ids = units
        .iter()
        .filter(|unit| unit.role == QuestItemTextRole::Description)
        .map(|unit| unit.id.as_str())
        .collect::<BTreeSet<_>>();
    let description_replacement_ids = description_replacements
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if !description_replacements.is_empty() && description_replacement_ids != source_description_ids
    {
        return Err(error(
            "QUEST item description replacement가 설명 unit 전수와 일치하지 않음",
        ));
    }
    for (id, replacement) in name_replacements {
        if replacement.len() < 2
            || !replacement.len().is_multiple_of(2)
            || !replacement.ends_with(&[0, 0])
            || replacement[..replacement.len() - 2]
                .chunks_exact(2)
                .any(|pair| pair == [0, 0])
        {
            return Err(error(format!(
                "QUEST item name replacement NUL 구조 불일치: {id}"
            )));
        }
    }
    for (id, replacement) in description_replacements {
        if replacement.page_id == 0
            || replacement.body.is_empty()
            || !replacement.body.ends_with(&[0x1A, 0xFF, 0, 0, 0, 0])
        {
            return Err(error(format!(
                "QUEST item description replacement page/종단 불일치: {id}"
            )));
        }
    }
    // 원본 text 범위만 재사용한다. table·record header·미해석 padding은 보호한다.
    // 모든 원문 검증과 배치를 끝낸 뒤 하나의 output에 합성한다.
    let mut pointer_fields = BTreeSet::new();
    let mut reusable = BTreeSet::new();
    let table_bytes = analysis
        .table_entries
        .checked_mul(4)
        .ok_or_else(|| error("QUEST item table 크기 overflow"))?;
    let mut jobs = Vec::new();
    for unit in &units {
        let field_end = unit
            .pointer_field_offset
            .checked_add(4)
            .ok_or_else(|| error("QUEST item pointer field 끝 overflow"))?;
        if raw.get(unit.pointer_field_offset..field_end)
            != Some(unit.text_runtime.to_le_bytes().as_slice())
            || raw.get(unit.text_offset..unit.end_offset) != Some(unit.protected_raw.as_slice())
        {
            return Err(error(format!(
                "QUEST item pointer/text 원본 불일치: {}",
                unit.id
            )));
        }
        for offset in unit.pointer_field_offset..field_end {
            if !pointer_fields.insert(offset) {
                return Err(error("QUEST item pointer write가 겹침"));
            }
        }
        for offset in unit.text_offset..unit.end_offset {
            if offset < table_bytes
                || record_roles
                    .keys()
                    .any(|start| (*start..start + 20).contains(&offset))
                || !reusable.insert(offset)
            {
                return Err(error(format!(
                    "QUEST item 재사용 text가 보호 영역이나 다른 text와 겹침: {}",
                    unit.id
                )));
            }
        }
        let body = match unit.role {
            QuestItemTextRole::Name => name_replacements
                .get(&unit.id)
                .map_or(unit.protected_raw.as_slice(), Vec::as_slice),
            QuestItemTextRole::Description => description_replacements
                .get(&unit.id)
                .map_or(unit.protected_raw.as_slice(), |replacement| {
                    replacement.body.as_slice()
                }),
        };
        let mut payload = Vec::new();
        if unit.role == QuestItemTextRole::Description {
            let page_id = description_replacements
                .get(&unit.id)
                .map_or(0, |replacement| replacement.page_id);
            payload.extend_from_slice(&[QUEST_SEQ_TEXT_MODE_OPCODE, page_id, 0, 0]);
        }
        payload.extend_from_slice(body);
        jobs.push((*unit, payload, body.len()));
    }
    let mut free: Vec<std::ops::Range<usize>> = Vec::new();
    for offset in &reusable {
        if let Some(last) = free.last_mut().filter(|last| last.end == *offset) {
            last.end += 1;
        } else {
            free.push(*offset..offset + 1);
        }
    }
    // 큰 stream부터 가장 작은 충분한 원문 자리에 넣어 단편화를 줄인다.
    jobs.sort_by_key(|(unit, payload, _)| (std::cmp::Reverse(payload.len()), unit.id.as_str()));
    let mut output = raw.to_vec();
    let mut relocated_units = Vec::with_capacity(units.len());
    let mut written = pointer_fields.clone();
    for (unit, payload, body_len) in jobs {
        let fit = free
            .iter()
            .enumerate()
            .filter_map(|(index, range)| {
                let start = range.start.next_multiple_of(4);
                (start + payload.len() <= range.end).then(|| (range.end - start, index, start))
            })
            .min();
        let start = if let Some((_, index, start)) = fit {
            let range = free.remove(index);
            if range.start < start {
                free.push(range.start..start);
            }
            if start + payload.len() < range.end {
                free.push(start + payload.len()..range.end);
            }
            start
        } else {
            align_output(&mut output);
            let start = output.len();
            if payload.len() > QUEST_ITEM_TEXT_WINDOW_BYTES - start {
                return Err(error("QUEST ITEM_DAT relink가 MCRD1 적재 영역을 침범함"));
            }
            output.resize(start + payload.len(), 0);
            start
        };
        for offset in start..start + payload.len() {
            if !written.insert(offset) {
                return Err(error("QUEST item 최종 text/pointer write가 겹침"));
            }
        }
        output[start..start + payload.len()].copy_from_slice(&payload);
        let relocated_record_offset =
            (unit.role == QuestItemTextRole::Description).then_some(start);
        let relocated_text_offset = start + payload.len() - body_len;
        let field_end = unit.pointer_field_offset + 4;
        // 설명 constructor는 active=1 direct text mode로 들어간다. 따라서 pointer는
        // 본문을 가리키고, 바로 앞 canonical OP21 record는 소비 직전 page metadata다.
        let pointer_offset = relocated_text_offset;
        let pointer_offset_u32 = u32::try_from(pointer_offset)
            .map_err(|_| error("QUEST item relocated pointer offset가 u32를 넘음"))?;
        let relocated_pointer = base
            .checked_add(pointer_offset_u32)
            .ok_or_else(|| error("QUEST item relocated pointer runtime overflow"))?;
        output[unit.pointer_field_offset..field_end]
            .copy_from_slice(&relocated_pointer.to_le_bytes());
        relocated_units.push(QuestItemTextRelocatedUnit {
            id: unit.id.clone(),
            role: unit.role,
            pointer_field_offset: unit.pointer_field_offset,
            original_text_offset: unit.text_offset,
            relocated_record_offset,
            relocated_text_offset,
            relocated_pointer,
            source_bytes: unit.protected_raw.len(),
            protected_bytes: body_len,
        });
    }

    if output.len() > QUEST_ITEM_TEXT_WINDOW_BYTES {
        return Err(error(format!(
            "QUEST ITEM_DAT relink가 active window를 넘음: {} > {}",
            output.len(),
            QUEST_ITEM_TEXT_WINDOW_BYTES
        )));
    }
    let source_exact_outside_relinked_fields = raw
        .iter()
        .zip(&output[..raw.len()])
        .enumerate()
        .all(|(offset, (before, after))| written.contains(&offset) || before == after);
    if !source_exact_outside_relinked_fields {
        return Err(error("QUEST ITEM_DAT relink write 밖 원본이 바뀜"));
    }
    for unit in &relocated_units {
        if let Some(record_offset) = unit.relocated_record_offset {
            let record_end = record_offset + QUEST_SEQ_TEXT_MODE_RECORD_BYTES;
            let page_id = description_replacements
                .get(&unit.id)
                .map_or(0, |replacement| replacement.page_id);
            if output.get(record_offset..record_end)
                != Some([QUEST_SEQ_TEXT_MODE_OPCODE, page_id, 0, 0].as_slice())
            {
                return Err(error(format!(
                    "QUEST item description OP21 record readback 불일치: {}",
                    unit.id
                )));
            }
        }
        let source = analysis
            .units
            .iter()
            .find(|source| source.id == unit.id)
            .expect("relocated unit은 입력 unit에서 생성함");
        let relocated_end = unit.relocated_text_offset + unit.protected_bytes;
        let expected = match unit.role {
            QuestItemTextRole::Name => name_replacements
                .get(&unit.id)
                .map_or(source.protected_raw.as_slice(), Vec::as_slice),
            QuestItemTextRole::Description => description_replacements
                .get(&unit.id)
                .map_or(source.protected_raw.as_slice(), |replacement| {
                    replacement.body.as_slice()
                }),
        };
        if output.get(unit.relocated_text_offset..relocated_end) != Some(expected) {
            return Err(error(format!(
                "QUEST item relocated text readback 불일치: {}",
                unit.id
            )));
        }
    }

    relocated_units.sort_by_key(|unit| unit.pointer_field_offset);
    Ok(QuestItemTextRelinkControl {
        source_bytes: raw.len(),
        output_bytes: output.len(),
        appended_bytes: output.len() - raw.len(),
        active_window_remaining: QUEST_ITEM_TEXT_WINDOW_BYTES - output.len(),
        pointer_writes: relocated_units.len(),
        name_units: relocated_units
            .iter()
            .filter(|unit| unit.role == QuestItemTextRole::Name)
            .count(),
        description_units: relocated_units
            .iter()
            .filter(|unit| unit.role == QuestItemTextRole::Description)
            .count(),
        description_op21_records: relocated_units
            .iter()
            .filter(|unit| unit.relocated_record_offset.is_some())
            .count(),
        source_exact_outside_relinked_fields,
        relocated_units,
        output,
    })
}

/// 원문 내용을 유지하며 이름과 OP21-prefixed 설명을 전용 적재 범위 안에 배치한다.
pub fn plan_quest_item_text_relink_control(
    raw: &[u8],
    analysis: &QuestItemTextUnitAnalysis,
) -> Result<QuestItemTextRelinkControl, QuestItemTextRelinkError> {
    plan_quest_item_text_relink_control_with_replacements(
        raw,
        analysis,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
}

#[cfg(test)]
pub(super) fn plan_quest_item_text_relink_control_with_names(
    raw: &[u8],
    analysis: &QuestItemTextUnitAnalysis,
    name_replacements: &BTreeMap<String, Vec<u8>>,
) -> Result<QuestItemTextRelinkControl, QuestItemTextRelinkError> {
    plan_quest_item_text_relink_control_with_replacements(
        raw,
        analysis,
        name_replacements,
        &BTreeMap::new(),
    )
}

/// 지원 QUEST.EXE 소비 경계와 page-0 ITEM_DAT 구조 control을 함께 검증한다.
pub fn verify_box_quest_item_text_relink_plan(
    map: &ExeMap,
    text: &[u8],
    raw: &[u8],
    analysis: &QuestItemTextUnitAnalysis,
) -> Result<QuestItemTextRelinkPlan, QuestItemTextRelinkError> {
    verify_box_quest_item_text_relink_plan_with_replacements(
        map,
        text,
        raw,
        analysis,
        &BTreeMap::new(),
        &BTreeMap::new(),
    )
}

pub(super) fn verify_box_quest_item_text_relink_plan_with_names(
    map: &ExeMap,
    text: &[u8],
    raw: &[u8],
    analysis: &QuestItemTextUnitAnalysis,
    name_replacements: &BTreeMap<String, Vec<u8>>,
) -> Result<QuestItemTextRelinkPlan, QuestItemTextRelinkError> {
    verify_box_quest_item_text_relink_plan_with_replacements(
        map,
        text,
        raw,
        analysis,
        name_replacements,
        &BTreeMap::new(),
    )
}

pub(super) fn verify_box_quest_item_text_relink_plan_with_replacements(
    map: &ExeMap,
    text: &[u8],
    raw: &[u8],
    analysis: &QuestItemTextUnitAnalysis,
    name_replacements: &BTreeMap<String, Vec<u8>>,
    description_replacements: &BTreeMap<String, QuestItemDescriptionRelinkReplacement>,
) -> Result<QuestItemTextRelinkPlan, QuestItemTextRelinkError> {
    verify_box_quest_item_text_consumer(map, text)?;
    // OP28 초기화 데이터의 목적지를 함께 검증한다. MCRD1 시작이 ITEM_DAT
    // 상한이며, 일반 SEQ의 staging/active copy 크기로 대신하지 않는다.
    for (runtime, resource, destination) in [
        (0x8008_9380, 14u32, 0x801A_1A70u32),
        (0x8008_9398, 15u32, 0x801A_7A70u32),
    ] {
        let offset = map
            .text_offset(runtime)
            .ok_or_else(|| error("QUEST item 적재 record 좌표 사상 실패"))?;
        let mut expected = vec![0x28, 0x10, 0x97, 0x01];
        expected.extend_from_slice(&resource.to_le_bytes());
        expected.extend_from_slice(&destination.to_le_bytes());
        expected.extend_from_slice(&[0; 4]);
        if text.get(offset..offset + expected.len()) != Some(expected.as_slice()) {
            return Err(error("QUEST ITEM_DAT/MCRD1 적재 record 원본 불일치"));
        }
    }
    let text_offset = map
        .text_offset(QUEST_ITEM_DESCRIPTION_ACTIVE_STORE_RUNTIME)
        .ok_or_else(|| error("QUEST item active store의 EXE text 사상 실패"))?;
    let original_bytes = text
        .get(text_offset..text_offset + 4)
        .ok_or_else(|| error("QUEST item active store 원본이 잘림"))?;
    let verified_active_store = verify_bytes(
        IsaProfile::DECLARED,
        QUEST_ITEM_DESCRIPTION_ACTIVE_STORE_RUNTIME,
        original_bytes,
    )?;
    if verified_active_store.instructions()[0].kind
        != (K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0,
        })
    {
        return Err(error(
            "QUEST item description active store 원본 의미 불일치",
        ));
    }
    let control = plan_quest_item_text_relink_control_with_replacements(
        raw,
        analysis,
        name_replacements,
        description_replacements,
    )?;
    Ok(QuestItemTextRelinkPlan {
        control,
        verified_active_store,
    })
}

#[cfg(test)]
#[path = "quest_item_text_relink_tests.rs"]
mod tests;
