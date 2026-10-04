use super::*;
use ps1_puyopuyo_box::graphics::btx_family_candidates::BtxCandidateConfidence;

fn family(id: &str, lane: BtxReviewLane) -> BtxCandidateFamily {
    BtxCandidateFamily {
        family_id: id.to_owned(),
        family_key: format!("scope:{id}:indexed4"),
        review_lane: lane,
        confidence: BtxCandidateConfidence::Low,
        members: 1,
        physical_copies: 1,
        representative_candidate_id: "BTXU-0001".to_owned(),
        representative_archive: "/TEST/TEST_GRP.VDD;1".to_owned(),
        representative_name: "TEST.BTX".to_owned(),
        representative_preview: "preview.png".to_owned(),
    }
}

#[test]
fn preview_route는_정확한_candidate_id만_받는다() {
    assert_eq!(
        preview_candidate_id("/preview/BTXU-0001.bmp"),
        Some("BTXU-0001")
    );
    assert_eq!(preview_candidate_id("/preview/.bmp"), None);
    assert_eq!(preview_candidate_id("/preview/../secret.bmp"), None);
    assert_eq!(preview_candidate_id("/preview/BTXU_0001.bmp"), None);
    assert_eq!(preview_candidate_id("/preview/BTXU-0001.png"), None);
}

#[test]
fn 사람_관측은_세_명시적_선택만_받는다() {
    assert_eq!(
        parse_decision("japanese-pixels-observed").unwrap(),
        BtxFamilyReviewDecisionKind::JapanesePixelsObserved
    );
    assert_eq!(
        parse_decision("no-japanese-observed-in-representative").unwrap(),
        BtxFamilyReviewDecisionKind::NoJapaneseObservedInRepresentative
    );
    assert_eq!(
        parse_decision("review-insufficient").unwrap(),
        BtxFamilyReviewDecisionKind::ReviewInsufficient
    );
    assert!(parse_decision("pending-human-review").is_err());
    assert!(parse_decision("non-text-family").is_err());
}

#[test]
fn 검토_모집단은_개별후보가_아니라_family_lane이_소유한다() {
    let families = vec![
        family("BTXFAM-UI", BtxReviewLane::UiFontTextCandidate),
        family("BTXFAM-AMB", BtxReviewLane::Ambiguous),
    ];

    assert!(is_review_family(&families, "BTXFAM-UI"));
    assert!(!is_review_family(&families, "BTXFAM-AMB"));
    assert!(!is_review_family(&families, "BTXFAM-MISSING"));
}

#[test]
fn 선택_제출은_검토자와_메모를_요구하지_않는다() {
    let fields = parse_form(
        b"token=expected&family_key=subgame%3ASP_FONT%3Aindexed4&baseline_sha256=abc&decision=japanese-pixels-observed",
    )
    .unwrap();

    let selection = decision_selection(&fields, "expected").unwrap();

    assert_eq!(selection.family_key, "subgame:SP_FONT:indexed4");
    assert_eq!(
        selection.decision,
        BtxFamilyReviewDecisionKind::JapanesePixelsObserved
    );
    assert!(selection.notes.is_empty());
}
