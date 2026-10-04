//! classic의 실제 소비가 확인된 별도 atlas와 생성 로고를 합성한다.
use super::general_raster_development_surface::GeneralRasterDevelopmentSurfaceReport;
use super::text_catalog::{
    verify_text_asset, PaletteIndexSpec, Rect, TextAssetSpec, TextBlockSpec, TextStorageSpec,
    BATAN2_TEXT_ASSET,
};
use crate::btx::{decode, encode, Layout, PixelFormat};
use sha2::{Digest, Sha256};
const CONTINUE_BLOCKS: [TextBlockSpec; 1] = [TextBlockSpec {
    id: "continue",
    source: "コンティニューするかい？",
    bbox: Rect {
        x: 0,
        y: 0,
        width: 192,
        height: 16,
    },
    palette_indices: PaletteIndexSpec::Values(&[0, 1, 15]),
    expected_pixels_sha256: "ca45abf247c98d90d11b7dd46771c7c4cbfec37b15a1a28b9261d398b8c46352",
}];
const RANKING_CAPTIONS: [TextBlockSpec; 3] = [
    TextBlockSpec {
        id: "ranking-solo",
        source: "ひとりで",
        bbox: Rect {
            x: 0,
            y: 48,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 15]),
        expected_pixels_sha256: "a763bd32cb2b6da1b3978b9026ea7fa22eeef2bf4df860abc04039623751c67f",
    },
    TextBlockSpec {
        id: "ranking-endless",
        source: "とことん",
        bbox: Rect {
            x: 64,
            y: 48,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 15]),
        expected_pixels_sha256: "043fa0866ab7c42fac246b955bb7c99e2fd30b33491809c90cbf8a6ae62c17f5",
    },
    TextBlockSpec {
        id: "ranking-puyo",
        source: "ぷよぷよ",
        bbox: Rect {
            x: 128,
            y: 48,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 15]),
        expected_pixels_sha256: "9546fa32ff8e16f4ed7c62da9294118f3d35e7d3e34ac0c781751546076394c2",
    },
];
pub(crate) const RANKING_ASSET: TextAssetSpec = TextAssetSpec {
    archive_paths: &["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"],
    name: "MENU02.BTX",
    layout: Layout {
        width: 256,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "6770ed9329d693aa50ed033369bda8826a6a875ebb7ab00cb1b8c882c49150e6",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &RANKING_CAPTIONS,
};
const fn difficulty_block(
    id: &'static str,
    source: &'static str,
    x: usize,
    y: usize,
    hash: &'static str,
) -> TextBlockSpec<'static> {
    TextBlockSpec {
        id,
        source,
        bbox: Rect {
            x,
            y,
            width: 40,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 7]),
        expected_pixels_sha256: hash,
    }
}
const DIFFICULTY_BLOCKS: [TextBlockSpec; 5] = [
    difficulty_block(
        "classic-level-0",
        "激甘",
        40,
        32,
        "98ce67d04f4bfeb7464219abf0e407b8f625f9038f7ef4f2055375a0e93b9a90",
    ),
    difficulty_block(
        "classic-level-1",
        "甘口",
        80,
        32,
        "33f1729238c01807305422718dd4c51914070a3ced742b1e9a0f0f2b2eb61e0f",
    ),
    difficulty_block(
        "classic-level-2",
        "中辛",
        120,
        32,
        "250ff599adc299a3346e30365d8bb96aceea2c62c34265c85633d7df30f17aa4",
    ),
    difficulty_block(
        "classic-level-3",
        "辛口",
        96,
        48,
        "f7e20e7bed649a49eb2d636af62965370eff1623073cfa15a760072fffe0d4a0",
    ),
    difficulty_block(
        "classic-level-4",
        "激辛",
        136,
        48,
        "728f33cadc4c552f33b03fa96e70a41f54b8073fb2691e8bd3280e45ac1affe1",
    ),
];
pub(crate) const DIFFICULTY_ASSET: TextAssetSpec = TextAssetSpec {
    archive_paths: &["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"],
    name: "P_LVSEL1.BTX",
    layout: Layout {
        width: 256,
        height: 94,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "3cc36907c79ab3c37bc2cccde06ba7cb1f9b1c21e4584f7e767a7ab93f5b687c",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &DIFFICULTY_BLOCKS,
};
const OPTION_BLOCKS: [TextBlockSpec; 3] = [
    TextBlockSpec {
        id: "rule-change",
        source: "RULE HENKA",
        bbox: Rect {
            x: 0,
            y: 112,
            width: 80,
            height: 8,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 15]),
        expected_pixels_sha256: "8df5242808eedfbb8da1a57d0d149afc65e152cf5fda8b793abed3c75fb08351",
    },
    TextBlockSpec {
        id: "offset",
        source: "SOUSAI",
        bbox: Rect {
            x: 88,
            y: 112,
            width: 48,
            height: 8,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 15]),
        expected_pixels_sha256: "a948fe2ea9539dbd5ae1ec65cd4e8a06811166d0e2193a2b353bbdb54e90011d",
    },
    TextBlockSpec {
        id: "chain-limit",
        source: "RENSA SIBARI",
        bbox: Rect {
            x: 144,
            y: 112,
            width: 96,
            height: 8,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 15]),
        expected_pixels_sha256: "5b39d9b47a7be631a4bfd5472fd5bdaa69feed3bf00635b218feea7a02d57584",
    },
];
const LOGO_BLOCKS: [TextBlockSpec; 1] = [TextBlockSpec {
    id: "puyo1-logo",
    source: "ぷよぷよ",
    bbox: Rect {
        x: 0,
        y: 0,
        width: 256,
        height: 80,
    },
    palette_indices: PaletteIndexSpec::Values(&[
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
    ]),
    expected_pixels_sha256: "9ae592396bc2fa8ee449698bdd22009109f2928479b31ec1d8e1556dac26f82e",
}];
const OPPONENT_BLOCKS: [TextBlockSpec; 2] = [
    TextBlockSpec {
        id: "opponent-top",
        source: "相手を",
        bbox: Rect {
            x: 136,
            y: 64,
            width: 80,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 4, 7, 8, 13]),
        expected_pixels_sha256: "03b819f9f527e1b67915981b551831a872f327944a33fee9f2eb55c6e12e4aef",
    },
    TextBlockSpec {
        id: "opponent-bottom",
        source: "えらんでね",
        bbox: Rect {
            x: 136,
            y: 80,
            width: 80,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 4, 7, 8, 13]),
        expected_pixels_sha256: "1fdeff3d0570ede5c74971273d97ebceae50d1a5bad18bdfe59eb666037afb2b",
    },
];
pub(crate) const OPPONENT_ASSET: TextAssetSpec = TextAssetSpec {
    archive_paths: &["/PUYO2/PP2_GRP.VDD;1"],
    name: "P2EMYD1.BTX",
    layout: Layout {
        width: 512,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "07838deab552b69b3745831b1fa095ffc045850bca09757b02c166a19d019016",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &OPPONENT_BLOCKS,
};
pub(crate) const CONTINUE_ASSET: TextAssetSpec = TextAssetSpec {
    archive_paths: &["/PUYO2/PP2_GRP.VDD;1"],
    name: "P2_OVER4.BTX",
    layout: Layout {
        width: 192,
        height: 16,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "7c3bb119e71979f51e87dd90c46c272509ad3d794f07374113c98728856fe756",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &CONTINUE_BLOCKS,
};
pub(crate) const OPTION_ASSET: TextAssetSpec = TextAssetSpec {
    archive_paths: &["/PUYO2/PP2_GRP.VDD;1"],
    name: "P2OPT1.BTX",
    layout: Layout {
        width: 256,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "3a7446a4c413d32bbeb1d1b8b2ab0d3da32460b44db19339a47094706162b782",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &OPTION_BLOCKS,
};
pub(crate) const LOGO_ASSET: TextAssetSpec = TextAssetSpec {
    archive_paths: &["/PUYO1/PP1_GRP.VDD;1"],
    name: "P1_OP4.BTX",
    layout: Layout {
        width: 256,
        height: 80,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "68e236a957b14406ea96f66da51225439adb390a66d374161f1d0f6113cc1d6e",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &LOGO_BLOCKS,
};
const BADGE_BLOCKS: [TextBlockSpec; 4] = [
    BATAN2_TEXT_ASSET.blocks[0],
    BATAN2_TEXT_ASSET.blocks[1],
    BATAN2_TEXT_ASSET.blocks[2],
    TextBlockSpec {
        bbox: Rect {
            x: 160,
            y: 88,
            width: 32,
            height: 16,
        },
        ..BATAN2_TEXT_ASSET.blocks[4]
    },
];
pub(crate) const BADGE_ASSETS: [(TextAssetSpec, &str); 2] = [
    (
        TextAssetSpec {
            archive_paths: &["/PUYO1/PP1_GRP.VDD;1"],
            layout: Layout {
                height: 128,
                ..BATAN2_TEXT_ASSET.layout
            },
            expected_decompressed_sha256:
                "eeaecdd75ac7ca7c8fb5cd105475ad77d84602964cca2f523c4a1c353854cc84",
            blocks: &BADGE_BLOCKS,
            ..BATAN2_TEXT_ASSET
        },
        "classic-badges-puyo1",
    ),
    (
        TextAssetSpec {
            archive_paths: &["/PUYO2/PP2_GRP.VDD;1"],
            layout: Layout {
                height: 128,
                ..BATAN2_TEXT_ASSET.layout
            },
            expected_decompressed_sha256:
                "0cca2fd700344305d74a0b387eef6539de6816128727631d2ae92bd84ebafc71",
            blocks: &BADGE_BLOCKS,
            ..BATAN2_TEXT_ASSET
        },
        "classic-badges-puyo2",
    ),
];
fn finish(
    original: &[u8],
    pixels: &[u8],
    spec: TextAssetSpec<'static>,
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    let source = decode(original, spec.layout).map_err(|e| e.to_string())?;
    for (i, (&a, &b)) in source.iter().zip(pixels).enumerate() {
        if a == b {
            continue;
        }
        let x = i % spec.layout.width;
        let y = i / spec.layout.width;
        let block = spec
            .blocks
            .iter()
            .find(|block| {
                let r = block.bbox;
                (r.x..r.x + r.width).contains(&x) && (r.y..r.y + r.height).contains(&y)
            })
            .ok_or("classic 보호 픽셀 변경")?;
        let PaletteIndexSpec::Values(allowed) = block.palette_indices else {
            return Err("classic palette 선언 누락".into());
        };
        // 원문 index 집합은 그대로 검증한다. 상대 안내의 새 외곽선만
        // 해당 CLUT에서 관측한 불투명 검정 index 1을 추가로 허용한다.
        let opponent_outline = spec.name == OPPONENT_ASSET.name && b == 1;
        if !allowed.contains(&b) && !opponent_outline {
            return Err("classic 비대상 palette index".into());
        }
    }
    let bytes = encode(pixels, spec.layout).map_err(|e| e.to_string())?;
    let report = GeneralRasterDevelopmentSurfaceReport {
        asset: spec.name,
        replacement_blocks: spec.blocks.len(),
        changed_pixels: source.iter().zip(pixels).filter(|(a, b)| a != b).count(),
        changed_bytes: original.iter().zip(&bytes).filter(|(a, b)| a != b).count(),
        output_sha256: format!("{:x}", Sha256::digest(&bytes)),
        translation_approved: false,
        profile_approved: false,
        runtime_verified: false,
        product_eligible: false,
    };
    Ok((bytes, report))
}
pub(crate) fn patch_badges(
    original: &[u8],
    spec: TextAssetSpec<'static>,
    translated: &[u8],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    verify_text_asset(original, spec).map_err(|e| e.to_string())?;
    let mut pixels = decode(original, spec.layout).map_err(|e| e.to_string())?;
    let translated = decode(translated, BATAN2_TEXT_ASSET.layout).map_err(|e| e.to_string())?;
    for (dst, src) in spec.blocks.iter().zip([
        BATAN2_TEXT_ASSET.blocks[0],
        BATAN2_TEXT_ASSET.blocks[1],
        BATAN2_TEXT_ASSET.blocks[2],
        BATAN2_TEXT_ASSET.blocks[4],
    ]) {
        let d = dst.bbox;
        let s = src.bbox;
        for y in 0..d.height {
            for x in 0..d.width {
                pixels[(d.y + y) * spec.layout.width + d.x + x] =
                    translated[(s.y + y) * 256 + s.x + x];
            }
        }
    }
    finish(original, &pixels, spec)
}

/// 동일한 8px 높이 옵션은 연쇄에서 검증한 Dalmoori로, 16px 질문은 선택한 Galmuri로 조판한다.
fn paint_label(
    pixels: &mut [u8],
    width: usize,
    rect: Rect,
    text: &str,
    font: &fontdue::Font,
    size: f32,
    baseline: i32,
) -> Result<(), String> {
    for y in rect.y..rect.y + rect.height {
        pixels[y * width + rect.x..y * width + rect.x + rect.width].fill(0);
    }
    let mut cursor = 0i32;
    for ch in text.chars() {
        if font.lookup_glyph_index(ch) == 0 {
            return Err(format!("classic 글리프 누락: {ch}"));
        }
        let (m, b) = font.rasterize(ch, size);
        let top = baseline - m.ymin - m.height as i32;
        for y in 0..m.height {
            for x in 0..m.width {
                if b[y * m.width + x] < 128 {
                    continue;
                }
                let px = cursor + m.xmin + x as i32;
                let py = top + y as i32;
                if px < 0 || py < 0 || px >= rect.width as i32 || py >= rect.height as i32 {
                    return Err(format!("classic 문구 범위 초과: {text}"));
                }
                pixels[(rect.y + py as usize) * width + rect.x + px as usize] = 15;
            }
        }
        cursor += m.advance_width.round() as i32;
    }
    Ok(())
}
/// 슈퍼뿌요2 상대 안내처럼 주아체 15px를 16px 칸에 배치한다.
/// PS1의 확정된 영역 밖 글리프는 자르지 않고 거부한다.
fn paint_opponent_cells(
    pixels: &mut [u8],
    width: usize,
    rect: Rect,
    text: &str,
    font: &fontdue::Font,
) -> Result<(), String> {
    let ascent = font
        .horizontal_line_metrics(15.0)
        .ok_or("주아체 수직 metrics 없음")?
        .ascent as i32;
    let baseline = (16 + ascent) / 2;
    for (column, ch) in text.chars().enumerate() {
        if font.lookup_glyph_index(ch) == 0 {
            return Err(format!("상대 선택 글리프 누락: {ch}"));
        }
        let (m, bitmap) = font.rasterize(ch, 15.0);
        let left = (16 - m.width as i32) / 2;
        let top = baseline - m.ymin - m.height as i32;
        if m.width == 0
            || m.height == 0
            || left < 0
            || top < 0
            || left + m.width as i32 > 16
            || top + m.height as i32 > rect.height as i32
            || (column + 1) * 16 > rect.width
        {
            return Err(format!("상대 선택 16px 칸 초과 또는 빈 글리프: {ch}"));
        }
        for y in 0..m.height {
            for x in 0..m.width {
                if bitmap[y * m.width + x] >= 128 {
                    pixels[(rect.y + top as usize + y) * width
                        + rect.x
                        + column * 16
                        + left as usize
                        + x] = 15;
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn patch_labels(
    original: &[u8],
    spec: TextAssetSpec<'static>,
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    verify_text_asset(original, spec).map_err(|e| e.to_string())?;
    let mut pixels = decode(original, spec.layout).map_err(|e| e.to_string())?;
    let small = spec.name == "P2OPT1.BTX";
    let opponent = spec.name == OPPONENT_ASSET.name;
    let ranking = spec.name == RANKING_ASSET.name;
    let difficulty = spec.name == DIFFICULTY_ASSET.name;
    let size = if small {
        8.0
    } else if opponent {
        15.0
    } else {
        12.0
    };
    let font = if small {
        super::common_chain_surface::bundled_dalmoori()?
    } else if difficulty {
        crate::font::selected::load(crate::font::selected::GALMURI11_BOLD)?
    } else if opponent {
        let bytes = include_bytes!("../../assets/fonts/bmjua.ttf");
        if format!("{:x}", Sha256::digest(bytes))
            != "e8e6aa8b1b662c7bf0d7f136f29e822e0985176458a6e5d0ba08afc4a5c901a9"
        {
            return Err("상대 선택 주아체 원천 불일치".into());
        }
        fontdue::Font::from_bytes(bytes.as_slice(), fontdue::FontSettings::default())
            .map_err(str::to_owned)?
    } else {
        crate::font::selected::load(crate::font::selected::GALMURI11)?
    };
    let input = include_str!("../../assets/translations/btx/classic/labels.tsv");
    if input.lines().next() != Some("id\tsource\tko") {
        return Err("classic 문구 header 불일치".into());
    }
    for block in spec.blocks {
        let rows = input
            .lines()
            .skip(1)
            .filter_map(|line| {
                let f = line.split('\t').collect::<Vec<_>>();
                (f.len() == 3 && f[0] == block.id).then_some(f)
            })
            .collect::<Vec<_>>();
        if rows.len() != 1 || rows[0][1] != block.source {
            return Err("classic 문구 identity 불일치".into());
        }
        let mut rect = block.bbox;
        if ranking || difficulty {
            let advance: usize = rows[0][2]
                .chars()
                .map(|c| font.metrics(c, size).advance_width.round() as usize)
                .sum();
            let inset = if difficulty {
                rect.width
                    .checked_sub(advance)
                    .ok_or("난이도 문구 폭 초과")?
                    / 2
            } else if block.id == "ranking-puyo" {
                2
            } else {
                rect.width
                    .checked_sub(advance + 3)
                    .ok_or("랭킹 문구 폭 초과")?
            };
            for y in rect.y..rect.y + rect.height {
                pixels[y * spec.layout.width + rect.x..y * spec.layout.width + rect.x + rect.width]
                    .fill(0);
            }
            rect.x += inset;
            rect.width -= inset;
        }
        if opponent {
            let advance = 16 * rows[0][2].chars().count();
            if advance > rect.width || advance == 0 {
                return Err("상대 선택 문구 폭 초과 또는 빈 문구".into());
            }
            // 원문의 두 줄 중심과 오른쪽 아래 shadow 여백을 보존한다.
            let inset = (rect.width - advance) / 2;
            for y in rect.y..rect.y + rect.height {
                pixels[y * spec.layout.width + rect.x..y * spec.layout.width + rect.x + rect.width]
                    .fill(0);
            }
            rect.x += inset;
            rect.width -= inset;
        }
        if opponent {
            paint_opponent_cells(&mut pixels, spec.layout.width, rect, rows[0][2], &font)?;
        } else {
            paint_label(
                &mut pixels,
                spec.layout.width,
                rect,
                rows[0][2],
                &font,
                size,
                if small { 7 } else { 13 },
            )?;
        }
        if difficulty {
            // P_LVSEL1의 실제 CLUT 0x7b07에서 원문 본문은 index 7이다.
            for y in block.bbox.y..block.bbox.y + block.bbox.height {
                for x in block.bbox.x..block.bbox.x + block.bbox.width {
                    let pixel = &mut pixels[y * spec.layout.width + x];
                    if *pixel == 15 {
                        *pixel = 7;
                    }
                }
            }
        }
        if opponent || ranking {
            let r = block.bbox;
            let mask = (0..r.height)
                .flat_map(|y| {
                    pixels[(r.y + y) * spec.layout.width + r.x
                        ..(r.y + y) * spec.layout.width + r.x + r.width]
                        .to_vec()
                })
                .collect::<Vec<_>>();
            for y in 0..r.height {
                for x in 0..r.width {
                    let ink = mask[y * r.width + x] == 15;
                    let above = y > 0 && mask[(y - 1) * r.width + x] == 15;
                    let shadow = x > 0 && y > 0 && mask[(y - 1) * r.width + x - 1] == 15;
                    let outline = (y.saturating_sub(1)..=(y + 1).min(r.height - 1)).any(|ny| {
                        (x.saturating_sub(1)..=(x + 1).min(r.width - 1))
                            .any(|nx| mask[ny * r.width + nx] == 15)
                    });
                    pixels[(r.y + y) * spec.layout.width + r.x + x] = if ink {
                        if above {
                            if ranking {
                                5
                            } else {
                                8
                            }
                        } else {
                            if ranking {
                                15
                            } else {
                                13
                            }
                        }
                    } else if shadow {
                        if ranking {
                            2
                        } else {
                            7
                        }
                    } else if outline {
                        1
                    } else {
                        0
                    };
                }
            }
        }
    }
    finish(original, &pixels, spec)
}
fn verify_logo_visible_pixels(pixels: &[u8]) -> Result<(), String> {
    // P1_OP4 저장 높이(80)와 실제 scanline SPRT 영역(255×56)을 구분한다.
    if pixels.len() != 256 * 80
        || pixels
            .iter()
            .enumerate()
            .any(|(i, &pixel)| pixel != 0 && (i % 256 >= 255 || i / 256 >= 56))
    {
        return Err("PUYO1 생성 로고가 실제 표시 영역 255×56을 벗어남".into());
    }
    Ok(())
}

pub(crate) fn patch_logo(
    original: &[u8],
) -> Result<(Vec<u8>, GeneralRasterDevelopmentSurfaceReport), String> {
    verify_text_asset(original, LOGO_ASSET).map_err(|e| e.to_string())?;
    let input = include_str!("../../assets/translations/btx/classic/puyo1-logo.hex").trim_end();
    if input.len() != 256 * 80 * 2 || !input.is_ascii() {
        return Err("classic 생성 로고 크기 불일치".into());
    }
    let pixels = input
        .as_bytes()
        .chunks_exact(2)
        .map(|p| {
            u8::from_str_radix(std::str::from_utf8(p).map_err(|e| e.to_string())?, 16)
                .map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if format!("{:x}", Sha256::digest(&pixels))
        != "573dd7669bc75032164720ac12bd6a7508a09644bde697070757b8e0d8434de2"
    {
        return Err("classic 생성 로고 digest 불일치".into());
    }
    verify_logo_visible_pixels(&pixels)?;
    finish(original, &pixels, LOGO_ASSET)
}
#[cfg(test)]
#[path = "classic_graphics_surface_tests.rs"]
mod tests;
