use crate::coords::LogicalFile;
use crate::isa::{encode, Reg};

use super::*;

fn fixture_map() -> ExeMap {
    ExeMap {
        name: "QUEST.EXE",
        load_addr: 0x8001_0000,
        text_len: 0x000A_E800,
        entry_pc: 0x800A_A6E8,
        file: LogicalFile::new(0, 0x000A_F000),
    }
}

fn write_word(map: &ExeMap, text: &mut [u8], runtime: u32, word: u32) {
    let offset = map.text_offset(runtime).expect("fixture runtime");
    text[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}

fn write_bytes(map: &ExeMap, text: &mut [u8], runtime: u32, bytes: &[u8]) {
    let offset = map.text_offset(runtime).expect("fixture runtime");
    text[offset..offset + bytes.len()].copy_from_slice(bytes);
}

fn consumer_evidence() -> QuestItemAcquisitionConsumerEvidence {
    QuestItemAcquisitionConsumerEvidence {
        record_table_runtime: 0x801A_1A70,
        name_pointer_field_offset: 0x0C,
        whole_string_call_runtimes: [0x8006_14D0, 0x8006_18F4],
        whole_string_draw_runtime: 0x8003_CD04,
    }
}

fn fixture_text(map: &ExeMap) -> Vec<u8> {
    let mut text = vec![0; map.text_len as usize];
    for (index, handler) in QUEST_ITEM_ACQUISITION_SUBDISPATCH_HANDLERS
        .iter()
        .copied()
        .enumerate()
    {
        write_word(
            map,
            &mut text,
            QUEST_ITEM_ACQUISITION_SUBDISPATCH_TABLE_RUNTIME + index as u32 * 4,
            handler,
        );
    }
    for (runtime, instruction) in acquisition_instructions(0x8003_CD04) {
        write_word(map, &mut text, runtime, encode(instruction));
    }
    for spec in QUEST_ITEM_ACQUISITION_MESSAGE_SPECS {
        write_bytes(
            map,
            &mut text,
            spec.message.runtime,
            spec.message.source_bytes_with_nul,
        );
    }
    text
}

#[test]
fn opaa는_record_item_id를_두_whole_string_name_consumer에_결속한다() {
    let map = fixture_map();
    let text = fixture_text(&map);

    let summary =
        audit_quest_item_acquisition(&map, &text, consumer_evidence()).expect("fixture OPAA route");

    assert_eq!(summary.opcode, 0xAA);
    assert_eq!(summary.whole_string_submodes, [0, 5]);
    assert_eq!(summary.record_submode_offset, 1);
    assert_eq!(summary.record_item_id_offset, 2);
    assert_eq!(summary.record_parameter_offset, 4);
    assert_eq!(
        summary.whole_string_call_runtimes,
        [0x8006_14D0, 0x8006_18F4]
    );
    assert_eq!(summary.fixed_messages.len(), 7);
}

#[test]
fn item_id_store가_바뀌면_opaa_contract를_거부한다() {
    let map = fixture_map();
    let mut text = fixture_text(&map);
    write_word(
        &map,
        &mut text,
        0x8007_4D68,
        encode(K::Sw {
            rt: Reg::V1,
            base: Reg::V1,
            offset: 0x120,
        }),
    );

    let error = audit_quest_item_acquisition(&map, &text, consumer_evidence())
        .expect_err("item ID store drift");

    assert!(error.to_string().contains("typed 명령 불일치"));
}

#[test]
fn 고정_sjis_문구가_바뀌면_opaa_contract를_거부한다() {
    let map = fixture_map();
    let mut text = fixture_text(&map);
    write_bytes(&map, &mut text, 0x8001_2224, b"X");

    let error = audit_quest_item_acquisition(&map, &text, consumer_evidence())
        .expect_err("fixed message drift");

    assert!(error
        .to_string()
        .contains("item-acquired-suffix 원문 불일치"));
}
