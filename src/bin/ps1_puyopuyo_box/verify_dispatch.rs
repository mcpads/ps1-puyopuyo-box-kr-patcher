//! verify 명령의 원본, roundtrip, inventory, runtime 대상을 연결한다.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use super::disc_commands::{audit_btx_inventory, audit_vdds, verify_runtime_btx_layout};
use super::quest_audio_runtime_commands::{
    verify_quest_glyph_cache_audio_runtime, verify_quest_sound_sequence_runtime,
    verify_quest_spu_voice_source_runtime,
};
use super::quest_carrier_runtime_command::verify_quest_development_carrier_scene_runtime_command;
use super::quest_development_disc_runtime_commands::{
    verify_quest_development_disc_max_page_runtime,
    verify_quest_development_disc_residency_runtime,
    verify_quest_development_disc_synthetic_route_runtime,
};
use super::quest_executable_runtime_command::verify_quest_exe_relocation_control_runtime;
use super::quest_font_runtime_commands::{
    verify_quest_font_active_lifetime_runtime, verify_quest_font_active_natural_runtime,
};
use super::quest_glyph_cache_bank_runtime_command::verify_quest_glyph_cache_bank_runtime_command;
use super::quest_glyph_cache_collision_runtime_commands::{
    verify_quest_glyph_cache_collision_runtime,
    verify_quest_glyph_cache_transition_collision_runtime,
};
use super::quest_glyph_cache_runtime_commands::{
    verify_quest_glyph_cache_control_runtime, verify_quest_glyph_cache_residency,
    verify_quest_glyph_cache_transition_runtime, verify_quest_glyph_cache_upload_runtime,
    verify_quest_resident_text_runtime,
};
use super::quest_page_runtime_commands::{
    verify_quest_page_first_dialogue_runtime, verify_quest_page_transition_runtime,
    verify_quest_page_two_dialogues_runtime,
};
use super::quest_save_runtime_command::verify_quest_save_card_runtime;
use super::quest_vram_bank_ownership_runtime_command::verify_quest_vram_bank_ownership_runtime_command;
use super::source_audit::audit_source_disc;
use super::usage::usage;

pub(super) fn dispatch_verify(
    subcommand: Option<&OsStr>,
    boundary: Option<&OsStr>,
    args: impl Iterator<Item = OsString>,
) -> Result<(), String> {
    match (subcommand, boundary) {
        (Some(subcommand), Some(boundary)) if subcommand == "roundtrip" && boundary == "vdd" => {
            let mut require_original_bytes = false;
            let mut paths = Vec::new();
            for arg in args {
                if arg == "--require-cnx-byte-identical" {
                    require_original_bytes = true;
                } else {
                    paths.push(PathBuf::from(arg));
                }
            }
            audit_vdds(&paths, require_original_bytes)
        }
        (Some(subcommand), Some(boundary)) if subcommand == "inventory" && boundary == "btx" => {
            audit_btx_inventory(&args.map(PathBuf::from).collect::<Vec<_>>())
        }
        (Some(subcommand), Some(boundary)) if subcommand == "source" && boundary == "disc" => {
            audit_source_disc(&args.map(PathBuf::from).collect::<Vec<_>>())
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "btx-layout" =>
        {
            verify_runtime_btx_layout(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-exe-relocation-control" =>
        {
            verify_quest_exe_relocation_control_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-glyph-cache-control" =>
        {
            verify_quest_glyph_cache_control_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-glyph-cache-upload" =>
        {
            verify_quest_glyph_cache_upload_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-resident-text" =>
        {
            verify_quest_resident_text_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-glyph-cache-transition" =>
        {
            verify_quest_glyph_cache_transition_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-glyph-cache-residency" =>
        {
            verify_quest_glyph_cache_residency(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-development-disc-residency" =>
        {
            verify_quest_development_disc_residency_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-development-carrier-scene" =>
        {
            verify_quest_development_carrier_scene_runtime_command(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-glyph-cache-bank" =>
        {
            verify_quest_glyph_cache_bank_runtime_command(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-vram-bank-ownership" =>
        {
            verify_quest_vram_bank_ownership_runtime_command(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-development-disc-synthetic-route" =>
        {
            verify_quest_development_disc_synthetic_route_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-development-disc-max-page" =>
        {
            verify_quest_development_disc_max_page_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-glyph-cache-audio" =>
        {
            verify_quest_glyph_cache_audio_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-spu-voice-source" =>
        {
            verify_quest_spu_voice_source_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-sound-sequence" =>
        {
            verify_quest_sound_sequence_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-glyph-cache-collision" =>
        {
            verify_quest_glyph_cache_collision_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-glyph-cache-transition-collision" =>
        {
            verify_quest_glyph_cache_transition_collision_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-save-card" =>
        {
            verify_quest_save_card_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-font-active-lifetime" =>
        {
            verify_quest_font_active_lifetime_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-font-active-natural" =>
        {
            verify_quest_font_active_natural_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-page-first-dialogue" =>
        {
            verify_quest_page_first_dialogue_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-page-transition" =>
        {
            verify_quest_page_transition_runtime(args)
        }
        (Some(subcommand), Some(boundary))
            if subcommand == "runtime" && boundary == "quest-page-two-dialogues" =>
        {
            verify_quest_page_two_dialogues_runtime(args)
        }
        _ => Err(usage().to_string()),
    }
}
