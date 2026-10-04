//! 버전 관리한 생성 배지 indexed cell을 기존 BATAN2 합성에 결속한다.
use sha2::{Digest, Sha256};

use super::btx_raster_translation_manifest::general_btx_raster_translation_entries;
use super::text_catalog::{PaletteIndexSpec, BATAN2_TEXT_ASSET};
use crate::btx::{decode, encode};

const PIXELS: &str = include_str!("../../assets/translations/btx/battle-badges/pixels.tsv");
const PIXELS_SHA: &str = "73819464e6e11b8d8d394ba469e4059edb326c3c6b11e2e1d436781cea96675b";

/// 기존 다섯 cell 중 생성 배지 세 개만 바꾸고 휴식·상쇄·장식을 보존한다.
pub(super) fn compose(patched: &[u8]) -> Result<Vec<u8>, String> {
    if format!("{:x}", Sha256::digest(PIXELS.as_bytes())) != PIXELS_SHA {
        return Err("생성 배지 입력 digest 불일치".into());
    }
    let entries = general_btx_raster_translation_entries().map_err(|e| e.to_string())?;
    for (block, word) in BATAN2_TEXT_ASSET.blocks[..3]
        .iter()
        .zip(["야호!", "꽈당큐~", "싹쓸이!"])
    {
        let entry = entries
            .iter()
            .find(|e| e.block_id == block.id)
            .ok_or("생성 배지 번역 누락")?;
        if entry.ko != word
            || entry.protected_source != block.source
            || entry.asset_sha256 != BATAN2_TEXT_ASSET.expected_decompressed_sha256
        {
            return Err("생성 배지 문구·원본 결속 불일치".into());
        }
    }
    compose_cells(patched, PIXELS)
}

/// 相殺의 흰 말풍선·투명 테두리를 원본에서 복원한다. 글자는 검정으로만 쓴다.
pub(super) fn restore_offset_background(
    original: &[u8],
    patched: &[u8],
) -> Result<Vec<u8>, String> {
    let layout = BATAN2_TEXT_ASSET.layout;
    let source = decode(original, layout).map_err(|e| e.to_string())?;
    let mut pixels = decode(patched, layout).map_err(|e| e.to_string())?;
    let rect = BATAN2_TEXT_ASSET.blocks[4].bbox;
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            let i = y * layout.width + x;
            if ![0, 1, 15].contains(&pixels[i]) {
                return Err("상쇄는 흰 배경·검은 글자만 허용함".into());
            }
            if source[i] == 0 {
                if pixels[i] == 1 {
                    return Err("상쇄 글자가 원본 말풍선 밖으로 나감".into());
                }
                pixels[i] = 0;
            } else if pixels[i] == 0 {
                // 두 단계 ramp의 coverage <128은 글자 잉크가 아니다.
                // 말풍선 안의 이 중간값은 흰 바탕으로 채운다.
                pixels[i] = 15;
            }
        }
    }
    encode(&pixels, layout).map_err(|e| e.to_string())
}

fn compose_cells(patched: &[u8], input: &str) -> Result<Vec<u8>, String> {
    let mut pixels = decode(patched, BATAN2_TEXT_ASSET.layout).map_err(|e| e.to_string())?;
    let mut lines = input.lines();
    if lines.next() != Some("block_id\twidth\theight\tpixels_hex") {
        return Err("생성 배지 header 불일치".into());
    }
    for block in &BATAN2_TEXT_ASSET.blocks[..3] {
        let fields: Vec<_> = lines.next().ok_or("생성 배지 누락")?.split('\t').collect();
        let [id, width, height, hex] = fields.as_slice() else {
            return Err("생성 배지 열 수 불일치".into());
        };
        let rect = block.bbox;
        if *id != block.id
            || width.parse::<usize>().ok() != Some(rect.width)
            || height.parse::<usize>().ok() != Some(rect.height)
            || !hex.is_ascii()
            || hex.len() != rect.width * rect.height * 2
        {
            return Err("생성 배지 identity·크기 불일치".into());
        }
        let PaletteIndexSpec::Values(allowed) = block.palette_indices else {
            return Err("생성 배지 palette 형식 불일치".into());
        };
        for (i, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
            let index =
                u8::from_str_radix(std::str::from_utf8(pair).map_err(|e| e.to_string())?, 16)
                    .map_err(|e| e.to_string())?;
            if !allowed.contains(&index) {
                return Err("생성 배지 비대상 palette index".into());
            }
            pixels[(rect.y + i / rect.width) * BATAN2_TEXT_ASSET.layout.width
                + rect.x
                + i % rect.width] = index;
        }
    }
    if lines.next().is_some() {
        return Err("생성 배지 초과 입력".into());
    }
    encode(&pixels, BATAN2_TEXT_ASSET.layout).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "battle_badge_surface_tests.rs"]
mod tests;
