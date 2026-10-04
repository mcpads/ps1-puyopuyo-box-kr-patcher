//! GOCHA1 결과 메시지 `G_MESS.BTX`의 256×16 raster block 계약.
//!
//! 정적 선택표는 23개 cell 중 19개를 고른다. atlas에는 그 19개와 별도로
//! 일본어 픽셀이 있는 왼쪽 y=112, y=208 행이 있으므로, 번역 모집단은
//! 사람이 확인한 21개 양성 block 전체를 보존한다.

use super::{
    Layout, PaletteIndexSpec, PixelFormat, Rect, TextAssetSpec, TextBlockSpec, TextStorageSpec,
};

const G_MESS_ARCHIVES: [&str; 1] = ["/GOCHA1/TUU_GRP.VDD;1"];
const G_MESS_PALETTE_INDICES: [u8; 14] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];

macro_rules! message {
    ($id:literal, $source:literal, $x:expr, $y:expr, $digest:literal) => {
        TextBlockSpec {
            id: $id,
            source: $source,
            bbox: Rect {
                x: $x,
                y: $y,
                width: 256,
                height: 16,
            },
            palette_indices: PaletteIndexSpec::Values(&G_MESS_PALETTE_INDICES),
            expected_pixels_sha256: $digest,
        }
    };
}

const G_MESS_BLOCKS: [TextBlockSpec<'static>; 21] = [
    message!(
        "BTXB-0254",
        "がーんっ！　せっかくクリア",
        0,
        0,
        "ca03eb22146c347409adf47bd62d9d16e18366283c117c03b7aa5411ca7bc381"
    ),
    message!(
        "BTXB-0255",
        "したのにこれだけ？",
        0,
        16,
        "26d6b0dcea47e08e94a9b3e8b1b7fb62c51d3bc6d67b7c9bee6203d570a2d2f9"
    ),
    message!(
        "BTXB-0256",
        "とほほー。",
        0,
        32,
        "1a6ed4ddbab6d8579c5d9cc0604f567e7d8ee534060bdef066cdafb51b5bb9d1"
    ),
    message!(
        "BTXB-0257",
        "よのなかやっぱり甘くないね。",
        0,
        48,
        "627ff18c655e2bbf30dddd87fc43f52d1b1d0bf58dcf92731e5da1b5749cb609"
    ),
    message!(
        "BTXB-0258",
        "うーん…それじゃこんどは",
        0,
        64,
        "c23b45fe5c238048b6d7da929218111afc16f3cea7dcf05e7bc480074e8db9b8"
    ),
    message!(
        "BTXB-0259",
        "中辛コースにチャレンジだ！",
        0,
        80,
        "baa19605537ee2d8f635a3397f48ccc5ebaf6c9aa3cd4dda921a0ebc41b4ea45"
    ),
    message!(
        "BTXB-0260",
        "ヤッホー！",
        0,
        96,
        "92b67cbf9f972ae5f7d19bfd65f0f460c749546f0336539661f46a0940e3ee0e"
    ),
    message!(
        "BTXB-0261",
        "中辛コース　クリア！",
        0,
        112,
        "5d4d25adbe74e7d97779e1bec0b723e16f0fdeb0c3f90bd633a02c2c37cf7dc9"
    ),
    message!(
        "BTXB-0262",
        "これでボクもぷよらーとして",
        0,
        128,
        "3e8cd672153629b3dd91c5e666cb7e153efaac02e4bb99021009988c877d6282"
    ),
    message!(
        "BTXB-0263",
        "いちにんまえかな？",
        0,
        144,
        "a09234161a9599445ed00f2d09dc21760bd3590efbdde65c50d4eb35b62e9d6f"
    ),
    message!(
        "BTXB-0264",
        "ぐぐー！",
        0,
        160,
        "4c5f83d1fa4919fae24b5ffc1c3b8839cae172bd56bba5180cfc9322d54bdcda"
    ),
    message!(
        "BTXB-0265",
        "つぎは辛口コースだね。",
        0,
        176,
        "332e36156a022dc43bcc940f0b885f2531b4fa678b2fa329564d4f7d0a76c673"
    ),
    message!(
        "BTXB-0266",
        "よーし　ガンバルぞぉ！",
        0,
        192,
        "b3c99338584bc2f7df2f0e1e8ed5755ead72ea4825ab124e614e5897593c496b"
    ),
    message!(
        "BTXB-0267",
        "ウ～～～～ッ　やったぁ！！",
        0,
        208,
        "c1539c40d0350a253f35f2ad61e5de47b4a181a40278f555e16bc3371f1f4bba"
    ),
    message!(
        "BTXB-0268",
        "うーん　ボクってすごいっ！",
        0,
        224,
        "d396ad9f3eb338c3a9d66e12434a6fa329e3c2bad4474bdc1d691c479eab5be1"
    ),
    message!(
        "BTXB-0269",
        "でも　こんなでっかいトロフィー",
        0,
        240,
        "c6c8a842c12d56342e3c6cf5f5246fa7d500ae1c477c3db58d963a52e1e95c9b"
    ),
    message!(
        "BTXB-0270",
        "かかえきれないよ。",
        256,
        0,
        "b90da46e6d1287b57d7db8aed30a224c9c21ae59be5198246dde270b2d3e9bf0"
    ),
    message!(
        "BTXB-0271",
        "…ところでかーくん。",
        256,
        16,
        "4ec4fdfe0b28eead1fad5092277a23e02bf559d220fbbc1e8dc95b1abdad8dcd"
    ),
    message!(
        "BTXB-0272",
        "ぐ？",
        256,
        32,
        "7491769af1365a2a3b2763736263bd18a1a72ab5afd8dae0820c9ae2a837fb46"
    ),
    message!(
        "BTXB-0273",
        "…この大会っていったい",
        256,
        48,
        "5919e93744a673b4bbedcde864c3219a55362229404040a9f9c40aa99aca9d28"
    ),
    message!(
        "BTXB-0274",
        "なんなんだろうね？",
        256,
        64,
        "7b6efe00180b96756dea9ebf6899c4b7f1be85bfe119b4e422dc47366b5d962c"
    ),
];

/// GOCHA1 결과 화면의 일본어 메시지 atlas.
pub const G_MESS_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &G_MESS_ARCHIVES,
    name: "G_MESS.BTX",
    layout: Layout {
        width: 512,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "e80411b0142228a8af7baf1166eb29412bdb03aca1afd49c0df3d015cb8d57da",
    storage: TextStorageSpec::RasterBlocks,
    blocks: &G_MESS_BLOCKS,
};
