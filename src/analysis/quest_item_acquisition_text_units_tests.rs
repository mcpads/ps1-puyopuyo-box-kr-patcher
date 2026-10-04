use crate::disasm::{
    QuestItemAcquisitionMessage, QuestItemAcquisitionMessageRole, QuestItemAcquisitionSummary,
};

use super::{
    audit_quest_item_acquisition_text_units, verify_box_quest_item_acquisition_text_units,
};

fn summary() -> QuestItemAcquisitionSummary {
    let messages = [
        (
            QuestItemAcquisitionMessageRole::CurrencyAcquiredSuffix,
            0x8001_2210,
            "Ｇを手に入れた!!",
            b"currency\0".as_slice(),
        ),
        (
            QuestItemAcquisitionMessageRole::ItemAcquiredSuffix,
            0x8001_2224,
            "を手に入れた!!",
            b"item\0".as_slice(),
        ),
        (
            QuestItemAcquisitionMessageRole::InventoryFullNotice,
            0x8001_2234,
            "しかし　持ち物がいっぱいで持てない。",
            b"full\0".as_slice(),
        ),
        (
            QuestItemAcquisitionMessageRole::DiscardPrompt,
            0x8001_225C,
            "何か　すてますか？",
            b"discard\0".as_slice(),
        ),
        (
            QuestItemAcquisitionMessageRole::AbandonSubject,
            0x8001_2270,
            "アルルは、",
            b"subject\0".as_slice(),
        ),
        (
            QuestItemAcquisitionMessageRole::AbandonParticle,
            0x800B_E1F0,
            "を",
            b"particle\0".as_slice(),
        ),
        (
            QuestItemAcquisitionMessageRole::AbandonPredicate,
            0x8001_227C,
            "あきらめた",
            b"predicate\0".as_slice(),
        ),
    ]
    .into_iter()
    .map(
        |(role, runtime, source_text, source_bytes_with_nul)| QuestItemAcquisitionMessage {
            role,
            runtime,
            source_text,
            source_bytes_with_nul,
        },
    )
    .collect();
    QuestItemAcquisitionSummary {
        opcode: 0xAA,
        handler_runtime: 0,
        subdispatch_table_runtime: 0,
        subdispatch_handlers: [0; 6],
        whole_string_submodes: [0, 5],
        record_bytes: 8,
        record_submode_offset: 1,
        record_item_id_offset: 2,
        record_parameter_offset: 4,
        object_state_offset: 0x73,
        item_id_state_runtime: 0,
        parameter_state_runtime: 0,
        state_machine_runtime: 0,
        draw_callback_runtime: 0,
        record_table_runtime: 0,
        name_pointer_field_offset: 0x0C,
        whole_string_call_runtimes: [0; 2],
        whole_string_draw_runtime: 0,
        fixed_message_call_runtimes: [1, 2, 3, 4, 5, 6],
        advance_sites: [(0, 0, 0); 2],
        fixed_messages: messages,
    }
}

#[test]
fn 고정문은_역할과_원문_digest에_결속된다() {
    let analysis = verify_box_quest_item_acquisition_text_units(&summary()).expect("units");
    assert_eq!(analysis.units.len(), 7);
    assert_eq!(
        analysis.units[4].role,
        QuestItemAcquisitionMessageRole::AbandonSubject
    );
    assert_eq!(analysis.units[4].source_text, "アルルは、");
    assert_eq!(analysis.units[4].source.last(), Some(&0));
}

#[test]
fn 중복_역할은_번역_단위가_되지_않는다() {
    let mut summary = summary();
    summary.fixed_messages[1].role = summary.fixed_messages[0].role;
    let error = audit_quest_item_acquisition_text_units(&summary).expect_err("duplicate role");
    assert!(error.to_string().contains("원문 계약 불일치"));
}
