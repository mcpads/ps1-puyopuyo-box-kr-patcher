//! 채택된 글자 형태를 참고한 생성 타이틀을 원본 surface에 한 번에 합성한다.
use super::general_raster_development_surface::GeneralRasterDevelopmentSurfaceReport;
use super::text_catalog::{verify_text_asset, TextAssetSpec};
use crate::btx::{decode, encode};
use sha2::{Digest, Sha256};

fn pixels(text: &str, count: usize, digest: &str) -> Result<Vec<u8>, String> {
    let text = text.trim_end();
    if !text.is_ascii() || text.len() != count * 2 {
        return Err("생성 타이틀 입력 크기 불일치".into());
    }
    let result = text
        .as_bytes()
        .chunks_exact(2)
        .map(|p| {
            u8::from_str_radix(std::str::from_utf8(p).map_err(|e| e.to_string())?, 16)
                .map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if format!("{:x}", Sha256::digest(&result)) != digest {
        return Err("생성 타이틀 입력 digest 불일치".into());
    }
    Ok(result)
}

fn writable(name: &str, x: usize, y: usize) -> bool {
    match name {
        // 첫 로고 끝과 B 왼쪽에 겹친 원문을 함께 복원한다. B 나머지는 보호한다.
        "TITLEA.BTX" => {
            (x < 168 && (24..88).contains(&y))
                || ((168..176).contains(&x) && (24..88).contains(&y))
                || ((176..181).contains(&x) && (51..88).contains(&y))
                || (x < 176 && (96..160).contains(&y))
        }
        "P2_OP13.BTX" => (x < 112 && y < 48) || (x < 88 && (48..120).contains(&y)),
        _ => false,
    }
}

pub(crate) fn patch(
    original: &[u8],
    asset: TextAssetSpec<'static>,
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    verify_text_asset(original, asset).map_err(|e| e.to_string())?;
    let source = decode(original, asset.layout).map_err(|e| e.to_string())?;
    let mut output = source.clone();
    match asset.name {
        "TITLEA.BTX" => {
            let border = pixels(
                include_str!("../../assets/translations/btx/classic/box-b-border.hex"),
                8 * 24,
                "407e60cd1e151d855fd2bcbe584e305ed49c1215dfb844cd35a16b3537476047",
            )?;
            let large = pixels(
                include_str!("../../assets/translations/btx/classic/box-title-168.hex"),
                168 * 64,
                "a8f470ec6e2e155227862e63f99a97c612cb343dc5e365f2aaa5732fed5c9b97",
            )?;
            let small = pixels(
                include_str!("../../assets/translations/btx/classic/box-title-176.hex"),
                176 * 64,
                "c7b70c783d812b205c7277bd4d8a3a66a8eadcf93bec21c20b28907dd6d958f2",
            )?;
            for y in 24..88 {
                output[y * 384..y * 384 + 168]
                    .copy_from_slice(&large[(y - 24) * 168..(y - 23) * 168]);
            }
            for y in 48..88 {
                output[y * 384 + 168..y * 384 + 176].fill(0);
            }
            // B 아래 원문 외곽선만 제거한다. x=181부터 O의 테두리다.
            for y in 51..88 {
                output[y * 384 + 176..y * 384 + 181].fill(0);
            }
            for y in 24..48 {
                output[y * 384 + 168..y * 384 + 176]
                    .copy_from_slice(&border[(y - 24) * 8..(y - 23) * 8]);
            }
            for y in 96..160 {
                output[y * 384..y * 384 + 176]
                    .copy_from_slice(&small[(y - 96) * 176..(y - 95) * 176]);
            }
        }
        "P2_OP13.BTX" => {
            let letters = pixels(
                include_str!("../../assets/translations/btx/classic/puyo2-title.hex"),
                112 * 48,
                "0b7580323bb169706184e74317ea1dc6e4e087e74721f3af7ea8325d14f80b8b",
            )?;
            if letters.iter().any(|&v| v > 15) {
                return Err("뿌요2 타이틀 palette 범위 초과".into());
            }
            for y in 0..48 {
                output[y * 240..y * 240 + 112].copy_from_slice(&letters[y * 112..(y + 1) * 112]);
            }
            // 한국 명칭은 뿌요뿌요2. 숫자 원판은 별도 원본 자산으로 유지한다.
            for y in 48..120 {
                output[y * 240..y * 240 + 88].fill(0);
            }
        }
        _ => return Err("미등록 생성 타이틀".into()),
    }
    for (i, (a, b)) in source.iter().zip(&output).enumerate() {
        if a != b && !writable(asset.name, i % asset.layout.width, i / asset.layout.width) {
            return Err("생성 타이틀 보호 영역 변경".into());
        }
    }
    let bytes = encode(&output, asset.layout).map_err(|e| e.to_string())?;
    let report = GeneralRasterDevelopmentSurfaceReport {
        asset: asset.name,
        replacement_blocks: if asset.name == "TITLEA.BTX" { 2 } else { 3 },
        changed_pixels: source.iter().zip(&output).filter(|(a, b)| a != b).count(),
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
#[path = "generated_title_surface_tests.rs"]
mod tests;
