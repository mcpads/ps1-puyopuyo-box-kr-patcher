//! 공식전의 생성 목판과 작은 안내를 하나의 불변 원본 결과로 합성한다.
use super::general_raster_development_surface::GeneralRasterDevelopmentSurfaceReport;
use super::official_menu_blocks::BLOCKS;
use super::text_catalog::{verify_text_asset, TextAssetSpec, TextStorageSpec};
use super::text_patch::render_baked_text_block;
use super::text_profile::{
    BakedFontBook, BakedTextEffect, BakedTextProfile, HorizontalAlign, MAPLESTORY_LIGHT_FONT_ID,
};
use crate::btx::{decode, encode, Layout, PixelFormat};
use sha2::{Digest, Sha256};

pub(crate) const OFFICIAL_MENU_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &["/KOUSIKI/KOU_GRP.VDD;1"],
    name: "KS_SPR.BTX",
    layout: Layout {
        width: 1280,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "5beb1b2a65528b2a617371415db07b06d4fe773595f72982b8e0e7d766da754c",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &BLOCKS,
};
const PANELS: &str = include_str!("../../assets/translations/official-menu/panels.tsv");
const MESSAGES: &str = include_str!("../../assets/translations/official-menu/messages.tsv");

fn guide_profile(lines: usize) -> BakedTextProfile<'static> {
    BakedTextProfile {
        id: "official-menu-guide",
        font_id: MAPLESTORY_LIGHT_FONT_ID,
        font_size: 14.0,
        baseline: 12,
        line_count: lines,
        line_height: 16,
        tracking: 0,
        align: HorizontalAlign::Left { padding: 0 },
        background: 0,
        body_ramp: &[1, 13],
        effect: BakedTextEffect::None,
    }
}

fn panel_pixels(input: &str) -> Result<Vec<Vec<u8>>, String> {
    let mut lines = input.lines();
    if lines.next() != Some("block_id\twidth\theight\tpixels_hex") {
        return Err("공식전 생성 판 header 불일치".into());
    }
    let mut panels = Vec::new();
    for block in &BLOCKS[..5] {
        let fields: Vec<_> = lines
            .next()
            .ok_or("공식전 생성 판 누락")?
            .split('\t')
            .collect();
        let [id, w, h, hex] = fields.as_slice() else {
            return Err("공식전 생성 판 열 수 불일치".into());
        };
        if *id != block.id
            || w.parse::<usize>().ok() != Some(block.bbox.width)
            || h.parse::<usize>().ok() != Some(block.bbox.height)
            || !hex.is_ascii()
            || hex.len() != block.bbox.width * block.bbox.height * 2
        {
            return Err("공식전 생성 판 identity·크기 불일치".into());
        }
        let pixels = hex
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let n =
                    u8::from_str_radix(std::str::from_utf8(pair).map_err(|e| e.to_string())?, 16)
                        .map_err(|e| e.to_string())?;
                if n > 15 {
                    return Err("공식전 indexed4 팔레트 범위 초과".into());
                }
                Ok(n)
            })
            .collect::<Result<Vec<_>, String>>()?;
        panels.push(pixels);
    }
    if lines.next().is_some() {
        return Err("공식전 생성 판 초과".into());
    }
    Ok(panels)
}

fn guide_pixels() -> Result<Vec<Vec<u8>>, String> {
    let fonts = BakedFontBook::with_bundled_maplestory_light().map_err(|e| e.to_string())?;
    let mut lines = MESSAGES.lines();
    if lines.next() != Some("block_id\tsource\tko") {
        return Err("공식전 안내 header 불일치".into());
    }
    let mut pixels = Vec::new();
    for block in &BLOCKS[5..] {
        let fields: Vec<_> = lines
            .next()
            .ok_or("공식전 안내 누락")?
            .split('\t')
            .collect();
        let [id, source, ko] = fields.as_slice() else {
            return Err("공식전 안내 열 수 불일치".into());
        };
        if *id != block.id || source.replace("\\n", "\n") != block.source {
            return Err("공식전 안내 원본 결속 불일치".into());
        }
        pixels.push(
            render_baked_text_block(
                *block,
                PixelFormat::Indexed4,
                &fonts,
                &ko.replace("\\n", "\n"),
                guide_profile(block.bbox.height / 16),
            )
            .map_err(|e| e.to_string())?,
        );
    }
    if lines.next().is_some() {
        return Err("공식전 안내 초과".into());
    }
    Ok(pixels)
}

pub(crate) fn patch_official_menu_surface(
    original: &[u8],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    let asset = OFFICIAL_MENU_ASSET;
    verify_text_asset(original, asset).map_err(|e| e.to_string())?;
    if format!("{:x}", Sha256::digest(PANELS.as_bytes()))
        != "a4243debb91a6640ca4b5e70ba90e7f6f6286476e2251c91afc8c1cfdc54b2c2"
    {
        return Err("공식전 생성 판 digest 불일치".into());
    }
    let mut replacements = panel_pixels(PANELS)?;
    replacements.extend(guide_pixels()?);
    let before = decode(original, asset.layout).map_err(|e| e.to_string())?;
    let mut output = before.clone();
    let mut menu_pixels = vec![false; before.len()];
    for (number, (block, pixels)) in BLOCKS.iter().zip(replacements).enumerate() {
        for (i, value) in pixels.into_iter().enumerate() {
            let target = (block.bbox.y + i / block.bbox.width) * asset.layout.width
                + block.bbox.x
                + i % block.bbox.width;
            if number < 5 && (before[target] == 0) != (value == 0) {
                return Err("공식전 판 외부 투명 경계 변경".into());
            }
            menu_pixels[target] = true;
            output[target] = value;
        }
    }
    // 같은 불변 원본에서 각 표면을 만들고 선언 영역의 겹침을 거부한다.
    let records = super::official_records_surface::render_official_records(original)?;
    let records = decode(&records, asset.layout).map_err(|e| e.to_string())?;
    for (x, y, width, height) in super::official_records_surface::RECORD_INTERIORS {
        for dy in 0..height {
            for dx in 0..width {
                let pos = (y + dy) * asset.layout.width + x + dx;
                if menu_pixels[pos] {
                    return Err("공식전 목판·전적 창 선언 영역 겹침".into());
                }
                output[pos] = records[pos];
            }
        }
    }
    // verify_text_asset가 겹침을 거부하며 선언한 셀만 순회한다. 다른 캐릭터·글리프·장식은 원본 그대로다.
    let bytes = encode(&output, asset.layout).map_err(|e| e.to_string())?;
    let report = GeneralRasterDevelopmentSurfaceReport {
        asset: asset.name,
        replacement_blocks: BLOCKS.len() + super::official_records_surface::RECORD_LABEL_COUNT,
        changed_pixels: before.iter().zip(&output).filter(|(a, b)| a != b).count(),
        changed_bytes: original.iter().zip(&bytes).filter(|(a, b)| a != b).count(),
        output_sha256: format!("{:x}", Sha256::digest(&bytes)),
        translation_approved: false,
        profile_approved: false,
        runtime_verified: false,
        product_eligible: false,
    };
    Ok((bytes, report))
}
#[cfg(test)]
#[path = "official_menu_surface_tests.rs"]
mod tests;
