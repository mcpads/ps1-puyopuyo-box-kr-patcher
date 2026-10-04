use super::*;

fn translation(block_id: &str, ko: &str) -> GeneralRasterTranslationEntry {
    GeneralRasterTranslationEntry {
        semantic_family: "fixture".to_string(),
        profile_candidate_index: 0,
        category_id: None,
        profile_id: None,
        archive_paths: "/FIXTURE.VDD;1".to_string(),
        asset_name: "FIXTURE.BTX".to_string(),
        asset_sha256: "asset".to_string(),
        block_id: block_id.to_string(),
        block_pixels_sha256: "block".to_string(),
        bbox_x: 0,
        bbox_y: 0,
        bbox_width: 16,
        bbox_height: 16,
        protected_source: "日本語".to_string(),
        ko: ko.to_string(),
        status: GeneralRasterTranslationStatus::DraftHumanReviewRequired,
    }
}

#[test]
fn 수동조판문자범위는_utf8_byte가아니라_unicode문자로자른다() {
    let entry = translation("BLOCK", "COM 난이도");
    let translations = BTreeMap::from([("BLOCK", &entry)]);

    assert_eq!(
        translation_slice(
            &translations,
            TranslationSlice {
                block_id: "BLOCK",
                start: 4,
                end: 7,
            },
        )
        .unwrap(),
        "난이도"
    );
    assert!(translation_slice(
        &translations,
        TranslationSlice {
            block_id: "BLOCK",
            start: 4,
            end: 8,
        },
    )
    .is_err());
}

#[test]
fn exact_boxopt이아닌입력은_최종합성전에실패한다() {
    let invalid = vec![0; BOXOPT_TEXT_ASSET.layout.byte_len().unwrap()];

    assert!(patch_boxopt_development_surface(&invalid).is_err());
}
