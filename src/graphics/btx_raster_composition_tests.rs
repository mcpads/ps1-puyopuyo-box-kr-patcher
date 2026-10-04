use super::*;

fn translation(
    block_id: &str,
    source: &str,
    ko: &str,
    status: GeneralRasterTranslationStatus,
) -> GeneralRasterTranslationEntry {
    GeneralRasterTranslationEntry {
        semantic_family: "fixture-family".to_string(),
        profile_candidate_index: 1,
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
        protected_source: source.to_string(),
        ko: ko.to_string(),
        status,
    }
}

fn translations(status: GeneralRasterTranslationStatus) -> Vec<GeneralRasterTranslationEntry> {
    vec![
        translation("BLOCK-0001", "日本", "한", status),
        translation("BLOCK-0002", "語", "글", status),
    ]
}

fn row(
    unit_id: &str,
    block_ids: &str,
    fragments: &str,
    composed_source: &str,
    consumer_order: &str,
    ko: &str,
    status: &str,
) -> String {
    [
        BTX_RASTER_COMPOSITION_SCHEMA_VERSION,
        unit_id,
        "fixture-family",
        "FIXTURE.BTX",
        block_ids,
        fragments,
        composed_source,
        "static-cell-linguistic-fragment",
        consumer_order,
        ko,
        status,
    ]
    .join("\t")
}

fn manifest(consumer_order: &str, status: &str, ko: &str) -> String {
    format!(
        "{HEADER}\n{}\n",
        row(
            "UNIT-0001",
            "BLOCK-0001\\nBLOCK-0002",
            "日本\\n語",
            "日本語",
            consumer_order,
            ko,
            status,
        )
    )
}

#[test]
fn 합성조립단위는_구성block의_상태와_번역을_연결한다() {
    let report = verify_manifest(
        &manifest(
            "runtime-order-unverified",
            "draft-human-review-required",
            "한 글",
        ),
        &translations(GeneralRasterTranslationStatus::DraftHumanReviewRequired),
    )
    .unwrap();
    assert_eq!(
        report,
        RasterCompositionReport {
            units: 1,
            member_blocks: 2,
            pending_units: 0,
            draft_units: 1,
            approved_units: 0,
            runtime_order_verified_units: 0,
            product_eligible_units: 0,
        }
    );
}

#[test]
fn 조립단위는_구성block과_합성문의_번역불일치를_거부한다() {
    let mut entries = translations(GeneralRasterTranslationStatus::DraftHumanReviewRequired);
    entries[1].ko = "다름".to_string();
    assert!(verify_manifest(
        &manifest(
            "runtime-order-unverified",
            "draft-human-review-required",
            "한 글",
        ),
        &entries,
    )
    .unwrap_err()
    .to_string()
    .contains("조립 번역문 불일치"));
}

#[test]
fn 조립단위는_보호원문과_member중복을_거부한다() {
    let source_drift = format!(
        "{HEADER}\n{}\n",
        row(
            "UNIT-0001",
            "BLOCK-0001\\nBLOCK-0002",
            "日本\\n字",
            "日本語",
            "runtime-order-unverified",
            "한 글",
            "draft-human-review-required",
        )
    );
    assert!(parse_manifest(&source_drift)
        .unwrap_err()
        .to_string()
        .contains("보호 조립 원문 불일치"));

    let duplicated = format!(
        "{HEADER}\n{}\n",
        row(
            "UNIT-0001",
            "BLOCK-0001\\nBLOCK-0001",
            "日本\\n日本",
            "日本日本",
            "runtime-order-unverified",
            "한한",
            "draft-human-review-required",
        )
    );
    assert!(verify_manifest(
        &duplicated,
        &translations(GeneralRasterTranslationStatus::DraftHumanReviewRequired),
    )
    .unwrap_err()
    .to_string()
    .contains("member block 중복"));
}

#[test]
fn 승인에는_구성block상태와_runtime순서가_모두_필요하다() {
    let mixed = verify_manifest(
        &manifest("runtime-order-verified", "approved", "한글"),
        &translations(GeneralRasterTranslationStatus::DraftHumanReviewRequired),
    );
    assert!(mixed
        .unwrap_err()
        .to_string()
        .contains("구성 block 상태 불일치"));

    assert!(
        parse_manifest(&manifest("runtime-order-unverified", "approved", "한글",))
            .unwrap_err()
            .to_string()
            .contains("runtime 순서 미검증 단위가 승인됨")
    );

    let approved = verify_manifest(
        &manifest("runtime-order-verified", "approved", "한글"),
        &translations(GeneralRasterTranslationStatus::Approved),
    )
    .unwrap();
    assert_eq!(approved.product_eligible_units, 1);
}
