use super::*;

fn fixture(expected_value: u8) -> Vec<u8> {
    let mut raw = vec![0; 0x80];
    let branch_runtime = QUEST_SEQ_ACTIVE_RUNTIME_BASE + 0x60;
    raw[0x20..0x2C].copy_from_slice(&[
        0xA1,
        expected_value,
        0x00,
        0x80,
        0x11,
        0xA5,
        0x5A,
        0xC3,
        branch_runtime as u8,
        (branch_runtime >> 8) as u8,
        (branch_runtime >> 16) as u8,
        (branch_runtime >> 24) as u8,
    ]);
    raw[0x2C] = 0x21;
    raw[0x60] = 0x21;
    raw
}

#[test]
fn opa1은_flag가_기대값이면_fallthrough를_고른다() {
    let raw = fixture(0);
    let branch = parse_quest_seq_flag_branch(
        "TEST.SEQ",
        &raw,
        QuestSeqRuntimeBaseEvidence::StaticCommonLoader,
        0x20,
    )
    .unwrap()
    .expect("OPA1 record");

    assert_eq!(branch.raw, raw[0x20..0x2C]);
    assert_eq!(branch.unconsumed_parameters, [0xA5, 0x5A, 0xC3]);
    assert_eq!(branch.probe.runtime_address, 0x800D_B8C0);
    assert_eq!(branch.probe.mask, 0x0002_0000);
    assert_eq!(branch.fallthrough_offset, 0x2C);
    assert_eq!(branch.branch_target_offset, 0x60);
    assert_eq!(
        branch.selection(0),
        QuestSeqFlagBranchSelection::Fallthrough
    );
    assert_eq!(branch.selected_offset(0), 0x2C);
    assert_eq!(
        branch.selection(0x0002_0000),
        QuestSeqFlagBranchSelection::Branch
    );
    assert_eq!(branch.selected_offset(0x0002_0000), 0x60);
}

#[test]
fn opa1의_기대값_1은_두_target선택을_뒤집는다() {
    let raw = fixture(1);
    let branch = parse_quest_seq_flag_branch(
        "TEST.SEQ",
        &raw,
        QuestSeqRuntimeBaseEvidence::StaticCommonLoader,
        0x20,
    )
    .unwrap()
    .expect("OPA1 record");

    assert_eq!(branch.selection(0), QuestSeqFlagBranchSelection::Branch);
    assert_eq!(
        branch.selection(0x0002_0000),
        QuestSeqFlagBranchSelection::Fallthrough
    );
}

#[test]
fn 다른_opcode와_잘린_record와_파일밖_target을_거부한다() {
    let mut raw = fixture(0);
    raw[0x20] = 0xA2;
    assert!(parse_quest_seq_flag_branch(
        "TEST.SEQ",
        &raw,
        QuestSeqRuntimeBaseEvidence::StaticCommonLoader,
        0x20,
    )
    .unwrap()
    .is_none());

    let truncated = [0xA1, 0, 0, 0];
    assert!(parse_quest_seq_flag_branch(
        "TEST.SEQ",
        &truncated,
        QuestSeqRuntimeBaseEvidence::StaticCommonLoader,
        0,
    )
    .is_err());

    let mut raw = fixture(0);
    raw[0x28..0x2C].copy_from_slice(&0x8000_1000u32.to_le_bytes());
    assert!(parse_quest_seq_flag_branch(
        "TEST.SEQ",
        &raw,
        QuestSeqRuntimeBaseEvidence::StaticCommonLoader,
        0x20,
    )
    .is_err());
}
