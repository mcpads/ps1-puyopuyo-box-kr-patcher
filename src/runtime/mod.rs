//! 에뮬레이터 실행 증거를 원본 파일과 연결하는 읽기 전용 도구.
//!
//! 제품 빌드 입력을 만들지 않는다. 정확한 state 파일에 관측값을
//! 귀속하고, 포맷이나 범위가 다르면 부분 결과 없이 실패한다.

pub mod btx_pending;
pub mod common_font_direct_draw;
pub mod common_font_draw;
pub mod common_font_writer_draw;
pub mod mednafen_state;
pub mod quest_font_scene;
pub mod quest_font_windows;
pub mod quest_item_container_contact;
pub mod quest_map_collision;
pub mod quest_map_route;
pub mod quest_map_walk_route;
pub mod subgame_seq_drac;
pub mod subgame_seq_e6;
pub mod subgame_seq_text_bridge;
pub mod subgame_seq_text_bridge_call;

pub use btx_pending::{
    inspect_pending_btx_runtime, render_pending_btx_runtime_tsv, PendingBtxRuntimeError,
    PendingBtxRuntimeObservation, PendingBtxRuntimeRow, PendingBtxRuntimeSource,
    BTX_DESCRIPTOR_IDENTITY_BYTES, BTX_PENDING_RUNTIME_SCHEMA_VERSION,
    BTX_SELECTION_CODE_MIN_BYTES,
};
pub use common_font_direct_draw::{
    inspect_common_font_direct_draw, render_common_font_direct_draw_tsv, CommonFontDirectDrawError,
    CommonFontDirectDrawObservationInput, CommonFontDirectDrawReport,
    COMMON_FONT_DIRECT_DRAW_SCHEMA_VERSION,
};
pub use common_font_draw::{
    inspect_common_font_draw, render_common_font_draw_tsv, CommonFontDrawError,
    CommonFontDrawObservationInput, CommonFontDrawReport, CommonFontDrawRow,
    COMMON_FONT_DRAW_SCHEMA_VERSION,
};
pub use common_font_writer_draw::{
    inspect_common_font_writer_draw, render_common_font_writer_draw_tsv, CommonFontWriterDrawError,
    CommonFontWriterDrawObservationInput, CommonFontWriterDrawReport, CommonFontWriterDrawRow,
    COMMON_FONT_WRITER_DRAW_SCHEMA_VERSION,
};
pub use mednafen_state::{MednafenPsxState, MednafenStateError};
pub use quest_font_scene::{
    inspect_town00_quest_font_scenes, QuestFontSceneCandidate, QuestFontSceneCensus,
    QuestFontSceneCensusError,
};
pub use quest_font_windows::{
    inspect_quest_font_windows, inspect_quest_font_windows_ram,
    render_quest_font_window_geometry_tsv, QuestFontWindowCensus, QuestFontWindowCensusError,
    QuestFontWindowGeometry, QuestFontWindowSlot, QUEST_FONT_WINDOW_ACTIVE_OFFSET,
    QUEST_FONT_WINDOW_CELL_HEIGHT_OFFSET, QUEST_FONT_WINDOW_CELL_WIDTH_OFFSET,
    QUEST_FONT_WINDOW_CURSOR_X_OFFSET, QUEST_FONT_WINDOW_CURSOR_Y_OFFSET,
    QUEST_FONT_WINDOW_GEOMETRY_SCHEMA_VERSION, QUEST_FONT_WINDOW_GLYPH_SPACING_OFFSET,
    QUEST_FONT_WINDOW_INPUT_INDICATOR_OFFSET, QUEST_FONT_WINDOW_LINE_HEIGHT_OFFSET,
    QUEST_FONT_WINDOW_LINE_SPACING_OFFSET, QUEST_FONT_WINDOW_ORIGIN_X_OFFSET,
    QUEST_FONT_WINDOW_ORIGIN_Y_OFFSET, QUEST_FONT_WINDOW_SELECTION_RENDERER_OFFSET,
};
pub use quest_item_container_contact::{
    inspect_quest_item_container_contact_state, QuestItemContainerContactError,
    QuestItemContainerContactReport, QuestItemContainerRuntimeObject,
};
pub use quest_map_collision::{
    inspect_quest_map_collision_state, QuestMapCollisionClearedCell, QuestMapCollisionSource,
    QuestMapCollisionSourceMatch, QuestMapCollisionStateError, QuestMapCollisionStateReport,
    QuestMapCollisionStationaryProbe,
};
pub use quest_map_route::{
    find_direction_robust_quest_map_route,
    find_direction_robust_quest_map_route_avoiding_rectangles, QuestMapRoute, QuestMapRouteError,
    QUEST_MAP_ROUTE_DIRECTION_BRANCH_REPRESENTATIVES,
};
pub use quest_map_walk_route::{
    find_direction_robust_quest_map_walk_route_avoiding_rectangles,
    find_direction_robust_quest_map_walk_route_to_any_avoiding_rectangles, QuestMapWalkRoute,
    QuestMapWalkRouteError,
};
pub use subgame_seq_drac::{
    inspect_puyo2_drac_ram, render_puyo2_drac_ram_tsv, Puyo2DracCurrentRecordCandidate,
    Puyo2DracGlyphObjectCandidate, Puyo2DracMessageObjectCandidate, Puyo2DracRamError,
    Puyo2DracRamObservation, PUYO2_DRAC_BSS_BYTES, PUYO2_DRAC_COMMAND_RECORDS,
    PUYO2_DRAC_GLYPH_ACTIVE_FIELD, PUYO2_DRAC_GLYPH_CALLBACK_FIELD, PUYO2_DRAC_GLYPH_CELL_FIELD,
    PUYO2_DRAC_GLYPH_CELL_FIRST, PUYO2_DRAC_GLYPH_CELL_LAST, PUYO2_DRAC_GLYPH_KIND_FIELD,
    PUYO2_DRAC_GLYPH_RESOURCE, PUYO2_DRAC_GLYPH_RESOURCE_FIELD,
    PUYO2_DRAC_GLYPH_SAME_NAMED_RESOURCE, PUYO2_DRAC_GLYPH_UPDATE_RUNTIME,
    PUYO2_DRAC_INITIALIZED_BYTES, PUYO2_DRAC_MESSAGE_CALLBACK_FIELD,
    PUYO2_DRAC_MESSAGE_UPDATE_RUNTIME, PUYO2_DRAC_OVERLAY_RUNTIME,
    PUYO2_DRAC_SCRIPT_CURRENT_RECORD_FIELD, PUYO2_DRAC_SEQUENCE_BYTES,
    SUBGAME_SEQ_DRAC_RAM_SCHEMA_VERSION,
};
pub use subgame_seq_e6::{
    inspect_puyo2_e6_ram, render_puyo2_e6_ram_tsv, Puyo2E6CurrentRecordCandidate,
    Puyo2E6GlyphObjectCandidate, Puyo2E6OverlayMatch, Puyo2E6RamError, Puyo2E6RamObservation,
    Puyo2E6RuntimeSource, PUYO2_E6_COMMAND_HANDLER_RUNTIME, PUYO2_E6_COMPLETION_CALLBACK_RUNTIME,
    PUYO2_E6_DESCRIPTOR_BASE_RUNTIME, PUYO2_E6_DESCRIPTOR_DESTINATION_FIELD,
    PUYO2_E6_DESCRIPTOR_STRIDE, PUYO2_E6_GLYPH_CALLBACK_RUNTIME, PUYO2_E6_GLYPH_RESOURCE,
    PUYO2_E6_MAX_CELL, PUYO2_E6_MIN_CELL, PUYO2_E6_OBJECT_ACTIVE_FIELD, PUYO2_E6_OBJECT_BYTES,
    PUYO2_E6_OBJECT_CALLBACK_FIELD, PUYO2_E6_OBJECT_CELL_FIELD,
    PUYO2_E6_OBJECT_CURRENT_RECORD_FIELD, PUYO2_E6_OBJECT_OVERRIDE_FIELD,
    PUYO2_E6_OBJECT_POOL_RUNTIME, PUYO2_E6_OBJECT_RESOURCE_FIELD, PUYO2_E6_OBJECT_SLOTS,
    PUYO2_E6_OBJECT_TYPE, PUYO2_E6_OBJECT_TYPE_FIELD, PUYO2_E6_OVERLAY_RUNTIME,
    PUYO2_E6_PARSER_RUNTIME, PUYO2_E6_RESOURCE_FIRST, PUYO2_E6_RESOURCE_LAST,
    PUYO2_E6_SELECTOR_RUNTIME, PUYO2_E6_SEQUENCE_COUNT, PUYO2_E6_SHORT_RENDERER_RUNTIME,
    PUYO2_E6_USED_CELL_COUNT, SUBGAME_SEQ_E6_RAM_SCHEMA_VERSION,
};
pub use subgame_seq_text_bridge::{
    inspect_box_subgame_seq_text_bridge_ram, inspect_puyo2_subgame_seq_text_bridge_ram,
    render_subgame_seq_text_bridge_ram_tsv, SubgameSeqTextBridgeOutsideSlotObservation,
    SubgameSeqTextBridgeRamError, SubgameSeqTextBridgeRamObservation, PS1_MAIN_RAM_BYTES,
    SUBGAME_SEQ_TEXT_BRIDGE_RAM_SCHEMA_VERSION,
};
pub use subgame_seq_text_bridge_call::{
    inspect_box_subgame_seq_text_bridge_call, render_subgame_seq_text_bridge_calls_tsv,
    SubgameSeqTextBridgeCallError, SubgameSeqTextBridgeCallObservation,
    SUBGAME_SEQ_TEXT_BRIDGE_CALL_SCHEMA_VERSION,
};
