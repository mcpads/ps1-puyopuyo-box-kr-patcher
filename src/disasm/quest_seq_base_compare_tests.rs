use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    for (table, handlers) in [
        (
            QUEST_SEQ_OP13_COMPARE_TABLE_RUNTIME,
            QUEST_SEQ_OP13_COMPARE_HANDLERS,
        ),
        (
            QUEST_SEQ_OP14_COMPARE_TABLE_RUNTIME,
            QUEST_SEQ_OP14_COMPARE_HANDLERS,
        ),
    ] {
        for (index, handler) in handlers.iter().enumerate() {
            let address = table + u32::try_from(index).expect("14") * 4;
            let offset = map.text_offset(address).expect("비교 table 범위");
            text[offset..offset + 4].copy_from_slice(&handler.to_le_bytes());
        }
    }
    for evidence in EVIDENCE {
        let offset = map
            .text_offset(evidence.address)
            .expect("fixture 명령 범위");
        text[offset..offset + 4].copy_from_slice(&encode(evidence.kind).to_le_bytes());
    }
    (map, text)
}

#[test]
fn op13_14는_같은_selector를_서로_다른_폭으로_해석한다() {
    let halfword = quest_seq_base_compare_record_layout(0x13, 0x80).unwrap();
    assert_eq!(halfword.value_bytes, 2);
    assert_eq!(halfword.operation, QuestSeqBaseCompareOperation::NotEqual);
    assert_eq!(
        halfword.left_source,
        QuestSeqBaseCompareOperandSource::ObjectField
    );
    assert_eq!(
        halfword.right_source,
        QuestSeqBaseCompareOperandSource::Inline
    );
    assert_eq!(halfword.left_parameter_offset, Some(2));
    assert_eq!(halfword.right_parameter_offset, 4);
    assert_eq!(halfword.branch_pointer_offset, 8);
    assert_eq!(halfword.fallthrough_offset, 12);

    let word = quest_seq_base_compare_record_layout(0x14, 0x51).unwrap();
    assert_eq!(word.value_bytes, 4);
    assert_eq!(word.operation, QuestSeqBaseCompareOperation::Equal);
    assert_eq!(
        word.left_source,
        QuestSeqBaseCompareOperandSource::GlobalState
    );
    assert_eq!(
        word.right_source,
        QuestSeqBaseCompareOperandSource::GlobalState
    );
    assert_eq!(word.right_parameter_offset, 3);
    assert_eq!(word.branch_pointer_offset, 4);
    assert_eq!(word.fallthrough_offset, 8);
}

#[test]
fn 지원하지_않는_opcode와_compare_selector를_승격하지_않는다() {
    assert_eq!(quest_seq_base_compare_record_layout(0x12, 0), None);
    assert_eq!(quest_seq_base_compare_record_layout(0x13, 0x0E), None);
    assert_eq!(quest_seq_base_compare_record_layout(0x14, 0x0F), None);
}

#[test]
fn selector_operand와_compare_table을_typed_source로_검증한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_seq_base_compares(map, &text),
        Ok(QuestSeqBaseCompareSummary {
            opcodes: 2,
            operations: 14,
            halfword_opcodes: 1,
            word_opcodes: 1,
            state_base_runtime: 0x800D_B6C0,
        })
    );
}

#[test]
fn selector와_compare_table_drift를_거부한다() {
    let (map, mut text) = fixture();
    let selector = map.text_offset(0x8002_0F50).unwrap();
    text[selector..selector + 4].copy_from_slice(
        &encode(K::Lbu {
            rt: Reg::T1,
            base: Reg::A0,
            offset: 2,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_base_compares(map, &text),
        Err(QuestSeqBaseCompareError::InstructionMismatch {
            opcode: 0x13,
            address: 0x8002_0F50,
            ..
        })
    ));

    let (map, mut text) = fixture();
    let table = map
        .text_offset(QUEST_SEQ_OP14_COMPARE_TABLE_RUNTIME)
        .unwrap();
    text[table..table + 4].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        verify_box_quest_seq_base_compares(map, &text),
        Err(QuestSeqBaseCompareError::DispatchWordMismatch {
            opcode: 0x14,
            selector: 0,
            ..
        })
    ));
}
