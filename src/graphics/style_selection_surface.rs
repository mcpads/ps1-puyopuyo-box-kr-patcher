//! 스타일·난이도·팀 선택의 상태별 cell을 불변 원본에서 함께 조판한다.
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
const LV_MENU2_STYLE_BLOCKS: [TextBlockSpec<'static>; 9] = [
    block(
        "lv_menu2-puyo-logo",
        "ぷよ",
        Rect {
            x: 72,
            y: 80,
            width: 32,
            height: 16,
        },
        &[0, 1, 2, 3, 15],
        "bf20a2b80c90c709f80088025fd5ab6789e292292b1e015111c1531d75e7dbda",
    ),
    block(
        "lv_menu2-puyo-idle",
        "ぷよ",
        Rect {
            x: 104,
            y: 80,
            width: 48,
            height: 16,
        },
        &[0, 1, 5],
        "01742d61d89450bbafa46c49c43f7e78dcb05e98ad87ff1246c0bd594c5c3f2e",
    ),
    block(
        "lv_menu2-puyo-selected",
        "ぷよ",
        Rect {
            x: 152,
            y: 80,
            width: 48,
            height: 16,
        },
        &[0, 1, 15],
        "92bb77ae003bd91fbef4994b620e97b08bc7a211d4b18c86238e8e1f4fa8e52f",
    ),
    block(
        "lv_menu2-tsu-logo",
        "通",
        Rect {
            x: 72,
            y: 96,
            width: 32,
            height: 16,
        },
        &[0, 1, 2, 3, 4, 5, 6, 8, 15],
        "6f7cc2461a671409b8ae85f88c312ef45755741cbf0bbaca9536dcb997c9323b",
    ),
    block(
        "lv_menu2-tsu-idle",
        "通",
        Rect {
            x: 104,
            y: 96,
            width: 48,
            height: 16,
        },
        &[0, 1, 5],
        "0958140132cce919d262e40c9dc96ffde09ed426dc83e925394ffb280245cb3a",
    ),
    block(
        "lv_menu2-tsu-selected",
        "通",
        Rect {
            x: 152,
            y: 96,
            width: 48,
            height: 16,
        },
        &[0, 1, 15],
        "fa9ed6310685cc1cc5c363983f591ff8b778244976ba6980404c4af845581260",
    ),
    block(
        "lv_menu2-yon-logo",
        "ぷ～ん",
        Rect {
            x: 72,
            y: 128,
            width: 32,
            height: 16,
        },
        &[0, 1, 2, 3, 4, 5, 6, 7, 9, 15],
        "c60f8356dfaaf4c8ce158bd165ee81efbb12ada9c26c0d34b80789498a46fb87",
    ),
    block(
        "lv_menu2-yon-idle",
        "ぷ～ん",
        Rect {
            x: 104,
            y: 128,
            width: 48,
            height: 16,
        },
        &[0, 1, 5],
        "f95b19723ad7a3388eaa8c30c58edb1671affbf7dc98a1b98605a6d41ed800eb",
    ),
    block(
        "lv_menu2-yon-selected",
        "ぷ～ん",
        Rect {
            x: 152,
            y: 128,
            width: 48,
            height: 16,
        },
        &[0, 1, 15],
        "4b5f76c3ecbdc9d7b1a54a1f6f6a38654dc4c9cb03eb953266dd481f2744f396",
    ),
];
const LV_MENU2_BLOCKS: [TextBlockSpec<'static>; 19] = super::level_team_blocks::join_blocks(
    &LV_MENU2_STYLE_BLOCKS,
    &super::level_team_blocks::LV_MENU2_BLOCKS,
);
pub(crate) const LV_MENU2_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &[
        "/GOCHA1/TUU_GRP.VDD;1",
        "/GOCHA2/GOTY_GRP.VDD;1",
        "/HORI2/HORI2GRP.VDD;1",
    ],
    name: "LV_MENU2.BTX",
    layout: Layout {
        width: 240,
        height: 144,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "41311386a5a4824cd3e7fc5741782bc0f7746fc901e5b787500c0defc117a00a",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &LV_MENU2_BLOCKS,
};
const LV_MENU_STYLE_BLOCKS: [TextBlockSpec<'static>; 9] = [
    block(
        "lv_menu-puyo-logo",
        "ぷよ",
        Rect {
            x: 0,
            y: 104,
            width: 32,
            height: 16,
        },
        &[0, 1, 2, 3, 15],
        "bf20a2b80c90c709f80088025fd5ab6789e292292b1e015111c1531d75e7dbda",
    ),
    block(
        "lv_menu-puyo-idle",
        "ぷよ",
        Rect {
            x: 32,
            y: 104,
            width: 40,
            height: 16,
        },
        &[0, 1, 4, 5, 13],
        "dd5a1b8d26002ae7c81e039dc596494b3dd21795843b30dfbb803aa6e8ed653f",
    ),
    block(
        "lv_menu-puyo-selected",
        "ぷよ",
        Rect {
            x: 72,
            y: 104,
            width: 40,
            height: 16,
        },
        &[0, 1, 13, 14, 15],
        "eed49ef41a228b829c7f513c2c7813389ca99a5de2b33b776ae3c522bfa02032",
    ),
    block(
        "lv_menu-tsu-logo",
        "通",
        Rect {
            x: 0,
            y: 120,
            width: 32,
            height: 16,
        },
        &[0, 1, 2, 3, 4, 5, 6, 8, 15],
        "6f7cc2461a671409b8ae85f88c312ef45755741cbf0bbaca9536dcb997c9323b",
    ),
    block(
        "lv_menu-tsu-idle",
        "通",
        Rect {
            x: 32,
            y: 120,
            width: 40,
            height: 16,
        },
        &[0, 1, 4, 5, 13],
        "819615c0510fe0090c6429e94b6abae8037b4841c446f959bbc4f409bc5579b7",
    ),
    block(
        "lv_menu-tsu-selected",
        "通",
        Rect {
            x: 72,
            y: 120,
            width: 40,
            height: 16,
        },
        &[0, 1, 13, 14, 15],
        "e79857a778937d4a7e30d60779e893ac4ba1c8acc43efe8b53dbb070273a0686",
    ),
    block(
        "lv_menu-yon-logo",
        "ぷ～ん",
        Rect {
            x: 0,
            y: 152,
            width: 32,
            height: 16,
        },
        &[0, 1, 2, 3, 4, 5, 6, 7, 9, 15],
        "c60f8356dfaaf4c8ce158bd165ee81efbb12ada9c26c0d34b80789498a46fb87",
    ),
    block(
        "lv_menu-yon-idle",
        "ぷ～ん",
        Rect {
            x: 32,
            y: 152,
            width: 40,
            height: 16,
        },
        &[0, 1, 4, 5, 13],
        "b6d4e16ff45bc807105927ee70b73001f5063be73202ad3cf515d6e7341bb741",
    ),
    block(
        "lv_menu-yon-selected",
        "ぷ～ん",
        Rect {
            x: 72,
            y: 152,
            width: 40,
            height: 16,
        },
        &[0, 1, 13, 14, 15],
        "75bbb3e2f8b276f12720530cb7340430f89d0b15329008e553d73771223436fd",
    ),
];
const LV_MENU_BLOCKS: [TextBlockSpec<'static>; 29] = super::level_team_blocks::join_blocks(
    &LV_MENU_STYLE_BLOCKS,
    &super::level_team_blocks::LV_MENU_BLOCKS,
);
pub(crate) const LV_MENU_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &["/HORI4/HORI4GRP.VDD;1", "/MINPUYO/MIN_GRP.VDD;1"],
    name: "LV_MENU.BTX",
    layout: Layout {
        width: 216,
        height: 168,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "ff85a852799fef2b91fbe30eaaf2e9f5ae4ca6bededcc6f9e45487508f428ec5",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &LV_MENU_BLOCKS,
};

pub(crate) fn patch_style_selection_surface(
    original: &[u8],
    asset: TextAssetSpec<'static>,
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    let input = match asset.name {
        "LV_MENU2.BTX" => include_str!("../../assets/translations/style-selection/lv_menu2.tsv"),
        "LV_MENU.BTX" => include_str!("../../assets/translations/style-selection/lv_menu.tsv"),
        _ => return Err("스타일 선택 자산 미등록".into()),
    };
    let fonts = BakedFontBook::with_selected_fonts().map_err(|e| e.to_string())?;
    let profiles = asset
        .blocks
        .iter()
        .map(|block| selection_profile(*block))
        .collect::<Vec<_>>();
    let mut lines = input.lines();
    if lines.next() != Some("block_id\tsource\tko") {
        return Err("스타일 번역 header 불일치".into());
    }
    let mut replacements = Vec::new();
    for (block, profile) in asset.blocks.iter().zip(&profiles) {
        let row = lines
            .next()
            .ok_or("스타일 번역 누락")?
            .split('\t')
            .collect::<Vec<_>>();
        if row.len() != 3 || row[0] != block.id || row[1] != block.source {
            return Err("스타일 번역 원본 결속 불일치".into());
        }
        if block.id.ends_with("-logo") {
            let expected = if block.id.contains("-puyo-") {
                "뿌요"
            } else if block.id.contains("-tsu-") {
                "뿌요2"
            } else {
                "뿌요4"
            };
            if row[2] != expected {
                return Err("생성 규칙 로고 문구 불일치".into());
            }
            continue;
        }
        replacements.push(BakedTextReplacement {
            block_id: block.id,
            text: row[2],
            profile,
        });
    }
    if lines.next().is_some() {
        return Err("스타일 번역 초과".into());
    }
    let (bytes, r) =
        patch_baked_text(original, asset, &fonts, &replacements).map_err(|e| e.to_string())?;
    let bytes = super::rule_logo_surface::compose(&bytes, asset)?;
    let source_pixels = crate::btx::decode(original, asset.layout).map_err(|e| e.to_string())?;
    let output_pixels = crate::btx::decode(&bytes, asset.layout).map_err(|e| e.to_string())?;
    let changed_pixels = source_pixels
        .iter()
        .zip(&output_pixels)
        .filter(|(a, b)| a != b)
        .count();
    let changed_bytes = original.iter().zip(&bytes).filter(|(a, b)| a != b).count();
    let output_sha256 = super::rule_logo_surface::digest(&bytes);
    Ok((
        bytes,
        GeneralRasterDevelopmentSurfaceReport {
            asset: asset.name,
            replacement_blocks: r.replacement_blocks + 3,
            changed_pixels,
            changed_bytes,
            output_sha256,
            translation_approved: false,
            profile_approved: false,
            runtime_verified: false,
            product_eligible: false,
        },
    ))
}

fn style_profile(i: usize) -> BakedTextProfile<'static> {
    BakedTextProfile {
        id: match i % 3 {
            0 => "style-logo",
            1 => "style-idle",
            _ => "style-selected",
        },
        font_id: MAPLESTORY_LIGHT_FONT_ID,
        font_size: if i.is_multiple_of(3) { 12.0 } else { 14.0 },
        baseline: 12,
        line_count: 1,
        line_height: 16,
        tracking: 0,
        align: HorizontalAlign::Center,
        background: 0,
        body_ramp: if i % 3 == 1 { &[1, 5] } else { &[1, 15] },
        effect: BakedTextEffect::Outline { palette_index: 1 },
    }
}

#[cfg(test)]
#[path = "style_selection_surface_tests.rs"]
mod tests;

fn selection_profile(block: TextBlockSpec<'static>) -> BakedTextProfile<'static> {
    if block.id.contains("-level-") || block.id.contains("-team-") {
        let team = block.id.contains("-team-");
        return BakedTextProfile {
            id: if team {
                "team-selection"
            } else {
                "level-selection"
            },
            font_id: if !team {
                crate::font::selected::GALMURI11_BOLD
            } else {
                MAPLESTORY_LIGHT_FONT_ID
            },
            font_size: 12.0,
            baseline: if team { 11 } else { 13 },
            line_count: 1,
            line_height: 16,
            tracking: 0,
            align: HorizontalAlign::Center,
            background: 0,
            body_ramp: if block.id.ends_with("-selected") {
                &[1, 9]
            } else if team {
                &[1, 15]
            } else {
                &[1, 7]
            },
            effect: if !team && block.bbox.width == 24 {
                BakedTextEffect::None
            } else {
                BakedTextEffect::Outline { palette_index: 1 }
            },
        };
    }
    style_profile(if block.id.ends_with("-logo") {
        0
    } else if block.id.ends_with("-idle") {
        1
    } else {
        2
    })
}
