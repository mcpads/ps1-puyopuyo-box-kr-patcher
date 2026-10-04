use super::*;
use crate::btx::encode;

const TEST_LAYOUT: Layout = Layout {
    width: 4,
    height: 2,
    format: PixelFormat::Indexed4,
};
const TEST_RAW: [u8; 4] = [0x21, 0x03, 0x10, 0x32];
const TEST_INDICES: [u8; 3] = [1, 2, 3];
const TEST_BLOCKS: [TextBlockSpec<'static>; 1] = [TextBlockSpec {
    id: "BTXB-TEST",
    source: "TEST",
    bbox: Rect {
        x: 1,
        y: 0,
        width: 2,
        height: 2,
    },
    palette_indices: PaletteIndexSpec::Values(&TEST_INDICES),
    expected_pixels_sha256: "8264e616f5c7d5d1ba76e8daa428ef656e0abe08279a5376c287c79a7fb3f481",
}];
const TEST_ARCHIVES: [&str; 1] = ["/TEST_GRP.VDD;1"];
const TEST_SPEC: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &TEST_ARCHIVES,
    name: "TEST.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: TEST_LAYOUT,
    expected_decompressed_sha256:
        "1176508b0a95ae7cb03c57c5b474a4e590db632deb853d5bc0c46adf4ab627f4",
    blocks: &TEST_BLOCKS,
};

#[test]
fn 합성catalog의_물리자산과_block_id를_유일하게_센다() {
    assert_eq!(
        verify_text_catalog_specs(&[TEST_SPEC]),
        Ok(TextCatalogSpecReport {
            assets: 1,
            declared_physical_assets: 1,
            blocks: 1,
            japanese_target_blocks: 0,
        })
    );
    assert_eq!(
        verify_text_catalog_specs(&[]),
        Err(TextCatalogError::EmptyCatalog)
    );

    assert!(matches!(
        verify_text_catalog_specs(&[TEST_SPEC, TEST_SPEC]),
        Err(TextCatalogError::DuplicatePhysicalAsset { .. })
    ));

    let other_archives = ["/OTHER_GRP.VDD;1"];
    let other = TextAssetSpec {
        archive_paths: &other_archives,
        name: "OTHER.BTX",
        ..TEST_SPEC
    };
    assert!(matches!(
        verify_text_catalog_specs(&[TEST_SPEC, other]),
        Err(TextCatalogError::DuplicateBlockId(_))
    ));
}

#[test]
fn fixed_cell_grid는_문장과_반복글리프와_empty_tail을_함께검증한다() {
    let layout = Layout {
        width: 4,
        height: 4,
        format: PixelFormat::Indexed4,
    };
    let pixels = [1, 2, 2, 1, 1, 1, 1, 1, 1, 1, 0, 0, 2, 1, 0, 0];
    let raw = encode(&pixels, layout).unwrap();
    let raw_digest = digest(&raw);
    let pixels_digest = digest(&pixels);
    let page_palette = [0, 1, 2];
    let active_palette = [1, 2];
    let empty_palette = [0];
    let blocks = [TextBlockSpec {
        id: "BTXB-GRID-TEST",
        source: "あい\nう",
        bbox: Rect {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        },
        palette_indices: PaletteIndexSpec::Values(&page_palette),
        expected_pixels_sha256: &pixels_digest,
    }];
    let spec = TextAssetSpec {
        archive_paths: &TEST_ARCHIVES,
        name: "GRID.BTX",
        storage: TextStorageSpec::FixedCellGrid(FixedCellGridSpec {
            cell_width: 2,
            cell_height: 2,
            expected_active_cells: 3,
            expected_empty_cells: 1,
            expected_unique_glyphs: 3,
            active_palette_indices: PaletteIndexSpec::Values(&active_palette),
            empty_palette_indices: PaletteIndexSpec::Values(&empty_palette),
        }),
        layout,
        expected_decompressed_sha256: &raw_digest,
        blocks: &blocks,
    };

    assert_eq!(
        verify_text_asset(&raw, spec),
        Ok(TextCatalogReport {
            declared_physical_assets: 1,
            blocks: 1,
            block_pixels: 12,
            japanese_target_blocks: 1,
            japanese_target_pixels: 12,
        })
    );

    let conflicting_blocks = [TextBlockSpec {
        source: "あい\nあ",
        ..blocks[0]
    }];
    assert!(matches!(
        verify_text_asset(
            &raw,
            TextAssetSpec {
                blocks: &conflicting_blocks,
                ..spec
            }
        ),
        Err(TextCatalogError::FixedGridCharacterBitmapMismatch { .. })
    ));
}

#[test]
fn 원본과_bbox와_palette가_모두_맞아야_승인한다() {
    assert_eq!(
        verify_text_asset(&TEST_RAW, TEST_SPEC),
        Ok(TextCatalogReport {
            declared_physical_assets: 1,
            blocks: 1,
            block_pixels: 4,
            japanese_target_blocks: 0,
            japanese_target_pixels: 0,
        })
    );
}

#[test]
fn palette_bitset은_목록과_같은_exact집합을_검증한다() {
    let bitset_blocks = [TextBlockSpec {
        palette_indices: PaletteIndexSpec::Bitset256([0x0E, 0, 0, 0]),
        ..TEST_BLOCKS[0]
    }];
    assert!(verify_text_asset(
        &TEST_RAW,
        TextAssetSpec {
            blocks: &bitset_blocks,
            ..TEST_SPEC
        }
    )
    .is_ok());

    let empty_blocks = [TextBlockSpec {
        palette_indices: PaletteIndexSpec::Bitset256([0; 4]),
        ..TEST_BLOCKS[0]
    }];
    assert!(matches!(
        verify_text_asset(
            &TEST_RAW,
            TextAssetSpec {
                blocks: &empty_blocks,
                ..TEST_SPEC
            }
        ),
        Err(TextCatalogError::EmptyPaletteIndices { .. })
    ));

    let out_of_range_blocks = [TextBlockSpec {
        palette_indices: PaletteIndexSpec::Bitset256([0x1_000E, 0, 0, 0]),
        ..TEST_BLOCKS[0]
    }];
    assert!(matches!(
        verify_text_asset(
            &TEST_RAW,
            TextAssetSpec {
                blocks: &out_of_range_blocks,
                ..TEST_SPEC
            }
        ),
        Err(TextCatalogError::PaletteIndexOutOfRange { value: 16, .. })
    ));
}

#[test]
fn 일본어가_있는_block만_번역대상으로_센다() {
    assert!(!is_japanese_translation_target("BEST RECORD"));
    assert!(!is_japanese_translation_target("1"));
    assert!(is_japanese_translation_target("ぷよぷよ / PUYO PUYO"));
    assert!(is_japanese_translation_target("3勝戦"));

    let japanese_blocks = [TextBlockSpec {
        source: "テスト",
        ..TEST_BLOCKS[0]
    }];
    assert_eq!(
        verify_text_asset(
            &TEST_RAW,
            TextAssetSpec {
                blocks: &japanese_blocks,
                ..TEST_SPEC
            }
        )
        .unwrap()
        .japanese_target_blocks,
        1
    );
}

#[test]
fn 해제원문_digest_drift를_bbox검사전에_거부한다() {
    let mut modified = TEST_RAW;
    modified[0] ^= 1;
    assert!(matches!(
        verify_text_asset(&modified, TEST_SPEC),
        Err(TextCatalogError::DecompressedDigestMismatch { .. })
    ));
}

#[test]
fn bbox가_layout을_벗어나거나_서로_겹치면_거부한다() {
    let out_of_bounds = [TextBlockSpec {
        bbox: Rect {
            x: 3,
            y: 0,
            width: 2,
            height: 1,
        },
        ..TEST_BLOCKS[0]
    }];
    assert!(matches!(
        verify_text_asset(
            &TEST_RAW,
            TextAssetSpec {
                blocks: &out_of_bounds,
                ..TEST_SPEC
            }
        ),
        Err(TextCatalogError::BboxOutOfBounds { .. })
    ));

    let overlapping = [
        TEST_BLOCKS[0],
        TextBlockSpec {
            id: "BTXB-TEST-2",
            bbox: Rect {
                x: 2,
                y: 0,
                width: 1,
                height: 1,
            },
            ..TEST_BLOCKS[0]
        },
    ];
    assert!(matches!(
        verify_text_asset(
            &TEST_RAW,
            TextAssetSpec {
                blocks: &overlapping,
                ..TEST_SPEC
            }
        ),
        Err(TextCatalogError::OverlappingBlocks { .. })
    ));
}

#[test]
fn sparse_selector는_허용한_겹침과_물리_union을_exact로_검증한다() {
    let all_indices = [0, 1, 2, 3];
    let right_indices = [0, 2, 3];
    let blocks = [
        TextBlockSpec {
            id: "BTXB-SPARSE-A",
            source: "テスト",
            bbox: Rect {
                x: 0,
                y: 0,
                width: 3,
                height: 2,
            },
            palette_indices: PaletteIndexSpec::Values(&all_indices),
            expected_pixels_sha256:
                "a6da5671467b03d21e4fa38ff746d6ca6152b029c174fdf0635edd48497ffd24",
        },
        TextBlockSpec {
            id: "BTXB-SPARSE-B",
            source: "COM",
            bbox: Rect {
                x: 2,
                y: 0,
                width: 2,
                height: 2,
            },
            palette_indices: PaletteIndexSpec::Values(&right_indices),
            expected_pixels_sha256:
                "76c7034fc9dc29496d9a621f3b54a7b1d807da2ab8705cf2c58f35ce1b3233d6",
        },
    ];
    let allowed = [("BTXB-SPARSE-A", "BTXB-SPARSE-B")];
    let spec = TextAssetSpec {
        blocks: &blocks,
        storage: TextStorageSpec::SparseSelectorAtlas(SparseSelectorAtlasSpec {
            allowed_overlap_pairs: &allowed,
            expected_physical_union_pixels: 8,
        }),
        ..TEST_SPEC
    };

    assert_eq!(
        verify_text_asset(&TEST_RAW, spec),
        Ok(TextCatalogReport {
            declared_physical_assets: 1,
            blocks: 2,
            block_pixels: 8,
            japanese_target_blocks: 1,
            japanese_target_pixels: 6,
        })
    );

    let no_allowed_overlaps = [];
    assert!(matches!(
        verify_text_catalog_specs(&[TextAssetSpec {
            storage: TextStorageSpec::SparseSelectorAtlas(SparseSelectorAtlasSpec {
                allowed_overlap_pairs: &no_allowed_overlaps,
                expected_physical_union_pixels: 8,
            }),
            ..spec
        }]),
        Err(TextCatalogError::SparseOverlapSetMismatch { .. })
    ));
    assert!(matches!(
        verify_text_catalog_specs(&[TextAssetSpec {
            storage: TextStorageSpec::SparseSelectorAtlas(SparseSelectorAtlasSpec {
                allowed_overlap_pairs: &allowed,
                expected_physical_union_pixels: 7,
            }),
            ..spec
        }]),
        Err(TextCatalogError::SparsePhysicalUnionMismatch { .. })
    ));
}

#[test]
fn 물리archive와_block_id중복을_거부한다() {
    let duplicate_archives = ["/TEST_GRP.VDD;1", "/TEST_GRP.VDD;1"];
    assert!(matches!(
        verify_text_asset(
            &TEST_RAW,
            TextAssetSpec {
                archive_paths: &duplicate_archives,
                ..TEST_SPEC
            }
        ),
        Err(TextCatalogError::DuplicateArchivePath(_))
    ));

    let duplicate_blocks = [
        TEST_BLOCKS[0],
        TextBlockSpec {
            bbox: Rect {
                x: 3,
                y: 1,
                width: 1,
                height: 1,
            },
            ..TEST_BLOCKS[0]
        },
    ];
    assert!(matches!(
        verify_text_asset(
            &TEST_RAW,
            TextAssetSpec {
                blocks: &duplicate_blocks,
                ..TEST_SPEC
            }
        ),
        Err(TextCatalogError::DuplicateBlockId(_))
    ));
}

#[test]
fn 선언하지않은_palette_index와_region_drift를_거부한다() {
    const WRONG_INDICES: [u8; 2] = [1, 2];
    let wrong_style = [TextBlockSpec {
        palette_indices: PaletteIndexSpec::Values(&WRONG_INDICES),
        ..TEST_BLOCKS[0]
    }];
    assert!(matches!(
        verify_text_asset(
            &TEST_RAW,
            TextAssetSpec {
                blocks: &wrong_style,
                ..TEST_SPEC
            }
        ),
        Err(TextCatalogError::PaletteSetMismatch { .. })
    ));

    const UNSORTED_INDICES: [u8; 3] = [3, 1, 2];
    let unsorted_style = [TextBlockSpec {
        palette_indices: PaletteIndexSpec::Values(&UNSORTED_INDICES),
        ..TEST_BLOCKS[0]
    }];
    assert!(matches!(
        verify_text_asset(
            &TEST_RAW,
            TextAssetSpec {
                blocks: &unsorted_style,
                ..TEST_SPEC
            }
        ),
        Err(TextCatalogError::UnsortedPaletteIndices { .. })
    ));

    let wrong_digest = [TextBlockSpec {
        expected_pixels_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        ..TEST_BLOCKS[0]
    }];
    assert!(matches!(
        verify_text_asset(
            &TEST_RAW,
            TextAssetSpec {
                blocks: &wrong_digest,
                ..TEST_SPEC
            }
        ),
        Err(TextCatalogError::PixelDigestMismatch { .. })
    ));
}
