//! 스크립트 포맷과 token 경계.
//!
//! 현재는 `QUEST.EXE` 소비 코드로 확정한 SEQ opcode `0x00`–`0xFF`
//! dispatch 256종, text control `FF00`–`FF1F` 32종 layout·의미,
//! `TOWN00.SEQ`의 선두 16×20바이트 레코드·본문 root OP03과
//! `RPG_SEQ.VDD` 전체의 내부주소·포인터 record 형태 후보, OP21 기반
//! text stream·일본어 run, direct incoming pointer 교차와 직접 폭·원본
//! record mode·하위 dispatch를 따르는 보수적 seed path, 고유 중단
//! frontier의 정적 하한, EXE 상주 시작 메뉴의 resource 전환 기준점, 정적
//! OP21 stream과 세 runtime 표본의 연결을 제공한다. section 소유권·정상
//! 실행 도달 전수와 실제 내부 참조 범위가 아직 열려 있으므로 전체 SEQ
//! parser·serializer나 번역문 쓰기 경계로 승격하지 않는다.

pub mod menu06_text;
pub mod menu06_translation_manifest;
pub mod official_card_text;
pub mod quest;
pub mod quest_item_description_drafts;
pub mod quest_item_name_drafts;
pub mod quest_item_text_units;
pub mod quest_manual_layout;
pub mod quest_seq;
pub mod quest_seq_child_contact;
pub mod quest_seq_inventory;
pub mod quest_seq_op28;
pub mod quest_seq_opcode;
pub mod quest_seq_text_candidates;
pub mod quest_seq_text_document;
pub mod quest_seq_text_extraction;
pub mod quest_seq_text_roots;
pub mod quest_seq_text_runtime;
pub mod quest_seq_text_streams;
pub mod quest_seq_translation_units;
pub mod quest_terminology;
pub mod quest_text_candidates;
pub mod quest_translation;
pub mod quest_translation_approvals;
pub mod quest_translation_batches;
pub mod quest_translation_decisions;
pub mod quest_translation_review;
pub mod quest_translation_selection;
pub mod subgame_selector_records;
pub mod subgame_seq_custom_byte_record;
pub mod subgame_seq_drac_relink;
pub mod subgame_seq_drac_text;
pub mod subgame_seq_drac_translation_plan;
pub mod subgame_seq_layouts;
pub mod subgame_seq_scene_text;
pub mod subgame_seq_text_candidates;
pub mod subgame_seq_text_roots;

pub use menu06_text::{
    render_menu06_text_review_tsv, serialize_menu06_control_preserving_rewrite,
    serialize_menu06_text_tokens, verify_box_menu06_text, Menu06RewriteCapacity,
    Menu06SourceEvidence, Menu06TextAnalysis, Menu06TextError, Menu06TextSegment, Menu06TextToken,
    MENU06_GLYPH_SOURCES, MENU06_STREAM_END_OFFSET, MENU06_STREAM_OFFSET,
    MENU06_TEXT_REVIEW_SCHEMA_VERSION, MENU06_TEXT_REVIEW_SHA256,
};
pub use menu06_translation_manifest::{
    approved_box_menu06_translation_plan, render_box_menu06_translation_manifest_tsv,
    review_box_menu06_translation_plan, verify_box_menu06_translation_manifest, Menu06GlyphMapping,
    Menu06TranslationManifestAnalysis, Menu06TranslationManifestEntry,
    Menu06TranslationManifestError, Menu06TranslationPlan, Menu06TranslationStatus,
    MENU06_TRANSLATION_MANIFEST_SCHEMA_VERSION,
};
pub use quest::{
    parse_quest_token, parse_quest_tokens, quest_control_meaning, quest_control_spec,
    verify_quest_control_dispatch, QuestControlDispatchError, QuestControlDispatchReport,
    QuestControlLayout, QuestControlMeaning, QuestControlSpec, QuestToken, QuestTokenError,
    QUEST_CONFIRMED_CONTROL_MEANINGS, QUEST_CONTROL_COUNT, QUEST_CONTROL_DISPATCH_BYTES,
    QUEST_CONTROL_DISPATCH_RUNTIME,
};
pub use quest_item_description_drafts::{
    audit_quest_item_description_drafts, QuestItemDescriptionDraftAnalysis,
    QuestItemDescriptionDraftEntry, QuestItemDescriptionDraftError, QuestItemDescriptionDraftPart,
    QuestItemDescriptionDraftStatus, QUEST_ITEM_DESCRIPTION_DRAFT_SCHEMA_VERSION,
};
pub use quest_item_name_drafts::{
    audit_quest_item_name_drafts, QuestItemNameDraftAnalysis, QuestItemNameDraftEntry,
    QuestItemNameDraftError, QuestItemNameDraftStatus, QUEST_ITEM_NAME_DRAFT_SCHEMA_VERSION,
};
pub use quest_item_text_units::{
    audit_quest_item_text_units, render_quest_item_text_units_tsv,
    verify_box_quest_item_text_units, QuestItemTextRole, QuestItemTextTerminal, QuestItemTextToken,
    QuestItemTextTokenKind, QuestItemTextUnit, QuestItemTextUnitAnalysis, QuestItemTextUnitError,
    BOX_QUEST_ITEM_DESCRIPTION_CANDIDATE_RUNS, BOX_QUEST_ITEM_DESCRIPTION_UNITS,
    BOX_QUEST_ITEM_NAME_CANDIDATE_RUNS, BOX_QUEST_ITEM_NAME_UNITS, BOX_QUEST_ITEM_RECORDS,
    BOX_QUEST_ITEM_SHORT_NAME_UNITS, BOX_QUEST_ITEM_TABLE_ENTRIES,
    BOX_QUEST_ITEM_TEXT_CANDIDATE_RUNS, BOX_QUEST_ITEM_TEXT_UNITS, QUEST_ITEM_TEXT_RUNTIME_BASE,
    QUEST_ITEM_TEXT_SEQUENCE, QUEST_ITEM_TEXT_SEQUENCE_BYTES, QUEST_ITEM_TEXT_SEQUENCE_SHA256,
};
pub use quest_manual_layout::{
    validate_quest_explicit_layout, QuestManualLayoutControl, QuestManualLayoutError,
    QuestManualLayoutPlan, QuestManualStreamLayout,
};
pub use quest_seq::{
    render_town00_internal_address_candidates_tsv, town00_internal_address_candidates,
    verify_town00_seq_roundtrip, QuestSeqDirectBranchRecord, QuestSeqError,
    QuestSeqGlobalWordBranchHeader, QuestSeqOp85Record, QuestSeqOp88Record, QuestSeqOp8fRecord,
    QuestSeqPrefixRecord, QuestSeqResolvedBranchTarget, QuestSeqRoundtripReport,
    Town00ConfirmedPointerField, Town00InternalAddressCandidate, Town00Op85Node, Town00Op88Node,
    Town00Op8fNode, Town00Seq, QUEST_SEQ_ACTIVE_RUNTIME_BASE, QUEST_SEQ_DIRECT_BRANCH_RECORD_LEN,
    QUEST_SEQ_GLOBAL_WORD_BRANCH_HEADER_LEN, QUEST_SEQ_OP88_MAX_TABLE_ENTRIES,
    QUEST_SEQ_OP88_RECORD_LEN, QUEST_SEQ_OP8F_MAX_TABLE_ENTRIES, QUEST_SEQ_OP8F_RECORD_LEN,
    QUEST_SEQ_PREFIX_RECORD_LEN, TOWN00_CONFIRMED_POINTER_FIELDS, TOWN00_PREFIX_RECORDS,
    TOWN00_ROOT_STREAM_OFFSET, TOWN00_ROOT_TARGET_OFFSET, TOWN00_SEQ_LEN, TOWN00_SEQ_RUNTIME_BASE,
    TOWN00_SEQ_SHA256,
};
pub use quest_seq_child_contact::{
    parse_quest_seq_child_contact, QuestSeqChildContact, QuestSeqChildContactError,
    QUEST_SEQ_CHILD_CONFIGURATION_OPCODE, QUEST_SEQ_CHILD_CONFIGURATION_RECORD_BYTES,
    QUEST_SEQ_CHILD_CONTACT_RECORDS_BYTES,
};
pub use quest_seq_inventory::{
    audit_quest_seq_addresses, render_quest_seq_address_candidates_tsv,
    verify_box_quest_seq_addresses, BoxQuestSeqAddressExpectation, QuestSeqAddressAnalysis,
    QuestSeqAddressCandidate, QuestSeqAddressError, QuestSeqAddressEvidence,
    QuestSeqAddressSummary, QuestSeqPointerBehavior, QuestSeqPointerRecordSpec,
    QuestSeqPointerRelation, QuestSeqRecordByteCondition, QuestSeqRuntimeBaseEvidence,
    BOX_QUEST_SEQ_ADDRESS_ANALYSIS_SHA256, BOX_QUEST_SEQ_ADDRESS_EXPECTATIONS,
    BOX_QUEST_SEQ_ADDRESS_FIELDS, BOX_QUEST_SEQ_ADDRESS_FILES, BOX_QUEST_SEQ_ADDRESS_TARGETS,
    BOX_QUEST_SEQ_BYTES, BOX_QUEST_SEQ_POINTER_RECORDS, BOX_QUEST_SEQ_POINTER_RECORD_COUNTS,
    BOX_QUEST_SEQ_RANGE_CANDIDATE_FIELDS, BOX_QUEST_SEQ_RELATIONAL_POINTER_EDGES,
    BOX_QUEST_SEQ_RELATIONAL_POINTER_FIELDS, BOX_QUEST_SEQ_RUNTIME_READ_FIELDS,
    BOX_QUEST_SEQ_SEQUENCES, BOX_QUEST_SEQ_STATIC_POINTER_FIELDS, QUEST_SEQ_POINTER_RECORD_SPECS,
};
pub use quest_seq_op28::{
    quest_seq_op28_fallback_entry_count, quest_seq_op28_flow, quest_seq_op28_handler,
    quest_seq_op28_unique_handler_count, QuestSeqOp28Flow, QUEST_SEQ_OP28_DISPATCH_HANDLERS,
    QUEST_SEQ_OP28_DISPATCH_RUNTIME, QUEST_SEQ_OP28_DISPATCH_TABLE_RUNTIME,
    QUEST_SEQ_OP28_FALLBACK_HANDLER_RUNTIME, QUEST_SEQ_OP28_FLOWS, QUEST_SEQ_OP28_HANDLER_RUNTIME,
    QUEST_SEQ_OP28_SUBOPCODE_COUNT,
};
pub use quest_seq_opcode::{
    quest_seq_base_dispatch_fallback_entries, quest_seq_base_dispatch_unique_handlers,
    quest_seq_dispatch_fallback_entries, quest_seq_dispatch_unique_handlers,
    quest_seq_opcode_handler, QUEST_SEQ_BASE_DISPATCH_HANDLERS,
    QUEST_SEQ_BASE_FALLBACK_HANDLER_RUNTIME, QUEST_SEQ_BASE_OPCODE_COUNT,
    QUEST_SEQ_DISPATCH_HANDLERS, QUEST_SEQ_EXTENDED_FALLBACK_HANDLER_RUNTIME,
    QUEST_SEQ_EXTENDED_OPCODE_BASE, QUEST_SEQ_EXTENDED_OPCODE_COUNT,
    QUEST_SEQ_MAIN_FALLBACK_HANDLER_RUNTIME, QUEST_SEQ_MAIN_OPCODE_COUNT, QUEST_SEQ_OPCODE_BASE,
    QUEST_SEQ_OPCODE_COUNT,
};
pub use quest_seq_text_candidates::{
    audit_quest_seq_text_candidates, render_quest_seq_text_candidates_tsv,
    render_quest_seq_text_summary_tsv, verify_box_quest_seq_text_candidates, QuestSeqTextCandidate,
    QuestSeqTextCandidateAnalysis, QuestSeqTextCandidateError, QuestSeqTextCandidateEvidence,
    QuestSeqTextSummary, BOX_QUEST_SEQ_RUNTIME_OBSERVED_TEXT_SPANS,
    BOX_QUEST_SEQ_STATIC_ONLY_TEXT_CANDIDATES, BOX_QUEST_SEQ_TEXT_BYTES,
    BOX_QUEST_SEQ_TEXT_CANDIDATES, BOX_QUEST_SEQ_TEXT_CANDIDATE_CODE_UNITS,
    BOX_QUEST_SEQ_TEXT_CANDIDATE_FILES, BOX_QUEST_SEQ_TEXT_CANDIDATE_REPORT_SHA256,
    BOX_QUEST_SEQ_TEXT_CANDIDATE_UNIQUE_CHARACTERS, BOX_QUEST_SEQ_TEXT_FILES_WITHOUT_CANDIDATES,
    BOX_QUEST_SEQ_TEXT_FILES_WITH_CANDIDATES, BOX_QUEST_SEQ_TEXT_SEQUENCES,
    BOX_QUEST_SEQ_TEXT_SUMMARY_REPORT_SHA256, QUEST_SEQ_TEXT_CANDIDATE_SCHEMA_VERSION,
    QUEST_SEQ_TEXT_SUMMARY_SCHEMA_VERSION,
};
pub use quest_seq_text_document::{
    audit_quest_seq_text_documents, render_quest_seq_text_document_summary_tsv,
    serialize_quest_seq_text_document, verify_box_quest_seq_text_documents, QuestSeqTextDocument,
    QuestSeqTextDocumentAnalysis, QuestSeqTextDocumentChunk, QuestSeqTextDocumentError,
    BOX_QUEST_SEQ_TEXT_DOCUMENTS, BOX_QUEST_SEQ_TEXT_DOCUMENT_BYTES,
    BOX_QUEST_SEQ_TEXT_DOCUMENT_OPAQUE_BYTES, BOX_QUEST_SEQ_TEXT_DOCUMENT_OPAQUE_CHUNKS,
    BOX_QUEST_SEQ_TEXT_DOCUMENT_REPORT_SHA256, BOX_QUEST_SEQ_TEXT_DOCUMENT_STREAM_BYTES,
    BOX_QUEST_SEQ_TEXT_DOCUMENT_STREAM_CHUNKS, QUEST_SEQ_TEXT_DOCUMENT_SCHEMA_VERSION,
};
pub use quest_seq_text_extraction::{
    audit_quest_seq_text_extraction, render_quest_seq_text_token_ledger_tsv,
    serialize_quest_seq_extracted_text_stream, verify_box_quest_seq_text_extraction,
    QuestSeqExtractedTextStream, QuestSeqExtractedToken, QuestSeqExtractedTokenKind,
    QuestSeqTextExtractionAnalysis, QuestSeqTextExtractionError,
    BOX_QUEST_SEQ_EXTRACTED_CONTROL_OCCURRENCES, BOX_QUEST_SEQ_EXTRACTED_GLYPH_OCCURRENCES,
    BOX_QUEST_SEQ_EXTRACTED_ROUNDTRIP_BYTES, BOX_QUEST_SEQ_EXTRACTED_TEXT_STREAMS,
    BOX_QUEST_SEQ_EXTRACTED_TOKEN_OCCURRENCES, BOX_QUEST_SEQ_TEXT_TOKEN_LEDGER_SHA256,
    QUEST_SEQ_TEXT_TOKEN_LEDGER_SCHEMA_VERSION,
};
pub use quest_seq_text_roots::{
    audit_quest_seq_text_roots, render_quest_seq_text_roots_tsv, verify_box_quest_seq_text_roots,
    QuestSeqTextRootAnalysis, QuestSeqTextRootBaseEvidence, QuestSeqTextRootEdge,
    QuestSeqTextRootError, QuestSeqTextRootEvidence, QuestSeqTextRootMarker,
    QuestSeqTextRootSummary, BOX_QUEST_SEQ_TEXT_ROOT_CODE_UNITS,
    BOX_QUEST_SEQ_TEXT_ROOT_CONSUMER_EDGES, BOX_QUEST_SEQ_TEXT_ROOT_CONSUMER_MARKERS,
    BOX_QUEST_SEQ_TEXT_ROOT_DIRECT_WORDS, BOX_QUEST_SEQ_TEXT_ROOT_DIRECT_WORD_MARKERS,
    BOX_QUEST_SEQ_TEXT_ROOT_FILE_EXPECTATIONS, BOX_QUEST_SEQ_TEXT_ROOT_JAPANESE_CODE_UNITS,
    BOX_QUEST_SEQ_TEXT_ROOT_MARKERS, BOX_QUEST_SEQ_TEXT_ROOT_NO_DIRECT_WORD_MARKERS,
    BOX_QUEST_SEQ_TEXT_ROOT_OTHER_POINTER_WORDS, BOX_QUEST_SEQ_TEXT_ROOT_RANGE_ONLY_MARKERS,
    BOX_QUEST_SEQ_TEXT_ROOT_RANGE_WORDS, BOX_QUEST_SEQ_TEXT_ROOT_REPORT_SHA256,
    BOX_QUEST_SEQ_TEXT_ROOT_RUNS, BOX_QUEST_SEQ_TEXT_ROOT_TOWN00_CONSUMER_MARKERS,
    BOX_QUEST_SEQ_TEXT_ROOT_TRANSFERRED_BASE_CONSUMER_MARKERS, QUEST_SEQ_TEXT_ROOT_SCHEMA_VERSION,
};
pub use quest_seq_text_runtime::{
    audit_quest_seq_text_runtime_evidence, render_quest_seq_text_runtime_evidence_tsv,
    verify_box_quest_seq_text_runtime_evidence, QuestSeqTextRuntimeEvidence,
    QuestSeqTextRuntimeEvidenceAnalysis, QuestSeqTextRuntimeEvidenceError,
    QuestSeqTextRuntimeEvidenceLevel, BOX_QUEST_SEQ_RUNTIME_OP21_HITS,
    BOX_QUEST_SEQ_RUNTIME_TEXT_EVIDENCE, BOX_QUEST_SEQ_RUNTIME_VISIBLE_ONLY,
    BOX_QUEST_SEQ_TEXT_RUNTIME_EVIDENCE_REPORT_SHA256, QUEST_OP21_HANDLER_RUNTIME,
    QUEST_SEQ_TEXT_RUNTIME_EVIDENCE, QUEST_SEQ_TEXT_RUNTIME_EVIDENCE_SCHEMA_VERSION,
};
pub use quest_seq_text_streams::{
    audit_quest_seq_text_streams, render_quest_seq_rooted_text_runs_tsv,
    render_quest_seq_text_streams_tsv, verify_box_quest_seq_text_streams, QuestSeqRootedTextRun,
    QuestSeqTextStreamAnalysis, QuestSeqTextStreamError, QuestSeqTextStreamMarker,
    QuestSeqTextStreamSummary, QuestTextTerminal, BOX_QUEST_SEQ_CANONICAL_INVALID_TEXT_STREAMS,
    BOX_QUEST_SEQ_NONCANONICAL_OP21_SHAPES, BOX_QUEST_SEQ_OP21_SHAPES,
    BOX_QUEST_SEQ_PARSEABLE_TEXT_STREAMS, BOX_QUEST_SEQ_REJECTED_OP21_SHAPES,
    BOX_QUEST_SEQ_ROOTED_TEXT_CODE_UNITS, BOX_QUEST_SEQ_ROOTED_TEXT_FILES,
    BOX_QUEST_SEQ_ROOTED_TEXT_REPORT_SHA256, BOX_QUEST_SEQ_ROOTED_TEXT_RUNS,
    BOX_QUEST_SEQ_ROOTED_TEXT_UNIQUE_CHARACTERS, BOX_QUEST_SEQ_TEXT_STREAM_FILES,
    BOX_QUEST_SEQ_TEXT_STREAM_FILE_EXPECTATIONS, BOX_QUEST_SEQ_TEXT_STREAM_REPORT_SHA256,
    BOX_QUEST_SEQ_UNROOTED_CODE_TABLE_RUNS, QUEST_SEQ_ROOTED_TEXT_RUN_SCHEMA_VERSION,
    QUEST_SEQ_TEXT_MODE_OPCODE, QUEST_SEQ_TEXT_MODE_RECORD_BYTES,
    QUEST_SEQ_TEXT_STREAM_SCHEMA_VERSION,
};
pub use quest_seq_translation_units::{
    audit_quest_seq_translation_units, render_quest_seq_translation_units_tsv,
    verify_box_quest_seq_translation_units, QuestSeqTranslationUnit,
    QuestSeqTranslationUnitAnalysis, QuestSeqTranslationUnitError, QuestSeqTranslationUnitPolicy,
    BOX_QUEST_SEQ_CONTROL_ONLY_REFERENCE_UNITS, BOX_QUEST_SEQ_JAPANESE_TRANSLATION_CANDIDATES,
    BOX_QUEST_SEQ_NON_JAPANESE_REFERENCE_UNITS, BOX_QUEST_SEQ_PROTECTED_RAW_REVIEW_UNITS,
    BOX_QUEST_SEQ_RUNTIME_OBSERVED_TRANSLATION_UNITS, BOX_QUEST_SEQ_TRANSLATION_UNITS,
    BOX_QUEST_SEQ_TRANSLATION_UNIT_REPORT_SHA256, QUEST_SEQ_TRANSLATION_UNIT_SCHEMA_VERSION,
};
pub use quest_terminology::{
    audit_quest_terminology_candidates, quest_terminology_report_sha256,
    render_quest_terminology_candidates_tsv, verify_box_quest_terminology_candidates,
    QuestTerminologyCandidate, QuestTerminologyCandidateAnalysis, QuestTerminologyCategory,
    QuestTerminologyDecision, QuestTerminologyError, QuestTerminologyEvidence,
    QuestTerminologyEvidenceState, QUEST_TERMINOLOGY_CANDIDATE_SCHEMA_VERSION,
};
pub use quest_text_candidates::{
    audit_town00_text_candidates, render_town00_text_candidates_tsv,
    verify_box_town00_text_candidates, Town00TextCandidate, Town00TextCandidateAnalysis,
    Town00TextCandidateError, Town00TextCandidateEvidence, BOX_TOWN00_RUNTIME_OBSERVED_TEXT_SPANS,
    BOX_TOWN00_STATIC_ONLY_TEXT_CANDIDATES, BOX_TOWN00_TEXT_CANDIDATES,
    BOX_TOWN00_TEXT_CANDIDATE_CODE_UNITS, BOX_TOWN00_TEXT_CANDIDATE_REPORT_SHA256,
    BOX_TOWN00_TEXT_CANDIDATE_UNIQUE_CHARACTERS, TOWN00_TEXT_CANDIDATE_MIN_CODE_UNITS,
    TOWN00_TEXT_CANDIDATE_SCHEMA_VERSION,
};
pub use quest_translation::{
    approved_quest_poc_translation, render_quest_dialogue_review_tsv,
    verify_quest_dialogue_translation_manifest, QuestApprovedPocTranslation, QuestDialogueBranch,
    QuestDialogueRuntimeEvidence, QuestDialogueSpan, QuestDialogueTranslationEntry,
    QuestDialogueTranslationError, QuestDialogueTranslationReport, QuestDialogueTranslationStatus,
    QUEST_DEFEAT_DIALOGUE_ID, QUEST_DEFEAT_DIALOGUE_SPANS, QUEST_DEFEAT_REWARD_ID,
    QUEST_DEFEAT_REWARD_SPANS, QUEST_DIALOGUE_REVIEW_SCHEMA_VERSION, QUEST_DIALOGUE_TRANSLATIONS,
    QUEST_VICTORY_DIALOGUE_ID, QUEST_VICTORY_DIALOGUE_SPANS,
};
pub use quest_translation_approvals::{
    audit_quest_translation_approval_manifest, audit_quest_translation_approvals,
    render_quest_translation_approval_manifest_tsv, render_quest_translation_approval_template_tsv,
    verify_box_quest_translation_approvals, QuestTranslationApproval,
    QuestTranslationApprovalAnalysis, QuestTranslationApprovalBaseline,
    QuestTranslationApprovalError, QuestTranslationApprovalKind,
    QUEST_TRANSLATION_APPROVAL_COLUMNS,
};
pub use quest_translation_batches::{
    audit_quest_translation_batches, quest_translation_batch_report_sha256,
    render_quest_translation_batches_tsv, verify_box_quest_translation_batches,
    QuestTranslationBatchAnalysis, QuestTranslationBatchEntry, QuestTranslationBatchError,
    QuestTranslationBatchRole, QuestTranslationContextConfidence, QuestTranslationWorkStatus,
    QUEST_TRANSLATION_BATCH_SCHEMA_VERSION, QUEST_TRANSLATION_WORK_SCHEMA_VERSION,
};
pub use quest_translation_decisions::{
    audit_quest_reviewed_batch_decisions, audit_quest_translation_batch_decisions,
    audit_quest_translation_decision_workbook, quest_translation_decision_workbook_report_sha256,
    render_quest_translation_batch_decisions_tsv, render_quest_translation_decision_workbook_tsv,
    verify_box_quest_first_scene_decisions, verify_box_quest_translation_decision_workbook,
    QuestTranslationDecisionAnalysis, QuestTranslationDecisionDependency,
    QuestTranslationDecisionEntry, QuestTranslationDecisionError, QuestTranslationDecisionWorkbook,
    QuestTranslationReviewAction, QUEST_TRANSLATION_DECISION_PACKET_SCHEMA_VERSION,
    QUEST_TRANSLATION_DECISION_WORKBOOK_SCHEMA_VERSION,
};
pub use quest_translation_review::{
    audit_quest_translation_reviews, quest_translation_review_report_sha256,
    render_quest_translation_reviews_tsv, verify_box_quest_translation_reviews,
    QuestTranslationIndependentReviewStatus, QuestTranslationReviewAnalysis,
    QuestTranslationReviewEntry, QuestTranslationReviewError, QuestTranslationReviewRequirement,
    QUEST_TRANSLATION_REVIEW_ASSET_SCHEMA_VERSION, QUEST_TRANSLATION_REVIEW_SCHEMA_VERSION,
};
pub use quest_translation_selection::{
    apply_quest_manual_composition_selections, audit_quest_translation_selection,
    quest_translation_selection_report_sha256, render_quest_translation_selection_tsv,
    select_quest_manual_compositions_for_development, verify_box_quest_translation_selection,
    QuestManualCompositionSelection, QuestTranslationSelectionAnalysis,
    QuestTranslationSelectionDisposition, QuestTranslationSelectionEntry,
    QuestTranslationSelectionError, QUEST_TRANSLATION_SELECTION_SCHEMA_VERSION,
};
pub use subgame_seq_custom_byte_record::{
    render_subgame_seq_custom_byte_records_tsv, verify_box_subgame_seq_custom_byte_records,
    SubgameSeqCustomByteRecordAnalysis, SubgameSeqCustomByteRecordError,
    SubgameSeqCustomByteRecordRow, BOX_SUBGAME_SEQ_CUSTOM_BYTE_CANDIDATE_ID,
    BOX_SUBGAME_SEQ_CUSTOM_BYTE_RECORDS, BOX_SUBGAME_SEQ_CUSTOM_BYTE_REPORT_SHA256,
    SUBGAME_SEQ_CUSTOM_BYTE_RECORD_SCHEMA_VERSION,
};
pub use subgame_seq_drac_relink::{
    render_subgame_seq_drac_relinks_tsv, repack_box_subgame_seq_drac_messages,
    verify_box_subgame_seq_drac_relinks, SubgameSeqDracRelinkAnalysis, SubgameSeqDracRelinkError,
    SubgameSeqDracRelinkPointer, SubgameSeqDracRelinkRow, SubgameSeqDracRelinkSpec,
    SubgameSeqDracRepack, BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_CAPACITY_BYTES,
    BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_EXTRA_TWO_BYTE_GLYPHS,
    BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_SLACK_BYTES, BOX_SUBGAME_SEQ_DRAC_RELINK_MESSAGE_RECORDS,
    BOX_SUBGAME_SEQ_DRAC_RELINK_ORIGINAL_MESSAGE_BYTES,
    BOX_SUBGAME_SEQ_DRAC_RELINK_POINTERS_PER_COPY, BOX_SUBGAME_SEQ_DRAC_RELINK_REPORT_SHA256,
    BOX_SUBGAME_SEQ_DRAC_RELINK_ROWS, BOX_SUBGAME_SEQ_DRAC_RELINK_SPECS,
    SUBGAME_SEQ_DRAC_RELINK_SCHEMA_VERSION,
};
pub use subgame_seq_drac_text::{
    render_subgame_seq_drac_text_tsv, verify_box_subgame_seq_drac_text, SubgameSeqDracSjisSpan,
    SubgameSeqDracTextAnalysis, SubgameSeqDracTextError, SubgameSeqDracTextKind,
    SubgameSeqDracTextRow, BOX_SUBGAME_SEQ_DRAC_KANA_CANDIDATES_IN_MESSAGES,
    BOX_SUBGAME_SEQ_DRAC_KANA_CANDIDATES_IN_REPERTOIRES, BOX_SUBGAME_SEQ_DRAC_MESSAGE_CODE_UNITS,
    BOX_SUBGAME_SEQ_DRAC_MESSAGE_PROTECTED_BYTES, BOX_SUBGAME_SEQ_DRAC_MESSAGE_RECORDS,
    BOX_SUBGAME_SEQ_DRAC_MESSAGE_SPANS, BOX_SUBGAME_SEQ_DRAC_MISSED_MESSAGE_SPANS,
    BOX_SUBGAME_SEQ_DRAC_REPERTOIRES, BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_CODE_UNITS,
    BOX_SUBGAME_SEQ_DRAC_TEXT_REPORT_SHA256, BOX_SUBGAME_SEQ_DRAC_TEXT_ROWS,
    SUBGAME_SEQ_DRAC_TEXT_SCHEMA_VERSION,
};
pub use subgame_seq_drac_translation_plan::{
    plan_box_subgame_seq_drac_translation, SubgameSeqDracGlyphMapping, SubgameSeqDracTranslation,
    SubgameSeqDracTranslationPlan, SubgameSeqDracTranslationPlanError,
    BOX_SUBGAME_SEQ_DRAC_MESSAGE_CAPACITY_BYTES, BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_BYTES,
    BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_SLOTS, BOX_SUBGAME_SEQ_DRAC_TRANSLATION_MESSAGES,
    SUBGAME_SEQ_DRAC_TRANSLATION_PLAN_SCHEMA_VERSION,
};
pub use subgame_seq_layouts::{
    audit_subgame_seq_layouts, parse_subgame_seq_overlay_footer, render_subgame_seq_layouts_tsv,
    verify_box_subgame_seq_layouts, BoxSubgameSeqLayoutExpectation, SubgameSeqLayoutAnalysis,
    SubgameSeqLayoutError, SubgameSeqLayoutKind, SubgameSeqLayoutRow, SubgameSeqOverlayFooter,
    SubgameSeqRootEvidence, BOX_SUBGAME_SEQ_LAYOUT_EXPECTATIONS, BOX_SUBGAME_SEQ_LAYOUT_FILES,
    BOX_SUBGAME_SEQ_LAYOUT_REPORT_SHA256, BOX_SUBGAME_SEQ_LOAD_BASE_COUNTS,
    BOX_SUBGAME_SEQ_OVERLAYS, BOX_SUBGAME_SEQ_OVERLAY_CANDIDATES,
    BOX_SUBGAME_SEQ_OVERLAY_CODE_UNITS, BOX_SUBGAME_SEQ_RAW_CANDIDATES,
    BOX_SUBGAME_SEQ_RAW_CODE_UNITS, BOX_SUBGAME_SEQ_RAW_OPAQUE, SUBGAME_SEQ_LAYOUT_SCHEMA_VERSION,
    SUBGAME_SEQ_OVERLAY_FOOTER_LEN,
};
pub use subgame_seq_scene_text::{
    audit_subgame_seq_scene_text, render_subgame_seq_scene_text_tsv,
    verify_box_subgame_seq_min_text, SubgameSeqSceneSjisSpan, SubgameSeqSceneTextAnalysis,
    SubgameSeqSceneTextError, SubgameSeqSceneTextKind, SubgameSeqSceneTextRow,
    SubgameSeqSceneTextSpec, BOX_SUBGAME_SEQ_MIN_DIRECT_POINTERS,
    BOX_SUBGAME_SEQ_MIN_MESSAGE_CODE_UNITS, BOX_SUBGAME_SEQ_MIN_MESSAGE_PROTECTED_BYTES,
    BOX_SUBGAME_SEQ_MIN_MESSAGE_RECORDS, BOX_SUBGAME_SEQ_MIN_MESSAGE_SPANS,
    BOX_SUBGAME_SEQ_MIN_MESSAGE_TERMINATORS, BOX_SUBGAME_SEQ_MIN_REPERTOIRE_CODE_UNITS,
    BOX_SUBGAME_SEQ_MIN_TEXT_REPORT_SHA256, BOX_SUBGAME_SEQ_MIN_TEXT_ROWS,
    BOX_SUBGAME_SEQ_MIN_TEXT_SPECS, BOX_SUBGAME_SEQ_MIN_ZERO_RESERVE_BYTES,
    SUBGAME_SEQ_SCENE_TEXT_SCHEMA_VERSION,
};
pub use subgame_seq_text_candidates::{
    audit_subgame_seq_text_candidates, render_subgame_seq_text_candidates_tsv,
    render_subgame_seq_text_summary_tsv, verify_box_subgame_seq_text_candidates,
    BoxSubgameSeqTextExpectation, SubgameSeqTextCandidate, SubgameSeqTextCandidateAnalysis,
    SubgameSeqTextCandidateError, SubgameSeqTextSummary, BOX_SUBGAME_SEQ_TEXT_ANALYSIS_SHA256,
    BOX_SUBGAME_SEQ_TEXT_CANDIDATE_REPORT_SHA256, BOX_SUBGAME_SEQ_TEXT_EXPECTATIONS,
    BOX_SUBGAME_SEQ_TEXT_SUMMARY_REPORT_SHA256, SUBGAME_SEQ_TEXT_CANDIDATE_SCHEMA_VERSION,
    SUBGAME_SEQ_TEXT_MIN_CODE_UNITS, SUBGAME_SEQ_TEXT_SUMMARY_SCHEMA_VERSION,
};
pub use subgame_seq_text_roots::{
    audit_subgame_seq_text_roots, render_subgame_seq_text_roots_tsv,
    verify_box_subgame_seq_text_roots, SubgameSeqTextPointerEvidence, SubgameSeqTextPointerRoot,
    SubgameSeqTextRootAnalysis, SubgameSeqTextRootError, SubgameSeqTextRootRow,
    BOX_SUBGAME_SEQ_TEXT_ROOT_CANDIDATES, BOX_SUBGAME_SEQ_TEXT_ROOT_EXACT_CANDIDATES,
    BOX_SUBGAME_SEQ_TEXT_ROOT_EXACT_RELATIONS, BOX_SUBGAME_SEQ_TEXT_ROOT_HEADER_BYTES,
    BOX_SUBGAME_SEQ_TEXT_ROOT_HEADER_CANDIDATES, BOX_SUBGAME_SEQ_TEXT_ROOT_HEADER_RELATIONS,
    BOX_SUBGAME_SEQ_TEXT_ROOT_NONE_CANDIDATES, BOX_SUBGAME_SEQ_TEXT_ROOT_REPORT_SHA256,
    SUBGAME_SEQ_TEXT_ROOT_SCHEMA_VERSION,
};
