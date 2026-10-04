//! QUEST의 고정 NUL slot에 수동 문구를 직렬화하는 공통 경계.
//!
//! 이 모듈은 문구를 고르거나 줄바꿈하지 않는다. 호출자가 준 수동 문구를
//! 지정 page code로 직렬화하고, 첫 NUL부터 slot 끝까지 0인지 검증한다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::font::QuestCodeTable;

use super::quest_manual_stream_writer::encode_quest_page_text;
use super::quest_translation_encoding::normalize_quest_translation_text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct QuestFixedSlotEncoding {
    pub(super) normalized_text: String,
    pub(super) glyph_cells: usize,
    pub(super) output: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct QuestFixedSlotWriterError(String);

impl fmt::Display for QuestFixedSlotWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestFixedSlotWriterError {}

fn error(message: impl Into<String>) -> QuestFixedSlotWriterError {
    QuestFixedSlotWriterError(message.into())
}

pub(super) fn encode_quest_fixed_slot_text(
    selected_text: &str,
    source_span_bytes: usize,
    page_id: u8,
    page_codes: &BTreeMap<u8, BTreeMap<char, [u8; 2]>>,
    code_table: &QuestCodeTable,
) -> Result<QuestFixedSlotEncoding, QuestFixedSlotWriterError> {
    if page_id == 0 || source_span_bytes == 0 {
        return Err(error("QUEST 고정문 page/span이 비었음"));
    }
    let normalized_text = normalize_quest_translation_text(selected_text)
        .map_err(|source| error(source.to_string()))?;
    let codes = encode_quest_page_text(&normalized_text, page_id, page_codes, code_table)
        .map_err(|source| error(source.to_string()))?;
    let glyph_cells = codes.len();
    let mut output = codes.into_iter().flatten().collect::<Vec<_>>();
    output.push(0);
    if output.len() > source_span_bytes {
        return Err(error(format!(
            "QUEST 수동 고정문이 원본 NUL slot을 넘음: output={}, capacity={source_span_bytes}",
            output.len()
        )));
    }
    output.resize(source_span_bytes, 0);
    if output.get(glyph_cells * 2) != Some(&0)
        || output[glyph_cells * 2..].iter().any(|byte| *byte != 0)
    {
        return Err(error("QUEST 고정문 NUL padding 직렬화 불일치"));
    }
    Ok(QuestFixedSlotEncoding {
        normalized_text,
        glyph_cells,
        output,
    })
}

#[cfg(test)]
#[path = "quest_fixed_slot_writer_tests.rs"]
mod tests;
