//! QUEST `OPAA` 아이템 획득 state machine과 이름 문구 소비 경로.
//!
//! `OPAA` submode 0/5는 record의 item ID를 전역 상태에 저장한 뒤
//! `0x8006125C`를 반복 호출한다. 이 함수는 같은 ID로 `ITEM_DAT.SEQ`
//! record를 찾고 `+0x0C` 이름 pointer를 두 whole-string site에 넘긴다.
//! 이 모듈은 그 연결과 주변 고정 SJIS 문구를 검증하며, 실제 record
//! 도달은 `analysis::quest_item_acquisition_records`가 별도로 소유한다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{decode, InstructionKind as K};
use crate::script::quest_seq_opcode::quest_seq_opcode_handler;

mod instructions;
mod messages;

use instructions::acquisition_instructions;
use messages::QUEST_ITEM_ACQUISITION_MESSAGE_SPECS;

use super::quest_item_text_consumer::QuestItemTextConsumerSummary;

pub const QUEST_ITEM_ACQUISITION_OPCODE: u8 = 0xAA;
pub const QUEST_ITEM_ACQUISITION_HANDLER_RUNTIME: u32 = 0x8007_4CEC;
pub const QUEST_ITEM_ACQUISITION_SUBDISPATCH_TABLE_RUNTIME: u32 = 0x8001_3088;
pub const QUEST_ITEM_ACQUISITION_RECORD_BYTES: usize = 8;
pub const QUEST_ITEM_ACQUISITION_RECORD_SUBMODE_OFFSET: usize = 1;
pub const QUEST_ITEM_ACQUISITION_RECORD_ITEM_ID_OFFSET: usize = 2;
pub const QUEST_ITEM_ACQUISITION_RECORD_PARAMETER_OFFSET: usize = 4;
pub const QUEST_ITEM_ACQUISITION_OBJECT_STATE_OFFSET: i16 = 0x73;
pub const QUEST_ITEM_ACQUISITION_ITEM_ID_STATE_RUNTIME: u32 = 0x800D_B7E0;
pub const QUEST_ITEM_ACQUISITION_PARAMETER_STATE_RUNTIME: u32 = 0x800D_B7E4;
pub const QUEST_ITEM_ACQUISITION_STATE_MACHINE_RUNTIME: u32 = 0x8006_125C;
pub const QUEST_ITEM_ACQUISITION_DRAW_CALLBACK_RUNTIME: u32 = 0x8006_0CE8;

const QUEST_ITEM_ACQUISITION_SUBDISPATCH_HANDLERS: [u32; 6] = [
    0x8007_4D38,
    0x8007_4DD8,
    0x8007_4E20,
    0x8007_4E4C,
    0x8007_4EBC,
    0x8007_4D38,
];
const QUEST_ITEM_ACQUISITION_WHOLE_STRING_SUBMODES: [u8; 2] = [0, 5];
pub const QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES: [u32; 6] = [
    0x8006_14E4,
    0x8006_17B8,
    0x8006_17FC,
    0x8006_18E8,
    0x8006_1904,
    0x8006_191C,
];
const QUEST_ITEM_ACQUISITION_ADVANCE_SITES: [(u32, u32, u32); 2] = [
    (0x8007_4EA8, 0x8007_4EB0, 0x8007_4EB8),
    (0x8007_5090, 0x8007_5098, 0x8007_509C),
];

/// 아이템 획득 문장에서 원문 EXE가 직접 공급하는 고정 조각의 역할.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuestItemAcquisitionMessageRole {
    CurrencyAcquiredSuffix,
    ItemAcquiredSuffix,
    InventoryFullNotice,
    DiscardPrompt,
    AbandonSubject,
    AbandonParticle,
    AbandonPredicate,
}

impl QuestItemAcquisitionMessageRole {
    pub const fn label(self) -> &'static str {
        match self {
            Self::CurrencyAcquiredSuffix => "currency-acquired-suffix",
            Self::ItemAcquiredSuffix => "item-acquired-suffix",
            Self::InventoryFullNotice => "inventory-full-notice",
            Self::DiscardPrompt => "discard-prompt",
            Self::AbandonSubject => "abandon-subject",
            Self::AbandonParticle => "abandon-particle",
            Self::AbandonPredicate => "abandon-predicate",
        }
    }
}

/// 검증된 고정 SJIS 문구 조각.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestItemAcquisitionMessage {
    pub role: QuestItemAcquisitionMessageRole,
    pub runtime: u32,
    pub source_text: &'static str,
    pub source_bytes_with_nul: &'static [u8],
}

/// `quest_item_text_consumer`가 먼저 검증한 이름 consumer 중 이 경로가 쓰는 부분.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestItemAcquisitionConsumerEvidence {
    pub record_table_runtime: u32,
    pub name_pointer_field_offset: i16,
    pub whole_string_call_runtimes: [u32; 2],
    pub whole_string_draw_runtime: u32,
}

/// 지원 원본에서 결속한 `OPAA` 아이템 획득 경로.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemAcquisitionSummary {
    pub opcode: u8,
    pub handler_runtime: u32,
    pub subdispatch_table_runtime: u32,
    pub subdispatch_handlers: [u32; 6],
    pub whole_string_submodes: [u8; 2],
    pub record_bytes: usize,
    pub record_submode_offset: usize,
    pub record_item_id_offset: usize,
    pub record_parameter_offset: usize,
    pub object_state_offset: i16,
    pub item_id_state_runtime: u32,
    pub parameter_state_runtime: u32,
    pub state_machine_runtime: u32,
    pub draw_callback_runtime: u32,
    pub record_table_runtime: u32,
    pub name_pointer_field_offset: i16,
    pub whole_string_call_runtimes: [u32; 2],
    pub whole_string_draw_runtime: u32,
    pub fixed_message_call_runtimes: [u32; 6],
    pub advance_sites: [(u32, u32, u32); 2],
    pub fixed_messages: Vec<QuestItemAcquisitionMessage>,
}

/// `OPAA` 획득 경로 정적 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemAcquisitionError(String);

impl fmt::Display for QuestItemAcquisitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemAcquisitionError {}

fn error(message: impl Into<String>) -> QuestItemAcquisitionError {
    QuestItemAcquisitionError(message.into())
}

fn read_word(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    role: &str,
) -> Result<u32, QuestItemAcquisitionError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "QUEST item acquisition {role} 주소가 EXE text 밖임: 0x{runtime:08X}"
        ))
    })?;
    let raw = text.get(offset..offset + 4).ok_or_else(|| {
        error(format!(
            "QUEST item acquisition {role} word 범위 이탈: 0x{runtime:08X}"
        ))
    })?;
    Ok(u32::from_le_bytes(raw.try_into().expect("4바이트 word")))
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
) -> Result<(), QuestItemAcquisitionError> {
    let actual = decode(runtime, read_word(map, text, runtime, "명령")?);
    if actual.kind != expected {
        return Err(error(format!(
            "QUEST item acquisition typed 명령 불일치: address=0x{runtime:08X}, expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn require_bytes(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: &[u8],
    role: QuestItemAcquisitionMessageRole,
) -> Result<(), QuestItemAcquisitionError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "QUEST item acquisition {} 주소가 EXE text 밖임: 0x{runtime:08X}",
            role.label()
        ))
    })?;
    let actual = text.get(offset..offset + expected.len()).ok_or_else(|| {
        error(format!(
            "QUEST item acquisition {} 원문 범위 이탈: 0x{runtime:08X}",
            role.label()
        ))
    })?;
    if actual != expected {
        return Err(error(format!(
            "QUEST item acquisition {} 원문 불일치: 0x{runtime:08X}",
            role.label()
        )));
    }
    Ok(())
}

/// 지정한 consumer 근거에 `OPAA` state machine을 결속한다.
pub fn audit_quest_item_acquisition(
    map: &ExeMap,
    text: &[u8],
    consumer: QuestItemAcquisitionConsumerEvidence,
) -> Result<QuestItemAcquisitionSummary, QuestItemAcquisitionError> {
    if map.name != "QUEST.EXE" {
        return Err(error(format!(
            "item acquisition 검증 대상이 QUEST.EXE가 아님: {}",
            map.name
        )));
    }
    if text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST item acquisition text 길이 불일치: expected={}, actual={}",
            map.text_len,
            text.len()
        )));
    }
    if quest_seq_opcode_handler(QUEST_ITEM_ACQUISITION_OPCODE)
        != Some(QUEST_ITEM_ACQUISITION_HANDLER_RUNTIME)
    {
        return Err(error("QUEST OPAA handler 모델 불일치"));
    }
    if consumer.record_table_runtime != 0x801A_1A70
        || consumer.name_pointer_field_offset != 0x0C
        || consumer.whole_string_call_runtimes != [0x8006_14D0, 0x8006_18F4]
        || consumer.whole_string_draw_runtime != 0x8003_CD04
    {
        return Err(error(format!(
            "QUEST item acquisition consumer 근거 불일치: {consumer:?}"
        )));
    }

    for (index, expected) in QUEST_ITEM_ACQUISITION_SUBDISPATCH_HANDLERS
        .iter()
        .copied()
        .enumerate()
    {
        let runtime = QUEST_ITEM_ACQUISITION_SUBDISPATCH_TABLE_RUNTIME
            + u32::try_from(index * 4).expect("subdispatch index");
        let actual = read_word(map, text, runtime, "subdispatch table")?;
        if actual != expected {
            return Err(error(format!(
                "QUEST item acquisition subdispatch 불일치: index={index}, expected=0x{expected:08X}, actual=0x{actual:08X}"
            )));
        }
    }
    for (runtime, expected) in acquisition_instructions(consumer.whole_string_draw_runtime) {
        require_instruction(map, text, runtime, expected)?;
    }
    for spec in QUEST_ITEM_ACQUISITION_MESSAGE_SPECS {
        require_bytes(
            map,
            text,
            spec.message.runtime,
            spec.message.source_bytes_with_nul,
            spec.message.role,
        )?;
    }

    Ok(QuestItemAcquisitionSummary {
        opcode: QUEST_ITEM_ACQUISITION_OPCODE,
        handler_runtime: QUEST_ITEM_ACQUISITION_HANDLER_RUNTIME,
        subdispatch_table_runtime: QUEST_ITEM_ACQUISITION_SUBDISPATCH_TABLE_RUNTIME,
        subdispatch_handlers: QUEST_ITEM_ACQUISITION_SUBDISPATCH_HANDLERS,
        whole_string_submodes: QUEST_ITEM_ACQUISITION_WHOLE_STRING_SUBMODES,
        record_bytes: QUEST_ITEM_ACQUISITION_RECORD_BYTES,
        record_submode_offset: QUEST_ITEM_ACQUISITION_RECORD_SUBMODE_OFFSET,
        record_item_id_offset: QUEST_ITEM_ACQUISITION_RECORD_ITEM_ID_OFFSET,
        record_parameter_offset: QUEST_ITEM_ACQUISITION_RECORD_PARAMETER_OFFSET,
        object_state_offset: QUEST_ITEM_ACQUISITION_OBJECT_STATE_OFFSET,
        item_id_state_runtime: QUEST_ITEM_ACQUISITION_ITEM_ID_STATE_RUNTIME,
        parameter_state_runtime: QUEST_ITEM_ACQUISITION_PARAMETER_STATE_RUNTIME,
        state_machine_runtime: QUEST_ITEM_ACQUISITION_STATE_MACHINE_RUNTIME,
        draw_callback_runtime: QUEST_ITEM_ACQUISITION_DRAW_CALLBACK_RUNTIME,
        record_table_runtime: consumer.record_table_runtime,
        name_pointer_field_offset: consumer.name_pointer_field_offset,
        whole_string_call_runtimes: consumer.whole_string_call_runtimes,
        whole_string_draw_runtime: consumer.whole_string_draw_runtime,
        fixed_message_call_runtimes: QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES,
        advance_sites: QUEST_ITEM_ACQUISITION_ADVANCE_SITES,
        fixed_messages: QUEST_ITEM_ACQUISITION_MESSAGE_SPECS
            .iter()
            .map(|spec| spec.message)
            .collect(),
    })
}

/// 검증된 item text consumer 요약과 지원 원본 `OPAA`를 함께 검증한다.
pub fn verify_box_quest_item_acquisition(
    map: &ExeMap,
    text: &[u8],
    item_text: &QuestItemTextConsumerSummary,
) -> Result<QuestItemAcquisitionSummary, QuestItemAcquisitionError> {
    // 획득문은 이름 consumer 전체 중 OPAA의 두 호출만 사용한다.
    let whole_string_call_runtimes = [0x8006_14D0, 0x8006_18F4];
    if !whole_string_call_runtimes
        .iter()
        .all(|site| item_text.whole_string_name_call_runtimes.contains(site))
    {
        return Err(error("QUEST item acquisition 이름 consumer 누락"));
    }
    audit_quest_item_acquisition(
        map,
        text,
        QuestItemAcquisitionConsumerEvidence {
            record_table_runtime: item_text.record_table_runtime,
            name_pointer_field_offset: item_text.name_pointer_field_offset,
            whole_string_call_runtimes,
            whole_string_draw_runtime: item_text.whole_string_draw_runtime,
        },
    )
}

#[cfg(test)]
#[path = "quest_item_acquisition_tests.rs"]
mod tests;
