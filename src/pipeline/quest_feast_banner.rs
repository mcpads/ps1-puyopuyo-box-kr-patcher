//! 연회장 MAP11의 축하 문구만 원본 팔레트 인덱스로 교체하는 개발 입력.
//! 행별 mask 밖의 장식과 검은 저장 여백은 불변이며, 최종 쓰기는 공용 CNX/VDD composer가 소유한다.
use super::cnx_development_entry::{
    prepare_cnx_development_entry, CnxDevelopmentEntrySpec, PreparedCnxDevelopmentEntry,
};
use crate::disc::{DiscImage, Iso9660, VerifiedSourceImage};
use sha2::{Digest, Sha256};

const PIXELS: &str = include_str!("../../assets/translations/btx/quest-feast-banner/pixels.tsv");
const SOURCE_SHA: &str = "5174ff55943b60b230350d8305a65ef3c69ba6e869ee8d551e72c5ed41cc2d9f";
const WIDTH: usize = 384;

/// 번역 SEQ와 합성한 최종 disc에서도 원본 색상 인덱스의 의미를 보존한다.
pub(super) fn verify_feast_palette(image: &[u8]) -> Result<(), String> {
    let image = DiscImage::from_bytes(image.to_vec()).map_err(|e| e.to_string())?;
    let iso = Iso9660::parse(&image).map_err(|e| e.to_string())?;
    let vdd = iso
        .read_file("/QUEST/RPG_SEQ.VDD;1")
        .map_err(|e| e.to_string())?;
    let archive = crate::archive::VddArchive::parse(&vdd).map_err(|e| e.to_string())?;
    let entry = archive.find("TOU.SEQ").ok_or("연회장 TOU.SEQ 누락")?;
    let plain = crate::compress::decompress(archive.read_file(entry)).map_err(|e| e.to_string())?;
    let palette = plain.get(0x38ac..0x3aac).ok_or("연회장 팔레트 범위 누락")?;
    if format!("{:x}", Sha256::digest(palette))
        != "f5982bf530f976801e17b7d09a659db95b00e44c68c91d29da08d3b676137372"
    {
        return Err("연회장 최종 팔레트 불일치".into());
    }
    Ok(())
}

fn bytes(hex: &str) -> Result<Vec<u8>, String> {
    if hex.is_empty() || !hex.len().is_multiple_of(2) || !hex.is_ascii() {
        return Err("축하 배경 pixel hex 형식 오류".into());
    }
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).map_err(|e| e.to_string())?;
            u8::from_str_radix(text, 16).map_err(|e| e.to_string())
        })
        .collect()
}

fn apply_rows(original: &[u8], rows: &str) -> Result<Vec<u8>, String> {
    if original.len() != WIDTH * 256 {
        return Err("MAP11 저장 크기 불일치".into());
    }
    let mut lines = rows.lines();
    if lines.next() != Some("y\tx\texpected_hex\treplacement_hex") {
        return Err("축하 배경 pixel header 불일치".into());
    }
    let mut output = original.to_vec();
    let mut covered = vec![false; original.len()];
    for line in lines {
        let fields: Vec<_> = line.split('\t').collect();
        let [y, x, expected, replacement] = fields.as_slice() else {
            return Err("축하 배경 pixel 열 수 불일치".into());
        };
        let y = y.parse::<usize>().map_err(|e| e.to_string())?;
        let x = x.parse::<usize>().map_err(|e| e.to_string())?;
        let expected = bytes(expected)?;
        let replacement = bytes(replacement)?;
        // 꽃 테두리와 케이크 얼굴을 피한 수동 mask. 우측 하단은 더 좁다.
        let right = if y < 34 { 196 } else { 194 };
        if !(28..53).contains(&y)
            || x < 70
            || x >= right
            || expected.len() != replacement.len()
            || expected.len() > right - x
        {
            return Err("축하 배경 보호 mask 침범 또는 길이 불일치".into());
        }
        let range = y * WIDTH + x..y * WIDTH + x + expected.len();
        if covered[range.clone()].iter().any(|v| *v) {
            return Err("축하 배경 pixel 쓰기 겹침".into());
        }
        if original[range.clone()] != expected {
            return Err("축하 배경 Expected Write 원본 불일치".into());
        }
        covered[range.clone()].fill(true);
        output[range].copy_from_slice(&replacement);
    }
    if output == original {
        return Err("축하 배경 변경 없음".into());
    }
    Ok(output)
}

pub(super) fn prepare_quest_feast_banner(
    source: &VerifiedSourceImage,
) -> Result<PreparedCnxDevelopmentEntry, String> {
    let (entry, ()) = prepare_cnx_development_entry(
        source,
        CnxDevelopmentEntrySpec {
            iso_path: "/QUEST/RPG_GRP.VDD;1",
            entry_name: "MAP11.BTX",
            expected_cnx_subtype: *b"bit",
            patch_id: "quest-feast-banner-development",
            actor: "pipeline::quest_feast_banner",
        },
        |original| {
            if format!("{:x}", Sha256::digest(original)) != SOURCE_SHA {
                return Err("MAP11 원본 SHA-256 불일치".into());
            }
            Ok((apply_rows(original, PIXELS)?, ()))
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(entry)
}

#[cfg(test)]
#[path = "quest_feast_banner_tests.rs"]
mod tests;
