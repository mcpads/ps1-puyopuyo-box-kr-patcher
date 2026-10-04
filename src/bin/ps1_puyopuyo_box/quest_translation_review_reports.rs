//! QUEST 번역 unit·용어·batch·review·selection report 명령.

use std::path::PathBuf;

use ps1_puyopuyo_box::pipeline::quest_translation_selection_routing::render_quest_translation_selection_routing_tsv;
use ps1_puyopuyo_box::script::quest_seq_translation_units::{
    render_quest_seq_translation_units_tsv, verify_box_quest_seq_translation_units,
};
use ps1_puyopuyo_box::script::quest_terminology::{
    render_quest_terminology_candidates_tsv, verify_box_quest_terminology_candidates,
};
use ps1_puyopuyo_box::script::quest_translation_batches::{
    render_quest_translation_batches_tsv, verify_box_quest_translation_batches,
};
use ps1_puyopuyo_box::script::quest_translation_review::{
    render_quest_translation_reviews_tsv, verify_box_quest_translation_reviews,
};
use ps1_puyopuyo_box::script::quest_translation_selection::{
    render_quest_translation_selection_tsv, verify_box_quest_translation_selection,
};

use super::quest_analysis_inputs::{
    read_quest_seq_text_extraction_analysis, read_quest_translation_selection_routing_analysis,
};
use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn write_quest_seq_translation_unit_review(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let extraction = read_quest_seq_text_extraction_analysis(source_path)?;
    let analysis =
        verify_box_quest_seq_translation_units(&extraction).map_err(|error| error.to_string())?;
    let report = render_quest_seq_translation_units_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST SEQ translation unit review TSV",
        analysis.units.len(),
        &report,
    )?;
    println!(
        "  Japanese candidates {}, non-Japanese references {}, protected-raw review {}, \
         control-only {}, runtime-observed {}",
        analysis.japanese_translation_candidates,
        analysis.non_japanese_reference_units,
        analysis.protected_raw_review_units,
        analysis.control_only_reference_units,
        analysis.runtime_observed_units,
    );
    Ok(())
}

pub(super) fn write_quest_terminology_candidate_review(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let extraction = read_quest_seq_text_extraction_analysis(source_path)?;
    let translation_units =
        verify_box_quest_seq_translation_units(&extraction).map_err(|error| error.to_string())?;
    let analysis = verify_box_quest_terminology_candidates(&translation_units)
        .map_err(|error| error.to_string())?;
    let report = render_quest_terminology_candidates_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST terminology candidate review TSV",
        analysis.terms.len(),
        &report,
    )?;
    println!(
        "  evidence {}, pending {}, conflicts {}, approved {}",
        analysis.evidence_rows,
        analysis.pending_terms,
        analysis.conflicting_terms,
        analysis.approved_terms,
    );
    Ok(())
}

pub(super) fn write_quest_translation_batch_review(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let extraction = read_quest_seq_text_extraction_analysis(source_path)?;
    let translation_units =
        verify_box_quest_seq_translation_units(&extraction).map_err(|error| error.to_string())?;
    let terminology = verify_box_quest_terminology_candidates(&translation_units)
        .map_err(|error| error.to_string())?;
    let analysis =
        verify_box_quest_translation_batches(&extraction, &translation_units, &terminology)
            .map_err(|error| error.to_string())?;
    let report = render_quest_translation_batches_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST scene translation batch review TSV",
        analysis.entries.len(),
        &report,
    )?;
    println!(
        "  batches {}, translate {}, protected context {}, drafts {}, term decisions {}, \
         runtime-observed {}, affected terms {}, build-eligible {}, draft Hangul syllables {}, \
         unassigned {}",
        analysis.batches,
        analysis.translate_entries,
        analysis.protected_context_entries,
        analysis.drafted_entries,
        analysis.term_decision_entries,
        analysis.runtime_observed_entries,
        analysis.affected_terms,
        analysis.build_eligible_entries,
        analysis.draft_hangul_repertoire.chars().count(),
        analysis.unassigned_translation_candidate_ids.len(),
    );
    Ok(())
}

pub(super) fn write_quest_translation_review_queue(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let extraction = read_quest_seq_text_extraction_analysis(source_path)?;
    let translation_units =
        verify_box_quest_seq_translation_units(&extraction).map_err(|error| error.to_string())?;
    let terminology = verify_box_quest_terminology_candidates(&translation_units)
        .map_err(|error| error.to_string())?;
    let batches =
        verify_box_quest_translation_batches(&extraction, &translation_units, &terminology)
            .map_err(|error| error.to_string())?;
    let analysis =
        verify_box_quest_translation_reviews(&batches).map_err(|error| error.to_string())?;
    let report = render_quest_translation_reviews_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST translation independent review queue TSV",
        analysis.entries.len(),
        &report,
    )?;
    println!(
        "  review assets {}, protected {}, term decisions {}, context decisions {}, runtime samples {}, \
         independent {}, reviewed {} (agrees {}, proposals {}, human decisions {}), \
         unreviewed drafts {}, build-eligible {}",
        analysis.review_assets,
        analysis.protected_context_entries,
        analysis.human_term_decision_entries,
        analysis.human_context_decision_entries,
        analysis.runtime_sample_entries,
        analysis.independent_review_queue_entries,
        analysis.reviewed_entries,
        analysis.review_agreements,
        analysis.review_proposals,
        analysis.review_human_decision_rows,
        analysis.unreviewed_drafts,
        analysis.build_eligible_entries,
    );
    Ok(())
}

pub(super) fn write_quest_translation_selection(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let extraction = read_quest_seq_text_extraction_analysis(source_path)?;
    let translation_units =
        verify_box_quest_seq_translation_units(&extraction).map_err(|error| error.to_string())?;
    let terminology = verify_box_quest_terminology_candidates(&translation_units)
        .map_err(|error| error.to_string())?;
    let batches =
        verify_box_quest_translation_batches(&extraction, &translation_units, &terminology)
            .map_err(|error| error.to_string())?;
    let reviews =
        verify_box_quest_translation_reviews(&batches).map_err(|error| error.to_string())?;
    let analysis =
        verify_box_quest_translation_selection(&reviews).map_err(|error| error.to_string())?;
    let report = render_quest_translation_selection_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST translation selection TSV",
        analysis.entries.len(),
        &report,
    )?;
    println!(
        "  selected {}, retained source {}, batches {}, Hangul {}, new/draft-only cache glyphs {}/{}, \
         translation-approved {}, product-eligible {}",
        analysis.selected_entries,
        analysis.retained_source_entries,
        analysis.selected_batches,
        analysis.selected_hangul_repertoire.chars().count(),
        analysis.new_cache_glyphs.chars().count(),
        analysis.draft_only_glyphs.chars().count(),
        analysis.translation_approved_entries,
        analysis.product_eligible_entries,
    );
    Ok(())
}

pub(super) fn write_quest_translation_selection_routing(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let analysis = read_quest_translation_selection_routing_analysis(source_path)?;
    let report = render_quest_translation_selection_routing_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST translation selection routing TSV",
        analysis.routes.len(),
        &report,
    )?;
    println!(
        "  selected {}, retained source {}, pages {}, inline {}, relocated {} / {}B, \
         sequences {} / tail fit {}, minimum active-window remaining {}B, product-eligible {}",
        analysis.selected,
        analysis.retained_source,
        analysis.selected_pages,
        analysis.inline,
        analysis.relocated,
        analysis.known_relocated_bytes,
        analysis.sequences.len(),
        analysis
            .sequences
            .iter()
            .filter(|sequence| sequence.known_tail_fits)
            .count(),
        analysis.minimum_active_window_remaining,
        analysis.product_eligible_routes,
    );
    Ok(())
}
