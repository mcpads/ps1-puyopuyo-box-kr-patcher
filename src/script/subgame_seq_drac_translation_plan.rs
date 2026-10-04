//! DRAC Unicode 번역 span을 scene-local 2바이트 pair로 컴파일하는 경계.
//!
//! 두 `DRAC_SEQ`는 같은 56칸 repertoire와 같은 다섯 원문 record를 가진다.
//! 이 모듈은 원문의 header와 control byte 위치를 템플릿으로 보존하고,
//! 번역 span의 기존 기호는 원래 pair로, 한글 음절은 사용하지 않는 고유
//! repertoire pair로 결정적으로 배정한다. 중복 공백 pair 네 칸은 현재
//! tokenizer에서 첫 occurrence로 수렴하므로 가용 칸으로 세지 않는다.
//! 실제 BTX cell 합성과 SEQ 재포장·최종 쓰기는 다음 계층의 책임이다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::text::decode_sjis;

use super::{SubgameSeqDracTextAnalysis, SubgameSeqDracTextKind, SubgameSeqDracTextRow};

pub const SUBGAME_SEQ_DRAC_TRANSLATION_PLAN_SCHEMA_VERSION: &str =
    "subgame-seq-drac-translation-plan-v1";
pub const BOX_SUBGAME_SEQ_DRAC_TRANSLATION_MESSAGES: usize = 5;
pub const BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_BYTES: usize = 112;
pub const BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_SLOTS: usize = 56;
pub const BOX_SUBGAME_SEQ_DRAC_MESSAGE_CAPACITY_BYTES: usize = 270;
const TERMINATOR: [u8; 2] = [0x65, 0x65];

/// 한 message의 원문 glyph run에 대응하는 Unicode 번역 span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqDracTranslation<'a> {
    pub index: usize,
    pub spans: &'a [&'a str],
}

/// 한 한글 음절에 배정한 scene-local repertoire pair와 cell 순번.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqDracGlyphMapping {
    pub glyph: char,
    pub repertoire_index: usize,
    pub pair: [u8; 2],
}

impl SubgameSeqDracGlyphMapping {
    /// renderer가 repertoire 순번에 더하는 고정 base 30을 적용한다.
    pub fn cell_index(self) -> usize {
        self.repertoire_index + 30
    }
}

/// 두 DRAC 사본에 공통으로 적용할 record와 glyph 배정 계획.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracTranslationPlan {
    pub replacement_records: Vec<Vec<u8>>,
    pub glyph_mappings: Vec<SubgameSeqDracGlyphMapping>,
    pub retained_source_characters: usize,
    pub source_unique_pairs: usize,
    pub source_duplicate_slots: usize,
    pub used_bytes: usize,
    pub remaining_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracTranslationPlanError(String);

impl fmt::Display for SubgameSeqDracTranslationPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqDracTranslationPlanError {}

fn error(message: impl Into<String>) -> SubgameSeqDracTranslationPlanError {
    SubgameSeqDracTranslationPlanError(message.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceSlot {
    index: usize,
    pair: [u8; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceRepertoire {
    by_character: BTreeMap<char, SourceSlot>,
    unique_slots: Vec<SourceSlot>,
    duplicate_slots: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TemplatePart {
    GlyphRun,
    Control(Vec<u8>),
}

fn is_hangul_syllable(glyph: char) -> bool {
    matches!(glyph, '\u{ac00}'..='\u{d7a3}')
}

fn is_japanese_text_character(glyph: char) -> bool {
    matches!(
        glyph,
        '\u{3040}'..='\u{30ff}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{ff65}'..='\u{ff9f}'
    )
}

fn scope_rows<'a>(
    analysis: &'a SubgameSeqDracTextAnalysis,
    scope: &str,
) -> Result<
    (&'a SubgameSeqDracTextRow, Vec<&'a SubgameSeqDracTextRow>),
    SubgameSeqDracTranslationPlanError,
> {
    let repertoires = analysis
        .rows
        .iter()
        .filter(|row| row.scope == scope && row.kind == SubgameSeqDracTextKind::Repertoire)
        .collect::<Vec<_>>();
    let [repertoire] = repertoires.as_slice() else {
        return Err(error(format!(
            "{scope} DRAC translation repertoire 행 분모 불일치"
        )));
    };
    let mut messages = analysis
        .rows
        .iter()
        .filter(|row| row.scope == scope && row.kind == SubgameSeqDracTextKind::Message)
        .collect::<Vec<_>>();
    messages.sort_by_key(|row| row.index);
    if messages.len() != BOX_SUBGAME_SEQ_DRAC_TRANSLATION_MESSAGES
        || messages
            .iter()
            .enumerate()
            .any(|(index, row)| row.index != index + 1)
    {
        return Err(error(format!(
            "{scope} DRAC translation message 행 분모 불일치"
        )));
    }
    Ok((repertoire, messages))
}

fn parse_repertoire(raw: &[u8]) -> Result<SourceRepertoire, SubgameSeqDracTranslationPlanError> {
    if raw.len() != BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_BYTES {
        return Err(error(format!(
            "DRAC translation repertoire 길이 불일치: expected={}, actual={}",
            BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_BYTES,
            raw.len()
        )));
    }
    let mut by_character = BTreeMap::new();
    let mut unique_pairs = BTreeSet::new();
    let mut unique_slots = Vec::new();
    for (index, pair) in raw.chunks_exact(2).enumerate() {
        let pair: [u8; 2] = pair.try_into().expect("2바이트 repertoire pair");
        let decoded = decode_sjis(&pair)
            .map_err(|source| error(format!("DRAC repertoire pair decode 실패: {source}")))?;
        let mut characters = decoded.chars();
        let glyph = characters
            .next()
            .ok_or_else(|| error("DRAC repertoire pair가 빈 문자열로 decode됨"))?;
        if characters.next().is_some() {
            return Err(error("DRAC repertoire pair가 한 문자가 아님"));
        }
        let slot = SourceSlot { index, pair };
        by_character.entry(glyph).or_insert(slot);
        if unique_pairs.insert(pair) {
            unique_slots.push(slot);
        }
    }
    let duplicate_slots = BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_SLOTS
        .checked_sub(unique_slots.len())
        .ok_or_else(|| error("DRAC repertoire unique pair 수 overflow"))?;
    Ok(SourceRepertoire {
        by_character,
        unique_slots,
        duplicate_slots,
    })
}

fn parse_template(
    row: &SubgameSeqDracTextRow,
    repertoire: &BTreeSet<[u8; 2]>,
) -> Result<Vec<TemplatePart>, SubgameSeqDracTranslationPlanError> {
    if row.raw.len() < 6 {
        return Err(error(format!(
            "DRAC message {}가 header와 종결쌍보다 짧음",
            row.index
        )));
    }
    let mut parts = Vec::new();
    let mut in_glyph_run = false;
    let mut cursor = 4usize;
    loop {
        if row.raw.get(cursor..cursor + 2) == Some(TERMINATOR.as_slice()) {
            if cursor + 2 != row.raw.len() {
                return Err(error(format!(
                    "DRAC message {} 종결 뒤 trailing byte가 있음",
                    row.index
                )));
            }
            break;
        }
        let (control, width) = match row.raw.get(cursor).copied() {
            Some(0x71 | 0x77) => {
                row.raw.get(cursor + 1).ok_or_else(|| {
                    error(format!("DRAC message {} control 인자가 잘림", row.index))
                })?;
                (true, 2)
            }
            Some(0x72) => {
                if row.raw.get(cursor + 1) != Some(&0x72) {
                    return Err(error(format!(
                        "DRAC message {} line break가 72 72가 아님",
                        row.index
                    )));
                }
                (true, 2)
            }
            Some(0x73) => (true, 1),
            Some(_) => (false, 2),
            None => return Err(error(format!("DRAC message {} 종결쌍이 없음", row.index))),
        };
        let end = cursor
            .checked_add(width)
            .ok_or_else(|| error("DRAC template cursor overflow"))?;
        let raw = row
            .raw
            .get(cursor..end)
            .ok_or_else(|| error(format!("DRAC message {} token 범위가 잘림", row.index)))?;
        if control {
            parts.push(TemplatePart::Control(raw.to_vec()));
            in_glyph_run = false;
        } else {
            let pair: [u8; 2] = raw.try_into().expect("2바이트 glyph pair");
            if !repertoire.contains(&pair) {
                return Err(error(format!(
                    "DRAC message {} glyph pair가 repertoire에 없음",
                    row.index
                )));
            }
            if !in_glyph_run {
                parts.push(TemplatePart::GlyphRun);
                in_glyph_run = true;
            }
        }
        cursor = end;
    }
    let glyph_runs = parts
        .iter()
        .filter(|part| matches!(part, TemplatePart::GlyphRun))
        .count();
    if glyph_runs != row.spans.len() {
        return Err(error(format!(
            "DRAC message {} glyph span 분모 불일치: extracted={}, template={glyph_runs}",
            row.index,
            row.spans.len()
        )));
    }
    Ok(parts)
}

fn verify_layout(record: &[u8], index: usize) -> Result<(), SubgameSeqDracTranslationPlanError> {
    let header = record
        .get(..4)
        .ok_or_else(|| error(format!("DRAC replacement message {index} header 누락")))?;
    let mut column = 0usize;
    let mut maximum_columns = 0usize;
    let mut cursor = 4usize;
    while cursor < record.len() {
        if record.get(cursor..cursor + 2) == Some(TERMINATOR.as_slice()) {
            if cursor + 2 != record.len() {
                return Err(error(format!(
                    "DRAC replacement message {index} 종결 뒤 trailing byte"
                )));
            }
            maximum_columns = maximum_columns.max(column);
            break;
        }
        match record[cursor] {
            0x71 | 0x77 => cursor += 2,
            0x72 => {
                if record.get(cursor + 1) != Some(&0x72) {
                    return Err(error(format!(
                        "DRAC replacement message {index} line break drift"
                    )));
                }
                maximum_columns = maximum_columns.max(column);
                column = 0;
                cursor += 2;
            }
            0x73 => {
                column = column
                    .checked_add(1)
                    .ok_or_else(|| error("DRAC replacement column overflow"))?;
                cursor += 1;
            }
            _ => {
                column = column
                    .checked_add(1)
                    .ok_or_else(|| error("DRAC replacement column overflow"))?;
                cursor += 2;
            }
        }
    }
    if maximum_columns > usize::from(header[2]) {
        return Err(error(format!(
            "DRAC replacement message {index} header 열 상한 초과: used={maximum_columns}, limit={}",
            header[2]
        )));
    }
    Ok(())
}

/// 두 사본의 byte-identical 원문을 기준으로 공통 DRAC 번역 record를 만든다.
pub fn plan_box_subgame_seq_drac_translation(
    translations: &[SubgameSeqDracTranslation<'_>],
    analysis: &SubgameSeqDracTextAnalysis,
) -> Result<SubgameSeqDracTranslationPlan, SubgameSeqDracTranslationPlanError> {
    let (puyo1_repertoire, puyo1_messages) = scope_rows(analysis, "PUYO1")?;
    let (puyo2_repertoire, puyo2_messages) = scope_rows(analysis, "PUYO2")?;
    if puyo1_repertoire.raw != puyo2_repertoire.raw
        || puyo1_messages
            .iter()
            .zip(&puyo2_messages)
            .any(|(left, right)| left.raw != right.raw)
    {
        return Err(error("두 DRAC 사본의 repertoire/message 원문이 다름"));
    }

    let mut translations = translations.to_vec();
    translations.sort_by_key(|translation| translation.index);
    if translations.len() != BOX_SUBGAME_SEQ_DRAC_TRANSLATION_MESSAGES
        || translations
            .iter()
            .enumerate()
            .any(|(index, translation)| translation.index != index + 1)
    {
        return Err(error("DRAC translation message 입력 분모 불일치"));
    }

    let source = parse_repertoire(&puyo1_repertoire.raw)?;
    let repertoire_pairs = source
        .unique_slots
        .iter()
        .map(|slot| slot.pair)
        .collect::<BTreeSet<_>>();
    let templates = puyo1_messages
        .iter()
        .map(|row| parse_template(row, &repertoire_pairs))
        .collect::<Result<Vec<_>, _>>()?;
    for ((translation, row), template) in translations.iter().zip(&puyo1_messages).zip(&templates) {
        let expected = template
            .iter()
            .filter(|part| matches!(part, TemplatePart::GlyphRun))
            .count();
        if translation.spans.len() != expected {
            return Err(error(format!(
                "DRAC translation message {} span 수 불일치: expected={expected}, actual={}",
                row.index,
                translation.spans.len()
            )));
        }
    }

    let mut requested = Vec::new();
    for translation in &translations {
        for span in translation.spans {
            if span.is_empty() {
                return Err(error(format!(
                    "DRAC translation message {}에 빈 span이 있음",
                    translation.index
                )));
            }
            if span.contains(['\r', '\n']) {
                return Err(error(format!(
                    "DRAC translation message {} span에 개행 문자가 있음",
                    translation.index
                )));
            }
            for glyph in span.chars() {
                if !requested.contains(&glyph) {
                    requested.push(glyph);
                }
            }
        }
    }

    let mut retained_slots = BTreeSet::new();
    let mut hangul = Vec::new();
    for &glyph in &requested {
        if is_hangul_syllable(glyph) {
            hangul.push(glyph);
        } else if is_japanese_text_character(glyph) {
            return Err(error(format!(
                "DRAC translation에 일본어 문자가 남음: {glyph} (U+{:04X})",
                glyph as u32
            )));
        } else if let Some(slot) = source.by_character.get(&glyph) {
            retained_slots.insert(slot.index);
        } else {
            return Err(error(format!(
                "DRAC translation에 repertoire mapping이 없는 문자: {glyph} (U+{:04X})",
                glyph as u32
            )));
        }
    }
    if hangul.is_empty() {
        return Err(error("DRAC translation에 한글 음절이 없음"));
    }

    let available = source
        .unique_slots
        .iter()
        .copied()
        .filter(|slot| !retained_slots.contains(&slot.index))
        .collect::<Vec<_>>();
    if hangul.len() > available.len() {
        return Err(error(format!(
            "DRAC translation 고유 한글이 가용 repertoire pair를 넘음: required={}, available={}",
            hangul.len(),
            available.len()
        )));
    }
    let glyph_mappings = hangul
        .iter()
        .copied()
        .zip(available)
        .map(|(glyph, slot)| SubgameSeqDracGlyphMapping {
            glyph,
            repertoire_index: slot.index,
            pair: slot.pair,
        })
        .collect::<Vec<_>>();
    let mut code_by_character = source
        .by_character
        .iter()
        .map(|(&glyph, slot)| (glyph, slot.pair))
        .collect::<BTreeMap<_, _>>();
    for mapping in &glyph_mappings {
        code_by_character.insert(mapping.glyph, mapping.pair);
    }

    let mut replacement_records = Vec::with_capacity(BOX_SUBGAME_SEQ_DRAC_TRANSLATION_MESSAGES);
    for ((translation, row), template) in translations.iter().zip(&puyo1_messages).zip(&templates) {
        let mut replacement = row.raw[..4].to_vec();
        let mut spans = translation.spans.iter();
        for part in template {
            match part {
                TemplatePart::GlyphRun => {
                    let span = spans.next().expect("span 분모를 앞에서 검증함");
                    for glyph in span.chars() {
                        let pair = code_by_character.get(&glyph).ok_or_else(|| {
                            error(format!("DRAC translation glyph code 누락: {glyph}"))
                        })?;
                        replacement.extend_from_slice(pair);
                    }
                }
                TemplatePart::Control(raw) => replacement.extend_from_slice(raw),
            }
        }
        replacement.extend_from_slice(&TERMINATOR);
        verify_layout(&replacement, row.index)?;
        replacement_records.push(replacement);
    }
    let used_bytes = replacement_records.iter().map(Vec::len).sum::<usize>();
    if used_bytes > BOX_SUBGAME_SEQ_DRAC_MESSAGE_CAPACITY_BYTES {
        return Err(error(format!(
            "DRAC translation 공통 270B capacity 초과: used={used_bytes}"
        )));
    }
    Ok(SubgameSeqDracTranslationPlan {
        replacement_records,
        glyph_mappings,
        retained_source_characters: retained_slots.len(),
        source_unique_pairs: source.unique_slots.len(),
        source_duplicate_slots: source.duplicate_slots,
        used_bytes,
        remaining_bytes: BOX_SUBGAME_SEQ_DRAC_MESSAGE_CAPACITY_BYTES - used_bytes,
    })
}

#[cfg(test)]
#[path = "subgame_seq_drac_translation_plan_tests.rs"]
mod subgame_seq_drac_translation_plan_tests;
