use super::*;
use crate::analysis::{
    QuestResidentSeqCompareFrontier, QuestResidentSeqFrontierRequirement, QuestResidentSeqPath,
    QuestResidentSeqPathNode, QuestResidentSeqPathStatus, QuestResidentSeqPathTransition,
};
use crate::coords::LogicalFile;
use crate::disasm::{QuestSeqBaseCompareOperandSource, QuestSeqBaseCompareOperation};
use crate::font::QUEST_CODE_TABLE_COUNT;
use crate::script::QuestTextTerminal;

const LOAD: u32 = 0x8001_0000;
const LEN: usize = 0x40;

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

fn fixture(mode: u8) -> Vec<u8> {
    let mut text = vec![0; LEN];
    text[..4].copy_from_slice(&[0x21, 0, 0, 0]);
    text[4..12].copy_from_slice(&[0x82, 0xA0, 0x1E, 0xFF, mode, 0, 0x82, 0xA0]);
    text[12..20].copy_from_slice(&[0x03, 0xFF, 0, 0, 0, 0, 0, 0]);
    text[20..24].copy_from_slice(&[0x09, 0x34, 1, 0]);
    text
}

fn compare() -> QuestResidentSeqCompareFrontier {
    QuestResidentSeqCompareFrontier {
        operation: QuestSeqBaseCompareOperation::Equal,
        value_bytes: 2,
        left_source: QuestSeqBaseCompareOperandSource::ObjectField,
        left_parameter: Some(0),
        right_source: QuestSeqBaseCompareOperandSource::Inline,
        right_parameter: 0,
        branch_target_runtime: LOAD,
        fallthrough_runtime: LOAD,
        runtime_probe_address: None,
        runtime_probe_length: None,
    }
}

fn path() -> QuestResidentEncounterPathAnalysis {
    let empty_path = QuestResidentSeqPath {
        root_runtime: LOAD,
        install_sites: 0,
        nodes: Vec::new(),
        status: QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier,
        frontier_requirement: Some(QuestResidentSeqFrontierRequirement::CompareState),
    };
    let display_path = QuestResidentSeqPath {
        root_runtime: LOAD,
        install_sites: 0,
        nodes: vec![
            QuestResidentSeqPathNode {
                runtime: LOAD,
                opcode: 0x21,
                transition: QuestResidentSeqPathTransition::TextDeactivateAndAlign,
                next_runtime: Some(LOAD + 20),
                side_stream_runtime: None,
                child_stream_runtime: None,
                text_terminal: Some(QuestTextTerminal::DeactivateAndAlign),
                text_glyphs: 2,
                text_controls: 2,
            },
            QuestResidentSeqPathNode {
                runtime: LOAD + 20,
                opcode: 0x09,
                transition: QuestResidentSeqPathTransition::RuntimeFrontier,
                next_runtime: None,
                side_stream_runtime: None,
                child_stream_runtime: None,
                text_terminal: None,
                text_glyphs: 0,
                text_controls: 0,
            },
        ],
        status: QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier,
        frontier_requirement: Some(QuestResidentSeqFrontierRequirement::ObjectBranchSelectorState),
    };
    QuestResidentEncounterPathAnalysis {
        literal_root_runtime: LOAD,
        first_compare_runtime: LOAD,
        first_compare: compare(),
        wait_path: empty_path,
        ready_compare: compare(),
        display_path,
    }
}

fn battle_names() -> QuestBattleNameTableAnalysis {
    QuestBattleNameTableAnalysis {
        selector_runtime: 0,
        selector_field_offset: 0,
        pointer_table_runtime: 0,
        pointer_table_end_runtime: 0,
        fixed_text_state_index: 0x38,
        fixed_text_state_runtime: QUEST_FIXED_TEXT_POINTER_RUNTIME,
        entries: Vec::new(),
        unique_sources: 0,
        glyph_occurrences: 0,
        slot_bytes: 0,
    }
}

#[test]
fn ff1e_mode0을_동적이름자리로_두고_앞뒤_run을_모두_보존한다() {
    let analysis = audit_quest_resident_encounter_text(
        &map(),
        &fixture(0),
        &code_table(),
        &path(),
        &battle_names(),
    )
    .unwrap();

    assert_eq!(analysis.dynamic_name_controls, 1);
    assert_eq!(analysis.unit.protected_source, "ああ");
    assert_eq!(
        analysis
            .unit
            .runs
            .iter()
            .map(|run| run.source.as_str())
            .collect::<Vec<_>>(),
        ["あ", "あ"]
    );
    assert_eq!(analysis.unit.protected_raw, fixture(0)[..20]);
}

#[test]
fn ff1e가_다른_mode면_고정이름으로_오인하지_않는다() {
    let failure = audit_quest_resident_encounter_text(
        &map(),
        &fixture(1),
        &code_table(),
        &path(),
        &battle_names(),
    )
    .unwrap_err();

    assert!(failure.to_string().contains("FF1E mode-0"));
}
