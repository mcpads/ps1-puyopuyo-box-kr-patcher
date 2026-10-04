//! 전적 요약·상대별 창의 명시한 글자 영역만 조판한다.
use super::text_catalog::{PaletteIndexSpec, Rect, TextBlockSpec};
use super::text_patch::render_baked_text_block;
use super::text_profile::{
    BakedFontBook, BakedTextEffect, BakedTextProfile, HorizontalAlign, MAPLESTORY_LIGHT_FONT_ID,
};
use crate::btx::{decode, encode, Layout, PixelFormat};
use sha2::{Digest, Sha256};

struct Label {
    id: &'static str,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    font_size: f32,
    source_sha256: &'static str,
}
const LABELS: [Label; 12] = [
    Label {
        id: "skill",
        x: 524,
        y: 6,
        width: 96,
        height: 20,
        font_size: 14.0,
        source_sha256: "096e8c3fd4bef57081b661707a2d8dbc32f45edb0db5250f5f715628a1e25a09",
    },
    Label {
        id: "grade",
        x: 677,
        y: 6,
        width: 24,
        height: 20,
        font_size: 14.0,
        source_sha256: "8fc4419f7294abebd278a1830dc3bcef84b48c7843079125d3e9d4f30a500676",
    },
    Label {
        id: "matches",
        x: 564,
        y: 30,
        width: 24,
        height: 20,
        font_size: 14.0,
        source_sha256: "edaf9f200f11c6f5dbc52c32e4461bd0e2a1257e68422d761781501eeb84cbee",
    },
    Label {
        id: "wins",
        x: 621,
        y: 30,
        width: 24,
        height: 20,
        font_size: 14.0,
        source_sha256: "e34ab7d3d4963b41b1221c86489f149acba261b28e867392588f1a0555af000a",
    },
    Label {
        id: "losses",
        x: 677,
        y: 30,
        width: 24,
        height: 20,
        font_size: 14.0,
        source_sha256: "b2ae50be1de5eacd4672d1ad05aa40543f6f9f20e3a15719d11a80fdc34cae0e",
    },
    Label {
        id: "max_chain",
        x: 524,
        y: 63,
        width: 100,
        height: 20,
        font_size: 14.0,
        source_sha256: "eb4d6e68a6e17021363a4d18cfa7541b51ed873bdc394942ea40fc448ba109d9",
    },
    Label {
        id: "win_streak",
        x: 524,
        y: 96,
        width: 72,
        height: 20,
        font_size: 14.0,
        source_sha256: "58011c836a2063cc1b2a01e3cbaf50a1661fec99b045b3da13aaae0590de735a",
    },
    Label {
        id: "shortest_play",
        x: 524,
        y: 129,
        width: 100,
        height: 20,
        font_size: 12.0,
        source_sha256: "b910182eeb77ef7b7d05afe52e4a757e1ed764fe01ceb499cb0be67efc4484e2",
    },
    Label {
        id: "versus",
        x: 549,
        y: 166,
        width: 24,
        height: 20,
        font_size: 10.0,
        source_sha256: "948af83df79c9d29c9236157dd326a2b9d40e0e4c079d1d51f0d611d6236ac12",
    },
    Label {
        id: "matches",
        x: 588,
        y: 190,
        width: 24,
        height: 20,
        font_size: 14.0,
        source_sha256: "e03b68228ffcff98f1823a1f049ac9f2d0455da532ef231da9e46b41946fd9d0",
    },
    Label {
        id: "wins",
        x: 645,
        y: 190,
        width: 24,
        height: 20,
        font_size: 14.0,
        source_sha256: "e34ab7d3d4963b41b1221c86489f149acba261b28e867392588f1a0555af000a",
    },
    Label {
        id: "losses",
        x: 701,
        y: 190,
        width: 24,
        height: 20,
        font_size: 14.0,
        source_sha256: "b2ae50be1de5eacd4672d1ad05aa40543f6f9f20e3a15719d11a80fdc34cae0e",
    },
];
pub(crate) const RECORD_INTERIORS: [(usize, usize, usize, usize); 5] = [
    (519, 6, 186, 44),
    (519, 63, 186, 20),
    (519, 96, 186, 20),
    (519, 129, 186, 20),
    (543, 166, 186, 44),
];
pub(crate) const RECORD_LABEL_COUNT: usize = LABELS.len();
const MANIFEST: &str = include_str!("../../assets/translations/official-records/messages.tsv");

/// 원본의 빈 창 무늬로 전적 창 내부를 구성하고 명시된 bbox에 한글을 합성한다.
/// 가려진 원래 무늬의 복원을 주장하지 않는 검토용 후보다.
pub fn render_official_records(original: &[u8]) -> Result<Vec<u8>, String> {
    if format!("{:x}", Sha256::digest(original))
        != "5beb1b2a65528b2a617371415db07b06d4fe773595f72982b8e0e7d766da754c"
    {
        return Err("전적 KS_SPR 원본 불일치".into());
    }
    let layout = Layout {
        width: 1280,
        height: 256,
        format: PixelFormat::Indexed4,
    };
    let before = decode(original, layout).map_err(|e| e.to_string())?;
    let fonts = BakedFontBook::with_bundled_maplestory_light().map_err(|e| e.to_string())?;
    let rows: Vec<_> = MANIFEST
        .lines()
        .skip(1)
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .collect();
    if rows.iter().any(|r| r.len() != 4 || r[3] != "draft") {
        return Err("전적 번역 표 형식 오류".into());
    }
    let mut output = before.clone();
    // 같은 atlas의 빈 창 내부만 참조한다. 창 테두리와 동적 숫자·이름 자산은 보존한다.
    let donor = Rect {
        x: 265,
        y: 8,
        width: 94,
        height: 16,
    };
    for y in donor.y..donor.y + donor.height {
        if before[y * 1280 + donor.x..y * 1280 + donor.x + donor.width]
            .iter()
            .any(|v| !matches!(v, 3 | 5))
        {
            return Err("전적 배경 donor에 창 무늬 외 색이 있음".into());
        }
    }
    for (x, y, width, height) in RECORD_INTERIORS {
        for dy in 0..height {
            for dx in 0..width {
                output[(y + dy) * 1280 + x + dx] = before[(donor.y + dy * donor.height / height)
                    * 1280
                    + donor.x
                    + dx * donor.width / width];
            }
        }
    }
    let mut touched = vec![false; before.len()];
    for label in &LABELS {
        let matches: Vec<_> = rows.iter().filter(|r| r[0] == label.id).collect();
        let [row] = matches.as_slice() else {
            return Err("전적 문구 누락·중복".into());
        };
        let mut pixels = Vec::new();
        for y in label.y..label.y + label.height {
            pixels.extend_from_slice(&before[y * 1280 + label.x..y * 1280 + label.x + label.width]);
        }
        if format!("{:x}", Sha256::digest(&pixels)) != label.source_sha256 {
            return Err("전적 글자 영역 원본 불일치".into());
        }
        let block = TextBlockSpec {
            id: label.id,
            source: row[1],
            bbox: Rect {
                x: 0,
                y: 0,
                width: label.width,
                height: label.height,
            },
            palette_indices: PaletteIndexSpec::Values(&[0, 1, 2, 3, 4, 5, 6, 12, 13, 14, 15]),
            expected_pixels_sha256: label.source_sha256,
        };
        let glyphs = render_baked_text_block(
            block,
            PixelFormat::Indexed4,
            &fonts,
            row[2],
            BakedTextProfile {
                id: "official-record-label",
                font_id: MAPLESTORY_LIGHT_FONT_ID,
                font_size: label.font_size,
                baseline: 16,
                line_count: 1,
                line_height: 20,
                tracking: 0,
                align: HorizontalAlign::Left { padding: 2 },
                background: 0,
                body_ramp: &[12, 13, 14, 15],
                effect: BakedTextEffect::Outline { palette_index: 1 },
            },
        )
        .map_err(|e| e.to_string())?;
        for y in 0..label.height {
            for x in 0..label.width {
                let pos = (label.y + y) * 1280 + label.x + x;
                if std::mem::replace(&mut touched[pos], true) {
                    return Err("전적 label bbox 겹침".into());
                }
                let value = glyphs[y * label.width + x];
                if value != 0 {
                    output[pos] = value;
                }
            }
        }
    }
    encode(&output, layout).map_err(|e| e.to_string())
}
