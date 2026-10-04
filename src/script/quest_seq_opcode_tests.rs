use super::*;
use crate::script::QUEST_SEQ_POINTER_RECORD_SPECS;

#[test]
fn opcode_80부터_ff까지_128개_handler를_소유한다() {
    assert_eq!(quest_seq_opcode_handler(0x00), Some(0x8002_2650));
    assert_eq!(quest_seq_opcode_handler(0x7F), Some(0x8002_263C));
    assert_eq!(quest_seq_opcode_handler(0x80), Some(0x8007_80E4));
    assert_eq!(quest_seq_opcode_handler(0xDF), Some(0x8007_80DC));
    assert_eq!(quest_seq_opcode_handler(0xE0), Some(0x8006_FDB8));
    assert_eq!(quest_seq_opcode_handler(0xFF), Some(0x8007_00A0));
    assert_eq!(QUEST_SEQ_DISPATCH_HANDLERS.len(), 128);
}

#[test]
fn base_00부터_7f까지_128개_handler를_소유한다() {
    assert_eq!(QUEST_SEQ_BASE_DISPATCH_HANDLERS.len(), 128);
    assert_eq!(quest_seq_base_dispatch_fallback_entries(), 82);
    assert_eq!(quest_seq_base_dispatch_unique_handlers(), 47);
}

#[test]
fn 두_table의_fallback과_고유_handler_분모가_고정되어_있다() {
    assert_eq!(
        QUEST_SEQ_DISPATCH_HANDLERS[..QUEST_SEQ_MAIN_OPCODE_COUNT]
            .iter()
            .filter(|&&handler| handler == QUEST_SEQ_MAIN_FALLBACK_HANDLER_RUNTIME)
            .count(),
        23
    );
    assert_eq!(
        QUEST_SEQ_DISPATCH_HANDLERS[QUEST_SEQ_MAIN_OPCODE_COUNT..]
            .iter()
            .filter(|&&handler| handler == QUEST_SEQ_EXTENDED_FALLBACK_HANDLER_RUNTIME)
            .count(),
        27
    );
    assert_eq!(quest_seq_dispatch_fallback_entries(), 50);
    assert_eq!(quest_seq_dispatch_unique_handlers(), 80);
}

#[test]
fn 포인터_record_사양의_handler가_전체_dispatch와_일치한다() {
    for spec in QUEST_SEQ_POINTER_RECORD_SPECS {
        assert_eq!(
            quest_seq_opcode_handler(spec.opcode),
            Some(spec.handler_runtime),
            "opcode 0x{:02X}",
            spec.opcode
        );
    }
}
