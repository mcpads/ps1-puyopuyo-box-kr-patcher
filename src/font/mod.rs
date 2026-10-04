//! 한글 글리프 원천과 아틀라스 생성 경계.
//!
//! 사용자가 지정한 Maplestory Light 원본과 QUEST의 고정 아틀라스 layout을
//! 검증한다. 다른 EXE의 폰트 layout에는 이 계약을 이식하지 않는다.

pub mod cell_table;
pub mod common;
pub mod common_text;
pub mod demo;
pub mod drac;
pub mod e6_demo;
pub mod info_page;
pub mod menu06;
pub mod mm_mes_storage_stress;
pub mod quest;
pub mod scene;
pub mod source;

pub use cell_table::{
    parse_runtime_cell_selection, parse_runtime_cell_table, render_runtime_cell_table_tsv,
    RuntimeCell, RuntimeCellTable, RuntimeCellTableError,
};
pub use common::{
    verify_box_common_fonts, CommonFontError, CommonFontSpec, CommonFontSummary,
    BOX_COMMON_FONT_SPECS,
};
pub use common_text::{
    render_common_font_text_surfaces_tsv, verify_box_common_font_text_surfaces,
    CommonFontTextSurface, CommonFontTextSurfaceError, CommonFontTextSurfaceReport,
    CommonFontTextSurfaceSpec, BOX_COMMON_FONT_TEXT_SURFACES,
    COMMON_FONT_TEXT_SURFACE_SCHEMA_VERSION,
};
pub use demo::{
    verify_box_demo_font, DemoFontAtlas, DemoFontError, DemoFontSlot, DemoFontSummary,
    DEMO_FONT_LAYOUT, DEMO_FONT_SHA256, DEMO_FONT_SLOT_COUNT,
};
pub use drac::{
    rasterize_drac_glyph, DracFontError, DracRasterEffect, DracRasterProfile,
    DRAC_GLYPH_CELL_HEIGHT, DRAC_GLYPH_CELL_WIDTH, DRAC_POC_RASTER_PROFILE,
};
pub use e6_demo::{
    rasterize_e6_demo_glyph, rasterize_e6_demo_glyph_with_geometry, E6DemoFontError,
    E6DemoGlyphGeometry, E6DemoGlyphRaster, E6DemoRasterEffect, E6DemoRasterProfile,
    E6_DEMO_GLYPH_CELL_HEIGHT, E6_DEMO_GLYPH_CELL_WIDTH, E6_DEMO_POC_RASTER_PROFILE,
};
pub use info_page::{
    rasterize_info_page_glyph, InfoPageFontError, InfoPageRasterEffect, InfoPageRasterProfile,
    INFO_PAGE_GLYPH_CELL_HEIGHT, INFO_PAGE_GLYPH_CELL_WIDTH, P2_INFO_PAGE_DRAFT_RASTER_PROFILE,
};
pub use menu06::{
    rasterize_menu06_glyph, Menu06FontError, Menu06RasterEffect, Menu06RasterProfile,
    MENU06_DRAFT_RASTER_PROFILE, MENU06_GLYPH_CELL_HEIGHT, MENU06_GLYPH_CELL_WIDTH,
};
pub use mm_mes_storage_stress::{
    rasterize_mm_mes_storage_stress_glyph, MmMesStorageStressFontError,
    MmMesStorageStressRasterEffect, MmMesStorageStressRasterProfile,
    MM_MES_STORAGE_STRESS_RASTER_PROFILE, MM_MES_STRESS_CELL_HEIGHT, MM_MES_STRESS_CELL_WIDTH,
};
pub use quest::{
    canonicalize_quest_display_glyph, canonicalize_quest_source_glyph, decode_quest_source_glyph,
    normalize_quest_display_text, quest_poc_code, quest_poc_font_candidates, rasterize_wide_glyph,
    render_quest_poc_font_plan_tsv, verify_quest_poc_font_plan, QuestCodeTable,
    QuestDisplayTextError, QuestFontError, QuestGlyphSlot, QuestPocFontCandidate,
    QuestPocFontCandidateStatus, QuestPocFontPlanReport, QuestPocGlyphMapping, QuestRasterEffect,
    QuestRasterProfile, QUEST_CODE_TABLE_BYTES, QUEST_CODE_TABLE_COUNT, QUEST_CODE_TABLE_RUNTIME,
    QUEST_DOUBLE_EXCLAMATION_CODE, QUEST_DOUBLE_EXCLAMATION_REVIEW_GLYPH,
    QUEST_DOUBLE_QUESTION_CODE, QUEST_DOUBLE_QUESTION_REVIEW_GLYPH, QUEST_FONT_ASSET_NAME,
    QUEST_FONT_CELL_HEIGHT, QUEST_FONT_FIRST_ROW_WIDE_SLOTS, QUEST_FONT_LAYOUT,
    QUEST_FONT_NARROW_CELL_WIDTH, QUEST_FONT_NARROW_SLOTS, QUEST_FONT_SHA256,
    QUEST_FONT_WIDE_CELL_WIDTH, QUEST_FONT_WIDE_COLUMNS, QUEST_FONT_WIDE_SLOTS,
    QUEST_POC_FONT_PLAN_SCHEMA_VERSION, QUEST_POC_GLYPH_MAPPINGS, QUEST_POC_RASTER_PROFILE,
    QUEST_POC_UNOBSERVED_CODE_COUNT, QUEST_POC_WIDE_CANDIDATE_COUNT,
};
pub use scene::{
    rasterize_scene_glyph, SceneFontError, SceneRasterEffect, SceneRasterProfile,
    SCENE_GLYPH_CELL_HEIGHT, SCENE_GLYPH_CELL_WIDTH, SCENE_POC_RASTER_PROFILE,
    SCENE_TRANSLATION_DRAFT_RASTER_PROFILE,
};
pub use source::{
    bundled_maplestory_light, load_maplestory_light, FontSourceError, MAPLESTORY_LIGHT_FONT_ID,
    MAPLESTORY_LIGHT_LEN, MAPLESTORY_LIGHT_SHA1, MAPLESTORY_LIGHT_SHA256,
};

pub mod selected;
