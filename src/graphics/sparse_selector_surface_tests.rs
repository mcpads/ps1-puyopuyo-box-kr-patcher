use super::*;
use crate::btx::{encode, Layout, PixelFormat};
use crate::graphics::text_catalog::{PaletteIndexSpec, SparseSelectorAtlasSpec, TextBlockSpec};

const LAYOUT: Layout = Layout {
    width: 4,
    height: 2,
    format: PixelFormat::Indexed4,
};
const ARCHIVES: [&str; 1] = ["/TEST_GRP.VDD;1"];
const ALL_INDICES: [u8; 4] = [0, 1, 2, 3];
const RIGHT_INDICES: [u8; 3] = [0, 2, 3];
const OVERLAPS: [(&str, &str); 1] = [("BTXB-SPARSE-A", "BTXB-SPARSE-B")];

fn raw() -> Vec<u8> {
    encode(&[1, 2, 3, 0, 0, 1, 2, 3], LAYOUT).unwrap()
}

fn blocks(first_source: &'static str) -> [TextBlockSpec<'static>; 2] {
    [
        TextBlockSpec {
            id: "BTXB-SPARSE-A",
            source: first_source,
            bbox: Rect {
                x: 0,
                y: 0,
                width: 3,
                height: 2,
            },
            palette_indices: PaletteIndexSpec::Values(&ALL_INDICES),
            expected_pixels_sha256:
                "a6da5671467b03d21e4fa38ff746d6ca6152b029c174fdf0635edd48497ffd24",
        },
        TextBlockSpec {
            id: "BTXB-SPARSE-B",
            source: "日本語",
            bbox: Rect {
                x: 2,
                y: 0,
                width: 2,
                height: 2,
            },
            palette_indices: PaletteIndexSpec::Values(&RIGHT_INDICES),
            expected_pixels_sha256:
                "76c7034fc9dc29496d9a621f3b54a7b1d807da2ab8705cf2c58f35ce1b3233d6",
        },
    ]
}

fn spec<'a>(blocks: &'a [TextBlockSpec<'a>]) -> TextAssetSpec<'a> {
    TextAssetSpec {
        archive_paths: &ARCHIVES,
        name: "SPARSE.BTX",
        layout: LAYOUT,
        expected_decompressed_sha256:
            "1176508b0a95ae7cb03c57c5b474a4e590db632deb853d5bc0c46adf4ab627f4",
        storage: TextStorageSpec::SparseSelectorAtlas(SparseSelectorAtlasSpec {
            allowed_overlap_pairs: &OVERLAPS,
            expected_physical_union_pixels: 8,
        }),
        blocks,
    }
}

#[test]
fn replacement순서와_무관하게_같은_전체surface를_만든다() {
    let raw = raw();
    let blocks = blocks("日本語");
    let first_pixels = [0, 2, 2, 0, 1, 2];
    let second_pixels = [2, 3, 2, 0];
    let first = SparseSelectorRegionReplacement {
        block_id: "BTXB-SPARSE-A",
        pixels: &first_pixels,
    };
    let second = SparseSelectorRegionReplacement {
        block_id: "BTXB-SPARSE-B",
        pixels: &second_pixels,
    };

    let (forward, report) =
        compose_sparse_selector_surface(&raw, spec(&blocks), &[first, second]).unwrap();
    let (reverse, _) =
        compose_sparse_selector_surface(&raw, spec(&blocks), &[second, first]).unwrap();

    assert_eq!(forward, reverse);
    assert_eq!(report.replacement_blocks, 2);
    assert_eq!(report.changed_pixels, 4);
}

#[test]
fn 겹침영역의_서로다른_pixel제안을_거부한다() {
    let raw = raw();
    let blocks = blocks("日本語");
    let first_pixels = [0, 2, 2, 0, 1, 2];
    let conflicting_pixels = [3, 3, 2, 0];

    assert!(matches!(
        compose_sparse_selector_surface(
            &raw,
            spec(&blocks),
            &[
                SparseSelectorRegionReplacement {
                    block_id: "BTXB-SPARSE-A",
                    pixels: &first_pixels,
                },
                SparseSelectorRegionReplacement {
                    block_id: "BTXB-SPARSE-B",
                    pixels: &conflicting_pixels,
                },
            ],
        ),
        Err(SparseSelectorSurfaceError::OverlapPixelConflict { .. })
    ));
}

#[test]
fn 교체하지않은_reference의_겹침pixel을_보존한다() {
    let raw = raw();
    let blocks = blocks("COM");
    let preserved_overlap = [3, 2, 2, 0];
    let changed_overlap = [2, 2, 2, 0];

    assert!(compose_sparse_selector_surface(
        &raw,
        spec(&blocks),
        &[SparseSelectorRegionReplacement {
            block_id: "BTXB-SPARSE-B",
            pixels: &preserved_overlap,
        }],
    )
    .is_ok());
    assert!(matches!(
        compose_sparse_selector_surface(
            &raw,
            spec(&blocks),
            &[SparseSelectorRegionReplacement {
                block_id: "BTXB-SPARSE-B",
                pixels: &changed_overlap,
            }],
        ),
        Err(SparseSelectorSurfaceError::OverlapPixelConflict { .. })
    ));
}
