//! 그래픽 자산 포맷.

pub mod boxcube_surface;
pub mod boxopt;
pub mod boxopt_development_surface;
pub mod boxparts_surface;
pub mod btx;
pub mod btx_classification;
pub mod btx_family_candidates;
pub mod btx_patch_strategy;
pub mod btx_raster_composition;
pub mod btx_raster_family;
pub mod btx_raster_surface_policy;
pub mod btx_raster_translation_manifest;
pub(crate) mod course_map_surface;
pub(crate) mod course_result_surface;
pub mod drac_glyph_patch;
pub mod e6_demo_font_patch;
pub mod fixed_cell_text_patch;
pub mod g_mess_translation;
pub mod general_raster_development_surface;
pub mod inventory;
pub mod mcrd01_development_surface;
pub mod menu06_glyph_patch;
pub mod mm_mes_family;
pub mod mm_mes_grid_stress;
pub mod mm_spr_patch;
pub mod mm_spr_translation;
pub mod mmenu_surface;
pub(crate) mod mode_help_surface;
pub mod official_card_glyphs;
pub mod official_name_commands;
pub mod official_records_surface;
pub mod p2_info_translation;
pub mod qmp;
pub mod rank_s_development_surface;
pub mod rpg_sys1_level_up_surface;
pub mod runtime_layout;
pub mod source_background;
pub mod source_masked_text_surface;
pub mod sparse_selector_surface;
pub mod text_candidates;
pub mod text_catalog;
pub mod text_patch;
pub mod text_profile;
pub mod translation_coverage;
pub mod tuu_cel_texture;

pub use fixed_cell_text_patch::{
    build_p2_info_page_capacity_stress_text, patch_fixed_cell_text_page,
    verify_box_p2_info_page_patch_strategy, BoxP2InfoPagePatchStrategyReport,
    FixedCellTextPatchError, FixedCellTextPatchReport, FixedCellTextStressExpectation,
    InfoPageCapacityStressText, BOX_P2_INFO_PAGE_PHYSICAL_ASSETS,
    P2_INFO_PAGE_CAPACITY_STRESS_CELLS, P2_INFO_PAGE_CAPACITY_STRESS_EXPECTATIONS,
};
pub use mm_mes_grid_stress::{
    stress_patch_mm_mes_grid, MmMesGridStressError, MmMesGridStressMapping, MmMesGridStressPatch,
};
pub mod tuu_cel;

pub mod generated_menu_surface;

pub(crate) mod special_skill_surface;
pub(crate) mod special_skill_text;
pub(crate) mod subgame_help;

pub(crate) mod style_selection_surface;

mod level_team_blocks;

mod official_menu_blocks;
pub(crate) mod official_menu_surface;

mod battle_badge_surface;
mod rule_logo_surface;

pub(crate) mod classic_graphics_surface;
pub(crate) mod common_chain_surface;
pub(crate) mod generated_title_surface;

pub(crate) mod small_battle_badge_surface;
