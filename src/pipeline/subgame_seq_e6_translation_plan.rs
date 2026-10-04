//! PUYO2 `E6` direct text의 전역 token 배정과 제자리 block 재포장.
//!
//! 원본 256-entry code table은 바꾸지 않고 first-match가 가능한 2바이트
//! pair를 불투명 token으로 재사용한다. 번역에 남기는 기존 비일본어 문자는
//! 원래 cell을 예약하고, 한글 음절은 남은 도달 가능 cell에 결정적으로
//! 배정한다. 같은 SEQ의 E6 stream은 연속 block이므로 번역 stream을 다시
//! 이어 붙이고 command pointer만 갱신한다. block 밖과 overlay footer는
//! 변경하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::disasm::{
    SubgameSeqE6TextAnalysis, SubgameSeqE6TextRow, DEMO_FONT_CODE_TABLE_ENTRIES,
    DEMO_FONT_CODE_TABLE_SHA256,
};
use crate::script::SUBGAME_SEQ_OVERLAY_FOOTER_LEN;
use crate::text::decode_sjis;

pub use crate::graphics::e6_demo_font_patch::SubgameSeqE6GlyphMapping;

pub const SUBGAME_SEQ_E6_TRANSLATION_PLAN_SCHEMA_VERSION: &str =
    "subgame-seq-e6-translation-plan-v1";
pub const PUYO2_E6_REACHABLE_CODE_TABLE_SLOTS: usize = 252;
pub const BOX_PUYO2_E6_TRANSLATION_PLAN_OVERLAYS: usize = 33;
pub const BOX_PUYO2_E6_TRANSLATION_PLAN_STREAMS: usize = 59;
pub const BOX_PUYO2_E6_TRANSLATION_PLAN_RETAINED_CHARACTERS: usize = 0;
pub const BOX_PUYO2_E6_TRANSLATION_PLAN_SYNTHETIC_GLYPHS: usize = 1;
pub const BOX_PUYO2_E6_TRANSLATION_PLAN_REMAINING_SLOTS: usize = 251;
pub const BOX_PUYO2_E6_TRANSLATION_PLAN_CAPACITY_BYTES: usize = 2_964;
pub const BOX_PUYO2_E6_TRANSLATION_PLAN_USED_BYTES: usize = 664;
pub const BOX_PUYO2_E6_TRANSLATION_PLAN_REMAINING_BYTES: usize = 2_300;
pub const BOX_PUYO2_E6_TRANSLATION_PLAN_REPORT_SHA256: &str =
    "27ef9c125b42562faa345cfe8c0db2a95abadc7bf5809c159eb56b16b84d8e4c";

const PUYO2_SCOPE: &str = "PUYO2";
const PUYO2_GLYPH_ASSET: &str = "DEMOFONT.BTX";
const TERMINATOR: [u8; 2] = [0x65, 0x65];
const CONTROL_70: [u8; 2] = [0x70, 0x70];
const CONTROL_72: [u8; 2] = [0x72, 0x72];
const CONTROL_77: [u8; 2] = [0x77, 0x24];
const TOKEN_TABLE_WORDS: usize = 100;

/// 한 E6 stream의 span별 Unicode 번역.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6Translation {
    pub sequence: String,
    pub sequence_index: usize,
    pub spans: Vec<String>,
}

/// 한 원본 stream을 대체할 가변 길이 byte열과 relink 전제.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6StreamReplacement {
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub sequence_index: usize,
    pub pointer_field_offset: usize,
    pub original_stream_offset: usize,
    pub original_stream_runtime: u32,
    pub original: Vec<u8>,
    pub replacement: Vec<u8>,
    pub glyphs: usize,
    pub controls: usize,
}

/// PUYO2 전체 E6 번역의 전역 glyph와 stream 계획.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6TranslationPlan {
    pub code_table_sha256: String,
    pub reachable_slots: usize,
    pub retained_source_characters: usize,
    pub glyph_mappings: Vec<SubgameSeqE6GlyphMapping>,
    pub replacements: Vec<SubgameSeqE6StreamReplacement>,
    pub capacity_bytes: usize,
    pub used_bytes: usize,
    pub remaining_bytes: usize,
    pub remaining_slots: usize,
}

/// 한 SEQ의 연속 E6 block 재포장 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6SequenceRepack {
    pub sequence: String,
    pub sequence_bytes: Vec<u8>,
    pub block_start_offset: usize,
    pub block_end_offset: usize,
    pub capacity_bytes: usize,
    pub used_bytes: usize,
    pub remaining_bytes: usize,
    pub pointer_relations: usize,
    pub stream_offsets: Vec<usize>,
}

/// 합성 한 글자로 검산한 한 PUYO2 E6 overlay의 재포장 행.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6TranslationPlanRow {
    pub sequence: String,
    pub streams: usize,
    pub capacity_bytes: usize,
    pub used_bytes: usize,
    pub remaining_bytes: usize,
    pub pointer_relations: usize,
    pub block_start_offset: usize,
    pub block_end_offset: usize,
    pub output_sha256: String,
}

/// 지원 원본 전체에 합성 한 글자를 적용한 정적 전략 검산.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6TranslationPlanAnalysis {
    pub plan: SubgameSeqE6TranslationPlan,
    pub rows: Vec<SubgameSeqE6TranslationPlanRow>,
    pub pointer_relations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6TranslationPlanError(String);

impl fmt::Display for SubgameSeqE6TranslationPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqE6TranslationPlanError {}

fn error(message: impl Into<String>) -> SubgameSeqE6TranslationPlanError {
    SubgameSeqE6TranslationPlanError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CodeSlot {
    index: usize,
    pair: [u8; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodeTable {
    sha256: String,
    pairs: BTreeSet<[u8; 2]>,
    first_slots: Vec<CodeSlot>,
    by_character: BTreeMap<char, CodeSlot>,
}

fn parse_code_table(
    raw: &[u8],
    expected_entries: usize,
    expected_sha256: Option<&str>,
) -> Result<CodeTable, SubgameSeqE6TranslationPlanError> {
    if raw.len() != expected_entries * 2 {
        return Err(error(format!(
            "PUYO2 E6 code table 길이 불일치: expected={}, actual={}",
            expected_entries * 2,
            raw.len()
        )));
    }
    let sha256 = digest(raw);
    if let Some(expected) = expected_sha256 {
        if expected != sha256 {
            return Err(error(format!(
                "PUYO2 E6 code table SHA-256 불일치: expected={expected}, actual={sha256}"
            )));
        }
    }
    let mut pairs = BTreeSet::new();
    let mut first_slots = Vec::new();
    let mut by_character = BTreeMap::new();
    for (index, pair) in raw.chunks_exact(2).enumerate() {
        let pair: [u8; 2] = pair.try_into().expect("2바이트 code pair");
        let decoded = decode_sjis(&pair)
            .map_err(|source| error(format!("PUYO2 E6 code pair decode 실패: {source}")))?;
        let mut characters = decoded.chars();
        let glyph = characters
            .next()
            .ok_or_else(|| error("PUYO2 E6 code pair가 빈 문자열로 decode됨"))?;
        if characters.next().is_some() {
            return Err(error("PUYO2 E6 code pair가 한 문자가 아님"));
        }
        if pairs.insert(pair) {
            let slot = CodeSlot { index, pair };
            first_slots.push(slot);
            by_character.entry(glyph).or_insert(slot);
        }
    }
    Ok(CodeTable {
        sha256,
        pairs,
        first_slots,
        by_character,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TokenShape {
    controls: Vec<[u8; 2]>,
    glyphs: usize,
}

fn token_shape(
    raw: &[u8],
    code_pairs: &BTreeSet<[u8; 2]>,
) -> Result<TokenShape, SubgameSeqE6TranslationPlanError> {
    if raw.is_empty() || !raw.len().is_multiple_of(2) {
        return Err(error("PUYO2 E6 stream 길이가 2바이트 정렬이 아님"));
    }
    let mut controls = Vec::new();
    let mut glyphs = 0usize;
    for (index, pair) in raw.chunks_exact(2).enumerate() {
        let pair: [u8; 2] = pair.try_into().expect("2바이트 E6 token");
        if pair == TERMINATOR {
            if index + 1 != raw.len() / 2 {
                return Err(error("PUYO2 E6 stream 종결 뒤 trailing token이 있음"));
            }
            let emitted = glyphs + controls.len() + 4;
            if raw.len() / 2 > TOKEN_TABLE_WORDS || emitted > TOKEN_TABLE_WORDS {
                return Err(error("PUYO2 E6 stream 100-word token table 용량 초과"));
            }
            return Ok(TokenShape { controls, glyphs });
        }
        if matches!(pair, CONTROL_70 | CONTROL_72 | CONTROL_77) {
            controls.push(pair);
        } else if code_pairs.contains(&pair) {
            glyphs += 1;
        } else {
            return Err(error(format!(
                "PUYO2 E6 stream에 미등록 pair가 있음: {:02X}{:02X}",
                pair[0], pair[1]
            )));
        }
    }
    Err(error("PUYO2 E6 stream 종결쌍이 없음"))
}

fn compile_stream(
    row: &SubgameSeqE6TextRow,
    translation: &SubgameSeqE6Translation,
    code_table: &CodeTable,
    retained: &BTreeMap<char, CodeSlot>,
    mappings: &BTreeMap<char, SubgameSeqE6GlyphMapping>,
) -> Result<SubgameSeqE6StreamReplacement, SubgameSeqE6TranslationPlanError> {
    if row.spans.len() != translation.spans.len() {
        return Err(error(format!(
            "{}#{} E6 번역 span 분모 불일치: expected={}, actual={}",
            row.sequence,
            row.sequence_index,
            row.spans.len(),
            translation.spans.len()
        )));
    }
    let mut replacement = Vec::new();
    let mut source_cursor = 0usize;
    for (source_span, translated) in row.spans.iter().zip(&translation.spans) {
        if translated.is_empty() {
            return Err(error(format!(
                "{}#{} E6 번역 span이 비어 있음",
                row.sequence, row.sequence_index
            )));
        }
        let relative_start = source_span
            .offset
            .checked_sub(row.stream_offset)
            .ok_or_else(|| error("PUYO2 E6 span offset underflow"))?;
        let relative_end = relative_start
            .checked_add(source_span.raw.len())
            .ok_or_else(|| error("PUYO2 E6 span 끝 overflow"))?;
        if relative_start < source_cursor || relative_end > row.raw.len() {
            return Err(error("PUYO2 E6 span 범위/순서 불일치"));
        }
        replacement.extend_from_slice(&row.raw[source_cursor..relative_start]);
        for glyph in translated.chars() {
            let pair = if let Some(mapping) = mappings.get(&glyph) {
                mapping.pair
            } else {
                retained
                    .get(&glyph)
                    .map(|slot| slot.pair)
                    .ok_or_else(|| error(format!("PUYO2 E6 번역 문자 mapping 누락: {glyph}")))?
            };
            replacement.extend_from_slice(&pair);
        }
        source_cursor = relative_end;
    }
    replacement.extend_from_slice(&row.raw[source_cursor..]);

    let original_shape = token_shape(&row.raw, &code_table.pairs)?;
    let replacement_shape = token_shape(&replacement, &code_table.pairs)?;
    if original_shape.controls != replacement_shape.controls {
        return Err(error(format!(
            "{}#{} E6 control 순서 변경",
            row.sequence, row.sequence_index
        )));
    }
    Ok(SubgameSeqE6StreamReplacement {
        sequence: row.sequence.clone(),
        sequence_sha256: row.sequence_sha256,
        sequence_index: row.sequence_index,
        pointer_field_offset: row.pointer_field_offset,
        original_stream_offset: row.stream_offset,
        original_stream_runtime: row.stream_runtime,
        original: row.raw.clone(),
        replacement,
        glyphs: replacement_shape.glyphs,
        controls: replacement_shape.controls.len(),
    })
}

fn plan_translation(
    translations: &[SubgameSeqE6Translation],
    code_table_raw: &[u8],
    analysis: &SubgameSeqE6TextAnalysis,
    expected_entries: usize,
    expected_sha256: Option<&str>,
    require_box_rows: bool,
) -> Result<SubgameSeqE6TranslationPlan, SubgameSeqE6TranslationPlanError> {
    let code_table = parse_code_table(code_table_raw, expected_entries, expected_sha256)?;
    let mut rows = analysis
        .rows
        .iter()
        .filter(|row| row.scope == PUYO2_SCOPE)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| (row.sequence.as_str(), row.sequence_index));
    if rows.is_empty()
        || (require_box_rows
            && (rows.len() != crate::disasm::BOX_SUBGAME_SEQ_E6_TEXT_ROWS_PER_EXE
                || code_table.first_slots.len() != PUYO2_E6_REACHABLE_CODE_TABLE_SLOTS))
        || rows.iter().any(|row| row.glyph_asset != PUYO2_GLYPH_ASSET)
    {
        return Err(error("PUYO2 E6 translation source 분모 불일치"));
    }

    let mut translation_map = BTreeMap::new();
    for translation in translations {
        let key = (translation.sequence.clone(), translation.sequence_index);
        if translation_map.insert(key.clone(), translation).is_some() {
            return Err(error(format!("PUYO2 E6 번역 ID 중복: {}#{}", key.0, key.1)));
        }
    }
    let row_keys = rows
        .iter()
        .map(|row| (row.sequence.clone(), row.sequence_index))
        .collect::<BTreeSet<_>>();
    if translation_map.keys().cloned().collect::<BTreeSet<_>>() != row_keys {
        return Err(error("PUYO2 E6 번역 입력 누락 또는 미등록 ID"));
    }

    let mut retained = BTreeMap::new();
    let mut retained_indices = BTreeSet::new();
    let mut hangul_order = Vec::new();
    let mut hangul_set = BTreeSet::new();
    for row in &rows {
        let translation = translation_map
            .get(&(row.sequence.clone(), row.sequence_index))
            .expect("번역 key 집합을 검증함");
        if translation.spans.len() != row.spans.len() {
            return Err(error(format!(
                "{}#{} E6 번역 span 분모 불일치",
                row.sequence, row.sequence_index
            )));
        }
        for span in &translation.spans {
            if span.is_empty() {
                return Err(error(format!(
                    "{}#{} E6 번역 span이 비어 있음",
                    row.sequence, row.sequence_index
                )));
            }
            for glyph in span.chars() {
                if is_hangul_syllable(glyph) {
                    if hangul_set.insert(glyph) {
                        hangul_order.push(glyph);
                    }
                } else if is_japanese_text_character(glyph) {
                    return Err(error(format!(
                        "{}#{} E6 번역에 일본어가 남음: {glyph}",
                        row.sequence, row.sequence_index
                    )));
                } else {
                    let slot = code_table
                        .by_character
                        .get(&glyph)
                        .copied()
                        .ok_or_else(|| {
                            error(format!("PUYO2 E6 code table에 번역 문자가 없음: {glyph}"))
                        })?;
                    retained.insert(glyph, slot);
                    retained_indices.insert(slot.index);
                }
            }
        }
    }

    let free_slots = code_table
        .first_slots
        .iter()
        .copied()
        .filter(|slot| !retained_indices.contains(&slot.index))
        .collect::<Vec<_>>();
    if hangul_order.len() > free_slots.len() {
        return Err(error(format!(
            "PUYO2 E6 전역 glyph 용량 초과: required={}, available={}",
            hangul_order.len(),
            free_slots.len()
        )));
    }
    let glyph_mappings = hangul_order
        .into_iter()
        .zip(free_slots)
        .map(|(glyph, slot)| SubgameSeqE6GlyphMapping {
            glyph,
            cell_index: slot.index,
            pair: slot.pair,
        })
        .collect::<Vec<_>>();
    let mapping_map = glyph_mappings
        .iter()
        .copied()
        .map(|mapping| (mapping.glyph, mapping))
        .collect::<BTreeMap<_, _>>();

    let mut replacements = Vec::with_capacity(rows.len());
    for row in &rows {
        let translation = translation_map
            .get(&(row.sequence.clone(), row.sequence_index))
            .expect("번역 key 집합을 검증함");
        replacements.push(compile_stream(
            row,
            translation,
            &code_table,
            &retained,
            &mapping_map,
        )?);
    }
    let mut capacity_bytes = 0usize;
    let mut used_bytes = 0usize;
    for sequence in replacements
        .iter()
        .map(|row| row.sequence.as_str())
        .collect::<BTreeSet<_>>()
    {
        let mut group = replacements
            .iter()
            .filter(|row| row.sequence == sequence)
            .collect::<Vec<_>>();
        group.sort_by_key(|row| row.original_stream_offset);
        if group.windows(2).any(|pair| {
            pair[0].original_stream_offset + pair[0].original.len()
                != pair[1].original_stream_offset
        }) {
            return Err(error(format!(
                "{sequence} E6 원본 stream block이 연속이 아님"
            )));
        }
        let capacity = group.iter().map(|row| row.original.len()).sum::<usize>();
        let used = group.iter().map(|row| row.replacement.len()).sum::<usize>();
        if used > capacity {
            return Err(error(format!(
                "{sequence} E6 번역 block 용량 초과: used={used}, capacity={capacity}"
            )));
        }
        capacity_bytes += capacity;
        used_bytes += used;
    }
    let used_slots = retained_indices.len() + glyph_mappings.len();
    Ok(SubgameSeqE6TranslationPlan {
        code_table_sha256: code_table.sha256,
        reachable_slots: code_table.first_slots.len(),
        retained_source_characters: retained_indices.len(),
        glyph_mappings,
        replacements,
        capacity_bytes,
        used_bytes,
        remaining_bytes: capacity_bytes - used_bytes,
        remaining_slots: code_table.first_slots.len() - used_slots,
    })
}

/// 지원 원본 PUYO2 code table과 59개 E6 stream을 번역 계획으로 컴파일한다.
pub fn plan_box_puyo2_subgame_seq_e6_translation(
    translations: &[SubgameSeqE6Translation],
    code_table_raw: &[u8],
    analysis: &SubgameSeqE6TextAnalysis,
) -> Result<SubgameSeqE6TranslationPlan, SubgameSeqE6TranslationPlanError> {
    plan_translation(
        translations,
        code_table_raw,
        analysis,
        DEMO_FONT_CODE_TABLE_ENTRIES,
        Some(DEMO_FONT_CODE_TABLE_SHA256),
        true,
    )
}

fn read_u32(raw: &[u8], offset: usize) -> Option<u32> {
    raw.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
}

/// 한 PUYO2 overlay의 연속 E6 stream block을 제자리에서 다시 포장한다.
pub fn repack_box_puyo2_subgame_seq_e6_sequence(
    sequence: &str,
    raw: &[u8],
    plan: &SubgameSeqE6TranslationPlan,
) -> Result<SubgameSeqE6SequenceRepack, SubgameSeqE6TranslationPlanError> {
    let mut rows = plan
        .replacements
        .iter()
        .filter(|row| row.sequence == sequence)
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.original_stream_offset);
    if rows.is_empty() {
        return Err(error(format!("{sequence} E6 replacement가 없음")));
    }
    let expected_sha256 = rows[0].sequence_sha256;
    if rows
        .iter()
        .any(|row| row.sequence_sha256 != expected_sha256)
        || Sha256::digest(raw).as_slice() != expected_sha256
    {
        return Err(error(format!("{sequence} E6 원본 SHA-256 불일치")));
    }
    if raw.len() < SUBGAME_SEQ_OVERLAY_FOOTER_LEN {
        return Err(error(format!("{sequence} E6 overlay가 footer보다 짧음")));
    }
    let initialized_len = raw.len() - SUBGAME_SEQ_OVERLAY_FOOTER_LEN;
    for pair in rows.windows(2) {
        if pair[0].original_stream_offset + pair[0].original.len() != pair[1].original_stream_offset
        {
            return Err(error(format!("{sequence} E6 stream block이 연속이 아님")));
        }
    }
    let block_start = rows[0].original_stream_offset;
    let block_end = rows
        .last()
        .expect("행이 비어 있지 않음")
        .original_stream_offset
        + rows.last().expect("행이 비어 있지 않음").original.len();
    if block_end > initialized_len {
        return Err(error(format!(
            "{sequence} E6 stream block이 initialized 영역 밖임"
        )));
    }
    for row in &rows {
        let end = row.original_stream_offset + row.original.len();
        if raw.get(row.original_stream_offset..end) != Some(row.original.as_slice()) {
            return Err(error(format!(
                "{}#{} E6 원본 stream 불일치",
                sequence, row.sequence_index
            )));
        }
    }
    let load_base = rows[0]
        .original_stream_runtime
        .checked_sub(
            u32::try_from(rows[0].original_stream_offset)
                .map_err(|_| error(format!("{sequence} E6 stream offset이 u32 범위를 넘음")))?,
        )
        .ok_or_else(|| error(format!("{sequence} E6 load base underflow")))?;
    let expected_fields = rows
        .iter()
        .map(|row| row.pointer_field_offset)
        .collect::<BTreeSet<_>>();
    if expected_fields.len() != rows.len() {
        return Err(error(format!("{sequence} E6 pointer field 중복")));
    }
    for row in &rows {
        if row.pointer_field_offset + 4 > initialized_len
            || read_u32(raw, row.pointer_field_offset) != Some(row.original_stream_runtime)
            || row.original_stream_runtime.checked_sub(
                u32::try_from(row.original_stream_offset)
                    .map_err(|_| error(format!("{sequence} E6 offset overflow")))?,
            ) != Some(load_base)
        {
            return Err(error(format!(
                "{}#{} E6 pointer/load base 전제 불일치",
                sequence, row.sequence_index
            )));
        }
    }
    let block_runtime_start = load_base
        .checked_add(u32::try_from(block_start).map_err(|_| error("E6 block 시작 overflow"))?)
        .ok_or_else(|| error("E6 block runtime 시작 overflow"))?;
    let block_runtime_end = load_base
        .checked_add(u32::try_from(block_end).map_err(|_| error("E6 block 끝 overflow"))?)
        .ok_or_else(|| error("E6 block runtime 끝 overflow"))?;
    let actual_fields = (0..initialized_len)
        .step_by(4)
        .filter(|offset| {
            read_u32(raw, *offset)
                .is_some_and(|value| (block_runtime_start..block_runtime_end).contains(&value))
        })
        .collect::<BTreeSet<_>>();
    if actual_fields != expected_fields {
        return Err(error(format!(
            "{sequence} E6 block incoming pointer 분모 불일치: expected={expected_fields:?}, actual={actual_fields:?}"
        )));
    }

    let capacity_bytes = block_end - block_start;
    let used_bytes = rows.iter().map(|row| row.replacement.len()).sum::<usize>();
    if used_bytes > capacity_bytes {
        return Err(error(format!(
            "{sequence} E6 replacement 용량 초과: used={used_bytes}, capacity={capacity_bytes}"
        )));
    }
    let mut sequence_bytes = raw.to_vec();
    sequence_bytes[block_start..block_end].fill(0);
    let mut cursor = block_start;
    let mut stream_offsets = Vec::with_capacity(rows.len());
    for row in &rows {
        let end = cursor
            .checked_add(row.replacement.len())
            .ok_or_else(|| error(format!("{sequence} E6 replacement cursor overflow")))?;
        sequence_bytes[cursor..end].copy_from_slice(&row.replacement);
        let runtime = load_base
            .checked_add(u32::try_from(cursor).map_err(|_| error("E6 새 offset overflow"))?)
            .ok_or_else(|| error("E6 새 runtime overflow"))?;
        sequence_bytes[row.pointer_field_offset..row.pointer_field_offset + 4]
            .copy_from_slice(&runtime.to_le_bytes());
        stream_offsets.push(cursor);
        cursor = end;
    }
    let pointer_ranges = rows
        .iter()
        .map(|row| row.pointer_field_offset..row.pointer_field_offset + 4)
        .collect::<Vec<_>>();
    for (offset, (&before, &after)) in raw.iter().zip(&sequence_bytes).enumerate() {
        if before == after {
            continue;
        }
        let allowed = (block_start..block_end).contains(&offset)
            || pointer_ranges.iter().any(|range| range.contains(&offset));
        if !allowed {
            return Err(error(format!(
                "{sequence} E6 재포장 비대상 byte 변경: +0x{offset:04X}"
            )));
        }
    }
    Ok(SubgameSeqE6SequenceRepack {
        sequence: sequence.to_string(),
        sequence_bytes,
        block_start_offset: block_start,
        block_end_offset: block_end,
        capacity_bytes,
        used_bytes,
        remaining_bytes: capacity_bytes - used_bytes,
        pointer_relations: rows.len(),
        stream_offsets,
    })
}

/// 지원 원본 33개 PUYO2 E6 overlay를 합성 `가` 입력으로 전수 재포장한다.
pub fn audit_box_puyo2_subgame_seq_e6_translation_plan<'a>(
    code_table_raw: &[u8],
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    text: &SubgameSeqE6TextAnalysis,
) -> Result<SubgameSeqE6TranslationPlanAnalysis, SubgameSeqE6TranslationPlanError> {
    let translations = text
        .rows
        .iter()
        .filter(|row| row.scope == PUYO2_SCOPE)
        .map(|row| SubgameSeqE6Translation {
            sequence: row.sequence.clone(),
            sequence_index: row.sequence_index,
            spans: vec!["가".to_string(); row.spans.len()],
        })
        .collect::<Vec<_>>();
    let plan = plan_box_puyo2_subgame_seq_e6_translation(&translations, code_table_raw, text)?;
    let mut source_map = BTreeMap::new();
    for (archive, sequence, raw) in sources {
        if archive != "/PUYO2/PP2_SEQ.VDD;1" {
            continue;
        }
        if source_map.insert(sequence.to_string(), raw).is_some() {
            return Err(error(format!("PUYO2 E6 source 중복: {sequence}")));
        }
    }
    let sequences = plan
        .replacements
        .iter()
        .map(|row| row.sequence.as_str())
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::with_capacity(sequences.len());
    for sequence in sequences {
        let raw = source_map
            .get(sequence)
            .ok_or_else(|| error(format!("PUYO2 E6 source 누락: {sequence}")))?;
        let repack = repack_box_puyo2_subgame_seq_e6_sequence(sequence, raw, &plan)?;
        rows.push(SubgameSeqE6TranslationPlanRow {
            sequence: sequence.to_string(),
            streams: repack.pointer_relations,
            capacity_bytes: repack.capacity_bytes,
            used_bytes: repack.used_bytes,
            remaining_bytes: repack.remaining_bytes,
            pointer_relations: repack.pointer_relations,
            block_start_offset: repack.block_start_offset,
            block_end_offset: repack.block_end_offset,
            output_sha256: digest(&repack.sequence_bytes),
        });
    }
    rows.sort_by(|left, right| left.sequence.cmp(&right.sequence));
    let pointer_relations = rows.iter().map(|row| row.pointer_relations).sum();
    Ok(SubgameSeqE6TranslationPlanAnalysis {
        plan,
        rows,
        pointer_relations,
    })
}

/// 지원 원본의 합성 한 글자 전략 수치와 보고서 digest를 고정값에 대조한다.
pub fn verify_box_puyo2_subgame_seq_e6_translation_plan<'a>(
    code_table_raw: &[u8],
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    text: &SubgameSeqE6TextAnalysis,
) -> Result<SubgameSeqE6TranslationPlanAnalysis, SubgameSeqE6TranslationPlanError> {
    let analysis = audit_box_puyo2_subgame_seq_e6_translation_plan(code_table_raw, sources, text)?;
    let plan = &analysis.plan;
    let row_capacity = analysis
        .rows
        .iter()
        .map(|row| row.capacity_bytes)
        .sum::<usize>();
    let row_used = analysis
        .rows
        .iter()
        .map(|row| row.used_bytes)
        .sum::<usize>();
    let row_remaining = analysis
        .rows
        .iter()
        .map(|row| row.remaining_bytes)
        .sum::<usize>();
    let actual = (
        analysis.rows.len(),
        analysis.pointer_relations,
        plan.reachable_slots,
        plan.retained_source_characters,
        plan.glyph_mappings.len(),
        plan.remaining_slots,
        plan.capacity_bytes,
        plan.used_bytes,
        plan.remaining_bytes,
    );
    let expected = (
        BOX_PUYO2_E6_TRANSLATION_PLAN_OVERLAYS,
        BOX_PUYO2_E6_TRANSLATION_PLAN_STREAMS,
        PUYO2_E6_REACHABLE_CODE_TABLE_SLOTS,
        BOX_PUYO2_E6_TRANSLATION_PLAN_RETAINED_CHARACTERS,
        BOX_PUYO2_E6_TRANSLATION_PLAN_SYNTHETIC_GLYPHS,
        BOX_PUYO2_E6_TRANSLATION_PLAN_REMAINING_SLOTS,
        BOX_PUYO2_E6_TRANSLATION_PLAN_CAPACITY_BYTES,
        BOX_PUYO2_E6_TRANSLATION_PLAN_USED_BYTES,
        BOX_PUYO2_E6_TRANSLATION_PLAN_REMAINING_BYTES,
    );
    if actual != expected
        || (row_capacity, row_used, row_remaining)
            != (plan.capacity_bytes, plan.used_bytes, plan.remaining_bytes)
        || analysis
            .rows
            .iter()
            .any(|row| row.streams != row.pointer_relations)
    {
        return Err(error(format!(
            "PUYO2 E6 translation plan 기준선 불일치: expected={expected:?}, actual={actual:?}, \
             row_sums=({row_capacity}, {row_used}, {row_remaining})"
        )));
    }
    let report = render_subgame_seq_e6_translation_plan_tsv(&analysis);
    let actual_report_sha256 = digest(report.as_bytes());
    if actual_report_sha256 != BOX_PUYO2_E6_TRANSLATION_PLAN_REPORT_SHA256 {
        return Err(error(format!(
            "PUYO2 E6 translation plan 보고서 SHA-256 불일치: expected={}, actual={actual_report_sha256}",
            BOX_PUYO2_E6_TRANSLATION_PLAN_REPORT_SHA256
        )));
    }
    Ok(analysis)
}

/// 합성 한 글자 전략의 overlay별 용량·pointer 검산을 TSV로 보존한다.
pub fn render_subgame_seq_e6_translation_plan_tsv(
    analysis: &SubgameSeqE6TranslationPlanAnalysis,
) -> String {
    let mut output = String::from(
        "schema_version\tsequence\tstreams\tcapacity_bytes\tused_bytes\tremaining_bytes\t\
         pointer_relations\tblock_start_offset\tblock_end_offset\toutput_sha256\t\
         strategy\tclaim_boundary\n",
    );
    for row in &analysis.rows {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t0x{:04X}\t{}\t\
             existing-code-pairs-first-match-cells-and-in-place-stream-block-repack\t\
             synthetic-translation-static-write-plan-runtime-unverified",
            SUBGAME_SEQ_E6_TRANSLATION_PLAN_SCHEMA_VERSION,
            row.sequence,
            row.streams,
            row.capacity_bytes,
            row.used_bytes,
            row.remaining_bytes,
            row.pointer_relations,
            row.block_start_offset,
            row.block_end_offset,
            row.output_sha256,
        )
        .expect("String 쓰기는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "subgame_seq_e6_translation_plan_tests.rs"]
mod tests;
