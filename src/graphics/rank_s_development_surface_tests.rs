use super::*;

#[test]
fn ranking_label은_열개_mask와_수동용어에결속된다() {
    assert_eq!(
        verify_source_text_mask_policy_for_asset(RANK_S_TEXT_ASSET).unwrap(),
        SOURCE_TEXT_MASKS.len()
    );
    assert_eq!(
        SOURCE_TEXT_MASKS
            .iter()
            .map(|mask| mask.expected_pixels)
            .sum::<usize>(),
        SOURCE_TEXT_PIXELS
    );
    let translations = general_btx_raster_translation_entries()
        .unwrap()
        .into_iter()
        .filter(|entry| entry.asset_name == RANK_S_TEXT_ASSET.name)
        .collect::<Vec<_>>();
    assert_eq!(translations.len(), SOURCE_TEXT_MASKS.len());
    assert!(translations.iter().all(|entry| !entry.ko.contains('\n')));
    for (block_id, expected) in [
        ("BTXB-0085", "순한맛 코스"),
        ("BTXB-0086", "보통맛 코스"),
        ("BTXB-0087", "매운맛 코스"),
        ("BTXB-0089", "둘이서 뿌요"),
        ("BTXB-0091", "둘이서 뿌요"),
    ] {
        assert_eq!(
            translations
                .iter()
                .find(|entry| entry.block_id == block_id)
                .unwrap()
                .ko,
            expected
        );
    }
}

#[test]
fn ranking_label_profile은_실제_셀높이와_색역할에_맞는다() {
    let profile = general_btx_raster_profile_assignments()
        .unwrap()
        .into_iter()
        .find(|profile| {
            profile.semantic_family == "shell-record-ranking"
                && profile
                    .configuration
                    .as_ref()
                    .is_some_and(|config| config.profile_id == "shell-ranking-label-development")
        })
        .unwrap()
        .configuration
        .unwrap();

    assert_eq!(profile.font_id, "maplestory-light");
    assert_eq!(profile.line_count, 1);
    assert_eq!(profile.line_height, 24);
    assert_eq!(profile.background, BACKGROUND_INDEX);
}

#[test]
fn exact_rank_s가아닌입력은_mask합성전에실패한다() {
    let invalid = vec![0; RANK_S_TEXT_ASSET.layout.byte_len().unwrap()];

    assert!(patch_rank_s_development_surface(&invalid).is_err());
}

#[test]
fn 랭킹의_실제_sprite가_읽는_제목과_코스_영역을_덮는다() {
    // 47f03e73 BIN의 BOX 랭킹 RAM에서 읽은 GP0 sprite UV·CLUT 및 크기다.
    // 화면 제목과 코스의 소비 영역을 잘못된 16px catalog로 되돌리지 않는다.
    for (id, uv_clut, wh) in [
        ("BTXB-0085", 0x7a80_0000_u32, 0x0018_0060_u32),
        ("BTXB-0093", 0x7a81_6000_u32, 0x0018_00b0_u32),
    ] {
        let block = RANK_S_TEXT_ASSET
            .blocks
            .iter()
            .find(|block| block.id == id)
            .unwrap();
        assert_eq!(block.bbox.x, (uv_clut & 0xff) as usize);
        assert_eq!(block.bbox.y, ((uv_clut >> 8) & 0xff) as usize);
        assert_eq!(block.bbox.width, (wh & 0xffff) as usize);
        assert_eq!(block.bbox.height, (wh >> 16) as usize);
    }
}
