//! COM_FONT의 연쇄 접미사만 교체한다. 숫자·다른 glyph·cell metadata는 보존한다.
use super::general_raster_development_surface::GeneralRasterDevelopmentSurfaceReport;
use super::text_catalog::verify_text_asset;
use super::text_catalog::{PaletteIndexSpec, Rect, TextAssetSpec, TextBlockSpec, TextStorageSpec};
use crate::btx::{decode, encode, Layout, PixelFormat};
use crate::font::BOX_COMMON_FONT_SPECS;
use sha2::{Digest, Sha256};

const fn block(
    x: usize,
    width: usize,
    indices: &'static [u8],
    digest: &'static str,
) -> TextBlockSpec<'static> {
    TextBlockSpec {
        id: "chain-label",
        source: "れんさ",
        bbox: Rect {
            x,
            y: 0,
            width,
            height: 8,
        },
        palette_indices: PaletteIndexSpec::Values(indices),
        expected_pixels_sha256: digest,
    }
}
const WIDE_DIGEST: &str = "2e1d7817a19c10816f62af1dc7e4f55bfaf058c7c0e81e412d3029c14777fa5b";
const SHORT_BLOCK: [TextBlockSpec<'static>; 1] = [block(176, 24, &[0, 2, 6], WIDE_DIGEST)];
const TALL_BLOCK: [TextBlockSpec<'static>; 1] = [block(216, 24, &[0, 2, 6], WIDE_DIGEST)];
const SMALL_BLOCK: [TextBlockSpec<'static>; 1] = [block(
    152,
    16,
    &[0, 1, 2, 3, 4, 5, 6],
    "436644feb964c47fcb7d14e5d7b9f06c5353fffc5557e0a6e052e4277add03dc",
)];
const fn asset(
    index: usize,
    paths: &'static [&'static str],
    blocks: &'static [TextBlockSpec<'static>],
) -> TextAssetSpec<'static> {
    let source = BOX_COMMON_FONT_SPECS[index];
    TextAssetSpec {
        archive_paths: paths,
        name: "COM_FONT.BTX",
        layout: Layout {
            width: source.width,
            height: source.height,
            format: PixelFormat::Indexed4,
        },
        expected_decompressed_sha256: source.plain_sha256,
        storage: TextStorageSpec::RasterBlocks,
        blocks,
    }
}
/// 같은 파일명이어도 원본 내용이 다른 atlas는 별도 합성한다. 공식전은 제외한다.
pub(crate) const CHAIN_ASSETS: [(TextAssetSpec<'static>, &str); 6] = [
    (
        asset(0, &["/GOCHA1/TUU_GRP.VDD;1"], &SHORT_BLOCK),
        "chain-gocha1",
    ),
    (
        asset(
            1,
            &["/GOCHA2/GOTY_GRP.VDD;1", "/HORI2/HORI2GRP.VDD;1"],
            &SHORT_BLOCK,
        ),
        "chain-gocha2-hori2",
    ),
    (
        asset(
            3,
            &["/HORI4/HORI4GRP.VDD;1", "/MINPUYO/MIN_GRP.VDD;1"],
            &SMALL_BLOCK,
        ),
        "chain-hori4-minpuyo",
    ),
    (
        asset(6, &["/PUYO1/PP1_GRP.VDD;1"], &TALL_BLOCK),
        "chain-puyo1",
    ),
    (
        asset(7, &["/PUYO2/PP2_GRP.VDD;1"], &TALL_BLOCK),
        "chain-puyo2",
    ),
    (
        asset(8, &["/QUEST/RPG_GRP.VDD;1"], &TALL_BLOCK),
        "chain-quest",
    ),
];
const TRANSLATION: &str = include_str!("../../assets/translations/subgame-help/chain-label.txt");
const DALMOORI_BYTES: &[u8] = include_bytes!("../../assets/fonts/dalmoori.ttf");
const DALMOORI_SHA256: &str = "ba40b8f9ada002005d1f5f3676a82b7bff07ddbd186a923073101099131999e6";

pub(crate) fn bundled_dalmoori() -> Result<fontdue::Font, String> {
    load_dalmoori(DALMOORI_BYTES)
}

fn load_dalmoori(bytes: &[u8]) -> Result<fontdue::Font, String> {
    if bytes.len() != 1_468_304 || format!("{:x}", Sha256::digest(bytes)) != DALMOORI_SHA256 {
        return Err("Dalmoori 글꼴 원천 크기·SHA-256 불일치".into());
    }
    fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).map_err(|e| e.to_string())
}

/// SFC 뿌요2의 Dalmoori 8px 배치와 8×8 내부 사방 외곽선을 따른다.
/// SNES planar encoding·OAM 주소는 가져오지 않고 BOX indexed4로 합성한다.
fn render_chain_glyph(font: &fontdue::Font, character: char) -> Result<[u8; 64], String> {
    if font.lookup_glyph_index(character) == 0 {
        return Err(format!("Dalmoori 글리프 누락: {character}"));
    }
    let (metrics, raster) = font.rasterize(character, 8.0);
    let ascent = font
        .horizontal_line_metrics(8.0)
        .ok_or("Dalmoori 수직 metric 누락")?
        .ascent as i32;
    let bbox_y0 = ascent - metrics.ymin - metrics.height as i32;
    let x_offset = ((8 - metrics.width as i32) / 2 - metrics.xmin).max(0);
    let y_offset = (8 - metrics.height as i32) / 2 - bbox_y0;
    let mut body = [false; 64];
    for row in 0..metrics.height {
        for col in 0..metrics.width {
            if raster[row * metrics.width + col] < 128 {
                continue;
            }
            let x = x_offset + col as i32;
            let y = y_offset + row as i32;
            if !(0..8).contains(&x) || !(0..8).contains(&y) {
                return Err(format!("연쇄 글리프가 8×8 셀을 넘음: {character}"));
            }
            body[y as usize * 8 + x as usize] = true;
        }
    }
    if !body.contains(&true) {
        return Err(format!("연쇄 글리프가 비어 있음: {character}"));
    }
    let mut pixels = [0; 64];
    for y in 0..8i32 {
        for x in 0..8i32 {
            if !body[(y * 8 + x) as usize] {
                continue;
            }
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if (0..8).contains(&nx) && (0..8).contains(&ny) {
                        pixels[(ny * 8 + nx) as usize] = 2;
                    }
                }
            }
        }
    }
    for (pixel, ink) in pixels.iter_mut().zip(body) {
        if ink {
            *pixel = 6;
        }
    }
    Ok(pixels)
}

pub(crate) fn patch_chain_surface(
    original: &[u8],
    asset: TextAssetSpec<'static>,
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    verify_text_asset(original, asset).map_err(|e| e.to_string())?;
    let text = TRANSLATION.trim_end();
    if text.chars().count() != 2 {
        return Err("연쇄 번역은 8×8 두 셀이어야 함".into());
    }
    let font = load_dalmoori(DALMOORI_BYTES)?;
    let glyphs = text
        .chars()
        .map(|c| render_chain_glyph(&font, c))
        .collect::<Result<Vec<_>, _>>()?;
    let source = decode(original, asset.layout).map_err(|e| e.to_string())?;
    let mut pixels = source.clone();
    let rect = asset.blocks[0].bbox;
    if rect.height != 8 || ![16, 24].contains(&rect.width) {
        return Err("연쇄 셀 크기 불일치".into());
    }
    for y in 0..8 {
        let start = (rect.y + y) * asset.layout.width + rect.x;
        pixels[start..start + rect.width].fill(0);
        for (index, glyph) in glyphs.iter().enumerate() {
            pixels[start + index * 8..start + index * 8 + 8]
                .copy_from_slice(&glyph[y * 8..y * 8 + 8]);
        }
    }
    let bytes = encode(&pixels, asset.layout).map_err(|e| e.to_string())?;
    let report = GeneralRasterDevelopmentSurfaceReport {
        asset: asset.name,
        replacement_blocks: 1,
        changed_pixels: pixels.iter().zip(&source).filter(|(a, b)| a != b).count(),
        changed_bytes: bytes.iter().zip(original).filter(|(a, b)| a != b).count(),
        output_sha256: format!("{:x}", Sha256::digest(&bytes)),
        translation_approved: false,
        profile_approved: false,
        runtime_verified: false,
        product_eligible: false,
    };
    Ok((bytes, report))
}
#[cfg(test)]
#[path = "common_chain_surface_tests.rs"]
mod tests;
