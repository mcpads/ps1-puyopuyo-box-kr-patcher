//! QUEST 누적 개발 disc의 검산 결과를 안정된 TSV 스키마로 직렬화한다.

use std::fmt::Write as _;

use super::QuestDevelopmentDiscReport;

const QUEST_DEVELOPMENT_DISC_BASE_HEADER: &str = concat!(
    "schema_version\tsequence_entries\ttarget_sequence_entries\t",
    "non_target_sequence_entries\tselected_routes\tretained_source_routes\tinline_routes\t",
    "relocated_routes\tsource_plain_bytes\toutput_plain_bytes\tsource_target_cnx_bytes\t",
    "output_target_cnx_bytes\tseq_vdd_lba\tseq_vdd_bytes\tseq_vdd_sectors\t",
    "seq_vdd_sha256\tseq_vdd_repacked\tseq_vdd_relocated\tseq_vdd_exact\t",
    "target_plain_exact\ttarget_cnx_roundtrip_exact\tnon_target_plain_exact\t",
    "non_target_cnx_exact\toriginal_font_archive_unchanged\tpackage_bytes\tpackage_sha256\t",
    "body_runtime\tbody_bytes\trelocated_quest_bytes\tmachine_code_writes\tdata_writes\t",
    "metadata_writes\ttouched_sectors\tfinal_expected_writes\timage_sha256\t",
    "manual_layout_routes\ttranslation_approved_routes\tlayout_approved_routes\t",
    "runtime_verified\tproduct_eligible"
);

fn write_quest_development_disc_base_row(output: &mut String, report: &QuestDevelopmentDiscReport) {
    write!(
        *output,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        report.schema_version,
        report.sequence_entries,
        report.target_sequence_entries,
        report.non_target_sequence_entries,
        report.selected_routes,
        report.retained_source_routes,
        report.inline_routes,
        report.relocated_routes,
        report.source_plain_bytes,
        report.output_plain_bytes,
        report.source_target_cnx_bytes,
        report.output_target_cnx_bytes,
        report.seq_vdd_lba,
        report.seq_vdd_bytes,
        report.seq_vdd_sectors,
        report.seq_vdd_sha256,
        report.seq_vdd_repacked,
        report.seq_vdd_relocated,
        report.seq_vdd_exact,
        report.target_plain_exact,
        report.target_cnx_roundtrip_exact,
        report.non_target_plain_exact,
        report.non_target_cnx_exact,
        report.original_font_archive_unchanged,
        report.disc.package_bytes,
        report.disc.package_sha256,
        report.disc.body_runtime,
        report.disc.body_bytes,
        report.disc.relocated_quest_bytes,
        report.disc.machine_code_writes,
        report.disc.data_writes,
        report.disc.metadata_writes,
        report.disc.touched_sectors,
        report.disc.final_expected_writes,
        report.disc.image_sha256,
        report.explicit_layout_routes,
        report.translation_approved_routes,
        report.layout_approved_routes,
        report.runtime_verified,
        report.product_eligible,
    )
    .expect("String write는 실패하지 않음");
}

/// resident와 ITEM_DAT 이름·설명 source/final 결속을 가진 development 스키마를 기록한다.
pub fn render_quest_development_disc_tsv(report: &QuestDevelopmentDiscReport) -> String {
    let resident = &report.resident_text;
    let mut output = String::from(QUEST_DEVELOPMENT_DISC_BASE_HEADER);
    output.push_str(concat!(
        "\tresident_units\tresident_glyph_cells\tresident_source_bytes\t",
        "resident_output_bytes\tresident_status\tresident_page_ids\t",
        "resident_record_runtimes\tresident_source_sha256\tresident_output_sha256\t",
        "resident_source_exact\tresident_controls_exact\tresident_terminal_exact\t",
        "resident_final_exact\tresident_japanese_glyphs_remaining\t",
        "resident_translation_approved\tresident_layout_approved\t",
        "resident_runtime_verified\tresident_product_eligible",
    ));
    output.push_str(concat!(
        "\tbattle_name_page_id\tbattle_name_physical_names\t",
        "battle_name_unique_sources\tbattle_name_glyph_cells\t",
        "battle_name_source_bytes\tbattle_name_output_bytes\t",
        "battle_name_selected_drafts\tbattle_name_needs_human_choice\t",
        "battle_name_unit_ids\tbattle_name_runtimes\t",
        "battle_name_source_sha256\tbattle_name_output_sha256\t",
        "battle_name_source_exact\tbattle_name_nul_terminal_exact\t",
        "battle_name_final_exact\tbattle_name_japanese_glyphs_remaining\t",
        "battle_name_translation_approved\tbattle_name_layout_approved\t",
        "battle_name_runtime_verified\tbattle_name_product_eligible",
    ));
    output.push_str(concat!(
        "\titem_name_page_id\titem_name_units\titem_name_glyph_cells\t",
        "item_name_source_bytes\titem_name_output_bytes\titem_name_source_sequence_bytes\t",
        "item_name_output_sequence_bytes\titem_name_source_sequence_sha256\t",
        "item_name_output_sequence_sha256\titem_name_active_window_remaining\t",
        "item_name_draft_needs_independent_review\titem_name_draft_needs_human_choice\t",
        "item_name_source_exact\titem_name_pointer_readback_exact\t",
        "item_name_nul_terminal_exact\titem_name_combined_relink_exact\t",
        "item_name_final_exact\titem_name_japanese_glyphs_remaining\t",
        "item_name_translation_approved\titem_name_layout_approved\t",
        "item_name_runtime_verified\titem_name_product_eligible",
    ));
    output.push_str(concat!(
        "\titem_description_page_ids\titem_description_units\t",
        "item_description_glyph_cells\titem_description_controls\t",
        "item_description_source_bytes\titem_description_output_bytes\t",
        "item_description_active_window_remaining\t",
        "item_description_draft_needs_independent_review\t",
        "item_description_draft_needs_human_choice\t",
        "item_description_source_exact\titem_description_pointer_readback_exact\t",
        "item_description_controls_exact\titem_description_terminal_exact\t",
        "item_description_final_exact\titem_description_japanese_glyphs_remaining\t",
        "item_description_translation_approved\titem_description_layout_approved\t",
        "item_description_runtime_verified\titem_description_product_eligible",
    ));
    output.push_str(concat!(
        "\titem_acquisition_page_id\titem_acquisition_units\t",
        "item_acquisition_glyph_cells\titem_acquisition_source_bytes\t",
        "item_acquisition_output_bytes\titem_acquisition_status\t",
        "item_acquisition_unit_ids\titem_acquisition_runtimes\t",
        "item_acquisition_source_sha256\titem_acquisition_output_sha256\t",
        "item_acquisition_source_exact\titem_acquisition_nul_terminal_exact\t",
        "item_acquisition_final_exact\titem_acquisition_japanese_glyphs_remaining\t",
        "item_acquisition_translation_approved\titem_acquisition_layout_approved\t",
        "item_acquisition_runtime_verified\titem_acquisition_product_eligible",
    ));
    output.push_str(concat!(
        "\tui_literal_page_id\tui_literal_units\tui_literal_world_map_labels\t",
        "ui_literal_binary_choices\tui_literal_glyph_cells\tui_literal_source_bytes\t",
        "ui_literal_output_bytes\tui_literal_status\tui_literal_unit_ids\t",
        "ui_literal_runtimes\tui_literal_source_sha256\tui_literal_output_sha256\t",
        "ui_literal_source_exact\tui_literal_nul_terminal_exact\tui_literal_final_exact\t",
        "ui_literal_japanese_glyphs_remaining\tui_literal_translation_approved\t",
        "ui_literal_layout_approved\tui_literal_runtime_verified\tui_literal_product_eligible",
    ));
    output.push('\n');
    write_quest_development_disc_base_row(&mut output, report);
    let page_ids = resident
        .writes
        .iter()
        .map(|write| write.page_id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let record_runtimes = resident
        .writes
        .iter()
        .map(|write| format!("0x{:08X}", write.record_runtime))
        .collect::<Vec<_>>()
        .join(",");
    let source_sha256 = resident
        .writes
        .iter()
        .map(|write| write.source_sha256.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let output_sha256 = resident
        .writes
        .iter()
        .map(|write| write.output_sha256.as_str())
        .collect::<Vec<_>>()
        .join(",");
    write!(
        output,
        "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        resident.units,
        resident.glyph_cells,
        resident.source_bytes,
        resident.output_bytes,
        resident.status.label(),
        page_ids,
        record_runtimes,
        source_sha256,
        output_sha256,
        resident.source_exact,
        resident.controls_exact,
        resident.terminal_exact,
        resident.final_exact,
        resident.japanese_glyphs_remaining,
        resident.translation_approved,
        resident.layout_approved,
        resident.runtime_verified,
        resident.product_eligible,
    )
    .expect("String write는 실패하지 않음");
    let names = &report.battle_names;
    let unit_ids = names
        .writes
        .iter()
        .map(|write| write.unit_id.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let runtimes = names
        .writes
        .iter()
        .map(|write| format!("0x{:08X}", write.runtime))
        .collect::<Vec<_>>()
        .join(",");
    let source_sha256 = names
        .writes
        .iter()
        .map(|write| write.source_sha256.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let output_sha256 = names
        .writes
        .iter()
        .map(|write| write.output_sha256.as_str())
        .collect::<Vec<_>>()
        .join(",");
    write!(
        output,
        "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        names.page_id,
        names.physical_names,
        names.unique_sources,
        names.glyph_cells,
        names.source_bytes,
        names.output_bytes,
        names.selected_drafts,
        names.needs_human_choice,
        unit_ids,
        runtimes,
        source_sha256,
        output_sha256,
        names.source_exact,
        names.nul_terminal_exact,
        names.final_exact,
        names.japanese_glyphs_remaining,
        names.translation_approved,
        names.layout_approved,
        names.runtime_verified,
        names.product_eligible,
    )
    .expect("String write는 실패하지 않음");
    let item = &report.item_names;
    write!(
        output,
        "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        item.page_id,
        item.units,
        item.glyph_cells,
        item.source_name_bytes,
        item.output_name_bytes,
        item.source_sequence_bytes,
        item.output_sequence_bytes,
        item.source_sequence_sha256,
        item.output_sequence_sha256,
        item.active_window_remaining,
        item.draft_needs_independent_review,
        item.draft_needs_human_choice,
        item.source_exact,
        item.pointer_readback_exact,
        item.nul_terminal_exact,
        item.combined_relink_exact,
        item.final_exact,
        item.japanese_glyphs_remaining,
        item.translation_approved,
        item.layout_approved,
        item.runtime_verified,
        item.product_eligible,
    )
    .expect("String write는 실패하지 않음");
    let item = &report.item_descriptions;
    write!(
        output,
        "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        item.page_ids
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(","),
        item.units,
        item.glyph_cells,
        item.controls,
        item.source_description_bytes,
        item.output_description_bytes,
        item.active_window_remaining,
        item.draft_needs_independent_review,
        item.draft_needs_human_choice,
        item.source_exact,
        item.pointer_readback_exact,
        item.controls_exact,
        item.terminal_exact,
        item.final_exact,
        item.japanese_glyphs_remaining,
        item.translation_approved,
        item.layout_approved,
        item.runtime_verified,
        item.product_eligible,
    )
    .expect("String write는 실패하지 않음");
    let item = &report.item_acquisition_text;
    let unit_ids = item
        .writes
        .iter()
        .map(|write| write.unit_id.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let runtimes = item
        .writes
        .iter()
        .map(|write| format!("0x{:08X}", write.runtime))
        .collect::<Vec<_>>()
        .join(",");
    let source_sha256 = item
        .writes
        .iter()
        .map(|write| write.source_sha256.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let output_sha256 = item
        .writes
        .iter()
        .map(|write| write.output_sha256.as_str())
        .collect::<Vec<_>>()
        .join(",");
    write!(
        output,
        "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        item.page_id,
        item.units,
        item.glyph_cells,
        item.source_bytes,
        item.output_bytes,
        item.status.label(),
        unit_ids,
        runtimes,
        source_sha256,
        output_sha256,
        item.source_exact,
        item.nul_terminal_exact,
        item.final_exact,
        item.japanese_glyphs_remaining,
        item.translation_approved,
        item.layout_approved,
        item.runtime_verified,
        item.product_eligible,
    )
    .expect("String write는 실패하지 않음");
    let item = &report.ui_literals;
    let unit_ids = item
        .writes
        .iter()
        .map(|write| write.unit_id.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let runtimes = item
        .writes
        .iter()
        .map(|write| format!("0x{:08X}", write.runtime))
        .collect::<Vec<_>>()
        .join(",");
    let source_sha256 = item
        .writes
        .iter()
        .map(|write| write.source_sha256.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let output_sha256 = item
        .writes
        .iter()
        .map(|write| write.output_sha256.as_str())
        .collect::<Vec<_>>()
        .join(",");
    write!(
        output,
        "\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        item.page_id,
        item.units,
        item.world_map_labels,
        item.binary_choices,
        item.glyph_cells,
        item.source_bytes,
        item.output_bytes,
        item.status.label(),
        unit_ids,
        runtimes,
        source_sha256,
        output_sha256,
        item.source_exact,
        item.nul_terminal_exact,
        item.final_exact,
        item.japanese_glyphs_remaining,
        item.translation_approved,
        item.layout_approved,
        item.runtime_verified,
        item.product_eligible,
    )
    .expect("String write는 실패하지 않음");
    output.push('\n');
    output
}
