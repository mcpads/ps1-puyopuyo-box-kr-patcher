//! 비한글로 끝나는 도구 이름의 발음을 원장에 결속한다. 숫자를 추측하지 않는다.
use super::quest_translation_encoding::normalize_quest_translation_text;
use crate::text::select_object_particle;
use std::collections::BTreeMap;
const INPUT: &str =
    include_str!("../../assets/translations/quest_item_name_particle_pronunciations.tsv");

// 숫자의 표시 폭은 달라도 발음 원장의 동일한 숫자와 결속한다.
fn pronunciation_key(name: &str) -> Result<String, String> {
    Ok(normalize_quest_translation_text(name)
        .map_err(|e| e.to_string())?
        .chars()
        .map(|ch| match ch {
            '０'..='９' => char::from_u32(u32::from(ch) - 0xFEE0).unwrap_or(ch),
            _ => ch,
        })
        .collect())
}

pub(super) fn pronunciations() -> Result<BTreeMap<String, (String, char)>, String> {
    let mut lines = INPUT.lines();
    if lines.next() != Some("unit_id\tdevelopment_text\tpronunciation_tail\tnotes") {
        return Err("item 조사 발음 header 불일치".into());
    }
    let mut result = BTreeMap::new();
    for line in lines {
        let fields = line.split('\t').collect::<Vec<_>>();
        let [id, name, tail, note] = fields.as_slice() else {
            return Err("item 조사 발음 열 수 불일치".into());
        };
        let mut chars = tail.chars();
        let ch = chars.next().ok_or("item 조사 발음 누락")?;
        if id.is_empty()
            || name.is_empty()
            || note.is_empty()
            || chars.next().is_some()
            || select_object_particle(ch).is_err()
            || result.insert((*id).into(), ((*name).into(), ch)).is_some()
        {
            return Err("item 조사 발음 중복 또는 값 불일치".into());
        }
    }
    Ok(result)
}

pub(super) fn pronunciation_tail(id: &str, name: &str) -> Result<char, String> {
    let last = name.chars().last().ok_or("빈 item 이름")?;
    let exceptions = pronunciations()?;
    if select_object_particle(last).is_ok() {
        if exceptions.contains_key(id) {
            return Err("한글 이름에 불필요한 발음 예외".into());
        }
        return Ok(last);
    }
    match exceptions.get(id) {
        Some((text, tail)) if pronunciation_key(text)? == pronunciation_key(name)? => Ok(*tail),
        _ => Err(format!("{id}: item 이름 발음 예외 누락 또는 표기 drift")),
    }
}

pub(super) fn non_hangul_tails() -> Result<BTreeMap<char, char>, String> {
    let mut result = BTreeMap::new();
    for (name, tail) in pronunciations()?.into_values() {
        let normalized = normalize_quest_translation_text(&name).map_err(|e| e.to_string())?;
        let last = normalized.chars().last().ok_or("빈 item 이름")?;
        let equivalent = match last {
            '0'..='9' => char::from_u32(u32::from(last) + 0xFEE0).unwrap_or(last),
            '０'..='９' => char::from_u32(u32::from(last) - 0xFEE0).unwrap_or(last),
            _ => last,
        };
        for glyph in [last, equivalent] {
            if let Some(previous) = result.insert(glyph, tail) {
                if previous != tail {
                    return Err("같은 표시 끝글자의 item 발음이 충돌함".into());
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "quest_item_name_particles_tests.rs"]
mod tests;
