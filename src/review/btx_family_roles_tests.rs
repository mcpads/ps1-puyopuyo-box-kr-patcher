use crate::btx::PixelFormat;
use crate::graphics::btx_family_candidates::{
    BtxCandidateConfidence, BtxCandidateFamily, BtxFamilyCandidate, BtxFamilyCandidateAnalysis,
    BtxPhysicalCandidateCopy, BtxReviewLane,
};

use super::*;

fn candidate(family_id: &str, family_key: &str, pixels: usize) -> BtxFamilyCandidate {
    BtxFamilyCandidate {
        candidate_id: "BTXU-9001".to_owned(),
        family_id: family_id.to_owned(),
        family_key: family_key.to_owned(),
        review_lane: BtxReviewLane::UiFontTextCandidate,
        confidence: BtxCandidateConfidence::High,
        candidate_reason: "합성 역할 후보",
        decompressed_sha1: "a".repeat(40),
        decompressed_sha256: "b".repeat(64),
        format: PixelFormat::Indexed4,
        decompressed_bytes: pixels / 2,
        pixel_count: pixels,
        dimension_candidates: 1,
        review_width: 8,
        review_height: pixels / 8,
        palette_histogram: vec![pixels],
        candidate_background_index: 0,
        occupied_box: None,
        horizontal_occupied_bands: 0,
        vertical_occupied_bands: 0,
        complete_16x16_cells: 0,
        unique_16x16_cells: 0,
        repeated_16x16_cells: 0,
        background_16x16_cells: 0,
        partial_16x16_edges: true,
        physical_copies: vec![BtxPhysicalCandidateCopy {
            archive: "/TEST/TEST_GRP.VDD;1".to_owned(),
            name: "TEST.BTX".to_owned(),
            resource_index: 1,
            static_evidence: "synthetic".to_owned(),
            static_reference_sites: 1,
            descriptor_runtime: 0x8001_1000,
        }],
    }
}

fn family(family_id: &str, family_key: &str) -> BtxCandidateFamily {
    BtxCandidateFamily {
        family_id: family_id.to_owned(),
        family_key: family_key.to_owned(),
        review_lane: BtxReviewLane::UiFontTextCandidate,
        confidence: BtxCandidateConfidence::High,
        members: 1,
        physical_copies: 1,
        representative_candidate_id: "BTXU-9001".to_owned(),
        representative_archive: "/TEST/TEST_GRP.VDD;1".to_owned(),
        representative_name: "TEST.BTX".to_owned(),
        representative_preview: "previews/test.png".to_owned(),
    }
}

fn analysis(family_id: &str, family_key: &str, pixels: usize) -> BtxFamilyCandidateAnalysis {
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
        candidates: vec![candidate(family_id, family_key, pixels)],
        families: vec![family(family_id, family_key)],
        automatic_approval: false,
    }
}

fn decision(family_key: &str, kind: BtxFamilyReviewDecisionKind) -> BtxFamilyReviewDecision {
    BtxFamilyReviewDecision {
        family_key: family_key.to_owned(),
        baseline_sha256: "c".repeat(64),
        decision: kind,
        notes: String::new(),
    }
}

fn spec(family_key: &'static str, width: usize, height: usize) -> BtxFamilyRoleSpec {
    BtxFamilyRoleSpec {
        family_key,
        role: BtxFamilyAssetRole::UiSurface,
        basis: BtxFamilyRoleBasis::HumanVisualCandidate,
        common_layout: Some(Layout {
            width,
            height,
            format: PixelFormat::Indexed4,
        }),
    }
}

#[test]
fn 역할은_유동적인_family_id가_아니라_안정적인_key로_결속한다() {
    let family_key = "scope:MENU:indexed4";
    let candidates = analysis("BTXFAM-9001", family_key, 128);
    let reviewed = [decision(
        family_key,
        BtxFamilyReviewDecisionKind::JapanesePixelsObserved,
    )];

    let roles = audit_btx_family_roles(&candidates, &reviewed, &[spec(family_key, 16, 8)])
        .expect("key 기반 역할 결속");

    assert_eq!(roles.families, 1);
    assert_eq!(roles.unique_contents, 1);
    assert_eq!(roles.physical_copies, 1);
    assert_eq!(roles.human_visual_candidates, 1);
    assert_eq!(roles.rows[0].family_id, "BTXFAM-9001");
}

#[test]
fn 사람_양성_family에_역할이_없으면_실패한다() {
    let first_key = "scope:MENU:indexed4";
    let second_key = "scope:FONT:indexed4";
    let mut candidates = analysis("BTXFAM-0001", first_key, 128);
    let mut second_candidate = candidate("BTXFAM-0002", second_key, 128);
    second_candidate.candidate_id = "BTXU-9002".to_owned();
    let mut second_family = family("BTXFAM-0002", second_key);
    second_family.representative_candidate_id = "BTXU-9002".to_owned();
    candidates.candidates.push(second_candidate);
    candidates.families.push(second_family);
    let reviewed = [
        decision(
            first_key,
            BtxFamilyReviewDecisionKind::JapanesePixelsObserved,
        ),
        decision(
            second_key,
            BtxFamilyReviewDecisionKind::JapanesePixelsObserved,
        ),
    ];

    assert!(audit_btx_family_roles(&candidates, &reviewed, &[spec(first_key, 16, 8)]).is_err());
}

#[test]
fn 비양성_family에_역할을_붙이면_실패한다() {
    let family_key = "scope:MENU:indexed4";
    let candidates = analysis("BTXFAM-0001", family_key, 128);
    let reviewed = [decision(
        family_key,
        BtxFamilyReviewDecisionKind::ReviewInsufficient,
    )];

    assert!(audit_btx_family_roles(&candidates, &reviewed, &[spec(family_key, 16, 8)]).is_err());
}

#[test]
fn 판독용_layout도_원문_전체_픽셀과_맞아야_한다() {
    let family_key = "scope:MENU:indexed4";
    let candidates = analysis("BTXFAM-0001", family_key, 128);
    let reviewed = [decision(
        family_key,
        BtxFamilyReviewDecisionKind::JapanesePixelsObserved,
    )];

    assert!(audit_btx_family_roles(&candidates, &reviewed, &[spec(family_key, 8, 8)]).is_err());
}
