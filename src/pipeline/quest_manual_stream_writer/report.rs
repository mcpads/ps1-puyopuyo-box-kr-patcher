use std::fmt::Write as _;

use super::model::{QuestManualStreamWriterAnalysis, QUEST_MANUAL_STREAM_WRITER_SCHEMA_VERSION};

/// 실제 SEQ 출력 digest와 비배포 판정을 기록하는 TSV.
pub fn render_quest_manual_stream_writer_tsv(analysis: &QuestManualStreamWriterAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tsequence\tsource_bytes\toutput_bytes\tsource_sha256\toutput_sha256\t\
         selected_routes\tretained_source_routes\tinline_routes\trelocated_routes\ttail_bytes\t\
         changed_bytes\tretained_source_exact\topaque_exact\tinternal_addresses_exact\t\
         non_target_exact\tcontrol_order_exact\tmanual_layout_only\ttranslation_approved\t\
         layout_approved\truntime_verified\tproduct_eligible\n",
    );
    for sequence in analysis
        .sequences
        .iter()
        .filter(|sequence| sequence.selected_routes + sequence.retained_source_routes > 0)
    {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tyes\tno\tno\t{}\t{}",
            QUEST_MANUAL_STREAM_WRITER_SCHEMA_VERSION,
            sequence.sequence,
            sequence.source_bytes,
            sequence.output.len(),
            sequence.source_sha256,
            sequence.output_sha256,
            sequence.selected_routes,
            sequence.retained_source_routes,
            sequence.inline_routes,
            sequence.relocated_routes,
            sequence.tail_bytes,
            sequence.changed_bytes,
            analysis.retained_source_exact,
            analysis.opaque_exact,
            analysis.internal_addresses_exact,
            analysis.non_target_exact,
            analysis.control_order_exact,
            analysis.runtime_verified,
            analysis.product_eligible,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}
