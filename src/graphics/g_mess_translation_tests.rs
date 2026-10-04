use super::*;
use crate::btx::{Layout, PixelFormat};
use crate::graphics::text_catalog::{PaletteIndexSpec, Rect, TextBlockSpec, TextStorageSpec};

const ARCHIVES: [&str; 1] = ["/FIXTURE.VDD;1"];
const PALETTE: [u8; 2] = [0, 1];
const BLOCKS: [TextBlockSpec<'static>; 1] = [TextBlockSpec {
    id: "BLOCK-0001",
    source: "日本語",
    bbox: Rect {
        x: 0,
        y: 0,
        width: 64,
        height: 16,
    },
    palette_indices: PaletteIndexSpec::Values(&PALETTE),
    expected_pixels_sha256: "block",
}];
const SPEC: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &ARCHIVES,
    name: "FIXTURE.BTX",
    layout: Layout {
        width: 64,
        height: 16,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256: "asset",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &BLOCKS,
};
const FAMILY: &str = "fixture-family";
const CATEGORY: &str = "fixture-category";

fn manifest(source: &str, ko: &str, status: &str) -> &'static str {
    Box::leak(
        format!(
            "{MANIFEST_HEADER}\n{}\t{FAMILY}\t{}\t{}\t{source}\t{ko}\t{CATEGORY}\t{}\t{status}\n",
            BTX_RASTER_TRANSLATION_SCHEMA_VERSION,
            SPEC.name,
            BLOCKS[0].id,
            GOCHA_RESULT_MESSAGE_PROFILE.id,
        )
        .into_boxed_str(),
    )
}

#[test]
fn 합성결과문장은_원문과_번역상태를_같은행에서_검증한다() {
    let (_, draft) = verify_manifest_with_profile(
        manifest("日本語", "한글 초안", "draft-human-review-required"),
        SPEC,
        FAMILY,
        CATEGORY,
        &GOCHA_RESULT_MESSAGE_PROFILE,
    )
    .unwrap();
    assert_eq!(
        draft,
        BtxRasterTranslationReport {
            entries: 1,
            drafts: 1,
            approved: 0,
            product_eligible: false,
        }
    );

    let (_, approved) = verify_manifest_with_profile(
        manifest("日本語", "한글 확정", "approved"),
        SPEC,
        FAMILY,
        CATEGORY,
        &GOCHA_RESULT_MESSAGE_PROFILE,
    )
    .unwrap();
    assert!(approved.product_eligible);
}

#[test]
fn 결과문장_profile은_다른category와독립된_maplestory설정이다() {
    assert_eq!(
        GOCHA_RESULT_MESSAGE_PROFILE.font_id,
        MAPLESTORY_LIGHT_FONT_ID
    );
    assert_eq!(GOCHA_RESULT_MESSAGE_PROFILE.font_size, 12.0);
    assert_eq!(GOCHA_RESULT_MESSAGE_PROFILE.baseline, 13);
    assert_eq!(GOCHA_RESULT_MESSAGE_PROFILE.line_count, 1);
    assert_eq!(GOCHA_RESULT_MESSAGE_PROFILE.body_ramp, &BODY_RAMP);
    assert_eq!(GOCHA_RESULT_MESSAGE_PROFILE.effect, BakedTextEffect::None);
}

#[test]
fn 미승인초안은_제품patch자격에서거부한다() {
    let report = BtxRasterTranslationReport {
        entries: 2,
        drafts: 2,
        approved: 0,
        product_eligible: false,
    };
    assert_eq!(
        require_product_eligible(report),
        Err(GmessTranslationError::UnapprovedDrafts { drafts: 2 })
    );
}

#[test]
fn 보호원문drift와_일본어잔존을거부한다() {
    let drift = verify_manifest_with_profile(
        manifest("다른 원문", "한글 초안", "draft-human-review-required"),
        SPEC,
        FAMILY,
        CATEGORY,
        &GOCHA_RESULT_MESSAGE_PROFILE,
    );
    assert!(drift.unwrap_err().to_string().contains("보호 원문"));

    let japanese = verify_manifest_with_profile(
        manifest("日本語", "아직 일본어だ", "draft-human-review-required"),
        SPEC,
        FAMILY,
        CATEGORY,
        &GOCHA_RESULT_MESSAGE_PROFILE,
    );
    assert!(japanese.unwrap_err().to_string().contains("번역·행·bbox"));
}
