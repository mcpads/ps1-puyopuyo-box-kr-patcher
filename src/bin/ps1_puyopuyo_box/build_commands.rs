//! PoC와 control 이미지 빌드 명령.

use std::path::PathBuf;

use ps1_puyopuyo_box::disc::VerifiedSourceImage;
use ps1_puyopuyo_box::pipeline::cnx_control::build_all_cnx_control;
use ps1_puyopuyo_box::pipeline::drac_single_glyph_poc::build_drac_single_glyph_poc;
use ps1_puyopuyo_box::pipeline::menu06_draft_reinsertion::build_box_menu06_draft_disc;
use ps1_puyopuyo_box::pipeline::mm_spr_poc::{
    build_mm_spr_approved_descriptions, build_mm_spr_first_description, MmSprPocMode, MmSprPocScope,
};
use ps1_puyopuyo_box::pipeline::p2_info_page_stress_disc::build_p2_info_page_stress_disc;
use ps1_puyopuyo_box::pipeline::puyo2_e6_single_glyph_poc::{
    build_puyo2_e6_approved_translation, build_puyo2_e6_single_glyph_poc,
    build_puyo2_full_glyph_population_poc,
};
use ps1_puyopuyo_box::pipeline::puyo2_scene_glyph_population_poc::build_puyo2_scene_glyph_population_poc;
use ps1_puyopuyo_box::pipeline::quest_exe_relocation_control::build_quest_exe_relocation_control;
use ps1_puyopuyo_box::pipeline::quest_glyph_cache_disc::verify_box_quest_glyph_cache_disc_control;
use ps1_puyopuyo_box::pipeline::quest_glyph_cache_transition_poc::verify_box_quest_glyph_cache_transition_poc;
use ps1_puyopuyo_box::pipeline::quest_glyph_cache_upload_poc::verify_box_quest_glyph_cache_upload_poc;
use ps1_puyopuyo_box::pipeline::quest_hook_poc::build_quest_hook_noop;
use ps1_puyopuyo_box::pipeline::quest_page_first_dialogue_poc::{
    build_quest_page_first_dialogue, build_quest_page_transition, build_quest_page_two_dialogues,
};
use ps1_puyopuyo_box::pipeline::quest_poc::{build_quest_first_dialogue, QuestPocMode};

use super::current_development_outputs::write_current_development_disc_outputs;
use super::development_disc_inputs::{
    read_current_development_disc_inputs, read_quest_development_disc_inputs,
};
use super::output_writers::{
    write_cnx_control_outputs, write_drac_single_glyph_poc_outputs, write_menu06_draft_outputs,
    write_mm_spr_poc_outputs, write_p2_info_page_stress_outputs, write_poc_outputs,
    write_puyo2_e6_approved_translation_outputs, write_puyo2_e6_single_glyph_poc_outputs,
    write_puyo2_full_glyph_population_poc_outputs, write_puyo2_scene_glyph_population_poc_outputs,
    write_quest_exe_relocation_control_outputs, write_quest_hook_noop_outputs,
    write_quest_page_first_dialogue_outputs, write_quest_page_transition_outputs,
    write_quest_page_two_dialogues_outputs,
};
use super::quest_analysis_inputs::read_quest_glyph_cache_control_inputs;
use super::quest_font_page_reports::read_verified_quest_second_dialogue_assets;
use super::quest_glyph_cache_outputs::{
    write_quest_development_disc_outputs, write_quest_glyph_cache_control_outputs,
    write_quest_glyph_cache_transition_outputs, write_quest_glyph_cache_upload_outputs,
};
use super::usage::usage;

pub(super) fn build_quest_poc(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let mut control = false;
    let mut paths = Vec::new();
    for arg in args {
        if arg == "--control" {
            if control {
                return Err("--control이 중복됨".to_string());
            }
            control = true;
        } else {
            paths.push(PathBuf::from(arg));
        }
    }
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let mode = if control {
        QuestPocMode::Control
    } else {
        QuestPocMode::Korean
    };
    let build = build_quest_first_dialogue(&source, mode).map_err(|error| error.to_string())?;
    write_poc_outputs(output_dir, build)
}

pub(super) fn build_mm_spr_poc(
    args: impl Iterator<Item = std::ffi::OsString>,
    scope: MmSprPocScope,
) -> Result<(), String> {
    let mut control = false;
    let mut paths = Vec::new();
    for arg in args {
        if arg == "--control" {
            if control {
                return Err("--control이 중복됨".to_string());
            }
            control = true;
        } else {
            paths.push(PathBuf::from(arg));
        }
    }
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let mode = if control {
        MmSprPocMode::Control
    } else {
        MmSprPocMode::Korean
    };
    let build = match scope {
        MmSprPocScope::FirstDescription => build_mm_spr_first_description(&source, mode),
        MmSprPocScope::ApprovedDescriptions => build_mm_spr_approved_descriptions(&source, mode),
    }
    .map_err(|error| error.to_string())?;
    write_mm_spr_poc_outputs(output_dir, build)
}

pub(super) fn build_drac_single_glyph(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let build = build_drac_single_glyph_poc(&source).map_err(|error| error.to_string())?;
    write_drac_single_glyph_poc_outputs(output_dir, build)
}

pub(super) fn build_puyo2_scene_glyph_population(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let build =
        build_puyo2_scene_glyph_population_poc(&source).map_err(|error| error.to_string())?;
    write_puyo2_scene_glyph_population_poc_outputs(output_dir, build)
}

pub(super) fn build_puyo2_full_glyph_population(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let build =
        build_puyo2_full_glyph_population_poc(&source).map_err(|error| error.to_string())?;
    write_puyo2_full_glyph_population_poc_outputs(output_dir, build)
}

pub(super) fn build_menu06_draft(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let build = build_box_menu06_draft_disc(&source).map_err(|error| error.to_string())?;
    write_menu06_draft_outputs(output_dir, build)
}

pub(super) fn build_p2_info_page_stress(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let build = build_p2_info_page_stress_disc(&source).map_err(|error| error.to_string())?;
    write_p2_info_page_stress_outputs(output_dir, build)
}

pub(super) fn build_puyo2_e6_single_glyph(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let build = build_puyo2_e6_single_glyph_poc(&source).map_err(|error| error.to_string())?;
    write_puyo2_e6_single_glyph_poc_outputs(output_dir, build)
}

pub(super) fn build_puyo2_e6_approved(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let build = build_puyo2_e6_approved_translation(&source).map_err(|error| error.to_string())?;
    write_puyo2_e6_approved_translation_outputs(output_dir, build)
}

pub(super) fn build_cnx_control(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let build = build_all_cnx_control(&source).map_err(|error| error.to_string())?;
    write_cnx_control_outputs(output_dir, build)
}

pub(super) fn build_quest_hook_noop_poc(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let build = build_quest_hook_noop(&source).map_err(|error| error.to_string())?;
    write_quest_hook_noop_outputs(output_dir, build)
}

pub(super) fn build_quest_exe_relocation_control_poc(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let build = build_quest_exe_relocation_control(&source).map_err(|error| error.to_string())?;
    write_quest_exe_relocation_control_outputs(output_dir, build)
}

pub(super) fn build_quest_page_first_dialogue_poc(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let build = build_quest_page_first_dialogue(&source).map_err(|error| error.to_string())?;
    write_quest_page_first_dialogue_outputs(output_dir, build)
}

pub(super) fn build_quest_page_transition_poc(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let build = build_quest_page_transition(&source).map_err(|error| error.to_string())?;
    write_quest_page_transition_outputs(output_dir, build)
}

pub(super) fn build_quest_page_two_dialogues_poc(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let second = read_verified_quest_second_dialogue_assets(source_path)?;
    let build =
        build_quest_page_two_dialogues(&source, &second).map_err(|error| error.to_string())?;
    write_quest_page_two_dialogues_outputs(output_dir, build)
}

pub(super) fn build_quest_glyph_cache_control_poc(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let inputs = read_quest_glyph_cache_control_inputs(&source)?;
    let build = verify_box_quest_glyph_cache_disc_control(&source, &inputs.package, &inputs.hook)
        .map_err(|error| error.to_string())?;
    write_quest_glyph_cache_control_outputs(output_dir, build)
}

pub(super) fn build_quest_glyph_cache_upload_poc_command(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let inputs = read_quest_glyph_cache_control_inputs(&source)?;
    let build = verify_box_quest_glyph_cache_upload_poc(
        &source,
        &inputs.batches,
        &inputs.routing,
        &inputs.package,
        &inputs.hook,
    )
    .map_err(|error| error.to_string())?;
    write_quest_glyph_cache_upload_outputs(output_dir, build)
}

pub(super) fn build_quest_glyph_cache_transition_poc_command(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let inputs = read_quest_glyph_cache_control_inputs(&source)?;
    let build = verify_box_quest_glyph_cache_transition_poc(
        &source,
        &inputs.batches,
        &inputs.routing,
        &inputs.package,
        &inputs.hook,
    )
    .map_err(|error| error.to_string())?;
    write_quest_glyph_cache_transition_outputs(output_dir, build)
}

pub(super) fn build_quest_development_disc_command(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let inputs = read_quest_development_disc_inputs(source_path, &source)?;
    write_quest_development_disc_outputs(output_dir, inputs.build, inputs.routing)
}

pub(super) fn build_current_development_disc_command(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_dir] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let source = VerifiedSourceImage::open(source_path)
        .map_err(|error| format!("{}: {error}", source_path.display()))?;
    let inputs = read_current_development_disc_inputs(source_path, &source)?;
    write_current_development_disc_outputs(output_dir, inputs.build, inputs.routing)
}
