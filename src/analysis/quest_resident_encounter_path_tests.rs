use super::*;
use crate::analysis::{QuestResidentSeqPathTransition, QuestResidentSeqRuntimeRequirement};
use crate::coords::LogicalFile;
use crate::font::QUEST_CODE_TABLE_COUNT;

const LOAD: u32 = 0x8008_C590;
const LEN: usize = 0x80;

fn map() -> ExeMap {
    ExeMap {
        name: "TEST.EXE",
        load_addr: LOAD,
        text_len: LEN as u32,
        entry_pc: LOAD,
        file: LogicalFile::new(0, (LEN + 0x800) as u32),
    }
}

fn code_table() -> QuestCodeTable {
    let mut codes = (0..QUEST_CODE_TABLE_COUNT)
        .map(|index| u16::try_from(index).expect("fixture code").to_le_bytes())
        .collect::<Vec<_>>();
    codes[0] = [0x82, 0xA0];
    QuestCodeTable::parse(&codes.into_iter().flatten().collect::<Vec<_>>())
        .expect("fixture code table")
}

fn offset(runtime: u32) -> usize {
    usize::try_from(runtime - LOAD).expect("fixture offset")
}

fn source_frontier() -> QuestResidentSeqFrontier {
    QuestResidentSeqFrontier {
        root_runtime: LOAD,
        record_runtime: 0x8008_C5A4,
        opcode: 0x14,
        requirement: QuestResidentSeqRuntimeRequirement::GlobalStateWord,
        compare: Some(QuestResidentSeqCompareFrontier {
            operation: QuestSeqBaseCompareOperation::Equal,
            value_bytes: 4,
            left_source: QuestSeqBaseCompareOperandSource::GlobalState,
            left_parameter: Some(0),
            right_source: QuestSeqBaseCompareOperandSource::Inline,
            right_parameter: 0,
            branch_target_runtime: 0x8008_C5A0,
            fallthrough_runtime: 0x8008_C5B0,
            runtime_probe_address: Some(0x800D_B6C0),
            runtime_probe_length: Some(4),
        }),
        spawn_stream_runtime: None,
    }
}

fn fixture() -> Vec<u8> {
    let mut text = vec![0; LEN];
    let wait = offset(0x8008_C5B0);
    text[wait..wait + 4].copy_from_slice(&[0x01, 0, 0, 0]);
    text[wait + 4..wait + 16]
        .copy_from_slice(&[0x13, 0x81, 0xC6, 0, 0, 0, 0, 0, 0xB0, 0xC5, 0x08, 0x80]);

    let display = offset(0x8008_C5C0);
    for (index, opcode) in [0x02, 0x9A, 0xBE, 0x8C, 0x02].into_iter().enumerate() {
        text[display + index * 4] = opcode;
    }
    let op21 = display + 20;
    text[op21..op21 + 4].copy_from_slice(&[0x21, 0, 0, 0]);
    text[op21 + 4..op21 + 12].copy_from_slice(&[0x82, 0xA0, 0x03, 0xFF, 0, 0, 0, 0]);
    text[op21 + 12..op21 + 16].copy_from_slice(&[0x09, 0x34, 1, 0]);
    text
}

#[test]
fn 두_compare의_false_edge와_op21_표시경로를_분리해_보존한다() {
    let analysis =
        audit_quest_resident_encounter_path(&map(), &fixture(), &code_table(), &source_frontier())
            .unwrap();

    assert_eq!(analysis.wait_path.root_runtime, 0x8008_C5B0);
    assert_eq!(analysis.ready_compare.left_parameter, Some(0xC6));
    assert_eq!(analysis.ready_compare.branch_target_runtime, 0x8008_C5B0);
    assert_eq!(analysis.ready_compare.fallthrough_runtime, 0x8008_C5C0);
    assert_eq!(analysis.display_path.root_runtime, 0x8008_C5C0);
    assert_eq!(
        analysis
            .display_path
            .nodes
            .iter()
            .map(|node| (node.opcode, node.transition))
            .collect::<Vec<_>>(),
        [
            (0x02, QuestResidentSeqPathTransition::DirectAdvance),
            (0x9A, QuestResidentSeqPathTransition::DirectAdvance),
            (0xBE, QuestResidentSeqPathTransition::DirectAdvance),
            (0x8C, QuestResidentSeqPathTransition::DirectAdvance),
            (0x02, QuestResidentSeqPathTransition::DirectAdvance),
            (0x21, QuestResidentSeqPathTransition::TextDeactivateAndAlign),
            (0x09, QuestResidentSeqPathTransition::RuntimeFrontier),
        ]
    );
}

#[test]
fn op13_true_edge가_wait_loop가_아니면_표시경로를_부분채택하지_않는다() {
    let mut text = fixture();
    let branch = offset(0x8008_C5B4) + 8;
    text[branch..branch + 4].copy_from_slice(&0x8008_C5C0u32.to_le_bytes());

    let failure =
        audit_quest_resident_encounter_path(&map(), &text, &code_table(), &source_frontier())
            .unwrap_err();

    assert!(failure.to_string().contains("wait loop로 돌아가지 않음"));
}
