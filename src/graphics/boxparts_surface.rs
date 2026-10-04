//! BOX 생성본의 나무판과 글자를 한 그림으로 공급한다.
use sha2::{Digest, Sha256};

use super::btx_raster_surface_policy::verify_generated_artwork_policy_for_asset;
use super::btx_raster_translation_manifest::general_btx_raster_translation_entries;
use super::text_catalog::{verify_text_asset, BOXPARTS_TEXT_ASSET};
use crate::btx::{decode, encode};

const PIXELS: &str = include_str!("../../assets/translations/btx/boxparts/pixels.tsv");
const PIXELS_SHA: &str = "a1220ae1e221aaba7bb3562394a54f773fc0db024fc14f66d9c03ffca46e7989";
const WORDS: [&str; 9] = [
    "뿌요 BOX",
    "혼자",
    "둘이",
    "다 같이",
    "랭킹",
    "혼자",
    "둘이",
    "다 같이",
    "랭킹",
];

/// 불변 원본의 9개 판만 교체한다. 다른 sprite·팔레트는 보존한다.
pub fn patch_boxparts_surface(original: &[u8]) -> Result<Vec<u8>, String> {
    verify_text_asset(original, BOXPARTS_TEXT_ASSET).map_err(|e| e.to_string())?;
    if verify_generated_artwork_policy_for_asset(BOXPARTS_TEXT_ASSET).map_err(|e| e.to_string())?
        != WORDS.len()
        || format!("{:x}", Sha256::digest(PIXELS.as_bytes())) != PIXELS_SHA
    {
        return Err("BOXPARTS 생성 판 정책·입력 digest 불일치".into());
    }
    let entries = general_btx_raster_translation_entries().map_err(|e| e.to_string())?;
    let pixels = decode(original, BOXPARTS_TEXT_ASSET.layout).map_err(|e| e.to_string())?;
    let mut output = pixels.clone();
    let mut declared = vec![false; pixels.len()];
    let mut lines = PIXELS.lines();
    if lines.next() != Some("block_id\twidth\theight\tpixels_hex") {
        return Err("BOX 생성 판 header 불일치".into());
    }
    for (block, word) in BOXPARTS_TEXT_ASSET.blocks.iter().zip(WORDS) {
        let entry = entries
            .iter()
            .find(|e| e.block_id == block.id)
            .ok_or("BOX 번역 누락")?;
        if entry.asset_name != BOXPARTS_TEXT_ASSET.name
            || entry.asset_sha256 != BOXPARTS_TEXT_ASSET.expected_decompressed_sha256
            || entry.ko != word
            || entry.protected_source != block.source
        {
            return Err(format!("{} BOX 번역 결속 불일치", block.id));
        }
        let line = lines.next().ok_or("BOX 생성 판 누락")?;
        let fields: Vec<_> = line.split('\t').collect();
        let [id, width, height, hex] = fields.as_slice() else {
            return Err("BOX 생성 판 열 수 불일치".into());
        };
        let rect = block.bbox;
        if *id != block.id
            || width.parse::<usize>().ok() != Some(rect.width)
            || height.parse::<usize>().ok() != Some(rect.height)
            || !hex.is_ascii()
            || hex.len() != rect.width * rect.height * 2
        {
            return Err("BOX 생성 판 identity·크기 불일치".into());
        }
        for (i, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
            let value =
                u8::from_str_radix(std::str::from_utf8(pair).map_err(|e| e.to_string())?, 16)
                    .map_err(|e| e.to_string())?;
            let target = (rect.y + i / rect.width) * 256 + rect.x + i % rect.width;
            if declared[target] {
                return Err("BOX 생성 판 영역 겹침".into());
            }
            declared[target] = true;
            output[target] = value;
        }
    }
    if lines.next().is_some()
        || output == pixels
        || pixels
            .iter()
            .zip(&output)
            .zip(&declared)
            .any(|((a, b), inside)| !inside && a != b)
    {
        return Err("BOX 미등록 판·보호 영역·변경 검사 실패".into());
    }
    encode(&output, BOXPARTS_TEXT_ASSET.layout).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "boxparts_surface_tests.rs"]
mod tests;
