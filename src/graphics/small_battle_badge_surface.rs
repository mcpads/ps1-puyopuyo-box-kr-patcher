//! HORI4·MINPUYO BATAM의 작은 결과·상태 셀을 기존 채택 번역에서 합성한다.
//! 생성 배지 세 장은 중심 nearest로 축소하고 같은 크기의 휴식·상쇄는 그대로 옮긴다.
use super::general_raster_development_surface::GeneralRasterDevelopmentSurfaceReport;
use super::text_catalog::{
    verify_text_asset, PaletteIndexSpec, Rect, TextAssetSpec, TextBlockSpec, BATAN2_TEXT_ASSET,
};
use crate::btx::{decode, encode, Layout, PixelFormat};
use crate::coords::exe_map;
use crate::disc::{DiscImage, Iso9660};
use sha2::{Digest, Sha256};

const BLOCKS: [TextBlockSpec<'static>; 5] = [
    TextBlockSpec {
        bbox: Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 32,
        },
        expected_pixels_sha256: "c6c7e96108c98e0f7256816d675ae4a50cf8d040614bb7ba9893949c5cdc8582",
        palette_indices: PaletteIndexSpec::Values(&[0, 4, 9, 11, 15]),
        ..BATAN2_TEXT_ASSET.blocks[0]
    },
    TextBlockSpec {
        bbox: Rect {
            x: 64,
            y: 0,
            width: 64,
            height: 40,
        },
        expected_pixels_sha256: "3a7ce9ec1858033309f308ff0c19fd34fb476edac99310661024e4c62fea3cdc",
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6]),
        ..BATAN2_TEXT_ASSET.blocks[1]
    },
    TextBlockSpec {
        bbox: Rect {
            x: 0,
            y: 48,
            width: 56,
            height: 16,
        },
        expected_pixels_sha256: "c864b2def37eeb2a48bcb1deca2f7a60987c40aaf57ef8a8f77f69ddeb20cbe6",
        palette_indices: PaletteIndexSpec::Values(&[0, 8, 9, 10, 11, 12]),
        ..BATAN2_TEXT_ASSET.blocks[2]
    },
    TextBlockSpec {
        bbox: Rect {
            x: 56,
            y: 40,
            width: 64,
            height: 24,
        },
        expected_pixels_sha256: "0572c23e78d6dca2f6773b69cc132f4f097d4b15bbfe7211f19d4ec9108da0e2",
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 3, 4, 5, 6, 7]),
        ..BATAN2_TEXT_ASSET.blocks[3]
    },
    TextBlockSpec {
        bbox: Rect {
            x: 0,
            y: 64,
            width: 32,
            height: 16,
        },
        expected_pixels_sha256: "77e952264455c3582b803c1f35c78f9cfbcb8258a5dbcb769691cb18e5b017e5",
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 15]),
        ..BATAN2_TEXT_ASSET.blocks[4]
    },
];
const fn asset(paths: &'static [&'static str], digest: &'static str) -> TextAssetSpec<'static> {
    TextAssetSpec {
        archive_paths: paths,
        name: "BATAM.BTX",
        layout: Layout {
            width: 168,
            height: 112,
            format: PixelFormat::Indexed4,
        },
        expected_decompressed_sha256: digest,
        blocks: &BLOCKS,
        ..BATAN2_TEXT_ASSET
    }
}
pub(crate) const SMALL_BADGE_ASSETS: [(TextAssetSpec<'static>, &str); 2] = [
    (
        asset(
            &["/HORI4/HORI4GRP.VDD;1"],
            "9a8e3eb101c0383fea98069337e025d7917acac0864aa63fdcb7153053ac2797",
        ),
        "small-badges-hori4",
    ),
    (
        asset(
            &["/MINPUYO/MIN_GRP.VDD;1"],
            "34217aef1e25596e8cc7de142e1195c5132ee1ca05388cce135d3db2cc6ed174",
        ),
        "small-badges-minpuyo",
    ),
];

fn compose_pixels(target: &mut [u8], translated: &[u8]) -> Result<(), String> {
    if target.len() != 168 * 112 || translated.len() != 256 * 126 {
        return Err("작은 배지 합성 surface 크기 불일치".into());
    }
    for (block, source) in BLOCKS.iter().zip(BATAN2_TEXT_ASSET.blocks) {
        let dst = block.bbox;
        let src = source.bbox;
        let PaletteIndexSpec::Values(allowed) = block.palette_indices else {
            return Err("작은 배지 palette 선언 누락".into());
        };
        for y in 0..dst.height {
            for x in 0..dst.width {
                let sx = (2 * x + 1) * src.width / (2 * dst.width);
                let sy = (2 * y + 1) * src.height / (2 * dst.height);
                let value = translated[(src.y + sy) * 256 + src.x + sx];
                if !allowed.contains(&value) {
                    return Err("작은 배지 비대상 palette index".into());
                }
                target[(dst.y + y) * 168 + dst.x + x] = value;
            }
        }
    }
    Ok(())
}

pub(crate) fn patch_small_badges(
    original: &[u8],
    spec: TextAssetSpec<'static>,
    translated: &[u8],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    verify_text_asset(original, spec).map_err(|e| e.to_string())?;
    let source = decode(original, spec.layout).map_err(|e| e.to_string())?;
    let translated = decode(translated, BATAN2_TEXT_ASSET.layout).map_err(|e| e.to_string())?;
    let mut pixels = source.clone();
    compose_pixels(&mut pixels, &translated)?;
    let bytes = encode(&pixels, spec.layout).map_err(|e| e.to_string())?;
    let report = GeneralRasterDevelopmentSurfaceReport {
        asset: spec.name,
        replacement_blocks: 5,
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

/// EXE별 원본 cell table과 실제 패배 SPRT가 참조한 CLUT를 보존한다.
pub(crate) fn verify_consumers(image: &DiscImage) -> Result<(), String> {
    let iso = Iso9660::parse(image).map_err(|e| e.to_string())?;
    for (name, path, table, len, digest, clut) in [
        (
            "MINPUYO.EXE",
            "/MINPUYO/MINPUYO.EXE;1",
            0x8005_7cf4,
            336,
            "f15403ecfd3bfed61a5b51d11329b2dc485b611ab330e00915953aac3150cc5d",
            0x8005_8b40,
        ),
        (
            "HORI4.EXE",
            "/HORI4/HORI4.EXE;1",
            0x8005_7908,
            312,
            "aff3ec0e19b01af903d49ba04c2e189334dbbbf4a745d678e18c6dec1b18097e",
            0x8005_873c,
        ),
    ] {
        let exe = iso.read_file(path).map_err(|e| e.to_string())?;
        let map = exe_map(name).ok_or("작은 배지 EXE 사상 누락")?;
        for (address, length, expected) in [
            (table, len, digest),
            (
                clut,
                32,
                "2ff16015f578e5c47aadbb5ac1b7a02b64563321668205dcc912dbdb7bba38ae",
            ),
        ] {
            let offset = map.file_offset(address).ok_or("작은 배지 좌표 범위 이탈")? as usize;
            let bytes = exe
                .get(offset..offset + length)
                .ok_or("작은 배지 consumer 범위 이탈")?;
            if format!("{:x}", Sha256::digest(bytes)) != expected {
                return Err(format!("{name} 작은 배지 cell·CLUT 불일치"));
            }
        }
    }
    Ok(())
}
#[cfg(test)]
#[path = "small_battle_badge_surface_tests.rs"]
mod tests;
