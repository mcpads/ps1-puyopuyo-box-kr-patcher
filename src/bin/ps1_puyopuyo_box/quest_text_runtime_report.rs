//! QUEST text runtime evidence report 명령.

use std::path::PathBuf;

use ps1_puyopuyo_box::script::quest_seq_text_runtime::{
    render_quest_seq_text_runtime_evidence_tsv, verify_box_quest_seq_text_runtime_evidence,
};

use super::quest_analysis_inputs::read_quest_seq_text_extraction_analysis;
use super::translation_reports::write_report_file;
use super::usage::usage;

pub(super) fn write_quest_seq_text_runtime_evidence(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<(), String> {
    let paths = args.map(PathBuf::from).collect::<Vec<_>>();
    let [source_path, output_path] = paths.as_slice() else {
        return Err(usage().to_string());
    };
    let extraction = read_quest_seq_text_extraction_analysis(source_path)?;
    let analysis = verify_box_quest_seq_text_runtime_evidence(&extraction)
        .map_err(|error| error.to_string())?;
    let report = render_quest_seq_text_runtime_evidence_tsv(&analysis);
    write_report_file(
        output_path,
        "QUEST SEQ runtime text evidence TSV",
        analysis.entries.len(),
        &report,
    )?;
    println!(
        "  OP21 hits {}, visible-only {}",
        analysis.op21_hits, analysis.visible_only,
    );
    Ok(())
}
