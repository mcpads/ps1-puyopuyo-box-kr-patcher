use super::*;
use crate::graphics::text_catalog::{
    is_japanese_translation_target, BOXCUBE_TEXT_ASSET, MMENU_TEXT_ASSET, MOPT_TEXT_ASSET,
    P2RULED1_TEXT_ASSET, P2_END16_TEXT_ASSET,
};

#[test]
fn surface정책은_일반raster분모를_정확히한번분류한다() {
    let strategy = verify_box_btx_patch_strategy().unwrap();
    let report = verify_general_btx_raster_surface_policies().unwrap();

    assert_eq!(report.assets, strategy.remaining_raster_assets);
    assert_eq!(report.blocks, strategy.remaining_raster_blocks);
    assert_eq!(
        report.flat_background_blocks
            + report.source_background_preserved_blocks
            + report.generated_artwork_blocks
            + report.scope_excluded_unassessed_blocks,
        report.blocks
    );
    assert_eq!(
        report.whole_bbox_blocks
            + report.source_text_mask_blocks
            + report.composer_required_blocks
            + report.protected_no_write_blocks,
        report.blocks
    );
}

#[test]
fn source_text_mask_writer는_구현된정책자산만받는다() {
    assert_eq!(
        verify_generated_artwork_policy_for_asset(MMENU_TEXT_ASSET).unwrap(),
        MMENU_TEXT_ASSET
            .blocks
            .iter()
            .filter(|block| is_japanese_translation_target(block.source))
            .count()
    );
    assert_eq!(
        verify_source_text_mask_policy_for_asset(P2_END16_TEXT_ASSET).unwrap(),
        1
    );
    assert!(verify_source_text_mask_policy_for_asset(BOXCUBE_TEXT_ASSET).is_err());
}

#[test]
fn flat배경_writer는_원장에_등록된block별배경만받는다() {
    let p2_ruled1 = flat_background_indices_for_asset(P2RULED1_TEXT_ASSET).unwrap();
    let mopt = flat_background_indices_for_asset(MOPT_TEXT_ASSET).unwrap();

    let japanese_blocks = |asset: TextAssetSpec<'_>| {
        asset
            .blocks
            .iter()
            .filter(|block| is_japanese_translation_target(block.source))
            .count()
    };
    assert_eq!(p2_ruled1.len(), japanese_blocks(P2RULED1_TEXT_ASSET));
    assert!(p2_ruled1.values().all(|background| *background == 0));
    assert_eq!(mopt.len(), japanese_blocks(MOPT_TEXT_ASSET));
    assert!(mopt.values().all(|background| *background == 0));
    assert!(flat_background_indices_for_asset(BOXCUBE_TEXT_ASSET).is_err());
}

#[test]
fn catalog_identity가_바뀐surface정책을_거부한다() {
    let invalid = SURFACE_POLICY_MANIFEST.replacen(
        "14caca5635bff3fb2b555369156f88e5802bc7b66c65a9b358d8b3271945f895",
        "04caca5635bff3fb2b555369156f88e5802bc7b66c65a9b358d8b3271945f895",
        1,
    );

    assert!(verify_manifest(&invalid)
        .unwrap_err()
        .to_string()
        .contains("catalog와 다름"));
}

#[test]
fn 중복되거나누락된block분류를_거부한다() {
    let invalid = SURFACE_POLICY_MANIFEST.replacen("BTXB-0037,BTXB-0038", "BTXB-0037,BTXB-0037", 1);

    assert!(verify_manifest(&invalid)
        .unwrap_err()
        .to_string()
        .contains("중복"));
}

#[test]
fn 배경보존block을_whole_bbox로_선언하면_거부한다() {
    let invalid = SURFACE_POLICY_MANIFEST.replacen(
        "source-background-preserved\t\tsource-text-mask\n",
        "source-background-preserved\t\twhole-bbox\n",
        1,
    );

    assert!(verify_manifest(&invalid)
        .unwrap_err()
        .to_string()
        .contains("조합 불일치"));
}

#[test]
fn flat배경의_배경index가_없으면_거부한다() {
    let invalid = SURFACE_POLICY_MANIFEST.replacen(
        "P2_OP1.BTX\t1dc8dfe5bc9fc309bf9cde4a3be1a1e80681aa458f96bf24497dfe67554d2921\tBTXB-0106\tflat-background\t15\twhole-bbox",
        "P2_OP1.BTX\t1dc8dfe5bc9fc309bf9cde4a3be1a1e80681aa458f96bf24497dfe67554d2921\tBTXB-0106\tflat-background\t\twhole-bbox",
        1,
    );

    assert!(verify_manifest(&invalid)
        .unwrap_err()
        .to_string()
        .contains("조합 불일치"));
}
