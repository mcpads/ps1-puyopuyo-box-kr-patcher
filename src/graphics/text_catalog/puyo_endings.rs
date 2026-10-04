//! PUYO1/PUYO2 결과·엔딩 BTX의 raster block 계약.
//!
//! 같은 픽셀을 공유하는 두 물리 사본의 소비 증거는 서로 일반화하지 않는다.
//! 이 모듈은 사람이 판정한 block과 원본 픽셀만 고정하며, 사본별 실행 근거는
//! 별도 provenance와 runtime 관측이 소유한다.

use super::{
    Layout, PaletteIndexSpec, PixelFormat, Rect, TextAssetSpec, TextBlockSpec, TextStorageSpec,
};

const PUYO_ARCHIVES: [&str; 2] = ["/PUYO1/PP1_GRP.VDD;1", "/PUYO2/PP2_GRP.VDD;1"];

pub(super) const P1_OVER3_PALETTE_INDICES: [u8; 3] = [0, 1, 15];
const P1_OVER3_BLOCKS: [TextBlockSpec<'static>; 12] = [
    TextBlockSpec {
        id: "BTXB-0222",
        source: "0",
        bbox: Rect {
            x: 0,
            y: 0,
            width: 48,
            height: 56,
        },
        palette_indices: PaletteIndexSpec::Values(&P1_OVER3_PALETTE_INDICES),
        expected_pixels_sha256: "5c5291f5606399d825de96e17cb8c5427069fe430ba9d1f0b6665d5abc9d4792",
    },
    TextBlockSpec {
        id: "BTXB-0223",
        source: "1",
        bbox: Rect {
            x: 48,
            y: 0,
            width: 48,
            height: 56,
        },
        palette_indices: PaletteIndexSpec::Values(&P1_OVER3_PALETTE_INDICES),
        expected_pixels_sha256: "b1e415dea069f09943684b076f0d56749528fe33747ecef9b9efe3cd41b54d6f",
    },
    TextBlockSpec {
        id: "BTXB-0224",
        source: "2",
        bbox: Rect {
            x: 96,
            y: 0,
            width: 48,
            height: 56,
        },
        palette_indices: PaletteIndexSpec::Values(&P1_OVER3_PALETTE_INDICES),
        expected_pixels_sha256: "df881900053b5c260b84de780a5c40a5a762d5965ab380afd26200e3e63ce798",
    },
    TextBlockSpec {
        id: "BTXB-0225",
        source: "3",
        bbox: Rect {
            x: 144,
            y: 0,
            width: 48,
            height: 56,
        },
        palette_indices: PaletteIndexSpec::Values(&P1_OVER3_PALETTE_INDICES),
        expected_pixels_sha256: "3cee5139b68a3a58508f9fee57ef1ec970014bb9427d214f80d6b3975e51506f",
    },
    TextBlockSpec {
        id: "BTXB-0226",
        source: "4",
        bbox: Rect {
            x: 192,
            y: 0,
            width: 48,
            height: 56,
        },
        palette_indices: PaletteIndexSpec::Values(&P1_OVER3_PALETTE_INDICES),
        expected_pixels_sha256: "e7e1a702bf0555661479af1380478fe7a4b41cc20d062b9037617e68c9078b04",
    },
    TextBlockSpec {
        id: "BTXB-0227",
        source: "5",
        bbox: Rect {
            x: 0,
            y: 56,
            width: 48,
            height: 56,
        },
        palette_indices: PaletteIndexSpec::Values(&P1_OVER3_PALETTE_INDICES),
        expected_pixels_sha256: "bad1a15a7405f437b3a1fa489e008e65da2cb68e25f99731dead004e92ebbe69",
    },
    TextBlockSpec {
        id: "BTXB-0228",
        source: "6",
        bbox: Rect {
            x: 48,
            y: 56,
            width: 48,
            height: 56,
        },
        palette_indices: PaletteIndexSpec::Values(&P1_OVER3_PALETTE_INDICES),
        expected_pixels_sha256: "6b96ef12dd11af81cb911a91c29b5cef73ecf274b7ca360c83473b5c0082d671",
    },
    TextBlockSpec {
        id: "BTXB-0229",
        source: "7",
        bbox: Rect {
            x: 96,
            y: 56,
            width: 48,
            height: 56,
        },
        palette_indices: PaletteIndexSpec::Values(&P1_OVER3_PALETTE_INDICES),
        expected_pixels_sha256: "f33a2eba4ee8365b0ec0f8cf1e2a8df26a3049765d8c22528525e49150af64af",
    },
    TextBlockSpec {
        id: "BTXB-0230",
        source: "8",
        bbox: Rect {
            x: 144,
            y: 56,
            width: 48,
            height: 56,
        },
        palette_indices: PaletteIndexSpec::Values(&P1_OVER3_PALETTE_INDICES),
        expected_pixels_sha256: "010ef0075e258a99334644641787989dc05c5149c69efc42e4ec87411546fb12",
    },
    TextBlockSpec {
        id: "BTXB-0231",
        source: "9",
        bbox: Rect {
            x: 192,
            y: 56,
            width: 48,
            height: 56,
        },
        palette_indices: PaletteIndexSpec::Values(&P1_OVER3_PALETTE_INDICES),
        expected_pixels_sha256: "12a2622be3754d69b2bb4b2dbb3ea5f7e5ef8aa8827ee06409723c1615365bdc",
    },
    TextBlockSpec {
        id: "BTXB-0232",
        source: "10",
        bbox: Rect {
            x: 0,
            y: 112,
            width: 64,
            height: 56,
        },
        palette_indices: PaletteIndexSpec::Values(&P1_OVER3_PALETTE_INDICES),
        expected_pixels_sha256: "1f6a21318c4e6116974eb20ab9eaebfcbe3bb3358b52bb328b08838a0a44a099",
    },
    TextBlockSpec {
        id: "BTXB-0233",
        source: "コンティニューするかい？",
        bbox: Rect {
            x: 0,
            y: 168,
            width: 192,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&P1_OVER3_PALETTE_INDICES),
        expected_pixels_sha256: "ca45abf247c98d90d11b7dd46771c7c4cbfec37b15a1a28b9261d398b8c46352",
    },
];

/// continue 화면의 숫자 cell과 실제 표시된 일본어 질문.
pub const P1_OVER3_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &PUYO_ARCHIVES,
    name: "P1_OVER3.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 192,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "6cd68f067a8c9494f84843fc30c77315e0f89078a66c456280a355d86a9a8a57",
    blocks: &P1_OVER3_BLOCKS,
};

const P2_END15_TOP_PALETTE_INDICES: [u8; 4] = [0, 1, 4, 13];
const P2_END15_BOTTOM_PALETTE_INDICES: [u8; 4] = [0, 1, 4, 8];
const P2_END15_BLOCKS: [TextBlockSpec<'static>; 2] = [
    TextBlockSpec {
        id: "BTXB-0234",
        source: "てっぺん",
        bbox: Rect {
            x: 112,
            y: 16,
            width: 72,
            height: 24,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_END15_TOP_PALETTE_INDICES),
        expected_pixels_sha256: "c2e19792dd3d5fc584f1578307cf81093637f884df84c091f12136838d5921b4",
    },
    TextBlockSpec {
        id: "BTXB-0235",
        source: "おしまい",
        bbox: Rect {
            x: 64,
            y: 184,
            width: 104,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&P2_END15_BOTTOM_PALETTE_INDICES),
        expected_pixels_sha256: "43823b6fa4b3c99e2ea6a150312211d6632c31a19a2d9c5beae89554e619cb3f",
    },
];

/// ending 그림의 큰 sprite cell 안에서 캐릭터와 겹치지 않게 분리한 두 문구.
pub const P2_END15_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &PUYO_ARCHIVES,
    name: "P2_END15.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 224,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "937d1cb1cf68ce74aa066b9e0e53ace465436f3ad40c4965c95f7a49ac545888",
    blocks: &P2_END15_BLOCKS,
};

const END10_HEADING_PALETTE_INDICES: [u8; 2] = [0, 11];
const END10_CREDIT_PALETTE_INDICES: [u8; 2] = [0, 15];
const END10_COPYRIGHT_PALETTE_INDICES: [u8; 3] = [0, 1, 5];
const P2_END10_BLOCKS: [TextBlockSpec<'static>; 17] = [
    TextBlockSpec {
        id: "BTXB-0236",
        source: "PRODUCER",
        bbox: Rect {
            x: 0,
            y: 0,
            width: 72,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_HEADING_PALETTE_INDICES),
        expected_pixels_sha256: "92658c373bca8f9fce2b371fe08f5b5f47c93e03427f9096dd02e19aadf0ca4b",
    },
    TextBlockSpec {
        id: "BTXB-0237",
        source: "PROGRAM",
        bbox: Rect {
            x: 72,
            y: 0,
            width: 80,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_HEADING_PALETTE_INDICES),
        expected_pixels_sha256: "541aabd8f22136492d4bcbe027a161f8901a6e8634adbc874fab5de36d366add",
    },
    TextBlockSpec {
        id: "BTXB-0238",
        source: "DESIGN",
        bbox: Rect {
            x: 152,
            y: 0,
            width: 56,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_HEADING_PALETTE_INDICES),
        expected_pixels_sha256: "719fd34bc95790d95a86f4abae2104b738b043e21ecf6388ed0399f8a26708ac",
    },
    TextBlockSpec {
        id: "BTXB-0239",
        source: "SOUND & VOICE",
        bbox: Rect {
            x: 0,
            y: 16,
            width: 120,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_HEADING_PALETTE_INDICES),
        expected_pixels_sha256: "d439c8891b806632c6913bb889fed645e1ce5db74c3346a2936f3d4369072f3e",
    },
    TextBlockSpec {
        id: "BTXB-0240",
        source: "MANUAL & PACKAGE",
        bbox: Rect {
            x: 0,
            y: 32,
            width: 144,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_HEADING_PALETTE_INDICES),
        expected_pixels_sha256: "ab189d73618970b7cd3ea4f1d495d824c99acfa75035a3478ce0d8c512ffc63e",
    },
    TextBlockSpec {
        id: "BTXB-0241",
        source: "SPECIAL THANKS",
        bbox: Rect {
            x: 0,
            y: 48,
            width: 120,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_HEADING_PALETTE_INDICES),
        expected_pixels_sha256: "da48fb3c57138a20be6e54d1fa31173f05b2618aa2d18a04b8d0f5274dc39b34",
    },
    TextBlockSpec {
        id: "BTXB-0242",
        source: "MOO仁井谷",
        bbox: Rect {
            x: 0,
            y: 64,
            width: 96,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_CREDIT_PALETTE_INDICES),
        expected_pixels_sha256: "aa5bcb51facea2fdfc65a6c6736e74e3e84f6656e25f445e7d2f0cc08a8f7656",
    },
    TextBlockSpec {
        id: "BTXB-0243",
        source: "じぇみに広野",
        bbox: Rect {
            x: 96,
            y: 64,
            width: 96,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_CREDIT_PALETTE_INDICES),
        expected_pixels_sha256: "ca436418a0b29cca672d6039a9218eff366813f772a605a084fea5681a54df0e",
    },
    TextBlockSpec {
        id: "BTXB-0244",
        source: "秋山泰俊S",
        bbox: Rect {
            x: 0,
            y: 80,
            width: 80,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_CREDIT_PALETTE_INDICES),
        expected_pixels_sha256: "6c03bfa24088cc03243132c04556b8ca1e2fcbabf0ff45d10d1f86e530fec691",
    },
    TextBlockSpec {
        id: "BTXB-0245",
        source: "RIU.S.AYA",
        bbox: Rect {
            x: 80,
            y: 80,
            width: 96,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_CREDIT_PALETTE_INDICES),
        expected_pixels_sha256: "7d035bdac158f3d8bddd43553935791d5149c3036e4fa5ab2e792dc3929db21d",
    },
    TextBlockSpec {
        id: "BTXB-0246",
        source: "松島剛史 (LMS MUSIC)",
        bbox: Rect {
            x: 0,
            y: 96,
            width: 152,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_CREDIT_PALETTE_INDICES),
        expected_pixels_sha256: "08c7183eac5fe7994642ab6ec52a96b0e3794d48321eabb7d155c8d2710c4841",
    },
    TextBlockSpec {
        id: "BTXB-0247",
        source: "BA.M (LMS MUSIC)",
        bbox: Rect {
            x: 0,
            y: 112,
            width: 136,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_CREDIT_PALETTE_INDICES),
        expected_pixels_sha256: "eace6ab3b35b6fb854d4c89ae655105c6bd1da6cdfcd283cf9a6d4c12ac2f6e0",
    },
    TextBlockSpec {
        id: "BTXB-0248",
        source: "木戸福三郎",
        bbox: Rect {
            x: 136,
            y: 112,
            width: 80,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_CREDIT_PALETTE_INDICES),
        expected_pixels_sha256: "24567cc1c02a13f749c5555481152080093f23bb20f5eee6ab3478c7402a4896",
    },
    TextBlockSpec {
        id: "BTXB-0249",
        source: "〆張鶴",
        bbox: Rect {
            x: 0,
            y: 128,
            width: 56,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_CREDIT_PALETTE_INDICES),
        expected_pixels_sha256: "47a63dbab82211683b0a8e90e8a5f4f006ce491528eab4d9182368d61d1a4973",
    },
    TextBlockSpec {
        id: "BTXB-0250",
        source: "北出和彦",
        bbox: Rect {
            x: 56,
            y: 128,
            width: 64,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_CREDIT_PALETTE_INDICES),
        expected_pixels_sha256: "31adb94e422066d7b9937915f8f184da824a67ce2992a687aa9de613518e0b65",
    },
    TextBlockSpec {
        id: "BTXB-0251",
        source: "森田健吾 (24)",
        bbox: Rect {
            x: 120,
            y: 128,
            width: 96,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_CREDIT_PALETTE_INDICES),
        expected_pixels_sha256: "953fbd14560b9623ec552f24e0e00da4deabfe9a47b426c2d555c935a5c0017a",
    },
    TextBlockSpec {
        id: "BTXB-0252",
        source: "© COMPILE 1994",
        bbox: Rect {
            x: 0,
            y: 144,
            width: 144,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&END10_COPYRIGHT_PALETTE_INDICES),
        expected_pixels_sha256: "297ffcfa9a709fe2b307a44655a06c8c683864f8e4c28e5cb4a0928d7efe381e",
    },
];

/// 원본 credit cell 17개를 일본어 인명과 영문 보존 대상으로 분리한 ending atlas.
pub const P2_END10_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &PUYO_ARCHIVES,
    name: "P2_END10.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 160,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "fe6a21b7e41e645d96ed6f4087688d5daa4d7e67d30f7f07d97d8ffa58019126",
    blocks: &P2_END10_BLOCKS,
};

const P2_END16_TEXT_PALETTE_INDICES: [u8; 9] = [1, 2, 3, 4, 5, 6, 7, 13, 14];
const P2_END16_BLOCKS: [TextBlockSpec<'static>; 1] = [TextBlockSpec {
    id: "BTXB-0253",
    source: "めざせ頂上サタン様",
    bbox: Rect {
        x: 0,
        y: 0,
        width: 160,
        height: 256,
    },
    palette_indices: PaletteIndexSpec::Values(&P2_END16_TEXT_PALETTE_INDICES),
    expected_pixels_sha256: "fd659bd2952dc0dc09feb9cc232c6ad40cdcf67a42793fdc00c00e30e55ee285",
}];

/// 왼쪽 두 cell의 조립 문구만 합치고 오른쪽 탑 두 cell을 보호한 ending atlas.
pub const P2_END16_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &PUYO_ARCHIVES,
    name: "P2_END16.BTX",
    storage: TextStorageSpec::RasterBlocks,
    layout: Layout {
        width: 256,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "5368d0cf79735a4ccd5a5be3f59ac86a78442bc41b2a980879c45d2214a93a3b",
    blocks: &P2_END16_BLOCKS,
};
