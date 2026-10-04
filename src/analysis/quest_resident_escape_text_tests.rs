use std::collections::BTreeSet;

use super::*;
use crate::coords::LogicalFile;
use crate::disasm::{QuestSeqBaseCompareOperandSource, QuestSeqBaseCompareOperation};
use crate::font::QUEST_CODE_TABLE_COUNT;
use crate::script::QuestTextTerminal;

const LOAD: u32 = QUEST_RESIDENT_ESCAPE_SELECTOR_RUNTIME;
const LEN: usize = 0x1B0;
const ATTEMPT_RAW: [u8; 44] = [
    0x21, 0x00, 0x00, 0x00, 0x05, 0xFF, 0x81, 0x40, 0x81, 0x40, 0x83, 0x41, 0x83, 0x8B, 0x83, 0x8B,
    0x82, 0xCD, 0x93, 0xA6, 0x82, 0xB0, 0x8F, 0x6F, 0x82, 0xB5, 0x82, 0xBD, 0x81, 0x49, 0x81, 0x49,
    0x04, 0xFF, 0x0F, 0xFF, 0x01, 0x00, 0x03, 0xFF, 0x00, 0x00, 0x00, 0x00,
];
const FAILURE_RAW: [u8; 48] = [
    0x21, 0x00, 0x00, 0x00, 0x81, 0x40, 0x81, 0x40, 0x82, 0xB5, 0x82, 0xA9, 0x82, 0xB5, 0x81, 0x41,
    0x93, 0xA6, 0x82, 0xB0, 0x82, 0xE7, 0x82, 0xEA, 0x82, 0xC8, 0x82, 0xA9, 0x82, 0xC1, 0x82, 0xBD,
    0x81, 0x49, 0x81, 0x49, 0x04, 0xFF, 0x0F, 0xFF, 0x01, 0x00, 0x03, 0xFF, 0x00, 0x00, 0x00, 0x00,
];

fn map() -> ExeMap {
    ExeMap {
        name: "QUEST.EXE",
        load_addr: LOAD,
        text_len: LEN as u32,
        entry_pc: LOAD,
        file: LogicalFile::new(0, (LEN + 0x800) as u32),
    }
}

fn offset(runtime: u32) -> usize {
    usize::try_from(runtime - LOAD).expect("fixture offset")
}

fn code_table() -> QuestCodeTable {
    let glyph_codes = [
        [0x81, 0x40],
        [0x83, 0x41],
        [0x83, 0x8B],
        [0x82, 0xCD],
        [0x93, 0xA6],
        [0x82, 0xB0],
        [0x8F, 0x6F],
        [0x82, 0xB5],
        [0x82, 0xBD],
        [0x81, 0x49],
        [0x82, 0xA9],
        [0x81, 0x41],
        [0x82, 0xE7],
        [0x82, 0xEA],
        [0x82, 0xC8],
        [0x82, 0xC1],
    ];
    let mut seen = glyph_codes.into_iter().collect::<BTreeSet<_>>();
    let mut codes = seen.iter().copied().collect::<Vec<_>>();
    for candidate in 0u16..=u16::MAX {
        if codes.len() == QUEST_CODE_TABLE_COUNT {
            break;
        }
        let code = candidate.to_le_bytes();
        if seen.insert(code) {
            codes.push(code);
        }
    }
    QuestCodeTable::parse(&codes.into_iter().flatten().collect::<Vec<_>>())
        .expect("fixture code table")
}

fn fixture() -> Vec<u8> {
    let mut text = vec![0; LEN];
    text[..4].copy_from_slice(&ESCAPE_SELECTOR_RAW);
    for (index, target) in ESCAPE_SELECTOR_TARGETS.into_iter().enumerate() {
        let start = 4 + index * 4;
        text[start..start + 4].copy_from_slice(&target.to_le_bytes());
    }

    let attempt = offset(QUEST_RESIDENT_ESCAPE_ATTEMPT_RUNTIME);
    text[attempt..attempt + ATTEMPT_RAW.len()].copy_from_slice(&ATTEMPT_RAW);
    let post_attempt = offset(0x8008_C688);
    text[post_attempt..post_attempt + 4].copy_from_slice(&[0xC8, 0, 0, 0]);
    let compare = offset(QUEST_RESIDENT_ESCAPE_OUTCOME_COMPARE_RUNTIME);
    text[compare..compare + 12].copy_from_slice(&[
        0x14,
        0x81,
        ESCAPE_SELECTOR_FIELD_OFFSET,
        0x00,
        0x01,
        0x00,
        0x00,
        0x00,
        0xD0,
        0xC6,
        0x08,
        0x80,
    ]);
    let failure = offset(QUEST_RESIDENT_ESCAPE_FAILURE_RUNTIME);
    text[failure..failure + FAILURE_RAW.len()].copy_from_slice(&FAILURE_RAW);
    text
}

fn compare_frontier_fixture() -> QuestResidentSeqCompareFrontier {
    QuestResidentSeqCompareFrontier {
        operation: QuestSeqBaseCompareOperation::Equal,
        value_bytes: 4,
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

fn encounter() -> QuestResidentEncounterPathAnalysis {
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
        nodes: vec![QuestResidentSeqPathNode {
            runtime: QUEST_RESIDENT_ESCAPE_SELECTOR_RUNTIME,
            opcode: 0x09,
            transition: QuestResidentSeqPathTransition::RuntimeFrontier,
            next_runtime: None,
            side_stream_runtime: None,
            child_stream_runtime: None,
            text_terminal: None,
            text_glyphs: 0,
            text_controls: 0,
        }],
        status: QuestResidentSeqPathStatus::StoppedAtRuntimeFrontier,
        frontier_requirement: Some(QuestResidentSeqFrontierRequirement::ObjectBranchSelectorState),
    };
    QuestResidentEncounterPathAnalysis {
        literal_root_runtime: LOAD,
        first_compare_runtime: LOAD,
        first_compare: compare_frontier_fixture(),
        wait_path: empty_path,
        ready_compare: compare_frontier_fixture(),
        display_path,
    }
}

#[test]
fn op09의_도주_target과_op14_false_edge에서_두_원문을_결속한다() {
    let analysis =
        verify_box_quest_resident_escape_text(&map(), &fixture(), &code_table(), &encounter())
            .unwrap();

    assert_eq!(
        analysis
            .units
            .units
            .iter()
            .map(|unit| (unit.role, unit.protected_source.as_str()))
            .collect::<Vec<_>>(),
        [
            (
                QuestResidentTextRole::EscapeAttempt,
                ESCAPE_ATTEMPT_RUN_SOURCE,
            ),
            (
                QuestResidentTextRole::EscapeFailure,
                ESCAPE_FAILURE_RUN_SOURCE,
            ),
        ]
    );
    assert_eq!(
        analysis.outcome_compare.fallthrough_runtime,
        QUEST_RESIDENT_ESCAPE_FAILURE_RUNTIME
    );
    assert_eq!(
        analysis
            .attempt_path
            .nodes
            .first()
            .and_then(|node| node.text_terminal),
        Some(QuestTextTerminal::DeactivateAndAlign)
    );
}

#[test]
fn selector와_보호원문_drift를_부분결과없이_거부한다() {
    let mut selector_drift = fixture();
    selector_drift[1] ^= 1;
    let selector_error =
        verify_box_quest_resident_escape_text(&map(), &selector_drift, &code_table(), &encounter())
            .unwrap_err();
    assert!(selector_error.to_string().contains("source 경계 drift"));

    let mut source_drift = fixture();
    source_drift[offset(QUEST_RESIDENT_ESCAPE_FAILURE_RUNTIME) + 5] ^= 1;
    let source_error =
        verify_box_quest_resident_escape_text(&map(), &source_drift, &code_table(), &encounter())
            .unwrap_err();
    assert!(
        source_error.to_string().contains("코드표에 없음")
            || source_error.to_string().contains("보호 source drift")
    );
}
