//! `BOXOPT.BTX`의 asset-backed selector와 수동 판독 문구 경계.
//!
//! 이 자산은 단순한 3-cell 아이콘 표가 아니다. descriptor가 가리키는
//! sparse selector 표의 앞 96 slot 가운데 78개가 자산 내부 rectangle이고,
//! 18개는 의도적인 빈 slot이다. 문구 selector 일부는 같은 원본 픽셀을
//! 겹쳐 쓰므로, 번역 픽셀은 selector별로 독립 적용하지 않고 겹침 group을
//! 먼저 한 결과로 합성해야 한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

pub const BOXOPT_ATLAS_WIDTH: u16 = 768;
pub const BOXOPT_ATLAS_HEIGHT: u16 = 256;
pub const BOXOPT_SELECTOR_SLOTS: usize = 96;
pub const BOXOPT_ASSET_BACKED_SELECTORS: usize = 78;
pub const BOXOPT_EMPTY_SELECTORS: usize = 18;
pub const BOXOPT_PHRASE_SELECTORS: usize = 49;
pub const BOXOPT_TRANSLATION_TARGET_SELECTORS: usize = 46;
pub const BOXOPT_REFERENCE_TEXT_SELECTORS: usize = 14;
pub const BOXOPT_NON_TEXT_SELECTORS: usize = 15;
pub const BOXOPT_OVERLAP_GROUPS: usize = 6;
pub const BOXOPT_SELECTOR_TABLE_BYTES: usize = 12 + BOXOPT_SELECTOR_SLOTS * 12;
pub const BOXOPT_SELECTOR_TABLE_SHA256: &str =
    "90abbe2163a7cde839afd702497706b3de7b5e43644fc9ba848c299469949e77";

/// selector 하나가 가리키는 원본 rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxoptSelector {
    pub index: usize,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub anchor_x: i16,
    pub anchor_y: i16,
}

impl BoxoptSelector {
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn is_asset_backed(self) -> bool {
        !self.is_empty()
            && u32::from(self.x) + u32::from(self.width) <= u32::from(BOXOPT_ATLAS_WIDTH)
            && u32::from(self.y) + u32::from(self.height) <= u32::from(BOXOPT_ATLAS_HEIGHT)
    }

    fn overlaps(self, other: Self) -> bool {
        u32::from(self.x) < u32::from(other.x) + u32::from(other.width)
            && u32::from(other.x) < u32::from(self.x) + u32::from(self.width)
            && u32::from(self.y) < u32::from(other.y) + u32::from(other.height)
            && u32::from(other.y) < u32::from(self.y) + u32::from(self.height)
    }
}

/// 사람이 원본 픽셀에서 판독한 문구 selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxoptPhraseSpec {
    pub selector: usize,
    pub source: &'static str,
    pub translation_target: bool,
}

const fn phrase(
    selector: usize,
    source: &'static str,
    translation_target: bool,
) -> BoxoptPhraseSpec {
    BoxoptPhraseSpec {
        selector,
        source,
        translation_target,
    }
}

/// 숫자·Latin 참고 cell을 제외한 원본 phrase selector 49개.
pub const BOXOPT_PHRASES: [BoxoptPhraseSpec; BOXOPT_PHRASE_SELECTORS] = [
    phrase(16, "とれーにんぐをはじめる", true),
    phrase(17, "ぷよの色数", true),
    phrase(18, "ぷよの落下スピード", true),
    phrase(19, "その他の設定", true),
    phrase(20, "フィールドの状態", true),
    phrase(21, "ゲームをはじめる", true),
    phrase(22, "ルール", true),
    phrase(23, "対戦数", true),
    phrase(24, "プレイヤー数", true),
    phrase(25, "COM", false),
    phrase(26, "プレイヤーの", true),
    phrase(27, "COMの強さ", true),
    phrase(28, "弱い", true),
    phrase(29, "普通", true),
    phrase(30, "強い", true),
    phrase(31, "戻る", true),
    phrase(32, "爆弾ぷよの数", true),
    phrase(33, "つたぷよは", true),
    phrase(35, "ノーマル", true),
    phrase(36, "かたぷよ", true),
    phrase(37, "得点ぷよ", true),
    phrase(38, "6個消し", true),
    phrase(39, "2個消し", true),
    phrase(40, "エディット", true),
    phrase(45, "ノーマル", true),
    phrase(46, "固ぷよ", true),
    phrase(47, "かたふた", true),
    phrase(48, "エディット終了", true),
    phrase(49, "OFF", false),
    phrase(50, "おじゃまぷよの固さ", true),
    phrase(51, "連鎖しばり", true),
    phrase(52, "逆連鎖しばり", true),
    phrase(53, "マージンタイム", true),
    phrase(54, "ぷよが消える個数", true),
    phrase(55, "おじゃまぷよのレート", true),
    phrase(56, "ページ切り替え", true),
    phrase(57, "おじゃまぷよの得点", true),
    phrase(58, "全消し攻撃力", true),
    phrase(59, "フィールドの継続", true),
    phrase(60, "する", true),
    phrase(61, "しない", true),
    phrase(62, "おじゃま化", true),
    phrase(63, "NEXT2の表示", true),
    phrase(64, "ルールの初期化", true),
    phrase(65, "甘口コース", true),
    phrase(66, "中辛コース", true),
    phrase(67, "辛口コース", true),
    phrase(68, "?????", false),
    phrase(69, "フリーズ対戦す", true),
];

/// 동일 원본 픽셀을 공유하므로 최종 쓰기 전에 하나로 합성할 selector pair.
pub const BOXOPT_OVERLAPS: [(usize, usize); BOXOPT_OVERLAP_GROUPS] =
    [(25, 27), (35, 45), (37, 57), (40, 48), (51, 52), (60, 69)];

const BOXOPT_REFERENCE_TEXT: [usize; BOXOPT_REFERENCE_TEXT_SELECTORS] =
    [80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 92, 93, 94, 95];

const BOXOPT_NON_TEXT: [usize; BOXOPT_NON_TEXT_SELECTORS] =
    [0, 1, 2, 7, 8, 9, 10, 11, 12, 13, 14, 15, 70, 90, 91];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxoptSelectorTable {
    pub selectors: Vec<BoxoptSelector>,
    pub first_non_asset_selector: BoxoptSelector,
    pub phrase_selector_pixels: usize,
    pub phrase_surface_union_pixels: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoxoptSelectorError(String);

impl fmt::Display for BoxoptSelectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "BOXOPT selector 감사 실패: {}", self.0)
    }
}

impl Error for BoxoptSelectorError {}

fn error(message: impl Into<String>) -> BoxoptSelectorError {
    BoxoptSelectorError(message.into())
}

fn read_selector(raw: &[u8], offset: usize, index: usize) -> Option<BoxoptSelector> {
    let cell = raw.get(offset..offset + 12)?;
    Some(BoxoptSelector {
        index,
        x: u16::from_le_bytes([cell[0], cell[1]]),
        y: u16::from_le_bytes([cell[2], cell[3]]),
        width: u16::from_le_bytes([cell[4], cell[5]]),
        height: u16::from_le_bytes([cell[6], cell[7]]),
        anchor_x: i16::from_le_bytes([cell[8], cell[9]]),
        anchor_y: i16::from_le_bytes([cell[10], cell[11]]),
    })
}

fn phrase_overlap_pairs(selectors: &[BoxoptSelector]) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for (left_offset, left_spec) in BOXOPT_PHRASES.iter().enumerate() {
        let left = selectors[left_spec.selector];
        for right_spec in BOXOPT_PHRASES.iter().skip(left_offset + 1) {
            let right = selectors[right_spec.selector];
            if left.overlaps(right) {
                pairs.push((left.index, right.index));
            }
        }
    }
    pairs
}

fn phrase_pixel_counts(selectors: &[BoxoptSelector]) -> (usize, usize) {
    let mut occupied =
        vec![false; usize::from(BOXOPT_ATLAS_WIDTH) * usize::from(BOXOPT_ATLAS_HEIGHT)];
    let mut selector_pixels = 0usize;
    for spec in BOXOPT_PHRASES {
        let selector = selectors[spec.selector];
        selector_pixels += usize::from(selector.width) * usize::from(selector.height);
        for y in selector.y..selector.y + selector.height {
            for x in selector.x..selector.x + selector.width {
                occupied[usize::from(y) * usize::from(BOXOPT_ATLAS_WIDTH) + usize::from(x)] = true;
            }
        }
    }
    (
        selector_pixels,
        occupied.into_iter().filter(|value| *value).count(),
    )
}

/// header, sparse 96-slot prefix와 첫 asset 밖 record를 검증한다.
pub fn parse_boxopt_selector_table(raw: &[u8]) -> Result<BoxoptSelectorTable, BoxoptSelectorError> {
    let needed = BOXOPT_SELECTOR_TABLE_BYTES + 12;
    if raw.len() < needed {
        return Err(error(format!(
            "selector table 길이 부족: expected-at-least={needed}, actual={}",
            raw.len()
        )));
    }
    let header = read_selector(raw, 0, usize::MAX).ok_or_else(|| error("header 읽기 실패"))?;
    if header.x != 0
        || header.y != 0
        || header.width != BOXOPT_ATLAS_WIDTH
        || header.height != BOXOPT_ATLAS_HEIGHT
        || header.anchor_x != 0
        || header.anchor_y != 0
    {
        return Err(error(format!("header 불일치: {header:?}")));
    }
    let digest = format!("{:x}", Sha256::digest(&raw[..BOXOPT_SELECTOR_TABLE_BYTES]));
    if digest != BOXOPT_SELECTOR_TABLE_SHA256 {
        return Err(error(format!(
            "selector table SHA-256 불일치: expected={BOXOPT_SELECTOR_TABLE_SHA256}, actual={digest}"
        )));
    }

    let selectors = (0..BOXOPT_SELECTOR_SLOTS)
        .map(|index| {
            read_selector(raw, 12 + index * 12, index)
                .ok_or_else(|| error(format!("selector {index} 읽기 실패")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let asset_backed = selectors
        .iter()
        .filter(|selector| selector.is_asset_backed())
        .count();
    let empty = selectors
        .iter()
        .filter(|selector| selector.is_empty())
        .count();
    if asset_backed != BOXOPT_ASSET_BACKED_SELECTORS || empty != BOXOPT_EMPTY_SELECTORS {
        return Err(error(format!(
            "selector 분모 불일치: asset-backed={asset_backed}, empty={empty}"
        )));
    }
    if let Some(selector) = selectors
        .iter()
        .find(|selector| !selector.is_empty() && !selector.is_asset_backed())
    {
        return Err(error(format!(
            "96-slot prefix에 asset 밖 selector가 있음: {selector:?}"
        )));
    }
    let first_non_asset_selector =
        read_selector(raw, BOXOPT_SELECTOR_TABLE_BYTES, BOXOPT_SELECTOR_SLOTS)
            .ok_or_else(|| error("첫 prefix 밖 selector 읽기 실패"))?;
    if first_non_asset_selector
        != (BoxoptSelector {
            index: BOXOPT_SELECTOR_SLOTS,
            x: 16,
            y: 256,
            width: 16,
            height: 16,
            anchor_x: 0,
            anchor_y: 0,
        })
    {
        return Err(error(format!(
            "첫 prefix 밖 selector 불일치: {first_non_asset_selector:?}"
        )));
    }

    let translation_targets = BOXOPT_PHRASES
        .iter()
        .filter(|spec| spec.translation_target)
        .count();
    if translation_targets != BOXOPT_TRANSLATION_TARGET_SELECTORS {
        return Err(error(format!(
            "번역 대상 phrase 분모 불일치: {translation_targets}"
        )));
    }

    let mut classified = BTreeSet::new();
    for spec in BOXOPT_PHRASES {
        if !classified.insert(spec.selector) {
            return Err(error(format!(
                "중복 phrase selector 분류: {}",
                spec.selector
            )));
        }
        let selector = selectors
            .get(spec.selector)
            .ok_or_else(|| error(format!("phrase selector 범위 이탈: {spec:?}")))?;
        if spec.source.is_empty() || !selector.is_asset_backed() {
            return Err(error(format!("phrase selector 계약 불일치: {spec:?}")));
        }
    }
    for selector in BOXOPT_REFERENCE_TEXT {
        if !classified.insert(selector) {
            return Err(error(format!("중복 참고 text selector 분류: {selector}")));
        }
        if !selectors[selector].is_asset_backed() {
            return Err(error(format!(
                "참고 text selector가 asset 밖임: {selector}"
            )));
        }
    }
    for selector in BOXOPT_NON_TEXT {
        if !classified.insert(selector) {
            return Err(error(format!("중복 non-text selector 분류: {selector}")));
        }
        if !selectors[selector].is_asset_backed() {
            return Err(error(format!("non-text selector가 asset 밖임: {selector}")));
        }
    }
    let asset_backed_indices = selectors
        .iter()
        .filter(|selector| selector.is_asset_backed())
        .map(|selector| selector.index)
        .collect::<BTreeSet<_>>();
    if classified != asset_backed_indices {
        return Err(error(format!(
            "asset-backed selector 분류 불완전: classified={}, actual={}",
            classified.len(),
            asset_backed_indices.len()
        )));
    }
    if phrase_overlap_pairs(&selectors) != BOXOPT_OVERLAPS {
        return Err(error("phrase selector overlap group 불일치"));
    }
    let (phrase_selector_pixels, phrase_surface_union_pixels) = phrase_pixel_counts(&selectors);
    if phrase_selector_pixels != 68_096 || phrase_surface_union_pixels != 62_976 {
        return Err(error(format!(
            "phrase pixel 분모 불일치: selector={phrase_selector_pixels}, union={phrase_surface_union_pixels}"
        )));
    }

    Ok(BoxoptSelectorTable {
        selectors,
        first_non_asset_selector,
        phrase_selector_pixels,
        phrase_surface_union_pixels,
    })
}

#[cfg(test)]
#[path = "boxopt_tests.rs"]
mod tests;
