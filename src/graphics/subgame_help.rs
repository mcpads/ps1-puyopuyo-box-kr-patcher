//! BOXOPT의 설명 전용 16px 글리프와 고정 길이 안내문.
//! 기존 selector 33과 겹치는 첫 다섯 slot은 새 글리프에 배정하지 않는다.
use super::text_catalog::BOXOPT_TEXT_ASSET;
use crate::btx::{decode, encode};
use crate::font::{
    bundled_maplestory_light, rasterize_menu06_glyph, Menu06RasterEffect, Menu06RasterProfile,
    MAPLESTORY_LIGHT_FONT_ID,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const MESSAGE_COUNT: usize = 46;
pub(crate) const MESSAGE_BYTES: usize = 60;
pub(crate) const TABLE_BYTES: usize = 352;
const FIRST_SLOT: usize = 5;
const SLOTS: usize = 176;

pub(crate) struct HelpMessage {
    pub source: String,
    pub alternate: String,
    pub ko: String,
}
pub(crate) struct HelpText {
    pub messages: Vec<HelpMessage>,
    pub glyphs: BTreeMap<char, usize>,
}

impl HelpText {
    pub(crate) fn load() -> Result<Self, String> {
        Self::parse(include_str!(
            "../../assets/translations/subgame-help/messages.tsv"
        ))
    }
    fn parse(raw: &str) -> Result<Self, String> {
        let mut lines = raw.lines();
        if lines.next() != Some("index\tsource\tsource_alternate\tko") {
            return Err("설명 번역 header 불일치".into());
        }
        let mut messages = Vec::new();
        let mut chars = BTreeSet::new();
        for line in lines {
            let f: Vec<_> = line.split('\t').collect();
            if f.len() != 4 || f[0].parse::<usize>().ok() != Some(messages.len()) {
                return Err("설명 번역 순서/열 불일치".into());
            }
            let ko = f[3].replace("\\n", "\n");
            if ko.is_empty()
                || ko.split('\n').count() > 2
                || ko
                    .split('\n')
                    .any(|l| l.is_empty() || l.chars().count() > 16)
                || ko.chars().count() + 1 > MESSAGE_BYTES / 2
            {
                return Err(format!("설명 {} 행/용량 초과", f[0]));
            }
            if ko
                .chars()
                .any(|c| !(('가'..='힣').contains(&c) || c.is_ascii_digit() || " .\n".contains(c)))
            {
                return Err(format!("설명 {} 미지원 문자", f[0]));
            }
            chars.extend(ko.chars().filter(|c| *c != '\n'));
            messages.push(HelpMessage {
                source: f[1].replace("\\n", "rr"),
                alternate: f[2].replace("\\n", "rr"),
                ko,
            });
        }
        if messages.len() != MESSAGE_COUNT || chars.len() + FIRST_SLOT > SLOTS {
            return Err("설명 모집단/글리프 용량 불일치".into());
        }
        Ok(Self {
            messages,
            glyphs: chars
                .into_iter()
                .enumerate()
                .map(|(i, c)| (c, i + FIRST_SLOT))
                .collect(),
        })
    }
    fn code(slot: usize) -> [u8; 2] {
        [0x80, (slot + 1) as u8]
    }
    pub(crate) fn table(&self, original: &[u8]) -> Result<Vec<u8>, String> {
        if original.len() != TABLE_BYTES {
            return Err("설명 코드표 길이 불일치".into());
        }
        let mut out = original.to_vec();
        for slot in self.glyphs.values() {
            out[slot * 2..slot * 2 + 2].copy_from_slice(&Self::code(*slot));
        }
        // first-match lookup에서 생성 code가 원본의 잔여 code와 충돌하면 실패한다.
        for slot in self.glyphs.values() {
            if out
                .chunks_exact(2)
                .filter(|p| *p == Self::code(*slot))
                .count()
                != 1
            {
                return Err("설명 code 충돌".into());
            }
        }
        Ok(out)
    }
    pub(crate) fn strings(&self, original: &[u8]) -> Result<Vec<u8>, String> {
        if original.len() != MESSAGE_COUNT * MESSAGE_BYTES {
            return Err("설명 문자열 영역 길이 불일치".into());
        }
        let mut out = original.to_vec();
        for (i, m) in self.messages.iter().enumerate() {
            let src = &original[i * MESSAGE_BYTES..(i + 1) * MESSAGE_BYTES];
            let end = src
                .chunks_exact(2)
                .position(|p| p == b"ee")
                .ok_or("설명 종료 누락")?
                * 2;
            let decoded = crate::text::decode_sjis(&src[..end]).map_err(|e| e.to_string())?;
            if decoded != m.source && decoded != m.alternate {
                return Err(format!("설명 {i} 원문 불일치"));
            }
            let mut encoded = Vec::new();
            for c in m.ko.chars() {
                if c == '\n' {
                    encoded.extend_from_slice(b"rr");
                } else {
                    encoded.extend_from_slice(&Self::code(
                        *self.glyphs.get(&c).ok_or("설명 glyph 누락")?,
                    ));
                }
            }
            encoded.extend_from_slice(b"ee");
            // ee 뒤는 해석되지 않는다. 원본 padding·잔여 문자열을 무단 정규화하지 않는다.
            out[i * MESSAGE_BYTES..i * MESSAGE_BYTES + encoded.len()].copy_from_slice(&encoded);
        }
        Ok(out)
    }
}

/// 불변 원본에서 만든 selector surface와 겹치지 않는 설명 glyph를 한 결과로 합성한다.
pub(crate) fn compose_help_glyphs(
    original: &[u8],
    selector_surface: &[u8],
) -> Result<Vec<u8>, String> {
    let asset = BOXOPT_TEXT_ASSET;
    let source = decode(original, asset.layout).map_err(|e| e.to_string())?;
    let mut pixels = decode(selector_surface, asset.layout).map_err(|e| e.to_string())?;
    let text = HelpText::load()?;
    let font = bundled_maplestory_light().map_err(|e| e.to_string())?;
    let profile = Menu06RasterProfile {
        id: "subgame-help-16px",
        font_id: MAPLESTORY_LIGHT_FONT_ID,
        font_size: 15.0,
        baseline: 12,
        effect: Menu06RasterEffect::Binary {
            threshold: 96,
            transparent: 0,
            foreground: 13,
        },
    };
    for (&ch, &slot) in &text.glyphs {
        let x = 256 + slot % 16 * 16;
        let y = slot / 16 * 16;
        if asset.blocks.iter().any(|b| {
            x < b.bbox.x + b.bbox.width
                && x + 16 > b.bbox.x
                && y < b.bbox.y + b.bbox.height
                && y + 16 > b.bbox.y
        }) {
            return Err("설명 glyph와 selector 겹침".into());
        }
        if ch != ' ' && font.lookup_glyph_index(ch) == 0 {
            return Err(format!("설명 폰트 누락: {ch}"));
        }
        let glyph = if ch == ' ' {
            vec![0; 256]
        } else {
            rasterize_menu06_glyph(&font, ch, profile).map_err(|e| e.to_string())?
        };
        if ch != ' ' && glyph.iter().all(|p| *p == 0) {
            return Err(format!("빈 설명 glyph: {ch}"));
        }
        for row in 0..16 {
            let at = (y + row) * asset.layout.width + x;
            if source[at..at + 16] != pixels[at..at + 16] {
                return Err("설명 glyph가 앞선 selector 쓰기와 겹침".into());
            }
            pixels[at..at + 16].copy_from_slice(&glyph[row * 16..row * 16 + 16]);
        }
    }
    encode(&pixels, asset.layout).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "subgame_help_tests.rs"]
mod tests;
