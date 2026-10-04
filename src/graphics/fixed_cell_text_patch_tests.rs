use super::*;
use crate::btx::{encode, Layout, PixelFormat};
use crate::font::P2_INFO_PAGE_DRAFT_RASTER_PROFILE;
use crate::graphics::text_catalog::{FixedCellGridSpec, Rect, TextBlockSpec, TextStorageSpec};

const TEST_ARCHIVES: [&str; 1] = ["/TEST_GRP.VDD;1"];
const TEST_PALETTE: [u8; 3] = [0, 1, 15];
const TEST_ACTIVE_PALETTE: [u8; 2] = [1, 15];
const TEST_EMPTY_PALETTE: [u8; 1] = [0];

#[test]
fn capacity_stress는_정확히_16행_256cell이다() {
    let fonts = BakedFontBook::with_bundled_maplestory_light().unwrap();
    let stress = build_p2_info_page_capacity_stress_text(&fonts).unwrap();
    let lines = stress.text.split('\n').collect::<Vec<_>>();
    assert_eq!(lines.len(), 16);
    assert!(lines.iter().all(|line| line.chars().count() == 16));
    assert_eq!(
        lines.iter().map(|line| line.chars().count()).sum::<usize>(),
        P2_INFO_PAGE_CAPACITY_STRESS_CELLS
    );
    assert_eq!(stress.selected_hangul, P2_INFO_PAGE_CAPACITY_STRESS_CELLS);
    assert_eq!(
        stress.scanned_hangul,
        stress.skipped_hangul + stress.renderable_hangul
    );
    assert_eq!(
        stress
            .text
            .chars()
            .filter(|glyph| *glyph != '\n')
            .collect::<BTreeSet<_>>()
            .len(),
        P2_INFO_PAGE_CAPACITY_STRESS_CELLS
    );
}

#[test]
fn box전수검증은_네_물리source가_아니면_거부한다() {
    assert!(matches!(
        verify_box_p2_info_page_patch_strategy(std::iter::empty()),
        Err(FixedCellTextPatchError::SourceCount {
            expected: 4,
            actual: 0
        })
    ));
}

fn with_fixture(run: impl FnOnce(&[u8], TextAssetSpec<'_>, &BakedFontBook)) {
    let layout = Layout {
        width: 32,
        height: 32,
        format: PixelFormat::Indexed4,
    };
    let mut pixels = vec![0u8; 32 * 32];
    for column in 0..2 {
        for row in 0..16 {
            let start = row * 32 + column * 16;
            pixels[start..start + 16].fill(1);
        }
        pixels[column * 16 + column] = 15;
    }
    let raw = encode(&pixels, layout).unwrap();
    let raw_digest = format!("{:x}", Sha256::digest(&raw));
    let pixel_digest = format!("{:x}", Sha256::digest(&pixels));
    let blocks = [TextBlockSpec {
        id: "BTXB-TEST",
        source: "あい",
        bbox: Rect {
            x: 0,
            y: 0,
            width: 32,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&TEST_PALETTE),
        expected_pixels_sha256: &pixel_digest,
    }];
    let spec = TextAssetSpec {
        archive_paths: &TEST_ARCHIVES,
        name: "TEST.BTX",
        layout,
        expected_decompressed_sha256: &raw_digest,
        storage: TextStorageSpec::FixedCellGrid(FixedCellGridSpec {
            cell_width: 16,
            cell_height: 16,
            expected_active_cells: 2,
            expected_empty_cells: 2,
            expected_unique_glyphs: 2,
            active_palette_indices: PaletteIndexSpec::Values(&TEST_ACTIVE_PALETTE),
            empty_palette_indices: PaletteIndexSpec::Values(&TEST_EMPTY_PALETTE),
        }),
        blocks: &blocks,
    };
    let fonts = BakedFontBook::with_bundled_maplestory_light().unwrap();
    run(&raw, spec, &fonts);
}

#[test]
fn 명시적_행열에_한글_cell을_조립하고_나머지를_지운다() {
    with_fixture(|raw, spec, fonts| {
        let (patched, report) = patch_fixed_cell_text_page(
            raw,
            spec,
            fonts,
            "가\n나",
            P2_INFO_PAGE_DRAFT_RASTER_PROFILE,
        )
        .unwrap();
        assert_eq!(
            (report.rows, report.used_cells, report.empty_cells),
            (2, 2, 2)
        );
        assert_eq!(report.changed_cells, 3);

        let pixels = decode(&patched, spec.layout).unwrap();
        assert!(pixels[16..32].iter().all(|pixel| *pixel == 0));
        assert!((16..32).any(|row| pixels[row * 32..row * 32 + 16].contains(&15)));
    });
}

#[test]
fn 행열초과와_일본어잔존을_부분출력없이_거부한다() {
    with_fixture(|raw, spec, fonts| {
        assert!(matches!(
            patch_fixed_cell_text_page(
                raw,
                spec,
                fonts,
                "가나다",
                P2_INFO_PAGE_DRAFT_RASTER_PROFILE,
            ),
            Err(FixedCellTextPatchError::RowTooWide { .. })
        ));
        assert!(matches!(
            patch_fixed_cell_text_page(
                raw,
                spec,
                fonts,
                "가\n나\n다",
                P2_INFO_PAGE_DRAFT_RASTER_PROFILE,
            ),
            Err(FixedCellTextPatchError::TooManyRows { .. })
        ));
        assert!(matches!(
            patch_fixed_cell_text_page(raw, spec, fonts, "가あ", P2_INFO_PAGE_DRAFT_RASTER_PROFILE,),
            Err(FixedCellTextPatchError::JapaneseRemaining(_))
        ));
    });
}

#[test]
fn 일반_raster_block을_fixed_cell로_취급하지_않는다() {
    with_fixture(|raw, mut spec, fonts| {
        spec.storage = TextStorageSpec::RasterBlocks;
        assert!(matches!(
            patch_fixed_cell_text_page(raw, spec, fonts, "가", P2_INFO_PAGE_DRAFT_RASTER_PROFILE,),
            Err(FixedCellTextPatchError::NotFixedCellGrid(_))
        ));
    });
}
