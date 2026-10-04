use super::*;
use crate::graphics::text_catalog::P2_END16_TEXT_ASSET;

#[test]
fn p2_end16은_source_mask정책과_수동3행조판에결속된다() {
    assert_eq!(
        verify_source_text_mask_policy_for_asset(P2_END16_TEXT_ASSET).unwrap(),
        1
    );
    let translation = general_btx_raster_translation_entries()
        .unwrap()
        .into_iter()
        .find(|entry| entry.block_id == "BTXB-0253")
        .unwrap();
    let assignment = general_btx_raster_profile_assignments()
        .unwrap()
        .into_iter()
        .find(|profile| {
            profile.semantic_family == translation.semantic_family
                && profile.profile_candidate_index == translation.profile_candidate_index
        })
        .unwrap();
    let profile = assignment.configuration.unwrap();

    assert_eq!(translation.ko, "정상을\n노려라\n사탄 님");
    assert_eq!(profile.line_count, 3);
    assert_eq!(profile.background, P2_END16_BACKGROUND_INDEX);
}

#[test]
fn exact_p2_end16이아닌입력은_mask합성전에실패한다() {
    let invalid = vec![0; P2_END16_TEXT_ASSET.layout.byte_len().unwrap()];

    assert!(patch_p2_end16_development_surface(&invalid).is_err());
}
