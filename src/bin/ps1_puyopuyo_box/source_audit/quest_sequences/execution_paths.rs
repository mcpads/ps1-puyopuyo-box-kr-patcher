//! QUEST OP21 정적 경로와 상주 SEQ 실행 경계를 검증한다.

use std::collections::BTreeSet;

use ps1_puyopuyo_box::analysis::quest_resident_encounter_path::verify_box_quest_resident_encounter_path;
use ps1_puyopuyo_box::analysis::quest_resident_encounter_text::{
    verify_box_quest_resident_encounter_text, QuestResidentEncounterTextAnalysis,
};
use ps1_puyopuyo_box::analysis::quest_resident_escape_text::verify_box_quest_resident_escape_text;
use ps1_puyopuyo_box::analysis::quest_resident_seq_child_graph::verify_box_quest_resident_seq_child_graph;
use ps1_puyopuyo_box::analysis::quest_resident_seq_frontiers::verify_box_quest_resident_seq_frontiers;
use ps1_puyopuyo_box::analysis::quest_resident_seq_paths::verify_box_quest_resident_seq_paths;
use ps1_puyopuyo_box::analysis::quest_resident_seq_spawn_path::verify_box_quest_resident_seq_spawn_path;
use ps1_puyopuyo_box::analysis::quest_resident_seq_transition::verify_box_quest_resident_seq_transition;
use ps1_puyopuyo_box::analysis::quest_resident_text_units::{
    combine_quest_resident_text_units, verify_box_quest_resident_text_units,
    QuestResidentTextUnitAnalysis,
};
use ps1_puyopuyo_box::analysis::quest_seq_flag_branches::verify_box_quest_seq_flag_branches;
use ps1_puyopuyo_box::analysis::quest_seq_resource_transitions::QuestSeqResourceCatalogLayout;
use ps1_puyopuyo_box::analysis::quest_seq_text_frontiers::{
    verify_box_quest_seq_text_frontiers, QuestSeqTextFrontierAnalysis,
};
use ps1_puyopuyo_box::analysis::quest_seq_text_paths::{
    verify_box_quest_seq_text_paths, QuestSeqTextPathAnalysis, QuestSeqTextPathTransition,
};
use ps1_puyopuyo_box::analysis::{
    verify_box_quest_battle_name_table, QuestBattleNameTableAnalysis,
};
use ps1_puyopuyo_box::coords::ExeMap;
use ps1_puyopuyo_box::disasm::quest_seq_base_compare::verify_box_quest_seq_base_compares;
use ps1_puyopuyo_box::disasm::{
    verify_box_quest_escape_outcome, QuestSeqChildActivationSummary,
    QuestSeqChildConfigurationSummary, QuestSeqRegistrationSummary,
};
use ps1_puyopuyo_box::font::quest::QuestCodeTable;
use ps1_puyopuyo_box::pipeline::{
    audit_quest_battle_name_compositions, audit_quest_resident_encounter_text_composition,
};
use ps1_puyopuyo_box::script::quest_seq_inventory::QuestSeqAddressAnalysis;
use ps1_puyopuyo_box::script::quest_seq_text_extraction::QuestSeqTextExtractionAnalysis;
use ps1_puyopuyo_box::script::quest_seq_text_roots::verify_box_quest_seq_text_roots;
use ps1_puyopuyo_box::script::quest_seq_text_runtime::verify_box_quest_seq_text_runtime_evidence;

use super::suke_confrontation::{
    audit_suke_confrontation_route, SukeConfrontationRouteAuditInputs,
};

pub(super) struct QuestExecutionPathAuditInputs<'a> {
    pub executable_map: &'a ExeMap,
    pub executable_text: &'a [u8],
    pub sequences: &'a [(String, Vec<u8>)],
    pub seq_addresses: &'a QuestSeqAddressAnalysis,
    pub text_extraction: &'a QuestSeqTextExtractionAnalysis,
    pub code_table: &'a QuestCodeTable,
    pub registration: &'a QuestSeqRegistrationSummary,
    pub child_activation: &'a QuestSeqChildActivationSummary,
    pub child_configuration: &'a QuestSeqChildConfigurationSummary,
}

pub(super) struct QuestExecutionPathAudit {
    pub text_paths: QuestSeqTextPathAnalysis,
    pub text_frontiers: QuestSeqTextFrontierAnalysis,
    pub resident_text_units: QuestResidentTextUnitAnalysis,
    pub battle_names: QuestBattleNameTableAnalysis,
    pub resident_encounter_text: QuestResidentEncounterTextAnalysis,
    pub registration: ps1_puyopuyo_box::disasm::QuestSeqRegistrationSummary,
}

pub(super) fn audit_quest_execution_paths(
    inputs: QuestExecutionPathAuditInputs<'_>,
) -> Result<QuestExecutionPathAudit, String> {
    let QuestExecutionPathAuditInputs {
        executable_map: quest_map,
        executable_text: quest_text,
        sequences: quest_seq_sources,
        seq_addresses: quest_seq_addresses,
        text_extraction: quest_seq_text_extraction,
        code_table,
        registration: quest_seq_registration,
        child_activation,
        child_configuration,
    } = inputs;
    let quest_seq_text_runtime =
        verify_box_quest_seq_text_runtime_evidence(quest_seq_text_extraction)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ runtime text evidence: streams {}, OP21 hits {}, visible-only {}",
        quest_seq_text_runtime.entries.len(),
        quest_seq_text_runtime.op21_hits,
        quest_seq_text_runtime.visible_only,
    );
    let quest_seq_text_roots = verify_box_quest_seq_text_roots(
        quest_seq_sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        code_table,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ OP21 direct roots: markers {}, consumer-backed {} / edges {}, \
             range-word-only {}, no direct word {}, rooted runs {} / code units {}",
        quest_seq_text_roots.markers.len(),
        quest_seq_text_roots
            .markers
            .iter()
            .filter(|marker| !marker.consumer_edges.is_empty())
            .count(),
        quest_seq_text_roots
            .markers
            .iter()
            .map(|marker| marker.consumer_edges.len())
            .sum::<usize>(),
        quest_seq_text_roots
            .summaries
            .iter()
            .map(|summary| summary.range_only_markers)
            .sum::<usize>(),
        quest_seq_text_roots
            .summaries
            .iter()
            .map(|summary| summary.no_direct_word_markers)
            .sum::<usize>(),
        quest_seq_text_roots.rooted_runs,
        quest_seq_text_roots.rooted_code_units,
    );
    let base_compares = verify_box_quest_seq_base_compares(quest_map, quest_text)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ base compares: {} opcodes / {} operations, \
             halfword {} / word {}, state base 0x{:08X}",
        base_compares.opcodes,
        base_compares.operations,
        base_compares.halfword_opcodes,
        base_compares.word_opcodes,
        base_compares.state_base_runtime,
    );
    let quest_seq_text_paths = verify_box_quest_seq_text_paths(
        quest_seq_sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        code_table,
    )
    .map_err(|error| error.to_string())?;
    let record_mode_advances =
        quest_seq_text_paths.transition_count(QuestSeqTextPathTransition::RecordModeAdvance);
    let subdispatch_advances =
        quest_seq_text_paths.transition_count(QuestSeqTextPathTransition::SubdispatchAdvance);
    let fallback_advances =
        quest_seq_text_paths.transition_count(QuestSeqTextPathTransition::FallbackAdvance);
    println!(
        "PASS QUEST SEQ OP21 static paths: seeds {}, direct {}, sequential {}, \
             OP03 branch-reached {}, unique edges {} / occurrences {}, stopped {}, path nodes {}, \
             record mode {} / subdispatch {} / fallback {}, rooted runs {} / code units {}",
        quest_seq_text_paths.paths.len(),
        quest_seq_text_paths
            .summaries
            .iter()
            .map(|summary| summary.direct_markers)
            .sum::<usize>(),
        quest_seq_text_paths
            .summaries
            .iter()
            .map(|summary| summary.sequential_markers)
            .sum::<usize>(),
        quest_seq_text_paths
            .summaries
            .iter()
            .map(|summary| summary.direct_branch_markers)
            .sum::<usize>(),
        quest_seq_text_paths.unique_direct_branch_edges,
        quest_seq_text_paths.direct_branch_edge_occurrences,
        quest_seq_text_paths
            .summaries
            .iter()
            .map(|summary| summary.stopped)
            .sum::<usize>(),
        quest_seq_text_paths
            .summaries
            .iter()
            .map(|summary| summary.path_node_occurrences)
            .sum::<usize>(),
        record_mode_advances,
        subdispatch_advances,
        fallback_advances,
        quest_seq_text_paths.rooted_runs,
        quest_seq_text_paths.rooted_code_units,
    );
    let quest_seq_text_frontiers = verify_box_quest_seq_text_frontiers(
        quest_seq_sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        &quest_seq_text_paths,
    )
    .map_err(|error| error.to_string())?;
    let quest_seq_flag_branches = verify_box_quest_seq_flag_branches(
        quest_seq_sources
            .iter()
            .map(|(name, raw)| (name.as_str(), raw.as_slice())),
        quest_seq_addresses,
    )
    .map_err(|error| error.to_string())?;
    let suke_confrontation = audit_suke_confrontation_route(SukeConfrontationRouteAuditInputs {
        sequences: quest_seq_sources,
        text_paths: &quest_seq_text_paths,
        flag_branches: &quest_seq_flag_branches,
        text_extraction: quest_seq_text_extraction,
        child_activation,
        child_configuration,
    })?;
    let unique_flag_probe_addresses = quest_seq_flag_branches
        .branches
        .iter()
        .map(|branch| branch.probe.runtime_address)
        .collect::<BTreeSet<_>>();
    println!(
        "PASS QUEST SEQ OPA1 flag branches: records {}, unique probes {}, \
         SUKE OPA1 0x{:04X} -> OP82 0x{:04X} -> child 0x{:04X} -> OPA1 0x{:04X}, \
         probe 0x{:08X}/mask 0x{:08X}, clear -> 0x{:04X}, set -> 0x{:04X}",
        quest_seq_flag_branches.branches.len(),
        unique_flag_probe_addresses.len(),
        suke_confrontation.producer_flag_record_offset,
        suke_confrontation.child_record_offset,
        suke_confrontation.child_stream_offset,
        suke_confrontation.flag_record_offset,
        suke_confrontation.probe_runtime,
        suke_confrontation.probe_mask,
        suke_confrontation.clear_target_offset,
        suke_confrontation.set_target_offset,
    );
    println!(
        "PASS QUEST SUKE child contact: OP83 0x{:04X}..0x{:04X}, \
         anchor ({}, {}), layer {}, rect ({}, {})..({}, {}), velocity ({}, {}), \
         activation callback 0x{:08X}",
        suke_confrontation.child_configuration_offset,
        suke_confrontation.child_configuration_end_offset,
        suke_confrontation.child_contact.anchor_x_pixels,
        suke_confrontation.child_contact.anchor_y_pixels,
        suke_confrontation.child_contact.layer,
        suke_confrontation.child_contact.contact_left_pixels,
        suke_confrontation.child_contact.contact_top_pixels,
        suke_confrontation.child_contact.contact_right_pixels,
        suke_confrontation.child_contact.contact_bottom_pixels,
        suke_confrontation.child_contact.velocity_x_fixed,
        suke_confrontation.child_contact.velocity_y_fixed,
        suke_confrontation.update_callback_runtime,
    );
    let quest_seq_op0d_frontiers = quest_seq_text_frontiers
        .frontiers
        .iter()
        .filter(|frontier| frontier.runtime_probe_address.is_some())
        .count();
    let quest_seq_probe_addresses = quest_seq_text_frontiers
        .frontiers
        .iter()
        .filter_map(|frontier| frontier.runtime_probe_address)
        .collect::<BTreeSet<_>>();
    println!(
            "PASS QUEST SEQ text frontiers: unique {}, path occurrences {}, \
             OP0D frontiers {}, unique probe addresses {}, compare records {}, runtime requirements {}",
            quest_seq_text_frontiers.frontiers.len(),
            quest_seq_text_frontiers.path_occurrences,
            quest_seq_op0d_frontiers,
            quest_seq_probe_addresses.len(),
            quest_seq_text_frontiers
                .frontiers
                .iter()
                .filter(|frontier| frontier.compare.is_some())
                .count(),
            quest_seq_text_frontiers
                .frontiers
                .iter()
                .map(|frontier| frontier.requirement.label())
                .collect::<BTreeSet<_>>()
                .len(),
        );
    let quest_resident_seq_transition = verify_box_quest_resident_seq_transition(
        quest_map,
        quest_text,
        quest_seq_sources.iter().map(|(name, _)| name.as_str()),
        QuestSeqResourceCatalogLayout {
            descriptor_root_runtime: quest_seq_registration.descriptor_root_runtime,
            descriptor_stride: quest_seq_registration.descriptor_stride,
            prefix_descriptor_rows: quest_seq_registration.prefix_descriptor_rows,
        },
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST resident SEQ transition source: menu OP09 0x{:08X} field +0x{:X} / \
             {} choices, new-game index {} -> 0x{:08X}; OP8A 0x{:08X} / text +0x{:X}, \
             ID {} -> descriptor 0x{:08X} / VDD[{}] {}",
        quest_resident_seq_transition.menu_branch_runtime,
        quest_resident_seq_transition.menu_selection_field_offset,
        quest_resident_seq_transition.menu_selection_count,
        quest_resident_seq_transition.new_game_selection_index,
        quest_resident_seq_transition.new_game_stream_runtime,
        quest_resident_seq_transition.record_runtime,
        quest_resident_seq_transition.record_text_offset,
        quest_resident_seq_transition.encoded_resource_id,
        quest_resident_seq_transition.descriptor_runtime,
        quest_resident_seq_transition.vdd_entry_index,
        quest_resident_seq_transition.target_sequence,
    );
    let quest_resident_seq_paths =
        verify_box_quest_resident_seq_paths(quest_map, quest_text, code_table)
            .map_err(|error| error.to_string())?;
    let resident_frontiers = quest_resident_seq_paths
        .paths
        .iter()
        .map(|path| {
            let terminal = path.nodes.last().expect("resident path에는 node가 있음");
            let opcodes = path
                .nodes
                .iter()
                .map(|node| format!("{:02X}", node.opcode))
                .collect::<Vec<_>>()
                .join(">");
            format!(
                "0x{:08X}->0x{:08X}/OP{:02X}/{}/[{}]",
                path.root_runtime,
                terminal.runtime,
                terminal.opcode,
                path.frontier_requirement
                    .map_or("released", |requirement| requirement.label()),
                opcodes,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let resident_child_edges = quest_resident_seq_paths
        .paths
        .iter()
        .flat_map(|path| &path.nodes)
        .filter_map(|node| {
            node.child_stream_runtime.map(|child| {
                format!(
                    "0x{:08X}->parent=0x{:08X}/child=0x{child:08X}",
                    node.runtime,
                    node.next_runtime.expect("검증된 OP1C parent next"),
                )
            })
        })
        .collect::<Vec<_>>()
        .join(",");
    println!(
            "PASS QUEST resident SEQ literal paths: roots {}, install sites {}, nodes {}, \
             OP21 {}, text glyphs/controls {}/{}, released {}, runtime frontiers {}, endpoints [{}], \
             OP1C edges [{}]",
            quest_resident_seq_paths.paths.len(),
            quest_resident_seq_paths.install_sites,
            quest_resident_seq_paths.nodes,
            quest_resident_seq_paths.op21_records,
            quest_resident_seq_paths.text_glyphs,
            quest_resident_seq_paths.text_controls,
            quest_resident_seq_paths.released_paths,
            quest_resident_seq_paths.runtime_frontiers,
            resident_frontiers,
            resident_child_edges,
        );
    let quest_resident_seq_child_graph = verify_box_quest_resident_seq_child_graph(
        quest_map,
        quest_text,
        code_table,
        &quest_resident_seq_paths,
    )
    .map_err(|error| error.to_string())?;
    let resident_child_paths = quest_resident_seq_child_graph
        .paths
        .iter()
        .map(|path| {
            let terminal = path
                .nodes
                .last()
                .expect("resident child path에는 node가 있음");
            let opcodes = path
                .nodes
                .iter()
                .map(|node| format!("{:02X}", node.opcode))
                .collect::<Vec<_>>()
                .join(">");
            format!(
                "0x{:08X}->0x{:08X}/OP{:02X}/{:?}/[{}]",
                path.root_runtime, terminal.runtime, terminal.opcode, path.status, opcodes,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "PASS QUEST resident OP1C child graph: edges {}, child paths {}, OP21 {}, \
             released {}, runtime frontiers {}, paths [{}]",
        quest_resident_seq_child_graph.edges.len(),
        quest_resident_seq_child_graph.paths.len(),
        quest_resident_seq_child_graph.op21_records,
        quest_resident_seq_child_graph.released_paths,
        quest_resident_seq_child_graph.runtime_frontiers,
        resident_child_paths,
    );
    let quest_resident_seq_frontiers =
        verify_box_quest_resident_seq_frontiers(quest_map, quest_text, &quest_resident_seq_paths)
            .map_err(|error| error.to_string())?;
    let resident_compare_targets = quest_resident_seq_frontiers
        .frontiers
        .iter()
        .filter_map(|frontier| {
            frontier.compare.map(|compare| {
                format!(
                    "0x{:08X}:probe=0x{:08X}/{}-0/true=0x{:08X}/false=0x{:08X}",
                    frontier.record_runtime,
                    compare.runtime_probe_address.expect("global-state probe"),
                    compare.operation.label(),
                    compare.branch_target_runtime,
                    compare.fallthrough_runtime,
                )
            })
        })
        .collect::<Vec<_>>()
        .join(",");
    let resident_spawn_targets = quest_resident_seq_frontiers
        .frontiers
        .iter()
        .filter_map(|frontier| {
            frontier
                .spawn_stream_runtime
                .map(|target| format!("0x{:08X}->0x{target:08X}", frontier.record_runtime))
        })
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "PASS QUEST resident SEQ runtime frontiers: rows {}, compare {}, spawn {}, \
             unique probes/streams {}/{}, compares [{}], spawns [{}]",
        quest_resident_seq_frontiers.frontiers.len(),
        quest_resident_seq_frontiers.compare_frontiers,
        quest_resident_seq_frontiers.spawn_frontiers,
        quest_resident_seq_frontiers.unique_probe_addresses,
        quest_resident_seq_frontiers.unique_spawn_streams,
        resident_compare_targets,
        resident_spawn_targets,
    );
    let quest_resident_encounter_path = verify_box_quest_resident_encounter_path(
        quest_map,
        quest_text,
        code_table,
        &quest_resident_seq_frontiers,
    )
    .map_err(|error| error.to_string())?;
    let encounter_wait_opcodes = quest_resident_encounter_path
        .wait_path
        .nodes
        .iter()
        .map(|node| format!("{:02X}", node.opcode))
        .collect::<Vec<_>>()
        .join(">");
    let encounter_display_opcodes = quest_resident_encounter_path
        .display_path
        .nodes
        .iter()
        .map(|node| format!("{:02X}", node.opcode))
        .collect::<Vec<_>>()
        .join(">");
    println!(
        "PASS QUEST resident encounter source path: root 0x{:08X}, OP14 false \
         0x{:08X}, wait [{}], OP13 object +0x{:X} == {} true 0x{:08X} / false \
         0x{:08X}, display [{}]",
        quest_resident_encounter_path.literal_root_runtime,
        quest_resident_encounter_path
            .first_compare
            .fallthrough_runtime,
        encounter_wait_opcodes,
        quest_resident_encounter_path
            .ready_compare
            .left_parameter
            .expect("검증된 object field"),
        quest_resident_encounter_path.ready_compare.right_parameter,
        quest_resident_encounter_path
            .ready_compare
            .branch_target_runtime,
        quest_resident_encounter_path
            .ready_compare
            .fallthrough_runtime,
        encounter_display_opcodes,
    );
    let quest_resident_spawn_path = verify_box_quest_resident_seq_spawn_path(
        quest_map,
        quest_text,
        code_table,
        &quest_resident_seq_frontiers,
        &quest_resident_seq_transition,
    )
    .map_err(|error| error.to_string())?;
    let spawn_path_opcodes = quest_resident_spawn_path
        .path
        .nodes
        .iter()
        .map(|node| format!("{:02X}", node.opcode))
        .collect::<Vec<_>>()
        .join(">");
    let deferred_path_opcodes = quest_resident_spawn_path
        .deferred_path
        .nodes
        .iter()
        .map(|node| format!("{:02X}", node.opcode))
        .collect::<Vec<_>>()
        .join(">");
    let deferred_terminal = quest_resident_spawn_path
        .deferred_path
        .nodes
        .last()
        .expect("검증된 resident deferred path terminal");
    println!(
        "PASS QUEST resident SEQ spawn path: OP80 0x{:08X} -> child 0x{:08X}, \
             path [{}], text glyphs/controls {}/{}, OPC1 deferred 0x{:08X}, \
             deferred path [{}] -> release 0x{:08X}, OP09 0x{:08X} field +0x{:X} / \
             {} choices, new-game {} -> 0x{:08X}",
        quest_resident_spawn_path.parent_record_runtime,
        quest_resident_spawn_path.spawned_stream_runtime,
        spawn_path_opcodes,
        quest_resident_spawn_path.text_glyphs,
        quest_resident_spawn_path.text_controls,
        quest_resident_spawn_path.deferred_stream_runtime,
        deferred_path_opcodes,
        deferred_terminal.runtime,
        quest_resident_spawn_path.menu_branch_runtime,
        quest_resident_spawn_path.menu_selection_field_offset,
        quest_resident_spawn_path.menu_selection_count,
        quest_resident_spawn_path.new_game_selection_index,
        quest_resident_spawn_path.new_game_stream_runtime,
    );
    let quest_resident_base_text_units = verify_box_quest_resident_text_units(
        quest_map,
        quest_text,
        code_table,
        &quest_resident_seq_paths,
        &quest_resident_spawn_path,
    )
    .map_err(|error| error.to_string())?;
    let quest_resident_escape_text = verify_box_quest_resident_escape_text(
        quest_map,
        quest_text,
        code_table,
        &quest_resident_encounter_path,
    )
    .map_err(|error| error.to_string())?;
    let quest_escape_outcome = verify_box_quest_escape_outcome(quest_map, quest_text)
        .map_err(|error| error.to_string())?;
    let escape_path_opcodes = quest_resident_escape_text
        .attempt_path
        .nodes
        .iter()
        .map(|node| format!("{:02X}", node.opcode))
        .collect::<Vec<_>>()
        .join(">");
    let escape_targets = quest_resident_escape_text
        .selector_targets
        .iter()
        .map(|target| format!("0x{target:08X}"))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "PASS QUEST resident escape source path: OP09 0x{:08X} field +0x{:X}, targets [{}], \
         attempt [{}], OP14 0x{:08X} object +0x{:X} == {}, true 0x{:08X} / false 0x{:08X}, \
         units {}",
        quest_resident_escape_text.selector_runtime,
        quest_resident_escape_text.selector_field_offset,
        escape_targets,
        escape_path_opcodes,
        quest_resident_escape_text
            .attempt_path
            .nodes
            .last()
            .expect("검증된 escape OP14")
            .runtime,
        quest_resident_escape_text
            .outcome_compare
            .left_parameter
            .expect("검증된 object field"),
        quest_resident_escape_text.outcome_compare.right_parameter,
        quest_resident_escape_text
            .outcome_compare
            .branch_target_runtime,
        quest_resident_escape_text
            .outcome_compare
            .fallthrough_runtime,
        quest_resident_escape_text.units.units.len(),
    );
    println!(
        "PASS QUEST escape outcome producer: OPC8 0x{:08X}, object +0x{:X}, \
         aggregate/comparison 0x{:08X}/0x{:08X}, forced failure 0x{:08X}, \
         random moduli {:?}, failure flag 0x{:04X}",
        quest_escape_outcome.handler_runtime,
        quest_escape_outcome.outcome_field_offset,
        quest_escape_outcome.aggregate_value_runtime,
        quest_escape_outcome.comparison_value_runtime,
        quest_escape_outcome.forced_failure_state_runtime,
        quest_escape_outcome.random_moduli,
        quest_escape_outcome.failure_flag_mask,
    );
    let quest_resident_text_units = combine_quest_resident_text_units([
        quest_resident_base_text_units.units.as_slice(),
        quest_resident_escape_text.units.units.as_slice(),
    ])
    .map_err(|error| error.to_string())?;
    let resident_text_spans = quest_resident_text_units
        .units
        .iter()
        .map(|unit| {
            format!(
                "{}/0x{:08X}..0x{:08X}/glyph={}/control={}/runs={}/sha256={}",
                unit.role.label(),
                unit.record_runtime,
                unit.end_runtime,
                unit.glyph_occurrences,
                unit.control_occurrences,
                unit.runs.len(),
                unit.expected_raw_sha256
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>(),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "PASS QUEST resident text units: units {}, glyphs {}, Japanese {}, controls {}, \
             runs {}, runtime observed {}, spans [{}]",
        quest_resident_text_units.units.len(),
        quest_resident_text_units.glyph_occurrences,
        quest_resident_text_units.japanese_glyph_occurrences,
        quest_resident_text_units.control_occurrences,
        quest_resident_text_units.runs,
        quest_resident_text_units.runtime_observed_units,
        resident_text_spans,
    );
    let quest_battle_names = verify_box_quest_battle_name_table(quest_map, quest_text, code_table)
        .map_err(|error| error.to_string())?;
    let narrowest_slot = quest_battle_names
        .entries
        .iter()
        .map(|entry| entry.slot_capacity_glyphs)
        .min()
        .expect("검증된 battle name 모집단");
    let widest_slot = quest_battle_names
        .entries
        .iter()
        .map(|entry| entry.slot_capacity_glyphs)
        .max()
        .expect("검증된 battle name 모집단");
    println!(
        "PASS QUEST battle name table: OP09 0x{:08X} field +0x{:X}, pointers \
         0x{:08X}..0x{:08X}, entries/unique {}/{}, glyphs {}, slots {}B / capacity {}..{}, \
         OP11 state 0x{:02X}->0x{:08X}",
        quest_battle_names.selector_runtime,
        quest_battle_names.selector_field_offset,
        quest_battle_names.pointer_table_runtime,
        quest_battle_names.pointer_table_end_runtime,
        quest_battle_names.entries.len(),
        quest_battle_names.unique_sources,
        quest_battle_names.glyph_occurrences,
        quest_battle_names.slot_bytes,
        narrowest_slot,
        widest_slot,
        quest_battle_names.fixed_text_state_index,
        quest_battle_names.fixed_text_state_runtime,
    );
    let quest_battle_name_compositions = audit_quest_battle_name_compositions(&quest_battle_names)
        .map_err(|error| error.to_string())?;
    if quest_battle_name_compositions.translation_approved
        || quest_battle_name_compositions.layout_approved
        || quest_battle_name_compositions.runtime_verified
        || quest_battle_name_compositions.product_eligible
    {
        return Err("QUEST battle name composition이 개발 상태를 넘어 승인됨".to_owned());
    }
    println!(
        "PASS QUEST battle name compositions: physical {}, selected drafts {}, human choices {}, \
         used glyphs {}, translation/layout/runtime/product approved false/false/false/false",
        quest_battle_name_compositions.compositions.len(),
        quest_battle_name_compositions.selected_drafts,
        quest_battle_name_compositions.needs_human_choice,
        quest_battle_name_compositions.used_glyphs.chars().count(),
    );
    let quest_resident_encounter_text = verify_box_quest_resident_encounter_text(
        quest_map,
        quest_text,
        code_table,
        &quest_resident_encounter_path,
        &quest_battle_names,
    )
    .map_err(|error| error.to_string())?;
    let encounter_dynamic_name = quest_resident_encounter_text
        .unit
        .tokens
        .iter()
        .find(|token| token.control_code == Some(0x1E))
        .expect("검증된 encounter FF1E mode 0");
    println!(
        "PASS QUEST resident encounter text: 0x{:08X}..0x{:08X}, glyphs/Japanese/controls \
         {}/{}/{}, runs {}, FF1E mode {} at 0x{:08X} -> state 0x{:08X}, battle names {}, \
         SHA-256 {}",
        quest_resident_encounter_text.unit.record_runtime,
        quest_resident_encounter_text.unit.end_runtime,
        quest_resident_encounter_text.unit.glyph_occurrences,
        quest_resident_encounter_text
            .unit
            .japanese_glyph_occurrences,
        quest_resident_encounter_text.unit.control_occurrences,
        quest_resident_encounter_text.unit.runs.len(),
        encounter_dynamic_name.raw[2],
        encounter_dynamic_name.runtime,
        quest_resident_encounter_text.fixed_text_state_runtime,
        quest_resident_encounter_text.battle_name_entries,
        quest_resident_encounter_text
            .unit
            .expected_raw_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    );
    let quest_resident_encounter_composition =
        audit_quest_resident_encounter_text_composition(&quest_resident_encounter_text.unit)
            .map_err(|error| error.to_string())?;
    if quest_resident_encounter_composition.translation_approved
        || quest_resident_encounter_composition.layout_approved
        || quest_resident_encounter_composition.runtime_verified
        || quest_resident_encounter_composition.product_eligible
    {
        return Err("QUEST resident encounter composition이 개발 상태를 넘어 승인됨".to_owned());
    }
    println!(
        "PASS QUEST resident encounter composition: runs {}, human choices {}, used glyphs {}, \
         translation/layout/runtime/product approved false/false/false/false",
        quest_resident_encounter_composition.decisions.len(),
        quest_resident_encounter_composition.human_choice_runs,
        quest_resident_encounter_composition
            .used_glyphs
            .chars()
            .count(),
    );

    Ok(QuestExecutionPathAudit {
        text_paths: quest_seq_text_paths,
        text_frontiers: quest_seq_text_frontiers,
        resident_text_units: quest_resident_text_units,
        battle_names: quest_battle_names,
        resident_encounter_text: quest_resident_encounter_text,
        registration: *quest_seq_registration,
    })
}
