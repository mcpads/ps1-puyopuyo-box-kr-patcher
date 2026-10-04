//! 진행 지도의 코스 이름과 시작 표기만 교체한다. 얼굴·영문 glyph·테두리는 보호한다.
use super::general_raster_development_surface::GeneralRasterDevelopmentSurfaceReport;
use super::text_catalog::{PaletteIndexSpec, Rect, TextAssetSpec, TextBlockSpec, TextStorageSpec};
use super::text_patch::{patch_baked_text, BakedTextReplacement};
use super::text_profile::{
    BakedFontBook, BakedTextEffect, BakedTextProfile, HorizontalAlign, MAPLESTORY_LIGHT_FONT_ID,
};
use crate::btx::{Layout, PixelFormat};

const fn block(
    id: &'static str,
    source: &'static str,
    bbox: Rect,
    indices: &'static [u8],
    digest: &'static str,
) -> TextBlockSpec<'static> {
    TextBlockSpec {
        id,
        source,
        bbox,
        palette_indices: PaletteIndexSpec::Values(indices),
        expected_pixels_sha256: digest,
    }
}
const BLOCKS: [TextBlockSpec<'static>; 5] = [
    block(
        "course-map-hot",
        "辛口コース",
        Rect {
            x: 266,
            y: 6,
            width: 100,
            height: 19,
        },
        &[2, 3, 4, 5, 6, 7, 8, 9],
        "be4f9bd9ab37dfe610b89d66f3e56cd09cbbca2896ed46c9ba3517638a340f26",
    ),
    block(
        "course-map-medium",
        "中辛コース",
        Rect {
            x: 266,
            y: 38,
            width: 100,
            height: 19,
        },
        &[2, 3, 4, 5, 6, 7, 8, 9],
        "7008dd17469813e9c0b5be54304408c61d24500b33ef7522a3e970f8a3ad4223",
    ),
    block(
        "course-map-mild",
        "甘口コース",
        Rect {
            x: 266,
            y: 70,
            width: 100,
            height: 19,
        },
        &[2, 3, 4, 5, 6, 7, 8, 9],
        "8492f892285bace8a1fe93817d3165f4441d44f3404148a9577788d16d017f98",
    ),
    block(
        "course-map-start-hiragana",
        "すたーと",
        Rect {
            x: 256,
            y: 96,
            width: 80,
            height: 40,
        },
        &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        "ea67f60f711d2faaee3dc14b60e78ae7d6068082ff7ec28321acef96ec1ec1a9",
    ),
    block(
        "course-map-start-katakana",
        "スタート",
        Rect {
            x: 256,
            y: 136,
            width: 80,
            height: 40,
        },
        &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        "9e69a3a411d8aece6f87dddaf3250b7cc54ba676e64ea0c7b600ce1cd1e139ee",
    ),
];
pub(crate) const COURSE_MAP_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &["/GOCHA1/TUU_GRP.VDD;1"],
    name: "G_FACE.BTX",
    layout: Layout {
        width: 376,
        height: 216,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "989304bfcb7f1ef2e9c01aaffc320e675356ef44728ad1e5a5db8e4da8695b4b",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &BLOCKS,
};
const COURSE_PROFILE: BakedTextProfile<'static> = BakedTextProfile {
    id: "course-map-label",
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 14.0,
    baseline: 15,
    line_count: 1,
    line_height: 19,
    tracking: 0,
    align: HorizontalAlign::Center,
    background: 2,
    body_ramp: &[4, 6, 8, 9],
    effect: BakedTextEffect::None,
};
const START_PROFILE: BakedTextProfile<'static> = BakedTextProfile {
    id: "course-map-start",
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 30.0,
    baseline: 31,
    line_count: 1,
    line_height: 40,
    tracking: 0,
    align: HorizontalAlign::Center,
    background: 0,
    body_ramp: &[5, 8, 10],
    effect: BakedTextEffect::Outline { palette_index: 1 },
};
pub(crate) fn patch_course_map_surface(
    original: &[u8],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    let input = include_str!("../../assets/translations/subgame-help/course-map.tsv");
    let mut lines = input.lines();
    if lines.next() != Some("source\tko") {
        return Err("진행 지도 번역 header 불일치".into());
    }
    let mut replacements = Vec::new();
    for (index, block) in BLOCKS.iter().enumerate() {
        let (source, ko) = lines
            .next()
            .and_then(|line| line.split_once('\t'))
            .ok_or("진행 지도 번역 누락")?;
        if source != block.source {
            return Err("진행 지도 번역 원문 순서 불일치".into());
        }
        replacements.push(BakedTextReplacement {
            block_id: block.id,
            text: ko,
            profile: if index < 3 {
                &COURSE_PROFILE
            } else {
                &START_PROFILE
            },
        });
    }
    if lines.next().is_some() {
        return Err("진행 지도 번역 초과".into());
    }
    let fonts = BakedFontBook::with_bundled_maplestory_light().map_err(|e| e.to_string())?;
    let (bytes, report) = patch_baked_text(original, COURSE_MAP_ASSET, &fonts, &replacements)
        .map_err(|e| e.to_string())?;
    Ok((
        bytes,
        GeneralRasterDevelopmentSurfaceReport {
            asset: COURSE_MAP_ASSET.name,
            replacement_blocks: report.replacement_blocks,
            changed_pixels: report.changed_pixels,
            changed_bytes: report.changed_bytes,
            output_sha256: report.output_sha256,
            translation_approved: false,
            profile_approved: false,
            runtime_verified: false,
            product_eligible: false,
        },
    ))
}
