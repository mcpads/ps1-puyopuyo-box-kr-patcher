//! 특기 이름·설명의 2바이트 코드와 고정 복사 길이 경계.
//! 공백과 개행은 renderer의 별도 control이며 글리프 slot을 소비하지 않는다.
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const TABLE_BYTES: usize = 502;
const FALLBACK_SLOT: usize = 250;
pub(crate) const NEWLINE: [u8; 2] = *b"nn";
pub(crate) const SPACE: [u8; 2] = [0x81, 0x40];

pub(crate) struct SkillMessage {
    pub source_name: String,
    pub source_description: String,
    pub name: String,
    pub description: String,
}
pub(crate) struct SkillText {
    pub messages: Vec<SkillMessage>,
    pub glyphs: BTreeMap<char, usize>,
}
impl SkillText {
    pub(crate) fn load() -> Result<Self, String> {
        Self::parse(include_str!(
            "../../assets/translations/special-skills/messages.tsv"
        ))
    }
    fn parse(raw: &str) -> Result<Self, String> {
        let mut lines = raw.lines();
        if lines.next() != Some("index\tsource_name\tsource_description\tko_name\tko_description") {
            return Err("특기 번역 header 불일치".into());
        }
        let mut messages = Vec::new();
        let mut chars = BTreeSet::new();
        for line in lines {
            let f: Vec<_> = line.split('\t').collect();
            if f.len() != 5 || f[0].parse::<usize>().ok() != Some(messages.len()) {
                return Err("특기 번역 순서/열 불일치".into());
            }
            let name = f[3].to_string();
            let description = f[4].replace("\\n", "\n");
            if name.is_empty()
                || name.chars().count() > 15
                || description.split('\n').count() > 3
                || description
                    .split('\n')
                    .any(|l| l.is_empty() || l.chars().count() > 12)
                || name.contains('\\')
            {
                return Err(format!("특기 {} 창 용량 초과", f[0]));
            }
            for c in name.chars().chain(description.chars()) {
                if !(('가'..='힣').contains(&c) || c.is_ascii_digit() || " .\n".contains(c)) {
                    return Err(format!("특기 미지원 문자: {c}"));
                }
                if c != ' ' && c != '\n' {
                    chars.insert(c);
                }
            }
            let source_name = f[1].to_string();
            let source_description = f[2].replace("\\n", "nn");
            for (source, ko) in [(&source_name, &name), (&source_description, &description)] {
                if ko.chars().count() * 2 > source_bytes(source)?.len() {
                    return Err(format!("특기 {} 원문 고정 복사 길이 초과", f[0]));
                }
            }
            messages.push(SkillMessage {
                source_name,
                source_description,
                name,
                description,
            });
        }
        if messages.len() != 27 || chars.len() > FALLBACK_SLOT {
            return Err("특기 모집단/글리프 용량 불일치".into());
        }
        Ok(Self {
            messages,
            glyphs: chars.into_iter().enumerate().map(|(i, c)| (c, i)).collect(),
        })
    }
    fn code(slot: usize) -> [u8; 2] {
        [0x80, (slot + 1) as u8]
    }
    pub(crate) fn table(&self, original: &[u8]) -> Result<Vec<u8>, String> {
        if original.len() != TABLE_BYTES {
            return Err("특기 문자표 길이 불일치".into());
        }
        let mut out = original.to_vec();
        for &slot in self.glyphs.values() {
            out[slot * 2..slot * 2 + 2].copy_from_slice(&Self::code(slot));
        }
        for &slot in self.glyphs.values() {
            let code = Self::code(slot);
            if code == NEWLINE
                || code == SPACE
                || code.contains(&0)
                || out.chunks_exact(2).filter(|p| *p == code).count() != 1
            {
                return Err("특기 문자표/control 충돌".into());
            }
        }
        Ok(out)
    }
    pub(crate) fn literal(
        &self,
        original: &[u8],
        source: &str,
        ko: &str,
    ) -> Result<Vec<u8>, String> {
        let mut expected = source_bytes(source)?;
        expected.push(0);
        if original != expected {
            return Err("특기 원문 literal 불일치".into());
        }
        let mut encoded = Vec::new();
        for c in ko.chars() {
            encoded.extend_from_slice(&match c {
                '\n' => NEWLINE,
                ' ' => SPACE,
                _ => Self::code(*self.glyphs.get(&c).ok_or("특기 glyph 누락")?),
            });
        }
        encoded.push(0);
        if encoded.len() > original.len() {
            return Err("특기 원문 고정 복사 길이 초과".into());
        }
        // 짧아진 문장 뒤의 원본 tail은 NUL 뒤에서 해석되지 않으므로 보존한다.
        let mut out = original.to_vec();
        out[..encoded.len()].copy_from_slice(&encoded);
        Ok(out)
    }
}
pub(crate) fn source_bytes(source: &str) -> Result<Vec<u8>, String> {
    let (bytes, _, errors) = encoding_rs::SHIFT_JIS.encode(source);
    if errors || bytes.contains(&0) {
        return Err("특기 원문 인코딩 오류".into());
    }
    Ok(bytes.into_owned())
}

#[cfg(test)]
#[path = "special_skill_text_tests.rs"]
mod tests;
