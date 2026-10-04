//! 서브게임 `KANBAN.BTX` 표지판의 제품 text catalog.
//!
//! 다섯 물리 사본은 같은 해제 픽셀을 공유한다. GOCHA1 runtime consumer에서
//! 200x64 cell 네 개를 확인했으므로, 번역도 서로 겹치지 않는 cell 단위의
//! 전체 surface를 합성한다. 표지판 배경과 장식은 글꼴 glyph가 아니다.

use super::{
    Layout, PaletteIndexSpec, PixelFormat, Rect, TextAssetSpec, TextBlockSpec, TextStorageSpec,
};

const KANBAN_ARCHIVES: [&str; 5] = [
    "/GOCHA1/TUU_GRP.VDD;1",
    "/GOCHA2/GOTY_GRP.VDD;1",
    "/HORI2/HORI2GRP.VDD;1",
    "/HORI4/HORI4GRP.VDD;1",
    "/MINPUYO/MIN_GRP.VDD;1",
];

const KANBAN_TRAINING_PALETTE_INDICES: [u64; 4] = [
    0xffff_ffff_ffff_ffff,
    0xffff_ffff_ffff_ffff,
    0xebff_ffff_deff_ffff,
    0x03ff_ffff_fddf_fbff,
];
const KANBAN_DIGGING_PALETTE_INDICES: [u64; 4] = [
    0xffff_ffff_ffff_ffff,
    0xffff_ffff_ffff_ffff,
    0xffff_ffef_7fff_ffff,
    0x03ff_ffff_ffff_ffff,
];
const KANBAN_MIXED_PALETTE_INDICES: [u64; 4] = [
    0xffff_ffff_ffff_ffff,
    0xffff_ffff_ffff_ffff,
    0xffff_ffff_dfff_ffff,
    0x03ff_ffff_ffff_ffff,
];
const KANBAN_BOMB_PALETTE_INDICES: [u64; 4] = [
    0xffff_ffff_ffff_ffff,
    0xffff_ffff_ffff_ffff,
    0xfff7_ffff_ffff_ffff,
    0x03ff_ffff_ffff_ffff,
];

const fn cell(
    id: &'static str,
    source: &'static str,
    y: usize,
    palette_indices: [u64; 4],
    expected_pixels_sha256: &'static str,
) -> TextBlockSpec<'static> {
    TextBlockSpec {
        id,
        source,
        bbox: Rect {
            x: 0,
            y,
            width: 200,
            height: 64,
        },
        palette_indices: PaletteIndexSpec::Bitset256(palette_indices),
        expected_pixels_sha256,
    }
}

/// runtime cell table 순서와 같은 네 표지판 surface.
pub const KANBAN_TEXT_BLOCKS: [TextBlockSpec<'static>; 4] = [
    cell(
        "BTXB-0324",
        "とれーにんぐ",
        0,
        KANBAN_TRAINING_PALETTE_INDICES,
        "3c82e0cc2df45c7d9cb9ff8c8707d87ffb76437325e72f6ec2d1ee033565bf19",
    ),
    cell(
        "BTXB-0325",
        "ほりあてぷよぷよ",
        64,
        KANBAN_DIGGING_PALETTE_INDICES,
        "a1b8ebd76c1e3ff4b28d5e10aa53a371106ee81356194826b0c4dadf4da1da6a",
    ),
    cell(
        "BTXB-0326",
        "ごちゃまぜぷよぷよ",
        128,
        KANBAN_MIXED_PALETTE_INDICES,
        "1c5527aff2e57d72c773630e73e292ca8843c77b0d6fd4d6141a1a2f68e1e0a2",
    ),
    cell(
        "BTXB-0327",
        "ばくだんぷよぷよ通",
        192,
        KANBAN_BOMB_PALETTE_INDICES,
        "2743353bd7ca04f98b790f80e434f1dfc33d54007570f76cf660b077a0b5f378",
    ),
];

/// 서브게임 mode 선택에서 cell 단위로 소비하는 200x256 표지판 surface.
pub const KANBAN_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &KANBAN_ARCHIVES,
    name: "KANBAN.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 200,
        height: 256,
        format: PixelFormat::Indexed8,
    },
    expected_decompressed_sha256:
        "7d220251fd0db49a62f7823330a9dcc9fec9dada86d40c9338af53267bff8587",
    blocks: &KANBAN_TEXT_BLOCKS,
};
