//! 공식전 이름 입력의 편집 명령만 한글로 조판한다. 선택 문자·저장 표현은 건드리지 않는다.
use super::general_raster_development_surface::GeneralRasterDevelopmentSurfaceReport;
use super::text_catalog::{PaletteIndexSpec, Rect, TextAssetSpec, TextBlockSpec, TextStorageSpec};
use super::text_patch::{patch_baked_text, BakedTextReplacement};
use super::text_profile::{
    BakedFontBook, BakedTextEffect, BakedTextProfile, HorizontalAlign, MAPLESTORY_LIGHT_FONT_ID,
};
use crate::btx::{Layout, PixelFormat};
const BLOCKS: [TextBlockSpec<'static>; 5] = [
    TextBlockSpec {
        id: "back",
        source: "戻る",
        bbox: Rect {
            x: 480,
            y: 0,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]),
        expected_pixels_sha256: "0b2ed13b5dd31ae1e0966d9372c3e5a3404690c7261215ad5dddd3b4c4521789",
    },
    TextBlockSpec {
        id: "forward",
        source: "進む",
        bbox: Rect {
            x: 480,
            y: 16,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]),
        expected_pixels_sha256: "bdd8270b536b9b9d0b6bd40cfffedcc0dd66e13f4f903b27cd73a566ef752ad9",
    },
    TextBlockSpec {
        id: "insert",
        source: "挿入",
        bbox: Rect {
            x: 480,
            y: 32,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]),
        expected_pixels_sha256: "5592032b16e8a801d3d7b771b182fed9f809c15446ac2500f7aec250a8402e4b",
    },
    TextBlockSpec {
        id: "delete",
        source: "削除",
        bbox: Rect {
            x: 480,
            y: 48,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]),
        expected_pixels_sha256: "6f330c5131a882ceb062ae57bf119e00b66532ebb58311802097da41c8fe44c0",
    },
    TextBlockSpec {
        id: "finish",
        source: "終了",
        bbox: Rect {
            x: 480,
            y: 96,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]),
        expected_pixels_sha256: "917e5468ed7351efd0e5e59bb448b8635b745176f57e711943f7f07ab9d7cab9",
    },
];
pub(crate) const NAME_COMMAND_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &["/KOUSIKI/KOU_GRP.VDD;1"],
    name: "STREDIT2.BTX",
    layout: Layout {
        width: 768,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "63bf8e11a846c06f1bbe1a181d758afaf7cc0498732fd74ae18296e28c63aed4",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &BLOCKS,
};
const PROFILE: BakedTextProfile<'static> = BakedTextProfile {
    id: "official-name-command",
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 14.0,
    baseline: 13,
    line_count: 1,
    line_height: 16,
    tracking: 0,
    align: HorizontalAlign::Center,
    background: 0,
    body_ramp: &[1, 5, 9, 13],
    effect: BakedTextEffect::None,
};
/// 검토·개발 빌드용 명령 표면. 한글 이름 입력 지원을 뜻하지 않는다.
pub fn patch_name_commands(
    original: &[u8],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    let mut lines = include_str!("../../assets/translations/official-name/commands.tsv").lines();
    if lines.next() != Some("source\tko") {
        return Err("이름 명령 표 header 불일치".into());
    }
    let mut replacements = Vec::new();
    for block in &BLOCKS {
        let (source, ko) = lines
            .next()
            .and_then(|line| line.split_once('\t'))
            .ok_or("이름 명령 누락")?;
        if source != block.source {
            return Err("이름 명령 원문 순서 불일치".into());
        }
        replacements.push(BakedTextReplacement {
            block_id: block.id,
            text: ko,
            profile: &PROFILE,
        });
    }
    if lines.next().is_some() {
        return Err("이름 명령 초과".into());
    }
    let fonts = BakedFontBook::with_bundled_maplestory_light().map_err(|e| e.to_string())?;
    let (bytes, report) = patch_baked_text(original, NAME_COMMAND_ASSET, &fonts, &replacements)
        .map_err(|e| e.to_string())?;
    Ok((
        bytes,
        GeneralRasterDevelopmentSurfaceReport {
            asset: NAME_COMMAND_ASSET.name,
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
