use super::*;
use crate::graphics::text_catalog::{FixedCellGridSpec, PaletteIndexSpec, TextStorageSpec};

const TEST_ARCHIVES: [&str; 1] = ["/TEST_GRP.VDD;1"];
const TEST_SPEC: PendingTextAssetSpec<'static> = PendingTextAssetSpec {
    finding_id: "BTXF-TEST",
    archive_paths: &TEST_ARCHIVES,
    name: "TEST.BTX",
    review_layout: Layout {
        width: 4,
        height: 2,
        format: PixelFormat::Indexed4,
    },
    layout_basis: ReviewLayoutBasis::VisualCandidate,
    expected_decompressed_sha256:
        "1176508b0a95ae7cb03c57c5b474a4e590db632deb853d5bc0c46adf4ab627f4",
    observed_text: "テスト",
};
const TEST_RAW: [u8; 4] = [0x21, 0x03, 0x10, 0x32];

#[test]
fn 해제_원문과_review_layout이_맞아야_승인한다() {
    assert_eq!(verify_pending_text_asset(&TEST_RAW, TEST_SPEC), Ok(()));

    let mut modified = TEST_RAW;
    modified[0] ^= 1;
    assert!(matches!(
        verify_pending_text_asset(&modified, TEST_SPEC),
        Err(PendingTextError::DecompressedDigestMismatch { .. })
    ));

    let wrong_layout = PendingTextAssetSpec {
        review_layout: Layout {
            width: 8,
            height: 2,
            format: PixelFormat::Indexed4,
        },
        ..TEST_SPEC
    };
    assert!(matches!(
        verify_pending_text_asset(&TEST_RAW, wrong_layout),
        Err(PendingTextError::Btx(BtxError::DataLengthMismatch { .. }))
    ));
}

#[test]
fn 중복_id와_물리_자산과_빈_관측을_거부한다() {
    assert!(matches!(
        verify_pending_text_catalog_specs(&[TEST_SPEC, TEST_SPEC]),
        Err(PendingTextError::DuplicateFindingId(_))
    ));

    let other_id = PendingTextAssetSpec {
        finding_id: "BTXF-OTHER",
        ..TEST_SPEC
    };
    assert!(matches!(
        verify_pending_text_catalog_specs(&[TEST_SPEC, other_id]),
        Err(PendingTextError::DuplicatePhysicalAsset { .. })
    ));

    let empty_observed = PendingTextAssetSpec {
        observed_text: "",
        ..TEST_SPEC
    };
    assert!(matches!(
        verify_pending_text_catalog_specs(&[empty_observed]),
        Err(PendingTextError::EmptyObservedText { .. })
    ));
}

#[test]
fn 승인_catalog와_pending_물리_자산의_중복을_거부한다() {
    let approved = TextAssetSpec {
        archive_paths: &TEST_ARCHIVES,
        name: TEST_SPEC.name,
        storage: TextStorageSpec::RasterBlocks,
        layout: TEST_SPEC.review_layout,
        expected_decompressed_sha256: TEST_SPEC.expected_decompressed_sha256,
        blocks: &[],
    };
    assert!(matches!(
        verify_pending_text_catalog_boundary(&[TEST_SPEC], &[approved]),
        Err(PendingTextError::ApprovedPhysicalAssetConflict { .. })
    ));
    assert!(matches!(
        verify_promoted_text_catalog_provenance(&[TEST_SPEC], &[approved]),
        Err(PendingTextError::PromotedStorageMismatch { .. })
    ));

    let fixed_cell = TextAssetSpec {
        storage: TextStorageSpec::FixedCellGrid(FixedCellGridSpec {
            cell_width: 2,
            cell_height: 2,
            expected_active_cells: 1,
            expected_empty_cells: 1,
            expected_unique_glyphs: 1,
            active_palette_indices: PaletteIndexSpec::Values(&[1]),
            empty_palette_indices: PaletteIndexSpec::Values(&[0]),
        }),
        ..approved
    };
    assert!(matches!(
        verify_promoted_raster_text_catalog_provenance(&[TEST_SPEC], &[fixed_cell]),
        Err(PendingTextError::PromotedRasterStorageMismatch { .. })
    ));
}

#[test]
fn pending_surface_queue를_결정적_tsv로_내보낸다() {
    const OTHER_ARCHIVES: [&str; 1] = ["/OTHER_GRP.VDD;1"];
    let other = PendingTextAssetSpec {
        finding_id: "BTXF-A",
        archive_paths: &OTHER_ARCHIVES,
        name: "OTHER.BTX",
        observed_text: "다른 후보",
        ..TEST_SPEC
    };
    let report = render_pending_text_review_tsv_from(&[TEST_SPEC, other]).unwrap();
    let rows = report.lines().skip(1).collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    assert!(rows[0].contains("BTXF-A"));
    assert!(rows[1].contains("BTXF-TEST"));
    assert!(rows[1].contains("visual-candidate\tneeds-runtime-and-block-split"));
}
