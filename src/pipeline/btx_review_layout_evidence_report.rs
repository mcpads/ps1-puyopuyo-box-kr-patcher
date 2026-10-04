//! BTX 검토 layout·selector 근거의 교환용 TSV를 직렬화한다.

use std::fmt::Write as _;

use crate::btx::PixelFormat;

use super::btx_review_layout_evidence::{
    BtxDescriptorStorageLayout, BtxReviewLayoutEvidenceAnalysis,
};

pub const BTX_REVIEW_LAYOUT_EVIDENCE_SCHEMA_VERSION: &str = "btx-review-layout-evidence-v3";

fn pixel_format_label(format: PixelFormat) -> &'static str {
    match format {
        PixelFormat::Indexed4 => "indexed4",
        PixelFormat::Indexed8 => "indexed8",
    }
}

fn render_layouts(layouts: &[(u16, u16)]) -> String {
    if layouts.is_empty() {
        return "none".to_owned();
    }
    layouts
        .iter()
        .map(|(width, height)| format!("{width}x{height}"))
        .collect::<Vec<_>>()
        .join("|")
}

fn render_storage_layouts(layouts: &[BtxDescriptorStorageLayout]) -> String {
    if layouts.is_empty() {
        return "none".to_owned();
    }
    layouts
        .iter()
        .map(|layout| {
            format!(
                "{}x{}-in-{}",
                layout.width, layout.stored_height, layout.atlas_height
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// 후보별 결론과 물리 사본별 setup 근거를 한 TSV로 직렬화한다.
pub fn render_btx_review_layout_evidence_tsv(analysis: &BtxReviewLayoutEvidenceAnalysis) -> String {
    let mut output = String::from(
        "schema_version\trow_kind\tfamily_key\tcandidate_id\tdecompressed_sha256\tformat\tdecompressed_bytes\treview_layout\tphysical_copies\tcopies_with_setup\tcopies_with_descriptor_sized_setup\tcopies_with_descriptor_compatible_setup\tdescriptor_sized_layouts\tdescriptor_storage_layouts\tresolution\texe\tarchive\tasset_name\tvdd_entry_index\tdescriptor_resource_id\tdescriptor_runtime\tdescriptor_selection_runtime\tsetup_storage\tsetup_source\tsetup_runtime\ttable_runtime\tsetup_layout\treview_layout_matches\tdescriptor_plain_layout_matches\tdescriptor_storage_height\tdescriptor_padding_rows\tcopies_with_graphics_selector\tgraphics_selector_records\tother_type_numeric_matches\tentry_trace_code_matches\tselector_storage\tselector_source\tselector_runtime\tselector_form\tselector_flags\tselector_record_type\tselects_graphics_archive\tselector_entry_trace_instruction\n",
    );
    let append_row = |output: &mut String, fields: Vec<String>| {
        debug_assert_eq!(fields.len(), 43);
        writeln!(output, "{}", fields.join("\t")).expect("String write는 실패하지 않음");
    };
    for candidate in &analysis.candidates {
        let mut fields = vec![String::new(); 43];
        fields[0] = BTX_REVIEW_LAYOUT_EVIDENCE_SCHEMA_VERSION.to_owned();
        fields[1] = "candidate".to_owned();
        fields[2] = candidate.family_key.clone();
        fields[3] = candidate.candidate_id.clone();
        fields[4] = candidate.decompressed_sha256.clone();
        fields[8] = candidate.physical_copies.to_string();
        fields[9] = candidate.copies_with_setup.to_string();
        fields[10] = candidate.copies_with_descriptor_sized_setup.to_string();
        fields[11] = candidate
            .copies_with_descriptor_compatible_setup
            .to_string();
        fields[12] = render_layouts(&candidate.descriptor_sized_layouts);
        fields[13] = render_storage_layouts(&candidate.descriptor_storage_layouts);
        fields[14] = candidate.resolution.label().to_owned();
        fields[31] = candidate.copies_with_graphics_selector.to_string();
        fields[32] = candidate.graphics_selector_records.to_string();
        fields[33] = candidate.other_type_numeric_matches.to_string();
        fields[34] = candidate.entry_trace_code_matches.to_string();
        append_row(&mut output, fields);
    }
    for copy in &analysis.copies {
        let copy_fields = || {
            let mut fields = vec![String::new(); 43];
            fields[0] = BTX_REVIEW_LAYOUT_EVIDENCE_SCHEMA_VERSION.to_owned();
            fields[2] = copy.family_key.clone();
            fields[3] = copy.candidate_id.clone();
            fields[4] = copy.decompressed_sha256.clone();
            fields[5] = pixel_format_label(copy.format).to_owned();
            fields[6] = copy.decompressed_bytes.to_string();
            fields[7] = format!("{}x{}", copy.review_width, copy.review_height);
            fields[15] = copy.exe.to_owned();
            fields[16] = copy.archive.clone();
            fields[17] = copy.asset_name.clone();
            fields[18] = copy.vdd_entry_index.to_string();
            fields[19] = copy.descriptor_resource_id.to_string();
            fields[20] = format!("0x{:08X}", copy.descriptor_runtime);
            fields[21] = copy
                .descriptor_selection_runtime
                .map(|runtime| format!("0x{runtime:08X}"))
                .unwrap_or_default();
            fields
        };
        if copy.setups.is_empty() {
            let mut fields = copy_fields();
            fields[1] = "copy".to_owned();
            append_row(&mut output, fields);
        } else {
            for setup in &copy.setups {
                let mut fields = copy_fields();
                fields[1] = "copy-setup".to_owned();
                fields[22] = setup.storage.label().to_owned();
                fields[23] = setup.source.clone();
                fields[24] = format!("0x{:08X}", setup.record_runtime);
                fields[25] = format!("0x{:08X}", setup.table_runtime);
                fields[26] = format!("{}x{}", setup.atlas_width, setup.atlas_height);
                fields[27] = setup.review_layout_matches.to_string();
                fields[28] = setup.descriptor_plain_layout_matches.to_string();
                fields[29] = setup
                    .descriptor_storage_height
                    .map(|height| height.to_string())
                    .unwrap_or_default();
                fields[30] = setup
                    .descriptor_padding_rows
                    .map(|rows| rows.to_string())
                    .unwrap_or_default();
                append_row(&mut output, fields);
            }
        }
        for selector in &copy.selectors {
            let mut fields = copy_fields();
            fields[1] = "copy-selector".to_owned();
            fields[35] = selector.storage.label().to_owned();
            fields[36] = selector.source.clone();
            fields[37] = format!("0x{:08X}", selector.record_runtime);
            fields[38] = selector.form.label().to_owned();
            fields[39] = selector
                .flags
                .map(|flags| format!("0x{flags:02X}"))
                .unwrap_or_default();
            fields[40] = selector.record_type.to_string();
            fields[41] = selector.selects_graphics_archive().to_string();
            fields[42] = selector
                .entry_trace_instruction
                .map(|reached| reached.to_string())
                .unwrap_or_default();
            append_row(&mut output, fields);
        }
    }
    output
}
