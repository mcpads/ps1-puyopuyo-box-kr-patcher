//! 사람이 번역 후보와 근거를 비교하고 명시적 결정을 남기는 경계.

pub mod btx_family_preview;
pub mod btx_family_review;
pub mod btx_family_review_html;
pub mod btx_family_review_store;
pub mod btx_family_roles;
pub mod quest_manual_composition;
pub mod quest_manual_composition_catalog;
pub mod quest_translation;
pub mod quest_translation_approval_store;
pub mod quest_translation_html;
pub mod quest_translation_source_layout;

pub use btx_family_preview::{render_btx_family_preview_bmp, BtxFamilyPreviewError};
pub use btx_family_review::{
    audit_btx_family_review_decisions, btx_family_review_baselines,
    render_btx_family_review_decisions_tsv, render_pending_btx_family_review_decisions_tsv,
    verify_box_btx_family_review_decisions, BtxFamilyReviewAnalysis, BtxFamilyReviewBaseline,
    BtxFamilyReviewDecision, BtxFamilyReviewDecisionKind, BtxFamilyReviewError,
    BTX_FAMILY_REVIEW_DECISION_SCHEMA_VERSION,
};
pub use btx_family_review_html::{
    render_btx_family_layout_review_page, render_btx_family_review_page, BtxFamilyReviewHtmlError,
    BtxFamilyReviewLayout, BtxFamilyReviewLayoutBasis,
};
pub use btx_family_review_store::{
    apply_btx_family_review_decision, save_btx_family_review_decision,
    BtxFamilyReviewManifestUpdate, BtxFamilyReviewSelection, BtxFamilyReviewStoreError,
};
pub use btx_family_roles::{
    audit_btx_family_roles, verify_box_btx_family_roles, BtxFamilyAssetRole, BtxFamilyRoleAnalysis,
    BtxFamilyRoleBasis, BtxFamilyRoleError, BtxFamilyRoleRow, BtxFamilyRoleSpec,
    BOX_BTX_FAMILY_ROLE_SPECS,
};

pub use quest_manual_composition::{
    audit_quest_manual_compositions, QuestCompositionCandidateBasis, QuestManualComposition,
    QuestManualCompositionAnalysis, QuestManualCompositionError, QuestManualCompositionSegment,
    QuestManualCompositionStatus, QUEST_MANUAL_COMPOSITION_SCHEMA_VERSION,
    QUEST_MANUAL_COMPOSITION_SEGMENT_SCHEMA_VERSION,
};
pub use quest_manual_composition_catalog::{
    audit_quest_manual_composition_catalog, QuestManualCompositionCatalogError,
};
pub use quest_translation::{
    build_quest_translation_review_catalog, build_quest_translation_review_queue,
    QuestReviewBatchSummary, QuestReviewCandidate, QuestReviewCandidateSource, QuestReviewCatalog,
    QuestReviewDependency, QuestReviewEntry, QuestReviewQueue, QuestReviewQueueError,
    QuestReviewTerm,
};
pub use quest_translation_approval_store::{
    apply_quest_translation_approval, save_quest_translation_approval, QuestApprovalManifestUpdate,
    QuestApprovalSelection, QuestApprovalStoreError,
};
pub use quest_translation_html::{render_quest_review_page, QuestReviewHtmlError};
pub use quest_translation_source_layout::{
    build_quest_review_queue_source_layouts, build_quest_source_layout, QuestSourceLayout,
    QuestSourceLayoutControl, QuestSourceLayoutError, QuestSourceLayoutSpan,
};
