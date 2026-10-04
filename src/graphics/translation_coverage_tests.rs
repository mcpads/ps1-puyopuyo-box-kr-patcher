use super::*;
use crate::btx::{Layout, PixelFormat};
use crate::graphics::text_catalog::{PaletteIndexSpec, Rect, TextBlockSpec, TextStorageSpec};

const ARCHIVES: [&str; 1] = ["/FIXTURE.VDD;1"];
const PALETTE: [u8; 2] = [0, 1];
const BLOCKS: [TextBlockSpec<'static>; 3] = [
    TextBlockSpec {
        id: "BLOCK-0001",
        source: "日本語",
        bbox: Rect {
            x: 0,
            y: 0,
            width: 8,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&PALETTE),
        expected_pixels_sha256: "block-one",
    },
    TextBlockSpec {
        id: "BLOCK-0002",
        source: "かな",
        bbox: Rect {
            x: 8,
            y: 0,
            width: 8,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&PALETTE),
        expected_pixels_sha256: "block-two",
    },
    TextBlockSpec {
        id: "BLOCK-0003",
        source: "REFERENCE",
        bbox: Rect {
            x: 16,
            y: 0,
            width: 8,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&PALETTE),
        expected_pixels_sha256: "block-three",
    },
];
const SPEC: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &ARCHIVES,
    name: "FIXTURE.BTX",
    layout: Layout {
        width: 24,
        height: 16,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256: "asset",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &BLOCKS,
};

fn managed(
    block_id: &str,
    protected_source: &str,
    status: ManagedTranslationStatus,
) -> ManagedTranslation {
    ManagedTranslation {
        block_id: block_id.to_string(),
        protected_source: protected_source.to_string(),
        status,
        category: "fixture-category".to_string(),
        profile_id: "fixture-profile".to_string(),
        translation: "한글 초안".to_string(),
    }
}

#[test]
fn 합성_catalog의_관리상태와_누락을_집계한다() {
    let entries = [
        managed(
            "BLOCK-0001",
            "日本語",
            ManagedTranslationStatus::DraftHumanReviewRequired,
        ),
        managed("BLOCK-0002", "かな", ManagedTranslationStatus::Approved),
    ];
    assert_eq!(
        verify_coverage(&[SPEC], &entries),
        Ok(TranslationCoverageReport {
            catalog_japanese_targets: 2,
            managed_targets: 2,
            poc_approved: 0,
            approved: 1,
            draft_human_review_required: 1,
            pending_human_translation: 0,
            scope_excluded_protected_no_write: 0,
            unmanaged_targets: 0,
        })
    );

    assert_eq!(
        verify_coverage(&[SPEC], &entries[..1])
            .unwrap()
            .unmanaged_targets,
        1
    );
}

#[test]
fn 중복과_원문_drift와_reference_only를_거부한다() {
    let duplicate = [
        managed(
            "BLOCK-0001",
            "日本語",
            ManagedTranslationStatus::PendingHumanTranslation,
        ),
        managed(
            "BLOCK-0001",
            "日本語",
            ManagedTranslationStatus::PendingHumanTranslation,
        ),
    ];
    assert!(matches!(
        verify_coverage(&[SPEC], &duplicate),
        Err(TranslationCoverageError::DuplicateManagedBlock(_))
    ));

    let drift = [managed(
        "BLOCK-0001",
        "다른 원문",
        ManagedTranslationStatus::PendingHumanTranslation,
    )];
    assert!(matches!(
        verify_coverage(&[SPEC], &drift),
        Err(TranslationCoverageError::ProtectedSourceMismatch { .. })
    ));

    let reference_only = [managed(
        "BLOCK-0003",
        "REFERENCE",
        ManagedTranslationStatus::PendingHumanTranslation,
    )];
    assert!(matches!(
        verify_coverage(&[SPEC], &reference_only),
        Err(TranslationCoverageError::ReferenceOnlyManagedBlock(_))
    ));
}

#[test]
fn catalog에_없는_관리_id를_거부한다() {
    let unknown = [managed(
        "BLOCK-9999",
        "日本語",
        ManagedTranslationStatus::PendingHumanTranslation,
    )];
    assert!(matches!(
        verify_coverage(&[SPEC], &unknown),
        Err(TranslationCoverageError::UnknownManagedBlock(_))
    ));
}

#[test]
fn review_tsv는_합성입력을_id순서와_관리상태로_내보낸다() {
    let mut first = managed(
        "BLOCK-0001",
        "日本語",
        ManagedTranslationStatus::DraftHumanReviewRequired,
    );
    first.translation = "첫줄\n둘째줄".to_string();
    let entries = [
        managed("BLOCK-0002", "かな", ManagedTranslationStatus::Approved),
        first,
    ];
    let report = render_translation_review_tsv_from(&[SPEC], &entries).unwrap();
    let rows = report.lines().skip(1).collect::<Vec<_>>();

    assert_eq!(rows.len(), 2);
    assert!(rows[0].contains("\tBLOCK-0001\t"));
    assert!(rows[0].contains("\tdraft-human-review-required\t"));
    assert!(rows[0].ends_with("첫줄\\n둘째줄"));
    assert!(rows[1].contains("\tBLOCK-0002\t"));
    assert!(rows[1].contains("\tapproved\t"));
}
