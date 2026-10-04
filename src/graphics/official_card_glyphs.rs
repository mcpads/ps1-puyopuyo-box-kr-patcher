//! 공식전 MCRD01의 104개 문자 셀만 교체하는 개발 글꼴 후보.
//! SEQ와 함께 검증한 뒤 최종 Expected Write 경계에서만 삽입한다.
use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::btx::{decode, encode, Layout, PixelFormat};
use crate::font::bundled_maplestory_light;

const LAYOUT: Layout = Layout {
    width: 256,
    height: 128,
    format: PixelFormat::Indexed4,
};
const SOURCE_SHA256: &str = "2f4972734790c639a67e7c17255b6c83e5658524a71b4c8ec2ec81483abb675a";

fn cell_origin(cell: u16) -> Result<(usize, usize), String> {
    match cell {
        24..=111 => {
            let n = usize::from(cell - 24);
            Ok(((n % 16) * 16, (n / 16) * 16))
        }
        112..=127 => Ok((usize::from(cell - 112) * 16, 112)),
        _ => Err(format!("카드 문자 셀 범위 이탈: {cell}")),
    }
}

fn rasterize(font: &fontdue::Font, ch: char) -> Result<Vec<u8>, String> {
    if ch == ' ' {
        return Ok(vec![0; 256]);
    }
    if font.lookup_glyph_index(ch) == 0 {
        return Err(format!("카드 글꼴 문자 누락: {ch}"));
    }
    let (m, coverage) = font.rasterize(ch, 14.0);
    let x = (16 - m.width as i32) / 2;
    let y = 13 - m.ymin - m.height as i32;
    if m.width == 0
        || m.height == 0
        || x < 0
        || y < 0
        || x + m.width as i32 > 16
        || y + m.height as i32 > 16
    {
        return Err(format!("카드 16px 셀에 글자 배치 불가: {ch}"));
    }
    let mut pixels = vec![0; 256];
    for row in 0..m.height {
        for column in 0..m.width {
            let alpha = coverage[row * m.width + column];
            // 실제 CLUT 7B80의 1..13은 어두운 회색부터 밝은 회색이다.
            if alpha > 0 {
                pixels[(y as usize + row) * 16 + x as usize + column] =
                    1 + ((u16::from(alpha) * 12) / 255) as u8;
            }
        }
    }
    if pixels.iter().all(|&v| v == 0) {
        return Err(format!("카드 빈 글자 래스터: {ch}"));
    }
    Ok(pixels)
}

/// 원본을 검증하고 등록한 문자 셀만 합성한다. 숫자·배지·비문자 영역은 원본 그대로다.
pub fn render_official_card_glyphs(
    original: &[u8],
    mapping: &BTreeMap<char, u16>,
) -> Result<Vec<u8>, String> {
    if original.len() != 16384 || format!("{:x}", Sha256::digest(original)) != SOURCE_SHA256 {
        return Err("공식전 MCRD01 원본 identity 불일치".into());
    }
    let font = bundled_maplestory_light().map_err(|e| e.to_string())?;
    let mut cells = BTreeSet::new();
    let mut prepared = Vec::new();
    for (&ch, &cell) in mapping {
        let (x, y) = cell_origin(cell)?;
        if !cells.insert(cell) {
            return Err("카드 글리프 셀 중복".into());
        }
        prepared.push((x, y, rasterize(&font, ch)?));
    }
    let mut pixels = decode(original, LAYOUT).map_err(|e| e.to_string())?;
    for (x, y, glyph) in prepared {
        for row in 0..16 {
            pixels[(y + row) * 256 + x..(y + row) * 256 + x + 16]
                .copy_from_slice(&glyph[row * 16..row * 16 + 16]);
        }
    }
    encode(&pixels, LAYOUT).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "official_card_glyphs_tests.rs"]
mod tests;
