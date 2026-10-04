use super::*;
use crate::btx::encode;

fn entry() -> GeneralRasterTranslationEntry {
    GeneralRasterTranslationEntry {
        semantic_family: LEVEL_UP_FAMILY.to_string(),
        profile_candidate_index: level_up_profile_candidate_index().unwrap(),
        category_id: None,
        profile_id: None,
        archive_paths: RPG_SYS1_TEXT_ASSET.archive_paths.join("\n"),
        asset_name: RPG_SYS1_TEXT_ASSET.name.to_string(),
        asset_sha256: RPG_SYS1_TEXT_ASSET.expected_decompressed_sha256.to_string(),
        block_id: LEVEL_UP_BLOCK_ID.to_string(),
        block_pixels_sha256: RPG_SYS1_TEXT_ASSET.blocks[0]
            .expected_pixels_sha256
            .to_string(),
        bbox_x: RPG_SYS1_TEXT_ASSET.blocks[0].bbox.x,
        bbox_y: RPG_SYS1_TEXT_ASSET.blocks[0].bbox.y,
        bbox_width: RPG_SYS1_TEXT_ASSET.blocks[0].bbox.width,
        bbox_height: RPG_SYS1_TEXT_ASSET.blocks[0].bbox.height,
        protected_source: LEVEL_UP_SOURCE.to_string(),
        ko: LEVEL_UP_DRAFT.to_string(),
        status: GeneralRasterTranslationStatus::DraftHumanReviewRequired,
    }
}

#[test]
fn 개발후보는_초안상태를_승인으로_바꾸지않는다() {
    let entry = entry();

    verify_translation(&entry).unwrap();

    let approved = GeneralRasterTranslationEntry {
        status: GeneralRasterTranslationStatus::Approved,
        ..entry
    };
    assert!(verify_translation(&approved).is_err());
}

#[test]
fn 번역문이나_profile결속이_달라지면_실패한다() {
    let wrong_text = GeneralRasterTranslationEntry {
        ko: "레벨업!".to_string(),
        ..entry()
    };
    assert!(verify_translation(&wrong_text).is_err());

    let approved_profile = GeneralRasterTranslationEntry {
        category_id: Some("status".to_string()),
        profile_id: Some("profile".to_string()),
        ..entry()
    };
    assert!(verify_translation(&approved_profile).is_err());
}

#[test]
fn 보호검사는_target_bbox_밖의_한픽셀변경도_거부한다() {
    let mut original_pixels =
        vec![0; RPG_SYS1_TEXT_ASSET.layout.width * RPG_SYS1_TEXT_ASSET.layout.height];
    let original = encode(&original_pixels, RPG_SYS1_TEXT_ASSET.layout).unwrap();
    let block = RPG_SYS1_TEXT_ASSET.blocks[0];
    original_pixels[block.bbox.y * RPG_SYS1_TEXT_ASSET.layout.width + block.bbox.x] = 1;
    let inside = encode(&original_pixels, RPG_SYS1_TEXT_ASSET.layout).unwrap();
    assert!(outside_target_block_exact(&original, &inside).unwrap());

    original_pixels[0] = 1;
    let outside = encode(&original_pixels, RPG_SYS1_TEXT_ASSET.layout).unwrap();
    assert!(!outside_target_block_exact(&original, &outside).unwrap());
}

#[test]
fn 현재_catalog와_다른_profile_후보번호는_거부한다() {
    let mut entry = entry();
    entry.profile_candidate_index += 1;
    assert!(verify_translation(&entry).is_err());
}
