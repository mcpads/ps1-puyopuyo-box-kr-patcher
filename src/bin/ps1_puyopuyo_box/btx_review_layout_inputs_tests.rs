use super::*;

fn candidate(
    resolution: BtxReviewLayoutResolution,
    layouts: Vec<ps1_puyopuyo_box::pipeline::BtxDescriptorStorageLayout>,
) -> ps1_puyopuyo_box::pipeline::BtxReviewLayoutCandidateEvidence {
    ps1_puyopuyo_box::pipeline::BtxReviewLayoutCandidateEvidence {
        family_key: "scope:MENU:indexed4".to_owned(),
        candidate_id: "BTXU-0001".to_owned(),
        decompressed_sha256: "a".repeat(64),
        physical_copies: 2,
        copies_with_setup: 2,
        copies_with_descriptor_sized_setup: usize::from(
            resolution == BtxReviewLayoutResolution::ExactAcrossCopies,
        ) * 2,
        copies_with_descriptor_compatible_setup: 2,
        copies_with_graphics_selector: 0,
        graphics_selector_records: 0,
        other_type_numeric_matches: 0,
        entry_trace_code_matches: 0,
        descriptor_sized_layouts: Vec::new(),
        descriptor_storage_layouts: layouts,
        resolution,
    }
}

fn analysis(
    candidate: ps1_puyopuyo_box::pipeline::BtxReviewLayoutCandidateEvidence,
) -> BtxReviewLayoutEvidenceAnalysis {
    BtxReviewLayoutEvidenceAnalysis {
        copies: Vec::new(),
        candidates: vec![candidate],
        families: 1,
        unique_contents: 1,
        physical_copies: 2,
        copies_with_setup: 2,
        copies_with_descriptor_sized_setup: 0,
        copies_with_descriptor_compatible_setup: 2,
        copies_with_graphics_selector: 0,
        graphics_selector_records: 0,
        other_type_numeric_matches: 0,
        entry_trace_code_matches: 0,
        exact_candidates: 0,
        row_padded_candidates: 1,
        partial_candidates: 0,
        multiple_layout_candidates: 0,
        candidates_without_compatible_setup: 0,
    }
}

#[test]
fn 재판독입력은_완전한_exact와_row_padded후보만_고른다() {
    let evidence = analysis(candidate(
        BtxReviewLayoutResolution::RowPaddedAcrossCopies,
        vec![ps1_puyopuyo_box::pipeline::BtxDescriptorStorageLayout {
            width: 320,
            stored_height: 252,
            atlas_height: 256,
        }],
    ));

    let layouts = source_bound_review_layouts(&evidence).unwrap();

    assert_eq!(layouts.len(), 1);
    assert_eq!(layouts[0].width, 320);
    assert_eq!(layouts[0].stored_height, 252);
    assert_eq!(layouts[0].atlas_height, 256);
    assert_eq!(layouts[0].basis, BtxFamilyReviewLayoutBasis::RowPaddedAtlas);
}

#[test]
fn 재판독입력은_복수layout을_단일preview로_축약하지_않는다() {
    let evidence = analysis(candidate(
        BtxReviewLayoutResolution::ExactAcrossCopies,
        vec![
            ps1_puyopuyo_box::pipeline::BtxDescriptorStorageLayout {
                width: 32,
                stored_height: 48,
                atlas_height: 48,
            },
            ps1_puyopuyo_box::pipeline::BtxDescriptorStorageLayout {
                width: 48,
                stored_height: 32,
                atlas_height: 32,
            },
        ],
    ));

    assert!(source_bound_review_layouts(&evidence).is_err());
}
