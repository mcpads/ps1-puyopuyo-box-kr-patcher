//! `BOXOPT.BTX` sparse selector atlas의 제품 text catalog.
//!
//! 다섯 물리 사본은 같은 해제 픽셀을 공유한다. selector bbox 중 여섯 쌍이
//! 겹치므로 block별 쓰기가 아니라 전체 768x256 surface 합성 경계를 요구한다.

use std::error::Error;
use std::fmt;

use crate::graphics::boxopt::{BoxoptSelector, BOXOPT_OVERLAPS, BOXOPT_PHRASES};

use super::{
    is_japanese_translation_target, Layout, PaletteIndexSpec, PixelFormat, Rect,
    SparseSelectorAtlasSpec, TextAssetSpec, TextBlockSpec, TextStorageSpec,
};

const BOXOPT_ARCHIVES: [&str; 5] = [
    "/GOCHA1/TUU_GRP.VDD;1",
    "/GOCHA2/GOTY_GRP.VDD;1",
    "/HORI2/HORI2GRP.VDD;1",
    "/HORI4/HORI4GRP.VDD;1",
    "/MINPUYO/MIN_GRP.VDD;1",
];
const BOXOPT_PALETTE_INDICES: [u8; 14] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13];

const fn block(
    id: &'static str,
    source: &'static str,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    expected_pixels_sha256: &'static str,
) -> TextBlockSpec<'static> {
    TextBlockSpec {
        id,
        source,
        bbox: Rect {
            x,
            y,
            width,
            height,
        },
        palette_indices: PaletteIndexSpec::Values(&BOXOPT_PALETTE_INDICES),
        expected_pixels_sha256,
    }
}

/// selector 16..69에서 사람이 판독한 문구 49개.
pub const BOXOPT_TEXT_BLOCKS: [TextBlockSpec<'static>; 49] = [
    block(
        "BTXB-0275",
        "とれーにんぐをはじめる",
        0,
        144,
        176,
        16,
        "8bfa0cec5637e9a575cb6c4b939f0e97e38c3c1566e2d659e00ed870afa89dce",
    ),
    block(
        "BTXB-0276",
        "ぷよの色数",
        176,
        144,
        80,
        16,
        "68fcb3c5084e388e8eb62d3a28dff4cb1030d4a204f811eedd0d17729693d51e",
    ),
    block(
        "BTXB-0277",
        "ぷよの落下スピード",
        0,
        160,
        144,
        16,
        "ad9a9aac9d62eab539aa698053070227c4c9df7a848d621464df864417f3c42a",
    ),
    block(
        "BTXB-0278",
        "その他の設定",
        144,
        160,
        96,
        16,
        "a4e5e0c4b19115bf4225c61ef46f7907547ce232b9a9176efb559cd9a014ca34",
    ),
    block(
        "BTXB-0279",
        "フィールドの状態",
        0,
        176,
        128,
        16,
        "414c1be23ec5aad2a309e654838038d70da3be5f7bc3d5a6848824946b519445",
    ),
    block(
        "BTXB-0280",
        "ゲームをはじめる",
        128,
        176,
        128,
        16,
        "e7d2bffb119fac3172f2df479367e16a4267feb458ec0d3dc8e8c65d13e81931",
    ),
    block(
        "BTXB-0281",
        "ルール",
        0,
        192,
        48,
        16,
        "9e50d28d536e51d1537b42b70379a3d1920f0f930ebee017e54bb3fccbd05d68",
    ),
    block(
        "BTXB-0282",
        "対戦数",
        48,
        192,
        48,
        16,
        "15a41300a023382a22d630d5185d992f6bc2bcf73c5a121f2de9c9a7f0d1dbe0",
    ),
    block(
        "BTXB-0283",
        "プレイヤー数",
        416,
        192,
        96,
        16,
        "55ba58cd6c2f0919263fd90827bf5d6c96d30c7508a240366683d39dc3d32801",
    ),
    block(
        "BTXB-0284",
        "COM",
        160,
        208,
        48,
        16,
        "6f25a91435d068393e6826a0b9e8bbccaf71cf09565e446ed732896ab8828075",
    ),
    block(
        "BTXB-0285",
        "プレイヤーの",
        96,
        192,
        96,
        16,
        "b2c05d64d657b3091009ccb3811b9657b1d83c219e268282aa93aa0b5d22bb92",
    ),
    block(
        "BTXB-0286",
        "COMの強さ",
        160,
        208,
        96,
        16,
        "87622dd1f0dd9fbac34d8cdc6aa5c84e66bfb7093199bd0366c1cd0f9393785b",
    ),
    block(
        "BTXB-0287",
        "弱い",
        0,
        224,
        32,
        16,
        "7bbf342d191f05d9b797fcf34862915995bfc2d5274dc23d6b07a8739661a233",
    ),
    block(
        "BTXB-0288",
        "普通",
        32,
        224,
        32,
        16,
        "dd299e7d3295bfa51bd83ec8fce4b5824697d72a6f61b99b8676aef9e8a7fa0d",
    ),
    block(
        "BTXB-0289",
        "強い",
        64,
        224,
        32,
        16,
        "8da6ffd7fd345c7670700dff11479091716e48de4d16daf0ef5c80791954cdae",
    ),
    block(
        "BTXB-0290",
        "戻る",
        96,
        224,
        32,
        16,
        "8450d231cbdb7eced2418027fe606e1d423db733b402cfb39408d4148df3a146",
    ),
    block(
        "BTXB-0291",
        "爆弾ぷよの数",
        0,
        240,
        96,
        16,
        "37370111da85537319f3984758644dfafb7005b965d96e225008cc7887fc1fa2",
    ),
    block(
        "BTXB-0292",
        "つたぷよは",
        240,
        0,
        96,
        16,
        "beb5ec5bee700432171672733abfbcc5bb5bf1f51c03f2a3a965169f25d10936",
    ),
    block(
        "BTXB-0293",
        "ノーマル",
        0,
        0,
        64,
        16,
        "2eb3383e2fbfdd4ce888fa47b08626587fadbf682f57edd1efb0cd6cdacfcc75",
    ),
    block(
        "BTXB-0294",
        "かたぷよ",
        64,
        0,
        64,
        16,
        "5fe8affc8e1bb9f5f4d76c68cd7832e672c42535ebea27de8b1934ac16cd8023",
    ),
    block(
        "BTXB-0295",
        "得点ぷよ",
        176,
        64,
        64,
        16,
        "4e728a6ac7a750cb1a27954ce0643a56b2d78fab7b1f9337cc46090537a5fab4",
    ),
    block(
        "BTXB-0296",
        "6個消し",
        0,
        48,
        64,
        16,
        "1b64c1cbb8bb92c83154f84b97a588c85f4f5af6d17f4ae6b84ecf01ada46bf0",
    ),
    block(
        "BTXB-0297",
        "2個消し",
        0,
        64,
        64,
        16,
        "8b49ddd689788b5580b33ad63ea65c79dd13a633bbce8fb5936c2d2d23179c8a",
    ),
    block(
        "BTXB-0298",
        "エディット",
        0,
        80,
        80,
        16,
        "8e6a77ab94f16acb3aa7e8a19c97f05addb335190d460b9b6c45d3ceecddbb3d",
    ),
    block(
        "BTXB-0299",
        "ノーマル",
        0,
        0,
        64,
        16,
        "2eb3383e2fbfdd4ce888fa47b08626587fadbf682f57edd1efb0cd6cdacfcc75",
    ),
    block(
        "BTXB-0300",
        "固ぷよ",
        0,
        16,
        48,
        16,
        "a98e68724ad2790bf6ee289c535a389175ef4422151a4164230de0f7f4aa18cd",
    ),
    block(
        "BTXB-0301",
        "かたふた",
        48,
        16,
        64,
        16,
        "9a5a2a2c92090560c38e4230a99b89cd275e7436b3c2dae588bb5d1fd101600f",
    ),
    block(
        "BTXB-0302",
        "エディット終了",
        0,
        80,
        112,
        16,
        "f7a877a4d1ad0133f22ff8a325b4d34e0344541be7b46ac09660e9ec7ce187de",
    ),
    block(
        "BTXB-0303",
        "OFF",
        208,
        192,
        48,
        16,
        "761b919d774d4df2b08df340d714f22503f8521eaece6e773ebcf76f7cb5a46b",
    ),
    block(
        "BTXB-0304",
        "おじゃまぷよの固さ",
        112,
        16,
        144,
        16,
        "c228ca631060750b04c2da634b22d01f5efe11dd94eb8ddbc26b2e3bddfade30",
    ),
    block(
        "BTXB-0305",
        "連鎖しばり",
        144,
        0,
        80,
        16,
        "e45cd5548714a9e3ccc251f5d3aa80d11bf38ec7f28961f411d259878a30c71d",
    ),
    block(
        "BTXB-0306",
        "逆連鎖しばり",
        128,
        0,
        96,
        16,
        "b32321ddab4dd2f0eca0360b0ee0061e7591a1be36e53c9dbbdbe1b315c0292a",
    ),
    block(
        "BTXB-0307",
        "マージンタイム",
        144,
        48,
        112,
        16,
        "e65769852951531eb4ab2dacd3040146be494ee186b7077d90cc49b22aeab732",
    ),
    block(
        "BTXB-0308",
        "ぷよが消える個数",
        256,
        176,
        128,
        16,
        "2c6d5e3bee69dff892d38f3b93f943126aafd737bb789653562056611d6fb936",
    ),
    block(
        "BTXB-0309",
        "おじゃまぷよのレート",
        256,
        192,
        160,
        16,
        "090515726cff32d80e4ed2f5e09b77c75ec1cf76f546a6a5422acfe12986cfe5",
    ),
    block(
        "BTXB-0310",
        "ページ切り替え",
        384,
        176,
        112,
        16,
        "f80df5edfbe152ccec88bfd1b228804b03c30d68592e80d051fc3efa8a1ffeab",
    ),
    block(
        "BTXB-0311",
        "おじゃまぷよの得点",
        64,
        64,
        144,
        16,
        "9f94611f15504ec24d693df3a60f5141bac146a928e76311480cebf1cd5dc082",
    ),
    block(
        "BTXB-0312",
        "全消し攻撃力",
        112,
        80,
        96,
        16,
        "316d52229b20d2074425ceb305b94ecc4df4de55983bbba844cacfb184c6d63a",
    ),
    block(
        "BTXB-0313",
        "フィールドの継続",
        384,
        208,
        128,
        16,
        "0eb2a6654bad91768da7be8b544268b52b708a38d15ce00530b78ed614b187ff",
    ),
    block(
        "BTXB-0314",
        "する",
        80,
        96,
        32,
        16,
        "6b637b377832d7861cd36aac6f572e7f2a93fa37e2d2041cb051abfa2a9b233b",
    ),
    block(
        "BTXB-0315",
        "しない",
        112,
        96,
        48,
        16,
        "6ee625dc6512cdb20b128a1e11c8a40115b09437a73828b6b5d0c8c0a6d01237",
    ),
    block(
        "BTXB-0316",
        "おじゃま化",
        160,
        96,
        80,
        16,
        "2fedcc0fb3f1a12ccb9fb7662d0e326ca3b2d20cec9f5f72b7208c1d228e6339",
    ),
    block(
        "BTXB-0317",
        "NEXT2の表示",
        256,
        208,
        128,
        16,
        "ed00875536f86c08b4bbef1a34928d7b6c16f264696895dd8c1ffca9aa0818fe",
    ),
    block(
        "BTXB-0318",
        "ルールの初期化",
        0,
        112,
        112,
        16,
        "3363aad894866bf4710173234028e0f423e66f558bb5aaccab1edb16c4c2bcc7",
    ),
    block(
        "BTXB-0319",
        "甘口コース",
        64,
        32,
        80,
        16,
        "2eecba9e6c68eb92feddf16b316e01e9452cf708bc1255be1c58ffcd7181c489",
    ),
    block(
        "BTXB-0320",
        "中辛コース",
        144,
        32,
        80,
        16,
        "4c665c98334d02cae0b4444651c8ef8a05cc6997a67ad2b950ddd446bea96925",
    ),
    block(
        "BTXB-0321",
        "辛口コース",
        64,
        48,
        80,
        16,
        "fdaebeb8d862ae0fba40c55f0e7d94f59f68764529e311db4f7d2099cf66200d",
    ),
    block(
        "BTXB-0322",
        "?????",
        64,
        208,
        80,
        16,
        "4c820fa511679d06b2385ab28dd95d6b6f3690c646007aabea528150d0da29da",
    ),
    block(
        "BTXB-0323",
        "フリーズ対戦す",
        0,
        96,
        96,
        16,
        "a001bfdca87cd483ef46aebdc2bc620b425842c506a11645d8b0ea5c22164d8e",
    ),
];

const BOXOPT_ALLOWED_OVERLAPS: [(&str, &str); 6] = [
    ("BTXB-0284", "BTXB-0286"),
    ("BTXB-0293", "BTXB-0299"),
    ("BTXB-0295", "BTXB-0311"),
    ("BTXB-0298", "BTXB-0302"),
    ("BTXB-0305", "BTXB-0306"),
    ("BTXB-0314", "BTXB-0323"),
];

/// 다섯 EXE가 공유하는 BOXOPT 문구 atlas.
pub const BOXOPT_TEXT_ASSET: TextAssetSpec<'static> = TextAssetSpec {
    archive_paths: &BOXOPT_ARCHIVES,
    name: "BOXOPT.BTX",
    layout: Layout {
        width: 768,
        height: 256,
        format: PixelFormat::Indexed4,
    },
    expected_decompressed_sha256:
        "fd29338f9cec880a0f07979539808f182854918bcc4d4e2d74ae7b75c72b5077",
    storage: TextStorageSpec::SparseSelectorAtlas(SparseSelectorAtlasSpec {
        allowed_overlap_pairs: &BOXOPT_ALLOWED_OVERLAPS,
        expected_physical_union_pixels: 62_976,
    }),
    blocks: &BOXOPT_TEXT_BLOCKS,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxoptCatalogBindingError(String);

impl fmt::Display for BoxoptCatalogBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "BOXOPT catalog-selector 결속 실패: {}", self.0)
    }
}

impl Error for BoxoptCatalogBindingError {}

fn error(message: impl Into<String>) -> BoxoptCatalogBindingError {
    BoxoptCatalogBindingError(message.into())
}

/// 정적 consumer에서 읽은 selector 표와 제품 catalog의 bbox·원문을 교차한다.
pub fn verify_boxopt_text_catalog_selectors(
    selectors: &[BoxoptSelector],
) -> Result<(), BoxoptCatalogBindingError> {
    if BOXOPT_PHRASES.len() != BOXOPT_TEXT_BLOCKS.len() {
        return Err(error("phrase와 block 분모가 다름"));
    }
    for (phrase, block) in BOXOPT_PHRASES.iter().zip(BOXOPT_TEXT_BLOCKS.iter()) {
        let selector = selectors
            .get(phrase.selector)
            .ok_or_else(|| error(format!("selector {} 누락", phrase.selector)))?;
        let selector_bbox = (
            usize::from(selector.x),
            usize::from(selector.y),
            usize::from(selector.width),
            usize::from(selector.height),
        );
        let block_bbox = (
            block.bbox.x,
            block.bbox.y,
            block.bbox.width,
            block.bbox.height,
        );
        if selector_bbox != block_bbox {
            return Err(error(format!(
                "selector {} bbox 불일치: selector={selector_bbox:?}, block={block_bbox:?}",
                phrase.selector
            )));
        }
        if phrase.source != block.source {
            return Err(error(format!(
                "selector {} 보호 원문 불일치: phrase={:?}, block={:?}",
                phrase.selector, phrase.source, block.source
            )));
        }
        if phrase.translation_target != is_japanese_translation_target(block.source) {
            return Err(error(format!(
                "selector {} 번역 대상 판정 불일치",
                phrase.selector
            )));
        }
    }

    let actual_overlap_ids = BOXOPT_OVERLAPS
        .iter()
        .map(|(left, right)| {
            let left_offset = BOXOPT_PHRASES
                .iter()
                .position(|phrase| phrase.selector == *left)
                .ok_or_else(|| error(format!("overlap selector {left}가 phrase에 없음")))?;
            let right_offset = BOXOPT_PHRASES
                .iter()
                .position(|phrase| phrase.selector == *right)
                .ok_or_else(|| error(format!("overlap selector {right}가 phrase에 없음")))?;
            let left_id = BOXOPT_TEXT_BLOCKS[left_offset].id;
            let right_id = BOXOPT_TEXT_BLOCKS[right_offset].id;
            Ok(if left_id < right_id {
                (left_id, right_id)
            } else {
                (right_id, left_id)
            })
        })
        .collect::<Result<Vec<_>, BoxoptCatalogBindingError>>()?;
    if actual_overlap_ids != BOXOPT_ALLOWED_OVERLAPS {
        return Err(error(format!(
            "selector와 catalog overlap 쌍 불일치: selector={actual_overlap_ids:?}, catalog={BOXOPT_ALLOWED_OVERLAPS:?}"
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "boxopt_tests.rs"]
mod tests;
