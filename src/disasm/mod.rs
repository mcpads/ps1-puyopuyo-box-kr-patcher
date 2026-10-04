//! 정적 분석 — 디스어셈블과 참조 그래프.
//!
//! 세 층이 서로 다른 것을 주장한다. 섞으면 안 된다.
//!
//! | 층 | 주장 | 한계 |
//! |----|------|------|
//! | [`linear`] | 이 바이트를 명령으로 읽으면 이렇다 | 데이터를 코드로 오인 |
//! | [`recursive`] | 엔트리에서 여기까지 흐름이 닿는다 | 간접 분기 너머 못 감 |
//! | [`analysis`] | 이 주소를 누가/무엇을 참조한다 | 정적 참조만 |
//!
//! 어느 층도 "도달 불가"를 증명하지 못한다. 미도달은 후보일 뿐이다.

pub mod analysis;
pub mod bios_font;
pub mod btx_cell;
pub mod btx_descriptor_select;
pub mod btx_names;
pub mod cnx_consumer;
pub mod common_font_cell_source;
pub mod common_font_direct_draw;
pub mod common_font_draw;
pub mod common_font_fixed_draw;
pub mod demo_font;
pub mod exe_strings;
pub mod linear;
pub mod outgoing_stack_arguments;
pub mod quest_breakable_contact;
pub mod quest_control;
pub mod quest_escape_outcome;
pub mod quest_font;
pub mod quest_font_hook;
pub mod quest_font_object_lifetime;
pub mod quest_hook_candidates;
pub mod quest_hook_space;
pub mod quest_input_wait;
pub mod quest_item_acquisition;
pub mod quest_item_container;
pub mod quest_item_description_selection;
pub mod quest_item_name_callback;
pub mod quest_item_text_consumer;
pub mod quest_map_collision;
pub mod quest_page_carrier;
pub mod quest_page_source;
pub mod quest_player_attack_contact;
pub mod quest_player_contact;
mod quest_player_input;
pub mod quest_player_interaction_contact;
pub mod quest_resident_seq_prerequisite;
pub mod quest_resident_seq_roots;
pub mod quest_resident_seq_selector;
pub mod quest_seq;
pub mod quest_seq_advance;
pub mod quest_seq_base_advance;
pub mod quest_seq_base_compare;
pub mod quest_seq_base_record;
pub mod quest_seq_base_transition;
pub mod quest_seq_child_activation;
pub mod quest_seq_child_configuration;
pub mod quest_seq_child_stream;
pub mod quest_seq_flag_query;
pub mod quest_seq_global_word_assignment;
pub mod quest_seq_loader;
pub mod quest_seq_op28;
pub mod quest_seq_registration;
pub mod quest_seq_transition;
pub mod quest_sound_sequence;
pub mod quest_text_mode;
pub mod quest_text_window;
pub mod quest_ui_literals;
pub mod recursive;
pub mod shell_exec;
pub mod subgame_seq_control;
pub mod subgame_seq_control_target;
pub mod subgame_seq_descriptor;
pub mod subgame_seq_descriptor_ingress;
pub mod subgame_seq_drac_consumer;
pub mod subgame_seq_drac_glyph;
pub mod subgame_seq_e6_glyph;
pub mod subgame_seq_e6_text;
pub mod subgame_seq_ending_route;
pub mod subgame_seq_load;
pub mod subgame_seq_master_route;
pub mod subgame_seq_pcomi_admission_producers;
pub mod subgame_seq_pcomi_custom_byte_atlas;
pub mod subgame_seq_pcomi_custom_byte_consumer;
pub mod subgame_seq_pcomi_glyph_code;
pub mod subgame_seq_pcomi_load;
pub mod subgame_seq_registration;
pub mod subgame_seq_selection;
pub mod subgame_seq_text_bridge;
pub mod subgame_seq_text_consumer;
pub mod trace_inventory;
pub mod vdd_registration;
pub mod xref;

pub use analysis::{data_reference_report, Ref, RefGraph, RefKind};
pub use bios_font::{
    audit_bios_font_routes, render_bios_font_analysis_tsv, verify_box_bios_font_routes,
    BiosFontAnalysis, BiosFontCallsiteSimilarity, BiosFontError, BiosFontRouteSpec,
    BiosFontRouteSummary, BOX_BIOS_FONT_ROUTES,
};
pub use btx_cell::{
    audit_btx_cell_consumers, render_btx_cell_consumers_tsv, verify_box_btx_cell_consumers,
    BtxCellAnalysis, BtxCellConsumer, BtxCellConsumerSummary, BtxCellError, BtxRendererCoreSpec,
    BtxRendererFamily, BOX_BTX_CELL_ANALYSIS_SHA256, BOX_BTX_RENDERER_CORES,
};
pub use btx_descriptor_select::{
    verify_box_btx_descriptor_selections, BtxDescriptorSelection, BtxDescriptorSelectionAnalysis,
    BtxDescriptorSelectionError, BtxDescriptorSelectionSpec, BOX_PENDING_BTX_DESCRIPTOR_SELECTIONS,
};
pub use btx_names::{
    audit_btx_name_consumers, render_btx_name_consumers_tsv, verify_box_btx_name_consumers,
    BtxNameAnalysis, BtxNameConsumerSpec, BtxNameConsumerSummary, BtxNameError, BtxNameLink,
    BOX_BTX_NAME_ANALYSIS_SHA256, BOX_BTX_NAME_CONSUMERS,
};
pub use cnx_consumer::{
    verify_box_cnx_consumers, CnxConsumerError, CnxConsumerSummary, CnxCoreFamily, CnxPathSpec,
    ExeCnxConsumerSpec, BOX_CNX_CONSUMERS,
};
pub use common_font_cell_source::{
    analyze_common_font_cell_sources, render_common_font_cell_sources_tsv,
    verify_box_common_font_cell_sources, CommonFontCellSourceAnalysis, CommonFontCellSourceError,
    CommonFontCellSourceRow, BOX_COMMON_FONT_CELL_SOURCE_SHA256,
};
pub use common_font_direct_draw::{
    verify_quest_common_font_direct_draw, CommonFontDirectDrawSite, CommonFontDirectDrawSiteError,
    QUEST_CHAIN_LABEL_CALLBACK_FIELD_OFFSET, QUEST_CHAIN_LABEL_CELL, QUEST_CHAIN_LABEL_CELL_COUNT,
    QUEST_CHAIN_LABEL_CELL_FIELD_OFFSET, QUEST_CHAIN_LABEL_COMMAND_BUFFER_RUNTIME,
    QUEST_CHAIN_LABEL_COMMAND_COUNT_GP_OFFSET, QUEST_CHAIN_LABEL_DESCRIPTOR_ROOT_POINTER_RUNTIME,
    QUEST_CHAIN_LABEL_DESCRIPTOR_RUNTIME, QUEST_CHAIN_LABEL_DESCRIPTOR_TABLE_POINTER_OFFSET,
    QUEST_CHAIN_LABEL_DRAW_COMMIT_RUNTIME, QUEST_CHAIN_LABEL_DRAW_HANDOFF_RUNTIME,
    QUEST_CHAIN_LABEL_DRAW_RUNTIME, QUEST_CHAIN_LABEL_DRAW_SOURCE_BYTES,
    QUEST_CHAIN_LABEL_DRAW_SOURCE_SHA256, QUEST_CHAIN_LABEL_RESOURCE,
    QUEST_CHAIN_LABEL_RESOURCE_FIELD_OFFSET, QUEST_CHAIN_LABEL_RESOURCE_STORE_RUNTIME,
    QUEST_CHAIN_LABEL_WRITER_POST_RUNTIME, QUEST_CHAIN_LABEL_WRITER_RUNTIME,
};
pub use common_font_draw::{
    analyze_common_font_draw_sites, render_common_font_draw_sites_tsv,
    verify_box_common_font_draw_sites, CommonFontDrawSite, CommonFontDrawSiteAnalysis,
    CommonFontDrawSiteError, BOX_COMMON_FONT_DRAW_SITE_SHA256, BOX_COMMON_FONT_DRAW_SOURCES,
};
pub use common_font_fixed_draw::{
    audit_common_font_fixed_draws, render_common_font_fixed_draws_tsv,
    verify_box_common_font_fixed_draws, CommonFontFixedDrawAnalysis, CommonFontFixedDrawError,
    CommonFontFixedDrawSite, BOX_COMMON_FONT_FIXED_DRAW_SHA256,
};
pub use demo_font::{
    verify_box_demo_font_consumer, DemoFontConsumerError, DemoFontConsumerSummary,
    DEMO_FONT_CODE_TABLE_ENTRIES, DEMO_FONT_CODE_TABLE_RUNTIME, DEMO_FONT_CODE_TABLE_SHA256,
    DEMO_FONT_DESCRIPTOR_RUNTIME, DEMO_FONT_RESOURCE_INDEX, DEMO_FONT_RESOURCE_TABLE_RUNTIME,
};
pub use exe_strings::{
    audit_exe_string_candidates, render_exe_string_candidates_tsv,
    verify_box_exe_string_candidates, BoxExeStringExpectation, ExeStringAnalysis,
    ExeStringCandidate, ExeStringError, ExeStringSummary, BOX_EXE_STRING_ANALYSIS_SHA256,
    BOX_EXE_STRING_EXPECTATIONS,
};
pub use quest_breakable_contact::{
    verify_box_quest_breakable_contact, QuestBreakableContactError, QuestBreakableContactSummary,
    QUEST_BREAKABLE_ATTACK_CONTACT_MASK, QUEST_BREAKABLE_CONTACT_CALLBACK_RUNTIME,
    QUEST_BREAKABLE_CONTACT_FLAGS_OFFSET, QUEST_BREAKABLE_RELEASE_CALL_RUNTIME,
    QUEST_BREAKABLE_STATE_OFFSET,
};
pub use quest_control::{
    verify_box_quest_control_meanings, QuestControlMeaningError, QuestControlMeaningSummary,
    QUEST_ADVANCE_LINE_RUNTIME, QUEST_ALLOCATE_OBJECT_RUNTIME, QUEST_CLOSE_TEXT_WINDOW_RUNTIME,
    QUEST_FIXED_TEXT_POINTER_RUNTIME, QUEST_OPEN_TEXT_WINDOW_RUNTIME, QUEST_RELEASE_OBJECT_RUNTIME,
    QUEST_SEQ_DISPATCH_YIELD_GP_OFFSET,
};
pub use quest_escape_outcome::{
    classify_quest_escape_outcome, verify_box_quest_escape_outcome, QuestEscapeOutcomeError,
    QuestEscapeOutcomeRule, QuestEscapeOutcomeSummary, QUEST_ESCAPE_AGGREGATE_VALUE_RUNTIME,
    QUEST_ESCAPE_COMPARISON_VALUE_RUNTIME, QUEST_ESCAPE_FAILURE_FLAGS_RUNTIME,
    QUEST_ESCAPE_FAILURE_FLAG_MASK, QUEST_ESCAPE_FORCED_FAILURE_STATE_RUNTIME,
    QUEST_ESCAPE_OUTCOME_FIELD_OFFSET, QUEST_ESCAPE_OUTCOME_HANDLER_RUNTIME,
    QUEST_ESCAPE_OUTCOME_OPCODE, QUEST_ESCAPE_OUTCOME_RECORD_BYTES,
    QUEST_ESCAPE_PARTICIPANT_STATE_RUNTIME, QUEST_ESCAPE_PARTICIPANT_TABLE_RUNTIME,
    QUEST_ESCAPE_RANDOM_RUNTIME, QUEST_ESCAPE_STATE_BASE_RUNTIME,
};
pub use quest_font::{
    verify_box_quest_font_consumer, QuestFontConsumerError, QuestFontConsumerSummary,
    QUEST_FONT_ASSET_NAME_RUNTIME, QUEST_FONT_ATLAS_HEIGHT, QUEST_FONT_ATLAS_WIDTH,
    QUEST_FONT_CELL_TABLE_RUNTIME, QUEST_FONT_COMPRESSED_BYTES, QUEST_FONT_DESCRIPTOR_AUX_OFFSET,
    QUEST_FONT_DESCRIPTOR_CELL_TABLE_OFFSET, QUEST_FONT_DESCRIPTOR_COMPRESSED_BYTES_OFFSET,
    QUEST_FONT_DESCRIPTOR_COPY_RUNTIME, QUEST_FONT_DESCRIPTOR_DECODED_BYTES_OFFSET,
    QUEST_FONT_DESCRIPTOR_DECODED_STORE_RUNTIME, QUEST_FONT_DESCRIPTOR_INITIAL_FLAGS,
    QUEST_FONT_DESCRIPTOR_NAME_OFFSET, QUEST_FONT_DESCRIPTOR_PIXEL_FORMAT_OFFSET,
    QUEST_FONT_DESCRIPTOR_ROOT_RUNTIME, QUEST_FONT_DESCRIPTOR_RUNTIME,
    QUEST_FONT_DESCRIPTOR_SOURCE_OFFSET, QUEST_FONT_DESCRIPTOR_STAGING_STORE_RUNTIME,
    QUEST_FONT_DESCRIPTOR_STRIDE, QUEST_FONT_DESCRIPTOR_VRAM_X_OFFSET,
    QUEST_FONT_DESCRIPTOR_VRAM_Y_OFFSET, QUEST_FONT_INITIAL_RESOURCE_SELECTOR,
    QUEST_FONT_PIXEL_FORMAT_INDEXED4, QUEST_FONT_SOURCE_BYTES, QUEST_FONT_STATE_BASE_RUNTIME,
    QUEST_FONT_STATE_BOOT_CALL_RUNTIME, QUEST_FONT_STATE_BYTES,
    QUEST_FONT_STATE_CONDITIONAL_RESET_CALL_RUNTIME, QUEST_FONT_STATE_INITIALIZER_RUNTIME,
    QUEST_FONT_STATE_RECORD_COUNT, QUEST_FONT_STATE_RECORD_STRIDE,
    QUEST_FONT_UPLOAD_CALLBACK_RUNTIME, QUEST_FONT_UPLOAD_CALL_RUNTIME,
    QUEST_FONT_UPLOAD_SYNC_CALL_RUNTIME, QUEST_FONT_UPLOAD_SYNC_NAME_RUNTIME,
    QUEST_FONT_UPLOAD_SYNC_TARGET_RUNTIME, QUEST_FONT_UPLOAD_TARGET_RUNTIME,
    QUEST_FONT_VRAM_WIDTH_WORDS, QUEST_FONT_VRAM_X_WORDS, QUEST_FONT_VRAM_Y,
    QUEST_GLYPH_CELL_INDEX_OBJECT_OFFSET, QUEST_GLYPH_CELL_INDEX_STORE_RUNTIME,
    QUEST_GLYPH_CELL_RECORD_STRIDE, QUEST_GLYPH_CELL_STRIDE_RUNTIME,
    QUEST_GLYPH_CELL_TABLE_DESCRIPTOR_OFFSET, QUEST_GLYPH_CELL_TABLE_LOAD_RUNTIME,
    QUEST_GLYPH_CODE_TABLE_LIMIT_RUNTIME, QUEST_GLYPH_CODE_TABLE_POINTER_RUNTIME,
    QUEST_GLYPH_FALLBACK_INDEX, QUEST_GLYPH_FALLBACK_INDEX_RUNTIME,
    QUEST_GLYPH_NARROW_BOUNDARY_RUNTIME, QUEST_GLYPH_RENDERER_CONTINUATION_RUNTIME,
    QUEST_GLYPH_RENDERER_RUNTIME, QUEST_GLYPH_RESOURCE_SELECTOR_LOAD_RUNTIME,
    QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET,
};
pub use quest_font_hook::{
    verify_box_quest_font_hook_site, QuestFontHookSiteError, QuestFontHookSiteSummary,
    QUEST_FONT_HOOK_CALLER_DELAY_RUNTIME, QUEST_FONT_HOOK_CALLER_JALR_RUNTIME,
    QUEST_FONT_HOOK_CALLER_LOAD_RUNTIME, QUEST_FONT_HOOK_CALLER_RETURN_RUNTIME,
    QUEST_FONT_HOOK_CONTINUATION_RUNTIME, QUEST_FONT_HOOK_HANDLER_BYTES,
    QUEST_FONT_HOOK_HANDLER_END_RUNTIME, QUEST_FONT_HOOK_OVERWRITE_BYTES,
    QUEST_FONT_HOOK_PRESERVED_DELAY_RUNTIME, QUEST_FONT_HOOK_SITE_RUNTIME,
};
pub use quest_font_object_lifetime::{
    verify_box_quest_font_object_lifetime, QuestFontObjectLifetimeError,
    QuestFontObjectLifetimeSummary, QUEST_FONT_DRAW_DESCRIPTOR_SITE_RUNTIME,
    QUEST_FONT_DRAW_LIST_HEAD_RUNTIME, QUEST_FONT_DRAW_LIST_SENTINEL_RUNTIME,
    QUEST_FONT_DRAW_QUAD_PATH_SCALE_X_LOAD_RUNTIME, QUEST_FONT_DRAW_RUNTIME,
    QUEST_FONT_DRAW_SHORT_PATH_SCALE_X_LOAD_RUNTIME, QUEST_FONT_DRAW_STATE_INDEX_LOAD_RUNTIME,
    QUEST_FONT_DRAW_TEXTURE_PAGE_SITE_RUNTIME, QUEST_FONT_OBJECT_CELL_INDEX_OFFSET,
    QUEST_FONT_OBJECT_FLAGS_OFFSET, QUEST_FONT_OBJECT_LIST_HEAD_RUNTIME,
    QUEST_FONT_OBJECT_LIST_SENTINEL_RUNTIME, QUEST_FONT_OBJECT_MARKER_SITE_RUNTIME,
    QUEST_FONT_OBJECT_NEXT_OFFSET, QUEST_FONT_OBJECT_PAGE_MARKER_OFFSET,
    QUEST_FONT_OBJECT_SCALE_X_INITIALIZE_RUNTIME, QUEST_FONT_OBJECT_SCALE_X_OFFSET,
    QUEST_FONT_OBJECT_STATE_INDEX_OFFSET, QUEST_FONT_OBJECT_TYPE, QUEST_OBJECT_ALLOCATE_RUNTIME,
    QUEST_OBJECT_CALLBACK_FIELD_OFFSET, QUEST_OBJECT_POOL_RECORD_BYTES,
    QUEST_OBJECT_POOL_RECORD_COUNT, QUEST_OBJECT_POOL_RUNTIME, QUEST_OBJECT_RELEASE_RUNTIME,
};
pub use quest_hook_candidates::{
    audit_quest_hook_space_candidates, render_quest_hook_space_candidates_tsv,
    verify_box_quest_hook_space_candidates, QuestHookCandidateAnalysis, QuestHookCandidateError,
    QuestHookCandidateStaticVerdict, QuestHookSpaceCandidate,
    BOX_QUEST_HOOK_CANDIDATE_ANALYSIS_SHA256, QUEST_HOOK_CANDIDATE_MIN_BYTES,
};
pub use quest_hook_space::{
    verify_box_quest_hook_space, QuestHookSpaceError, QuestHookSpaceSummary, QUEST_HOOK_GP_RUNTIME,
    QUEST_HOOK_INITIALIZED_END_RUNTIME, QUEST_HOOK_LAST_INITIALIZED_WORD_RUNTIME,
    QUEST_HOOK_MUTABLE_SELECTOR_RUNTIME, QUEST_HOOK_SECTION_TABLE_RUNTIME,
    QUEST_HOOK_TEXT_END_RUNTIME, QUEST_HOOK_ZERO_CLEAR_END_RUNTIME,
    QUEST_HOOK_ZERO_CLEAR_START_RUNTIME,
};
pub use quest_input_wait::{
    verify_box_quest_input_wait, QuestInputWaitError, QuestInputWaitSummary,
    QUEST_INPUT_WAIT_BUTTON_MASK, QUEST_INPUT_WAIT_HANDLER_RUNTIME,
    QUEST_INPUT_WAIT_INDICATOR_OFFSET, QUEST_INPUT_WAIT_INITIAL_COUNT,
    QUEST_INPUT_WAIT_POLL_RUNTIME, QUEST_INPUT_WAIT_START_RUNTIME, QUEST_INPUT_WAIT_STATE_OFFSET,
    QUEST_INPUT_WAIT_STREAM_BYTES,
};
pub use quest_item_acquisition::{
    audit_quest_item_acquisition, verify_box_quest_item_acquisition,
    QuestItemAcquisitionConsumerEvidence, QuestItemAcquisitionError, QuestItemAcquisitionMessage,
    QuestItemAcquisitionMessageRole, QuestItemAcquisitionSummary,
    QUEST_ITEM_ACQUISITION_DRAW_CALLBACK_RUNTIME,
    QUEST_ITEM_ACQUISITION_FIXED_MESSAGE_CALL_RUNTIMES, QUEST_ITEM_ACQUISITION_HANDLER_RUNTIME,
    QUEST_ITEM_ACQUISITION_ITEM_ID_STATE_RUNTIME, QUEST_ITEM_ACQUISITION_OBJECT_STATE_OFFSET,
    QUEST_ITEM_ACQUISITION_OPCODE, QUEST_ITEM_ACQUISITION_PARAMETER_STATE_RUNTIME,
    QUEST_ITEM_ACQUISITION_RECORD_BYTES, QUEST_ITEM_ACQUISITION_RECORD_ITEM_ID_OFFSET,
    QUEST_ITEM_ACQUISITION_RECORD_PARAMETER_OFFSET, QUEST_ITEM_ACQUISITION_RECORD_SUBMODE_OFFSET,
    QUEST_ITEM_ACQUISITION_STATE_MACHINE_RUNTIME, QUEST_ITEM_ACQUISITION_SUBDISPATCH_TABLE_RUNTIME,
};
pub use quest_item_container::{
    verify_box_quest_item_container, QuestItemContainerError, QuestItemContainerSummary,
    QUEST_ITEM_CONTAINER_ATTACK_REACTION_MASK, QUEST_ITEM_CONTAINER_CALLBACK_RUNTIME,
    QUEST_ITEM_CONTAINER_HANDLER_RUNTIME, QUEST_ITEM_CONTAINER_ITEM_ID_FIELD_OFFSET,
    QUEST_ITEM_CONTAINER_LOCAL_RECTANGLE, QUEST_ITEM_CONTAINER_OBJECT_TYPE,
    QUEST_ITEM_CONTAINER_OPCODE, QUEST_ITEM_CONTAINER_OPEN_CONTACT_MASK,
    QUEST_ITEM_CONTAINER_OPEN_DIRECTION, QUEST_ITEM_CONTAINER_PARAMETER_FIELD_OFFSET,
    QUEST_ITEM_CONTAINER_PERSISTENCE_FIELD_OFFSET, QUEST_ITEM_CONTAINER_RECORD_BYTES,
    QUEST_ITEM_CONTAINER_RECORD_ITEM_ID_OFFSET, QUEST_ITEM_CONTAINER_RECORD_LAYER_OFFSET,
    QUEST_ITEM_CONTAINER_RECORD_PARAMETER_OFFSET, QUEST_ITEM_CONTAINER_RECORD_PERSISTENCE_OFFSET,
    QUEST_ITEM_CONTAINER_RECORD_X_OFFSET, QUEST_ITEM_CONTAINER_RECORD_Y_OFFSET,
};
pub use quest_item_description_selection::{
    verify_box_quest_item_description_selection, QuestItemDescriptionSelectionError,
    QuestItemDescriptionSelectionPath, QuestItemDescriptionSelectionSummary,
    QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_CALL_RUNTIMES,
};
pub use quest_item_name_callback::{
    verify_box_quest_item_name_callback, QuestItemNameCallbackError, QuestItemNameCallbackSummary,
    QUEST_ITEM_NAME_CALLBACK_EPILOGUE_RUNTIME, QUEST_ITEM_NAME_CALLBACK_RUNTIME,
    QUEST_ITEM_NAME_FALLBACK_GLYPH_WRITER_RETURN_RUNTIME,
    QUEST_ITEM_NAME_GLYPH_WRITER_RETURN_RUNTIME, QUEST_ITEM_NAME_RESIDENCY_CONTINUATION_RUNTIME,
    QUEST_ITEM_NAME_RESIDENCY_SITE_RUNTIME,
};
pub use quest_item_text_consumer::{
    verify_box_quest_item_text_consumer, QuestItemTextConsumerError, QuestItemTextConsumerSummary,
    QUEST_ITEM_DESCRIPTION_ACTIVE_STORE_RUNTIME, QUEST_ITEM_DESCRIPTION_CONSTRUCTOR_RUNTIME,
    QUEST_ITEM_DESCRIPTION_POINTER_FIELD_OFFSET, QUEST_ITEM_NAME_DIRECT_GLYPH_CALL_RUNTIMES,
    QUEST_ITEM_NAME_DRAW_ROUTE_RUNTIME, QUEST_ITEM_NAME_POINTER_FIELD_OFFSET,
    QUEST_ITEM_NAME_WHOLE_STRING_CALL_RUNTIMES, QUEST_ITEM_NAME_WHOLE_STRING_DRAW_RUNTIME,
    QUEST_ITEM_NAME_WHOLE_STRING_GLYPH_CALL_RUNTIME,
    QUEST_ITEM_NAME_WHOLE_STRING_GLYPH_RETURN_RUNTIME, QUEST_ITEM_RECORD_TABLE_RUNTIME,
};
pub use quest_map_collision::{
    classify_quest_collision_tile, verify_box_quest_map_collision, QuestCollisionClass,
    QuestCollisionClassifierContext, QuestMapCollisionError, QuestMapCollisionSummary,
    QUEST_COLLISION_CLASSIFIER_RUNTIME, QUEST_COLLISION_PLANE_OFFSET,
    QUEST_MAP_ACTIVE_SLOT_RUNTIME, QUEST_MAP_COLLISION_BUFFER_RUNTIME,
    QUEST_MAP_PLANE_CAPACITY_CELLS, QUEST_MAP_PLANE_STRIDE_BYTES,
};
pub use quest_page_carrier::{
    verify_box_quest_page_carrier, QuestPageCarrierError, QuestPageCarrierSummary,
    QUEST_PAGE_CARRIER_ALIGNMENT, QUEST_PAGE_CARRIER_CAPACITY_BYTES,
    QUEST_PAGE_CARRIER_END_RUNTIME, QUEST_PAGE_CARRIER_SEARCH_END_RUNTIME,
    QUEST_PAGE_CARRIER_START_RUNTIME,
};
pub use quest_page_source::{
    verify_box_quest_page_source, QuestPageSourceError, QuestPageSourceSummary,
    QUEST_CNX_CORE_RUNTIME, QUEST_CNX_DIRECT_CALL_RUNTIME, QUEST_CNX_DIRECT_WRAPPER_END_RUNTIME,
    QUEST_CNX_DIRECT_WRAPPER_RUNTIME, QUEST_CNX_INPUT_POINTER_GP_OFFSET,
    QUEST_CNX_INPUT_PROGRESS_GP_OFFSET, QUEST_CNX_OUTPUT_BYTES_GP_OFFSET,
    QUEST_CNX_OUTPUT_POINTER_GP_OFFSET, QUEST_CNX_PROGRESS_GP_OFFSET,
    QUEST_CNX_STREAMING_CORE_CALL_RUNTIME, QUEST_CNX_STREAMING_WRAPPER_RUNTIME,
    QUEST_LOADER_ARENA_BYTES, QUEST_LOADER_ARENA_END_RUNTIME, QUEST_LOADER_ARENA_POINTER_GP_OFFSET,
    QUEST_LOADER_ARENA_START_RUNTIME,
};
pub use quest_player_attack_contact::{
    verify_box_quest_player_attack_contact, QuestPlayerAttackContactError,
    QuestPlayerAttackContactSummary, QUEST_PLAYER_ATTACK_COLLISION_CALL_RUNTIME,
    QUEST_PLAYER_ATTACK_DIRECTION_CONTACT_MASKS, QUEST_PLAYER_ATTACK_DIRECTION_OFFSET,
    QUEST_PLAYER_ATTACK_INPUT_MASK, QUEST_PLAYER_ATTACK_INPUT_RUNTIME, QUEST_PLAYER_ATTACK_STATE,
    QUEST_PLAYER_ATTACK_STATE_OFFSET, QUEST_PLAYER_ATTACK_TARGET_CONTACT_FLAGS_OFFSET,
};
pub use quest_player_contact::{
    verify_box_quest_player_contact, QuestPlayerContactError, QuestPlayerContactSummary,
    QUEST_PLAYER_CONTACT_WRITER_CALL_RUNTIME, QUEST_PLAYER_CONTACT_WRITER_RUNTIME,
    QUEST_PLAYER_DIRECTION_CONTACT_MASKS, QUEST_PLAYER_DIRECTION_OFFSET,
    QUEST_PLAYER_UPDATE_CALLBACK_RUNTIME,
};
pub use quest_player_interaction_contact::{
    quest_player_interaction_local_probe, quest_player_interaction_probe_rectangle,
    verify_box_quest_player_interaction_contact, QuestPlayerInteractionContactError,
    QuestPlayerInteractionContactSummary, QUEST_PLAYER_INTERACTION_COLLISION_CALL_RUNTIME,
    QUEST_PLAYER_INTERACTION_DIRECTION_CONTACT_MASKS, QUEST_PLAYER_INTERACTION_DIRECTION_OFFSET,
    QUEST_PLAYER_INTERACTION_DIRECTION_VECTORS, QUEST_PLAYER_INTERACTION_INPUT_MASK,
    QUEST_PLAYER_INTERACTION_INPUT_RUNTIME,
    QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_EXTENT_TRIM_PIXELS,
    QUEST_PLAYER_INTERACTION_PROBE_CROSS_AXIS_INSET_PIXELS,
    QUEST_PLAYER_INTERACTION_PROBE_EXTENT_PIXELS, QUEST_PLAYER_INTERACTION_PROBE_GAP_PIXELS,
    QUEST_PLAYER_INTERACTION_WRITER_CALL_RUNTIME, QUEST_PLAYER_INTERACTION_WRITER_RUNTIME,
};
pub use quest_resident_seq_prerequisite::{
    verify_box_quest_resident_seq_prerequisite, QuestResidentSeqPrerequisiteError,
    QuestResidentSeqPrerequisiteSummary, QUEST_RESIDENT_DEFEAT_PREREQUISITE_RUNTIME,
};
pub use quest_resident_seq_roots::{
    verify_box_quest_resident_seq_roots, QuestResidentSeqRootError, QuestResidentSeqRootInstall,
    QuestResidentSeqRootSelection, QuestResidentSeqRootSource, QuestResidentSeqRootSummary,
};
pub use quest_resident_seq_selector::{
    verify_box_quest_resident_seq_selector, QuestResidentSeqDefeatSelectorWriter,
    QuestResidentSeqSelectorError, QuestResidentSeqSelectorSummary, QuestResidentSeqSelectorWrite,
    QuestResidentSeqSelectorWriteValue,
};
pub use quest_seq::{
    verify_box_quest_seq_pointer_consumer, QuestSeqPointerConsumerError,
    QuestSeqPointerConsumerSummary, QUEST_SEQ_BASE_DISPATCH_TABLE_RUNTIME,
    QUEST_SEQ_BRANCH_DISPATCH_OPCODE_BASE, QUEST_SEQ_BRANCH_DISPATCH_OPCODE_COUNT,
    QUEST_SEQ_BRANCH_DISPATCH_REGISTER_RUNTIME, QUEST_SEQ_BRANCH_DISPATCH_REGISTRATION_RUNTIME,
    QUEST_SEQ_BRANCH_DISPATCH_TABLE_RUNTIME, QUEST_SEQ_BRANCH_POINTER_CONSUMER_RUNTIME,
    QUEST_SEQ_BRANCH_RECORD_POINTER_OFFSET, QUEST_SEQ_BRANCH_RECORD_STRIDE,
    QUEST_SEQ_DEFAULT_RESOURCE_ID_RUNTIME, QUEST_SEQ_INTERPRETER_RUNTIME,
    QUEST_SEQ_STACK_BRANCH_POINTER_CONSUMER_RUNTIME, QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
};
pub use quest_seq_advance::{
    verify_box_quest_seq_direct_advances, QuestSeqAdvanceError, QuestSeqAdvanceSummary,
    QuestSeqDirectAdvanceSite, QUEST_SEQ_DIRECT_ADVANCE_SITES,
    QUEST_SEQ_STREAM_POINTER_FIELD_OFFSET, QUEST_SEQ_WITHOUT_DIRECT_ADVANCE,
};
pub use quest_seq_base_advance::{
    verify_box_quest_seq_base_direct_advances, QuestSeqBaseAdvanceError,
    QuestSeqBaseAdvanceSummary, QuestSeqBaseDirectAdvanceSite, QUEST_SEQ_BASE_DIRECT_ADVANCE_SITES,
    QUEST_SEQ_BASE_WITHOUT_DIRECT_ADVANCE,
};
pub use quest_seq_base_compare::{
    quest_seq_base_compare_record_layout, verify_box_quest_seq_base_compares,
    QuestSeqBaseCompareError, QuestSeqBaseCompareOperandSource, QuestSeqBaseCompareOperation,
    QuestSeqBaseCompareRecordLayout, QuestSeqBaseCompareSummary,
    QUEST_SEQ_BASE_COMPARE_LEFT_SOURCE_MASK, QUEST_SEQ_BASE_COMPARE_OPERATION_COUNT,
    QUEST_SEQ_BASE_COMPARE_OPERATION_MASK, QUEST_SEQ_BASE_COMPARE_RIGHT_SOURCE_MASK,
    QUEST_SEQ_BASE_COMPARE_STATE_BASE_RUNTIME, QUEST_SEQ_OP13_COMPARE_HANDLERS,
    QUEST_SEQ_OP13_COMPARE_TABLE_RUNTIME, QUEST_SEQ_OP14_COMPARE_HANDLERS,
    QUEST_SEQ_OP14_COMPARE_TABLE_RUNTIME,
};
pub use quest_seq_base_record::{
    quest_seq_base_fallback_advance, quest_seq_base_record_selected_advance,
    verify_box_quest_seq_base_record_advances, QuestSeqBaseRecordError, QuestSeqBaseRecordSummary,
    QUEST_SEQ_BASE_RECORD_MODE_MASK, QUEST_SEQ_OP3A_HANDLER_RUNTIME,
    QUEST_SEQ_OP3A_STATIC_ADVANCE_SUBOPCODES, QUEST_SEQ_OP3A_SUBDISPATCH_HANDLERS,
    QUEST_SEQ_OP3A_SUBDISPATCH_TABLE_RUNTIME,
};
pub use quest_seq_base_transition::{
    verify_box_quest_seq_base_transitions, QuestSeqBaseLifecycleMeaning, QuestSeqBaseLifecycleSpec,
    QuestSeqBaseTransitionError, QuestSeqBaseTransitionMeaning, QuestSeqBaseTransitionSpec,
    QuestSeqBaseTransitionSummary, QUEST_SEQ_BASE_LIFECYCLE_SPECS, QUEST_SEQ_BASE_TRANSITION_SPECS,
    QUEST_SEQ_BRANCH_STATE_BASE_RUNTIME, QUEST_SEQ_CATALOG_GROUP_TABLE_RUNTIME,
    QUEST_SEQ_RETURN_DEPTH_OFFSET, QUEST_SEQ_RETURN_STACK_OFFSET,
};
pub use quest_seq_child_activation::{
    verify_box_quest_seq_child_activation, QuestSeqChildActivationError,
    QuestSeqChildActivationSummary, QUEST_SEQ_CHILD_ALTERNATE_CONTACT_MASK,
    QUEST_SEQ_CHILD_COLLISION_LAYER_OFFSET, QUEST_SEQ_CHILD_COLLISION_MEMBER_FLAGS_OFFSET,
    QUEST_SEQ_CHILD_COLLISION_MEMBER_MASK, QUEST_SEQ_CHILD_COLLISION_QUERY_RUNTIME,
    QUEST_SEQ_CHILD_CONTACT_FLAGS_OFFSET, QUEST_SEQ_CHILD_CONTACT_WRITER_RUNTIME,
    QUEST_SEQ_CHILD_DIRECT_CONTACT_MASK, QUEST_SEQ_CHILD_STATE_OFFSET,
    QUEST_SEQ_CHILD_UPDATE_CALLBACK_RUNTIME,
};
pub use quest_seq_child_configuration::{
    verify_box_quest_seq_child_configuration, QuestSeqChildConfigurationError,
    QuestSeqChildConfigurationSummary, QUEST_SEQ_CHILD_BOUNDS_ARM_RUNTIME,
    QUEST_SEQ_CHILD_CONFIGURATION_HANDLER_RUNTIME, QUEST_SEQ_CHILD_CONFIGURATION_OPCODE,
    QUEST_SEQ_CHILD_CONFIGURATION_RECORD_BYTES, QUEST_SEQ_CHILD_CONFIGURATION_SUBTYPE_LIMIT,
    QUEST_SEQ_CHILD_CONFIGURATION_TABLE_RUNTIME, QUEST_SEQ_CHILD_FIXED_POINT_SHIFT,
    QUEST_SEQ_CHILD_MOTION_ARM_RUNTIME, QUEST_SEQ_CHILD_POINTER_FIELD_OFFSET,
    QUEST_SEQ_CHILD_POSITION_ARM_RUNTIME,
};
pub use quest_seq_child_stream::{
    verify_box_quest_seq_child_stream, QuestSeqChildStreamError, QuestSeqChildStreamSummary,
    QUEST_SEQ_CHILD_STREAM_HANDLER_RUNTIME, QUEST_SEQ_CHILD_STREAM_OPCODE,
    QUEST_SEQ_CHILD_STREAM_POINTER_OFFSET, QUEST_SEQ_CHILD_STREAM_RECORD_BYTES,
    QUEST_SEQ_CHILD_STREAM_SUBTYPE2_ARM_RUNTIME, QUEST_SEQ_CHILD_STREAM_SUBTYPE2_TABLE_RUNTIME,
    QUEST_SEQ_CHILD_STREAM_SUBTYPE_LIMIT,
};
pub use quest_seq_flag_query::{
    resolve_quest_seq_flag_probe, verify_box_quest_seq_flag_query, QuestSeqFlagProbe,
    QuestSeqFlagQueryError, QuestSeqFlagQuerySummary, QUEST_SEQ_FLAG_BASE_OFFSET,
    QUEST_SEQ_FLAG_BRANCH_POINTER_OFFSET, QUEST_SEQ_FLAG_EXPECTED_OFFSET,
    QUEST_SEQ_FLAG_OFFSET_OFFSET, QUEST_SEQ_FLAG_QUERY_HANDLER_RUNTIME,
    QUEST_SEQ_FLAG_QUERY_OPCODE, QUEST_SEQ_FLAG_QUERY_RECORD_BYTES, QUEST_SEQ_FLAG_QUERY_RUNTIME,
    QUEST_SEQ_FLAG_STATE_BASE_RUNTIME,
};
pub use quest_seq_global_word_assignment::{
    parse_quest_seq_global_word_assignment, verify_box_quest_seq_global_word_assignment,
    QuestSeqGlobalWordAssignment, QuestSeqGlobalWordAssignmentError,
    QuestSeqGlobalWordAssignmentSummary, QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_BYTES,
    QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_HANDLER_RUNTIME, QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_MODE,
    QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPCODE, QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPERATION_RUNTIME,
    QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_OPERATION_TABLE_RUNTIME,
    QUEST_SEQ_GLOBAL_WORD_STATE_BASE_RUNTIME,
};
pub use quest_seq_loader::{
    verify_box_quest_seq_loader, QuestSeqLoaderError, QuestSeqLoaderSummary,
    QUEST_SEQ_ACTIVE_COPY_WORDS, QUEST_SEQ_ACTIVE_RUNTIME, QUEST_SEQ_DESCRIPTOR_SLOT_RUNTIME,
    QUEST_SEQ_DESCRIPTOR_STRIDE, QUEST_SEQ_LOAD_RUNTIME, QUEST_SEQ_RESOURCE_ID_STATE_OFFSET,
    QUEST_SEQ_STAGING_RUNTIME, QUEST_SEQ_WORD_COPY_RUNTIME,
};
pub use quest_seq_op28::{
    verify_box_quest_seq_op28, QuestSeqOp28DirectAdvanceSite, QuestSeqOp28Error,
    QuestSeqOp28Summary, QUEST_SEQ_OP28_DIRECT_ADVANCE_SITES,
};
pub use quest_seq_registration::{
    verify_box_quest_seq_registration, QuestSeqRegistrationSummary,
    QUEST_SEQ_ARCHIVE_LOADER_RUNTIME, QUEST_SEQ_ARCHIVE_NAME_RUNTIME,
    QUEST_SEQ_PREFIX_DESCRIPTOR_ROWS, QUEST_SEQ_REGISTRATION_HANDLER_RUNTIME,
    QUEST_SEQ_REGISTRATION_RECORD_RUNTIME, QUEST_SEQ_RESOURCE_TABLE_RUNTIME,
    QUEST_SEQ_RESOURCE_TYPE,
};
pub use quest_seq_transition::{
    verify_box_quest_seq_non_advance_meanings, QuestSeqNonAdvanceError, QuestSeqNonAdvanceMeaning,
    QuestSeqNonAdvanceSpec, QuestSeqNonAdvanceSummary, QUEST_SEQ_ALLOCATE_OBJECT_RUNTIME,
    QUEST_SEQ_NON_ADVANCE_SPECS, QUEST_SEQ_PRESET_INTERPRETER_STREAM_RUNTIME,
    QUEST_SEQ_RELEASE_OBJECT_RUNTIME, QUEST_SEQ_RESOURCE_READY_RUNTIME,
    QUEST_SEQ_TRANSITION_RESOURCE_ID_OFFSET,
};
pub use quest_sound_sequence::{
    verify_box_quest_sound_sequence_consumer, QuestSoundSequenceConsumerError,
    QuestSoundSequenceConsumerSummary, QUEST_SOUND_APPLICATION_SLOT_RUNTIME,
    QUEST_SOUND_APPLICATION_SLOT_STRIDE, QUEST_SOUND_LOADER_STATE_RUNTIME,
    QUEST_SOUND_LOAD_DESTINATION_RUNTIME, QUEST_SOUND_SEQUENCE_POINTER_TABLE_RUNTIME,
    QUEST_SOUND_SEQUENCE_STATE_RUNTIME, QUEST_SOUND_STAGING_RUNTIME, QUEST_WAKEUP_SEQUENCE_BYTES,
    QUEST_WAKEUP_SEQUENCE_DESCRIPTOR_RUNTIME, QUEST_WAKEUP_SEQUENCE_NAME_RUNTIME,
};
pub use quest_text_mode::{
    verify_box_quest_text_mode, QuestTextModeError, QuestTextModeSummary,
    QUEST_SEQ_TEXT_ACTIVE_WORD_OFFSET, QUEST_SEQ_TEXT_CONSUMER_CALL_RUNTIME,
    QUEST_SEQ_TEXT_CONSUMER_RUNTIME, QUEST_SEQ_TEXT_GLYPH_RENDERER_RETURN_RUNTIME,
    QUEST_SEQ_TEXT_GLYPH_RENDERER_RUNTIME, QUEST_SEQ_TEXT_MODE_HANDLER_RUNTIME,
};
pub use quest_text_window::{
    verify_box_quest_text_window_consumer, QuestTextWindowConsumerError,
    QuestTextWindowConsumerSummary, QUEST_TEXT_WINDOW_CELL_STATE_OFFSET,
    QUEST_TEXT_WINDOW_DRAW_RUNTIME, QUEST_TEXT_WINDOW_FRAME_OUTSET_PX,
    QUEST_TEXT_WINDOW_PARENT_HEIGHT_OFFSET, QUEST_TEXT_WINDOW_PARENT_STATE_OFFSET,
    QUEST_TEXT_WINDOW_PARENT_WIDTH_OFFSET, QUEST_TEXT_WINDOW_PARENT_X_FIXED12_OFFSET,
    QUEST_TEXT_WINDOW_PARENT_Y_FIXED12_OFFSET, QUEST_TEXT_WINDOW_POSITION_FRACTION_BITS,
    QUEST_TEXT_WINDOW_RESOURCE_STATE_OFFSET, QUEST_TEXT_WINDOW_TILE_STEP_PX,
};
pub use quest_ui_literals::{
    verify_box_quest_ui_literals, QuestMenuGlyphCall, QuestUiLiteral, QuestUiLiteralError,
    QuestUiLiteralRole, QuestUiLiteralSummary, QuestUiLiteralSurface, QUEST_MENU_GLYPH_CALLS,
    QUEST_MENU_LITERAL_COUNT, QUEST_MENU_LITERAL_SPECS, QUEST_UI_CHOICE_DIRECT_GLYPH_CALL_RUNTIMES,
    QUEST_UI_CHOICE_NO_RUNTIME, QUEST_UI_CHOICE_YES_RUNTIME,
    QUEST_WORLD_MAP_LABEL_WHOLE_STRING_CALL_RUNTIME,
};
pub use recursive::{trace, Trace};
pub use shell_exec::{
    verify_box_shell_exec_path, ShellExecError, ShellExecSummary, SHELL_EXEC_PATHS,
};
pub use subgame_seq_control::{
    render_subgame_seq_controls_tsv, verify_box_subgame_seq_controls, SubgameControlAnalysis,
    SubgameControlBehavior, SubgameControlError, SubgameControlSite, BOX_SUBGAME_CONTROL_BEHAVIORS,
};
pub use subgame_seq_control_target::{
    render_subgame_seq_control_targets_tsv, verify_box_subgame_seq_control_targets,
    SubgameControlTargetAnalysis, SubgameControlTargetError, SubgameControlTargetKind,
    SubgameControlTargetOccurrence, SubgameControlTargetRow, SubgameControlTargetSpec,
    BOX_SUBGAME_SEQ_CONTROL_TARGET_EXE_CANDIDATES,
    BOX_SUBGAME_SEQ_CONTROL_TARGET_INTERNAL_POINTERS, BOX_SUBGAME_SEQ_CONTROL_TARGET_OVERLAYS,
    BOX_SUBGAME_SEQ_CONTROL_TARGET_RAW, BOX_SUBGAME_SEQ_CONTROL_TARGET_REPORT_SHA256,
    BOX_SUBGAME_SEQ_CONTROL_TARGET_ROWS, BOX_SUBGAME_SEQ_CONTROL_TARGET_SEQUENCE_CANDIDATES,
    BOX_SUBGAME_SEQ_CONTROL_TARGET_SEQUENCE_FILES, BOX_SUBGAME_SEQ_CONTROL_TARGET_SPECS,
    BOX_SUBGAME_SEQ_FF12_GENERATED_BYTES, BOX_SUBGAME_SEQ_FF15_GUARD_EXCLUSIVE_HALFWORDS,
    BOX_SUBGAME_SEQ_FF15_MAX_GENERATED_BYTES, SUBGAME_SEQ_CONTROL_TARGET_SCHEMA_VERSION,
};
pub use subgame_seq_descriptor::{
    render_subgame_seq_descriptors_tsv, verify_box_subgame_seq_descriptors,
    SubgameSeqDescriptorAnalysis, SubgameSeqDescriptorError, SubgameSeqDescriptorRow,
    SubgameSeqDescriptorSpec, BOX_SUBGAME_SEQ_DESCRIPTOR_REPORT_SHA256,
    BOX_SUBGAME_SEQ_DESCRIPTOR_ROWS, BOX_SUBGAME_SEQ_DESCRIPTOR_SPECS,
    SUBGAME_SEQ_DESCRIPTOR_SCHEMA_VERSION,
};
pub use subgame_seq_drac_consumer::{
    render_subgame_seq_drac_consumers_tsv, verify_box_subgame_seq_drac_consumers,
    SubgameSeqDracConsumerAnalysis, SubgameSeqDracConsumerError, SubgameSeqDracConsumerRow,
    SubgameSeqDracConsumerSpec, SubgameSeqDracMessageLayout,
    BOX_SUBGAME_SEQ_DRAC_ARGUMENT_CONTROLS_PER_EXE, BOX_SUBGAME_SEQ_DRAC_ARGUMENT_TOKENS,
    BOX_SUBGAME_SEQ_DRAC_BODY_BYTES_PER_EXE, BOX_SUBGAME_SEQ_DRAC_COLUMN_ADVANCES_PER_EXE,
    BOX_SUBGAME_SEQ_DRAC_COMMAND_OPCODE, BOX_SUBGAME_SEQ_DRAC_COMMAND_TABLE_ENTRIES,
    BOX_SUBGAME_SEQ_DRAC_CONSUMER_REPORT_SHA256, BOX_SUBGAME_SEQ_DRAC_CONSUMER_ROWS,
    BOX_SUBGAME_SEQ_DRAC_CONSUMER_SPECS, BOX_SUBGAME_SEQ_DRAC_CURRENT_WAITS_PER_EXE,
    BOX_SUBGAME_SEQ_DRAC_END_WAITS_PER_EXE, BOX_SUBGAME_SEQ_DRAC_GLYPH_TOKENS_PER_EXE,
    BOX_SUBGAME_SEQ_DRAC_HEADER_BYTES_PER_EXE, BOX_SUBGAME_SEQ_DRAC_HEADER_PROFILES,
    BOX_SUBGAME_SEQ_DRAC_LAYOUT_MATCHES_PER_EXE, BOX_SUBGAME_SEQ_DRAC_LINE_BREAK_PAIRS_PER_EXE,
    BOX_SUBGAME_SEQ_DRAC_RECORDS_PER_EXE, BOX_SUBGAME_SEQ_DRAC_STATE_CONTROLS_PER_EXE,
    BOX_SUBGAME_SEQ_DRAC_TERMINATORS_PER_EXE, BOX_SUBGAME_SEQ_DRAC_TOKEN_FIRST,
    BOX_SUBGAME_SEQ_DRAC_VARIANT_RECORDS_PER_EXE, SUBGAME_SEQ_DRAC_CONSUMER_SCHEMA_VERSION,
};
pub use subgame_seq_drac_glyph::{
    render_subgame_seq_drac_glyphs_tsv, verify_box_subgame_seq_drac_glyphs,
    SubgameSeqDracGlyphAnalysis, SubgameSeqDracGlyphError, SubgameSeqDracGlyphRow,
    SubgameSeqDracGlyphSpec, BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES, BOX_SUBGAME_SEQ_DRAC_ATLAS_HEIGHT,
    BOX_SUBGAME_SEQ_DRAC_ATLAS_PIXELS, BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH,
    BOX_SUBGAME_SEQ_DRAC_AUXILIARY_CELL, BOX_SUBGAME_SEQ_DRAC_AUXILIARY_GP_ACCESSES,
    BOX_SUBGAME_SEQ_DRAC_AUXILIARY_NEXT_CELL, BOX_SUBGAME_SEQ_DRAC_AUXILIARY_OBJECT_SITES,
    BOX_SUBGAME_SEQ_DRAC_CELL_BASE, BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS,
    BOX_SUBGAME_SEQ_DRAC_CELL_GRID_ROWS, BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X,
    BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y, BOX_SUBGAME_SEQ_DRAC_CELL_HEADER_BYTES,
    BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT, BOX_SUBGAME_SEQ_DRAC_CELL_LAST,
    BOX_SUBGAME_SEQ_DRAC_CELL_STRIDE, BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET,
    BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_PREFIX_END, BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_PREFIX_SHA256,
    BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_RUNTIME, BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH,
    BOX_SUBGAME_SEQ_DRAC_DESCRIPTOR_STRIDE, BOX_SUBGAME_SEQ_DRAC_GLYPH_REPORT_SHA256,
    BOX_SUBGAME_SEQ_DRAC_GLYPH_ROWS, BOX_SUBGAME_SEQ_DRAC_GLYPH_SPECS,
    BOX_SUBGAME_SEQ_DRAC_MAIN_GP_ACCESSES, BOX_SUBGAME_SEQ_DRAC_MAIN_RESOURCE_BRANCH_SITES,
    BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS, BOX_SUBGAME_SEQ_DRAC_SOURCE_DUPLICATE_SLOTS,
    BOX_SUBGAME_SEQ_DRAC_SOURCE_UNIQUE_PAIRS, BOX_SUBGAME_SEQ_DRAC_SPACE_PAIR_INDICES,
    PUYO1_DRAC_AUXILIARY_RESOURCE_GLOBAL_RUNTIME, PUYO1_DRAC_GP_RUNTIME,
    PUYO1_DRAC_MAIN_RESOURCE_GLOBAL_RUNTIME, PUYO2_DRAC_AUXILIARY_RESOURCE_GLOBAL_RUNTIME,
    PUYO2_DRAC_GP_RUNTIME, PUYO2_DRAC_MAIN_RESOURCE_GLOBAL_RUNTIME,
    SUBGAME_SEQ_DRAC_GLYPH_SCHEMA_VERSION,
};
pub use subgame_seq_e6_glyph::{
    audit_subgame_seq_e6_glyphs, render_subgame_seq_e6_glyphs_tsv,
    verify_box_subgame_seq_e6_glyphs, SubgameSeqE6GlyphAnalysis, SubgameSeqE6GlyphError,
    SubgameSeqE6GlyphRow, SubgameSeqE6GlyphSpec, SubgameSeqE6StorageLink,
    BOX_SUBGAME_SEQ_E6_GLYPH_REPORT_SHA256, BOX_SUBGAME_SEQ_E6_GLYPH_SPECS,
    SUBGAME_SEQ_E6_GLYPH_SCHEMA_VERSION,
};
pub use subgame_seq_e6_text::{
    audit_subgame_seq_e6_text, render_subgame_seq_e6_text_tsv, verify_box_subgame_seq_e6_text,
    SubgameSeqE6TextAnalysis, SubgameSeqE6TextError, SubgameSeqE6TextRow, SubgameSeqE6TextSpan,
    SubgameSeqE6TextSpec, BOX_SUBGAME_SEQ_E6_TEXT_REPORT_SHA256,
    BOX_SUBGAME_SEQ_E6_TEXT_ROWS_PER_EXE, BOX_SUBGAME_SEQ_E6_TEXT_SPECS,
    SUBGAME_SEQ_E6_TEXT_SCHEMA_VERSION,
};
pub use subgame_seq_ending_route::{
    verify_box_puyo2_ending_route, Puyo2EndingRouteAnalysis, Puyo2EndingRouteError,
    BOX_PUYO2_ENDING_ROUTE_ARCHIVE, BOX_PUYO2_ENDING_ROUTE_ARCHIVE_ENTRIES,
};
pub use subgame_seq_load::{
    render_subgame_seq_loads_tsv, verify_box_subgame_seq_loads, SubgameSeqLoadAnalysis,
    SubgameSeqLoadError, SubgameSeqLoadRow, SubgameSeqLoadSpec, BOX_SUBGAME_SEQ_LOAD_REPORT_SHA256,
    BOX_SUBGAME_SEQ_LOAD_ROWS, BOX_SUBGAME_SEQ_LOAD_SPECS, SUBGAME_SEQ_LOAD_DESTINATION,
    SUBGAME_SEQ_LOAD_FLAGS, SUBGAME_SEQ_LOAD_SCHEMA_VERSION,
};
pub use subgame_seq_master_route::{
    verify_box_puyo2_master_route, Puyo2MasterRouteAnalysis, Puyo2MasterRouteError,
    BOX_PUYO2_MASTER_ROUTE_ARCHIVE, BOX_PUYO2_MASTER_ROUTE_ARCHIVE_ENTRIES,
};
pub use subgame_seq_pcomi_admission_producers::{
    verify_box_pcomi_custom_byte_admission_producers, PcomiAdmissionProducer,
    PcomiAdmissionProducerAnalysis, PcomiAdmissionProducerError, PCOMI1_FIRST_ADMISSION_OFFSET,
    PCOMI1_REJECTED_HALFWORD_OFFSET, PCOMI1_SECOND_ADMISSION_OFFSET,
    PCOMI1_SECOND_UI_CALLBACK_OFFSET, PCOMI_ADMISSION_CALLBACK_INDEX,
    PCOMI_ADMISSION_CALLBACK_RUNTIME, PCOMI_CALLBACK_DISPATCH_OPCODE,
    PCOMI_CALLBACK_DISPATCH_RUNTIME, PCOMI_LOCAL_CALLBACK_TABLE_RUNTIME, PCOMI_UI_CALLBACK_INDEX,
    PCOMI_UI_CALLBACK_RUNTIME,
};
pub use subgame_seq_pcomi_custom_byte_atlas::{
    verify_box_pcomi_custom_byte_atlas, PcomiCustomByteAtlasAnalysis, PcomiCustomByteAtlasError,
    PCOMI_CUSTOM_BYTE_ATLAS_HEIGHT, PCOMI_CUSTOM_BYTE_ATLAS_RESOURCE,
    PCOMI_CUSTOM_BYTE_ATLAS_WIDTH, PCOMI_CUSTOM_BYTE_CELL_RECORDS,
    PCOMI_CUSTOM_BYTE_DESCRIPTOR_RUNTIME,
};
pub use subgame_seq_pcomi_custom_byte_consumer::{
    verify_box_pcomi_custom_byte_consumer, PcomiCustomByteConsumerAnalysis,
    PcomiCustomByteConsumerError, PCOMI_CUSTOM_BYTE_BUFFER_RUNTIME,
    PCOMI_CUSTOM_BYTE_DIRECT_CELL_STORE_RUNTIME, PCOMI_CUSTOM_BYTE_FORMATTER_RUNTIME,
    PCOMI_CUSTOM_BYTE_FORMAT_SCRATCH_RUNTIME, PCOMI_CUSTOM_BYTE_RENDERER_EXTENDED_RUNTIME,
    PCOMI_CUSTOM_BYTE_RENDERER_SHORT_RUNTIME, PCOMI_CUSTOM_BYTE_ROOT_FIELD_RUNTIME,
    PCOMI_CUSTOM_BYTE_SLOT_COUNT, PCOMI_CUSTOM_BYTE_SLOT_PAYLOAD_BYTES,
    PCOMI_CUSTOM_BYTE_SLOT_SELECTOR, PCOMI_CUSTOM_BYTE_SLOT_TABLE_RUNTIME,
    PCOMI_CUSTOM_BYTE_U16_STREAM_RUNTIME, PCOMI_CUSTOM_BYTE_UI_CONSUMER_RUNTIME,
};
pub use subgame_seq_pcomi_glyph_code::{
    classify_pcomi_glyph_code, render_pcomi_glyph_codes_tsv, verify_box_pcomi_glyph_codes,
    PcomiGlyphCodeAnalysis, PcomiGlyphCodeConsumer, PcomiGlyphCodeError, PcomiGlyphCodeRoute,
    PcomiGlyphSuffix, PcomiGlyphSuffixTable, PCOMI_GLYPH_CELL_BASE, PCOMI_GLYPH_DIRECT_LIMIT,
    PCOMI_GLYPH_FALLBACK_RUNTIME, PCOMI_GLYPH_OBJECT_CELL_OFFSET, PCOMI_GLYPH_STAR_TABLE_RUNTIME,
    PCOMI_GLYPH_SUFFIX_HIGH_MAX, PCOMI_GLYPH_SUFFIX_HIGH_MIN, PCOMI_GLYPH_SUFFIX_TABLE_ENTRIES,
    PCOMI_GLYPH_UNDERSCORE_TABLE_RUNTIME,
};
pub use subgame_seq_pcomi_load::{
    verify_box_pcomi_seq_load_route, PcomiSeqLoadError, PcomiSeqLoadRoute,
    PCOMI1_COMPLETION_CALLBACK_RUNTIME, PCOMI1_DECOMPRESSED_BYTES,
    PCOMI1_DESCRIPTOR_DESTINATION_RUNTIME, PCOMI1_DESCRIPTOR_RUNTIME, PCOMI1_INITIALIZED_BYTES,
    PCOMI1_LOAD_BASE, PCOMI1_LOAD_FLAGS, PCOMI1_LOAD_RECORD_RUNTIME, PCOMI1_MASTER_CALL_RUNTIME,
    PCOMI1_MASTER_CONTINUATION_RUNTIME, PCOMI1_OVERLAY_ENTRY_OFFSET, PCOMI1_OVERLAY_ENTRY_RUNTIME,
    PCOMI1_OVERLAY_START_OFFSET, PCOMI1_OVERLAY_START_RUNTIME, PCOMI1_RESOURCE_ID,
    PCOMI1_RESOURCE_TYPE, PCOMI1_SEQUENCE_BYTES, PCOMI1_SEQUENCE_SHA256,
};
pub use subgame_seq_registration::{
    render_subgame_seq_registrations_tsv, verify_box_subgame_seq_registrations,
    SubgameSeqRegistrationAnalysis, SubgameSeqRegistrationError, SubgameSeqRegistrationRow,
    SubgameSeqRegistrationSpec, BOX_SUBGAME_SEQ_REGISTRATION_REPORT_SHA256,
    BOX_SUBGAME_SEQ_REGISTRATION_ROWS, BOX_SUBGAME_SEQ_REGISTRATION_SPECS,
    SUBGAME_SEQ_DESCRIPTOR_STRIDE, SUBGAME_SEQ_REGISTRATION_OPCODE,
    SUBGAME_SEQ_REGISTRATION_SCHEMA_VERSION, SUBGAME_SEQ_TYPE,
};
pub use subgame_seq_selection::{
    render_subgame_seq_selections_tsv, verify_box_subgame_seq_selections,
    SubgameSeqSelectionAnalysis, SubgameSeqSelectionDispatcherSpec, SubgameSeqSelectionError,
    SubgameSeqSelectionRow, SubgameSeqSelectionSpec, BOX_SUBGAME_SEQ_SELECTION_DISPATCHERS,
    BOX_SUBGAME_SEQ_SELECTION_REPORT_SHA256, BOX_SUBGAME_SEQ_SELECTION_ROWS,
    BOX_SUBGAME_SEQ_SELECTION_SPECS, SUBGAME_SEQ_OPCODE_TABLE_ENTRIES,
    SUBGAME_SEQ_SELECTION_OPCODE, SUBGAME_SEQ_SELECTION_RESOURCE,
    SUBGAME_SEQ_SELECTION_SCHEMA_VERSION, SUBGAME_SEQ_SELECTION_TYPE,
};
pub use subgame_seq_text_bridge::{
    render_subgame_seq_text_bridges_tsv, verify_box_subgame_seq_text_bridges,
    SubgameSeqTextBridgeAnalysis, SubgameSeqTextBridgeError, SubgameSeqTextBridgeRow,
    SubgameSeqTextBridgeSpec, BOX_SUBGAME_SEQ_TEXT_BRIDGE_ENTRY_REACHED,
    BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXE_RECORDS, BOX_SUBGAME_SEQ_TEXT_BRIDGE_REPORT_SHA256,
    BOX_SUBGAME_SEQ_TEXT_BRIDGE_ROWS, BOX_SUBGAME_SEQ_TEXT_BRIDGE_SEQUENCE_FILES,
    BOX_SUBGAME_SEQ_TEXT_BRIDGE_SEQUENCE_RECORDS, BOX_SUBGAME_SEQ_TEXT_BRIDGE_SPECS,
    SUBGAME_SEQ_TEXT_BRIDGE_MODE, SUBGAME_SEQ_TEXT_BRIDGE_OPCODE,
    SUBGAME_SEQ_TEXT_BRIDGE_SCHEMA_VERSION, SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES,
};
pub use subgame_seq_text_consumer::{
    render_subgame_text_consumers_tsv, verify_box_subgame_text_consumers, SubgameTextConsumer,
    SubgameTextConsumerAnalysis, SubgameTextConsumerError, SubgameTextConsumerSpec,
    SubgameTextControlTable, SubgameTextSourceKind, BOX_SUBGAME_TEXT_CONSUMERS,
};
pub use trace_inventory::{
    audit_executable_traces, render_executable_trace_tsv, verify_box_executable_traces,
    ExecutableTraceSummary, TraceInventoryError,
};
pub use xref::{find_address_loads, AddressLoad, LoadForm};
