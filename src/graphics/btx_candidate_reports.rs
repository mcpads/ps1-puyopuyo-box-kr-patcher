//! 미분류 BTX 후보와 family의 안정된 TSV 직렬화.

use super::{
    format_label, BtxFamilyCandidateAnalysis, BtxOccupiedBox, BtxPhysicalCandidateCopy,
    BTX_FAMILY_CANDIDATE_SCHEMA_VERSION,
};

fn join_copy_field(
    copies: &[BtxPhysicalCandidateCopy],
    field: impl Fn(&BtxPhysicalCandidateCopy) -> String,
) -> String {
    copies.iter().map(field).collect::<Vec<_>>().join("|")
}

fn palette_label(histogram: &[usize]) -> String {
    histogram
        .iter()
        .enumerate()
        .filter(|(_, count)| **count != 0)
        .map(|(index, count)| format!("{index}:{count}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn bbox_label(bbox: Option<BtxOccupiedBox>) -> String {
    bbox.map_or_else(
        || "none".to_string(),
        |bbox| format!("{},{},{},{}", bbox.x, bbox.y, bbox.width, bbox.height),
    )
}

/// 현재 미판정 고유 내용 전수의 특징 TSV.
pub fn render_btx_family_candidate_features_tsv(analysis: &BtxFamilyCandidateAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tcandidate_id\tfamily_id\tfamily_key\treview_lane\tconfidence\t\
         candidate_reason\tpixel_format\tbpp\tdecompressed_bytes\tpixel_count\t\
         dimension_candidates\treview_width\treview_height\treview_layout_status\t\
         palette_histogram\tcandidate_background_index\toccupied_bbox_xywh\t\
         horizontal_occupied_bands\tvertical_occupied_bands\tcomplete_16x16_cells\t\
         unique_16x16_cells\trepeated_16x16_cells\tbackground_16x16_cells\t\
         partial_16x16_edges\tphysical_copies\tarchives\tasset_names\tresource_indices\t\
         static_evidence\tstatic_reference_sites\tdescriptor_runtimes\tseq_evidence\t\
         decompressed_sha1\tdecompressed_sha256\tpreview\tstatus\n",
    );
    for row in &analysis.candidates {
        let fields = vec![
            BTX_FAMILY_CANDIDATE_SCHEMA_VERSION.to_string(),
            row.candidate_id.clone(),
            row.family_id.clone(),
            row.family_key.clone(),
            row.review_lane.as_str().to_string(),
            row.confidence.as_str().to_string(),
            row.candidate_reason.to_string(),
            format_label(row.format).to_string(),
            row.format.bits_per_pixel().to_string(),
            row.decompressed_bytes.to_string(),
            row.pixel_count.to_string(),
            row.dimension_candidates.to_string(),
            row.review_width.to_string(),
            row.review_height.to_string(),
            "heuristic-not-consumer-layout".to_string(),
            palette_label(&row.palette_histogram),
            row.candidate_background_index.to_string(),
            bbox_label(row.occupied_box),
            row.horizontal_occupied_bands.to_string(),
            row.vertical_occupied_bands.to_string(),
            row.complete_16x16_cells.to_string(),
            row.unique_16x16_cells.to_string(),
            row.repeated_16x16_cells.to_string(),
            row.background_16x16_cells.to_string(),
            row.partial_16x16_edges.to_string(),
            row.physical_copies.len().to_string(),
            join_copy_field(&row.physical_copies, |copy| copy.archive.clone()),
            join_copy_field(&row.physical_copies, |copy| copy.name.clone()),
            join_copy_field(&row.physical_copies, |copy| copy.resource_index.to_string()),
            join_copy_field(&row.physical_copies, |copy| copy.static_evidence.clone()),
            join_copy_field(&row.physical_copies, |copy| {
                copy.static_reference_sites.to_string()
            }),
            join_copy_field(&row.physical_copies, |copy| {
                format!("0x{:08X}", copy.descriptor_runtime)
            }),
            "not-established-by-family-pass".to_string(),
            row.decompressed_sha1.clone(),
            row.decompressed_sha256.clone(),
            format!(
                "previews/{}-{}.png",
                format_label(row.format),
                row.decompressed_sha1
            ),
            "human-review-required".to_string(),
        ];
        output.push_str(&fields.join("\t"));
        output.push('\n');
    }
    output
}

/// family별 한 대표를 고른 사람 검토용 TSV.
pub fn render_btx_candidate_families_tsv(analysis: &BtxFamilyCandidateAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tfamily_id\tfamily_key\treview_lane\tconfidence\tmembers\t\
         physical_copies\trepresentative_candidate_id\trepresentative_archive\t\
         representative_name\trepresentative_preview\tautomatic_approval\tstatus\n",
    );
    for row in &analysis.families {
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tfalse\thuman-review-required\n",
            BTX_FAMILY_CANDIDATE_SCHEMA_VERSION,
            row.family_id,
            row.family_key,
            row.review_lane.as_str(),
            row.confidence.as_str(),
            row.members,
            row.physical_copies,
            row.representative_candidate_id,
            row.representative_archive,
            row.representative_name,
            row.representative_preview,
        ));
    }
    output
}
