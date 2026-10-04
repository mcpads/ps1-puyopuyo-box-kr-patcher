//! 원본 검증을 마친 스타일 atlas에 생성 로고 세 셀을 합성한다.
use sha2::{Digest, Sha256};

use super::text_catalog::TextAssetSpec;
use crate::btx::{decode, encode};

const INPUT: &str = include_str!("../../assets/translations/style-selection/generated/pixels.tsv");

pub(super) fn compose(patched: &[u8], asset: TextAssetSpec<'_>) -> Result<Vec<u8>, String> {
    if digest(INPUT.as_bytes())
        != "2565ae711554d601358e63b5081a73afe18a195849c8fbf0cebc9f0666ef77f3"
    {
        return Err("생성 규칙 로고 입력 digest 불일치".into());
    }
    compose_cells(patched, asset, INPUT)
}

fn compose_cells(patched: &[u8], asset: TextAssetSpec<'_>, input: &str) -> Result<Vec<u8>, String> {
    let positions = match asset.name {
        "LV_MENU2.BTX" => [(72, 80), (72, 96), (72, 128)],
        "LV_MENU.BTX" => [(0, 104), (0, 120), (0, 152)],
        _ => return Err("생성 규칙 로고의 비대상 atlas".into()),
    };
    let mut pixels = decode(patched, asset.layout).map_err(|e| e.to_string())?;
    let mut lines = input.lines();
    if lines.next() != Some("rule\twidth\theight\tpixels_hex") {
        return Err("생성 규칙 로고 header 불일치".into());
    }
    for ((rule, (x, y)), block) in ["puyo", "puyo2", "puyo4"]
        .into_iter()
        .zip(positions)
        .zip(asset.blocks.iter().take(9).step_by(3))
    {
        let fields: Vec<_> = lines
            .next()
            .ok_or("생성 규칙 로고 누락")?
            .split('\t')
            .collect();
        let [name, width, height, hex] = fields.as_slice() else {
            return Err("생성 규칙 로고 열 수 불일치".into());
        };
        if *name != rule
            || *width != "32"
            || *height != "16"
            || hex.len() != 1024
            || !hex.is_ascii()
            || (
                block.bbox.x,
                block.bbox.y,
                block.bbox.width,
                block.bbox.height,
            ) != (x, y, 32, 16)
        {
            return Err("생성 규칙 로고 identity·셀 불일치".into());
        }
        for (i, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
            let index =
                u8::from_str_radix(std::str::from_utf8(pair).map_err(|e| e.to_string())?, 16)
                    .map_err(|e| e.to_string())?;
            // 원본 로고의 사용 index 집합과 새 로고에 허용하는 기존 CLUT는 다르다.
            // CLUT 자체는 쓰지 않으며 배경 분리용 임시 index 16은 반입하지 않는다.
            if index > 15 {
                return Err("생성 규칙 로고 palette 범위 초과".into());
            }
            pixels[(y + i / 32) * asset.layout.width + x + i % 32] = index;
        }
    }
    if lines.next().is_some() {
        return Err("생성 규칙 로고 초과 입력".into());
    }
    encode(&pixels, asset.layout).map_err(|e| e.to_string())
}

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
#[path = "rule_logo_surface_tests.rs"]
mod tests;
