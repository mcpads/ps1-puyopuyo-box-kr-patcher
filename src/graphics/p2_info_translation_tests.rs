use super::*;
use crate::btx::{Layout, PixelFormat};
use crate::graphics::text_catalog::{PaletteIndexSpec, Rect, TextBlockSpec, TextStorageSpec};

const ARCHIVES_A: [&str; 1] = ["/A.VDD;1"];
const ARCHIVES_B: [&str; 1] = ["/B.VDD;1"];
const PALETTE: [u8; 2] = [0, 1];
const BLOCK_A: [TextBlockSpec<'static>; 1] = [TextBlockSpec {
    id: "BLOCK-0001",
    source: "日本語",
    bbox: Rect {
        x: 0,
        y: 0,
        width: 256,
        height: 256,
    },
    palette_indices: PaletteIndexSpec::Values(&PALETTE),
    expected_pixels_sha256: "block-a",
}];
const BLOCK_B: [TextBlockSpec<'static>; 1] = [TextBlockSpec {
    id: "BLOCK-0002",
    source: "かな",
    bbox: Rect {
        x: 0,
        y: 0,
        width: 256,
        height: 256,
    },
    palette_indices: PaletteIndexSpec::Values(&PALETTE),
    expected_pixels_sha256: "block-b",
}];
const SPECS: [TextAssetSpec<'static>; 2] = [
    TextAssetSpec {
        archive_paths: &ARCHIVES_A,
        name: "PAGE_A.BTX",
        layout: Layout {
            width: 256,
            height: 256,
            format: PixelFormat::Indexed4,
        },
        expected_decompressed_sha256: "asset-a",
        storage: TextStorageSpec::RasterBlocks,
        blocks: &BLOCK_A,
    },
    TextAssetSpec {
        archive_paths: &ARCHIVES_B,
        name: "PAGE_B.BTX",
        layout: Layout {
            width: 256,
            height: 256,
            format: PixelFormat::Indexed4,
        },
        expected_decompressed_sha256: "asset-b",
        storage: TextStorageSpec::RasterBlocks,
        blocks: &BLOCK_B,
    },
];
const PROFILE_ID: &str = "fixture-profile";

fn row(
    category: &str,
    spec: TextAssetSpec<'static>,
    source: &str,
    ko: &str,
    profile_status: &str,
    translation_status: &str,
) -> String {
    let [block] = spec.blocks else { unreachable!() };
    [
        P2_INFO_TRANSLATION_SCHEMA_VERSION.to_string(),
        category.to_string(),
        spec.archive_paths.join("\\n"),
        spec.name.to_string(),
        spec.expected_decompressed_sha256.to_string(),
        block.id.to_string(),
        block.expected_pixels_sha256.to_string(),
        block.bbox.x.to_string(),
        block.bbox.y.to_string(),
        block.bbox.width.to_string(),
        block.bbox.height.to_string(),
        source.to_string(),
        ko.to_string(),
        PROFILE_ID.to_string(),
        profile_status.to_string(),
        translation_status.to_string(),
    ]
    .join("\t")
}

fn manifest(first_ko: &str, first_profile_status: &str, first_translation_status: &str) -> String {
    format!(
        "{HEADER}\n{}\n{}\n",
        row(
            "page-a",
            SPECS[0],
            BLOCK_A[0].source,
            first_ko,
            first_profile_status,
            first_translation_status,
        ),
        row(
            "page-b",
            SPECS[1],
            BLOCK_B[0].source,
            "둘째 초안",
            "draft-profile-human-review-required",
            "draft-human-review-required",
        )
    )
}

#[test]
fn 합성fixed_cell_page를_원문과_승인상태에_연결한다() {
    let (_, report) = verify_manifest_against(
        &manifest(
            "첫째 초안",
            "draft-profile-human-review-required",
            "draft-human-review-required",
        ),
        &SPECS,
        PROFILE_ID,
    )
    .unwrap();
    assert_eq!(
        report,
        P2InfoTranslationReport {
            entries: 2,
            pending_translations: 0,
            draft_translations: 2,
            approved_translations: 0,
            profile_approved: false,
            product_eligible: false,
        }
    );
}

#[test]
fn 보호원문과_geometry_drift를_거부한다() {
    let source_drift = manifest(
        "첫째 초안",
        "draft-profile-human-review-required",
        "draft-human-review-required",
    )
    .replacen(BLOCK_A[0].source, "다른 원문", 1);
    assert!(verify_manifest_against(&source_drift, &SPECS, PROFILE_ID)
        .unwrap_err()
        .to_string()
        .contains("보호 원본 필드 drift"));

    let geometry_drift = manifest(
        "첫째 초안",
        "draft-profile-human-review-required",
        "draft-human-review-required",
    )
    .replacen("\t0\t0\t256\t256\t日本語", "\t0\t0\t255\t256\t日本語", 1);
    assert!(verify_manifest_against(&geometry_drift, &SPECS, PROFILE_ID)
        .unwrap_err()
        .to_string()
        .contains("보호 원본 필드 drift"));
}

#[test]
fn 대기행의번역과_16x16_page초과를_거부한다() {
    let pending_with_text = manifest(
        "대기인데 있음",
        "draft-profile-human-review-required",
        "pending-human-translation",
    );
    assert!(
        verify_manifest_against(&pending_with_text, &SPECS, PROFILE_ID)
            .unwrap_err()
            .to_string()
            .contains("대기 번역에 한국어가 있음")
    );

    let too_wide = manifest(
        "가나다라마바사아자차카타파하가나다",
        "draft-profile-human-review-required",
        "draft-human-review-required",
    );
    assert!(verify_manifest_against(&too_wide, &SPECS, PROFILE_ID)
        .unwrap_err()
        .to_string()
        .contains("16x16 page 범위를 넘음"));
}

#[test]
fn 공유profile의_행별승인상태차이를_거부한다() {
    let mixed = manifest("첫째 확정", "approved", "approved");
    assert!(verify_manifest_against(&mixed, &SPECS, PROFILE_ID)
        .unwrap_err()
        .to_string()
        .contains("공유 INFO page profile 승인 상태"));
}
