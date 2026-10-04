use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::btx::PixelFormat;
use crate::graphics::btx_family_candidates::{
    BtxCandidateConfidence, BtxCandidateFamily, BtxFamilyCandidate, BtxFamilyCandidateAnalysis,
    BtxPhysicalCandidateCopy, BtxReviewLane,
};
use crate::review::render_pending_btx_family_review_decisions_tsv;

use super::*;

static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn create() -> Self {
        let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ps1-puyopuyo-box-btx-family-review-store-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn manifest_path(&self) -> PathBuf {
        self.0.join("btx_family_decisions.tsv")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn candidates() -> BtxFamilyCandidateAnalysis {
    let candidate = BtxFamilyCandidate {
        candidate_id: "BTXU-0001".to_owned(),
        family_id: "BTXFAM-0001".to_owned(),
        family_key: "scope:MENU:indexed4".to_owned(),
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
            archive: "/TEST/TEST_GRP.VDD;1".to_owned(),
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
            family_key: "scope:MENU:indexed4".to_owned(),
            review_lane: BtxReviewLane::UiFontTextCandidate,
            confidence: BtxCandidateConfidence::High,
            members: 1,
            physical_copies: 1,
            representative_candidate_id: "BTXU-0001".to_owned(),
            representative_archive: "/TEST/TEST_GRP.VDD;1".to_owned(),
            representative_name: "MENU.BTX".to_owned(),
            representative_preview: "previews/MENU.png".to_owned(),
        }],
        automatic_approval: false,
    }
}

fn selection(
    candidates: &BtxFamilyCandidateAnalysis,
    decision: BtxFamilyReviewDecisionKind,
) -> BtxFamilyReviewSelection {
    let manifest = render_pending_btx_family_review_decisions_tsv(candidates).unwrap();
    let analysis = audit_btx_family_review_decisions(candidates, &manifest).unwrap();
    BtxFamilyReviewSelection {
        family_key: "scope:MENU:indexed4".to_owned(),
        baseline_sha256: analysis.decisions[0].baseline_sha256.clone(),
        decision,
        notes: String::new(),
    }
}

#[test]
fn 사람_관측은_전체_원장을_재검증한_뒤_저장된다() {
    let directory = TestDirectory::create();
    let path = directory.manifest_path();
    let candidates = candidates();
    let manifest = render_pending_btx_family_review_decisions_tsv(&candidates).unwrap();
    fs::write(&path, &manifest).unwrap();
    let digest = audit_btx_family_review_decisions(&candidates, &manifest)
        .unwrap()
        .manifest_sha256;

    let saved = save_btx_family_review_decision(
        &path,
        &candidates,
        &digest,
        selection(
            &candidates,
            BtxFamilyReviewDecisionKind::JapanesePixelsObserved,
        ),
    )
    .unwrap();

    assert_eq!(saved.japanese_pixels_observed, 1);
    assert_eq!(saved.pending_human_review, 0);
    let reloaded =
        audit_btx_family_review_decisions(&candidates, &fs::read_to_string(&path).unwrap())
            .unwrap();
    assert_eq!(reloaded.decisions, saved.decisions);
}

#[test]
fn stale_manifest와_stale_baseline은_기존_byte를_보존한다() {
    let directory = TestDirectory::create();
    let path = directory.manifest_path();
    let candidates = candidates();
    let manifest = render_pending_btx_family_review_decisions_tsv(&candidates).unwrap();
    fs::write(&path, &manifest).unwrap();
    let original = fs::read(&path).unwrap();

    let mut stale = selection(&candidates, BtxFamilyReviewDecisionKind::ReviewInsufficient);
    stale.baseline_sha256 = "0".repeat(64);
    let error =
        save_btx_family_review_decision(&path, &candidates, &"f".repeat(64), stale).unwrap_err();
    assert!(matches!(
        error,
        BtxFamilyReviewStoreError::ConcurrentChange { .. }
    ));
    assert_eq!(fs::read(&path).unwrap(), original);

    let digest = audit_btx_family_review_decisions(&candidates, &manifest)
        .unwrap()
        .manifest_sha256;
    let mut stale = selection(&candidates, BtxFamilyReviewDecisionKind::ReviewInsufficient);
    stale.baseline_sha256 = "0".repeat(64);
    let error = save_btx_family_review_decision(&path, &candidates, &digest, stale).unwrap_err();
    assert!(matches!(
        error,
        BtxFamilyReviewStoreError::BaselineDrift { .. }
    ));
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn lock과_pending제출은_쓰기를_시작하지_않는다() {
    let directory = TestDirectory::create();
    let path = directory.manifest_path();
    let candidates = candidates();
    let manifest = render_pending_btx_family_review_decisions_tsv(&candidates).unwrap();
    fs::write(&path, &manifest).unwrap();
    let original = fs::read(&path).unwrap();
    let digest = audit_btx_family_review_decisions(&candidates, &manifest)
        .unwrap()
        .manifest_sha256;

    let error = apply_btx_family_review_decision(
        &candidates,
        &manifest,
        &digest,
        BtxFamilyReviewSelection {
            family_key: "scope:MENU:indexed4".to_owned(),
            baseline_sha256: "0".repeat(64),
            decision: BtxFamilyReviewDecisionKind::PendingHumanReview,
            notes: String::new(),
        },
    )
    .unwrap_err();
    assert!(matches!(error, BtxFamilyReviewStoreError::PendingSelection));

    fs::write(path.with_extension("tsv.lock"), "writer\n").unwrap();
    let error = save_btx_family_review_decision(
        &path,
        &candidates,
        &digest,
        selection(
            &candidates,
            BtxFamilyReviewDecisionKind::JapanesePixelsObserved,
        ),
    )
    .unwrap_err();
    assert!(matches!(error, BtxFamilyReviewStoreError::Locked(_)));
    assert_eq!(fs::read(&path).unwrap(), original);
}
