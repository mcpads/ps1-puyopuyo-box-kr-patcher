//! 셸과 QUEST의 정적 BTX text asset 계약.

use super::{
    Layout, PaletteIndexSpec, PixelFormat, Rect, TextAssetSpec, TextBlockSpec, TextStorageSpec,
};

const NOWLOAD_ARCHIVES: [&str; 2] = ["/FIL_GRP.VDD;1", "/KOUSIKI/KOU_GRP.VDD;1"];
const NOWLOAD_PALETTE_INDICES: [u8; 9] = [0, 2, 3, 4, 5, 6, 7, 8, 9];
const NOWLOAD_BLOCKS: [TextBlockSpec<'static>; 1] = [TextBlockSpec {
    id: "BTXB-0001",
    source: "NOW LOADING...",
    bbox: Rect {
        x: 34,
        y: 12,
        width: 92,
        height: 10,
    },
    palette_indices: PaletteIndexSpec::Values(&NOWLOAD_PALETTE_INDICES),
    expected_pixels_sha256: "b0a6b5f19531bf86e899d55731dbb8c27a5ebdef5a342ba1e971acc99296189d",
}];

/// 첫 버전 관리 BTX 텍스트 block: 두 `NOWLOAD.BTX` 사본의 동일 내용.
pub const NOWLOAD_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &NOWLOAD_ARCHIVES,
    name: "NOWLOAD.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 128,
        height: 24,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "8999d57cdfe034cafa8ee640d18b0ce213a6729e6a598a0522904e5709e8c311",
    blocks: &NOWLOAD_BLOCKS,
};

const MOPT_ARCHIVES: [&str; 3] = [
    "/FIL_GRP.VDD;1",
    "/PUYO1/PP1_GRP.VDD;1",
    "/PUYO2/PP2_GRP.VDD;1",
];
const MOPT_PALETTE_INDICES: [u8; 14] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
const MOPT_NO_ELEVEN_PALETTE_INDICES: [u8; 13] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13];
const MOPT_NO_TWO_ELEVEN_PALETTE_INDICES: [u8; 12] = [0, 1, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13];
const MOPT_NO_TWO_PALETTE_INDICES: [u8; 13] = [0, 1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
const MOPT_BLOCKS: [TextBlockSpec<'static>; 23] = [
    TextBlockSpec {
        id: "BTXB-0057",
        source: "おぷしょん",
        bbox: Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "c3e53166c811b37bdf58d98f93abf705a3b118bc66c63e2acc3ab2294ac952c5",
    },
    TextBlockSpec {
        id: "BTXB-0058",
        source: "ぷよの回転方向",
        bbox: Rect {
            x: 80,
            y: 0,
            width: 112,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "8b9c4ad4cffb7e91f1cd7ad815222f32a509a6e13eddd2173c6ca6beb3a413c8",
    },
    TextBlockSpec {
        id: "BTXB-0059",
        source: "□右",
        bbox: Rect {
            x: 0,
            y: 16,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "7746ec15937af397d89fde7330c28648323956f6b8210ca0766ea98720fd8d06",
    },
    TextBlockSpec {
        id: "BTXB-0060",
        source: "×左",
        bbox: Rect {
            x: 40,
            y: 16,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_NO_ELEVEN_PALETTE_INDICES),
        expected_pixels_sha256: "bf2af63caa8bcab3e76308221f01d4353c025dc3c3058be31dd7fefd2aed4bb7",
    },
    TextBlockSpec {
        id: "BTXB-0061",
        source: "○右",
        bbox: Rect {
            x: 80,
            y: 16,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "67f4bf62862ef7879ef5e9caab3ce1ca6fa52e4b77cd4206eaa4077dcca8c496",
    },
    TextBlockSpec {
        id: "BTXB-0062",
        source: "□左",
        bbox: Rect {
            x: 112,
            y: 16,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_NO_TWO_ELEVEN_PALETTE_INDICES),
        expected_pixels_sha256: "8c453fff4d142e9ef90a3045eb09fb74aa8f64b4fb7d09a097353d80f6ce8ee8",
    },
    TextBlockSpec {
        id: "BTXB-0063",
        source: "×右",
        bbox: Rect {
            x: 152,
            y: 16,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "fa536bd32747808373eec95a97dcb200ae8cd55111b536be430aff8e70348536",
    },
    TextBlockSpec {
        id: "BTXB-0064",
        source: "○左",
        bbox: Rect {
            x: 192,
            y: 16,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_NO_TWO_PALETTE_INDICES),
        expected_pixels_sha256: "aaafaaaa8a0dae7e031dfaf753bb5f10ff796d6f7f8f6ec530fddaef76a7e3ed",
    },
    TextBlockSpec {
        id: "BTXB-0065",
        source: "サウンド",
        bbox: Rect {
            x: 0,
            y: 32,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "786b52264994bcae6cdeeed9edcebe0a7f2c9a1490deb1e7744d980b0eabc6da",
    },
    TextBlockSpec {
        id: "BTXB-0066",
        source: "ステレオ",
        bbox: Rect {
            x: 64,
            y: 32,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "85c52c9deb518f0dbdf257ac3a9ddc9084be0feeaa5f6e7eab83e650d5b9a885",
    },
    TextBlockSpec {
        id: "BTXB-0067",
        source: "モノラル",
        bbox: Rect {
            x: 128,
            y: 32,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "917d3797d9b66167deddf3f3f806fe54408d8f1352bb0798155b64d1f4afec6c",
    },
    TextBlockSpec {
        id: "BTXB-0068",
        source: "セーブ",
        bbox: Rect {
            x: 192,
            y: 32,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "dddfcced6daf4d15f3bc4089b394a61bb8da07c1ac4f25fb7dd73a69fb9e67ef",
    },
    TextBlockSpec {
        id: "BTXB-0069",
        source: "多人数対戦のコントローラ",
        bbox: Rect {
            x: 0,
            y: 48,
            width: 192,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "54ecda07d0eb193a28c684843a59dc2daa521b169a59a9fa9d2a70ecda784517",
    },
    TextBlockSpec {
        id: "BTXB-0070",
        source: "ロード",
        bbox: Rect {
            x: 192,
            y: 48,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "a9fe6d338f2f928695999de5bcfe7223fbccf2ceed172a22a4a51fbebcf572fe",
    },
    TextBlockSpec {
        id: "BTXB-0071",
        source: "1人に1つ",
        bbox: Rect {
            x: 0,
            y: 64,
            width: 80,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "4aef9f5c9720598458a2dc7383373b5174af53d2e656c8b50f009caa2c629b66",
    },
    TextBlockSpec {
        id: "BTXB-0072",
        source: "2人に1つ",
        bbox: Rect {
            x: 96,
            y: 64,
            width: 80,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "5109442390aff618872c3a324aed5bfab0621f508aaaca8b3aaf2a24c1503807",
    },
    TextBlockSpec {
        id: "BTXB-0073",
        source: "スタッフ",
        bbox: Rect {
            x: 176,
            y: 64,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "6efbdcc5922b08d45c955d56d91e6e591ac1212c0c3ece5c93887bad7ac3f5dd",
    },
    TextBlockSpec {
        id: "BTXB-0074",
        source: "ぷよを回す方向を変更できます。",
        bbox: Rect {
            x: 0,
            y: 80,
            width: 240,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "e321b032a5e2fa269bd256e3da66ce8c8c9f2d7494dd3882d199d7f1b44ab580",
    },
    TextBlockSpec {
        id: "BTXB-0075",
        source: "音声出力設定を変更できます。",
        bbox: Rect {
            x: 0,
            y: 96,
            width: 224,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "96dd9a1c6d6aafd6f869d4290454221b338f7519c5968dd4e82a97f106faf091",
    },
    TextBlockSpec {
        id: "BTXB-0076",
        source: "3人以上で対戦できるモードでの\nコントローラ数を変更できます。",
        bbox: Rect {
            x: 0,
            y: 112,
            width: 240,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "306b255f2b7858134f5627da95fd704e415848a3b6e60fe55e0f73810e6390f2",
    },
    TextBlockSpec {
        id: "BTXB-0077",
        source: "データをセーブします。",
        bbox: Rect {
            x: 0,
            y: 144,
            width: 176,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "746ee284c010e9e3997929c30615c7469d7ccbebc4b4982c4c668c6438265e27",
    },
    TextBlockSpec {
        id: "BTXB-0078",
        source: "データをロードします。",
        bbox: Rect {
            x: 0,
            y: 160,
            width: 176,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "e11c8675c955ad63556a005fb5cb8b773ddf2e8014c24df3c7fcfdbf636c180f",
    },
    TextBlockSpec {
        id: "BTXB-0079",
        source: "スタッフロールを見ることができ\nます。",
        bbox: Rect {
            x: 0,
            y: 176,
            width: 240,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&MOPT_PALETTE_INDICES),
        expected_pixels_sha256: "d53800a069ccb32902857ae11142c307557c7ce40ac8b540b1cf588100843c3c",
    },
];

/// 셸 옵션 atlas에서 판독한 menu label·선택 상태·설명 23개.
pub const MOPT_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &MOPT_ARCHIVES,
    name: "MOPT.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "f1ae953b54d288e5497bd24ffb76d042cd846af441ee533f68ba3f045ace646d",
    blocks: &MOPT_BLOCKS,
};

const MCRD01_ARCHIVES: [&str; 1] = ["/FIL_GRP.VDD;1"];
const MCRD01_LOAD_PALETTE_INDICES: [u8; 14] = [0, 1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 15];
const MCRD01_SAVE_PALETTE_INDICES: [u8; 15] = [0, 1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const MCRD01_INSERT_PALETTE_INDICES: [u8; 15] = [0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const MCRD01_ONE_PALETTE_INDICES: [u8; 9] = [0, 2, 3, 4, 7, 9, 10, 12, 15];
const MCRD01_TWO_PALETTE_INDICES: [u8; 13] = [0, 1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 14, 15];
const MCRD01_BLOCKS: [TextBlockSpec<'static>; 5] = [
    TextBlockSpec {
        id: "BTXB-0080",
        source: "ロード",
        bbox: Rect {
            x: 0,
            y: 96,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MCRD01_LOAD_PALETTE_INDICES),
        expected_pixels_sha256: "c11da4884ef37f8aafba389cc140fa5babc13d3631e256b723c3a08ce067ff28",
    },
    TextBlockSpec {
        id: "BTXB-0081",
        source: "セーブ",
        bbox: Rect {
            x: 48,
            y: 96,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MCRD01_SAVE_PALETTE_INDICES),
        expected_pixels_sha256: "1317f88ee82f2633e56428393ba3c600a55cc92fd7f58cdc56356cf8bca5ab9c",
    },
    TextBlockSpec {
        id: "BTXB-0082",
        source: "差込",
        bbox: Rect {
            x: 96,
            y: 96,
            width: 48,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MCRD01_INSERT_PALETTE_INDICES),
        expected_pixels_sha256: "2aef31a3887773d7420d25942d8161677b0881c30a274679cb99f04b18d78f13",
    },
    TextBlockSpec {
        id: "BTXB-0083",
        source: "1",
        bbox: Rect {
            x: 144,
            y: 96,
            width: 16,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MCRD01_ONE_PALETTE_INDICES),
        expected_pixels_sha256: "02d85460d6b31cf3745bcab9a278aaa949239253e3b82bdd7f63ab0d5fa58d86",
    },
    TextBlockSpec {
        id: "BTXB-0084",
        source: "2",
        bbox: Rect {
            x: 160,
            y: 96,
            width: 16,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MCRD01_TWO_PALETTE_INDICES),
        expected_pixels_sha256: "0bf382a7ee09b796865438ea4518587454ea50b2d4b3b63254336caffd0c6a14",
    },
];

/// memory card UI parts에서 독립된 baked label로 확인한 다섯 상태.
pub const MCRD01_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &MCRD01_ARCHIVES,
    name: "MCRD01.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 128,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "2f4972734790c639a67e7c17255b6c83e5658524a71b4c8ec2ec81483abb675a",
    blocks: &MCRD01_BLOCKS,
};

const RANK_S_ARCHIVES: [&str; 1] = ["/FIL_GRP.VDD;1"];
const RANK_S_JAPANESE_PALETTE_INDICES: [u8; 16] =
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const RANK_S_GOCHA_PALETTE_INDICES: [u8; 14] = [0, 1, 2, 3, 4, 5, 7, 8, 9, 11, 12, 13, 14, 15];
const RANK_S_BLOCKS: [TextBlockSpec<'static>; 16] = [
    TextBlockSpec {
        id: "BTXB-0085",
        source: "甘口コース",
        bbox: Rect {
            x: 0,
            y: 0,
            width: 96,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&RANK_S_JAPANESE_PALETTE_INDICES),
        expected_pixels_sha256: "862aaf84140b91a73520b1cdea15ced093cc2a10e5bd216341af237ad84cd569",
    },
    TextBlockSpec {
        id: "BTXB-0086",
        source: "中辛コース",
        bbox: Rect {
            x: 96,
            y: 0,
            width: 96,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&RANK_S_JAPANESE_PALETTE_INDICES),
        expected_pixels_sha256: "94e0dcd86abc7909b9e3652960bb6e85b8b4544e5be60502c6461f4a0a966320",
    },
    TextBlockSpec {
        id: "BTXB-0087",
        source: "辛口コース",
        bbox: Rect {
            x: 0,
            y: 24,
            width: 96,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&RANK_S_JAPANESE_PALETTE_INDICES),
        expected_pixels_sha256: "7b846701feb0bac7c05b032adab5d3d79e13f7bf28fadc797955aac6024079f3",
    },
    TextBlockSpec {
        id: "BTXB-0088",
        source: "めーきゅう",
        bbox: Rect {
            x: 96,
            y: 24,
            width: 96,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&RANK_S_JAPANESE_PALETTE_INDICES),
        expected_pixels_sha256: "cc2565d54b831a06a663e0c705022e52fdfa7dfb4bcaf4650de848e23fb556e6",
    },
    TextBlockSpec {
        id: "BTXB-0089",
        source: "ふたりぷよ",
        bbox: Rect {
            x: 0,
            y: 48,
            width: 96,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&RANK_S_JAPANESE_PALETTE_INDICES),
        expected_pixels_sha256: "ae55d1299fe73e266d4de188fdbcde6f7d10c895528c0c405f6fa703cc7c40ab",
    },
    TextBlockSpec {
        id: "BTXB-0090",
        source: "3勝戦",
        bbox: Rect {
            x: 96,
            y: 48,
            width: 64,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&RANK_S_JAPANESE_PALETTE_INDICES),
        expected_pixels_sha256: "76486a28e67357c1df7662e061e759b5f3a21ff6c3a9ce5c5102840eadc40aec",
    },
    TextBlockSpec {
        id: "BTXB-0091",
        source: "ふたりぷよ",
        bbox: Rect {
            x: 0,
            y: 72,
            width: 96,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&RANK_S_JAPANESE_PALETTE_INDICES),
        expected_pixels_sha256: "ae55d1299fe73e266d4de188fdbcde6f7d10c895528c0c405f6fa703cc7c40ab",
    },
    TextBlockSpec {
        id: "BTXB-0092",
        source: "1勝戦",
        bbox: Rect {
            x: 96,
            y: 72,
            width: 64,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&RANK_S_JAPANESE_PALETTE_INDICES),
        expected_pixels_sha256: "6bf43df62e75168c8ab899a966fd52305bc0d38a3e89f85b0a3518e72250e2a8",
    },
    TextBlockSpec {
        id: "BTXB-0093",
        source: "ごちゃまぜぷよぷよ",
        bbox: Rect {
            x: 0,
            y: 96,
            width: 176,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&RANK_S_GOCHA_PALETTE_INDICES),
        expected_pixels_sha256: "dacce0d3a1128cdf85e3473affda7f13f6b2e65d576ee8f0ad6b30d4ad3aee43",
    },
    TextBlockSpec {
        id: "BTXB-0094",
        source: "とことんぷよぷよ通",
        bbox: Rect {
            x: 0,
            y: 120,
            width: 176,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&RANK_S_JAPANESE_PALETTE_INDICES),
        expected_pixels_sha256: "85702dbc4a9bc8665fdf50e0a6129bd2b174dd2600e8440f897bda3eb20efe29",
    },
    TextBlockSpec {
        id: "BTXB-0095",
        source: "NAME",
        bbox: Rect {
            x: 176,
            y: 48,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 8, 9, 10]),
        expected_pixels_sha256: "513d8ade72eb4a21ac177cda4cd7bf310b53a9910df2b538a7118044b105aa4f",
    },
    TextBlockSpec {
        id: "BTXB-0096",
        source: "SCORE",
        bbox: Rect {
            x: 176,
            y: 64,
            width: 40,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 8, 9]),
        expected_pixels_sha256: "71cab66025aca4a40fdcd73d86cfe7545131a1174635c7c7bc31e2de98856136",
    },
    TextBlockSpec {
        id: "BTXB-0097",
        source: "BLOCK",
        bbox: Rect {
            x: 176,
            y: 80,
            width: 40,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 8, 9]),
        expected_pixels_sha256: "eec996847ff861dec996aae62983c35e9322da0879990ddab26da32abc2665ba",
    },
    TextBlockSpec {
        id: "BTXB-0098",
        source: "STAGE",
        bbox: Rect {
            x: 176,
            y: 96,
            width: 40,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 8, 9, 10]),
        expected_pixels_sha256: "dfcb11f3a918a15591a12230ec175fe30a608176b1b629f8e4f6faacb211e497",
    },
    TextBlockSpec {
        id: "BTXB-0099",
        source: "RENSA",
        bbox: Rect {
            x: 176,
            y: 112,
            width: 40,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 8, 9, 10]),
        expected_pixels_sha256: "b02cfd62abb910c8eec2563d7635fca972ca1e533e5f1ee3d23af3f1cbe910ab",
    },
    TextBlockSpec {
        id: "BTXB-0100",
        source: "ALL",
        bbox: Rect {
            x: 176,
            y: 128,
            width: 24,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 8, 9, 10]),
        expected_pixels_sha256: "d00bd0435e9a4320eec0d5f9d05533a1213793d820e632392d82f4a57dc1d99b",
    },
];

/// 셸 ranking atlas의 course·mode·score heading 16개.
pub const RANK_S_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &RANK_S_ARCHIVES,
    name: "RANK_S.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "997b4eec5269cbbf80a63ef894f2162ed829175705061454ee11c5beb1beb46d",
    blocks: &RANK_S_BLOCKS,
};

const MENU02_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];
const MENU02_HEADING_PALETTE_INDICES: [u8; 8] = [1, 2, 4, 5, 8, 9, 12, 15];
const MENU02_TITLE_PALETTE_INDICES: [u8; 9] = [0, 1, 2, 4, 5, 8, 9, 12, 15];
const MENU02_BLOCKS: [TextBlockSpec<'static>; 4] = [
    TextBlockSpec {
        id: "BTXB-0101",
        source: "NAME",
        bbox: Rect {
            x: 0,
            y: 32,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU02_HEADING_PALETTE_INDICES),
        expected_pixels_sha256: "1ef955f6697908e46f901d3e890f28f559e802efd138f1a5a70766a9e91eb883",
    },
    TextBlockSpec {
        id: "BTXB-0102",
        source: "SCORE",
        bbox: Rect {
            x: 64,
            y: 32,
            width: 80,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU02_HEADING_PALETTE_INDICES),
        expected_pixels_sha256: "e1648b55952829e5f1d09e5f1602e469ec38f0c7062c71270b14e50cc88acce7",
    },
    TextBlockSpec {
        id: "BTXB-0103",
        source: "BLOCK",
        bbox: Rect {
            x: 144,
            y: 32,
            width: 80,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU02_HEADING_PALETTE_INDICES),
        expected_pixels_sha256: "4f588d66115e85c2b7c2ddf13ec9f6b2181e17c5736f9d502e12efc6d5008c4f",
    },
    TextBlockSpec {
        id: "BTXB-0104",
        source: "BEST RECORD",
        bbox: Rect {
            x: 0,
            y: 64,
            width: 256,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&MENU02_TITLE_PALETTE_INDICES),
        expected_pixels_sha256: "bc56a1c952c987a116183528323df08f91b7afda6e8d228117d1998eb7153660",
    },
];

/// 복각 `ぷよぷよ`의 score heading과 `BEST RECORD` title 네 개.
pub const MENU02_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &MENU02_ARCHIVES,
    name: "MENU02.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "6770ed9329d693aa50ed033369bda8826a6a875ebb7ab00cb1b8c882c49150e6",
    blocks: &MENU02_BLOCKS,
};

const MM_SPR_ARCHIVES: [&str; 1] = ["/FIL_GRP.VDD;1"];
const MM_SPR_PALETTE_INDICES: [u8; 14] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
const MM_SPR_LATIN_PALETTE_INDICES: [u8; 2] = [1, 5];
const MM_SPR_LATIN_WITH_ZERO_PALETTE_INDICES: [u8; 3] = [0, 1, 5];
const MM_SPR_LATIN_WITH_EDGE_PALETTE_INDICES: [u8; 4] = [1, 2, 5, 6];
const MM_SPR_BLOCKS: [TextBlockSpec<'static>; 25] = [
    TextBlockSpec {
        id: "BTXB-0002",
        source: "「ぷよぷよ」で遊べるよ。",
        bbox: Rect {
            x: 537,
            y: 0,
            width: 182,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "f29837f3c44e954774a66b8567b1b49a223b7f6f16f3d27dd8f2753ef266b1dc",
    },
    TextBlockSpec {
        id: "BTXB-0003",
        source: "「ぷよぷよ通」で遊べるよ。",
        bbox: Rect {
            x: 529,
            y: 16,
            width: 198,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "5caffff2c428dad4b9650be0ac85266369fcb3097385176133f10c2d2ac02e07",
    },
    TextBlockSpec {
        id: "BTXB-0004",
        source: "対戦専用のモードだよ。",
        bbox: Rect {
            x: 544,
            y: 32,
            width: 167,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "ac908d58d31578fb153f9fd8a5d00e6982a58f6ccde076e69e4b8416819d1bee",
    },
    TextBlockSpec {
        id: "BTXB-0005",
        source: "いろんなぷよぷよが遊べるよ。",
        bbox: Rect {
            x: 528,
            y: 48,
            width: 215,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "cb817b6fe4497ab4c2eb28448ce717a35ae635d1c94bc3b5cde3d3d67ed5533b",
    },
    TextBlockSpec {
        id: "BTXB-0006",
        source: "コミュニケーションツールだよ。",
        bbox: Rect {
            x: 520,
            y: 64,
            width: 231,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "000c091c2b34d86e319f3c484217ebb34778c20881b5ac34805bdb4b85fd8810",
    },
    TextBlockSpec {
        id: "BTXB-0007",
        source: "設定が変更できるよ。",
        bbox: Rect {
            x: 552,
            y: 80,
            width: 151,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "027432d92f42cb6aa61a59c99aa3a759961edd8e5ecff837e2d22fdde076fb68",
    },
    TextBlockSpec {
        id: "BTXB-0008",
        source: "今までのぷよのルールが\nひとつになったモードだよ。",
        bbox: Rect {
            x: 768,
            y: 0,
            width: 199,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "cdd95759ff11ff6e23d31b000bfa245f921d9c95f5a1e5fbc477a6438e03d6b7",
    },
    TextBlockSpec {
        id: "BTXB-0009",
        source: "ぷよぷよで対戦しながら\nぷよの世界を冒険できるよ。",
        bbox: Rect {
            x: 768,
            y: 32,
            width: 199,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "b612c051f582d429e1993a54b0e909b6a4c0e44a1b43bbb044ec03c4caa9115a",
    },
    TextBlockSpec {
        id: "BTXB-0010",
        source: "ぷよぷよで対戦しながら\nぷよの世界を冒険できるよ。",
        bbox: Rect {
            x: 768,
            y: 64,
            width: 199,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "b612c051f582d429e1993a54b0e909b6a4c0e44a1b43bbb044ec03c4caa9115a",
    },
    TextBlockSpec {
        id: "BTXB-0011",
        source: "ぷよぷよで対戦しながら\nぷよの世界を冒険できるよ。",
        bbox: Rect {
            x: 768,
            y: 96,
            width: 199,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "b612c051f582d429e1993a54b0e909b6a4c0e44a1b43bbb044ec03c4caa9115a",
    },
    TextBlockSpec {
        id: "BTXB-0012",
        source: "1人で遊べるよ。",
        bbox: Rect {
            x: 516,
            y: 128,
            width: 115,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "53686a101b7468f7764692eeb23df4a906a2387e29616312622709bb1f0e2242",
    },
    TextBlockSpec {
        id: "BTXB-0013",
        source: "2人で遊べるよ。",
        bbox: Rect {
            x: 514,
            y: 160,
            width: 117,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "30fbeef2d86cecae1e9fe07c866e20429413725f0837ff5fc5fc6a74baa0f95d",
    },
    TextBlockSpec {
        id: "BTXB-0014",
        source: "4人までいっしょに\n遊べるよ。\nハイスコアが見れるよ。",
        bbox: Rect {
            x: 512,
            y: 192,
            width: 167,
            height: 48,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "00944dae9c39c799891d46426c486f1b0f2d52663dfb9e6dda356608b89dca1a",
    },
    TextBlockSpec {
        id: "BTXB-0015",
        source: "いろんなルールを使って\n2人で対戦できるよ。",
        bbox: Rect {
            x: 768,
            y: 128,
            width: 176,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "072ea7d63e543d34507c895d68835027b5c5ba6cbe778ba3b523283da6486a2d",
    },
    TextBlockSpec {
        id: "BTXB-0016",
        source: "ぷよにうもれた宝箱を消す\nスピードを競うモードだよ。",
        bbox: Rect {
            x: 768,
            y: 160,
            width: 199,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "3b899e695765ceee1c815c52e4e9d079f90b6557b7dca23f7616ef2742e6eaf0",
    },
    TextBlockSpec {
        id: "BTXB-0017",
        source: "いろんなルールを使って\n4人まで対戦できるよ。",
        bbox: Rect {
            x: 768,
            y: 192,
            width: 176,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "56f7402120c5e4c050ca2c32e9b217e4f617ff8cbe55941f5756964c21efba09",
    },
    TextBlockSpec {
        id: "BTXB-0018",
        source: "ぷよにうもれた宝箱を消す\nスピードを競うモードだよ。",
        bbox: Rect {
            x: 768,
            y: 224,
            width: 199,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_PALETTE_INDICES),
        expected_pixels_sha256: "3b899e695765ceee1c815c52e4e9d079f90b6557b7dca23f7616ef2742e6eaf0",
    },
    TextBlockSpec {
        id: "BTXB-0048",
        source: "MAIN MENU",
        bbox: Rect {
            x: 84,
            y: 128,
            width: 36,
            height: 20,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_LATIN_PALETTE_INDICES),
        expected_pixels_sha256: "7d076a7168f273d03d60ec4d8b62c930f633e94c42c6ba53eff16387aff7ea52",
    },
    TextBlockSpec {
        id: "BTXB-0049",
        source: "PUYO PUYO BOX",
        bbox: Rect {
            x: 140,
            y: 152,
            width: 84,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_LATIN_PALETTE_INDICES),
        expected_pixels_sha256: "b43e84a24ae1bb8197d676f00e166f2cce4cc5bc2394446025fa283219525332",
    },
    TextBlockSpec {
        id: "BTXB-0050",
        source: "PUYO BOX",
        bbox: Rect {
            x: 340,
            y: 148,
            width: 40,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_LATIN_PALETTE_INDICES),
        expected_pixels_sha256: "28900307f3db86ce06ca3809598ba353855c0afd315d67a44e0d02e9271a3d6d",
    },
    TextBlockSpec {
        id: "BTXB-0051",
        source: "PUYO PUYO BOX",
        bbox: Rect {
            x: 396,
            y: 176,
            width: 84,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_LATIN_PALETTE_INDICES),
        expected_pixels_sha256: "b43e84a24ae1bb8197d676f00e166f2cce4cc5bc2394446025fa283219525332",
    },
    TextBlockSpec {
        id: "BTXB-0052",
        source: "MAIN MENU",
        bbox: Rect {
            x: 204,
            y: 188,
            width: 36,
            height: 20,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_LATIN_PALETTE_INDICES),
        expected_pixels_sha256: "7d076a7168f273d03d60ec4d8b62c930f633e94c42c6ba53eff16387aff7ea52",
    },
    TextBlockSpec {
        id: "BTXB-0053",
        source: "PUYO PUYO BOX",
        bbox: Rect {
            x: 16,
            y: 212,
            width: 88,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_LATIN_PALETTE_INDICES),
        expected_pixels_sha256: "3bdeec1352fb664ceb8fb8587fb20b89bb2ebfd243aa76ffea8026fcf65528a2",
    },
    TextBlockSpec {
        id: "BTXB-0054",
        source: "PUYO BOX",
        bbox: Rect {
            x: 460,
            y: 212,
            width: 40,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_LATIN_WITH_ZERO_PALETTE_INDICES),
        expected_pixels_sha256: "4d5df91c1ee8db2ff0d47e6405329b0bd20b65714596a2e725db114d131b0db6",
    },
    TextBlockSpec {
        id: "BTXB-0055",
        source: "PUYO PUYO BOX",
        bbox: Rect {
            x: 272,
            y: 232,
            width: 88,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&MM_SPR_LATIN_WITH_EDGE_PALETTE_INDICES),
        expected_pixels_sha256: "d5a662c473dbad130c167e07a7384e986c350fd05558297ef6d29a0340910aaf",
    },
];

/// 셸 menu 설명문과 Latin sprite label 25개가 있는 `MM_SPR.BTX`.
pub const MM_SPR_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &MM_SPR_ARCHIVES,
    name: "MM_SPR.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 1024,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "adf11001e6cf3ab3e62168d44324d55ad9e70eb272cf48a3776b8cf53aa69da0",
    blocks: &MM_SPR_BLOCKS,
};

const TITLEB_ARCHIVES: [&str; 1] = ["/FIL_GRP.VDD;1"];
const TITLEB_FULL_PALETTE_INDICES: [u8; 16] =
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const TITLEB_PLAYSTATION_PALETTE_INDICES: [u8; 14] =
    [0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 15];
const TITLEB_PASSTHROUGH_PALETTE_INDICES: [u8; 12] = [0, 1, 2, 4, 5, 7, 8, 10, 11, 12, 14, 15];
const TITLEB_COMPILE_PALETTE_INDICES: [u8; 2] = [0, 2];
const TITLEB_BLOCKS: [TextBlockSpec<'static>; 4] = [
    TextBlockSpec {
        id: "BTXB-0019",
        source: "PLAYSTATION",
        bbox: Rect {
            x: 152,
            y: 1,
            width: 80,
            height: 14,
        },
        palette_indices: PaletteIndexSpec::Values(&TITLEB_FULL_PALETTE_INDICES),
        expected_pixels_sha256: "82e8378baa3f1427fb9856f7c60da2f6e4d8ed0ee96a24acaceaae669c42434d",
    },
    TextBlockSpec {
        id: "BTXB-0020",
        source: "PLAYSTATION",
        bbox: Rect {
            x: 152,
            y: 18,
            width: 80,
            height: 11,
        },
        palette_indices: PaletteIndexSpec::Values(&TITLEB_PLAYSTATION_PALETTE_INDICES),
        expected_pixels_sha256: "acdac27e7f7d94fdb2783806d64c4dcf7e919ccf8ea0c55f7ed7637c0749c000",
    },
    TextBlockSpec {
        id: "BTXB-0021",
        source: "PASSTHROUGH",
        bbox: Rect {
            x: 152,
            y: 38,
            width: 80,
            height: 9,
        },
        palette_indices: PaletteIndexSpec::Values(&TITLEB_PASSTHROUGH_PALETTE_INDICES),
        expected_pixels_sha256: "ba499be4015b268c470d268ef5d51e3ece08bf1622201979566ed554f3f80c8f",
    },
    TextBlockSpec {
        id: "BTXB-0056",
        source: "COMPILE",
        bbox: Rect {
            x: 392,
            y: 96,
            width: 80,
            height: 88,
        },
        palette_indices: PaletteIndexSpec::Values(&TITLEB_COMPILE_PALETTE_INDICES),
        expected_pixels_sha256: "ec34efab648f9809ad8bf17fe2d25aa8886e469980107952b836671a74575589",
    },
];

/// 셸 presentation sprite의 독립된 Latin 텍스트 4개.
pub const TITLEB_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &TITLEB_ARCHIVES,
    name: "TITLEB.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 512,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "b0782242fbd7d661d6d0d1213d005e6dad0d8eb83d998f072d3694137de49dad",
    blocks: &TITLEB_BLOCKS,
};

const BOXCUBE_ARCHIVES: [&str; 2] = ["/FIL_GRP.VDD;1", "/PUYO1/PP1_GRP.VDD;1"];
const BOXCUBE_GOCHA_PALETTE_INDICES: [u64; 4] = [
    0x5185_38C3_0808_0001,
    0xA29C_380C_28B0_A869,
    0x4203_0B88_E2D8_6218,
    0xFD12_A410_4904_4210,
];
const BOXCUBE_QUEST_PALETTE_INDICES: [u64; 4] = [
    0x0A18_4124_52E3_9C2F,
    0x8A31_0E9A_8C13_0A32,
    0x484A_5521_A241_2048,
    0xED12_A610_492D_4614,
];
const BOXCUBE_ENDLESS_PALETTE_INDICES: [u64; 4] = [
    0x8D2C_D43C_3367_5A7F,
    0x8654_8C43_1B5A_5414,
    0xD4A2_2102_AE20_A88C,
    0xFD12_A610_4904_4A9A,
];
const BOXCUBE_HORI_PALETTE_INDICES: [u64; 4] = [
    0x290E_52A4_9E31_392F,
    0xC316_0D2A_4816_01B1,
    0x4102_8164_B346_3128,
    0xFFFF_BF5E_ED14_6311,
];
const BOXCUBE_BOMB_PALETTE_INDICES: [u64; 4] = [
    0x8C28_C43C_3367_5A7F,
    0x8644_8C43_135A_5414,
    0xD4A2_2122_8E60_A88C,
    0xFD12_A610_4904_4A9A,
];
const BOXCUBE_TRAINING_PALETTE_INDICES: [u64; 4] = [
    0x290E_52A4_9E31_392F,
    0xD316_0D2A_4816_09B1,
    0x4102_8144_9346_3120,
    0xFFD7_B75A_E954_6331,
];
const BOXCUBE_BLOCKS: [TextBlockSpec<'static>; 6] = [
    TextBlockSpec {
        id: "BTXB-0022",
        source: "ごちゃまぜ",
        bbox: Rect {
            x: 0,
            y: 14,
            width: 64,
            height: 20,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXCUBE_GOCHA_PALETTE_INDICES),
        expected_pixels_sha256: "26f6f6af1444718156be285ff7a42d3a0f717c801a0067e93f9b90ccd512ecdb",
    },
    TextBlockSpec {
        id: "BTXB-0023",
        source: "クエスト",
        bbox: Rect {
            x: 64,
            y: 14,
            width: 64,
            height: 20,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXCUBE_QUEST_PALETTE_INDICES),
        expected_pixels_sha256: "9369dc21e4ae0a5187d190537123baf5e537d791c355f8c73146f708f8166455",
    },
    TextBlockSpec {
        id: "BTXB-0024",
        source: "とこぷよ通",
        bbox: Rect {
            x: 0,
            y: 62,
            width: 64,
            height: 20,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXCUBE_ENDLESS_PALETTE_INDICES),
        expected_pixels_sha256: "e745c516d7ab2e2123d480264374f8cdc48188e01a9edd4a39449e0674ba3e59",
    },
    TextBlockSpec {
        id: "BTXB-0025",
        source: "ほりぷよ",
        bbox: Rect {
            x: 64,
            y: 62,
            width: 64,
            height: 20,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXCUBE_HORI_PALETTE_INDICES),
        expected_pixels_sha256: "5d5c52f514dca68cd1986b4a451d10aacf48e0270c689512fa3e27fc541ee599",
    },
    TextBlockSpec {
        id: "BTXB-0026",
        source: "ばくぷよ通",
        bbox: Rect {
            x: 0,
            y: 110,
            width: 64,
            height: 20,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXCUBE_BOMB_PALETTE_INDICES),
        expected_pixels_sha256: "020379b01ab27a01642b081adb6872341da9fd1560e23ccd3a52f49d3a42d9a4",
    },
    TextBlockSpec {
        id: "BTXB-0027",
        source: "とれーにんぐ",
        bbox: Rect {
            x: 0,
            y: 158,
            width: 80,
            height: 20,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXCUBE_TRAINING_PALETTE_INDICES),
        expected_pixels_sha256: "46d981f4df80cf3608e3bad6038ecb556477b85c73ea18f8cc779804a2bcf3a9",
    },
];

/// 셸 mode cube에 들어 있는 일본어 label 6개.
pub const BOXCUBE_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &BOXCUBE_ARCHIVES,
    name: "BOXCUBE.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 128,
        height: 192,
        format: PixelFormat::Indexed8,
    },
    expected_decompressed_sha256:
        "ac2af240e70f50c49aa2b93924bbd3eb0b876ba564f1e12ff933366b95fa6da1",
    blocks: &BOXCUBE_BLOCKS,
};

const BOXPARTS_ARCHIVES: [&str; 1] = ["/FIL_GRP.VDD;1"];
const BOXPARTS_LOGO_PALETTE_INDICES: [u64; 4] = [
    0x6F9F_FFBF_D7FF_FFFF,
    0xF766_77D9_CA79_4F5B,
    0xFB6E_7F5F_7D8D_FB45,
    0xD5AD_FE9D_B759_D77C,
];
const BOXPARTS_ONE_MAIN_PALETTE_INDICES: [u64; 4] = [
    0xAD1A_D73B_9574_EBDF,
    0x2540_9EC8_0652_C519,
    0x9A6E_3D4F_6888_E801,
    0xA919_6CD4_B658_C178,
];
const BOXPARTS_TWO_MAIN_PALETTE_INDICES: [u64; 4] = [
    0x6D1A_571B_1574_EBDF,
    0x0340_92C8_0241_C519,
    0xAA04_384F_2088_2901,
    0xA910_4440_34DA_1358,
];
const BOXPARTS_ALL_MAIN_PALETTE_INDICES: [u64; 4] = [
    0x6D1A_D71B_9574_EBDF,
    0x2740_9AC8_8253_C519,
    0xAA64_3C4F_6088_6901,
    0xA910_4454_B458_5150,
];
const BOXPARTS_RANKING_MAIN_PALETTE_INDICES: [u64; 4] = [
    0x651B_D71B_1D5C_EBDF,
    0x2740_9EC8_8250_C59D,
    0xBA4E_3D4F_6888_E901,
    0xA911_6CD4_B658_C378,
];
const BOXPARTS_ONE_SUB_PALETTE_INDICES: [u64; 4] = [
    0xAD1A_D33B_9574_EBDF,
    0x0540_1CC8_0612_C419,
    0x9A2E_394F_2888_A801,
    0xA919_6CD4_B658_C170,
];
const BOXPARTS_TWO_SUB_PALETTE_INDICES: [u64; 4] = [
    0x6D12_571B_9574_EBDF,
    0x0340_10C8_0201_C519,
    0xAA04_384F_2088_2901,
    0xA910_4440_34DA_1358,
];
const BOXPARTS_ALL_SUB_PALETTE_INDICES: [u64; 4] = [
    0x6D1A_D71B_9574_EBDF,
    0x0740_18C8_8253_C419,
    0x8A24_384F_2088_2901,
    0xA910_4454_B458_5150,
];
const BOXPARTS_RANKING_SUB_PALETTE_INDICES: [u64; 4] = [
    0x611A_D71A_1554_EBDF,
    0x0740_1CC8_8210_C419,
    0x9A0E_394F_2888_A901,
    0xA911_6CD4_B658_C170,
];
const BOXPARTS_BLOCKS: [TextBlockSpec<'static>; 9] = [
    TextBlockSpec {
        id: "BTXB-0028",
        source: "ぷよBOX",
        bbox: Rect {
            x: 0,
            y: 0,
            width: 152,
            height: 36,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXPARTS_LOGO_PALETTE_INDICES),
        expected_pixels_sha256: "e33dbb3293882680c7ac270fbeb558c7681abd8179dde7cbace8dd4025c8ecdc",
    },
    TextBlockSpec {
        id: "BTXB-0029",
        source: "ひとり",
        bbox: Rect {
            x: 16,
            y: 36,
            width: 112,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXPARTS_ONE_MAIN_PALETTE_INDICES),
        expected_pixels_sha256: "3faf7111de476bf5245f6d9f2d7c8017aec2a8b1db128f14f4473346c4e31143",
    },
    TextBlockSpec {
        id: "BTXB-0030",
        source: "ふたり",
        bbox: Rect {
            x: 8,
            y: 60,
            width: 120,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXPARTS_TWO_MAIN_PALETTE_INDICES),
        expected_pixels_sha256: "8c38be9d2163b960c3bbe9a5ee85cdb3f8c6b6f9ff1feba24a94a05d4107c47a",
    },
    TextBlockSpec {
        id: "BTXB-0031",
        source: "みんな",
        bbox: Rect {
            x: 0,
            y: 84,
            width: 120,
            height: 28,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXPARTS_ALL_MAIN_PALETTE_INDICES),
        expected_pixels_sha256: "2d25f5c19c0934a58032184ce18c5459b635a528b7b8e56f230c9d58404be7b2",
    },
    TextBlockSpec {
        id: "BTXB-0032",
        source: "らんきんぐ",
        bbox: Rect {
            x: 8,
            y: 112,
            width: 120,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXPARTS_RANKING_MAIN_PALETTE_INDICES),
        expected_pixels_sha256: "b1f2531f23e8704062f97d4cb53b4893867e499716122dfbc8c9b64ffb319dcc",
    },
    TextBlockSpec {
        id: "BTXB-0033",
        source: "ひとり",
        bbox: Rect {
            x: 0,
            y: 208,
            width: 112,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXPARTS_ONE_SUB_PALETTE_INDICES),
        expected_pixels_sha256: "d24589e6c0f8ac45083f05025075c041dcf407249298a123daa9972f2f61ef55",
    },
    TextBlockSpec {
        id: "BTXB-0034",
        source: "ふたり",
        bbox: Rect {
            x: 0,
            y: 232,
            width: 120,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXPARTS_TWO_SUB_PALETTE_INDICES),
        expected_pixels_sha256: "b5f7c7190bdc45467cabef0ec2ceff8b517279e78657a2c69c97a737eecf1dae",
    },
    TextBlockSpec {
        id: "BTXB-0035",
        source: "みんな",
        bbox: Rect {
            x: 128,
            y: 200,
            width: 112,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXPARTS_ALL_SUB_PALETTE_INDICES),
        expected_pixels_sha256: "6fa6de690be92f25a8c4ea0a5460459345f2d3e5ee8dac617cd5a3c86732912e",
    },
    TextBlockSpec {
        id: "BTXB-0036",
        source: "らんきんぐ",
        bbox: Rect {
            x: 128,
            y: 232,
            width: 112,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Bitset256(BOXPARTS_RANKING_SUB_PALETTE_INDICES),
        expected_pixels_sha256: "904e197d9b12889db3a9f2900195a7b53c254418da708f5e8096e2c97ab9ec40",
    },
];

/// 셸 menu parts의 logo와 일본어 label 상태 9개.
pub const BOXPARTS_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &BOXPARTS_ARCHIVES,
    name: "BOXPARTS.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 256,
        format: PixelFormat::Indexed8,
    },
    expected_decompressed_sha256:
        "910e4e8f6f0b5586abeef9c7bb20a8c2ff7bd463ce7c693d387d74234f3cd7af",
    blocks: &BOXPARTS_BLOCKS,
};

const TITLEA_ARCHIVES: [&str; 1] = ["/FIL_GRP.VDD;1"];
const TITLEA_LOGO_LARGE_PALETTE_INDICES: [u64; 4] = [
    0x1928_8C13_2C94_0001,
    0x2510_DA55_6032_A229,
    0x0018_1012_4484_8283,
    0xEA28_8300_8441_2209,
];
const TITLEA_LOGO_SMALL_PALETTE_INDICES: [u64; 4] = [
    0x1928_0C13_2C94_0001,
    0x2410_CA51_6032_A229,
    0x0008_1002_4084_8283,
    0x2A28_0100_8401_2209,
];
const TITLEA_COPYRIGHT_PALETTE_INDICES: [u64; 4] = [
    0x0001_8000_6009_493F,
    0x0800_0020_0000_0400,
    0x0010_0800_0400_0400,
    0x0220_8200_8040_2008,
];
const TITLEA_YEAR_PALETTE_INDICES: [u64; 4] = [
    0x0001_8000_2008_493F,
    0x0800_0020_0000_0400,
    0x0000_0800_0400_0400,
    0x0200_8200_8040_2000,
];
const TITLEA_BLOCKS: [TextBlockSpec<'static>; 4] = [
    TextBlockSpec {
        id: "BTXB-0037",
        source: "ぷよぷよ / PUYO PUYO BOX",
        bbox: Rect {
            x: 0,
            y: 24,
            width: 168,
            height: 64,
        },
        palette_indices: PaletteIndexSpec::Bitset256(TITLEA_LOGO_LARGE_PALETTE_INDICES),
        expected_pixels_sha256: "a64f82ed9de731dd4948d41ad399874c2faec1501b9d094dc7223fc6ab5edb34",
    },
    TextBlockSpec {
        id: "BTXB-0038",
        source: "ぷよぷよ / PUYO PUYO BOX",
        bbox: Rect {
            x: 0,
            y: 96,
            width: 176,
            height: 64,
        },
        palette_indices: PaletteIndexSpec::Bitset256(TITLEA_LOGO_SMALL_PALETTE_INDICES),
        expected_pixels_sha256: "9dc8cd918bd752de5fa141c5481f1134565c77a8b054b7d78fdb2e9c9bca5a50",
    },
    TextBlockSpec {
        id: "BTXB-0039",
        source: "Developed by COMPILE 1991-2000 / (C) SEGA CORPORATION",
        bbox: Rect {
            x: 0,
            y: 224,
            width: 128,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Bitset256(TITLEA_COPYRIGHT_PALETTE_INDICES),
        expected_pixels_sha256: "a5780734b6d723bafa2f857fc9dcee15513fb5014dda3dc1a45f0b25151da4d3",
    },
    TextBlockSpec {
        id: "BTXB-0040",
        source: "-2000",
        bbox: Rect {
            x: 352,
            y: 232,
            width: 32,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Bitset256(TITLEA_YEAR_PALETTE_INDICES),
        expected_pixels_sha256: "ba9722c0fe5f588591fd161102593e08af5ab192d427eeb37869e4555f8da8ad",
    },
];

/// 셸 title atlas에서 원문을 확정한 logo와 copyright 상태.
pub const TITLEA_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &TITLEA_ARCHIVES,
    name: "TITLEA.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 384,
        height: 256,
        format: PixelFormat::Indexed8,
    },
    expected_decompressed_sha256:
        "a0de62287cec5c8277d2e10553a5f854d6072ab9aacdee64d2ffa6f637674622",
    blocks: &TITLEA_BLOCKS,
};

const MMENU_ARCHIVES: [&str; 1] = ["/FIL_GRP.VDD;1"];
const MMENU_TITLE_PALETTE_INDICES: [u64; 4] = [
    0x0000_01B7_F12F_5EAB,
    0x0000_0010_003E_0000,
    0xA03C_C11B_B6D2_0000,
    0x1024_0154_00FE_1F83,
];
const MMENU_PUYO_PALETTE_INDICES: [u64; 4] = [
    0xCFBF_FFFF_F000_0000,
    0x0030_8058_DFFF_FF3F,
    0x7FC3_1EC0_FEB0_E006,
    0x07FF_FFFF_FF91_F0FC,
];
const MMENU_TSU_PALETTE_INDICES: [u64; 4] = [
    0xC8B5_FFFF_F000_0000,
    0x0030_0018_57FF_FF3F,
    0x7FC3_1E80_0600_2006,
    0x07FF_FFFF_FF91_F1FC,
];
const MMENU_OPTION_PALETTE_INDICES: [u64; 4] = [
    0xCFBA_FFFF_F007_43B7,
    0x0010_EC18_7FFF_FF3F,
    0xFFF3_7EDD_FDF3_2006,
    0x07FF_FFFF_FFDF_FEFF,
];
const MMENU_OFFICIAL_PALETTE_INDICES: [u64; 4] = [
    0xCDA6_FFFF_F000_0000,
    0x0020_8078_5FFF_FF3F,
    0x7FC3_1E80_0610_6006,
    0x07FF_FFFF_FF91_F17C,
];
const MMENU_BOX_PALETTE_INDICES: [u64; 4] = [
    0xC1A6_FDFF_F000_0000,
    0x0010_0049_DFFF_FF3A,
    0x7FC3_1E80_0002_2000,
    0x07FF_FFFF_FF91_F1FC,
];
const MMENU_COMI_PALETTE_INDICES: [u64; 4] = [
    0xCFB6_FDFF_F000_0000,
    0x0010_8019_57FF_FF3F,
    0x7FC3_1EDC_FDD2_E00E,
    0x07FF_FFFF_FF91_F0FC,
];
const MMENU_BLOCKS: [TextBlockSpec<'static>; 7] = [
    TextBlockSpec {
        id: "BTXB-0041",
        source: "MAIN MENU",
        bbox: Rect {
            x: 32,
            y: 0,
            width: 224,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Bitset256(MMENU_TITLE_PALETTE_INDICES),
        expected_pixels_sha256: "0f1f9d44cf0d4e32e494c53a91f72b10fd34a69a991cccf94a07731621252ee6",
    },
    TextBlockSpec {
        id: "BTXB-0042",
        source: "ぷよぷよ",
        bbox: Rect {
            x: 48,
            y: 40,
            width: 72,
            height: 48,
        },
        palette_indices: PaletteIndexSpec::Bitset256(MMENU_PUYO_PALETTE_INDICES),
        expected_pixels_sha256: "811c14c32b8b7c91c459aa61c2145e62116f3b385a402ce9d34b94cba5067544",
    },
    TextBlockSpec {
        id: "BTXB-0043",
        source: "ぷよ通",
        bbox: Rect {
            x: 128,
            y: 40,
            width: 64,
            height: 48,
        },
        palette_indices: PaletteIndexSpec::Bitset256(MMENU_TSU_PALETTE_INDICES),
        expected_pixels_sha256: "6f66b11651850514bfaed0c5fd263b98589b3198b5f38f18277282223c4c0f1c",
    },
    TextBlockSpec {
        id: "BTXB-0044",
        source: "おぷしょん",
        bbox: Rect {
            x: 192,
            y: 48,
            width: 56,
            height: 40,
        },
        palette_indices: PaletteIndexSpec::Bitset256(MMENU_OPTION_PALETTE_INDICES),
        expected_pixels_sha256: "ba3b1f74ba2aa146a4f8b99b06961df7a6b0b865c290df93a49dde4c8f57c837",
    },
    TextBlockSpec {
        id: "BTXB-0045",
        source: "公式戦",
        bbox: Rect {
            x: 72,
            y: 104,
            width: 64,
            height: 48,
        },
        palette_indices: PaletteIndexSpec::Bitset256(MMENU_OFFICIAL_PALETTE_INDICES),
        expected_pixels_sha256: "35b852f8434745817fe77c7659935293dc6421ae5a0e5a1f7cfce74796bff2a8",
    },
    TextBlockSpec {
        id: "BTXB-0046",
        source: "ぷよBOX",
        bbox: Rect {
            x: 144,
            y: 104,
            width: 64,
            height: 48,
        },
        palette_indices: PaletteIndexSpec::Bitset256(MMENU_BOX_PALETTE_INDICES),
        expected_pixels_sha256: "641ef18c2ae2f377be05abfa0f88de70cda74ef0b4d1f06ac5f4752ce477f9e7",
    },
    TextBlockSpec {
        id: "BTXB-0047",
        source: "ぷよコミ！",
        bbox: Rect {
            x: 208,
            y: 104,
            width: 72,
            height: 48,
        },
        palette_indices: PaletteIndexSpec::Bitset256(MMENU_COMI_PALETTE_INDICES),
        expected_pixels_sha256: "c853cb8966c564fc7ad9fa35f1c5341782751c7271bfe3f8a9c6fa5502242fab",
    },
];

/// 셸 main menu의 현재 표시 상태 title과 mode label.
pub const MMENU_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &MMENU_ARCHIVES,
    name: "MMENU.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 640,
        height: 256,
        format: PixelFormat::Indexed8,
    },
    expected_decompressed_sha256:
        "382f3bfb9b12dad8065c09367893593834eb28dd9904b1389ffce3c5988319ea",
    blocks: &MMENU_BLOCKS,
};

const RPG_SYS1_ARCHIVES: [&str; 1] = ["/QUEST/RPG_GRP.VDD;1"];
const RPG_SYS1_PALETTE_INDICES: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const RPG_SYS1_BLOCKS: [TextBlockSpec<'static>; 1] = [TextBlockSpec {
    id: "BTXB-0105",
    source: "レベルアップ！",
    bbox: Rect {
        x: 128,
        y: 104,
        width: 96,
        height: 24,
    },
    palette_indices: PaletteIndexSpec::Values(&RPG_SYS1_PALETTE_INDICES),
    expected_pixels_sha256: "eeb8b2fe77413c65123af73e41a7885f18e1644beb3e26260dbc5710278f2d64",
}];

/// QUEST system atlas에서 분리한 baked level-up label.
pub const RPG_SYS1_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &RPG_SYS1_ARCHIVES,
    name: "RPG_SYS1.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "f3b06741f40e80c4bb1b65f3d165a66bf0990aa737d6a98899ebe85c912e2d6b",
    blocks: &RPG_SYS1_BLOCKS,
};
