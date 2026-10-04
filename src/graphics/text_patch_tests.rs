use sha2::{Digest, Sha256};

use super::*;
use crate::btx::{encode, Layout};
use crate::graphics::text_catalog::{
    FixedCellGridSpec, PaletteIndexSpec, Rect, SparseSelectorAtlasSpec, TextStorageSpec,
};
use crate::graphics::text_profile::{
    BakedFontBook, BakedTextEffect, BakedTextProfile, HorizontalAlign, MAPLESTORY_LIGHT_FONT_ID,
};

const TEST_INDICES: [u8; 14] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
const TEST_ARCHIVES: [&str; 1] = ["/TEST_GRP.VDD;1"];

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn fixture_with_height(height: usize) -> (Vec<u8>, String, String) {
    let layout = Layout {
        width: 200,
        height,
        format: PixelFormat::Indexed4,
    };
    let pixels = (0..layout.width * layout.height)
        .map(|index| (index % TEST_INDICES.len()) as u8)
        .collect::<Vec<_>>();
    let raw = encode(&pixels, layout).unwrap();
    let mut block_pixels = Vec::new();
    for row in 0..height {
        block_pixels.extend_from_slice(&pixels[row * 200 + 10..row * 200 + 190]);
    }
    (raw.clone(), digest(&raw), digest(&block_pixels))
}

fn fixture(_source: &str) -> (Vec<u8>, String, String) {
    fixture_with_height(16)
}

fn profile() -> BakedTextProfile<'static> {
    BakedTextProfile {
        id: "test-body",
        font_id: MAPLESTORY_LIGHT_FONT_ID,
        font_size: 12.0,
        baseline: 13,
        line_count: 1,
        line_height: 16,
        tracking: 0,
        align: HorizontalAlign::Center,
        background: 0,
        body_ramp: &TEST_INDICES[1..],
        effect: BakedTextEffect::None,
    }
}

fn fonts() -> BakedFontBook {
    BakedFontBook::with_bundled_maplestory_light().unwrap()
}

#[test]
fn fixed_cell_grid는_일반_bbox_rasterizer로_쓰지_못한다() {
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
        patch_baked_text(&raw, spec, &fonts(), &[]),
        Err(BakedTextPatchError::FixedCellGridRequiresAssembler(
            "GRID.BTX".to_string()
        ))
    );
}

#[test]
fn sparse_selector_atlas는_일반_bbox_rasterizer로_쓰지_못한다() {
    let (raw, raw_digest, block_digest) = fixture("日本語");
    let blocks = [TextBlockSpec {
        id: "BTXB-SPARSE-TEST",
        source: "日本語",
        bbox: Rect {
            x: 10,
            y: 0,
            width: 180,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&TEST_INDICES),
        expected_pixels_sha256: &block_digest,
    }];
    let allowed_overlaps = [];
    let spec = TextAssetSpec {
        archive_paths: &TEST_ARCHIVES,
        name: "SPARSE.BTX",
        storage: TextStorageSpec::SparseSelectorAtlas(SparseSelectorAtlasSpec {
            allowed_overlap_pairs: &allowed_overlaps,
            expected_physical_union_pixels: 180 * 16,
        }),
        layout: Layout {
            width: 200,
            height: 16,
            format: PixelFormat::Indexed4,
        },
        expected_decompressed_sha256: &raw_digest,
        blocks: &blocks,
    };

    assert_eq!(
        patch_baked_text(&raw, spec, &fonts(), &[]),
        Err(
            BakedTextPatchError::SparseSelectorAtlasRequiresSurfaceComposer(
                "SPARSE.BTX".to_string()
            )
        )
    );
}

#[test]
fn 일본어_block만_bbox와_palette_안에서_한글로_바꾼다() {
    let (raw, raw_digest, block_digest) = fixture("日本語");
    let blocks = [TextBlockSpec {
        id: "BTXB-TEST",
        source: "日本語",
        bbox: Rect {
            x: 10,
            y: 0,
            width: 180,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&TEST_INDICES),
        expected_pixels_sha256: &block_digest,
    }];
    let spec = TextAssetSpec {
        archive_paths: &TEST_ARCHIVES,
        name: "TEST.BTX",
        storage: TextStorageSpec::RasterBlocks,
        layout: Layout {
            width: 200,
            height: 16,
            format: PixelFormat::Indexed4,
        },
        expected_decompressed_sha256: &raw_digest,
        blocks: &blocks,
    };
    let fonts = fonts();
    let profile = profile();
    let (patched, report) = patch_baked_text(
        &raw,
        spec,
        &fonts,
        &[BakedTextReplacement {
            block_id: "BTXB-TEST",
            text: "한국어 문장",
            profile: &profile,
        }],
    )
    .unwrap();

    assert_eq!(patched.len(), raw.len());
    assert_eq!(report.replacement_blocks, 1);
    assert!(report.changed_pixels > 0);
    assert!(report.changed_bytes > 0);
    let original_pixels = decode(&raw, spec.layout).unwrap();
    let patched_pixels = decode(&patched, spec.layout).unwrap();
    for row in 0..16 {
        assert_eq!(
            &patched_pixels[row * 200..row * 200 + 10],
            &original_pixels[row * 200..row * 200 + 10]
        );
        assert_eq!(
            &patched_pixels[row * 200 + 190..row * 200 + 200],
            &original_pixels[row * 200 + 190..row * 200 + 200]
        );
    }
    assert!(patched_pixels.iter().all(|value| *value <= 13));
}

#[test]
fn 영문_reference와_한글_없는_번역을_거부한다() {
    let (raw, raw_digest, block_digest) = fixture("BEST RECORD");
    let blocks = [TextBlockSpec {
        id: "BTXB-TEST",
        source: "BEST RECORD",
        bbox: Rect {
            x: 10,
            y: 0,
            width: 180,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&TEST_INDICES),
        expected_pixels_sha256: &block_digest,
    }];
    let spec = TextAssetSpec {
        archive_paths: &TEST_ARCHIVES,
        name: "TEST.BTX",
        storage: TextStorageSpec::RasterBlocks,
        layout: Layout {
            width: 200,
            height: 16,
            format: PixelFormat::Indexed4,
        },
        expected_decompressed_sha256: &raw_digest,
        blocks: &blocks,
    };
    let fonts = fonts();
    let profile = profile();
    assert!(matches!(
        patch_baked_text(
            &raw,
            spec,
            &fonts,
            &[BakedTextReplacement {
                block_id: "BTXB-TEST",
                text: "한국어",
                profile: &profile,
            }]
        ),
        Err(BakedTextPatchError::ReferenceOnlyBlock(_))
    ));

    let japanese_blocks = [TextBlockSpec {
        source: "日本語",
        ..blocks[0]
    }];
    assert!(matches!(
        patch_baked_text(
            &raw,
            TextAssetSpec {
                blocks: &japanese_blocks,
                ..spec
            },
            &fonts,
            &[BakedTextReplacement {
                block_id: "BTXB-TEST",
                text: "KOREAN",
                profile: &profile,
            }]
        ),
        Err(BakedTextPatchError::MissingHangul(_))
    ));
}

#[test]
fn 잘림과_미등록_palette와_중복_쓰기를_거부한다() {
    let (raw, raw_digest, block_digest) = fixture("日本語");
    let blocks = [TextBlockSpec {
        id: "BTXB-TEST",
        source: "日本語",
        bbox: Rect {
            x: 10,
            y: 0,
            width: 180,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&TEST_INDICES),
        expected_pixels_sha256: &block_digest,
    }];
    let spec = TextAssetSpec {
        archive_paths: &TEST_ARCHIVES,
        name: "TEST.BTX",
        storage: TextStorageSpec::RasterBlocks,
        layout: Layout {
            width: 200,
            height: 16,
            format: PixelFormat::Indexed4,
        },
        expected_decompressed_sha256: &raw_digest,
        blocks: &blocks,
    };
    let fonts = fonts();
    let profile = profile();
    let long = BakedTextReplacement {
        block_id: "BTXB-TEST",
        text: "아주아주아주아주아주아주아주아주아주긴문장",
        profile: &profile,
    };
    assert!(matches!(
        patch_baked_text(&raw, spec, &fonts, &[long]),
        Err(BakedTextPatchError::TextDoesNotFit { .. })
    ));

    let invalid_palette_profile = BakedTextProfile {
        body_ramp: &[14],
        ..profile
    };
    let invalid_palette = BakedTextReplacement {
        profile: &invalid_palette_profile,
        text: "한국어",
        ..long
    };
    assert!(matches!(
        patch_baked_text(&raw, spec, &fonts, &[invalid_palette]),
        Err(BakedTextPatchError::PaletteIndexNotDeclared { value: 14, .. })
    ));

    let valid = BakedTextReplacement {
        text: "한국어",
        profile: &profile,
        ..long
    };
    assert!(matches!(
        patch_baked_text(&raw, spec, &fonts, &[valid, valid]),
        Err(BakedTextPatchError::DuplicateBlockId(_))
    ));

    let wrong_line_count = BakedTextReplacement {
        text: "첫째\n둘째",
        ..valid
    };
    assert!(matches!(
        patch_baked_text(&raw, spec, &fonts, &[wrong_line_count]),
        Err(BakedTextPatchError::LineCountMismatch {
            expected: 1,
            actual: 2,
            ..
        })
    ));
}

#[test]
fn profile이_요구한_두_행을_각_baseline에_합성한다() {
    let (raw, raw_digest, block_digest) = fixture_with_height(32);
    let blocks = [TextBlockSpec {
        id: "BTXB-TEST",
        source: "一行目\n二行目",
        bbox: Rect {
            x: 10,
            y: 0,
            width: 180,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&TEST_INDICES),
        expected_pixels_sha256: &block_digest,
    }];
    let spec = TextAssetSpec {
        archive_paths: &TEST_ARCHIVES,
        name: "TEST.BTX",
        storage: TextStorageSpec::RasterBlocks,
        layout: Layout {
            width: 200,
            height: 32,
            format: PixelFormat::Indexed4,
        },
        expected_decompressed_sha256: &raw_digest,
        blocks: &blocks,
    };
    let fonts = fonts();
    let profile = BakedTextProfile {
        line_count: 2,
        ..profile()
    };
    let (patched, report) = patch_baked_text(
        &raw,
        spec,
        &fonts,
        &[BakedTextReplacement {
            block_id: "BTXB-TEST",
            text: "첫째 줄\n둘째 줄",
            profile: &profile,
        }],
    )
    .unwrap();

    assert_eq!(patched.len(), raw.len());
    assert_eq!(report.replacement_blocks, 1);
    let pixels = decode(&patched, spec.layout).unwrap();
    assert!(pixels[..200 * 16].iter().any(|value| *value != 0));
    assert!(pixels[200 * 16..].iter().any(|value| *value != 0));
}

#[test]
fn category별_font와_effect_profile을_독립적으로_선택한다() {
    let (raw, raw_digest, block_digest) = fixture("日本語");
    let blocks = [TextBlockSpec {
        id: "BTXB-TEST",
        source: "日本語",
        bbox: Rect {
            x: 10,
            y: 0,
            width: 180,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&TEST_INDICES),
        expected_pixels_sha256: &block_digest,
    }];
    let spec = TextAssetSpec {
        archive_paths: &TEST_ARCHIVES,
        name: "TEST.BTX",
        storage: TextStorageSpec::RasterBlocks,
        layout: Layout {
            width: 200,
            height: 16,
            format: PixelFormat::Indexed4,
        },
        expected_decompressed_sha256: &raw_digest,
        blocks: &blocks,
    };
    let fonts = fonts();
    let body = BakedTextProfile {
        font_size: 10.0,
        baseline: 12,
        ..profile()
    };
    let shadow = BakedTextProfile {
        id: "test-shadow",
        effect: BakedTextEffect::Shadow {
            offset_x: 1,
            offset_y: 1,
            palette_index: 13,
        },
        ..body
    };
    let (body_output, _) = patch_baked_text(
        &raw,
        spec,
        &fonts,
        &[BakedTextReplacement {
            block_id: "BTXB-TEST",
            text: "한국어",
            profile: &body,
        }],
    )
    .unwrap();
    let (shadow_output, _) = patch_baked_text(
        &raw,
        spec,
        &fonts,
        &[BakedTextReplacement {
            block_id: "BTXB-TEST",
            text: "한국어",
            profile: &shadow,
        }],
    )
    .unwrap();
    assert_ne!(body_output, shadow_output);

    let unknown_font = BakedTextProfile {
        id: "test-other-font",
        font_id: "future-font",
        ..body
    };
    assert!(matches!(
        patch_baked_text(
            &raw,
            spec,
            &fonts,
            &[BakedTextReplacement {
                block_id: "BTXB-TEST",
                text: "한국어",
                profile: &unknown_font,
            }]
        ),
        Err(BakedTextPatchError::UnknownFont { font_id, .. }) if font_id == "future-font"
    ));
}
