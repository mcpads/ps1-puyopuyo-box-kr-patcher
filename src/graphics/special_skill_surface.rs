//! 특기 전용 SP_FONT의 12px 글리프만 교체한다. 창·화살표·미배정 cell은 보존한다.
use super::general_raster_development_surface::GeneralRasterDevelopmentSurfaceReport;
use super::special_skill_text::SkillText;
use super::text_catalog::{TextAssetSpec, TextStorageSpec};
use crate::btx::{decode, encode, Layout, PixelFormat};
use crate::font::selected;
use sha2::{Digest, Sha256};

// 이 사양은 물리 저장 경로다. 가변 글리프 할당 영역은 아래 합성기가 검증한다.
pub(crate) const SPECIAL_SKILL_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &[
        "/GOCHA1/TUU_GRP.VDD;1",
        "/GOCHA2/GOTY_GRP.VDD;1",
        "/HORI2/HORI2GRP.VDD;1",
        "/HORI4/HORI4GRP.VDD;1",
        "/MINPUYO/MIN_GRP.VDD;1",
    ],
    name: "SP_FONT.BTX",
    layout: Layout {
        width: 256,
        height: 208,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "301e518c25451ac5ceaf8e2b93375fb3756c8879262ae8f1ba5cd7289f8fe868",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &[],
};
fn glyphs(text: &SkillText) -> Result<Vec<(usize, Vec<u8>)>, String> {
    let font = selected::load(selected::GALMURI11)?;
    let mut result = Vec::new();
    for (&ch, &slot) in &text.glyphs {
        if font.lookup_glyph_index(ch) == 0 {
            return Err(format!("특기 폰트 누락: {ch}"));
        }
        let (m, bitmap) = font.rasterize(ch, 12.0);
        let x = (12 - m.width as i32) / 2;
        let y = 11 - m.ymin - m.height as i32;
        if m.width == 0
            || m.height == 0
            || x < 0
            || y < 0
            || x + m.width as i32 > 12
            || y + m.height as i32 > 12
        {
            return Err(format!(
                "특기 12px cell 초과: {ch} ({x},{y},{}×{}, ymin={})",
                m.width, m.height, m.ymin
            ));
        }
        let mut pixels = vec![0; 144];
        for row in 0..m.height {
            for col in 0..m.width {
                if bitmap[row * m.width + col] >= 128 {
                    pixels[(y as usize + row) * 12 + x as usize + col] = 15;
                }
            }
        }
        if pixels.iter().all(|v| *v == 0) {
            return Err(format!("특기 빈 glyph: {ch}"));
        }
        result.push((slot, pixels));
    }
    Ok(result)
}
fn compose(original: &[u8], text: &SkillText) -> Result<Vec<u8>, String> {
    let asset = SPECIAL_SKILL_ASSET;
    let before = decode(original, asset.layout).map_err(|e| e.to_string())?;
    let mut pixels = before.clone();
    for (slot, glyph) in glyphs(text)? {
        let x = slot % 21 * 12;
        let y = slot / 21 * 12;
        for row in 0..12 {
            let at = (y + row) * 256 + x;
            pixels[at..at + 12].copy_from_slice(&glyph[row * 12..row * 12 + 12]);
        }
    }
    for (at, (a, b)) in before.iter().zip(&pixels).enumerate() {
        let x = at % 256;
        let y = at / 256;
        if a != b
            && (x >= 252
                || y >= 144
                || !text
                    .glyphs
                    .values()
                    .any(|slot| *slot == y / 12 * 21 + x / 12))
        {
            return Err("특기 비대상 픽셀 변경".into());
        }
    }
    encode(&pixels, asset.layout).map_err(|e| e.to_string())
}
pub(crate) fn patch_special_skill_surface(
    original: &[u8],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    let asset = SPECIAL_SKILL_ASSET;
    if format!("{:x}", Sha256::digest(original)) != asset.expected_decompressed_sha256 {
        return Err("특기 SP_FONT 원본 SHA-256 불일치".into());
    }
    let text = SkillText::load()?;
    let bytes = compose(original, &text)?;
    let before = decode(original, asset.layout).map_err(|e| e.to_string())?;
    let after = decode(&bytes, asset.layout).map_err(|e| e.to_string())?;
    let report = GeneralRasterDevelopmentSurfaceReport {
        asset: asset.name,
        replacement_blocks: text.glyphs.len(),
        changed_pixels: before.iter().zip(&after).filter(|(a, b)| a != b).count(),
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
#[path = "special_skill_surface_tests.rs"]
mod tests;
