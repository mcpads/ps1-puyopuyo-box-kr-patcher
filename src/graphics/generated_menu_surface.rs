//! 추가 메뉴의 통짜 생성 판을 exact catalog cell에 합성한다.
use sha2::{Digest, Sha256};

use super::btx_raster_surface_policy::verify_generated_artwork_policy_for_asset;
use super::btx_raster_translation_manifest::general_btx_raster_translation_entries;
use super::general_raster_development_surface::GeneralRasterDevelopmentSurfaceReport;
use super::text_catalog::{
    verify_text_asset, TextAssetSpec, KANBAN_TEXT_ASSET, MENU202_TEXT_ASSET,
};
use crate::btx::{decode, encode};

const MENU202_PIXELS: &str = include_str!("../../assets/translations/btx/menu202/pixels.tsv");
const KANBAN_PIXELS: &str = include_str!("../../assets/translations/btx/kanban/pixels.tsv");

// 슈퍼뿌요2 한글판의 BMJUA 22px·coverage 128·1px 외곽선을 따른다.
// PS1에서는 서로 붙는 96px/168px 두 cell로 한 줄을 나눠 공급한다.
fn paint_mode_prompt(output: &mut [u8], width: usize) -> Result<(), String> {
    let bytes = include_bytes!("../../assets/fonts/bmjua.ttf");
    if format!("{:x}", Sha256::digest(bytes))
        != "e8e6aa8b1b662c7bf0d7f136f29e822e0985176458a6e5d0ba08afc4a5c901a9"
    {
        return Err("모드 안내 주아체 원천 불일치".into());
    }
    let font = fontdue::Font::from_bytes(bytes.as_slice(), fontdue::FontSettings::default())
        .map_err(str::to_owned)?;
    let mut glyphs = Vec::new();
    let (mut left, mut top, mut right, mut bottom) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    let mut pen = 0.0_f32;
    for ch in "모드를 선택해 주세요".chars() {
        if ch != ' ' && font.lookup_glyph_index(ch) == 0 {
            return Err(format!("모드 안내 글리프 누락: {ch}"));
        }
        let (m, bitmap) = font.rasterize(ch, 22.0);
        let x = pen.round() as i32 + m.xmin;
        let y = -m.ymin - m.height as i32;
        if m.width > 0 && m.height > 0 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + m.width as i32);
            bottom = bottom.max(y + m.height as i32);
        }
        pen += m.advance_width;
        glyphs.push((x, y, m, bitmap));
    }
    if right - left + 2 > 264 || bottom - top + 2 > 24 {
        return Err("모드 안내 표시 범위 초과".into());
    }
    let dx = (264 - (right - left)) / 2 - left;
    let dy = (24 - (bottom - top)) / 2 - top;
    let mut mask = vec![false; 264 * 24];
    for (x, y, m, bitmap) in glyphs {
        for row in 0..m.height {
            for col in 0..m.width {
                if bitmap[row * m.width + col] >= 128 {
                    mask[(y + dy + row as i32) as usize * 264 + (x + dx + col as i32) as usize] =
                        true;
                }
            }
        }
    }
    for y in 0..24 {
        for x in 0..264 {
            let outline = (-1..=1).any(|oy| {
                (-1..=1).any(|ox| {
                    let (nx, ny) = (x as i32 + ox, y as i32 + oy);
                    (0..264).contains(&nx)
                        && (0..24).contains(&ny)
                        && mask[ny as usize * 264 + nx as usize]
                })
            });
            // 실제 메뉴 CLUT 0x7b00: 원본과 같은 분홍 본문·밝은 윗선·자주색 그림자.
            let above = y > 0 && mask[(y - 1) * 264 + x];
            let value = if mask[y * 264 + x] {
                if above {
                    10
                } else {
                    11
                }
            } else if outline {
                if y + 1 < 24 && mask[(y + 1) * 264 + x] {
                    12
                } else {
                    8
                }
            } else {
                0
            };
            let (tx, ty) = if x < 96 {
                (152 + x, 200 + y)
            } else {
                (80 + x - 96, 224 + y)
            };
            output[ty * width + tx] = value;
        }
    }
    Ok(())
}

/// 원본의 안내 두 cell과 버튼 세 종류의 세 표시 상태를 교체한다.
pub fn patch_menu202_surface(
    original: &[u8],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    patch_generated_cells(
        original,
        MENU202_TEXT_ASSET,
        MENU202_PIXELS,
        "ce1092bca6c48a4ad83db234796175f2245793f136dc4f6d341009de56bf4b32",
        &[
            "모드를",
            "선택해 주세요",
            "혼자서\n뿌요뿌요",
            "혼자서\n뿌요뿌요",
            "혼자서\n뿌요뿌요",
            "둘이서\n뿌요뿌요",
            "둘이서\n뿌요뿌요",
            "둘이서\n뿌요뿌요",
            "옵션",
            "옵션",
            "옵션",
        ],
    )
}

/// 네 나무판의 배경과 글자를 함께 교체한다.
pub fn patch_kanban_surface(
    original: &[u8],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    patch_generated_cells(
        original,
        KANBAN_TEXT_ASSET,
        KANBAN_PIXELS,
        "7248391eebc9f617aaad7b326c7cef78f6dde09bb49c97cf8a9976cfb7444b45",
        &[
            "트레이닝",
            "발굴 뿌요뿌요",
            "뒤죽박죽 뿌요뿌요",
            "폭탄 뿌요뿌요 통",
        ],
    )
}

fn patch_generated_cells(
    original: &[u8],
    asset: TextAssetSpec<'static>,
    input: &str,
    expected_sha: &str,
    words: &[&str],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    verify_text_asset(original, asset).map_err(|e| e.to_string())?;
    if verify_generated_artwork_policy_for_asset(asset).map_err(|e| e.to_string())? != words.len()
        || asset.blocks.len() != words.len()
        || format!("{:x}", Sha256::digest(input.as_bytes())) != expected_sha
    {
        return Err(format!("{} 생성 판 정책·입력 digest 불일치", asset.name));
    }
    let entries = general_btx_raster_translation_entries().map_err(|e| e.to_string())?;
    let pixels = decode(original, asset.layout).map_err(|e| e.to_string())?;
    let mut output = pixels.clone();
    let mut declared = vec![false; pixels.len()];
    let mut lines = input.lines();
    if lines.next() != Some("block_id\twidth\theight\tpixels_hex") {
        return Err("생성 판 header 불일치".into());
    }
    for (block, word) in asset.blocks.iter().zip(words) {
        let entry = entries
            .iter()
            .find(|e| e.block_id == block.id)
            .ok_or("생성 판 번역 누락")?;
        if entry.asset_name != asset.name
            || entry.asset_sha256 != asset.expected_decompressed_sha256
            || entry.ko != *word
            || entry.protected_source != block.source
        {
            return Err(format!("{} 생성 판 번역 결속 불일치", block.id));
        }
        let line = lines.next().ok_or("생성 판 누락")?;
        let fields: Vec<_> = line.split('\t').collect();
        let [id, width, height, hex] = fields.as_slice() else {
            return Err("생성 판 열 수 불일치".into());
        };
        let rect = block.bbox;
        if *id != block.id
            || width.parse::<usize>().ok() != Some(rect.width)
            || height.parse::<usize>().ok() != Some(rect.height)
            || !hex.is_ascii()
            || hex.len() != rect.width * rect.height * 2
        {
            return Err("생성 판 identity·크기 불일치".into());
        }
        for (i, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
            let value =
                u8::from_str_radix(std::str::from_utf8(pair).map_err(|e| e.to_string())?, 16)
                    .map_err(|e| e.to_string())?;
            let target = (rect.y + i / rect.width) * asset.layout.width + rect.x + i % rect.width;
            if declared[target] {
                return Err("생성 판 영역 겹침".into());
            }
            declared[target] = true;
            output[target] = value;
        }
    }
    if asset.name == "MENU202.BTX" {
        paint_mode_prompt(&mut output, asset.layout.width)?;
    }
    if lines.next().is_some()
        || output == pixels
        || pixels
            .iter()
            .zip(&output)
            .zip(&declared)
            .any(|((a, b), inside)| !inside && a != b)
    {
        return Err("생성 판 미등록 입력·보호 영역·변경 검사 실패".into());
    }
    let bytes = encode(&output, asset.layout).map_err(|e| e.to_string())?;
    let report = GeneralRasterDevelopmentSurfaceReport {
        asset: asset.name,
        replacement_blocks: words.len(),
        changed_pixels: pixels.iter().zip(&output).filter(|(a, b)| a != b).count(),
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
#[path = "generated_menu_surface_tests.rs"]
mod tests;
