//! `OPAA` source record와 검증된 아이템 이름 consumer를 결속한다.

use ps1_puyopuyo_box::analysis::{
    verify_box_quest_item_acquisition_records, QuestSeqTextPathAnalysis,
};
use ps1_puyopuyo_box::disasm::QuestItemAcquisitionSummary;

pub(super) struct QuestItemAcquisitionRouteAuditInputs<'a> {
    pub sequences: &'a [(String, Vec<u8>)],
    pub text_paths: &'a QuestSeqTextPathAnalysis,
    pub consumer: &'a QuestItemAcquisitionSummary,
}

pub(super) fn audit_quest_item_acquisition_route(
    inputs: QuestItemAcquisitionRouteAuditInputs<'_>,
) -> Result<(), String> {
    let records = verify_box_quest_item_acquisition_records(
        inputs
            .sequences
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        inputs.text_paths,
        inputs.consumer,
    )
    .map_err(|error| error.to_string())?;
    let record = records
        .occurrences
        .first()
        .ok_or_else(|| "검증 뒤 QUEST item acquisition record가 없음".to_string())?;
    println!(
        "PASS QUEST item acquisition source route: {} seed/record 0x{:04X}/0x{:04X}, \
         OP{:02X} submode {}, item {}, parameter {}, whole-string {}",
        record.sequence,
        record.seed_offset,
        record.record_offset,
        inputs.consumer.opcode,
        record.submode,
        record.item_id,
        record.parameter,
        record.uses_whole_string_name,
    );
    Ok(())
}
