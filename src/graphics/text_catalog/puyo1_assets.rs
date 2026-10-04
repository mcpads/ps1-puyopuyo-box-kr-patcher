//! PUYO1 복각 계열의 정적 BTX text asset 계약.

use super::{
    Layout, PaletteIndexSpec, PixelFormat, Rect, TextAssetSpec, TextBlockSpec, TextStorageSpec,
};

const MENU01_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
const MENU01_TITLE_PALETTE_INDICES: [u8; 4] = [0, 6, 7, 14];
const MENU01_LABEL_PALETTE_INDICES: [u8; 2] = [0, 7];
const MENU01_LOGO_PALETTE_INDICES: [u8; 14] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 15];
const MENU01_PANEL_PALETTE_INDICES: [u8; 2] = [1, 15];
const MENU01_STAGE_PALETTE_INDICES: [u8; 3] = [0, 1, 7];
const MENU01_ON_OFF_PALETTE_INDICES: [u8; 2] = [0, 15];
const MENU01_BLOCKS: [TextBlockSpec<'static>; 18] = [
    TextBlockSpec {
        id: "BTXB-0174",
        source: "ぷよぷよ",
        bbox: Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_TITLE_PALETTE_INDICES),
        expected_pixels_sha256: "24009edecda86662efdcad8f5cad6dc404feba99c6316d4d84541252434e1004",
    },
    TextBlockSpec {
        id: "BTXB-0175",
        source: "おぷしょん",
        bbox: Rect {
            x: 64,
            y: 0,
            width: 80,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_TITLE_PALETTE_INDICES),
        expected_pixels_sha256: "72efcf90ac7fac5f9e31720ce1eb949e214b12c8fb09b57f0d378747a0dd9ece",
    },
    TextBlockSpec {
        id: "BTXB-0176",
        source: "とことん",
        bbox: Rect {
            x: 144,
            y: 0,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_TITLE_PALETTE_INDICES),
        expected_pixels_sha256: "e330121c4bf68fbe3720181d6cd5cbbcd18eae6a2ae7934869848ed33e018525",
    },
    TextBlockSpec {
        id: "BTXB-0177",
        source: "モード",
        bbox: Rect {
            x: 0,
            y: 16,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_LABEL_PALETTE_INDICES),
        expected_pixels_sha256: "86ba5c4843d2af4c57c46eed41f95bd1087511b96855a0de9b5061d99b96b46d",
    },
    TextBlockSpec {
        id: "BTXB-0178",
        source: "レベル",
        bbox: Rect {
            x: 48,
            y: 16,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_LABEL_PALETTE_INDICES),
        expected_pixels_sha256: "56ce6385ffc280e0bd57400b01bd434ca346898000dcdf93f6f66c5d1534cd72",
    },
    TextBlockSpec {
        id: "BTXB-0179",
        source: "を選択して下さい。",
        bbox: Rect {
            x: 96,
            y: 16,
            width: 144,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_LABEL_PALETTE_INDICES),
        expected_pixels_sha256: "4dea4bec9384be1f4e63f91d26d6546c9e1a5629c5924108d231e3fb09f6b6cf",
    },
    TextBlockSpec {
        id: "BTXB-0180",
        source: "やさしい",
        bbox: Rect {
            x: 0,
            y: 32,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_LABEL_PALETTE_INDICES),
        expected_pixels_sha256: "b9cf16d029820fa67d335ed11654175448eb2e5c0109bdd3503a756b35b289bc",
    },
    TextBlockSpec {
        id: "BTXB-0181",
        source: "ふつう",
        bbox: Rect {
            x: 64,
            y: 32,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_LABEL_PALETTE_INDICES),
        expected_pixels_sha256: "870227c84150db09a329e740708c338cae68a8b7c0a9602ce2d6b7040adade4d",
    },
    TextBlockSpec {
        id: "BTXB-0182",
        source: "むずかしい",
        bbox: Rect {
            x: 112,
            y: 32,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_LABEL_PALETTE_INDICES),
        expected_pixels_sha256: "60dbbc36317ba6d55109d6d4025040242927374f2127b19bb263a82b6f071254",
    },
    TextBlockSpec {
        id: "BTXB-0183",
        source: "ひとりで",
        bbox: Rect {
            x: 112,
            y: 48,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_TITLE_PALETTE_INDICES),
        expected_pixels_sha256: "8a10770dc94d85e2099bc49221b6a3df1ed7e292591beb2018622d04d74f9b09",
    },
    TextBlockSpec {
        id: "BTXB-0184",
        source: "ふたりで",
        bbox: Rect {
            x: 176,
            y: 48,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_TITLE_PALETTE_INDICES),
        expected_pixels_sha256: "2a692d32e21c86638c11ad4b395beb094f49dcaa4505c59d32d59cfa3bd52901",
    },
    TextBlockSpec {
        id: "BTXB-0185",
        source: "ぷよぷよステージ",
        bbox: Rect {
            x: 256,
            y: 0,
            width: 80,
            height: 56,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_LOGO_PALETTE_INDICES),
        expected_pixels_sha256: "519200412b1a3ba97b913b210c61fe94ef26a5f0e51b459056baec035f88fb50",
    },
    TextBlockSpec {
        id: "BTXB-0186",
        source: "3ステージで",
        bbox: Rect {
            x: 264,
            y: 64,
            width: 104,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_PANEL_PALETTE_INDICES),
        expected_pixels_sha256: "c636f84f5d5b7653488b27f9e5640a8b65786123294c20e6cbfc2fa066abf3c3",
    },
    TextBlockSpec {
        id: "BTXB-0187",
        source: "おしまい。",
        bbox: Rect {
            x: 296,
            y: 88,
            width: 72,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_PANEL_PALETTE_INDICES),
        expected_pixels_sha256: "481737282910290971572e4348fe9cc3c444561ef880005da72314ee88a457ec",
    },
    TextBlockSpec {
        id: "BTXB-0188",
        source: "STAGE1",
        bbox: Rect {
            x: 160,
            y: 64,
            width: 48,
            height: 8,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_STAGE_PALETTE_INDICES),
        expected_pixels_sha256: "ac291ac26f605f6c5715185a1ab74144c5275b043e1cc96c52959cfab87d408c",
    },
    TextBlockSpec {
        id: "BTXB-0189",
        source: "STAGE4",
        bbox: Rect {
            x: 160,
            y: 72,
            width: 48,
            height: 8,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_STAGE_PALETTE_INDICES),
        expected_pixels_sha256: "66b11ea7ed14aaafdb683c13258a05277dff1d870ed46b7ba11b32c18636d7be",
    },
    TextBlockSpec {
        id: "BTXB-0190",
        source: "ON",
        bbox: Rect {
            x: 192,
            y: 208,
            width: 16,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_ON_OFF_PALETTE_INDICES),
        expected_pixels_sha256: "286ebd2fa0644ea911d5bcda97c837835583a7baa3ea8c82578a96263f3eee36",
    },
    TextBlockSpec {
        id: "BTXB-0191",
        source: "OFF",
        bbox: Rect {
            x: 192,
            y: 224,
            width: 16,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU01_ON_OFF_PALETTE_INDICES),
        expected_pixels_sha256: "ee906fbc2120249574279a3763481370e064038c76d63dae4f68966f9aad6099",
    },
];

/// 복각 `ぷよぷよ`의 menu label·안내 panel과 영문 상태 네 개.
pub const MENU01_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &MENU01_ARCHIVES,
    name: "MENU01.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 512,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "b9cbb55b12018a2a0f6ff1ba41058e8442676b60a54a21282edb339e8715b746",
    blocks: &MENU01_BLOCKS,
};

const MENU203_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
const MENU203_LARGE_PALETTE_INDICES: [u8; 6] = [0, 8, 9, 10, 11, 12];
const MENU203_SMALL_MIX_PALETTE_INDICES: [u8; 3] = [0, 11, 15];
const MENU203_WHITE_PALETTE_INDICES: [u8; 2] = [0, 15];
const MENU203_SMALL_PALETTE_INDICES: [u8; 2] = [0, 11];
const MENU203_BLOCKS: [TextBlockSpec<'static>; 10] = [
    TextBlockSpec {
        id: "BTXB-0192",
        source: "ルール",
        bbox: Rect {
            x: 152,
            y: 88,
            width: 72,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU203_LARGE_PALETTE_INDICES),
        expected_pixels_sha256: "ca803914e73244e24f358a74e410063617355b3726cdd5de7189c9a8c4465f7e",
    },
    TextBlockSpec {
        id: "BTXB-0193",
        source: "を選んでください",
        bbox: Rect {
            x: 56,
            y: 112,
            width: 192,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU203_LARGE_PALETTE_INDICES),
        expected_pixels_sha256: "623f9628f16fefa362e7c71c2adaeace80a00ea1cbc60e6c7e8aa0dfc5ec274d",
    },
    TextBlockSpec {
        id: "BTXB-0194",
        source: "ぷよぷよのルー",
        bbox: Rect {
            x: 0,
            y: 32,
            width: 112,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU203_SMALL_MIX_PALETTE_INDICES),
        expected_pixels_sha256: "9c9223f8d78d8fefc8db123027ea4a3361dd37827a1999b6caa1267ecef3e5f3",
    },
    TextBlockSpec {
        id: "BTXB-0195",
        source: "ルを",
        bbox: Rect {
            x: 112,
            y: 32,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU203_WHITE_PALETTE_INDICES),
        expected_pixels_sha256: "0d679bc8b04df9ef7e7affd0c244aea5c361c2afe2c437b7dce6f03d6a6e1134",
    },
    TextBlockSpec {
        id: "BTXB-0196",
        source: "得点",
        bbox: Rect {
            x: 0,
            y: 48,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU203_SMALL_PALETTE_INDICES),
        expected_pixels_sha256: "195876ce9afba7d86e45776492394ba9711ebba03e9433f0543fc355e964cc85",
    },
    TextBlockSpec {
        id: "BTXB-0197",
        source: "ぷよ",
        bbox: Rect {
            x: 32,
            y: 48,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU203_SMALL_PALETTE_INDICES),
        expected_pixels_sha256: "38a69374e50312037630e9e311ebeb46b4ba5a78c65b41892ffa7eb17b1797d1",
    },
    TextBlockSpec {
        id: "BTXB-0198",
        source: "4こ",
        bbox: Rect {
            x: 64,
            y: 48,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU203_SMALL_PALETTE_INDICES),
        expected_pixels_sha256: "03010c9534af816a23fa1cf538b7d1aaf830df27984aa9c3cd562ad3fd6b7076",
    },
    TextBlockSpec {
        id: "BTXB-0199",
        source: "を消し",
        bbox: Rect {
            x: 96,
            y: 48,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU203_SMALL_PALETTE_INDICES),
        expected_pixels_sha256: "3411d69814d49c0e16ba70680b24ef8e12178fa901ac0c6ad92f9550f20d913a",
    },
    TextBlockSpec {
        id: "BTXB-0200",
        source: "2",
        bbox: Rect {
            x: 112,
            y: 0,
            width: 16,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU203_SMALL_PALETTE_INDICES),
        expected_pixels_sha256: "6681914b34c1904330dee279e5341071e37982e0135067ff48ffff68033ff3a9",
    },
    TextBlockSpec {
        id: "BTXB-0201",
        source: "6",
        bbox: Rect {
            x: 112,
            y: 16,
            width: 16,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU203_SMALL_PALETTE_INDICES),
        expected_pixels_sha256: "d4a44c140ed63e34eec2a54223dcaa390324363f2ed53655a14e8ecf0ad4dd6e",
    },
];

/// 복각 rule 화면의 조립형 안내 cell과 숫자 참고 두 개.
pub const MENU203_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &MENU203_ARCHIVES,
    name: "MENU203.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "cf16ac48e9ab4d01fec3164491ab7897ce1ce54226e22c72bb5482d6e1051b80",
    blocks: &MENU203_BLOCKS,
};

const MENU202_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
const MENU202_GUIDE_PALETTE_INDICES: [u8; 6] = [0, 8, 9, 10, 11, 12];
const MENU202_SELECTED_PUYO_PALETTE_INDICES: [u8; 15] =
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 15];
const MENU202_SMALL_PUYO_PALETTE_INDICES: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 14, 15];
const MENU202_SELECTED_OPTION_PALETTE_INDICES: [u8; 14] =
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 14, 15];
const MENU202_PRESSED_OPTION_PALETTE_INDICES: [u8; 12] = [1, 2, 3, 4, 5, 6, 7, 8, 11, 12, 14, 15];
const MENU202_BLOCKS: [TextBlockSpec<'static>; 11] = [
    TextBlockSpec {
        id: "BTXB-0202",
        source: "モードを",
        bbox: Rect {
            x: 152,
            y: 200,
            width: 96,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU202_GUIDE_PALETTE_INDICES),
        expected_pixels_sha256: "7d1763d207d5d762b5f53d78c2cc7077dc71309f4447919970b3aef6ffdbe466",
    },
    TextBlockSpec {
        id: "BTXB-0203",
        source: "選んでください",
        bbox: Rect {
            x: 80,
            y: 224,
            width: 168,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU202_GUIDE_PALETTE_INDICES),
        expected_pixels_sha256: "bdccac8c86a388e6374a2c095fd98cb1f57587be7b52884757893e7e8537a61c",
    },
    TextBlockSpec {
        id: "BTXB-0204",
        source: "ひとりで\nぷよぷよ",
        bbox: Rect {
            x: 104,
            y: 0,
            width: 80,
            height: 48,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU202_SELECTED_PUYO_PALETTE_INDICES),
        expected_pixels_sha256: "a1c9997a8bdf8ca6a590a1cf5addf78a99e4bed46c265072a483c543f15700ba",
    },
    TextBlockSpec {
        id: "BTXB-0205",
        source: "ひとりで\nぷよぷよ",
        bbox: Rect {
            x: 184,
            y: 0,
            width: 64,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU202_SMALL_PUYO_PALETTE_INDICES),
        expected_pixels_sha256: "f1b5d92516f69d5c218cb4ae945be63d07747047bc67b97479db77a40dedafed",
    },
    TextBlockSpec {
        id: "BTXB-0206",
        source: "ひとりで\nぷよぷよ",
        bbox: Rect {
            x: 184,
            y: 96,
            width: 64,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU202_SMALL_PUYO_PALETTE_INDICES),
        expected_pixels_sha256: "d4a4c51efc3fc9a9538b9c17d89815518ad5587c65564ab38909f124c020fa9d",
    },
    TextBlockSpec {
        id: "BTXB-0207",
        source: "ふたりで\nぷよぷよ",
        bbox: Rect {
            x: 104,
            y: 48,
            width: 80,
            height: 48,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU202_SELECTED_PUYO_PALETTE_INDICES),
        expected_pixels_sha256: "7bf3a6792aed0422bb757a7e405199bc1411f7fc922189676337a4dbe6db472a",
    },
    TextBlockSpec {
        id: "BTXB-0208",
        source: "ふたりで\nぷよぷよ",
        bbox: Rect {
            x: 184,
            y: 32,
            width: 64,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU202_SMALL_PUYO_PALETTE_INDICES),
        expected_pixels_sha256: "578ac76e8ff3a47852aec14c03f5ed24c5cd578f89f4d242cdf97fb1aaef986b",
    },
    TextBlockSpec {
        id: "BTXB-0209",
        source: "ふたりで\nぷよぷよ",
        bbox: Rect {
            x: 184,
            y: 120,
            width: 64,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU202_SMALL_PUYO_PALETTE_INDICES),
        expected_pixels_sha256: "4bbfb163a3f24006dbc47c966a8de3d7f551752e83438db4c58997b6be07a5db",
    },
    TextBlockSpec {
        id: "BTXB-0210",
        source: "おぷしょん",
        bbox: Rect {
            x: 104,
            y: 96,
            width: 80,
            height: 48,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU202_SELECTED_OPTION_PALETTE_INDICES),
        expected_pixels_sha256: "ebf76b43cea3efff919cea87a052fcd34e728625e6d5fcba8d329e7b4b422fab",
    },
    TextBlockSpec {
        id: "BTXB-0211",
        source: "おぷしょん",
        bbox: Rect {
            x: 184,
            y: 64,
            width: 64,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU202_SMALL_PUYO_PALETTE_INDICES),
        expected_pixels_sha256: "03fe69eab2a7d29ee82285c2a834ca5fa664377283fc33b68049456279364c25",
    },
    TextBlockSpec {
        id: "BTXB-0212",
        source: "おぷしょん",
        bbox: Rect {
            x: 184,
            y: 144,
            width: 64,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU202_PRESSED_OPTION_PALETTE_INDICES),
        expected_pixels_sha256: "d934a0f6154aa114c936b0c9b0ccacdc8aa2fbbbbbba924760e3c2494d1c3418",
    },
];

/// 복각 mode 선택 화면의 안내와 세 button label의 세 표시 상태.
pub const MENU202_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &MENU202_ARCHIVES,
    name: "MENU202.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 768,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "004e32789dee11bc7e27fcee47a12e820edf1ffd341b37632d376fc7a16c18d4",
    blocks: &MENU202_BLOCKS,
};

const MENU201_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
const MENU201_ALL_CLEAR_PALETTE_INDICES: [u8; 4] = [0, 1, 9, 10];
const MENU201_HELP_LABEL_PALETTE_INDICES: [u8; 2] = [0, 2];
const MENU201_BLOCKS: [TextBlockSpec<'static>; 9] = [
    TextBlockSpec {
        id: "BTXB-0213",
        source: "全消し",
        bbox: Rect {
            x: 48,
            y: 64,
            width: 56,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU201_ALL_CLEAR_PALETTE_INDICES),
        expected_pixels_sha256: "83aecd8ec9e716829f141069a252f6785eb94861cbe28628dc50fc93c295ce2d",
    },
    TextBlockSpec {
        id: "BTXB-0214",
        source: "のひみつ",
        bbox: Rect {
            x: 0,
            y: 136,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU201_HELP_LABEL_PALETTE_INDICES),
        expected_pixels_sha256: "f2c0c2c488cd330cb13baa1fe7e26ba1775f1220a92133c006bb0be4accff7b6",
    },
    TextBlockSpec {
        id: "BTXB-0215",
        source: "きほんそうさ",
        bbox: Rect {
            x: 0,
            y: 152,
            width: 96,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU201_HELP_LABEL_PALETTE_INDICES),
        expected_pixels_sha256: "bbd1679bb6ec3ec413bfd6e796ff5447ce51d3184bc69d549f3de0d202653da6",
    },
    TextBlockSpec {
        id: "BTXB-0216",
        source: "ゲームオーバー",
        bbox: Rect {
            x: 0,
            y: 168,
            width: 112,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU201_HELP_LABEL_PALETTE_INDICES),
        expected_pixels_sha256: "023cbf41450a31f8e570c442f157893e3126d5ca42a88063840405850b005867",
    },
    TextBlockSpec {
        id: "BTXB-0217",
        source: "れんさをつづけ",
        bbox: Rect {
            x: 0,
            y: 184,
            width: 112,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU201_HELP_LABEL_PALETTE_INDICES),
        expected_pixels_sha256: "044057f0a8939eced3ec9e1b6f77dd2e1478659a40f4e550d6036ee4704d2cf5",
    },
    TextBlockSpec {
        id: "BTXB-0218",
        source: "おじゃまぷよ",
        bbox: Rect {
            x: 0,
            y: 200,
            width: 96,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU201_HELP_LABEL_PALETTE_INDICES),
        expected_pixels_sha256: "a8321d9c0dbc6e88b701b76ff336e34d6bf46baff8ebed9afc83242ea3692463",
    },
    TextBlockSpec {
        id: "BTXB-0219",
        source: "そうさい",
        bbox: Rect {
            x: 0,
            y: 216,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU201_HELP_LABEL_PALETTE_INDICES),
        expected_pixels_sha256: "0a388cc961f42899c713db0588098a837090d56ab4766c236e4f0a4187b3f19a",
    },
    TextBlockSpec {
        id: "BTXB-0220",
        source: "ぜんけし",
        bbox: Rect {
            x: 80,
            y: 216,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU201_HELP_LABEL_PALETTE_INDICES),
        expected_pixels_sha256: "da3149cc2a6c09e8163d2c99745588ece65f9dd58982ea88ff166eca3edb8f3f",
    },
    TextBlockSpec {
        id: "BTXB-0221",
        source: "クイックターン",
        bbox: Rect {
            x: 0,
            y: 232,
            width: 112,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU201_HELP_LABEL_PALETTE_INDICES),
        expected_pixels_sha256: "2b65900be7f0446c7556a395704452881a45301c5ee9a97bf19a2c1b7b4ec84e",
    },
];

/// 복각 도움말 화면의 독립 heading과 전소거 label.
pub const MENU201_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &MENU201_ARCHIVES,
    name: "MENU201.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 512,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "7f9741908762cd6a26d8fcc31f3d9f8551aa1e5bbdc790f3e149a9e9c690e3a1",
    blocks: &MENU201_BLOCKS,
};
