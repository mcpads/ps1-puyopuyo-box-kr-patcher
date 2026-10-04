use super::*;

#[test]
fn op28의_하위_32개_dispatch와_flow를_전수_고정한다() {
    for subopcode in 0u8..0x20 {
        assert_eq!(
            quest_seq_op28_handler(subopcode),
            Some(QUEST_SEQ_OP28_DISPATCH_HANDLERS[usize::from(subopcode)])
        );
        assert_eq!(
            quest_seq_op28_flow(subopcode),
            Some(QUEST_SEQ_OP28_FLOWS[usize::from(subopcode)])
        );
    }
    assert_eq!(quest_seq_op28_handler(0x20), None);
    assert_eq!(quest_seq_op28_flow(0x20), None);
}

#[test]
fn op28의_unique와_fallback분모를_고정한다() {
    assert_eq!(quest_seq_op28_unique_handler_count(), 14);
    assert_eq!(quest_seq_op28_fallback_entry_count(), 19);
}

#[test]
fn sub00과01만_고정전진외_branch를_가진다() {
    let branches = QUEST_SEQ_OP28_FLOWS
        .iter()
        .filter(|flow| matches!(flow, QuestSeqOp28Flow::AdvanceOrBranch { .. }))
        .count();
    assert_eq!(branches, 2);
    assert_eq!(
        QUEST_SEQ_OP28_FLOWS[0],
        QuestSeqOp28Flow::AdvanceOrBranch {
            advance_bytes: 8,
            condition_offset: 3,
            pointer_offset: 4,
        }
    );
    assert_eq!(
        QUEST_SEQ_OP28_FLOWS[1],
        QuestSeqOp28Flow::AdvanceOrBranch {
            advance_bytes: 12,
            condition_offset: 3,
            pointer_offset: 8,
        }
    );
}
