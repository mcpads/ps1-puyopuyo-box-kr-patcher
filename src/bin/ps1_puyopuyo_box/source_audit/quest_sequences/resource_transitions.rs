//! QUEST SEQ 등록 원장과 정적 frontier의 resource 전이를 결속한다.

use ps1_puyopuyo_box::analysis::quest_seq_resource_transitions::{
    verify_box_quest_seq_resource_transitions, QuestSeqResourceCatalogLayout,
};
use ps1_puyopuyo_box::analysis::quest_seq_text_frontiers::QuestSeqTextFrontierAnalysis;
use ps1_puyopuyo_box::disasm::quest_seq_registration::QuestSeqRegistrationSummary;

pub(super) fn audit_quest_resource_transitions(
    sequences: &[(String, Vec<u8>)],
    registration: &QuestSeqRegistrationSummary,
    text_frontiers: &QuestSeqTextFrontierAnalysis,
) -> Result<(), String> {
    let transitions = verify_box_quest_seq_resource_transitions(
        sequences.iter().map(|(name, _)| name.as_str()),
        QuestSeqResourceCatalogLayout {
            descriptor_root_runtime: registration.descriptor_root_runtime,
            descriptor_stride: registration.descriptor_stride,
            prefix_descriptor_rows: registration.prefix_descriptor_rows,
        },
        text_frontiers,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ resource transitions: catalog {}, static frontiers {} / \
         path occurrences {}, direct unique targets {}, runtime-default {}",
        transitions.resource_catalog_entries,
        transitions.transitions.len(),
        transitions.path_occurrences,
        transitions.unique_target_resources,
        transitions.runtime_default_transitions,
    );
    Ok(())
}
