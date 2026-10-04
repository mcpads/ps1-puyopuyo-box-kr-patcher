use super::*;

const ARCHIVES: [&str; 2] = ["/A.VDD;1", "/B.VDD;1"];

fn report() -> BtxPatchStrategyReport {
    BtxPatchStrategyReport {
        schema_version: BTX_PATCH_STRATEGY_SCHEMA_VERSION,
        typed_japanese_blocks: 3,
        mm_spr_managed_blocks: 1,
        fixed_cell_page_blocks: 1,
        remaining_raster_blocks: 1,
        remaining_raster_assets: 1,
        profile_candidate_groups: vec![BtxRasterProfileCandidateGroup {
            pixel_format: "indexed4",
            bbox_height: 16,
            source_lines: 1,
            palette_indices: vec![0, 1, 15],
            blocks: 1,
            assets: 1,
            minimum_width: 32,
            maximum_width: 32,
            block_pixels: 512,
        }],
        profile_candidate_members: vec![BtxRasterProfileCandidateMember {
            candidate_index: 1,
            semantic_family_proposal: "fixture-family",
            semantic_family_purpose: "줄\t설명",
            archive_paths: &ARCHIVES,
            asset_name: "FIXTURE.BTX",
            asset_sha256: "asset",
            block_id: "BLOCK-0001",
            block_pixels_sha256: "block",
            bbox_x: 0,
            bbox_y: 0,
            bbox_width: 32,
            bbox_height: 16,
            protected_source: "日本語",
        }],
        semantic_family_proposals: 1,
        automatic_semantic_family_approval: false,
        pending_menu06_findings: 1,
        pending_menu06_physical_assets: 2,
        protected_mm_mes_findings: 1,
        protected_mm_mes_physical_assets: 2,
        automatic_profile_approval: false,
    }
}

#[test]
fn profile후보_tsv는_합성group을_자동승인없이_내보낸다() {
    let tsv = render_btx_raster_profile_candidates_tsv_from(&report());
    let rows = tsv.lines().collect::<Vec<_>>();

    assert_eq!(rows.len(), 2);
    assert!(rows[1].contains("\t1\tindexed4\t16\t1\t0,1,15\t"));
    assert!(rows[1].ends_with("profile-candidate-human-approval-required"));
}

#[test]
fn profile후보구성원은_보호원문과_제안상태를_함께노출한다() {
    let tsv = render_btx_raster_profile_candidate_members_tsv_from(&report()).unwrap();
    let row = tsv.lines().nth(1).unwrap();

    assert!(row.contains("fixture-family\t줄\\t설명\t"));
    assert!(row.contains("semantic-family-proposal-human-approval-required"));
    assert!(row.contains("/A.VDD;1\\n/B.VDD;1"));
    assert!(row.contains("\tBLOCK-0001\t"));
    assert!(row.ends_with("日本語\tprofile-candidate-human-approval-required"));
}

#[test]
fn 구성원의_profile후보index가_범위밖이면_거부한다() {
    let mut invalid = report();
    invalid.profile_candidate_members[0].candidate_index = 0;
    assert!(
        render_btx_raster_profile_candidate_members_tsv_from(&invalid)
            .unwrap_err()
            .to_string()
            .contains("index가 0")
    );

    invalid.profile_candidate_members[0].candidate_index = 2;
    assert!(
        render_btx_raster_profile_candidate_members_tsv_from(&invalid)
            .unwrap_err()
            .to_string()
            .contains("범위 밖")
    );
}

#[test]
fn palette표현을_profile후보집합으로_변환한다() {
    assert_eq!(
        palette_indices(PaletteIndexSpec::Values(&[0, 2, 15])),
        vec![0, 2, 15]
    );
    assert_eq!(
        palette_indices(PaletteIndexSpec::Bitset256([0b101, 0, 0, 1 << 63])),
        vec![0, 2, 255]
    );
}
