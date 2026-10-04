use super::*;

#[test]
fn 이름_family는_숫자_frame을_합치되_의미있는_접두사는_보존한다() {
    assert_eq!(normalized_name_stem("GT69.BTX"), "GT");
    assert_eq!(normalized_name_stem("FLAME3A.BTX"), "FLAME");
    assert_eq!(normalized_name_stem("P2_INFO5.BTX"), "P2_INFO");
    assert_eq!(normalized_name_stem("ED_AMA.BTX"), "ED_AMA");
}

#[test]
fn review_layout은_원본_픽셀을_보존하며_256px에_가장_가깝다() {
    let (width, height, candidates) = review_layout(32_768).unwrap();
    assert_eq!((width, height), (256, 128));
    assert!(candidates > 1);

    let (width, height, candidates) = review_layout(4_480).unwrap();
    assert_eq!((width, height), (280, 16));
    assert!(candidates > 1);
}

#[test]
fn 점유_band와_반복_cell을_계산한다() {
    let mut pixels = vec![0u8; 32 * 16];
    for y in 0..16 {
        for x in 0..16 {
            pixels[y * 32 + x] = 1;
            pixels[y * 32 + x + 16] = 1;
        }
    }
    assert_eq!(
        occupied_features(&pixels, 32, 16, 0),
        (
            Some(BtxOccupiedBox {
                x: 0,
                y: 0,
                width: 32,
                height: 16,
            }),
            1,
            1,
        )
    );
    assert_eq!(cell_features(&pixels, 32, 16, 0), (2, 1, 1, 0, false));
}

#[test]
fn 자동_lane은_승인이_아닌_검토_후보다() {
    assert_eq!(
        candidate_lane("MENU", PixelFormat::Indexed4, 512, 128, 8),
        (
            BtxReviewLane::UiFontTextCandidate,
            BtxCandidateConfidence::High,
            "ui-font-text name token",
        )
    );
    assert_eq!(
        candidate_lane("ARLE", PixelFormat::Indexed4, 16_384, 256, 128),
        (
            BtxReviewLane::CharacterBackgroundEffectCandidate,
            BtxCandidateConfidence::Medium,
            "common 256x128 4bpp sprite-atlas review geometry",
        )
    );
}

#[test]
fn family_report는_inventory_preview와_승인경계를_함께_내보낸다() {
    let family = BtxCandidateFamily {
        family_id: "BTXFAM-0001".to_string(),
        family_key: "quest:RPG_FNT:indexed4".to_string(),
        review_lane: BtxReviewLane::UiFontTextCandidate,
        confidence: BtxCandidateConfidence::High,
        members: 1,
        physical_copies: 1,
        representative_candidate_id: "BTXU-0001".to_string(),
        representative_archive: "/QUEST/RPG_GRP.VDD;1".to_string(),
        representative_name: "RPG_FNT1.BTX".to_string(),
        representative_preview: "previews/indexed4-0123456789abcdef.png".to_string(),
    };
    let analysis = BtxFamilyCandidateAnalysis {
        physical_btx: 1,
        unique_contents: 1,
        known_text_contents: 0,
        known_pending_contents: 0,
        known_visual_positive_contents: 0,
        known_nontext_contents: 0,
        declared_known_physical_btx: 0,
        known_content_physical_btx: 0,
        unclassified_known_content_aliases: Vec::new(),
        candidates: Vec::new(),
        families: vec![family],
        automatic_approval: false,
    };

    let report = render_btx_candidate_families_tsv(&analysis);
    assert!(report.contains("representative_preview\tautomatic_approval"));
    assert!(report.contains("previews/indexed4-0123456789abcdef.png\tfalse"));

    let feature_report = render_btx_family_candidate_features_tsv(&analysis);
    assert!(feature_report
        .contains("seq_evidence\tdecompressed_sha1\tdecompressed_sha256\tpreview\tstatus"));
}
