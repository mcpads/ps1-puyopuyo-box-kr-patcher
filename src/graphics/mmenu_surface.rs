//! 원본 MMENU 전체를 입력으로 생성한 메뉴 시트를 한 번에 공급한다.
//! 글자·판·프레임을 나누지 않으며 도움말 창·cursor·MAIN MENU는 원본을 보존한다.
use super::btx_raster_surface_policy::verify_generated_artwork_policy_for_asset;
use super::btx_raster_translation_manifest::general_btx_raster_translation_entries;
use super::text_catalog::{verify_text_asset, Rect, MMENU_TEXT_ASSET};
use crate::btx::{decode, encode};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
mod fragments;
mod layout;
use layout::{verify_translation, ReplicaRole, LABELS};

/// 메뉴 그림 전체를 하나의 물리 surface로 합성한다.
pub const MMENU_PHYSICAL_REGIONS: usize = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmenuSurfaceReport {
    pub labels: usize,
    pub physical_regions: usize,
    pub erased_pixels: usize,
    pub rendered_pixels: usize,
    pub changed_pixels: usize,
    pub changed_bytes: usize,
    pub replica_sources_exact: bool,
    pub main_menu_exact: bool,
    pub outside_declared_regions_exact: bool,
    pub output_sha256: String,
    pub translation_approved: bool,
    pub profile_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MmenuSurfaceError(String);
impl fmt::Display for MmenuSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl Error for MmenuSurfaceError {}
fn error(message: impl Into<String>) -> MmenuSurfaceError {
    MmenuSurfaceError(message.into())
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn extract_region(pixels: &[u8], width: usize, rect: Rect) -> Vec<u8> {
    (rect.y..rect.y + rect.height)
        .flat_map(|y| {
            pixels[y * width + rect.x..y * width + rect.x + rect.width]
                .iter()
                .copied()
        })
        .collect()
}

/// 도움말·cursor 영역과 원본 영문 제목은 번역 시트 밖이다.
fn writable(x: usize, y: usize) -> bool {
    !(32..256).contains(&x) || y >= 32
}
fn artwork_region(x: usize, y: usize) -> bool {
    writable(x, y) && (x < 512 || y < 64)
}
fn generated_atlas() -> Result<Vec<u8>, MmenuSurfaceError> {
    let hex =
        include_str!("../../assets/translations/btx/main-menu-lettering/atlas.hex").trim_end();
    if hex.len() != 640 * 256 * 2 || !hex.is_ascii() {
        return Err(error("전체 메뉴 시트 입력 크기 불일치"));
    }
    let pixels = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|p| {
            let s = std::str::from_utf8(p).map_err(|e| error(e.to_string()))?;
            u8::from_str_radix(s, 16).map_err(|e| error(e.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if digest(&pixels) != "dd9e52c92a2ccef90dfbb3bcbe26c5f9019f03be2add5a8c7367fb8d79f0a9d4" {
        return Err(error("전체 메뉴 시트 입력 digest 불일치"));
    }
    Ok(pixels)
}

pub fn patch_mmenu_development_surface(
    original: &[u8],
) -> Result<(Vec<u8>, MmenuSurfaceReport), MmenuSurfaceError> {
    verify_text_asset(original, MMENU_TEXT_ASSET).map_err(|e| error(e.to_string()))?;
    if verify_generated_artwork_policy_for_asset(MMENU_TEXT_ASSET)
        .map_err(|e| error(e.to_string()))?
        != LABELS.len()
    {
        return Err(error("첫 메뉴 생성 정책 분모 불일치"));
    }
    let entries = general_btx_raster_translation_entries()
        .map_err(|e| error(e.to_string()))?
        .into_iter()
        .filter(|e| e.asset_name == MMENU_TEXT_ASSET.name)
        .map(|e| (e.block_id.clone(), e))
        .collect::<BTreeMap<_, _>>();
    if entries.len() != LABELS.len() {
        return Err(error("첫 메뉴 번역 분모 불일치"));
    }
    let source = decode(original, MMENU_TEXT_ASSET.layout).map_err(|e| error(e.to_string()))?;
    for label in LABELS {
        if label
            .replicas
            .iter()
            .filter(|r| r.role == ReplicaRole::Base)
            .count()
            != 1
        {
            return Err(error("메뉴 기본 사본 분모 불일치"));
        }
        let block = MMENU_TEXT_ASSET
            .blocks
            .iter()
            .find(|b| b.id == label.block_id)
            .ok_or_else(|| error("메뉴 block 누락"))?;
        let entry = entries
            .get(label.block_id)
            .ok_or_else(|| error("메뉴 번역 누락"))?;
        verify_translation(label, *block, entry)?;
        for replica in label.replicas {
            if digest(&extract_region(&source, 640, replica.rect)) != replica.expected_sha256 {
                return Err(error("메뉴 원본 사본 digest 불일치"));
            }
        }
    }
    let mut atlas = generated_atlas()?;
    fragments::compose(&source, &mut atlas)?;
    let mut output = source.clone();
    for (i, value) in output.iter_mut().enumerate() {
        if artwork_region(i % 640, i / 640) {
            // 원본의 외부 투명 윤곽은 유지하고 내부 전체를 같은 생성 시트에서 읽는다.
            *value = if source[i] == 0 { 0 } else { atlas[i] };
        }
    }
    let main_menu_exact = source
        .iter()
        .zip(&output)
        .enumerate()
        .all(|(i, (a, b))| writable(i % 640, i / 640) || a == b);
    let outside_declared_regions_exact = source
        .iter()
        .zip(&output)
        .enumerate()
        .all(|(i, (a, b))| artwork_region(i % 640, i / 640) || a == b);
    if !main_menu_exact || !outside_declared_regions_exact {
        return Err(error("메뉴 보호 영역 변경"));
    }
    let changed_pixels = source.iter().zip(&output).filter(|(a, b)| a != b).count();
    if changed_pixels == 0 {
        return Err(error("메뉴 변경 없음"));
    }
    let bytes = encode(&output, MMENU_TEXT_ASSET.layout).map_err(|e| error(e.to_string()))?;
    let report = MmenuSurfaceReport {
        labels: LABELS.len(),
        physical_regions: MMENU_PHYSICAL_REGIONS,
        erased_pixels: source
            .iter()
            .zip(&output)
            .filter(|(a, b)| **a != 0 && **b == 0)
            .count(),
        rendered_pixels: output
            .iter()
            .enumerate()
            .filter(|(i, _)| artwork_region(i % 640, i / 640))
            .count(),
        changed_pixels,
        changed_bytes: original.iter().zip(&bytes).filter(|(a, b)| a != b).count(),
        replica_sources_exact: true,
        main_menu_exact,
        outside_declared_regions_exact,
        output_sha256: digest(&bytes),
        translation_approved: false,
        profile_approved: false,
        runtime_verified: false,
        product_eligible: false,
    };
    Ok((bytes, report))
}
#[cfg(test)]
#[path = "mmenu_surface_tests.rs"]
mod tests;
