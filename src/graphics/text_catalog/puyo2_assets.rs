//! PUYO2 계열의 정적 BTX text asset 계약.

use super::{
    FixedCellGridSpec, Layout, PaletteIndexSpec, PixelFormat, Rect, TextAssetSpec, TextBlockSpec,
    TextStorageSpec,
};

const P2_OP1_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
const P2_OP1_PALETTE_INDICES: [u8; 2] = [0, 15];
const P2_OP1_BLOCKS: [TextBlockSpec<'static>; 1] = [TextBlockSpec {
    id: "BTXB-0106",
    source: "勇気と愛気",
    bbox: Rect {
        x: 80,
        y: 152,
        width: 152,
        height: 18,
    },
    palette_indices: PaletteIndexSpec::Values(&P2_OP1_PALETTE_INDICES),
    expected_pixels_sha256: "e1aad6cc6dc825721ab2cba27f97b38f49775f40b7d13cd62a8ce38ecef88bd9",
}];

/// PUYO2 opening에서 실제 표시를 확인한 AIDS 캠페인 일본어 slogan.
pub const P2_OP1_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &P2_OP1_ARCHIVES,
    name: "P2_OP1.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 224,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "1dc8dfe5bc9fc309bf9cde4a3be1a1e80681aa458f96bf24497dfe67554d2921",
    blocks: &P2_OP1_BLOCKS,
};

const BATAN2_ARCHIVES: [&str; 3] = [
    "/GOCHA1/TUU_GRP.VDD;1",
    "/GOCHA2/GOTY_GRP.VDD;1",
    "/HORI2/HORI2GRP.VDD;1",
];
const BATAN2_YATTA_PALETTE_INDICES: [u8; 5] = [0, 4, 9, 11, 15];
const BATAN2_BATANKYU_PALETTE_INDICES: [u8; 6] = [0, 1, 2, 3, 5, 6];
const BATAN2_ALL_CLEAR_PALETTE_INDICES: [u8; 6] = [0, 8, 9, 10, 11, 12];
const BATAN2_BREAK_PALETTE_INDICES: [u8; 7] = [0, 1, 3, 4, 5, 6, 7];
const BATAN2_OFFSET_PALETTE_INDICES: [u8; 3] = [0, 1, 15];
const BATAN2_BLOCKS: [TextBlockSpec<'static>; 5] = [
    TextBlockSpec {
        id: "BTXB-0107",
        source: "やった!",
        bbox: Rect {
            x: 0,
            y: 0,
            width: 96,
            height: 48,
        },
        palette_indices: PaletteIndexSpec::Values(&BATAN2_YATTA_PALETTE_INDICES),
        expected_pixels_sha256: "5c50cce30b0123b317d34cb2dc3693b8aa2c66a7b64b10e422724344417412f9",
    },
    TextBlockSpec {
        id: "BTXB-0108",
        source: "ばたんきゅ〜",
        bbox: Rect {
            x: 96,
            y: 0,
            width: 96,
            height: 56,
        },
        palette_indices: PaletteIndexSpec::Values(&BATAN2_BATANKYU_PALETTE_INDICES),
        expected_pixels_sha256: "03bf32a46fe0eeef71b81c7a98de6f26be94c53458357d72ca06e927c14bd74e",
    },
    TextBlockSpec {
        id: "BTXB-0109",
        source: "全消し!",
        bbox: Rect {
            x: 0,
            y: 64,
            width: 80,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&BATAN2_ALL_CLEAR_PALETTE_INDICES),
        expected_pixels_sha256: "685e6c1abb836771bbeda48024fc016e486aa82ed85d9dfec819d2c0b168c045",
    },
    TextBlockSpec {
        id: "BTXB-0110",
        source: "きゅうけい",
        bbox: Rect {
            x: 80,
            y: 64,
            width: 64,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&BATAN2_BREAK_PALETTE_INDICES),
        expected_pixels_sha256: "0572c23e78d6dca2f6773b69cc132f4f097d4b15bbfe7211f19d4ec9108da0e2",
    },
    TextBlockSpec {
        id: "BTXB-0111",
        source: "相殺",
        bbox: Rect {
            x: 48,
            y: 88,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&BATAN2_OFFSET_PALETTE_INDICES),
        expected_pixels_sha256: "77e952264455c3582b803c1f35c78f9cfbcb8258a5dbcb769691cb18e5b017e5",
    },
];

/// 세 미니게임이 공유하는 결과·상태 atlas의 일본어 cell 다섯 개.
pub const BATAN2_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &BATAN2_ARCHIVES,
    name: "BATAN2.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 126,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "14caca5635bff3fb2b555369156f88e5802bc7b66c65a9b358d8b3271945f895",
    blocks: &BATAN2_BLOCKS,
};

const P2_OP13_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
const P2_OP13_PUYO_PALETTE_INDICES: [u8; 14] = [0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15];
const P2_OP13_TSU_PALETTE_INDICES: [u8; 13] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12, 13];
const P2_OP13_COPYRIGHT_PALETTE_INDICES: [u8; 3] = [0, 1, 11];
const P2_OP13_BLOCKS: [TextBlockSpec<'static>; 4] = [
    TextBlockSpec {
        id: "BTXB-0112",
        source: "ぷ",
        bbox: Rect {
            x: 0,
            y: 0,
            width: 56,
            height: 48,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_OP13_PUYO_PALETTE_INDICES),
        expected_pixels_sha256: "973aea90b93cf32b4d688550c5428a0ee1895e52c7b011253029bd4644bf04b0",
    },
    TextBlockSpec {
        id: "BTXB-0113",
        source: "よ",
        bbox: Rect {
            x: 56,
            y: 0,
            width: 56,
            height: 48,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_OP13_PUYO_PALETTE_INDICES),
        expected_pixels_sha256: "dd1c01f79b34e2047849810742f88e968a5fb648ecbcd28514d7d0d605c39b7a",
    },
    TextBlockSpec {
        id: "BTXB-0114",
        source: "通",
        bbox: Rect {
            x: 0,
            y: 48,
            width: 88,
            height: 72,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_OP13_TSU_PALETTE_INDICES),
        expected_pixels_sha256: "6e7b580c611fadafd3357923cb377a41904776b5e458319640f9a7cfaabdadaf",
    },
    TextBlockSpec {
        id: "BTXB-0115",
        source: "© COMPILE 1994",
        bbox: Rect {
            x: 88,
            y: 48,
            width: 144,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_OP13_COPYRIGHT_PALETTE_INDICES),
        expected_pixels_sha256: "c36ae17e67b4908feefac7059857d94bf36ad812ec67dc9aeb32ac0098c6528b",
    },
];

/// `ぷ`·`よ`·`通`을 별도 cell로 조립하는 opening logo와 저작권 표기.
pub const P2_OP13_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &P2_OP13_ARCHIVES,
    name: "P2_OP13.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 240,
        height: 120,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "f31c8c65a8099bbe2dbaf0d95ccbec7039e32e2938829f323c29c1726292c331",
    blocks: &P2_OP13_BLOCKS,
};

const PCOMI01_ARCHIVES: [&str; 1] = ["/PCOMI/PC1_GRP.VDD;1"];
const PCOMI01_MENU_PALETTE_INDICES: [u8; 15] = [0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const PCOMI01_WALLPAPER_PALETTE_INDICES: [u8; 3] = [0, 6, 15];
const PCOMI01_LOGO_PALETTE_INDICES: [u8; 7] = [3, 6, 9, 10, 11, 13, 14];
const PCOMI01_BLOCKS: [TextBlockSpec<'static>; 11] = [
    TextBlockSpec {
        id: "BTXB-0116",
        source: "プロフィール 編集",
        bbox: Rect {
            x: 0,
            y: 32,
            width: 128,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&PCOMI01_MENU_PALETTE_INDICES),
        expected_pixels_sha256: "9cdb8eea19b66ed6473afa4a7df72a77712ab78d6e40a3cc883b84e7d15fcdba",
    },
    TextBlockSpec {
        id: "BTXB-0117",
        source: "プロフィール 一覧",
        bbox: Rect {
            x: 0,
            y: 56,
            width: 128,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&PCOMI01_MENU_PALETTE_INDICES),
        expected_pixels_sha256: "2380e66433a23878b8a2c83792b1216414d910c9ad057ef655a746ae7a9b0b91",
    },
    TextBlockSpec {
        id: "BTXB-0118",
        source: "メッセージ 編集",
        bbox: Rect {
            x: 0,
            y: 80,
            width: 120,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&PCOMI01_MENU_PALETTE_INDICES),
        expected_pixels_sha256: "fc886dcc1c2bcff765c4ac25b1fff04687af701950ef4b1154f6ecd2a14fad9b",
    },
    TextBlockSpec {
        id: "BTXB-0119",
        source: "メッセージ 一覧",
        bbox: Rect {
            x: 0,
            y: 104,
            width: 120,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&PCOMI01_MENU_PALETTE_INDICES),
        expected_pixels_sha256: "a3eda0289dd346f6884f370b964dc65c7e545f1618dc25030a883704783caad6",
    },
    TextBlockSpec {
        id: "BTXB-0120",
        source: "似顔絵 編集",
        bbox: Rect {
            x: 0,
            y: 128,
            width: 104,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&PCOMI01_MENU_PALETTE_INDICES),
        expected_pixels_sha256: "4579875bcc3ec30f7f22341ae08eb15f28087fc4ce2383468ee1165a4f588df5",
    },
    TextBlockSpec {
        id: "BTXB-0121",
        source: "ダウンロード",
        bbox: Rect {
            x: 0,
            y: 152,
            width: 96,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&PCOMI01_MENU_PALETTE_INDICES),
        expected_pixels_sha256: "dc3a930549f8938f410e785f015500f06ba705cc050ae8d4b8ff9a37f90a8fcc",
    },
    TextBlockSpec {
        id: "BTXB-0122",
        source: "ぷよコミ！ 終了",
        bbox: Rect {
            x: 0,
            y: 176,
            width: 120,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&PCOMI01_MENU_PALETTE_INDICES),
        expected_pixels_sha256: "fd636a2423753adfda83b1eed614e14bbd77b89e4f831c179630f531c2496990",
    },
    TextBlockSpec {
        id: "BTXB-0123",
        source: "かべがみ",
        bbox: Rect {
            x: 176,
            y: 0,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&PCOMI01_WALLPAPER_PALETTE_INDICES),
        expected_pixels_sha256: "1d2e34db6c5d70307c13c02ad1e06dd4ee7e343b00689ebd23e4a3ed93481716",
    },
    TextBlockSpec {
        id: "BTXB-0124",
        source: "ぷよコミ logo + mascot",
        bbox: Rect {
            x: 256,
            y: 0,
            width: 64,
            height: 64,
        },
        palette_indices: PaletteIndexSpec::Values(&PCOMI01_LOGO_PALETTE_INDICES),
        expected_pixels_sha256: "2cade94d1bec751c1f48cf55f839a2be4e7439c0878c969ac70380a619663f68",
    },
    TextBlockSpec {
        id: "BTXB-0125",
        source: "データのロード",
        bbox: Rect {
            x: 0,
            y: 200,
            width: 128,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&PCOMI01_MENU_PALETTE_INDICES),
        expected_pixels_sha256: "c025e2564bd10d406bd9dbaa346e32a01992a5900bc06f04c737a1b840b82e9c",
    },
    TextBlockSpec {
        id: "BTXB-0126",
        source: "新しく作る",
        bbox: Rect {
            x: 0,
            y: 224,
            width: 96,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&PCOMI01_MENU_PALETTE_INDICES),
        expected_pixels_sha256: "b7209c9d5381186cadd1539cba797d560c866ae956131265b3d229265ca91e99",
    },
];

/// PUYO communication 메뉴·logo의 선택된 runtime cell 11개.
pub const PCOMI01_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &PCOMI01_ARCHIVES,
    name: "PCOMI01.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 512,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "6cd2b5ed08ae7b9c3b02be43d91ba9dc5db97b2f75fd60f0459ad7937b24afa0",
    blocks: &PCOMI01_BLOCKS,
};

const PCOMI02_ARCHIVES: [&str; 2] = ["/PCOMI/PC1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
const PCOMI02_LOGO_PALETTE_INDICES: [u8; 33] = [
    0, 1, 7, 8, 9, 10, 12, 13, 14, 15, 17, 18, 21, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34,
    35, 36, 37, 39, 40, 44, 45, 46,
];
const PCOMI02_BLOCKS: [TextBlockSpec<'static>; 1] = [TextBlockSpec {
    id: "BTXB-0127",
    source: "ぷよコミ！ logo + mascot",
    bbox: Rect {
        x: 0,
        y: 0,
        width: 176,
        height: 40,
    },
    palette_indices: PaletteIndexSpec::Values(&PCOMI02_LOGO_PALETTE_INDICES),
    expected_pixels_sha256: "8f12ddf9ad38acbe9b5a0e26c05e0af3f2ac96c15ac6d5af1ddb2767d0a1ff0b",
}];

/// PCOMI와 PUYO2가 공유하는 PUYO communication logo cell.
pub const PCOMI02_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &PCOMI02_ARCHIVES,
    name: "PCOMI02.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 128,
        format: PixelFormat::Indexed8,
    },
    expected_decompressed_sha256:
        "20f6023a1eb7dfac025313c907d2ae1478d32005837093f170d5d592306c276e",
    blocks: &PCOMI02_BLOCKS,
};

const P2RULED1_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
const P2RULED1_TEXT_PALETTE_INDICES: [u8; 3] = [0, 1, 15];
const P2RULED1_BLOCKS: [TextBlockSpec<'static>; 33] = [
    TextBlockSpec {
        id: "BTXB-0139",
        source: "0",
        bbox: Rect {
            x: 16,
            y: 0,
            width: 8,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "e35635a173ba9b501bb68574e6369e336ae16d38b42440cb2b470851ced4f845",
    },
    TextBlockSpec {
        id: "BTXB-0140",
        source: "1",
        bbox: Rect {
            x: 24,
            y: 0,
            width: 8,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "627637d8593fe696fa1b3b7389d2a85dfc339e548c66d116a70908be7a613bc1",
    },
    TextBlockSpec {
        id: "BTXB-0141",
        source: "2",
        bbox: Rect {
            x: 32,
            y: 0,
            width: 8,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "f6d6c064025d3d3e8a824cb96374fe41023e026da9e3d696e93b9fa9760e32ea",
    },
    TextBlockSpec {
        id: "BTXB-0142",
        source: "3",
        bbox: Rect {
            x: 40,
            y: 0,
            width: 8,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "99ac0ef904738f3275ec3f489a1a5dd8dbd17861d9dcbbc2de74dc276759164e",
    },
    TextBlockSpec {
        id: "BTXB-0143",
        source: "4",
        bbox: Rect {
            x: 48,
            y: 0,
            width: 8,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "f1c9b50bced055d6bcedabd95e3b7794a24a9a5b31850f3be97c9bc9520b2e7a",
    },
    TextBlockSpec {
        id: "BTXB-0144",
        source: "5",
        bbox: Rect {
            x: 56,
            y: 0,
            width: 8,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "91f8053e8e92f432db68b5737011411ecd11ea937dc3e471883f023f91c93231",
    },
    TextBlockSpec {
        id: "BTXB-0145",
        source: "6",
        bbox: Rect {
            x: 64,
            y: 0,
            width: 8,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "0957c98a0bf65f0dbf02f8dfcc29d59868726f77333ff101234362a579c4b150",
    },
    TextBlockSpec {
        id: "BTXB-0146",
        source: "7",
        bbox: Rect {
            x: 72,
            y: 0,
            width: 8,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "837c7f2ff9f9b78043519cce61844449fbb001a289dada15ff5516a9ca6d4b43",
    },
    TextBlockSpec {
        id: "BTXB-0147",
        source: "8",
        bbox: Rect {
            x: 80,
            y: 0,
            width: 8,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "7a2e85c60f90c921ad6552e99e0a60cb5028f7d67c57f888240e3657291296f1",
    },
    TextBlockSpec {
        id: "BTXB-0148",
        source: "9",
        bbox: Rect {
            x: 88,
            y: 0,
            width: 8,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "f82d36f9af4e16f9161347c882976d939995adb4bcf748f7371d60ae6f45b024",
    },
    TextBlockSpec {
        id: "BTXB-0149",
        source: "から",
        bbox: Rect {
            x: 160,
            y: 0,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "829945d17d00f20cf20452798d9a042df052f6d2e1f153070e1028094790d788",
    },
    TextBlockSpec {
        id: "BTXB-0150",
        source: "タイプ",
        bbox: Rect {
            x: 192,
            y: 0,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "375073e447435c8420bc9643974b6a5479c783fadf2e7e2af24461d2d36c2bb2",
    },
    TextBlockSpec {
        id: "BTXB-0151",
        source: "ふつう",
        bbox: Rect {
            x: 112,
            y: 16,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "50ecdf6fb7b2b6818cd3dd5da3ce042e91af24459a8f59ecd86e278f7cc89655",
    },
    TextBlockSpec {
        id: "BTXB-0152",
        source: "かたい",
        bbox: Rect {
            x: 160,
            y: 16,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "1990470a66f0e140ef0c6299c65fcf7f552a6b16e8b82fd7626c928e91add560",
    },
    TextBlockSpec {
        id: "BTXB-0153",
        source: "かたぷよ",
        bbox: Rect {
            x: 112,
            y: 32,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "0a4c066f36dcd2f87664b6e39c3b48fe3b62b75451dc27307b8da7e3b3321132",
    },
    TextBlockSpec {
        id: "BTXB-0154",
        source: "おわり",
        bbox: Rect {
            x: 176,
            y: 32,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "51a2823913736949545baa16604cccee48b1a80dc66014569fe3472d0b10176d",
    },
    TextBlockSpec {
        id: "BTXB-0155",
        source: "個",
        bbox: Rect {
            x: 208,
            y: 16,
            width: 16,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "169c21f6a91ac25eb34f0920b57af5f5fe3fdd79bfa38bdc9d18bed247c551e0",
    },
    TextBlockSpec {
        id: "BTXB-0156",
        source: "点",
        bbox: Rect {
            x: 224,
            y: 16,
            width: 16,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "42ad04273e995db8d4cd7bcdc010eb2c63fc9ac69dbdb148a18fd3cf01b4fbd8",
    },
    TextBlockSpec {
        id: "BTXB-0157",
        source: "ぷよのかたさ",
        bbox: Rect {
            x: 112,
            y: 48,
            width: 96,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "6666c0e827a549ef9c78f07c5c672fa536baf0a3d9838b49003ae0eab6fc0e98",
    },
    TextBlockSpec {
        id: "BTXB-0158",
        source: "なにをするの？",
        bbox: Rect {
            x: 0,
            y: 64,
            width: 112,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "282b5cc6e3bc00d81655bf45b8517a6897b34d1386e19972038c6b3b32569423",
    },
    TextBlockSpec {
        id: "BTXB-0159",
        source: "ルール",
        bbox: Rect {
            x: 112,
            y: 64,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "2827f48e4fed1e4b35feca41f29854c7aad8a2d6248623fbcadb13be8430aacb",
    },
    TextBlockSpec {
        id: "BTXB-0160",
        source: "をつくる",
        bbox: Rect {
            x: 160,
            y: 64,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "fd3a51bd01d8baf4865a7b1fde30615c4c925034b9ff81a53c1e18f00b7a5f4d",
    },
    TextBlockSpec {
        id: "BTXB-0161",
        source: "選びにもどる",
        bbox: Rect {
            x: 0,
            y: 80,
            width: 96,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "d2be3e61d874ba77f69047faca11d854fc0d4c5b4018214a0ecf08f88e892ce7",
    },
    TextBlockSpec {
        id: "BTXB-0162",
        source: "作成中",
        bbox: Rect {
            x: 96,
            y: 80,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "c8f45e1b171bb4a42ecf8a3499a0f227c0788432ddc65aece1093a8a7339685a",
    },
    TextBlockSpec {
        id: "BTXB-0163",
        source: "をコピーする",
        bbox: Rect {
            x: 144,
            y: 80,
            width: 96,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "bbe75bfde6e78a7550ec046550f4117b47ea64089a32db6ae817cd02d61bfddf",
    },
    TextBlockSpec {
        id: "BTXB-0164",
        source: "ぷよはいくつで消える",
        bbox: Rect {
            x: 0,
            y: 96,
            width: 176,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "8218ede82de01851f98e8835992bd6eeae8c293d7277905a5dbfad3daf0fc745",
    },
    TextBlockSpec {
        id: "BTXB-0165",
        source: "おじゃま",
        bbox: Rect {
            x: 176,
            y: 96,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "00953b18ff80c53ae562f22e47753a0415b7b4ac28c19688eeecb00ef0b23b96",
    },
    TextBlockSpec {
        id: "BTXB-0166",
        source: "を 1つ降らせる",
        bbox: Rect {
            x: 0,
            y: 112,
            width: 128,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "b5183c4eb13769f4c59106094dce5de83c55a7c4c5acfa487535adabfce091ed",
    },
    TextBlockSpec {
        id: "BTXB-0167",
        source: "点を消した時の点",
        bbox: Rect {
            x: 128,
            y: 112,
            width: 112,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "694ce3ad0da624767816c9dd9a77ca9a34a43d019064948d8b7d773de955d9df",
    },
    TextBlockSpec {
        id: "BTXB-0168",
        source: "連鎖の倍率の上がりかた",
        bbox: Rect {
            x: 0,
            y: 128,
            width: 176,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "693600fcb751e93b6092377b27d84ca8438c146ddbefbdbf4048cf46298c7366",
    },
    TextBlockSpec {
        id: "BTXB-0169",
        source: "全消しで降らせる個数",
        bbox: Rect {
            x: 0,
            y: 144,
            width: 160,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "6028739b99685d1e2734d3d98220130234ae20de7f0ad79f4e99809d93b5d727",
    },
    TextBlockSpec {
        id: "BTXB-0170",
        source: "ゲームをはじめる",
        bbox: Rect {
            x: 0,
            y: 160,
            width: 128,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "9dbf2f6041e97d44e3ddec5dec6b34e09a664053fa221f449646867f40ffd7c5",
    },
    TextBlockSpec {
        id: "BTXB-0171",
        source: "乱入されたよ",
        bbox: Rect {
            x: 128,
            y: 160,
            width: 96,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2RULED1_TEXT_PALETTE_INDICES),
        expected_pixels_sha256: "0c713b50a3dad5cea40060b08db7eb38ba816f37bfe12c456aad698bd5d8e281",
    },
];

/// PUYO2 static cell table로 분리한 rule 편집 숫자와 일본어 문구.
pub const P2RULED1_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &P2RULED1_ARCHIVES,
    name: "P2RULED1.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 176,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "eae5033fbbcd56caaedf0f25eaac33c692e741837f85c2cb1cece087498253da",
    blocks: &P2RULED1_BLOCKS,
};

const P2_INFO3_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
const P2_INFO3_ALL_CLEAR_PALETTE_INDICES: [u8; 4] = [0, 1, 9, 10];
const P2_INFO3_BLOCKS: [TextBlockSpec<'static>; 1] = [TextBlockSpec {
    id: "BTXB-0128",
    source: "全消し",
    bbox: Rect {
        x: 0,
        y: 56,
        width: 56,
        height: 24,
    },
    palette_indices: PaletteIndexSpec::Values(&P2_INFO3_ALL_CLEAR_PALETTE_INDICES),
    expected_pixels_sha256: "83aecd8ec9e716829f141069a252f6785eb94861cbe28628dc50fc93c295ce2d",
}];

/// PUYO2 static cell table로 분리한 `全消し` 상태 label.
pub const P2_INFO3_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &P2_INFO3_ARCHIVES,
    name: "P2_INFO3.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 120,
        height: 80,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "55ccc34fff6bae32fe5d5cf30130952b21f1f061187af978ef623ca6c11f9a49",
    blocks: &P2_INFO3_BLOCKS,
};

const P2_INFO1_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
const P2_INFO1_DESCRIPTION_PALETTE_INDICES: [u8; 2] = [0, 2];
const P2_INFO1_START_PALETTE_INDICES: [u8; 3] = [0, 1, 15];
const P2_INFO1_CALLOUT_PALETTE_INDICES: [u8; 2] = [0, 1];
const P2_INFO1_BLOCKS: [TextBlockSpec<'static>; 10] = [
    TextBlockSpec {
        id: "BTXB-0129",
        source: "きほんそうさのひみつ",
        bbox: Rect {
            x: 0,
            y: 136,
            width: 176,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_INFO1_DESCRIPTION_PALETTE_INDICES),
        expected_pixels_sha256: "8f3d1aa2cb10465587da243b2cdc8a8794281e7b1461b6bfe1613d839be53a60",
    },
    TextBlockSpec {
        id: "BTXB-0130",
        source: "げーむおーばーのひみつ",
        bbox: Rect {
            x: 0,
            y: 152,
            width: 176,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_INFO1_DESCRIPTION_PALETTE_INDICES),
        expected_pixels_sha256: "879ac9d2b9939f6e29b66a54bd65fc1dd2dc00a4902abe327092ab6cea547fd2",
    },
    TextBlockSpec {
        id: "BTXB-0131",
        source: "れんぞくこうげきのひみつ",
        bbox: Rect {
            x: 0,
            y: 168,
            width: 176,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_INFO1_DESCRIPTION_PALETTE_INDICES),
        expected_pixels_sha256: "4e4fa7d6e9df9750021c5a9467389d5c345e827d9be34dda63fbb1d992bec4c4",
    },
    TextBlockSpec {
        id: "BTXB-0132",
        source: "おじゃまぷよのひみつ",
        bbox: Rect {
            x: 0,
            y: 184,
            width: 176,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_INFO1_DESCRIPTION_PALETTE_INDICES),
        expected_pixels_sha256: "b4059c3079d38805f436174e5bdcf150f622e8cd557270744be8b00f2623e142",
    },
    TextBlockSpec {
        id: "BTXB-0133",
        source: "そうさいのひみつ",
        bbox: Rect {
            x: 0,
            y: 200,
            width: 176,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_INFO1_DESCRIPTION_PALETTE_INDICES),
        expected_pixels_sha256: "04c593b13becc48fbf29f8c368fc686a33bc3dab3017546cd03f583fddb51ef0",
    },
    TextBlockSpec {
        id: "BTXB-0134",
        source: "ぜんけしのひみつ",
        bbox: Rect {
            x: 0,
            y: 216,
            width: 176,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_INFO1_DESCRIPTION_PALETTE_INDICES),
        expected_pixels_sha256: "3acc163b1984e33635c57316ab74069088b127178b1b9ead8281dedfba79a705",
    },
    TextBlockSpec {
        id: "BTXB-0135",
        source: "クイックターンのひみつ",
        bbox: Rect {
            x: 0,
            y: 232,
            width: 176,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_INFO1_DESCRIPTION_PALETTE_INDICES),
        expected_pixels_sha256: "b683566eb72ba7896e8026beac96ac6c7eb7a510bd19638d884012b44aed4539",
    },
    TextBlockSpec {
        id: "BTXB-0136",
        source: "START",
        bbox: Rect {
            x: 176,
            y: 216,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_INFO1_START_PALETTE_INDICES),
        expected_pixels_sha256: "9ac926fc1b6c753b327ece34d62c8baefd4fe4955d8c76ea429d7477003a9db2",
    },
    TextBlockSpec {
        id: "BTXB-0137",
        source: "コレ",
        bbox: Rect {
            x: 224,
            y: 192,
            width: 24,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_INFO1_CALLOUT_PALETTE_INDICES),
        expected_pixels_sha256: "525333aef0824aee9ce213ff2575065728ca6bd7164e0f1a8d8a5755c512ce21",
    },
    TextBlockSpec {
        id: "BTXB-0138",
        source: "ここ",
        bbox: Rect {
            x: 224,
            y: 208,
            width: 16,
            height: 8,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_INFO1_CALLOUT_PALETTE_INDICES),
        expected_pixels_sha256: "1ce0d1e8bfac9f8e5f55de0c5352ce401722efa3bdc32e18a3e00a2e2d0769bc",
    },
];

/// PUYO2 static cell table로 분리한 조작 도움말 label과 보조표시.
pub const P2_INFO1_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &P2_INFO1_ARCHIVES,
    name: "P2_INFO1.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "a2c8431089413d659dbd119aed3e9f479ddb9ab6cbf600b2f3c2abe06b66b464",
    blocks: &P2_INFO1_BLOCKS,
};

const P2_INFO_GRID_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
const P2_INFO_GRID_PAGE_PALETTE_INDICES: [u8; 3] = [0, 1, 15];
const P2_INFO_GRID_ACTIVE_PALETTE_INDICES: [u8; 2] = [1, 15];
const P2_INFO_GRID_EMPTY_PALETTE_INDICES: [u8; 1] = [0];
const P2_INFO4_BLOCKS: [TextBlockSpec<'static>; 1] = [TextBlockSpec {
    id: "BTXB-0172",
    source: "落ちてくるぷよはレバーで左右にう\n\
ごかせる。下にいれるとはやく落と\n\
せる。ボタンをおすと回転する。右\n\
ボタンで右回転。左ボタンで左回転\n\
。ぷよは上下左右に4匹以上くっつ\n\
くと消えちゃうよ。左から3列目に\n\
つみあげるとゲームオーバーだ。ぷ\n\
よを消すと相手におじゃまぷよがふ\n\
る。連鎖をいっぱいつくると、おじ\n\
ゃまぷよをいっぱい送れるぞ。おじ\n\
ゃまぷよは、ふつうのぷよにつられ\n\
て消えてしまう。固ぷよは1回では\n\
消えないんだ。でも、こうすればい\n\
っぺんに消えてしまうぞ。得点ぷよ\n\
はふつうに消えるけど点が入るんだ\n\
。",
    bbox: Rect {
        x: 0,
        y: 0,
        width: 256,
        height: 256,
    },
    palette_indices: PaletteIndexSpec::Values(&P2_INFO_GRID_PAGE_PALETTE_INDICES),
    expected_pixels_sha256: "36339a027faf473211cd13c84d1bf2e39a03e060caa025b115c9ef51fb479c35",
}];

/// PUYO2 static cell table로 확정한 16x16 글리프 조립형 기본 조작 설명 page.
pub const P2_INFO4_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &P2_INFO_GRID_ARCHIVES,
    name: "P2_INFO4.BTX",
    storage: TextStorageSpec::FixedCellGrid(FixedCellGridSpec {
        cell_width: 16,
        cell_height: 16,
        expected_active_cells: 241,
        expected_empty_cells: 15,
        expected_unique_glyphs: 80,
        active_palette_indices: PaletteIndexSpec::Values(&P2_INFO_GRID_ACTIVE_PALETTE_INDICES),
        empty_palette_indices: PaletteIndexSpec::Values(&P2_INFO_GRID_EMPTY_PALETTE_INDICES),
    }),
    layout: Layout {
        width: 256,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "e033d4211bf2e7ebb58e5540a252fb69ff29ad62890f32609bc1552de1273f09",
    blocks: &P2_INFO4_BLOCKS,
};

const P2_INFO5_BLOCKS: [TextBlockSpec<'static>; 1] = [TextBlockSpec {
    id: "BTXB-0173",
    source: "相手からのおじゃまぷよはこちらか\n\
らのおじゃまぷよで打ち消すことが\n\
できるぞ。これが出てるときにぷよ\n\
を消すと、たくさんおじゃまぷよを\n\
送れるぞ。こんな場合でもボタンを\n\
2回おすと上下が入れかわる。",
    bbox: Rect {
        x: 0,
        y: 0,
        width: 256,
        height: 256,
    },
    palette_indices: PaletteIndexSpec::Values(&P2_INFO_GRID_PAGE_PALETTE_INDICES),
    expected_pixels_sha256: "de359e2b9c75f7029825bfd3b8b202d8d2adabc34d6fdb1359ecbc782b20de06",
}];

/// PUYO2 static cell table로 확정한 16x16 글리프 조립형 상쇄 설명 page.
pub const P2_INFO5_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &P2_INFO_GRID_ARCHIVES,
    name: "P2_INFO5.BTX",
    storage: TextStorageSpec::FixedCellGrid(FixedCellGridSpec {
        cell_width: 16,
        cell_height: 16,
        expected_active_cells: 94,
        expected_empty_cells: 162,
        expected_unique_glyphs: 48,
        active_palette_indices: PaletteIndexSpec::Values(&P2_INFO_GRID_ACTIVE_PALETTE_INDICES),
        empty_palette_indices: PaletteIndexSpec::Values(&P2_INFO_GRID_EMPTY_PALETTE_INDICES),
    }),
    layout: Layout {
        width: 256,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "16ada14af66f89e2788ad825c805f2d14be59445e138d53950520d184bb912ed",
    blocks: &P2_INFO5_BLOCKS,
};
