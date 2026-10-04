//! report 명령의 translation, analysis, runtime 대상을 연결한다.

use std::ffi::{OsStr, OsString};

use super::analysis_reports::{
    write_bios_font_report, write_btx_cell_report, write_common_font_draw_site_report,
    write_exe_string_report, write_exe_trace_report, write_subgame_seq_control_report,
    write_subgame_seq_control_target_report, write_subgame_seq_descriptor_report,
    write_subgame_seq_drac_consumer_report, write_subgame_seq_drac_glyph_report,
    write_subgame_seq_drac_ram_report, write_subgame_seq_drac_relink_report,
    write_subgame_seq_drac_text_report, write_subgame_seq_e6_glyph_report,
    write_subgame_seq_e6_ram_report, write_subgame_seq_e6_text_report,
    write_subgame_seq_layout_report, write_subgame_seq_load_report,
    write_subgame_seq_min_text_report, write_subgame_seq_overlay_call_argument_report,
    write_subgame_seq_registration_report, write_subgame_seq_scene_glyph_patch_report,
    write_subgame_seq_scene_glyph_report, write_subgame_seq_scene_message_report,
    write_subgame_seq_scene_relink_report, write_subgame_seq_scene_translation_plan_report,
    write_subgame_seq_selection_report, write_subgame_seq_text_bridge_call_reports,
    write_subgame_seq_text_bridge_ram_report, write_subgame_seq_text_bridge_report,
    write_subgame_seq_text_candidate_report, write_subgame_seq_text_root_report,
    write_subgame_seq_text_summary_report, write_subgame_text_consumer_report,
};
use super::btx_reports::{
    write_btx_family_candidate_reports, write_btx_family_review_template, write_btx_name_report,
    write_btx_pending_consumer_report, write_btx_pending_runtime_report,
    write_btx_pending_static_cell_report, write_btx_profile_candidate_member_report,
    write_btx_profile_candidate_report, write_btx_review_layout_evidence_report,
    write_common_font_cell_source_report, write_common_font_fixed_draw_report,
    write_common_font_text_report,
};
use super::common_font_runtime_reports::{
    write_common_font_direct_draw_report, write_common_font_draw_report,
    write_common_font_writer_draw_report,
};
use super::quest_font_page_reports::{
    write_quest_capacity_poc_report, write_quest_font_alternatives_report,
    write_quest_font_plan_report, write_quest_page_supply_plan_report,
    write_quest_second_dialogue_plan_report, write_quest_source_repertoire_report,
};
use super::quest_hook_space_report::write_quest_hook_space_report;
use super::quest_item_container_contact_runtime_report::write_quest_item_container_contact_candidate;
use super::quest_map_collision_runtime_report::report_quest_map_collision_state;
use super::quest_map_route_runtime_report::write_quest_map_route_candidate;
use super::quest_text_runtime_report::write_quest_seq_text_runtime_evidence;
use super::quest_text_source_reports::{
    write_quest_battle_name_source_report, write_quest_item_text_source_report,
    write_quest_seq_flag_branch_report, write_quest_seq_resource_transition_report,
    write_quest_seq_rooted_text_run_report, write_quest_seq_text_candidate_review,
    write_quest_seq_text_document_summary, write_quest_seq_text_frontier_report,
    write_quest_seq_text_path_report, write_quest_seq_text_root_report,
    write_quest_seq_text_stream_report, write_quest_seq_text_summary,
    write_quest_seq_text_token_ledger, write_town00_text_candidate_review,
};
use super::quest_translation_approval_reports::{
    write_quest_translation_approval_template, write_quest_translation_batch_decisions,
    write_quest_translation_decision_workbook,
};
use super::quest_translation_glyph_cache_reports::{
    write_quest_translation_glyph_cache_hook_plan, write_quest_translation_glyph_cache_package,
    write_quest_translation_glyph_cache_plan,
};
use super::quest_translation_review_reports::{
    write_quest_seq_translation_unit_review, write_quest_terminology_candidate_review,
    write_quest_translation_batch_review, write_quest_translation_review_queue,
    write_quest_translation_selection, write_quest_translation_selection_routing,
};
use super::runtime_reports::{
    report_quest_font_boundary, report_quest_hook_space_runtime, report_town00_root_state,
    write_quest_font_window_geometry_report, write_quest_seq_address_report,
    write_runtime_cell_selection_report, write_runtime_cell_table_report,
    write_town00_address_report,
};
use super::translation_reports::{
    report_translation_coverage, write_btx_pending_review, write_btx_translation_review,
    write_menu06_text_review, write_menu06_translation_manifest, write_mm_spr_review,
    write_puyo2_e6_translation_review, write_quest_dialogue_review,
    write_subgame_seq_scene_translation_review,
};
use super::usage::usage;

pub(super) fn dispatch_report(
    subcommand: Option<&OsStr>,
    boundary: Option<&OsStr>,
    args: impl Iterator<Item = OsString>,
) -> Result<(), String> {
    if subcommand == Some("translation".as_ref()) && boundary == Some("coverage".as_ref()) {
        return report_translation_coverage(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("mm-spr".as_ref()) {
        return write_mm_spr_review(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("btx-catalog".as_ref()) {
        return write_btx_translation_review(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("btx-pending".as_ref()) {
        return write_btx_pending_review(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("menu06".as_ref()) {
        return write_menu06_text_review(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("menu06-manifest".as_ref()) {
        return write_menu06_translation_manifest(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("quest-dialogue".as_ref()) {
        return write_quest_dialogue_review(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("quest-item-text".as_ref()) {
        return write_quest_item_text_source_report(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("quest-battle-names".as_ref())
    {
        return write_quest_battle_name_source_report(args);
    }
    if subcommand == Some("translation".as_ref())
        && boundary == Some("quest-town00-candidates".as_ref())
    {
        return write_town00_text_candidate_review(args);
    }
    if subcommand == Some("translation".as_ref())
        && boundary == Some("quest-seq-candidates".as_ref())
    {
        return write_quest_seq_text_candidate_review(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("quest-seq-summary".as_ref())
    {
        return write_quest_seq_text_summary(args);
    }
    if subcommand == Some("translation".as_ref())
        && boundary == Some("quest-seq-rooted-runs".as_ref())
    {
        return write_quest_seq_rooted_text_run_report(args);
    }
    if subcommand == Some("translation".as_ref())
        && boundary == Some("quest-seq-token-ledger".as_ref())
    {
        return write_quest_seq_text_token_ledger(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("quest-seq-review".as_ref()) {
        return write_quest_seq_translation_unit_review(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("quest-terms".as_ref()) {
        return write_quest_terminology_candidate_review(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("quest-batches".as_ref()) {
        return write_quest_translation_batch_review(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("quest-reviews".as_ref()) {
        return write_quest_translation_review_queue(args);
    }
    if subcommand == Some("translation".as_ref())
        && boundary == Some("quest-translation-selection".as_ref())
    {
        return write_quest_translation_selection(args);
    }
    if subcommand == Some("translation".as_ref())
        && boundary == Some("quest-translation-selection-routing".as_ref())
    {
        return write_quest_translation_selection_routing(args);
    }
    if subcommand == Some("translation".as_ref())
        && boundary == Some("quest-batch-decisions".as_ref())
    {
        return write_quest_translation_batch_decisions(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("quest-decisions".as_ref()) {
        return write_quest_translation_decision_workbook(args);
    }
    if subcommand == Some("translation".as_ref())
        && boundary == Some("quest-approval-template".as_ref())
    {
        return write_quest_translation_approval_template(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("puyo2-e6".as_ref()) {
        return write_puyo2_e6_translation_review(args);
    }
    if subcommand == Some("translation".as_ref()) && boundary == Some("scene-e0".as_ref()) {
        return write_subgame_seq_scene_translation_review(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("quest-seq-text-documents".as_ref())
    {
        return write_quest_seq_text_document_summary(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("quest-translation-glyph-cache-plan".as_ref())
    {
        return write_quest_translation_glyph_cache_plan(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("quest-translation-glyph-cache-package".as_ref())
    {
        return write_quest_translation_glyph_cache_package(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("quest-translation-glyph-cache-hook".as_ref())
    {
        return write_quest_translation_glyph_cache_hook_plan(args);
    }
    if subcommand == Some("runtime".as_ref())
        && boundary == Some("quest-seq-text-evidence".as_ref())
    {
        return write_quest_seq_text_runtime_evidence(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("exe-trace".as_ref()) {
        return write_exe_trace_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("bios-font".as_ref()) {
        return write_bios_font_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("btx-cells".as_ref()) {
        return write_btx_cell_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("common-font-draw-sites".as_ref())
    {
        return write_common_font_draw_site_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("common-font-cell-sources".as_ref())
    {
        return write_common_font_cell_source_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("common-font-fixed-draws".as_ref())
    {
        return write_common_font_fixed_draw_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("exe-strings".as_ref()) {
        return write_exe_string_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-text-candidates".as_ref())
    {
        return write_subgame_seq_text_candidate_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-text-summary".as_ref())
    {
        return write_subgame_seq_text_summary_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-text-roots".as_ref())
    {
        return write_subgame_seq_text_root_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("subgame-seq-min-text".as_ref())
    {
        return write_subgame_seq_min_text_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("subgame-seq-e6-text".as_ref()) {
        return write_subgame_seq_e6_text_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("subgame-seq-e6-glyphs".as_ref())
    {
        return write_subgame_seq_e6_glyph_report(args);
    }
    if subcommand == Some("runtime".as_ref()) && boundary == Some("subgame-seq-e6".as_ref()) {
        return write_subgame_seq_e6_ram_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("subgame-seq-layouts".as_ref()) {
        return write_subgame_seq_layout_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("subgame-seq-drac-text".as_ref())
    {
        return write_subgame_seq_drac_text_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-drac-consumers".as_ref())
    {
        return write_subgame_seq_drac_consumer_report(args);
    }
    if subcommand == Some("runtime".as_ref()) && boundary == Some("subgame-seq-drac".as_ref()) {
        return write_subgame_seq_drac_ram_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-drac-glyphs".as_ref())
    {
        return write_subgame_seq_drac_glyph_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-scene-glyphs".as_ref())
    {
        return write_subgame_seq_scene_glyph_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-scene-messages".as_ref())
    {
        return write_subgame_seq_scene_message_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-scene-relinks".as_ref())
    {
        return write_subgame_seq_scene_relink_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-scene-translation-plan".as_ref())
    {
        return write_subgame_seq_scene_translation_plan_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-scene-glyph-patches".as_ref())
    {
        return write_subgame_seq_scene_glyph_patch_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-drac-relinks".as_ref())
    {
        return write_subgame_seq_drac_relink_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-descriptors".as_ref())
    {
        return write_subgame_seq_descriptor_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-registrations".as_ref())
    {
        return write_subgame_seq_registration_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-selections".as_ref())
    {
        return write_subgame_seq_selection_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("subgame-seq-loads".as_ref()) {
        return write_subgame_seq_load_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-text-bridges".as_ref())
    {
        return write_subgame_seq_text_bridge_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-overlay-call-arguments".as_ref())
    {
        return write_subgame_seq_overlay_call_argument_report(args);
    }
    if subcommand == Some("runtime".as_ref())
        && boundary == Some("subgame-seq-text-bridge".as_ref())
    {
        return write_subgame_seq_text_bridge_ram_report(args);
    }
    if subcommand == Some("runtime".as_ref())
        && boundary == Some("subgame-seq-text-bridge-calls".as_ref())
    {
        return write_subgame_seq_text_bridge_call_reports(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-control-targets".as_ref())
    {
        return write_subgame_seq_control_target_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("subgame-seq-text-consumers".as_ref())
    {
        return write_subgame_text_consumer_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("subgame-seq-controls".as_ref())
    {
        return write_subgame_seq_control_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("btx-names".as_ref()) {
        return write_btx_name_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("common-font-text".as_ref()) {
        return write_common_font_text_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("btx-pending-consumers".as_ref())
    {
        return write_btx_pending_consumer_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("btx-pending-static-cells".as_ref())
    {
        return write_btx_pending_static_cell_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("btx-profile-candidates".as_ref())
    {
        return write_btx_profile_candidate_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("btx-profile-candidate-members".as_ref())
    {
        return write_btx_profile_candidate_member_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("btx-family-candidates".as_ref())
    {
        return write_btx_family_candidate_reports(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("btx-family-review-template".as_ref())
    {
        return write_btx_family_review_template(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("btx-review-layout-evidence".as_ref())
    {
        return write_btx_review_layout_evidence_report(args);
    }
    if subcommand == Some("runtime".as_ref()) && boundary == Some("btx-pending".as_ref()) {
        return write_btx_pending_runtime_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("town00-addresses".as_ref()) {
        return write_town00_address_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("quest-seq-addresses".as_ref()) {
        return write_quest_seq_address_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("quest-seq-text-streams".as_ref())
    {
        return write_quest_seq_text_stream_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("quest-seq-text-roots".as_ref())
    {
        return write_quest_seq_text_root_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("quest-seq-text-paths".as_ref())
    {
        return write_quest_seq_text_path_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("quest-seq-text-frontiers".as_ref())
    {
        return write_quest_seq_text_frontier_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("quest-seq-flag-branches".as_ref())
    {
        return write_quest_seq_flag_branch_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("quest-seq-resource-transitions".as_ref())
    {
        return write_quest_seq_resource_transition_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("quest-font-plan".as_ref()) {
        return write_quest_font_plan_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("quest-source-repertoire".as_ref())
    {
        return write_quest_source_repertoire_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("quest-capacity".as_ref()) {
        return write_quest_capacity_poc_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("quest-font-alternatives".as_ref())
    {
        return write_quest_font_alternatives_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("quest-page-supply".as_ref()) {
        return write_quest_page_supply_plan_report(args);
    }
    if subcommand == Some("analysis".as_ref())
        && boundary == Some("quest-second-dialogue-plan".as_ref())
    {
        return write_quest_second_dialogue_plan_report(args);
    }
    if subcommand == Some("analysis".as_ref()) && boundary == Some("quest-hook-space".as_ref()) {
        return write_quest_hook_space_report(args);
    }
    if subcommand == Some("runtime".as_ref()) && boundary == Some("quest-font-boundary".as_ref()) {
        return report_quest_font_boundary(args);
    }
    if subcommand == Some("runtime".as_ref()) && boundary == Some("quest-font-windows".as_ref()) {
        return write_quest_font_window_geometry_report(args);
    }
    if subcommand == Some("runtime".as_ref()) && boundary == Some("quest-hook-space".as_ref()) {
        return report_quest_hook_space_runtime(args);
    }
    if subcommand == Some("runtime".as_ref()) && boundary == Some("quest-map-collision".as_ref()) {
        return report_quest_map_collision_state(args);
    }
    if subcommand == Some("runtime".as_ref()) && boundary == Some("quest-map-route".as_ref()) {
        return write_quest_map_route_candidate(args);
    }
    if subcommand == Some("runtime".as_ref())
        && boundary == Some("quest-item-container-contact".as_ref())
    {
        return write_quest_item_container_contact_candidate(args);
    }
    if subcommand == Some("runtime".as_ref()) && boundary == Some("town00-root".as_ref()) {
        return report_town00_root_state(args);
    }
    if subcommand == Some("runtime".as_ref()) && boundary == Some("cell-table".as_ref()) {
        return write_runtime_cell_table_report(args);
    }
    if subcommand == Some("runtime".as_ref()) && boundary == Some("common-font-draw".as_ref()) {
        return write_common_font_draw_report(args);
    }
    if subcommand == Some("runtime".as_ref())
        && boundary == Some("common-font-writer-draw".as_ref())
    {
        return write_common_font_writer_draw_report(args);
    }
    if subcommand == Some("runtime".as_ref())
        && boundary == Some("common-font-direct-draw".as_ref())
    {
        return write_common_font_direct_draw_report(args);
    }
    if subcommand == Some("runtime".as_ref()) && boundary == Some("cell-selection".as_ref()) {
        return write_runtime_cell_selection_report(args);
    }

    Err(usage().to_string())
}
