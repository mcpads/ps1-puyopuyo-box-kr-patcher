//! QUEST 번역 검토 원장에서 development 선택·routing·page 조판을 만든다.

use sha2::{Digest, Sha256};

use ps1_puyopuyo_box::coords::ExeMap;
use ps1_puyopuyo_box::disc::VerifiedSourceImage;
use ps1_puyopuyo_box::font::quest::QuestCodeTable;
use ps1_puyopuyo_box::pipeline::quest_development_page_packing::QuestDevelopmentPagePackingAnalysis;
use ps1_puyopuyo_box::pipeline::quest_manual_stream_writer::QuestManualStreamLayout;
use ps1_puyopuyo_box::pipeline::quest_translation_glyph_cache_hook_plan::verify_box_quest_translation_glyph_cache_hook_plan;
use ps1_puyopuyo_box::pipeline::quest_translation_glyph_cache_package::verify_box_quest_translation_glyph_cache_package;
use ps1_puyopuyo_box::pipeline::quest_translation_glyph_cache_plan::verify_box_quest_translation_glyph_cache_plan;
use ps1_puyopuyo_box::pipeline::quest_translation_routing::{
    render_quest_translation_routing_tsv, verify_box_quest_translation_routing,
    QuestTranslationRoutingAnalysis,
};
use ps1_puyopuyo_box::pipeline::quest_translation_selection_routing::{
    render_quest_translation_selection_routing_tsv, verify_box_quest_translation_selection_routing,
    QuestTranslationSelectionRoutingAnalysis,
};
use ps1_puyopuyo_box::review::quest_manual_composition_catalog::audit_quest_manual_composition_catalog;
use ps1_puyopuyo_box::script::quest_seq_inventory::QuestSeqAddressAnalysis;
use ps1_puyopuyo_box::script::quest_seq_text_document::QuestSeqTextDocumentAnalysis;
use ps1_puyopuyo_box::script::quest_seq_text_extraction::QuestSeqTextExtractionAnalysis;
use ps1_puyopuyo_box::script::quest_seq_translation_units::verify_box_quest_seq_translation_units;
use ps1_puyopuyo_box::script::quest_terminology::verify_box_quest_terminology_candidates;
use ps1_puyopuyo_box::script::quest_translation_approvals::verify_box_quest_translation_approvals;
use ps1_puyopuyo_box::script::quest_translation_batches::{
    verify_box_quest_translation_batches, QuestTranslationBatchAnalysis,
};
use ps1_puyopuyo_box::script::quest_translation_decisions::{
    audit_quest_reviewed_batch_decisions, verify_box_quest_first_scene_decisions,
    verify_box_quest_translation_decision_workbook,
};
use ps1_puyopuyo_box::script::quest_translation_review::{
    verify_box_quest_translation_reviews, QuestTranslationReviewAnalysis,
};
use ps1_puyopuyo_box::script::quest_translation_selection::{
    verify_box_quest_translation_selection, QuestTranslationSelectionAnalysis,
};

use super::super::super::quest_manual_composition_development::{
    build_quest_manual_composition_development, QuestManualCompositionDevelopmentInput,
};

pub(super) struct QuestTranslationDevelopmentAuditInputs<'a> {
    pub source: &'a VerifiedSourceImage,
    pub executable_map: &'a ExeMap,
    pub executable_text: &'a [u8],
    pub text_extraction: &'a QuestSeqTextExtractionAnalysis,
    pub text_documents: &'a QuestSeqTextDocumentAnalysis,
    pub seq_addresses: &'a QuestSeqAddressAnalysis,
    pub code_table: &'a QuestCodeTable,
}

pub(super) struct QuestTranslationDevelopmentAudit {
    pub translation_batches: QuestTranslationBatchAnalysis,
    pub translation_reviews: QuestTranslationReviewAnalysis,
    pub translation_routing: QuestTranslationRoutingAnalysis,
    pub development_selection: QuestTranslationSelectionAnalysis,
    pub development_routing: QuestTranslationSelectionRoutingAnalysis,
    pub development_layouts: Vec<QuestManualStreamLayout>,
    pub development_page_packing: QuestDevelopmentPagePackingAnalysis,
}

pub(super) fn audit_quest_translation_development(
    inputs: QuestTranslationDevelopmentAuditInputs<'_>,
) -> Result<QuestTranslationDevelopmentAudit, String> {
    let QuestTranslationDevelopmentAuditInputs {
        source,
        executable_map,
        executable_text,
        text_extraction,
        text_documents,
        seq_addresses,
        code_table,
    } = inputs;

    let translation_units = verify_box_quest_seq_translation_units(text_extraction)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST SEQ translation review: units {}, Japanese candidates {}, \
         non-Japanese references {}, protected-raw review {}, control-only {}, \
         runtime-observed {}",
        translation_units.units.len(),
        translation_units.japanese_translation_candidates,
        translation_units.non_japanese_reference_units,
        translation_units.protected_raw_review_units,
        translation_units.control_only_reference_units,
        translation_units.runtime_observed_units,
    );
    let terminology = verify_box_quest_terminology_candidates(&translation_units)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST terminology candidates: terms {}, evidence {}, pending {}, \
         conflicts {}, approved {}",
        terminology.terms.len(),
        terminology.evidence_rows,
        terminology.pending_terms,
        terminology.conflicting_terms,
        terminology.approved_terms,
    );
    let translation_batches =
        verify_box_quest_translation_batches(text_extraction, &translation_units, &terminology)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST translation batches: batches {}, entries {} \
         (translate {}, context {}, drafts {}, term decisions {}), runtime-observed {}, \
         affected terms {}, build-eligible {}, draft Hangul syllables {}, unassigned {}",
        translation_batches.batches,
        translation_batches.entries.len(),
        translation_batches.translate_entries,
        translation_batches.protected_context_entries,
        translation_batches.drafted_entries,
        translation_batches.term_decision_entries,
        translation_batches.runtime_observed_entries,
        translation_batches.affected_terms,
        translation_batches.build_eligible_entries,
        translation_batches.draft_hangul_repertoire.chars().count(),
        translation_batches
            .unassigned_translation_candidate_ids
            .len(),
    );
    let translation_reviews = verify_box_quest_translation_reviews(&translation_batches)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST translation review queue: entries {} \
         / assets {} (protected {}, term decisions {}, context decisions {}, runtime samples {}, \
         independent {}), reviewed {} (agrees {}, proposals {}, human decisions {}), \
         unreviewed drafts {}, build-eligible {}",
        translation_reviews.entries.len(),
        translation_reviews.review_assets,
        translation_reviews.protected_context_entries,
        translation_reviews.human_term_decision_entries,
        translation_reviews.human_context_decision_entries,
        translation_reviews.runtime_sample_entries,
        translation_reviews.independent_review_queue_entries,
        translation_reviews.reviewed_entries,
        translation_reviews.review_agreements,
        translation_reviews.review_proposals,
        translation_reviews.review_human_decision_rows,
        translation_reviews.unreviewed_drafts,
        translation_reviews.build_eligible_entries,
    );
    let reviewed_batch_decisions =
        audit_quest_reviewed_batch_decisions(&translation_reviews, &terminology)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST reviewed-batch human decisions: batches {}, entries {}, build-eligible {}",
        reviewed_batch_decisions.len(),
        reviewed_batch_decisions
            .iter()
            .map(|analysis| analysis.entries.len())
            .sum::<usize>(),
        reviewed_batch_decisions
            .iter()
            .map(|analysis| analysis.build_eligible_entries)
            .sum::<usize>(),
    );
    let first_scene_decisions =
        verify_box_quest_first_scene_decisions(&translation_reviews, &terminology)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST first-scene human decisions: entries {}, terms {} \
         (pending {}, conflicts {}, approved {}), reviews {}/{}/{}, \
         pending human decisions {}, build-eligible {}",
        first_scene_decisions.entries.len(),
        first_scene_decisions.affected_terms,
        first_scene_decisions.pending_term_decisions,
        first_scene_decisions.conflicting_term_decisions,
        first_scene_decisions.approved_term_decisions,
        first_scene_decisions.review_agreements,
        first_scene_decisions.review_proposals,
        first_scene_decisions.review_human_decisions,
        first_scene_decisions.pending_human_decisions,
        first_scene_decisions.build_eligible_entries,
    );
    let translation_decisions =
        verify_box_quest_translation_decision_workbook(&translation_reviews, &terminology)
            .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST translation decision workbook: batches {}, terms {}, dependencies {}, \
         entries {}, build-eligible {}",
        translation_decisions.batch_decisions.len(),
        translation_decisions.terms.len(),
        translation_decisions.dependencies.len(),
        translation_decisions.entries,
        translation_decisions.build_eligible_entries,
    );
    let translation_approvals = verify_box_quest_translation_approvals(&translation_decisions)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST translation approvals: baselines {}, approved dependencies {}, \
         approved entries {}, waiting terms/dependencies {}/{}, build-eligible {}",
        translation_approvals.baselines.len(),
        translation_approvals.approved_dependencies,
        translation_approvals.approved_entries,
        translation_approvals.entry_approvals_waiting_for_terms,
        translation_approvals.entry_approvals_waiting_for_dependencies,
        translation_approvals.build_eligible_entries,
    );
    let manual_compositions = audit_quest_manual_composition_catalog(
        &translation_decisions,
        &translation_approvals,
        &text_extraction.streams,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST manual composition drafts: compositions {}, segments {}, \
         geometry-verified {}, layout-approved {}, product-eligible {}",
        manual_compositions.manually_authored,
        manual_compositions
            .compositions
            .iter()
            .map(|composition| composition.segments.len())
            .sum::<usize>(),
        manual_compositions.geometry_verified,
        manual_compositions.layout_approved,
        manual_compositions.product_eligible,
    );
    let translation_selection = verify_box_quest_translation_selection(&translation_reviews)
        .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST translation selection: selected {}, retained source {}, \
         batches {}, Hangul {}, new/draft-only cache glyphs {}/{}, product-eligible {}",
        translation_selection.selected_entries,
        translation_selection.retained_source_entries,
        translation_selection.selected_batches,
        translation_selection
            .selected_hangul_repertoire
            .chars()
            .count(),
        translation_selection.new_cache_glyphs.chars().count(),
        translation_selection.draft_only_glyphs.chars().count(),
        translation_selection.product_eligible_entries,
    );
    let translation_routing =
        verify_box_quest_translation_routing(text_documents, seq_addresses, &translation_batches)
            .map_err(|error| error.to_string())?;
    let translation_routing_sha256 = format!(
        "{:x}",
        Sha256::digest(render_quest_translation_routing_tsv(&translation_routing).as_bytes())
    );
    println!(
        "PASS QUEST translation routing draft: routes {}, pages {}, resolved {} \
         (inline {}, relocated {}, pending draft decisions {}), sequences {} / tail fit {}, \
         active base observed/static {}/{}, binding proven {}, relocated {}B, minimum active-window \
         remaining {}B, build-eligible {}, report SHA-256 {}",
        translation_routing.routes.len(),
        translation_routing.pages,
        translation_routing.resolved_drafts,
        translation_routing.inline_candidates,
        translation_routing.relocated_candidates,
        translation_routing.pending_draft_decisions,
        translation_routing.sequences.len(),
        translation_routing
            .sequences
            .iter()
            .filter(|sequence| sequence.known_tail_fits)
            .count(),
        translation_routing.runtime_base_observed_sequences,
        translation_routing.static_loader_runtime_base_sequences,
        translation_routing.runtime_base_bindings_proven,
        translation_routing.known_relocated_bytes,
        translation_routing.minimum_active_window_remaining,
        translation_routing.build_eligible_routes,
        translation_routing_sha256,
    );
    let translation_selection_routing = verify_box_quest_translation_selection_routing(
        text_documents,
        &translation_routing,
        &translation_selection,
    )
    .map_err(|error| error.to_string())?;
    let translation_selection_routing_sha256 = format!(
        "{:x}",
        Sha256::digest(
            render_quest_translation_selection_routing_tsv(&translation_selection_routing)
                .as_bytes(),
        )
    );
    println!(
        "PASS QUEST translation selection routing: selected {}, retained source {}, pages {}, \
         inline {}, relocated {} / {}B, sequences {} / tail fit {}, \
         active base observed/static {}/{}, binding proven {}, minimum active-window remaining {}B, \
         product-eligible {}, report SHA-256 {}",
        translation_selection_routing.selected,
        translation_selection_routing.retained_source,
        translation_selection_routing.selected_pages,
        translation_selection_routing.inline,
        translation_selection_routing.relocated,
        translation_selection_routing.known_relocated_bytes,
        translation_selection_routing.sequences.len(),
        translation_selection_routing
            .sequences
            .iter()
            .filter(|sequence| sequence.known_tail_fits)
            .count(),
        translation_selection_routing.runtime_base_observed_sequences,
        translation_selection_routing.static_loader_runtime_base_sequences,
        translation_selection_routing.runtime_base_bindings_proven,
        translation_selection_routing.minimum_active_window_remaining,
        translation_selection_routing.product_eligible_routes,
        translation_selection_routing_sha256,
    );
    let translation_glyph_cache = verify_box_quest_translation_glyph_cache_plan(
        source,
        &translation_selection,
        &translation_selection_routing,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST translation glyph cache plan: glyphs {}, directory/selected pages {}/{}, \
         max page {}, assignments {}/{}B, dictionary rows {}, catalog/original-source/package \
         {}/{}/{}B, carrier remaining {}B, product-eligible {}",
        translation_glyph_cache.plan.glyphs,
        translation_glyph_cache.plan.pages,
        translation_glyph_cache.selected_pages,
        translation_glyph_cache.plan.max_page_glyphs,
        translation_glyph_cache.plan.page_assignments,
        translation_glyph_cache.plan.page_assignment_bytes,
        translation_glyph_cache.plan.catalog.dictionary_rows,
        translation_glyph_cache.plan.catalog.stored_bytes,
        translation_glyph_cache
            .plan
            .original_atlas_source
            .stored_bytes,
        translation_glyph_cache.plan.package_bytes,
        translation_glyph_cache.plan.carrier_remaining_bytes,
        translation_glyph_cache.product_eligible_routes,
    );
    let translation_glyph_package = verify_box_quest_translation_glyph_cache_package(
        source,
        &translation_selection,
        &translation_selection_routing,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST translation glyph cache package: bytes {}, SHA-256 {}, \
         scratch offset/bytes {}/{}, sections exact {}, runtime {}, build-eligible {}",
        translation_glyph_package.bytes.len(),
        translation_glyph_package.sha256,
        translation_glyph_package.layout.scratch_offset,
        translation_glyph_package.layout.scratch_bytes,
        translation_glyph_package.sections_exact,
        translation_glyph_package.runtime_verified,
        translation_glyph_package.build_eligible,
    );
    let translation_glyph_hook = verify_box_quest_translation_glyph_cache_hook_plan(
        executable_map,
        executable_text,
        &translation_glyph_package,
        &translation_selection_routing,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "PASS QUEST translation glyph cache typed hook: package/body {}/{}B, \
         body runtime 0x{:08X}, carrier used/remaining {}/{}B, max uploads {}, \
         relocated {}, runtime {}, build-eligible {}",
        translation_glyph_hook.package_bytes,
        translation_glyph_hook.body_bytes,
        translation_glyph_hook.body_runtime,
        translation_glyph_hook.carrier_used_bytes,
        translation_glyph_hook.carrier_remaining_bytes,
        translation_glyph_hook.max_page_uploads,
        translation_glyph_hook.relocated_routes,
        translation_glyph_hook.runtime_verified,
        translation_glyph_hook.build_eligible,
    );

    let development =
        build_quest_manual_composition_development(QuestManualCompositionDevelopmentInput {
            manual_compositions: &manual_compositions,
            source_selection: &translation_selection,
            text_documents,
            source_routing: &translation_routing,
            code_table,
        })?;

    Ok(QuestTranslationDevelopmentAudit {
        translation_batches,
        translation_reviews,
        translation_routing,
        development_selection: development.selection,
        development_routing: development.routing,
        development_layouts: development.layouts,
        development_page_packing: development.page_packing,
    })
}
