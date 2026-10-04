//! 코스 완료 표기의 독립 32×16 cell 세 개를 개발 조판한다.
//! 원본 전체와 cell별 identity를 검증하고 기존 VDD Expected Write에 넘긴다.
use super::general_raster_development_surface::GeneralRasterDevelopmentSurfaceReport;
use super::text_catalog::{
    verify_text_asset, PaletteIndexSpec, Rect, TextAssetSpec, TextBlockSpec, TextStorageSpec,
};
use super::text_patch::render_baked_text_block;
use super::text_profile::{
    BakedFontBook, BakedTextEffect, BakedTextProfile, HorizontalAlign, MAPLESTORY_LIGHT_FONT_ID,
};
use crate::btx::{decode, encode, Layout, PixelFormat};
use sha2::{Digest, Sha256};

const fn cell(
    id: &'static str,
    source: &'static str,
    y: usize,
    indices: &'static [u8],
    digest: &'static str,
) -> TextBlockSpec<'static> {
    TextBlockSpec {
        id,
        source,
        bbox: Rect {
            x: 0,
            y,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(indices),
        expected_pixels_sha256: digest,
    }
}
const BLOCKS: [TextBlockSpec<'static>; 3] = [
    cell(
        "course-result-mild",
        "甘口",
        0,
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        "2c3ffd7c0a106723107e28becab329de247904755d5e4bb2069ce94c4396925b",
    ),
    cell(
        "course-result-medium",
        "中辛",
        16,
        &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        "bb017aa8073c9d047e67ca47bb995119038b343ef996e6ea13e768779529b00c",
    ),
    cell(
        "course-result-hot",
        "辛口",
        32,
        &[1, 2, 3, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        "85042f8c7b972c5ae9b55e2815a94098e3c873df650e441347f0bb6fe1379d0b",
    ),
];
pub(crate) const COURSE_RESULT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &["/GOCHA1/TUU_GRP.VDD;1"],
    name: "ED_MODE.BTX",
    layout: Layout {
        width: 32,
        height: 48,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "6f9413636eb8b84fa62e38d9cc75372b42d6b823c03e28920ee45170742fcab4",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &BLOCKS,
};

/// 작은 결과 표기에만 쓰는 profile. 원본의 불투명 배경 index 1을 보존한다.
const PROFILE: BakedTextProfile<'static> = BakedTextProfile {
    id: "course-result-compact",
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 10.0,
    baseline: 12,
    line_count: 1,
    line_height: 16,
    tracking: 0,
    align: HorizontalAlign::Center,
    background: 1,
    body_ramp: &[5, 9, 13, 15],
    effect: BakedTextEffect::None,
};

pub(crate) fn patch_course_result_surface(
    original: &[u8],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    let asset = COURSE_RESULT_ASSET;
    verify_text_asset(original, asset).map_err(|e| e.to_string())?;
    let fonts = BakedFontBook::with_bundled_maplestory_light().map_err(|e| e.to_string())?;
    let input = include_str!("../../assets/translations/subgame-help/course-result.tsv");
    let mut lines = input.lines();
    if lines.next() != Some("source\tko") {
        return Err("코스 결과 번역 header 불일치".into());
    }
    let mut output = Vec::new();
    for block in BLOCKS {
        let (source, ko) = lines
            .next()
            .and_then(|line| line.split_once('\t'))
            .ok_or("코스 결과 번역 누락")?;
        if source != block.source {
            return Err("코스 결과 원문 순서 불일치".into());
        }
        output.extend(
            render_baked_text_block(block, asset.layout.format, &fonts, ko, PROFILE)
                .map_err(|e| e.to_string())?,
        );
    }
    if lines.next().is_some() {
        return Err("코스 결과 번역 초과".into());
    }
    let before = decode(original, asset.layout).map_err(|e| e.to_string())?;
    let bytes = encode(&output, asset.layout).map_err(|e| e.to_string())?;
    let report = GeneralRasterDevelopmentSurfaceReport {
        asset: asset.name,
        replacement_blocks: BLOCKS.len(),
        changed_pixels: before.iter().zip(&output).filter(|(a, b)| a != b).count(),
        changed_bytes: original.iter().zip(&bytes).filter(|(a, b)| a != b).count(),
        output_sha256: format!("{:x}", Sha256::digest(&bytes)),
        translation_approved: false,
        profile_approved: false,
        runtime_verified: false,
        product_eligible: false,
    };
    Ok((bytes, report))
}
