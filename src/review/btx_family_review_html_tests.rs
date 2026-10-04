use crate::btx::PixelFormat;
use crate::graphics::btx_family_candidates::{
    BtxCandidateConfidence, BtxCandidateFamily, BtxFamilyCandidate, BtxFamilyCandidateAnalysis,
    BtxPhysicalCandidateCopy, BtxReviewLane,
};
use crate::review::{
    audit_btx_family_review_decisions, render_btx_family_review_decisions_tsv,
    render_pending_btx_family_review_decisions_tsv, BtxFamilyReviewDecisionKind,
};

use super::*;

fn candidates() -> BtxFamilyCandidateAnalysis {
    let candidate = BtxFamilyCandidate {
        candidate_id: "BTXU-0001".to_owned(),
        family_id: "BTXFAM-0001".to_owned(),
        family_key: "scope:<MENU>:indexed4".to_owned(),
        review_lane: BtxReviewLane::UiFontTextCandidate,
        confidence: BtxCandidateConfidence::High,
        candidate_reason: "합성 검토 후보",
        decompressed_sha1: "a".repeat(40),
        decompressed_sha256: "b".repeat(64),
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
            archive: "/TEST/&.VDD;1".to_owned(),
            name: "MENU.BTX".to_owned(),
            resource_index: 1,
            static_evidence: "synthetic".to_owned(),
            static_reference_sites: 1,
            descriptor_runtime: 0x8001_1000,
        }],
    };
    BtxFamilyCandidateAnalysis {
        physical_btx: 1,
        unique_contents: 1,
        known_text_contents: 0,
        known_pending_contents: 0,
        known_visual_positive_contents: 0,
        known_nontext_contents: 0,
        declared_known_physical_btx: 0,
        known_content_physical_btx: 0,
        unclassified_known_content_aliases: Vec::new(),
        candidates: vec![candidate],
        families: vec![BtxCandidateFamily {
            family_id: "BTXFAM-0001".to_owned(),
            family_key: "scope:<MENU>:indexed4".to_owned(),
            review_lane: BtxReviewLane::UiFontTextCandidate,
            confidence: BtxCandidateConfidence::High,
            members: 1,
            physical_copies: 1,
            representative_candidate_id: "BTXU-0001".to_owned(),
            representative_archive: "/TEST/&.VDD;1".to_owned(),
            representative_name: "MENU.BTX".to_owned(),
            representative_preview: "previews/MENU.png".to_owned(),
        }],
        automatic_approval: false,
    }
}

#[test]
fn 화면은_원본_preview와_세_관측안과_미확정_경계를_함께_보여준다() {
    let candidates = candidates();
    let manifest = render_pending_btx_family_review_decisions_tsv(&candidates).unwrap();
    let review = audit_btx_family_review_decisions(&candidates, &manifest).unwrap();
    let html = render_btx_family_review_page(&candidates, &review, "token<&", None).unwrap();

    assert!(html.contains("/preview/BTXU-0001.bmp"));
    assert!(html.contains("japanese-pixels-observed"));
    assert!(html.contains("no-japanese-observed-in-representative"));
    assert!(html.contains("review-insufficient"));
    assert!(!html.contains("name=reviewer"));
    assert!(html.contains("<textarea name=notes>"));
    assert!(!html.contains("<textarea name=notes required>"));
    assert!(html.contains("실제 CLUT·consumer layout·인게임 표시를 재현하지 않으며"));
    assert!(html.contains(&review.manifest_sha256));
    assert!(html.contains("scope:&lt;MENU&gt;:indexed4"));
    assert!(html.contains("name=family_key"));
    assert!(!html.contains("name=family_id"));
    assert!(html.contains("/TEST/&amp;.VDD;1"));
    assert!(html.contains("token&lt;&amp;"));
    assert!(!html.contains("value=\"token<&\""));
}

#[test]
fn 원장과_family_분모가_다르면_부분_화면을_만들지_않는다() {
    let candidates = candidates();
    let manifest = render_pending_btx_family_review_decisions_tsv(&candidates).unwrap();
    let mut review = audit_btx_family_review_decisions(&candidates, &manifest).unwrap();
    review.decisions.clear();
    assert!(render_btx_family_review_page(&candidates, &review, "token", None).is_err());
}

fn insufficient_review(candidates: &BtxFamilyCandidateAnalysis) -> BtxFamilyReviewAnalysis {
    let manifest = render_pending_btx_family_review_decisions_tsv(candidates).unwrap();
    let mut decisions = audit_btx_family_review_decisions(candidates, &manifest)
        .unwrap()
        .decisions;
    decisions[0].decision = BtxFamilyReviewDecisionKind::ReviewInsufficient;
    let manifest = render_btx_family_review_decisions_tsv(&decisions);
    audit_btx_family_review_decisions(candidates, &manifest).unwrap()
}

#[test]
fn 재판독_화면은_source_bound_저장행과_atlas경계를_함께_보여준다() {
    let candidates = candidates();
    let review = insufficient_review(&candidates);
    let layouts = [BtxFamilyReviewLayout {
        family_key: "scope:<MENU>:indexed4".to_owned(),
        candidate_id: "BTXU-0001".to_owned(),
        width: 8,
        stored_height: 2,
        atlas_height: 4,
        basis: BtxFamilyReviewLayoutBasis::RowPaddedAtlas,
    }];

    let html = render_btx_family_layout_review_page(&candidates, &review, &layouts, "token", None)
        .unwrap();

    assert!(html.contains("BTX source-bound layout 재판독"));
    assert!(html.contains("재판독 family 1"));
    assert!(html.contains("재판독 candidate 1"));
    assert!(html.contains("8×2 / atlas 4행"));
    assert!(html.contains("descriptor 행 + atlas padding"));
    assert!(html.contains("setup 실행·VRAM upload·primitive draw·인게임 표시"));
}

#[test]
fn 재판독_화면은_원본크기와_맞지_않는_layout을_거부한다() {
    let candidates = candidates();
    let review = insufficient_review(&candidates);
    let wrong_size = [BtxFamilyReviewLayout {
        family_key: "scope:<MENU>:indexed4".to_owned(),
        candidate_id: "BTXU-0001".to_owned(),
        width: 4,
        stored_height: 2,
        atlas_height: 2,
        basis: BtxFamilyReviewLayoutBasis::ExactDescriptorSize,
    }];

    assert!(
        render_btx_family_layout_review_page(&candidates, &review, &wrong_size, "token", None)
            .is_err()
    );
}

#[test]
fn 재판독_화면은_한결정저장뒤에도_같은_source_layout을_유지한다() {
    let candidates = candidates();
    let manifest = render_pending_btx_family_review_decisions_tsv(&candidates).unwrap();
    let mut decisions = audit_btx_family_review_decisions(&candidates, &manifest)
        .unwrap()
        .decisions;
    decisions[0].decision = BtxFamilyReviewDecisionKind::JapanesePixelsObserved;
    let manifest = render_btx_family_review_decisions_tsv(&decisions);
    let review = audit_btx_family_review_decisions(&candidates, &manifest).unwrap();
    let layouts = [BtxFamilyReviewLayout {
        family_key: "scope:<MENU>:indexed4".to_owned(),
        candidate_id: "BTXU-0001".to_owned(),
        width: 8,
        stored_height: 2,
        atlas_height: 2,
        basis: BtxFamilyReviewLayoutBasis::ExactDescriptorSize,
    }];

    let html = render_btx_family_layout_review_page(&candidates, &review, &layouts, "token", None)
        .unwrap();

    assert!(html.contains("japanese-pixels-observed\" checked"));
    assert!(html.contains("8×2 / atlas 2행"));
}
