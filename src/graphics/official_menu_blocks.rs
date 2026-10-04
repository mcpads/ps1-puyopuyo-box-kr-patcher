//! 공식전 자산에서 확인한 목판·제목·안내의 원본 경계.
use super::text_catalog::{PaletteIndexSpec, Rect, TextBlockSpec};
pub(super) const BLOCKS: [TextBlockSpec<'static>; 9] = [
    TextBlockSpec {
        id: "battle",
        source: "対戦",
        bbox: Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 104,
        },
        palette_indices: PaletteIndexSpec::Values(&[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        ]),
        expected_pixels_sha256: "b9d3cf82c903976bb50bbee0645d8d71a5076e3ff4b7182d0b23fb10b8514b13",
    },
    TextBlockSpec {
        id: "records",
        source: "データ",
        bbox: Rect {
            x: 64,
            y: 0,
            width: 64,
            height: 104,
        },
        palette_indices: PaletteIndexSpec::Values(&[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        ]),
        expected_pixels_sha256: "eb99c8cf471c37f0314b84a78dd456252b1444bd0b4a2e58866593df67595ce9",
    },
    TextBlockSpec {
        id: "view",
        source: "見る",
        bbox: Rect {
            x: 0,
            y: 104,
            width: 64,
            height: 104,
        },
        palette_indices: PaletteIndexSpec::Values(&[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        ]),
        expected_pixels_sha256: "5602932474d43338af35a98c1c9dedbcade5129650988d1af642d996b8ac5b50",
    },
    TextBlockSpec {
        id: "create",
        source: "作る",
        bbox: Rect {
            x: 64,
            y: 104,
            width: 64,
            height: 104,
        },
        palette_indices: PaletteIndexSpec::Values(&[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        ]),
        expected_pixels_sha256: "78caed99c885668c1b32ff4c9e1f58329c5a4dbace1b702c4c84c2c42e7ef26a",
    },
    TextBlockSpec {
        id: "title",
        source: "公式戦",
        bbox: Rect {
            x: 256,
            y: 160,
            width: 160,
            height: 48,
        },
        palette_indices: PaletteIndexSpec::Values(&[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        ]),
        expected_pixels_sha256: "aa7895efca300f015e04890d1bb136129f7691bd0f6d84459221c7fdb9f43f02",
    },
    TextBlockSpec {
        id: "battle-guide",
        source: "対戦をはじめるよ。",
        bbox: Rect {
            x: 768,
            y: 0,
            width: 144,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]),
        expected_pixels_sha256: "057ee37e7fad2cec1856487fa2fec1454ba9d77e72bb7c14234f5ab4c5d7c88b",
    },
    TextBlockSpec {
        id: "create-guide",
        source: "データを作れるよ。",
        bbox: Rect {
            x: 768,
            y: 16,
            width: 144,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]),
        expected_pixels_sha256: "3600fb512c957db09d537f3c2c3c7b4bac1b62826b8f8aef98e45303c6d8505f",
    },
    TextBlockSpec {
        id: "view-guide",
        source: "データを見ることができるよ。",
        bbox: Rect {
            x: 768,
            y: 32,
            width: 224,
            height: 16,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]),
        expected_pixels_sha256: "a82c356113507f708da944a93a66cf3e93060467b1796f9d2e792133e4181e58",
    },
    TextBlockSpec {
        id: "records-guide",
        source: "データを作ったり\n見たりできるよ。",
        bbox: Rect {
            x: 128,
            y: 224,
            width: 128,
            height: 32,
        },
        palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]),
        expected_pixels_sha256: "e01261ef0d8a091d088d67a7dc6c213fb007910df670a3bc831656034bbfb6f8",
    },
];
