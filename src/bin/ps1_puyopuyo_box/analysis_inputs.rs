//! 분석 보고서가 공유하는 원본·서브게임 분석 stage를 역할별로 연결한다.

mod source_files;
mod subgame_drac;
mod subgame_e6;
mod subgame_resource_routes;
mod subgame_scene;
mod subgame_text;

pub(super) use source_files::{
    read_quest_seq_sources, read_subgame_seq_sources, read_town00_source,
};
pub(super) use subgame_drac::{
    read_subgame_seq_drac_consumer_analysis, read_subgame_seq_drac_glyph_analysis,
    read_subgame_seq_drac_relink_analysis, read_subgame_seq_drac_text_analysis,
};
pub(super) use subgame_e6::{
    read_subgame_seq_e6_glyph_analysis, read_subgame_seq_e6_text_analysis,
};
pub(super) use subgame_resource_routes::{
    read_subgame_seq_descriptor_analysis, read_subgame_seq_load_analysis,
    read_subgame_seq_registration_analysis, read_subgame_seq_selection_analysis,
};
pub(super) use subgame_scene::{
    read_subgame_seq_scene_glyph_analysis, read_subgame_seq_scene_message_analysis,
    read_subgame_seq_scene_relink_analysis,
};
pub(super) use subgame_text::{
    read_menu06_text_analysis, read_subgame_seq_control_target_analysis,
    read_subgame_seq_layout_analysis, read_subgame_seq_min_text_analysis,
    read_subgame_seq_overlay_call_argument_analysis, read_subgame_seq_text_analysis,
    read_subgame_seq_text_bridge_analysis, read_subgame_seq_text_bridge_source,
    read_subgame_seq_text_root_analysis,
};
