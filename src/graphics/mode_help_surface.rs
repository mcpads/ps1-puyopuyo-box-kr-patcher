//! MENU201 글자 atlas와 MENU2 설명 stream을 같은 문자 사전에 결속한다.
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const INPUT: &str = include_str!("../../assets/translations/btx/classic/mode-help.tsv");

fn rows() -> Result<Vec<(usize, usize, &'static str)>, String> {
    let mut lines = INPUT.lines();
    if lines.next() != Some("offset\tslots\tko") {
        return Err("모드 설명 header 불일치".into());
    }
    let mut result = Vec::new();
    let mut end = 0;
    for line in lines {
        let f: Vec<_> = line.split('\t').collect();
        if f.len() != 3 {
            return Err("모드 설명 열 수 불일치".into());
        }
        let offset = usize::from_str_radix(f[0], 16).map_err(|e| e.to_string())?;
        let slots: usize = f[1].parse::<usize>().map_err(|e| e.to_string())?;
        if offset < end || slots == 0 || f[2].is_empty() || f[2].chars().count() > slots {
            return Err("모드 설명 중복·길이 불일치".into());
        }
        end = offset + slots * 2;
        result.push((offset, slots, f[2]));
    }
    if result.len() != 48 {
        return Err("모드 설명 48행 누락".into());
    }
    Ok(result)
}

fn glyphs() -> Result<Vec<char>, String> {
    let set: BTreeSet<_> = rows()?
        .iter()
        .flat_map(|r| r.2.chars())
        .filter(|&c| c != ' ')
        .collect();
    if set.len() > 100 {
        return Err("모드 설명 glyph slot 초과".into());
    }
    Ok(set.into_iter().collect())
}

pub(crate) fn patch_sequence(original: &[u8], puyo1: bool) -> Result<Vec<u8>, String> {
    let (size, hash, delta) = if puyo1 {
        (
            29_740,
            "f3c486a71d07364332ec33ec202d4b76edfe3a316b5dc83909aee95e0282c460",
            28,
        )
    } else {
        (
            29_712,
            "aa8f79acbf380762205218efad9af926f8403d3f6abf8b193ecf96f8be79e529",
            0,
        )
    };
    if original.len() != size || format!("{:x}", Sha256::digest(original)) != hash {
        return Err("MENU2 설명 원본 크기·SHA-256 불일치".into());
    }
    let glyphs = glyphs()?;
    let mut output = original.to_vec();
    for (offset, slots, text) in rows()? {
        let offset = offset + delta;
        if original[offset..offset + slots * 2]
            .chunks_exact(2)
            .any(|b| u16::from_le_bytes([b[0], b[1]]) >= 90)
        {
            return Err("모드 설명 glyph 대신 제어 명령 발견".into());
        }
        let mut chars = text.chars();
        for i in 0..slots {
            let cell = match chars.next() {
                None | Some(' ') => 0,
                Some(c) => {
                    glyphs
                        .binary_search(&c)
                        .map_err(|_| "모드 설명 매핑 누락")? as u16
                        + 1
                }
            };
            output[offset + i * 2..offset + i * 2 + 2].copy_from_slice(&cell.to_le_bytes());
        }
    }
    // 원래 빈 90..100 cell 중 필요한 칸만 오른쪽 빈 atlas 공간에 등록한다.
    for cell in 90..=glyphs.len() {
        let offset = 0x4c50 + delta + 12 + cell * 12;
        if original[offset..offset + 12] != [0; 12] {
            return Err("모드 설명 여분 cell 원본 불일치".into());
        }
        let record = [
            256 + ((cell - 1) % 15) * 16,
            ((cell - 1) / 15) * 16,
            16,
            16,
            0,
            0,
        ];
        for (i, value) in record.into_iter().enumerate() {
            output[offset + i * 2..offset + i * 2 + 2]
                .copy_from_slice(&(value as u16).to_le_bytes());
        }
    }
    Ok(output)
}

/// 원본 검증과 일반 label 합성을 마친 한 surface 안에서 glyph를 합성한다.
pub(crate) fn compose(atlas: &[u8]) -> Result<Vec<u8>, String> {
    let layout = super::text_catalog::MENU201_TEXT_ASSET.layout;
    let mut pixels = crate::btx::decode(atlas, layout).map_err(|e| e.to_string())?;
    let font = crate::font::selected::load(crate::font::selected::GALMURI11)?;
    let mut profile = crate::font::MENU06_DRAFT_RASTER_PROFILE;
    profile.font_id = crate::font::selected::GALMURI11;
    profile.font_size = 12.0;
    for (i, glyph) in glyphs()?.into_iter().enumerate() {
        if font.lookup_glyph_index(glyph) == 0 {
            return Err(format!("모드 설명 font 누락: {glyph}"));
        }
        let cell = crate::font::rasterize_menu06_glyph(&font, glyph, profile)
            .map_err(|e| e.to_string())?;
        let (x, y) = (256 + (i % 15) * 16, (i / 15) * 16);
        for row in 0..16 {
            pixels[(y + row) * 512 + x..(y + row) * 512 + x + 16]
                .copy_from_slice(&cell[row * 16..row * 16 + 16]);
        }
    }
    crate::btx::encode(&pixels, layout).map_err(|e| e.to_string())
}
