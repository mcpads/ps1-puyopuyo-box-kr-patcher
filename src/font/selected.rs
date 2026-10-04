//! 사용자 선택 원천과 QUEST 메뉴의 독립 글리프 identity.
use fontdue::{Font, FontSettings};
use sha2::{Digest, Sha256};

pub const GALMURI11: &str = "galmuri11";
pub const GALMURI11_BOLD: &str = "galmuri11-bold";
pub const DENKI: &str = "denki";
pub const MULMARU: &str = "mulmaru";

pub fn load(id: &str) -> Result<Font, String> {
    let bytes: &[u8] = match id {
        GALMURI11 => include_bytes!("../../assets/fonts/galmuri11.ttf"),
        GALMURI11_BOLD => include_bytes!("../../assets/fonts/galmuri11-bold.ttf"),
        DENKI => include_bytes!("../../assets/fonts/denki.ttf"),
        MULMARU => include_bytes!("../../assets/fonts/mulmaru.ttf"),
        _ => return Err(format!("미등록 선택 폰트: {id}")),
    };
    let hash = format!("{:x}", Sha256::digest(bytes));
    let mut found = false;
    for row in include_str!("../../assets/reviews/font_selections.tsv")
        .lines()
        .skip(1)
    {
        let fields: Vec<_> = row.split('\t').collect();
        if fields.len() != 7 {
            return Err("폰트 선택 원장의 열 수 불일치".into());
        }
        if fields[1] == id {
            found = true;
            if fields[5] != hash {
                return Err(format!("선택 폰트 원천 SHA-256 불일치: {id}"));
            }
        }
    }
    if !found {
        return Err(format!("선택 원장에 없는 폰트: {id}"));
    }
    Font::from_bytes(bytes, FontSettings::default()).map_err(str::to_owned)
}

// 번역문·코드 순서는 유지하고 메뉴 page의 catalog 참조만 별도 identity를 쓴다.
// 기존 page identity는 유지하되 QUEST 소비 글꼴은 Denki로 통일한다.
const MENU_GLYPH_BASE: u32 = 0xF0000;
fn menu_character(ch: char) -> bool {
    ('가'..='힣').contains(&ch) || ch.is_ascii_alphanumeric()
}
pub fn menu_catalog_text(text: &str) -> String {
    text.chars()
        .map(|ch| {
            if menu_character(ch) {
                char::from_u32(MENU_GLYPH_BASE + u32::from(ch)).expect("메뉴 glyph identity 범위")
            } else {
                ch
            }
        })
        .collect()
}
pub fn menu_glyph(ch: char) -> Option<char> {
    u32::from(ch)
        .checked_sub(MENU_GLYPH_BASE)
        .and_then(char::from_u32)
        .filter(|c| menu_character(*c))
}

#[cfg(test)]
#[path = "selected_tests.rs"]
mod tests;

/// 저장 glyph identity에 맞는 QUEST 폰트 profile.
pub fn quest_profile(_glyph: char) -> super::QuestRasterProfile {
    super::QuestRasterProfile {
        id: "quest-selected-12x16",
        font_id: DENKI,
        font_size: 12.0,
        baseline: 13,
        effect: super::QuestRasterEffect::None,
    }
}

/// 전각 코드의 폭은 소비자가 소유하고, 획은 같은 Denki 원천에서 만든다.
pub fn quest_cell(font: &Font, glyph: char, width: usize) -> Result<Vec<u8>, String> {
    let glyph = menu_glyph(glyph).unwrap_or(glyph);
    if matches!(glyph, ' ' | '　') {
        return Ok(vec![0; width * 16]);
    }
    let text = match glyph {
        // 원본 슬롯 identity와 소비 폭은 유지하되 한국어 문장부호 획을 쓴다.
        '。' => ".".to_owned(),
        '、' => ",".to_owned(),
        '‼' => "!!".to_owned(),
        '⁇' => "??".to_owned(),
        '〜' | '～' => "~".to_owned(),
        c if ('！'..='～').contains(&c) => {
            char::from_u32(u32::from(c) - 0xfee0).unwrap().to_string()
        }
        c => c.to_string(),
    };
    let mut pixels = vec![0; width * 16];
    let count = text.chars().count();
    for (part, character) in text.chars().enumerate() {
        if font.lookup_glyph_index(character) == 0 {
            return Err(format!(
                "Denki QUEST 글리프 누락: {glyph:?} → {character:?}"
            ));
        }
        let (metrics, bitmap) = font.rasterize(character, 12.0);
        let part_width = width / count;
        // 원본의 6px !!/?? 합자만 두 획을 3px씩 배치한다. 일반 글자는 축소하지 않는다.
        let ink_width = if count == 2 && width == 6 {
            metrics.width.min(part_width)
        } else {
            metrics.width
        };
        let x = part as i32 * part_width as i32 + (part_width as i32 - ink_width as i32) / 2;
        let y = 13 - metrics.ymin - metrics.height as i32;
        if metrics.width == 0
            || metrics.height == 0
            || bitmap.iter().all(|v| *v == 0)
            || ink_width > part_width
            || x < 0
            || y < 0
            || y + metrics.height as i32 > 16
        {
            return Err(format!("Denki QUEST cell 범위/빈 글리프 오류: {glyph:?}, width={width}, metrics={metrics:?}"));
        }
        for row in 0..metrics.height {
            for col in 0..ink_width {
                let source_col = if ink_width < metrics.width {
                    col * (metrics.width - 1) / (ink_width - 1)
                } else {
                    col
                };
                pixels[(y as usize + row) * width + x as usize + col] =
                    ((u16::from(bitmap[row * metrics.width + source_col]) * 15 + 127) / 255) as u8;
            }
        }
    }
    Ok(pixels)
}

/// 일본어 글자와 그림 슬롯을 제외한 공통 텍스트 글리프.
pub fn quest_common_glyph(glyph: char) -> bool {
    // 표준 SJIS 이름만으로 그림의 의미를 판단하지 않는다. 819C는 원본 하트,
    // 81A0는 버튼 안내용 네모이며 음표·별표도 원본 그림 획을 보존한다.
    if matches!(glyph, '●' | '□' | '♪' | '＊') {
        return false;
    }
    glyph.is_ascii_alphanumeric()
        || glyph.is_ascii_punctuation()
        || matches!(
            glyph,
            ' ' | '　' | '‼' | '⁇' | '〜' | '、' | '。' | '「' | '」' | '・' | '…'
        )
        || ('！'..='～').contains(&glyph)
}
