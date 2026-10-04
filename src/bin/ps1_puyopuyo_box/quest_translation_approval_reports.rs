//! QUEST 번역 decision·approval report 명령.

use std::path::PathBuf;

use ps1_puyopuyo_box::script::quest_seq_translation_units::verify_box_quest_seq_translation_units;
use ps1_puyopuyo_box::script::quest_terminology::verify_box_quest_terminology_candidates;
use ps1_puyopuyo_box::script::quest_translation_approvals::{
    audit_quest_translation_approvals, render_quest_translation_approval_template_tsv,
};
use ps1_puyopuyo_box::script::quest_translation_batches::verify_box_quest_translation_batches;
use ps1_puyopuyo_box::script::quest_translation_decisions::{
    audit_quest_translation_batch_decisions, render_quest_translation_batch_decisions_tsv,
    render_quest_translation_decision_workbook_tsv, verify_box_quest_translation_decision_workbook,
};
use ps1_puyopuyo_box::script::quest_translation_review::verify_box_quest_translation_reviews;

use super::quest_analysis_inputs::read_quest_seq_text_extraction_analysis;
use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn write_quest_translation_batch_decisions(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let arguments = args.collect::<Vec<_>>();
    let [batch_id, source_path, output_path] = arguments.as_slice() else {
        return Err(usage().to_string());
    };
    let batch_id = batch_id
        .to_str()
        .ok_or_else(|| "QUEST batch ID가 UTF-8이 아님".to_string())?;
    let source_path = PathBuf::from(source_path);
    let output_path = PathBuf::from(output_path);
    let extraction = read_quest_seq_text_extraction_analysis(&source_path)?;
    let translation_units =
        verify_box_quest_seq_translation_units(&extraction).map_err(|error| error.to_string())?;
    let terminology = verify_box_quest_terminology_candidates(&translation_units)
        .map_err(|error| error.to_string())?;
    let batches =
        verify_box_quest_translation_batches(&extraction, &translation_units, &terminology)
            .map_err(|error| error.to_string())?;
    let reviews =
        verify_box_quest_translation_reviews(&batches).map_err(|error| error.to_string())?;
    let analysis = audit_quest_translation_batch_decisions(&reviews, &terminology, batch_id)
        .map_err(|error| error.to_string())?;
    let report = render_quest_translation_batch_decisions_tsv(&analysis);
    write_report_file(
        &output_path,
        "QUEST translation batch human decision TSV",
        analysis.entries.len(),
        &report,
    )?;
    println!(
        "  batch {}, terms {} (pending {}, conflicts {}, approved {}), reviews agrees {}, \
         proposals {}, dependency decisions {}, pending human decisions {}, build-eligible {}",
        analysis.batch_id,
        analysis.affected_terms,
        analysis.pending_term_decisions,
        analysis.conflicting_term_decisions,
        analysis.approved_term_decisions,
        analysis.review_agreements,
        analysis.review_proposals,
        analysis.review_human_decisions,
        analysis.pending_human_decisions,
        analysis.build_eligible_entries,
    );
    Ok(())
}

pub(super) fn write_quest_translation_decision_workbook(
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
    let workbook = verify_box_quest_translation_decision_workbook(&reviews, &terminology)
        .map_err(|error| error.to_string())?;
    let report = render_quest_translation_decision_workbook_tsv(&workbook);
    let rows = workbook.terms.len() + workbook.dependencies.len() + workbook.entries;
    write_report_file(
        output_path,
        "QUEST translation human decision workbook TSV",
        rows,
        &report,
    )?;
    println!(
        "  batches {}, entries {} (agrees {}, proposals {}, dependency decisions {}), \
         terms {} (pending {}, conflicts {}, approved {}), shared dependencies {} / occurrences {}, \
         build-eligible {}",
        workbook.batch_decisions.len(),
        workbook.entries,
        workbook.review_agreements,
        workbook.review_proposals,
        workbook.review_human_decisions,
        workbook.terms.len(),
        workbook.pending_term_decisions,
        workbook.conflicting_term_decisions,
        workbook.approved_term_decisions,
        workbook.dependencies.len(),
        workbook.dependency_occurrences,
        workbook.build_eligible_entries,
    );
    Ok(())
}

pub(super) fn write_quest_translation_approval_template(
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
    let workbook = verify_box_quest_translation_decision_workbook(&reviews, &terminology)
        .map_err(|error| error.to_string())?;
    let approvals =
        audit_quest_translation_approvals(&workbook).map_err(|error| error.to_string())?;
    let report = render_quest_translation_approval_template_tsv(&approvals);
    write_report_file(
        output_path,
        "QUEST translation approval template TSV",
        approvals.baselines.len(),
        &report,
    )?;
    println!(
        "  approved dependencies {}, entries {}, waiting terms/dependencies {}/{}, \
         build-eligible {}",
        approvals.approved_dependencies,
        approvals.approved_entries,
        approvals.entry_approvals_waiting_for_terms,
        approvals.entry_approvals_waiting_for_dependencies,
        approvals.build_eligible_entries,
    );
    Ok(())
}
