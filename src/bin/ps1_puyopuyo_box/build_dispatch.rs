//! build 명령의 종류와 대상을 실제 builder에 연결한다.

use std::ffi::{OsStr, OsString};

use ps1_puyopuyo_box::pipeline::MmSprPocScope;

use super::build_commands::{
    build_cnx_control, build_current_development_disc_command, build_drac_single_glyph,
    build_menu06_draft, build_mm_spr_poc, build_p2_info_page_stress, build_puyo2_e6_approved,
    build_puyo2_e6_single_glyph, build_puyo2_full_glyph_population,
    build_puyo2_scene_glyph_population, build_quest_development_disc_command,
    build_quest_exe_relocation_control_poc, build_quest_glyph_cache_control_poc,
    build_quest_glyph_cache_transition_poc_command, build_quest_glyph_cache_upload_poc_command,
    build_quest_hook_noop_poc, build_quest_page_first_dialogue_poc,
    build_quest_page_transition_poc, build_quest_page_two_dialogues_poc, build_quest_poc,
};
use super::usage::usage;

pub(super) fn dispatch_build(
    subcommand: Option<&OsStr>,
    boundary: Option<&OsStr>,
    args: impl Iterator<Item = OsString>,
) -> Result<(), String> {
    if subcommand == Some("poc".as_ref()) && boundary == Some("quest-first-dialogue".as_ref()) {
        return build_quest_poc(args);
    }
    if subcommand == Some("poc".as_ref()) && boundary == Some("quest-hook-noop".as_ref()) {
        return build_quest_hook_noop_poc(args);
    }
    if subcommand == Some("poc".as_ref())
        && boundary == Some("quest-exe-relocation-control".as_ref())
    {
        return build_quest_exe_relocation_control_poc(args);
    }
    if subcommand == Some("poc".as_ref()) && boundary == Some("quest-page-first-dialogue".as_ref())
    {
        return build_quest_page_first_dialogue_poc(args);
    }
    if subcommand == Some("poc".as_ref()) && boundary == Some("quest-page-transition".as_ref()) {
        return build_quest_page_transition_poc(args);
    }
    if subcommand == Some("poc".as_ref()) && boundary == Some("quest-page-two-dialogues".as_ref()) {
        return build_quest_page_two_dialogues_poc(args);
    }
    if subcommand == Some("poc".as_ref()) && boundary == Some("quest-glyph-cache-control".as_ref())
    {
        return build_quest_glyph_cache_control_poc(args);
    }
    if subcommand == Some("poc".as_ref()) && boundary == Some("quest-glyph-cache-upload".as_ref()) {
        return build_quest_glyph_cache_upload_poc_command(args);
    }
    if subcommand == Some("poc".as_ref())
        && boundary == Some("quest-glyph-cache-transition".as_ref())
    {
        return build_quest_glyph_cache_transition_poc_command(args);
    }
    if subcommand == Some("dev".as_ref()) && boundary == Some("quest".as_ref()) {
        return build_quest_development_disc_command(args);
    }
    if subcommand == Some("dev".as_ref()) && boundary == Some("current".as_ref()) {
        return build_current_development_disc_command(args);
    }
    if subcommand == Some("poc".as_ref()) && boundary == Some("menu06-draft".as_ref()) {
        return build_menu06_draft(args);
    }
    if subcommand == Some("poc".as_ref()) && boundary == Some("p2-info-page-stress".as_ref()) {
        return build_p2_info_page_stress(args);
    }
    if subcommand == Some("poc".as_ref()) && boundary == Some("drac-single-glyph".as_ref()) {
        return build_drac_single_glyph(args);
    }
    if subcommand == Some("poc".as_ref())
        && boundary == Some("puyo2-scene-glyph-population".as_ref())
    {
        return build_puyo2_scene_glyph_population(args);
    }
    if subcommand == Some("poc".as_ref())
        && boundary == Some("puyo2-full-glyph-population".as_ref())
    {
        return build_puyo2_full_glyph_population(args);
    }
    if subcommand == Some("poc".as_ref()) && boundary == Some("puyo2-e6-single-glyph".as_ref()) {
        return build_puyo2_e6_single_glyph(args);
    }
    if subcommand == Some("patch".as_ref()) && boundary == Some("puyo2-e6".as_ref()) {
        return build_puyo2_e6_approved(args);
    }
    if subcommand == Some("poc".as_ref()) && boundary == Some("shell-first-description".as_ref()) {
        return build_mm_spr_poc(args, MmSprPocScope::FirstDescription);
    }
    if subcommand == Some("poc".as_ref())
        && boundary == Some("shell-approved-descriptions".as_ref())
    {
        return build_mm_spr_poc(args, MmSprPocScope::ApprovedDescriptions);
    }
    if subcommand == Some("control".as_ref()) && boundary == Some("cnx-all".as_ref()) {
        return build_cnx_control(args);
    }

    Err(usage().to_string())
}
