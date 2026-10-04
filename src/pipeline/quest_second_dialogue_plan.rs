//! `TOWN00:0x22DC`의 비배포 fixed-slot 재삽입·scene page 계획.
//!
//! 독립 검토 수정안을 새 번역으로 복제하지 않고 단어 단위로 네 행에
//! 나눈다. 원본 control raw와 glyph token 수·바이트 길이는 그대로 두고,
//! 첫 대사의 고정 mapping과 문장부호는 보존하고, 남은 QUEST code table의
//! wide slot을 같은 batch의 공용 Korean page 안에서 한글·문장부호에
//! 재매핑한다. 사람 승인과 실제 화면 layout 전에는 제품 입력으로
//! 승격하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::btx::{decode, encode};
use crate::font::{
    bundled_maplestory_light, rasterize_wide_glyph, QuestCodeTable, QuestGlyphSlot,
    QUEST_FONT_LAYOUT, QUEST_FONT_SHA256, QUEST_FONT_WIDE_CELL_WIDTH, QUEST_FONT_WIDE_SLOTS,
    QUEST_POC_GLYPH_MAPPINGS, QUEST_POC_RASTER_PROFILE,
};
use crate::script::{
    parse_quest_token, parse_quest_tokens, serialize_quest_seq_extracted_text_stream,
    QuestControlMeaning, QuestSeqExtractedTextStream, QuestSeqExtractedTokenKind,
    QuestSeqTextExtractionAnalysis, QuestToken, QuestTranslationIndependentReviewStatus,
    QuestTranslationReviewAnalysis, QuestTranslationReviewRequirement,
};

use super::QUEST_PAGE_SUPPLY_PLAN_TARGET_STREAM_ID;

pub const QUEST_SECOND_DIALOGUE_PLAN_SCHEMA_VERSION: &str = "quest-second-dialogue-plan-v2";
pub const QUEST_SECOND_DIALOGUE_LAYOUT_WORDS: [usize; 4] = [4, 4, 5, 2];
pub const BOX_QUEST_SECOND_DIALOGUE_LINES: usize = 4;
pub const BOX_QUEST_SECOND_DIALOGUE_MAPPINGS: usize = 37;
pub const BOX_QUEST_TWO_DIALOGUE_UNIQUE_MAPPINGS: usize = 42;
pub const BOX_QUEST_TWO_DIALOGUE_SHARED_MAPPINGS: usize = 6;
pub const BOX_QUEST_SECOND_DIALOGUE_REPORT_ROWS: usize =
    BOX_QUEST_SECOND_DIALOGUE_LINES + BOX_QUEST_SECOND_DIALOGUE_MAPPINGS;
pub const BOX_QUEST_SECOND_DIALOGUE_ORIGINAL_STREAM_SHA256: &str =
    "523f08d0b36f8200cae0aba808be2d0922ed2a1acd3aa52011bb1d70315c8fca";
pub const BOX_QUEST_SECOND_DIALOGUE_REPLACED_STREAM_SHA256: &str =
    "2722e47afed583ae99cd9ba5b3b4a9fe72d195f3ea84aed12915f91e1078ca19";
pub const BOX_QUEST_SECOND_DIALOGUE_CONTROL_RAW_SHA256: &str =
    "914e0c00b8638c4b5f67500a8c3b9b81bef41619079039dd1b963dacfe4d4866";
pub const BOX_QUEST_SECOND_DIALOGUE_PAGE_SHA256: &str =
    "f5380c90e1fabc4b55adfbda4e4e690cd3fe9f8571d690d96257b17fcbc50ef8";
pub const BOX_QUEST_SECOND_DIALOGUE_REPORT_SHA256: &str =
    "491786c6795ab201d762a2b5d5555e80b2ca8cee070a6185f8c11374b879e831";

const IDEOGRAPHIC_SPACE: char = '　';
const IDEOGRAPHIC_SPACE_CODE: [u8; 2] = [0x81, 0x40];
const FIRST_DIALOGUE_PRESERVED_CODES: [[u8; 2]; 3] = [[0x81, 0x75], [0x81, 0x60], [0x81, 0x49]];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSecondDialogueLine {
    pub index: usize,
    pub source: String,
    pub reviewed: String,
    pub padded: String,
    pub capacity: usize,
    pub used: usize,
    pub padding: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSecondDialogueGlyphMapping {
    pub glyph: char,
    pub index: usize,
    pub code: [u8; 2],
    pub slot: QuestGlyphSlot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSecondDialoguePlan {
    pub schema_version: &'static str,
    pub target_stream_id: &'static str,
    pub review_status: QuestTranslationIndependentReviewStatus,
    pub lines: Vec<QuestSecondDialogueLine>,
    pub mappings: Vec<QuestSecondDialogueGlyphMapping>,
    pub first_dialogue_mappings: usize,
    pub shared_mappings: usize,
    pub batch_unique_mappings: usize,
    pub source_glyphs: usize,
    pub replacement_glyphs: usize,
    pub control_tokens: usize,
    pub original_stream_bytes: usize,
    pub replacement_stream_bytes: usize,
    pub original_stream_sha256: String,
    pub replaced_stream_sha256: String,
    pub control_raw_sha256: String,
    pub page_bytes: usize,
    pub page_sha256: String,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub build_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSecondDialogueAssets {
    pub original_stream: Vec<u8>,
    pub replacement_stream: Vec<u8>,
    pub page: Vec<u8>,
    pub plan: QuestSecondDialoguePlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSecondDialoguePlanError(String);

impl fmt::Display for QuestSecondDialoguePlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestSecondDialoguePlanError {}

fn error(message: impl Into<String>) -> QuestSecondDialoguePlanError {
    QuestSecondDialoguePlanError(message.into())
}

fn sha256(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn split_words(
    reviewed: &str,
    words_per_line: &[usize],
) -> Result<Vec<String>, QuestSecondDialoguePlanError> {
    let words = reviewed.split(' ').collect::<Vec<_>>();
    if words.iter().any(|word| word.is_empty()) {
        return Err(error(
            "QUEST 두 번째 대사 검토 후보에 연속·경계 공백이 있음",
        ));
    }
    let expected_words = words_per_line.iter().try_fold(0usize, |sum, count| {
        sum.checked_add(*count)
            .ok_or_else(|| error("QUEST 두 번째 대사 layout 단어 수 overflow"))
    })?;
    if words.len() != expected_words {
        return Err(error(format!(
            "QUEST 두 번째 대사 layout 단어 수 불일치: expected={expected_words}, actual={}",
            words.len()
        )));
    }
    let mut lines = Vec::with_capacity(words_per_line.len());
    let mut cursor = 0usize;
    for count in words_per_line {
        let end = cursor
            .checked_add(*count)
            .ok_or_else(|| error("QUEST 두 번째 대사 layout cursor overflow"))?;
        lines.push(words[cursor..end].join(" "));
        cursor = end;
    }
    if lines.join(" ") != reviewed {
        return Err(error("QUEST 두 번째 대사 layout 결합이 검토 후보와 다름"));
    }
    Ok(lines)
}

fn flush_source_line(current: &mut String, lines: &mut Vec<String>) {
    if !current.is_empty() {
        lines.push(std::mem::take(current));
    }
}

fn source_lines(
    stream: &QuestSeqExtractedTextStream,
) -> Result<Vec<String>, QuestSecondDialoguePlanError> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for token in &stream.tokens {
        match token.kind {
            QuestSeqExtractedTokenKind::Glyph => {
                let source = token.source.as_deref().ok_or_else(|| {
                    error(format!(
                        "{}: fixed-slot 표본에 미해독 glyph가 있음: token={}",
                        stream.id(),
                        token.index
                    ))
                })?;
                current.push_str(source);
            }
            QuestSeqExtractedTokenKind::Control => {
                if matches!(
                    token.control_meaning,
                    Some(QuestControlMeaning::AdvanceTextLine)
                        | Some(QuestControlMeaning::StartInputWait)
                ) {
                    flush_source_line(&mut current, &mut lines);
                }
            }
        }
    }
    flush_source_line(&mut current, &mut lines);
    Ok(lines)
}

fn mapping_characters(lines: &[String]) -> BTreeSet<char> {
    lines
        .iter()
        .flat_map(|line| line.chars())
        .filter(|glyph| *glyph != ' ' && *glyph != IDEOGRAPHIC_SPACE)
        .collect()
}

fn glyph_code_is_safe(code: [u8; 2]) -> bool {
    matches!(parse_quest_token(&code, 0), Ok(QuestToken::Glyph(raw)) if raw == code)
}

fn build_mappings(
    code_table: &QuestCodeTable,
    glyphs: &BTreeSet<char>,
) -> Result<Vec<QuestSecondDialogueGlyphMapping>, QuestSecondDialoguePlanError> {
    let first_by_glyph = QUEST_POC_GLYPH_MAPPINGS
        .iter()
        .map(|mapping| (mapping.glyph, *mapping))
        .collect::<BTreeMap<_, _>>();
    for mapping in QUEST_POC_GLYPH_MAPPINGS {
        let slot =
            QuestGlyphSlot::for_index(mapping.index).map_err(|source| error(source.to_string()))?;
        if slot.width != QUEST_FONT_WIDE_CELL_WIDTH
            || code_table.code_at(mapping.index) != Some(mapping.code)
        {
            return Err(error(format!(
                "QUEST 첫 대사 mapping의 code/slot 불일치: glyph={}, index={}",
                mapping.glyph, mapping.index
            )));
        }
    }
    let mut reserved_indices = QUEST_POC_GLYPH_MAPPINGS
        .iter()
        .map(|mapping| mapping.index)
        .collect::<BTreeSet<_>>();
    for code in FIRST_DIALOGUE_PRESERVED_CODES
        .into_iter()
        .chain([IDEOGRAPHIC_SPACE_CODE])
    {
        let index = code_table.index_of(code).ok_or_else(|| {
            error(format!(
                "QUEST 첫 대사 보존 code가 코드표에 없음: {code:02X?}"
            ))
        })?;
        let slot = QuestGlyphSlot::for_index(index).map_err(|source| error(source.to_string()))?;
        if slot.width == QUEST_FONT_WIDE_CELL_WIDTH && !reserved_indices.insert(index) {
            return Err(error(format!(
                "QUEST 첫 대사 보존 code의 wide slot이 기존 예약과 겹침: code={code:02X?}, index={index}"
            )));
        }
    }
    let candidates = code_table
        .codes()
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, code)| {
            let slot = QuestGlyphSlot::for_index(index).ok()?;
            (slot.width == QUEST_FONT_WIDE_CELL_WIDTH
                && !reserved_indices.contains(&index)
                && glyph_code_is_safe(code))
            .then_some((index, code, slot))
        })
        .collect::<Vec<_>>();
    let expected_candidates = QUEST_FONT_WIDE_SLOTS
        .checked_sub(reserved_indices.len())
        .ok_or_else(|| error("QUEST 두 대사 예약 slot underflow"))?;
    let new_glyphs = glyphs
        .iter()
        .filter(|glyph| !first_by_glyph.contains_key(glyph))
        .count();
    if new_glyphs > candidates.len() || candidates.len() != expected_candidates {
        return Err(error(format!(
            "QUEST 두 대사 wide mapping 공급 불일치: demand={new_glyphs}, supply={}, reserved={}",
            candidates.len(),
            reserved_indices.len()
        )));
    }
    let mut candidates = candidates.into_iter();
    glyphs
        .iter()
        .copied()
        .map(|glyph| {
            if let Some(mapping) = first_by_glyph.get(&glyph) {
                if code_table.code_at(mapping.index) != Some(mapping.code) {
                    return Err(error(format!(
                        "QUEST 첫 대사 mapping이 원본 코드표와 다름: glyph={glyph}, index={}",
                        mapping.index
                    )));
                }
                let slot = QuestGlyphSlot::for_index(mapping.index)
                    .map_err(|source| error(source.to_string()))?;
                return Ok(QuestSecondDialogueGlyphMapping {
                    glyph,
                    index: mapping.index,
                    code: mapping.code,
                    slot,
                });
            }
            let (index, code, slot) = candidates
                .next()
                .ok_or_else(|| error("QUEST 두 번째 대사 신규 mapping 공급 고갈"))?;
            Ok(QuestSecondDialogueGlyphMapping {
                glyph,
                index,
                code,
                slot,
            })
        })
        .collect()
}

fn build_page(
    original_font: &[u8],
    mappings: &[QuestSecondDialogueGlyphMapping],
) -> Result<Vec<u8>, QuestSecondDialoguePlanError> {
    if sha256(original_font) != QUEST_FONT_SHA256 {
        return Err(error("QUEST 두 번째 대사 원본 font SHA-256 불일치"));
    }
    let mut pixels =
        decode(original_font, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))?;
    let font = bundled_maplestory_light().map_err(|source| error(source.to_string()))?;
    for mapping in QUEST_POC_GLYPH_MAPPINGS {
        let slot =
            QuestGlyphSlot::for_index(mapping.index).map_err(|source| error(source.to_string()))?;
        let glyph = rasterize_wide_glyph(&font, mapping.glyph, QUEST_POC_RASTER_PROFILE)
            .map_err(|source| error(source.to_string()))?;
        for row in 0..slot.height {
            let source_start = row * slot.width;
            let target_start = (slot.y + row) * QUEST_FONT_LAYOUT.width + slot.x;
            pixels[target_start..target_start + slot.width]
                .copy_from_slice(&glyph[source_start..source_start + slot.width]);
        }
    }
    for mapping in mappings {
        let glyph = rasterize_wide_glyph(&font, mapping.glyph, QUEST_POC_RASTER_PROFILE)
            .map_err(|source| error(source.to_string()))?;
        for row in 0..mapping.slot.height {
            let source_start = row * mapping.slot.width;
            let target_start = (mapping.slot.y + row) * QUEST_FONT_LAYOUT.width + mapping.slot.x;
            pixels[target_start..target_start + mapping.slot.width]
                .copy_from_slice(&glyph[source_start..source_start + mapping.slot.width]);
        }
    }
    let page = encode(&pixels, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))?;
    if decode(&page, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))? != pixels {
        return Err(error("QUEST 두 번째 대사 page encode readback 불일치"));
    }
    Ok(page)
}

fn verify_space_slot(
    code_table: &QuestCodeTable,
    original_font: &[u8],
) -> Result<(), QuestSecondDialoguePlanError> {
    if !glyph_code_is_safe(IDEOGRAPHIC_SPACE_CODE) {
        return Err(error("QUEST ideographic space code가 glyph token이 아님"));
    }
    let slot = code_table
        .slot_for_code(IDEOGRAPHIC_SPACE_CODE)
        .ok_or_else(|| error("QUEST code table에 ideographic space가 없음"))?;
    let pixels =
        decode(original_font, QUEST_FONT_LAYOUT).map_err(|source| error(source.to_string()))?;
    let blank = (0..slot.height).all(|row| {
        let start = (slot.y + row) * QUEST_FONT_LAYOUT.width + slot.x;
        pixels[start..start + slot.width]
            .iter()
            .all(|pixel| *pixel == 0)
    });
    if !blank {
        return Err(error("QUEST ideographic space 원본 slot이 비어 있지 않음"));
    }
    Ok(())
}

fn encode_layout(
    lines: &[QuestSecondDialogueLine],
    mappings: &[QuestSecondDialogueGlyphMapping],
) -> Result<Vec<[u8; 2]>, QuestSecondDialoguePlanError> {
    let by_glyph = mappings
        .iter()
        .map(|mapping| (mapping.glyph, mapping.code))
        .collect::<BTreeMap<_, _>>();
    lines
        .iter()
        .flat_map(|line| line.padded.chars())
        .map(|glyph| {
            if glyph == ' ' || glyph == IDEOGRAPHIC_SPACE {
                Ok(IDEOGRAPHIC_SPACE_CODE)
            } else {
                by_glyph
                    .get(&glyph)
                    .copied()
                    .ok_or_else(|| error(format!("QUEST 두 번째 대사 mapping 누락: {glyph:?}")))
            }
        })
        .collect()
}

fn replace_stream(
    stream: &QuestSeqExtractedTextStream,
    encoded_glyphs: &[[u8; 2]],
) -> Result<(Vec<u8>, Vec<u8>), QuestSecondDialoguePlanError> {
    let mut glyphs = encoded_glyphs.iter();
    let mut replacement = Vec::with_capacity(
        stream
            .byte_len()
            .ok_or_else(|| error("QUEST 두 번째 대사 stream 길이 underflow"))?,
    );
    let mut control_raw = Vec::new();
    replacement.extend_from_slice(&stream.record_raw);
    for token in &stream.tokens {
        match token.kind {
            QuestSeqExtractedTokenKind::Glyph => {
                if token.raw.len() != 2 {
                    return Err(error(format!(
                        "{}: fixed-slot 원본 glyph가 2바이트가 아님: token={}, bytes={}",
                        stream.id(),
                        token.index,
                        token.raw.len()
                    )));
                }
                let code = glyphs
                    .next()
                    .ok_or_else(|| error("QUEST 두 번째 대사 replacement glyph 부족"))?;
                if !glyph_code_is_safe(*code) {
                    return Err(error("QUEST 두 번째 대사 replacement code가 glyph가 아님"));
                }
                replacement.extend_from_slice(code);
            }
            QuestSeqExtractedTokenKind::Control => {
                replacement.extend_from_slice(&token.raw);
                control_raw.extend_from_slice(&token.raw);
            }
        }
    }
    if glyphs.next().is_some() {
        return Err(error("QUEST 두 번째 대사 replacement glyph가 남음"));
    }
    let stream_offset = u32::try_from(stream.stream_offset)
        .map_err(|_| error("QUEST 두 번째 대사 stream offset이 u32를 넘음"))?;
    let body = &replacement[stream.record_raw.len()..];
    let parsed =
        parse_quest_tokens(body, stream_offset).map_err(|source| error(source.to_string()))?;
    let roundtrip = parsed
        .iter()
        .flat_map(|token| token.raw().iter().copied())
        .collect::<Vec<_>>();
    if roundtrip != body || parsed.len() != stream.tokens.len() {
        return Err(error(
            "QUEST 두 번째 대사 replacement token 연속 왕복 불일치",
        ));
    }
    Ok((replacement, control_raw))
}

/// 원본 추출·독립 검토·font에서 두 대사 공용 page의 비배포 준비물을 만든다.
pub fn audit_quest_second_dialogue_plan(
    extraction: &QuestSeqTextExtractionAnalysis,
    reviews: &QuestTranslationReviewAnalysis,
    code_table: &QuestCodeTable,
    original_font: &[u8],
) -> Result<QuestSecondDialogueAssets, QuestSecondDialoguePlanError> {
    let stream = extraction
        .streams
        .iter()
        .find(|stream| stream.id() == QUEST_PAGE_SUPPLY_PLAN_TARGET_STREAM_ID)
        .ok_or_else(|| error("QUEST 두 번째 대사 추출 stream이 없음"))?;
    let review = reviews
        .entries
        .iter()
        .find(|entry| entry.batch_entry.stream_id == QUEST_PAGE_SUPPLY_PLAN_TARGET_STREAM_ID)
        .ok_or_else(|| error("QUEST 두 번째 대사 review entry가 없음"))?;
    if review.requirement != QuestTranslationReviewRequirement::RuntimeSampleIndependentReview {
        return Err(error("QUEST 두 번째 대사가 runtime 독립 검토 표본이 아님"));
    }
    let review_status = review
        .independent_review_status
        .ok_or_else(|| error("QUEST 두 번째 대사 독립 검토 결과가 없음"))?;
    if review.reviewed_korean.is_empty() {
        return Err(error("QUEST 두 번째 대사 검토 수정안이 비어 있음"));
    }

    let original_stream = serialize_quest_seq_extracted_text_stream(stream)
        .map_err(|source| error(source.to_string()))?;
    let source_lines = source_lines(stream)?;
    if source_lines.concat() != review.batch_entry.protected_source {
        return Err(error(
            "QUEST 두 번째 대사 원문 행이 review 보호 원문과 다름",
        ));
    }
    let reviewed_lines = split_words(&review.reviewed_korean, &QUEST_SECOND_DIALOGUE_LAYOUT_WORDS)?;
    if source_lines.len() != BOX_QUEST_SECOND_DIALOGUE_LINES
        || reviewed_lines.len() != BOX_QUEST_SECOND_DIALOGUE_LINES
    {
        return Err(error(format!(
            "QUEST 두 번째 대사 행 수 불일치: source={}, reviewed={}",
            source_lines.len(),
            reviewed_lines.len()
        )));
    }
    let mut lines = Vec::with_capacity(BOX_QUEST_SECOND_DIALOGUE_LINES);
    for (index, (source, reviewed)) in source_lines.into_iter().zip(reviewed_lines).enumerate() {
        let capacity = source.chars().count();
        let used = reviewed.chars().count();
        let padding = capacity.checked_sub(used).ok_or_else(|| {
            error(format!(
                "QUEST 두 번째 대사 layout이 원본 행을 넘음: line={}, capacity={capacity}, used={used}",
                index + 1
            ))
        })?;
        let mut padded = reviewed.clone();
        padded.extend(std::iter::repeat_n(IDEOGRAPHIC_SPACE, padding));
        lines.push(QuestSecondDialogueLine {
            index: index + 1,
            source,
            reviewed,
            padded,
            capacity,
            used,
            padding,
        });
    }
    let glyphs = mapping_characters(
        &lines
            .iter()
            .map(|line| line.reviewed.clone())
            .collect::<Vec<_>>(),
    );
    verify_space_slot(code_table, original_font)?;
    let mappings = build_mappings(code_table, &glyphs)?;
    let first_glyphs = QUEST_POC_GLYPH_MAPPINGS
        .iter()
        .map(|mapping| mapping.glyph)
        .collect::<BTreeSet<_>>();
    let shared_mappings = glyphs.intersection(&first_glyphs).count();
    let batch_unique_mappings = glyphs.union(&first_glyphs).count();
    let encoded_glyphs = encode_layout(&lines, &mappings)?;
    let (replacement_stream, control_raw) = replace_stream(stream, &encoded_glyphs)?;
    if replacement_stream.len() != original_stream.len() {
        return Err(error("QUEST 두 번째 대사 fixed-slot byte 길이가 바뀜"));
    }
    let page = build_page(original_font, &mappings)?;
    let source_glyphs = stream
        .tokens
        .iter()
        .filter(|token| token.kind == QuestSeqExtractedTokenKind::Glyph)
        .count();
    let control_tokens = stream.tokens.len() - source_glyphs;

    Ok(QuestSecondDialogueAssets {
        plan: QuestSecondDialoguePlan {
            schema_version: QUEST_SECOND_DIALOGUE_PLAN_SCHEMA_VERSION,
            target_stream_id: QUEST_PAGE_SUPPLY_PLAN_TARGET_STREAM_ID,
            review_status,
            lines,
            mappings,
            first_dialogue_mappings: first_glyphs.len(),
            shared_mappings,
            batch_unique_mappings,
            source_glyphs,
            replacement_glyphs: encoded_glyphs.len(),
            control_tokens,
            original_stream_bytes: original_stream.len(),
            replacement_stream_bytes: replacement_stream.len(),
            original_stream_sha256: sha256(&original_stream),
            replaced_stream_sha256: sha256(&replacement_stream),
            control_raw_sha256: sha256(&control_raw),
            page_bytes: page.len(),
            page_sha256: sha256(&page),
            translation_approved: false,
            layout_approved: false,
            runtime_verified: false,
            build_eligible: false,
        },
        original_stream,
        replacement_stream,
        page,
    })
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

/// 검토 가능한 원문 행·배치 후보와 scene page mapping을 TSV로 만든다.
pub fn render_quest_second_dialogue_plan_tsv(plan: &QuestSecondDialoguePlan) -> String {
    let mut out = String::from(
        "schema_version\trow_type\trow_id\tsource\treviewed\tpadded\tcapacity\tused\t\
         padding\tglyph\tindex\tcode\tslot_x\tslot_y\tfirst_dialogue_mappings\t\
         shared_mappings\tbatch_unique_mappings\tpage_sha256\t\
         replaced_stream_sha256\ttranslation_approved\tlayout_approved\t\
         runtime_verified\tbuild_eligible\tnotes\n",
    );
    for line in &plan.lines {
        writeln!(
            out,
            "{}\tline\tline-{}\t{}\t{}\t{}\t{}\t{}\t{}\t\t\t\t\t\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t원본 control 경계별 fixed-slot layout 후보",
            plan.schema_version,
            line.index,
            escape_tsv(&line.source),
            escape_tsv(&line.reviewed),
            escape_tsv(&line.padded),
            line.capacity,
            line.used,
            line.padding,
            plan.first_dialogue_mappings,
            plan.shared_mappings,
            plan.batch_unique_mappings,
            plan.page_sha256,
            plan.replaced_stream_sha256,
            plan.translation_approved,
            plan.layout_approved,
            plan.runtime_verified,
            plan.build_eligible,
        )
        .expect("String write는 실패하지 않음");
    }
    for mapping in &plan.mappings {
        writeln!(
            out,
            "{}\tmapping\tU+{:04X}\t\t\t\t\t\t\t{}\t{}\t{:02X} {:02X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t같은 batch의 공용 Korean page 안에서만 유효한 임시 mapping",
            plan.schema_version,
            mapping.glyph as u32,
            mapping.glyph,
            mapping.index,
            mapping.code[0],
            mapping.code[1],
            mapping.slot.x,
            mapping.slot.y,
            plan.first_dialogue_mappings,
            plan.shared_mappings,
            plan.batch_unique_mappings,
            plan.page_sha256,
            plan.replaced_stream_sha256,
            plan.translation_approved,
            plan.layout_approved,
            plan.runtime_verified,
            plan.build_eligible,
        )
        .expect("String write는 실패하지 않음");
    }
    out
}

/// 지원 원본의 두 번째 대사 계획 수치와 결정적 report를 고정한다.
pub fn verify_box_quest_second_dialogue_plan(
    extraction: &QuestSeqTextExtractionAnalysis,
    reviews: &QuestTranslationReviewAnalysis,
    code_table: &QuestCodeTable,
    original_font: &[u8],
) -> Result<QuestSecondDialogueAssets, QuestSecondDialoguePlanError> {
    let assets = audit_quest_second_dialogue_plan(extraction, reviews, code_table, original_font)?;
    let plan = &assets.plan;
    let capacities = plan
        .lines
        .iter()
        .map(|line| line.capacity)
        .collect::<Vec<_>>();
    let used = plan.lines.iter().map(|line| line.used).collect::<Vec<_>>();
    let padding = plan
        .lines
        .iter()
        .map(|line| line.padding)
        .collect::<Vec<_>>();
    if plan.review_status != QuestTranslationIndependentReviewStatus::ReviewProposesChange
        || capacities != [15, 16, 17, 15]
        || used != [13, 15, 16, 10]
        || padding != [2, 1, 1, 5]
        || plan.mappings.len() != BOX_QUEST_SECOND_DIALOGUE_MAPPINGS
        || plan.first_dialogue_mappings != QUEST_POC_GLYPH_MAPPINGS.len()
        || plan.shared_mappings != BOX_QUEST_TWO_DIALOGUE_SHARED_MAPPINGS
        || plan.batch_unique_mappings != BOX_QUEST_TWO_DIALOGUE_UNIQUE_MAPPINGS
        || plan.source_glyphs != 63
        || plan.replacement_glyphs != 63
        || plan.control_tokens != 10
        || plan.original_stream_bytes != 164
        || plan.replacement_stream_bytes != 164
        || plan.original_stream_sha256 != BOX_QUEST_SECOND_DIALOGUE_ORIGINAL_STREAM_SHA256
        || plan.replaced_stream_sha256 != BOX_QUEST_SECOND_DIALOGUE_REPLACED_STREAM_SHA256
        || plan.control_raw_sha256 != BOX_QUEST_SECOND_DIALOGUE_CONTROL_RAW_SHA256
        || plan.page_bytes != 32_768
        || plan.page_sha256 != BOX_QUEST_SECOND_DIALOGUE_PAGE_SHA256
        || plan.translation_approved
        || plan.layout_approved
        || plan.runtime_verified
        || plan.build_eligible
    {
        return Err(error(format!(
            "QUEST 두 번째 대사 source gate drift: {plan:?}"
        )));
    }
    let report = render_quest_second_dialogue_plan_tsv(plan);
    let columns = report
        .lines()
        .map(|line| line.split('\t').count())
        .collect::<BTreeSet<_>>();
    if report.lines().count() != BOX_QUEST_SECOND_DIALOGUE_REPORT_ROWS + 1 || columns.len() != 1 {
        return Err(error(format!(
            "QUEST 두 번째 대사 TSV shape 불일치: rows={}, columns={columns:?}",
            report.lines().count()
        )));
    }
    let report_sha256 = sha256(report.as_bytes());
    if report_sha256 != BOX_QUEST_SECOND_DIALOGUE_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST 두 번째 대사 TSV SHA-256 불일치: expected={}, actual={report_sha256}",
            BOX_QUEST_SECOND_DIALOGUE_REPORT_SHA256
        )));
    }
    Ok(assets)
}

#[cfg(test)]
#[path = "quest_second_dialogue_plan_tests.rs"]
mod tests;
