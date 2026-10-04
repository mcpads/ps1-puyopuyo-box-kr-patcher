use crate::btx::PixelFormat;
use crate::graphics::btx_family_candidates::{
    BtxCandidateConfidence, BtxCandidateFamily, BtxFamilyCandidate, BtxFamilyCandidateAnalysis,
    BtxPhysicalCandidateCopy, BtxReviewLane,
};

use super::*;

fn candidate(id: &str, family_id: &str, name: &str, digest_byte: char) -> BtxFamilyCandidate {
    BtxFamilyCandidate {
        candidate_id: id.to_owned(),
        family_id: family_id.to_owned(),
        family_key: format!("scope:{name}:indexed4"),
        review_lane: BtxReviewLane::UiFontTextCandidate,
        confidence: BtxCandidateConfidence::High,
        candidate_reason: "합성 검토 후보",
        decompressed_sha1: digest_byte.to_string().repeat(40),
        decompressed_sha256: digest_byte.to_string().repeat(64),
        format: PixelFormat::Indexed4,
        decompressed_bytes: 8,
        pixel_count: 16,
        dimension_candidates: 1,
        review_width: 8,
        review_height: 2,
        palette_histogram: vec![8, 8],
        candidate_background_index: 0,
        occupied_box: None,
        horizontal_occupied_bands: 1,
        vertical_occupied_bands: 1,
        complete_16x16_cells: 0,
        unique_16x16_cells: 0,
        repeated_16x16_cells: 0,
        background_16x16_cells: 0,
        partial_16x16_edges: true,
        physical_copies: vec![BtxPhysicalCandidateCopy {
            archive: "/TEST/TEST_GRP.VDD;1".to_owned(),
            name: format!("{name}.BTX"),
            resource_index: 1,
            static_evidence: "synthetic".to_owned(),
            static_reference_sites: 1,
            descriptor_runtime: 0x8001_1000,
        }],
    }
}

fn family(id: &str, candidate_id: &str, name: &str) -> BtxCandidateFamily {
    BtxCandidateFamily {
        family_id: id.to_owned(),
        family_key: format!("scope:{name}:indexed4"),
        review_lane: BtxReviewLane::UiFontTextCandidate,
        confidence: BtxCandidateConfidence::High,
        members: 1,
        physical_copies: 1,
        representative_candidate_id: candidate_id.to_owned(),
        representative_archive: "/TEST/TEST_GRP.VDD;1".to_owned(),
        representative_name: format!("{name}.BTX"),
        representative_preview: format!("previews/{name}.png"),
    }
}

fn analysis() -> BtxFamilyCandidateAnalysis {
    BtxFamilyCandidateAnalysis {
        physical_btx: 2,
        unique_contents: 2,
        known_text_contents: 0,
        known_pending_contents: 0,
        known_visual_positive_contents: 0,
        known_nontext_contents: 0,
        declared_known_physical_btx: 0,
        known_content_physical_btx: 0,
        unclassified_known_content_aliases: Vec::new(),
        candidates: vec![
            candidate("BTXU-0001", "BTXFAM-0001", "MENU", 'a'),
            candidate("BTXU-0002", "BTXFAM-0002", "FONT", 'b'),
        ],
        families: vec![
            family("BTXFAM-0001", "BTXU-0001", "MENU"),
            family("BTXFAM-0002", "BTXU-0002", "FONT"),
        ],
        automatic_approval: false,
    }
}

#[test]
fn 현재_family에서_만든_미결정_원장은_동적_분모와_baseline을_검증한다() {
    let candidates = analysis();
    let manifest = render_pending_btx_family_review_decisions_tsv(&candidates).unwrap();
    let reviewed = audit_btx_family_review_decisions(&candidates, &manifest).unwrap();

    assert_eq!(reviewed.families, 2);
    assert_eq!(reviewed.pending_human_review, 2);
    assert_eq!(reviewed.japanese_pixels_observed, 0);
    assert_eq!(reviewed.decisions[0].family_key, "scope:FONT:indexed4");
    assert!(manifest.lines().skip(1).all(|row| row.ends_with("\t\\e")));
}

#[test]
fn 순차_id만_바뀌어도_stable_key_baseline은_유지된다() {
    let original = analysis();
    let original_manifest = render_pending_btx_family_review_decisions_tsv(&original).unwrap();
    let mut renumbered = original.clone();
    for (index, candidate) in renumbered.candidates.iter_mut().enumerate() {
        candidate.candidate_id = format!("BTXU-9{index:03}");
        candidate.family_id = format!("BTXFAM-9{index:03}");
    }
    for (index, family) in renumbered.families.iter_mut().enumerate() {
        family.family_id = format!("BTXFAM-9{index:03}");
        family.representative_candidate_id = format!("BTXU-9{index:03}");
    }

    let renumbered_manifest = render_pending_btx_family_review_decisions_tsv(&renumbered).unwrap();

    assert_eq!(renumbered_manifest, original_manifest);
}

#[test]
fn 사람_관측은_선택만으로_저장되고_대표_비관측을_family제외로_부르지_않는다() {
    let candidates = analysis();
    let manifest = render_pending_btx_family_review_decisions_tsv(&candidates).unwrap();
    let mut decisions = audit_btx_family_review_decisions(&candidates, &manifest)
        .unwrap()
        .decisions;
    decisions[0].decision = BtxFamilyReviewDecisionKind::NoJapaneseObservedInRepresentative;
    let updated = render_btx_family_review_decisions_tsv(&decisions);
    let reviewed = audit_btx_family_review_decisions(&candidates, &updated).unwrap();

    assert_eq!(reviewed.no_japanese_observed_in_representative, 1);
    assert_eq!(reviewed.pending_human_review, 1);

    decisions[0].notes = " 선택 메모 앞 공백".to_owned();
    assert!(audit_btx_family_review_decisions(
        &candidates,
        &render_btx_family_review_decisions_tsv(&decisions)
    )
    .is_err());
}

#[test]
fn stale_baseline과_누락행과_비canonical순서는_거부한다() {
    let candidates = analysis();
    let manifest = render_pending_btx_family_review_decisions_tsv(&candidates).unwrap();
    let mut decisions = audit_btx_family_review_decisions(&candidates, &manifest)
        .unwrap()
        .decisions;

    decisions[0].baseline_sha256 = "0".repeat(64);
    assert!(audit_btx_family_review_decisions(
        &candidates,
        &render_btx_family_review_decisions_tsv(&decisions)
    )
    .is_err());

    let mut decisions = audit_btx_family_review_decisions(&candidates, &manifest)
        .unwrap()
        .decisions;
    decisions.pop();
    assert!(audit_btx_family_review_decisions(
        &candidates,
        &render_btx_family_review_decisions_tsv(&decisions)
    )
    .is_err());

    let mut rows = manifest.lines().collect::<Vec<_>>();
    rows.swap(1, 2);
    let noncanonical = format!("{}\n", rows.join("\n"));
    assert!(audit_btx_family_review_decisions(&candidates, &noncanonical).is_err());
}
