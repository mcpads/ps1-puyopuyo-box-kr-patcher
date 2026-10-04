//! QUEST 원본 일본어 code table과 실제 OP21 사용 repertoire.
//!
//! `RPG_FNT1.BTX`의 345개 cell은 범용 Shift-JIS 폰트가 아니다.
//! `QUEST.EXE`의 고정 u16 code table이 고른 가나·선별 한자·기호
//! repertoire이며, 이 모듈은 그 공급 집합과 canonical OP21 stream의
//! 소비 집합을 분리해 검증한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::font::{
    decode_quest_source_glyph, QuestCodeTable, QuestGlyphSlot, QUEST_CODE_TABLE_COUNT,
    QUEST_CODE_TABLE_RUNTIME, QUEST_DOUBLE_EXCLAMATION_CODE, QUEST_DOUBLE_QUESTION_CODE,
    QUEST_FONT_ASSET_NAME, QUEST_FONT_SHA256,
};
use crate::script::{
    QuestSeqExtractedTokenKind, QuestSeqTextExtractionAnalysis,
    BOX_QUEST_SEQ_EXTRACTED_GLYPH_OCCURRENCES,
};

pub const QUEST_SOURCE_REPERTOIRE_SCHEMA_VERSION: &str = "quest-source-repertoire-v2";
pub const BOX_QUEST_SOURCE_REPERTOIRE_REPORT_SHA256: &str =
    "04c8c014e63ff56dd482342221aac85f05799e17d0734bd0ed8a0d4076ccb6a8";

pub const BOX_QUEST_SOURCE_CANONICAL_UNIQUE_CODES: usize = 276;
pub const BOX_QUEST_SOURCE_WINDOW_OBSERVED_CODES: usize = 319;
pub const BOX_QUEST_SOURCE_WINDOW_UNOBSERVED_CODES: usize = 26;

/// 원본 code table 한 cell의 문자 역할.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuestSourceGlyphClass {
    Digit,
    Hiragana,
    KatakanaAndLengthMark,
    SelectedKanji,
    SymbolOrSpace,
    FullwidthLatin,
    CustomAsciiPair,
}

impl QuestSourceGlyphClass {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Digit => "digit",
            Self::Hiragana => "hiragana",
            Self::KatakanaAndLengthMark => "katakana-and-length-mark",
            Self::SelectedKanji => "selected-kanji",
            Self::SymbolOrSpace => "symbol-or-space",
            Self::FullwidthLatin => "fullwidth-latin",
            Self::CustomAsciiPair => "custom-ascii-pair",
        }
    }
}

/// 역할별 cell 또는 occurrence 수.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct QuestSourceGlyphClassCounts {
    pub digits: usize,
    pub hiragana: usize,
    pub katakana_and_length_marks: usize,
    pub selected_kanji: usize,
    pub symbols_or_space: usize,
    pub fullwidth_latin: usize,
    pub custom_ascii_pairs: usize,
}

impl QuestSourceGlyphClassCounts {
    fn add(&mut self, class: QuestSourceGlyphClass, amount: usize) {
        let target = match class {
            QuestSourceGlyphClass::Digit => &mut self.digits,
            QuestSourceGlyphClass::Hiragana => &mut self.hiragana,
            QuestSourceGlyphClass::KatakanaAndLengthMark => &mut self.katakana_and_length_marks,
            QuestSourceGlyphClass::SelectedKanji => &mut self.selected_kanji,
            QuestSourceGlyphClass::SymbolOrSpace => &mut self.symbols_or_space,
            QuestSourceGlyphClass::FullwidthLatin => &mut self.fullwidth_latin,
            QuestSourceGlyphClass::CustomAsciiPair => &mut self.custom_ascii_pairs,
        };
        *target += amount;
    }
}

/// code table의 한 cell과 canonical OP21 사용량.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSourceRepertoireRow {
    pub index: usize,
    pub code: [u8; 2],
    pub display: String,
    pub unicode: String,
    pub class: QuestSourceGlyphClass,
    pub slot: QuestGlyphSlot,
    pub canonical_occurrences: usize,
    pub canonical_streams: usize,
    pub any_seq_window_observed: bool,
}

/// 원본 공급 repertoire와 정적 OP21 소비 repertoire의 분리 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSourceRepertoireAnalysis {
    pub rows: Vec<QuestSourceRepertoireRow>,
    pub source_classes: QuestSourceGlyphClassCounts,
    pub canonical_unique_classes: QuestSourceGlyphClassCounts,
    pub canonical_occurrence_classes: QuestSourceGlyphClassCounts,
    pub canonical_unique_codes: usize,
    pub canonical_glyph_occurrences: usize,
    pub sequence_window_observed_codes: usize,
    pub sequence_window_unobserved_codes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSourceRepertoireError(String);

impl fmt::Display for QuestSourceRepertoireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestSourceRepertoireError {}

fn error(message: impl Into<String>) -> QuestSourceRepertoireError {
    QuestSourceRepertoireError(message.into())
}

fn classify_code(
    code: [u8; 2],
) -> Result<(String, String, QuestSourceGlyphClass), QuestSourceRepertoireError> {
    let character = decode_quest_source_glyph(code).ok_or_else(|| {
        error(format!(
            "QUEST 원본 code {:02X} {:02X}를 단일 glyph로 decode할 수 없음",
            code[0], code[1]
        ))
    })?;
    let display = character.to_string();
    let unicode = format!("U+{:04X}", character as u32);
    if matches!(
        code,
        QUEST_DOUBLE_EXCLAMATION_CODE | QUEST_DOUBLE_QUESTION_CODE
    ) {
        return Ok((display, unicode, QuestSourceGlyphClass::CustomAsciiPair));
    }
    let class = match character {
        '\u{ff10}'..='\u{ff19}' => QuestSourceGlyphClass::Digit,
        '\u{3041}'..='\u{3096}' => QuestSourceGlyphClass::Hiragana,
        '\u{30a1}'..='\u{30fa}' | 'ー' | '〜' | '～' => {
            QuestSourceGlyphClass::KatakanaAndLengthMark
        }
        '\u{4e00}'..='\u{9fff}' => QuestSourceGlyphClass::SelectedKanji,
        '\u{ff21}'..='\u{ff3a}' | '\u{ff41}'..='\u{ff5a}' => QuestSourceGlyphClass::FullwidthLatin,
        _ => QuestSourceGlyphClass::SymbolOrSpace,
    };
    Ok((display, unicode, class))
}

/// code table 공급 집합, canonical OP21 소비, 전 SEQ raw-window 하한을 함께 센다.
pub fn audit_quest_source_repertoire(
    code_table: &QuestCodeTable,
    extraction: &QuestSeqTextExtractionAnalysis,
    observed_pairs: &BTreeSet<[u8; 2]>,
) -> Result<QuestSourceRepertoireAnalysis, QuestSourceRepertoireError> {
    if code_table.codes().len() != QUEST_CODE_TABLE_COUNT {
        return Err(error(format!(
            "QUEST 원본 repertoire code table 수 불일치: expected={QUEST_CODE_TABLE_COUNT}, \
             actual={}",
            code_table.codes().len()
        )));
    }

    let mut source_classes = QuestSourceGlyphClassCounts::default();
    let mut rows = Vec::with_capacity(code_table.codes().len());
    for (index, code) in code_table.codes().iter().copied().enumerate() {
        let (display, unicode, class) = classify_code(code)?;
        source_classes.add(class, 1);
        rows.push(QuestSourceRepertoireRow {
            index,
            code,
            display,
            unicode,
            class,
            slot: QuestGlyphSlot::for_index(index).map_err(|source| error(source.to_string()))?,
            canonical_occurrences: 0,
            canonical_streams: 0,
            any_seq_window_observed: observed_pairs.contains(&code),
        });
    }

    let mut stream_sets = vec![BTreeSet::<String>::new(); rows.len()];
    for stream in &extraction.streams {
        let stream_id = stream.id();
        for token in &stream.tokens {
            if token.kind != QuestSeqExtractedTokenKind::Glyph {
                continue;
            }
            let [first, second] = token.raw.as_slice() else {
                return Err(error(format!(
                    "{}: canonical glyph raw 길이가 2가 아님: offset=0x{:04X}, len={}",
                    stream_id,
                    token.offset,
                    token.raw.len()
                )));
            };
            let code = [*first, *second];
            let index = code_table.index_of(code).ok_or_else(|| {
                error(format!(
                    "{}: canonical glyph가 QUEST code table에 없음: \
                     offset=0x{:04X}, code={:02X} {:02X}",
                    stream_id, token.offset, code[0], code[1]
                ))
            })?;
            rows[index].canonical_occurrences += 1;
            stream_sets[index].insert(stream_id.clone());
        }
    }
    for (row, streams) in rows.iter_mut().zip(stream_sets) {
        row.canonical_streams = streams.len();
    }

    let mut canonical_unique_classes = QuestSourceGlyphClassCounts::default();
    let mut canonical_occurrence_classes = QuestSourceGlyphClassCounts::default();
    for row in &rows {
        if row.canonical_occurrences > 0 {
            canonical_unique_classes.add(row.class, 1);
            canonical_occurrence_classes.add(row.class, row.canonical_occurrences);
        }
    }
    let canonical_unique_codes = rows
        .iter()
        .filter(|row| row.canonical_occurrences > 0)
        .count();
    let canonical_glyph_occurrences = rows.iter().map(|row| row.canonical_occurrences).sum();
    let sequence_window_observed_codes = rows
        .iter()
        .filter(|row| row.any_seq_window_observed)
        .count();

    Ok(QuestSourceRepertoireAnalysis {
        rows,
        source_classes,
        canonical_unique_classes,
        canonical_occurrence_classes,
        canonical_unique_codes,
        canonical_glyph_occurrences,
        sequence_window_observed_codes,
        sequence_window_unobserved_codes: QUEST_CODE_TABLE_COUNT - sequence_window_observed_codes,
    })
}

/// 지원 원본의 고정 분모와 결정적 report를 검증한다.
pub fn verify_box_quest_source_repertoire(
    code_table: &QuestCodeTable,
    extraction: &QuestSeqTextExtractionAnalysis,
    observed_pairs: &BTreeSet<[u8; 2]>,
) -> Result<QuestSourceRepertoireAnalysis, QuestSourceRepertoireError> {
    let analysis = audit_quest_source_repertoire(code_table, extraction, observed_pairs)?;
    let expected_source = QuestSourceGlyphClassCounts {
        digits: 10,
        hiragana: 80,
        katakana_and_length_marks: 82,
        selected_kanji: 148,
        symbols_or_space: 15,
        fullwidth_latin: 8,
        custom_ascii_pairs: 2,
    };
    let expected_unique = QuestSourceGlyphClassCounts {
        digits: 5,
        hiragana: 79,
        katakana_and_length_marks: 76,
        selected_kanji: 100,
        symbols_or_space: 12,
        fullwidth_latin: 2,
        custom_ascii_pairs: 2,
    };
    let expected_occurrences = QuestSourceGlyphClassCounts {
        digits: 46,
        hiragana: 16_190,
        katakana_and_length_marks: 5_118,
        selected_kanji: 837,
        symbols_or_space: 9_532,
        fullwidth_latin: 14,
        custom_ascii_pairs: 155,
    };
    if analysis.source_classes != expected_source {
        return Err(error(format!(
            "QUEST 원본 공급 class 분모 불일치: expected={expected_source:?}, \
             actual={:?}",
            analysis.source_classes
        )));
    }
    if analysis.canonical_unique_classes != expected_unique {
        return Err(error(format!(
            "QUEST canonical 고유 class 분모 불일치: expected={expected_unique:?}, \
             actual={:?}",
            analysis.canonical_unique_classes
        )));
    }
    if analysis.canonical_occurrence_classes != expected_occurrences {
        return Err(error(format!(
            "QUEST canonical occurrence class 분모 불일치: \
             expected={expected_occurrences:?}, actual={:?}",
            analysis.canonical_occurrence_classes
        )));
    }
    let actual_totals = [
        analysis.rows.len(),
        analysis.canonical_unique_codes,
        analysis.canonical_glyph_occurrences,
        analysis.sequence_window_observed_codes,
        analysis.sequence_window_unobserved_codes,
    ];
    let expected_totals = [
        QUEST_CODE_TABLE_COUNT,
        BOX_QUEST_SOURCE_CANONICAL_UNIQUE_CODES,
        BOX_QUEST_SEQ_EXTRACTED_GLYPH_OCCURRENCES,
        BOX_QUEST_SOURCE_WINDOW_OBSERVED_CODES,
        BOX_QUEST_SOURCE_WINDOW_UNOBSERVED_CODES,
    ];
    if actual_totals != expected_totals {
        return Err(error(format!(
            "QUEST 원본 repertoire 분모 불일치: expected={expected_totals:?}, \
             actual={actual_totals:?}"
        )));
    }

    let report = render_quest_source_repertoire_tsv(&analysis);
    let actual_digest = format!("{:x}", Sha256::digest(report.as_bytes()));
    if actual_digest != BOX_QUEST_SOURCE_REPERTOIRE_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST 원본 repertoire TSV SHA-256 불일치: \
             expected={BOX_QUEST_SOURCE_REPERTOIRE_REPORT_SHA256}, actual={actual_digest}"
        )));
    }
    Ok(analysis)
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

/// code table의 345개 cell과 canonical OP21 사용량을 결정적 TSV로 만든다.
pub fn render_quest_source_repertoire_tsv(analysis: &QuestSourceRepertoireAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tasset_name\tasset_sha256\tcode_table_runtime\tindex\tcode\t\
         display\tunicode\tclass\tslot_x\tslot_y\tslot_width\tslot_height\t\
         canonical_occurrences\tcanonical_streams\tcanonical_used\t\
         any_seq_window_observed\n",
    );
    for row in &analysis.rows {
        writeln!(
            output,
            "{}\t{}\t{}\t0x{:08X}\t{}\t{:02X} {:02X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
             {}\t{}\t{}\t{}",
            QUEST_SOURCE_REPERTOIRE_SCHEMA_VERSION,
            QUEST_FONT_ASSET_NAME,
            QUEST_FONT_SHA256,
            QUEST_CODE_TABLE_RUNTIME,
            row.index,
            row.code[0],
            row.code[1],
            escape_tsv(&row.display),
            row.unicode,
            row.class.label(),
            row.slot.x,
            row.slot.y,
            row.slot.width,
            row.slot.height,
            row.canonical_occurrences,
            row.canonical_streams,
            row.canonical_occurrences > 0,
            row.any_seq_window_observed,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_source_repertoire_tests.rs"]
mod tests;
