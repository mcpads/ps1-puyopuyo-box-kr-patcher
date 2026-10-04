use crate::analysis::audit_quest_seq_text_paths;
use crate::disasm::QuestItemAcquisitionSummary;
use crate::font::{QuestCodeTable, QUEST_CODE_TABLE_COUNT};
use crate::script::TOWN00_SEQ_RUNTIME_BASE;

use super::*;

fn code_table() -> QuestCodeTable {
    let mut table = (0..QUEST_CODE_TABLE_COUNT)
        .map(|index| {
            let value = u16::try_from(index).expect("test index");
            value.to_le_bytes()
        })
        .collect::<Vec<_>>();
    table[0] = [0x82, 0xA0];
    QuestCodeTable::parse(&table.into_iter().flatten().collect::<Vec<_>>())
        .expect("test code table")
}

fn route() -> QuestItemAcquisitionSummary {
    QuestItemAcquisitionSummary {
        opcode: 0xAA,
        handler_runtime: 0x8007_4CEC,
        subdispatch_table_runtime: 0x8001_3088,
        subdispatch_handlers: [0; 6],
        whole_string_submodes: [0, 5],
        record_bytes: 8,
        record_submode_offset: 1,
        record_item_id_offset: 2,
        record_parameter_offset: 4,
        object_state_offset: 0x73,
        item_id_state_runtime: 0x800D_B7E0,
        parameter_state_runtime: 0x800D_B7E4,
        state_machine_runtime: 0x8006_125C,
        draw_callback_runtime: 0x8006_0CE8,
        record_table_runtime: 0x801A_1A70,
        name_pointer_field_offset: 0x0C,
        whole_string_call_runtimes: [0x8006_14D0, 0x8006_18F4],
        whole_string_draw_runtime: 0x8003_CD04,
        fixed_message_call_runtimes:
            crate::disasm::QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES,
        advance_sites: [(0, 0, 0); 2],
        fixed_messages: Vec::new(),
    }
}

fn source(submode: u8) -> Vec<u8> {
    let mut raw = vec![0xA1, 0, 0, 0, 0, 0, 0, 0];
    raw.extend_from_slice(&(TOWN00_SEQ_RUNTIME_BASE + 12).to_le_bytes());
    raw.extend_from_slice(&[0xAA, submode, 5, 0, 0x34, 0x12, 0, 0]);
    raw.extend_from_slice(&[0, 0, 0, 0]);
    raw
}

#[test]
fn 정적_path의_opaa_record를_consumer_layout으로_해석한다() {
    let raw = source(5);
    let paths = audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table())
        .expect("fixture path");

    let analysis =
        audit_quest_item_acquisition_records([("TEST.SEQ", raw.as_slice())], &paths, &route())
            .expect("fixture acquisition record");

    assert_eq!(analysis.occurrences.len(), 1);
    let occurrence = &analysis.occurrences[0];
    assert_eq!(occurrence.seed_offset, 12);
    assert_eq!(occurrence.record_offset, 12);
    assert_eq!(occurrence.submode, 5);
    assert_eq!(occurrence.item_id, 5);
    assert_eq!(occurrence.parameter, 0x1234);
    assert!(occurrence.uses_whole_string_name);
}

#[test]
fn 다른_submode는_whole_string_name경로로_세지_않는다() {
    let raw = source(1);
    let paths = audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table())
        .expect("fixture path");

    let analysis =
        audit_quest_item_acquisition_records([("TEST.SEQ", raw.as_slice())], &paths, &route())
            .expect("fixture acquisition record");

    assert!(!analysis.occurrences[0].uses_whole_string_name);
}

#[test]
fn path의_advance폭이_consumer_record폭과_다르면_거부한다() {
    let raw = source(5);
    let mut paths = audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table())
        .expect("fixture path");
    paths.paths[0].nodes[0].direct_advance_bytes = Some(4);

    let error =
        audit_quest_item_acquisition_records([("TEST.SEQ", raw.as_slice())], &paths, &route())
            .expect_err("advance drift");

    assert!(error.to_string().contains("path advance 불일치"));
}

#[test]
fn path를_만든_원문과_다른_sequence는_거부한다() {
    let raw = source(5);
    let paths = audit_quest_seq_text_paths([("TEST.SEQ", raw.as_slice())], &code_table())
        .expect("fixture path");
    let mut changed = raw.clone();
    changed[3] = 1;

    let error =
        audit_quest_item_acquisition_records([("TEST.SEQ", changed.as_slice())], &paths, &route())
            .expect_err("source digest drift");

    assert!(error.to_string().contains("path/source digest 불일치"));
}
