//! `SUKE.SEQ` 조우가 두 대사 stream으로 갈라지는 source 경로를 결속한다.

use ps1_puyopuyo_box::analysis::quest_seq_flag_branches::{
    QuestSeqFlagBranchAnalysis, BOX_QUEST_SUKE_CONFRONTATION_BRANCH_OFFSET,
    BOX_QUEST_SUKE_CONFRONTATION_FALLTHROUGH_OFFSET,
    BOX_QUEST_SUKE_CONFRONTATION_FLAG_RECORD_OFFSET,
};
use ps1_puyopuyo_box::analysis::quest_seq_text_paths::{
    QuestSeqTextPathAnalysis, QuestSeqTextPathStatus, QuestSeqTextPathTransition,
};
use ps1_puyopuyo_box::disasm::{QuestSeqChildActivationSummary, QuestSeqChildConfigurationSummary};
use ps1_puyopuyo_box::script::quest_seq_child_contact::{
    parse_quest_seq_child_contact, QuestSeqChildContact, QUEST_SEQ_CHILD_CONFIGURATION_OPCODE,
    QUEST_SEQ_CHILD_CONFIGURATION_RECORD_BYTES, QUEST_SEQ_CHILD_CONTACT_RECORDS_BYTES,
};
use ps1_puyopuyo_box::script::quest_seq_inventory::QuestSeqPointerBehavior;
use ps1_puyopuyo_box::script::quest_seq_text_extraction::QuestSeqTextExtractionAnalysis;

const SUKE_SEQUENCE: &str = "SUKE.SEQ";
const SUKE_PRODUCER_FLAG_RECORD_OFFSET: usize = 0x06E8;
const SUKE_CHILD_RECORD_OFFSET: usize = 0x06F4;
const SUKE_CHILD_POINTER_FIELD_OFFSET: usize = 0x06FC;
const SUKE_CHILD_CONFIGURATION_OFFSET: usize = 0x0700;
const SUKE_CHILD_STREAM_OFFSET: usize = 0x075C;
const QUEST_FIXED_POINT_SHIFT: u8 = 12;
const SUKE_CHILD_PRELUDE: [u8; 8] = [0xA4, 0x00, 0x00, 0x00, 0x90, 0x00, 0x03, 0x00];

pub(super) struct SukeConfrontationRouteAuditInputs<'a> {
    pub sequences: &'a [(String, Vec<u8>)],
    pub text_paths: &'a QuestSeqTextPathAnalysis,
    pub flag_branches: &'a QuestSeqFlagBranchAnalysis,
    pub text_extraction: &'a QuestSeqTextExtractionAnalysis,
    pub child_activation: &'a QuestSeqChildActivationSummary,
    pub child_configuration: &'a QuestSeqChildConfigurationSummary,
}

pub(super) struct SukeConfrontationRouteAudit {
    pub producer_flag_record_offset: usize,
    pub child_record_offset: usize,
    pub child_configuration_offset: usize,
    pub child_configuration_end_offset: usize,
    pub child_stream_offset: usize,
    pub child_contact: QuestSeqChildContact,
    pub update_callback_runtime: u32,
    pub flag_record_offset: usize,
    pub probe_runtime: u32,
    pub probe_mask: u32,
    pub clear_target_offset: usize,
    pub set_target_offset: usize,
}

pub(super) fn audit_suke_confrontation_route(
    inputs: SukeConfrontationRouteAuditInputs<'_>,
) -> Result<SukeConfrontationRouteAudit, String> {
    let suke_raw = inputs
        .sequences
        .iter()
        .find_map(|(name, raw)| (name == SUKE_SEQUENCE).then_some(raw.as_slice()))
        .ok_or_else(|| "SUKE.SEQ 원본이 없음".to_owned())?;
    if suke_raw.get(SUKE_CHILD_RECORD_OFFSET + 1) != Some(&2) {
        return Err("SUKE OP82 record의 subtype이 2가 아님".to_owned());
    }
    if inputs.child_configuration.opcode != QUEST_SEQ_CHILD_CONFIGURATION_OPCODE
        || inputs.child_configuration.record_bytes != QUEST_SEQ_CHILD_CONFIGURATION_RECORD_BYTES
        || inputs.child_configuration.fixed_point_shift != QUEST_FIXED_POINT_SHIFT
    {
        return Err(format!(
            "QUEST OP83 consumer와 source parser 의미 불일치: {:?}",
            inputs.child_configuration
        ));
    }
    let child_configuration_end_offset = SUKE_CHILD_CONFIGURATION_OFFSET
        .checked_add(QUEST_SEQ_CHILD_CONTACT_RECORDS_BYTES)
        .ok_or_else(|| "SUKE OP83 record 끝 offset overflow".to_owned())?;
    let child_contact = parse_quest_seq_child_contact(
        suke_raw
            .get(SUKE_CHILD_CONFIGURATION_OFFSET..child_configuration_end_offset)
            .ok_or_else(|| "SUKE OP83 child 구성 record가 잘림".to_owned())?,
    )
    .map_err(|error| error.to_string())?;
    if suke_raw.get(SUKE_CHILD_STREAM_OFFSET..SUKE_CHILD_STREAM_OFFSET + 8)
        != Some(SUKE_CHILD_PRELUDE.as_slice())
    {
        return Err("SUKE child의 OPA4/OP90 mode record가 원본과 다름".to_owned());
    }
    let producer = inputs
        .flag_branches
        .branches
        .iter()
        .find(|branch| {
            branch.sequence == SUKE_SEQUENCE
                && branch.record_offset == SUKE_PRODUCER_FLAG_RECORD_OFFSET
        })
        .ok_or_else(|| "SUKE OP82 producer OPA1 record가 없음".to_owned())?;
    if producer.expected_value != 0
        || producer.fallthrough_offset != SUKE_CHILD_RECORD_OFFSET
        || producer.fallthrough_opcode != 0x82
    {
        return Err(format!(
            "SUKE producer OPA1과 OP82의 결속 불일치: {producer:?}"
        ));
    }

    let path = inputs
        .text_paths
        .paths
        .iter()
        .find(|path| path.sequence == SUKE_SEQUENCE && path.seed_offset == SUKE_CHILD_STREAM_OFFSET)
        .ok_or_else(|| "SUKE OP82 child stream 경로가 없음".to_owned())?;
    if !path.sources.iter().any(|source| {
        source.source_record_offset == SUKE_CHILD_RECORD_OFFSET
            && source.pointer_field_offset == SUKE_CHILD_POINTER_FIELD_OFFSET
            && source.opcode == 0x82
            && source.behavior == QuestSeqPointerBehavior::SpawnStream
    }) {
        return Err("SUKE OP82 record와 child stream seed의 결속이 없음".to_owned());
    }
    let expected_nodes = [
        (
            0x075C,
            0xA4,
            QuestSeqTextPathTransition::DirectAdvance,
            Some(0x0760),
        ),
        (
            0x0760,
            0x90,
            QuestSeqTextPathTransition::DirectAdvance,
            Some(0x0764),
        ),
        (0x0764, 0xA1, QuestSeqTextPathTransition::End, None),
    ];
    if path.nodes.len() != expected_nodes.len()
        || path.nodes.iter().zip(expected_nodes).any(
            |(node, (offset, opcode, transition, next_offset))| {
                node.offset != offset
                    || node.opcode != opcode
                    || node.transition != transition
                    || node.next_offset != next_offset
            },
        )
        || path.status != QuestSeqTextPathStatus::StoppedAtStreamPointerOpcode
    {
        return Err(format!(
            "SUKE OP82 child stream의 OPA1 선행 경로 불일치: {path:?}"
        ));
    }

    let branch = inputs
        .flag_branches
        .branches
        .iter()
        .find(|branch| {
            branch.sequence == SUKE_SEQUENCE
                && branch.record_offset == BOX_QUEST_SUKE_CONFRONTATION_FLAG_RECORD_OFFSET
        })
        .ok_or_else(|| "SUKE 조우 OPA1 record가 없음".to_owned())?;
    for offset in [
        BOX_QUEST_SUKE_CONFRONTATION_FALLTHROUGH_OFFSET,
        BOX_QUEST_SUKE_CONFRONTATION_BRANCH_OFFSET,
    ] {
        if !inputs
            .text_extraction
            .streams
            .iter()
            .any(|stream| stream.sequence == SUKE_SEQUENCE && stream.record_offset == offset)
        {
            return Err(format!(
                "SUKE OPA1 target이 canonical OP21 추출에 없음: 0x{offset:04X}"
            ));
        }
    }

    Ok(SukeConfrontationRouteAudit {
        producer_flag_record_offset: producer.record_offset,
        child_record_offset: SUKE_CHILD_RECORD_OFFSET,
        child_configuration_offset: SUKE_CHILD_CONFIGURATION_OFFSET,
        child_configuration_end_offset,
        child_stream_offset: SUKE_CHILD_STREAM_OFFSET,
        child_contact,
        update_callback_runtime: inputs.child_activation.update_callback_runtime,
        flag_record_offset: branch.record_offset,
        probe_runtime: branch.probe.runtime_address,
        probe_mask: branch.probe.mask,
        clear_target_offset: branch.selected_offset(0),
        set_target_offset: branch.selected_offset(branch.probe.mask),
    })
}
