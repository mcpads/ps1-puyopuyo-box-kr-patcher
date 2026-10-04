use super::*;
use crate::pipeline::btx_review_layout_evidence_report::render_btx_review_layout_evidence_tsv;

fn candidate(copies: usize) -> BtxFamilyCandidate {
    BtxFamilyCandidate {
        candidate_id: "BTXU-test".to_owned(),
        family_id: "BTXFAM-test".to_owned(),
        family_key: "subgame:TEST:indexed4".to_owned(),
        review_lane: crate::graphics::btx_family_candidates::BtxReviewLane::UiFontTextCandidate,
        confidence: crate::graphics::btx_family_candidates::BtxCandidateConfidence::High,
        candidate_reason: "fixture",
        decompressed_sha1: "1".repeat(40),
        decompressed_sha256: "2".repeat(64),
        format: PixelFormat::Indexed4,
        decompressed_bytes: 256,
        pixel_count: 512,
        dimension_candidates: 1,
        review_width: 32,
        review_height: 16,
        palette_histogram: vec![512],
        candidate_background_index: 0,
        occupied_box: None,
        horizontal_occupied_bands: 0,
        vertical_occupied_bands: 0,
        complete_16x16_cells: 2,
        unique_16x16_cells: 1,
        repeated_16x16_cells: 1,
        background_16x16_cells: 2,
        partial_16x16_edges: false,
        physical_copies: (0..copies)
            .map(|index| BtxPhysicalCandidateCopy {
                archive: format!("/{index}_GRP.VDD;1"),
                name: "TEST.BTX".to_owned(),
                resource_index: index,
                static_evidence: "fixture".to_owned(),
                static_reference_sites: 0,
                descriptor_runtime: 0,
            })
            .collect(),
    }
}

fn copy(
    candidate: &BtxFamilyCandidate,
    exact_layouts: &[(u16, u16)],
) -> BtxReviewLayoutCopyEvidence {
    BtxReviewLayoutCopyEvidence {
        family_key: candidate.family_key.clone(),
        candidate_id: candidate.candidate_id.clone(),
        decompressed_sha256: candidate.decompressed_sha256.clone(),
        format: candidate.format,
        decompressed_bytes: candidate.decompressed_bytes,
        review_width: candidate.review_width,
        review_height: candidate.review_height,
        exe: "TEST.EXE",
        archive: "/TEST_GRP.VDD;1".to_owned(),
        asset_name: "TEST.BTX".to_owned(),
        vdd_entry_index: 0,
        descriptor_resource_id: 0,
        descriptor_runtime: 0x8001_0000,
        descriptor_selection_runtime: None,
        setups: exact_layouts
            .iter()
            .map(|&(width, height)| BtxReviewSetupEvidence {
                storage: BtxReviewSetupStorage::Executable,
                source: "TEST.EXE".to_owned(),
                record_runtime: 0x8001_0010,
                table_runtime: 0x8001_0040,
                atlas_width: width,
                atlas_height: height,
                review_layout_matches: false,
                descriptor_plain_layout_matches: true,
                descriptor_storage_height: Some(height),
                descriptor_padding_rows: Some(0),
            })
            .collect(),
        selectors: Vec::new(),
    }
}

#[test]
fn 같은_descriptor_sized_layout이_모든_사본에_있으면_일관됨이다() {
    let candidate = candidate(2);
    let copies = vec![copy(&candidate, &[(32, 16)]), copy(&candidate, &[(32, 16)])];

    let summary = summarize_candidate_layouts(&candidate, &copies).unwrap();

    assert_eq!(summary.descriptor_sized_layouts, vec![(32, 16)]);
    assert_eq!(
        summary.resolution,
        BtxReviewLayoutResolution::ExactAcrossCopies
    );
}

#[test]
fn 일부_사본만_descriptor_sized_setup이면_부분_근거다() {
    let candidate = candidate(2);
    let copies = vec![copy(&candidate, &[(32, 16)]), copy(&candidate, &[])];

    let summary = summarize_candidate_layouts(&candidate, &copies).unwrap();

    assert_eq!(summary.copies_with_descriptor_sized_setup, 1);
    assert_eq!(
        summary.resolution,
        BtxReviewLayoutResolution::PartialAcrossCopies
    );
}

#[test]
fn descriptor_sized_layout이_여럿이면_하나로_축약하지_않는다() {
    let candidate = candidate(1);
    let copies = vec![copy(&candidate, &[(32, 16), (16, 32)])];

    let summary = summarize_candidate_layouts(&candidate, &copies).unwrap();

    assert_eq!(summary.descriptor_sized_layouts, vec![(16, 32), (32, 16)]);
    assert_eq!(
        summary.resolution,
        BtxReviewLayoutResolution::MultipleLayouts
    );
}

#[test]
fn setup이_없으면_판독용_layout을_정적_근거로_승격하지_않는다() {
    let candidate = candidate(1);
    let copies = vec![copy(&candidate, &[])];

    let summary = summarize_candidate_layouts(&candidate, &copies).unwrap();

    assert!(summary.descriptor_sized_layouts.is_empty());
    assert_eq!(
        summary.resolution,
        BtxReviewLayoutResolution::NoCompatibleSetup
    );
}

#[test]
fn selector는_grptype과_같은숫자의_다른type을_분리한다() {
    let raw = [
        0xE1, 0x00, 0x00, 0x00, 0xE1, 0x00, 0x00, 0x00, 0x28, 0x12, 0x97, 0x00, 0xE1, 0x00, 0x00,
        0x00, 0x28, 0x10, 0x97, 0x01, 0xE1, 0x00, 0x00, 0x00,
    ];

    let records = scan_selector_records(
        BtxReviewSetupStorage::SequenceOverlay,
        "TEST.SEQ",
        &raw,
        raw.len(),
        0x8010_0000,
        0x00E1,
        None,
    )
    .unwrap();

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].record_runtime, 0x8010_0008);
    assert!(records[0].selects_graphics_archive());
    assert_eq!(records[1].record_runtime, 0x8010_0010);
    assert_eq!(records[1].form, SubgameSelectorRecordForm::Op28Sub10);
    assert!(!records[1].selects_graphics_archive());
}

#[test]
fn main_selector형식도_같은_resource공간에서_분리한다() {
    let raw = [
        0x39, 0x00, 0xAF, 0x00, 0xFF, 0xFF, 0x00, 0x00, // OP39 type 0
        0xC3, 0xFF, 0xFF, 0x01, 0xAF, 0x00, 0x00, 0x00, // OPC3 type 1
    ];

    let records = scan_selector_records(
        BtxReviewSetupStorage::SequenceOverlay,
        "TEST.SEQ",
        &raw,
        raw.len(),
        0x8010_0000,
        0x00AF,
        None,
    )
    .unwrap();

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].form, SubgameSelectorRecordForm::MainOp39);
    assert_eq!(records[0].flags, None);
    assert!(records[0].selects_graphics_archive());
    assert_eq!(records[1].form, SubgameSelectorRecordForm::MainOpC3);
    assert!(!records[1].selects_graphics_archive());
}

#[test]
fn exe_entry_trace의_명령바이트는_selector후보에서_제외한다() {
    let raw = [0xC3, 0x27, 0x04, 0x00, 0x02, 0x00, 0x03, 0x24];
    let entry_trace = BTreeSet::from([0x8007_B4B0]);

    let records = scan_selector_records(
        BtxReviewSetupStorage::Executable,
        "TEST.EXE",
        &raw,
        raw.len(),
        0x8007_B4B0,
        2,
        Some(&entry_trace),
    )
    .unwrap();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].entry_trace_instruction, Some(true));
    assert!(!records[0].is_record_candidate());
    assert!(!records[0].selects_graphics_archive());
}

#[test]
fn 정렬되지_않은_selector모양은_실행record로_세지_않는다() {
    let raw = [
        0x00, 0x28, 0x12, 0x97, 0x00, 0xA7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    let records = scan_selector_records(
        BtxReviewSetupStorage::SequenceOverlay,
        "TEST.SEQ",
        &raw,
        raw.len(),
        0x8010_0000,
        0x00A7,
        None,
    )
    .unwrap();

    assert!(records.is_empty());
}

#[test]
fn atlas의_끝_padding_row는_저장폭_근거를_보존한다() {
    let layout = descriptor_storage_layout(PixelFormat::Indexed8, 80_640, 320, 256).unwrap();

    assert_eq!(
        layout,
        BtxDescriptorStorageLayout {
            width: 320,
            stored_height: 252,
            atlas_height: 256,
        }
    );
}

#[test]
fn tsv의_모든_행은_header와_같은_field수를_가진다() {
    let candidate = candidate(2);
    let mut no_setup = copy(&candidate, &[]);
    no_setup.selectors.push(BtxReviewSelectorEvidence {
        storage: BtxReviewSetupStorage::SequenceOverlay,
        source: "TEST.SEQ".to_owned(),
        record_runtime: 0x8010_0000,
        form: SubgameSelectorRecordForm::Op28Sub12,
        flags: Some(0x88),
        record_type: 0,
        entry_trace_instruction: None,
    });
    let with_setup = copy(&candidate, &[(32, 16)]);
    let candidate_summary =
        summarize_candidate_layouts(&candidate, &[no_setup.clone(), with_setup.clone()]).unwrap();
    let analysis = BtxReviewLayoutEvidenceAnalysis {
        copies: vec![no_setup, with_setup],
        candidates: vec![candidate_summary],
        families: 1,
        unique_contents: 1,
        physical_copies: 2,
        copies_with_setup: 1,
        copies_with_descriptor_sized_setup: 1,
        copies_with_descriptor_compatible_setup: 1,
        copies_with_graphics_selector: 1,
        graphics_selector_records: 1,
        other_type_numeric_matches: 0,
        entry_trace_code_matches: 0,
        exact_candidates: 0,
        row_padded_candidates: 0,
        partial_candidates: 1,
        multiple_layout_candidates: 0,
        candidates_without_compatible_setup: 0,
    };

    let report = render_btx_review_layout_evidence_tsv(&analysis);
    let field_counts = report
        .lines()
        .map(|line| line.split('\t').count())
        .collect::<Vec<_>>();

    assert!(field_counts.iter().all(|count| *count == field_counts[0]));
    assert!(report.contains("copy-selector"));
}
