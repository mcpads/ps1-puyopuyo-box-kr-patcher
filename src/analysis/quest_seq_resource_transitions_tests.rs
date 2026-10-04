use super::*;
use crate::analysis::{
    QuestSeqTextFrontier, QuestSeqTextFrontierParameter, QuestSeqTextFrontierRequirement,
    QuestSeqTextPathStatus,
};
use crate::script::QuestSeqTextRootBaseEvidence;

fn layout() -> QuestSeqResourceCatalogLayout {
    QuestSeqResourceCatalogLayout {
        descriptor_root_runtime: 0x8008_F9D0,
        descriptor_stride: 36,
        prefix_descriptor_rows: 1,
    }
}

fn frontier(resource_id: u16) -> QuestSeqTextFrontier {
    QuestSeqTextFrontier {
        sequence: "SOURCE.SEQ".to_string(),
        sequence_sha256: [0xAB; 32],
        terminal_offset: 0x20,
        terminal_runtime: 0x8019_9A90,
        base_evidence: QuestSeqTextRootBaseEvidence::StaticCommonLoader,
        opcode: 0x8A,
        status: QuestSeqTextPathStatus::StoppedWithoutUniqueDirectWidth,
        terminal_raw: vec![0x8A, 0, resource_id as u8, (resource_id >> 8) as u8],
        path_occurrences: 2,
        seed_offsets: vec![0x10],
        source_opcodes: vec![0xA1],
        requirement: QuestSeqTextFrontierRequirement::ResourceTransitionState,
        parameter: Some(QuestSeqTextFrontierParameter::ResourceId { id: resource_id }),
        compare: None,
        runtime_probe_address: None,
        runtime_probe_length: None,
    }
}

#[test]
fn archive_descriptor_다음부터_vdd_entry를_결속한다() {
    let analysis = QuestSeqTextFrontierAnalysis {
        frontiers: vec![frontier(2)],
        path_occurrences: 2,
    };
    let result =
        audit_quest_seq_resource_transitions(["FIRST.SEQ", "SECOND.SEQ"], layout(), &analysis)
            .unwrap();

    assert_eq!(result.resource_catalog_entries, 2);
    assert_eq!(result.path_occurrences, 2);
    assert_eq!(result.unique_target_resources, 1);
    assert_eq!(result.runtime_default_transitions, 0);
    assert_eq!(result.transitions[0].vdd_entry_index, Some(1));
    assert_eq!(
        result.transitions[0].target_sequence.as_deref(),
        Some("SECOND.SEQ")
    );
    assert_eq!(result.transitions[0].descriptor_runtime, Some(0x8008_FA18));
    assert!(render_quest_seq_resource_transitions_tsv(&result)
        .contains("static-path-frontier\tnot-observed"));
}

#[test]
fn resource_id_0은_runtime_default로_남긴다() {
    let analysis = QuestSeqTextFrontierAnalysis {
        frontiers: vec![frontier(0)],
        path_occurrences: 2,
    };
    let result =
        audit_quest_seq_resource_transitions(["FIRST.SEQ", "SECOND.SEQ"], layout(), &analysis)
            .unwrap();
    assert_eq!(result.runtime_default_transitions, 1);
    assert_eq!(result.unique_target_resources, 0);
    assert_eq!(result.transitions[0].descriptor_runtime, None);
    assert_eq!(result.transitions[0].target_sequence, None);
    assert!(render_quest_seq_resource_transitions_tsv(&result)
        .contains("runtime-default-state-required"));
}

#[test]
fn catalog밖_resource_id를_거부한다() {
    let analysis = QuestSeqTextFrontierAnalysis {
        frontiers: vec![frontier(3)],
        path_occurrences: 2,
    };
    assert!(
        audit_quest_seq_resource_transitions(["FIRST.SEQ", "SECOND.SEQ"], layout(), &analysis,)
            .is_err()
    );
}

#[test]
fn catalog의_빈이름과_중복을_거부한다() {
    let analysis = QuestSeqTextFrontierAnalysis {
        frontiers: vec![frontier(1)],
        path_occurrences: 2,
    };
    assert!(audit_quest_seq_resource_transitions([""], layout(), &analysis).is_err());
    assert!(
        audit_quest_seq_resource_transitions(["SAME.SEQ", "same.seq"], layout(), &analysis,)
            .is_err()
    );
}
