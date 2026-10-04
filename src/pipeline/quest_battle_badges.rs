//! QUEST BATAN2의 승패·규칙 안내를 원본 bbox별로 합성한다.
//!
//! 공통 네 셀만 기존 번역에서 가져오고, QUEST 전용 세 셀은 별도 입력으로
//! 렌더링한다. 다른 모드의 atlas 전체나 셀 좌표를 이식하지 않는다.
use super::cnx_development_entry::{
    prepare_cnx_development_entry, CnxDevelopmentEntrySpec, PreparedCnxDevelopmentEntry,
};
use super::general_raster_development_entries::PreparedGeneralRasterDevelopmentBatch;
use crate::btx::{decode, encode, Layout};
use crate::coords::exe_map;
use crate::disc::{DiscImage, Iso9660, VerifiedSourceImage};
use crate::graphics::text_catalog::{
    verify_text_asset, PaletteIndexSpec, Rect, TextAssetSpec, TextBlockSpec, BATAN2_TEXT_ASSET,
};
use crate::graphics::text_patch::{patch_baked_text, BakedTextReplacement};
use crate::graphics::text_profile::{
    BakedFontBook, BakedTextEffect, BakedTextProfile, HorizontalAlign, MAPLESTORY_LIGHT_FONT_ID,
};
use sha2::{Digest, Sha256};

const QUEST_LAYOUT: Layout = Layout {
    height: 128,
    ..BATAN2_TEXT_ASSET.layout
};
const QUEST_SOURCE_SHA: &str = "245bcb240c1fc59bfa9177ac504bbdf3571ab2afe27897507bb6d380b5006f91";

const RULE_BLOCKS: [TextBlockSpec<'static>; 3] = [
    TextBlockSpec {
        id: "quest-normal-rule",
        source: "通常ルール",
        bbox: Rect {
            x: 80,
            y: 64,
            width: 80,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 5]),
        expected_pixels_sha256: "90fc9bf5521e450e648ac1c87e607fbca63e5818979e03c78ef59bcedbf0bff7",
    },
    TextBlockSpec {
        id: "quest-score-puyo",
        source: "得点ぷよ",
        bbox: Rect {
            x: 160,
            y: 64,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 13]),
        expected_pixels_sha256: "718d597d4b07f3d786fee5281817e2f3d6a40a68b6dbc26aaf08190e87db2675",
    },
    TextBlockSpec {
        id: "quest-hard-puyo",
        source: "かたぷよ2",
        bbox: Rect {
            x: 80,
            y: 88,
            width: 80,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 13]),
        expected_pixels_sha256: "a5db99ca8427c62e3e2756b29c149d7d56212ae6d5f45bc42dae14b6da88431c",
    },
];
const RULE_PROFILE: BakedTextProfile<'static> = BakedTextProfile {
    id: "quest-battle-rule",
    font_id: MAPLESTORY_LIGHT_FONT_ID,
    font_size: 12.0,
    baseline: 13,
    line_count: 1,
    line_height: 16,
    tracking: 0,
    align: HorizontalAlign::Center,
    background: 0,
    body_ramp: &[13],
    effect: BakedTextEffect::None,
};

fn shared_blocks() -> [TextBlockSpec<'static>; 4] {
    let mut offset = BATAN2_TEXT_ASSET.blocks[4];
    // 원본 픽셀은 같지만 QUEST의 상쇄 셀은 다른 위치에 있다.
    offset.bbox.x = 160;
    [
        BATAN2_TEXT_ASSET.blocks[0],
        BATAN2_TEXT_ASSET.blocks[1],
        BATAN2_TEXT_ASSET.blocks[2],
        offset,
    ]
}

fn compose(original: &[u8], translated: &[u8]) -> Result<Vec<u8>, String> {
    let mut blocks = shared_blocks().to_vec();
    blocks.extend(RULE_BLOCKS);
    let spec = TextAssetSpec {
        archive_paths: &["/QUEST/RPG_GRP.VDD;1"],
        layout: QUEST_LAYOUT,
        expected_decompressed_sha256: QUEST_SOURCE_SHA,
        blocks: &blocks,
        ..BATAN2_TEXT_ASSET
    };
    verify_text_asset(original, spec).map_err(|e| e.to_string())?;
    let profiles = [
        BakedTextProfile {
            body_ramp: &[5],
            ..RULE_PROFILE
        },
        RULE_PROFILE,
        RULE_PROFILE,
    ];
    let mut lines = include_str!("../../assets/translations/btx/quest_battle_status.tsv").lines();
    if lines.next() != Some("source\tko") {
        return Err("QUEST 규칙 번역 header 불일치".into());
    }
    let mut replacements = Vec::new();
    for (block, profile) in RULE_BLOCKS.iter().zip(&profiles) {
        let (source, ko) = lines
            .next()
            .and_then(|line| line.split_once('\t'))
            .ok_or("QUEST 규칙 번역 누락")?;
        if source != block.source {
            return Err("QUEST 규칙 원문 불일치".into());
        }
        replacements.push(BakedTextReplacement {
            block_id: block.id,
            text: ko,
            profile,
        });
    }
    if lines.next().is_some() {
        return Err("QUEST 규칙 번역 초과".into());
    }
    let fonts = BakedFontBook::with_bundled_maplestory_light().map_err(|e| e.to_string())?;
    let (rules, _) =
        patch_baked_text(original, spec, &fonts, &replacements).map_err(|e| e.to_string())?;
    let mut pixels = decode(&rules, QUEST_LAYOUT).map_err(|e| e.to_string())?;
    let translated = decode(translated, BATAN2_TEXT_ASSET.layout).map_err(|e| e.to_string())?;
    copy_badge_pixels(&mut pixels, &translated);
    encode(&pixels, QUEST_LAYOUT).map_err(|e| e.to_string())
}

fn copy_badge_pixels(pixels: &mut [u8], translated: &[u8]) {
    for (destination, source_index) in shared_blocks().iter().zip([0, 1, 2, 4]) {
        let dst = destination.bbox;
        let src = BATAN2_TEXT_ASSET.blocks[source_index].bbox;
        for y in 0..dst.height {
            let from = (src.y + y) * BATAN2_TEXT_ASSET.layout.width + src.x;
            let to = (dst.y + y) * QUEST_LAYOUT.width + dst.x;
            pixels[to..to + dst.width].copy_from_slice(&translated[from..from + src.width]);
        }
    }
}

pub(super) fn prepare(
    source: &VerifiedSourceImage,
    general: &PreparedGeneralRasterDevelopmentBatch,
) -> Result<PreparedCnxDevelopmentEntry, String> {
    verify_consumer(source.image())?;
    let references = general
        .entry_refs("/GOCHA1/TUU_GRP.VDD;1")
        .map_err(|e| e.to_string())?;
    let reference = references
        .into_iter()
        .find(|entry| entry.spec.entry_name == BATAN2_TEXT_ASSET.name)
        .ok_or("QUEST 배지의 기존 번역 surface 누락")?;
    prepare_cnx_development_entry(
        source,
        CnxDevelopmentEntrySpec {
            iso_path: "/QUEST/RPG_GRP.VDD;1",
            entry_name: "BATAN2.BTX",
            expected_cnx_subtype: *b"bi_",
            patch_id: "quest-battle-badges",
            actor: "pipeline::quest_battle_badges",
        },
        |original| Ok((compose(original, reference.patched_plain())?, ())),
    )
    .map(|(entry, ())| entry)
    .map_err(|e| e.to_string())
}

// QUEST 자신의 header·26개 cell record와 CLUT다. source와 최종 EXE에서 보존한다.
pub(super) fn verify_consumer(image: &DiscImage) -> Result<(), String> {
    let iso = Iso9660::parse(image).map_err(|e| e.to_string())?;
    let exe = iso
        .read_file("/QUEST/QUEST.EXE;1")
        .map_err(|e| e.to_string())?;
    let map = exe_map("QUEST.EXE").ok_or("QUEST 좌표 사상 누락")?;
    for (runtime, length, expected) in [
        (
            0x8008_4648,
            324,
            "bc41a9614fec4b803391f59e83e2e95b3ae75955156dceb515fe7b528da8b520",
        ),
        (
            0x8008_4AF0,
            32,
            "bbd89e7ffcee312b12f4434078bdc1b09bc72727e2ed2f3c8a64e9ca4e332ad5",
        ),
    ] {
        let offset = map
            .file_offset(runtime)
            .ok_or("QUEST 배지 좌표 범위 이탈")? as usize;
        let bytes = exe
            .get(offset..offset + length)
            .ok_or("QUEST 배지 consumer 누락")?;
        if format!("{:x}", Sha256::digest(bytes)) != expected {
            return Err(format!("QUEST 배지 cell·CLUT 불일치: {runtime:#010x}"));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_battle_badges_tests.rs"]
mod tests;
