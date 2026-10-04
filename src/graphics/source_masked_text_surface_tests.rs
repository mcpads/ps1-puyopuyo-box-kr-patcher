use super::*;
use crate::btx::{Layout, PixelFormat};
use crate::graphics::text_catalog::{Rect, TextBlockSpec};

const LAYOUT: Layout = Layout {
    width: 4,
    height: 2,
    format: PixelFormat::Indexed4,
};
const ARCHIVES: [&str; 1] = ["/TEST.VDD;1"];
const PALETTE: [u8; 5] = [0, 1, 2, 3, 4];
const BLOCKS: [TextBlockSpec<'static>; 1] = [TextBlockSpec {
    id: "BTXB-MASK",
    source: "日本語",
    bbox: Rect {
        x: 0,
        y: 0,
        width: 3,
        height: 2,
    },
    palette_indices: PaletteIndexSpec::Values(&PALETTE),
    expected_pixels_sha256: "067a40f2d8dae2d4a8d7eaa175a31d0d484e8eb4a3dc4e2007057ade2b17909e",
}];
const ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &ARCHIVES,
    name: "MASK.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: LAYOUT,
    expected_decompressed_sha256:
        "4635ba89f87c983819d4e28e02f767e61e5db34dd03050133feb826057c3bb87",
    blocks: &BLOCKS,
};
const ORIGINAL: [u8; 4] = [0x21, 0x54, 0x30, 0x50];
const SOURCE_TEXT: [bool; 6] = [true, true, false, false, true, false];
const PROTECTED: [bool; 8] = [false, false, true, true, false, false, false, true];
const RENDERED: [u8; 6] = [0, 0, 0, 4, 0, 0];

fn protected() -> ProtectedPixelMask<'static> {
    ProtectedPixelMask {
        pixels: &PROTECTED,
        expected_pixels: 3,
        expected_sha256: "2f99cd08d148f6c4ded50cc69665ed646c5c11fb539abbc39f0723c767800dc6",
    }
}

fn replacement(rendered_pixels: &'static [u8]) -> SourceMaskedTextReplacement<'static> {
    SourceMaskedTextReplacement {
        block_id: "BTXB-MASK",
        source_text_pixels: &SOURCE_TEXT,
        expected_source_text_pixels: 3,
        expected_source_text_mask_sha256:
            "8eb597945e99773d014aafc3fc7d1a8c540d7d1d6f7b20261ac0bb04996ed28c",
        removal: SourceTextRemoval::Fill {
            background_index: 0,
        },
        transparent_index: 0,
        rendered_pixels,
    }
}

#[test]
fn 글자mask만지우고_새조판과_보호pixel을함께합성한다() {
    let (output, report) = compose_source_masked_text_surface(
        &ORIGINAL,
        ASSET,
        protected(),
        &[replacement(&RENDERED)],
    )
    .unwrap();
    let output_pixels = decode(&output, LAYOUT).unwrap();

    assert_eq!(output_pixels, [0, 0, 4, 5, 4, 0, 0, 5]);
    assert_eq!(report.source_text_pixels, 3);
    assert_eq!(report.rendered_pixels, 1);
    assert_eq!(report.protected_pixels, 3);
    assert!(report.protected_pixels_exact);
}

#[test]
fn 새조판이_보호pixel을건드리면_실패한다() {
    const TOUCHES_PROTECTED: [u8; 6] = [0, 0, 4, 0, 0, 0];

    assert!(matches!(
        compose_source_masked_text_surface(
            &ORIGINAL,
            ASSET,
            protected(),
            &[replacement(&TOUCHES_PROTECTED)],
        ),
        Err(SourceMaskedTextSurfaceError::RenderTouchesProtectedPixel { .. })
    ));
}

#[test]
fn 원문의_비배경pixel을_분류하지않으면_실패한다() {
    const INCOMPLETE_SOURCE_TEXT: [bool; 6] = [true, false, false, false, true, false];
    let incomplete = SourceMaskedTextReplacement {
        source_text_pixels: &INCOMPLETE_SOURCE_TEXT,
        expected_source_text_pixels: 2,
        expected_source_text_mask_sha256:
            "a90d4563c6a0ae0417ab3110f1ba68592833465954774201b0a69e8c457dc6ad",
        ..replacement(&RENDERED)
    };

    assert!(matches!(
        compose_source_masked_text_surface(&ORIGINAL, ASSET, protected(), &[incomplete]),
        Err(SourceMaskedTextSurfaceError::UnclassifiedSourceDifference { .. })
    ));
}

#[test]
fn 원문mask는_단색이아닌검증된배경pixel로복원할수있다() {
    const RESTORED: [u8; 6] = [0, 4, 4, 0, 0, 0];
    const TRANSPARENT: [u8; 6] = [0; 6];
    let restored = SourceMaskedTextReplacement {
        removal: SourceTextRemoval::Restore {
            pixels: &RESTORED,
            expected_pixels_sha256:
                "45c0d30765af90746de79de4860e48dc99496f2f0891bd56f29111e5e78e7a89",
        },
        transparent_index: 0,
        rendered_pixels: &TRANSPARENT,
        ..replacement(&RENDERED)
    };

    let (output, _) =
        compose_source_masked_text_surface(&ORIGINAL, ASSET, protected(), &[restored]).unwrap();

    assert_eq!(decode(&output, LAYOUT).unwrap(), [0, 4, 4, 5, 0, 0, 0, 5]);
}

#[test]
fn 복원배경이_mask밖의원본차이를숨기면_실패한다() {
    const RESTORED: [u8; 6] = [0, 4, 4, 4, 0, 0];
    const TRANSPARENT: [u8; 6] = [0; 6];
    let restored = SourceMaskedTextReplacement {
        removal: SourceTextRemoval::Restore {
            pixels: &RESTORED,
            expected_pixels_sha256:
                "85e3970c561164b013f18765772ecbb3a39a5eb75319efee96c5be4044392d10",
        },
        transparent_index: 0,
        rendered_pixels: &TRANSPARENT,
        ..replacement(&RENDERED)
    };

    assert!(matches!(
        compose_source_masked_text_surface(&ORIGINAL, ASSET, protected(), &[restored]),
        Err(SourceMaskedTextSurfaceError::UnclassifiedSourceDifference { .. })
    ));
}
