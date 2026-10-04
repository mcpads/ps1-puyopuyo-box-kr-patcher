use super::*;

fn synthetic_spec<'a>(archives: &'a [&'a str]) -> BtxClassificationSpec<'a> {
    BtxClassificationSpec {
        sample_id: "sample",
        archive_paths: archives,
        name: "SAMPLE.BTX",
        layout: Layout {
            width: 2,
            height: 2,
            format: PixelFormat::Indexed4,
        },
        expected_decompressed_sha256:
            "5c6037e17112ad2502ced33a32a65e1df780e7996de2b65aff08f47c4c58a3d0",
        expected_palette_indices: &[0, 1, 2],
        expected_nonzero_pixels: 2,
        observed_class: BtxObservedClass::NonTextSpriteCells,
        observation: "전체 synthetic cell 검토",
    }
}

#[test]
fn 정확한_원본_layout_palette와_nonzero분모를_검증한다() {
    let spec = synthetic_spec(&["/A"]);
    let report = verify_btx_classification_asset(&[0x10, 0x02], spec).unwrap();
    assert_eq!(report.pixels, 4);
    assert_eq!(report.nonzero_pixels, 2);
    assert_eq!(report.palette_indices, 3);
}

#[test]
fn 원본_digest와_palette_drift를_거부한다() {
    let spec = synthetic_spec(&["/A"]);
    assert!(matches!(
        verify_btx_classification_asset(&[0x11, 0x02], spec),
        Err(BtxClassificationError::DigestMismatch { .. })
    ));

    let mut palette_spec = spec;
    palette_spec.expected_decompressed_sha256 =
        "5c6037e17112ad2502ced33a32a65e1df780e7996de2b65aff08f47c4c58a3d0";
    palette_spec.expected_palette_indices = &[0, 1];
    assert!(matches!(
        verify_btx_classification_asset(&[0x10, 0x02], palette_spec),
        Err(BtxClassificationError::PaletteMismatch { .. })
    ));
}

#[test]
fn 중복물리자산과_빈관측을_거부한다() {
    let spec = synthetic_spec(&["/A", "/A"]);
    assert!(matches!(
        verify_btx_classification_specs(&[spec]),
        Err(BtxClassificationError::DuplicatePhysicalAsset { .. })
    ));

    let mut empty = synthetic_spec(&["/A"]);
    empty.observation = "";
    assert!(matches!(
        verify_btx_classification_specs(&[empty]),
        Err(BtxClassificationError::EmptyObservation { .. })
    ));
}

#[test]
fn text_catalog와_같은_물리자산의_비텍스트판정을_거부한다() {
    let classified = synthetic_spec(&["/A"]);
    let text = TextAssetSpec {
        archive_paths: &["/A"],
        name: "SAMPLE.BTX",
        layout: classified.layout,
        expected_decompressed_sha256: classified.expected_decompressed_sha256,
        storage: super::super::text_catalog::TextStorageSpec::RasterBlocks,
        blocks: &[],
    };
    assert!(matches!(
        verify_btx_classification_boundary(&[classified], &[text], &[]),
        Err(BtxClassificationError::TextPhysicalAssetConflict { .. })
    ));
}
