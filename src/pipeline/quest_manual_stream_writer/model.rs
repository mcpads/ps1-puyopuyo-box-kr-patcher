use std::error::Error;
use std::fmt;

use crate::font::QuestCodeTable;

use super::super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::super::quest_translation_encoding::QuestTranslationEncodingPlan;

pub const QUEST_MANUAL_STREAM_WRITER_SCHEMA_VERSION: &str = "quest-manual-stream-writer-v1";

/// stream code가 참조하는 원본 code table, package와 page encoding의 결속 입력.
#[derive(Debug, Clone, Copy)]
pub struct QuestManualStreamCodeSupply<'a> {
    pub code_table: &'a QuestCodeTable,
    pub package: &'a QuestGlyphCachePackage,
    pub encoding: &'a QuestTranslationEncodingPlan,
}

/// SEQ 하나의 원본과 기술 재삽입 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestManualSequenceWrite {
    pub sequence: String,
    pub source_bytes: usize,
    pub output: Vec<u8>,
    pub source_sha256: String,
    pub output_sha256: String,
    pub selected_routes: usize,
    pub retained_source_routes: usize,
    pub inline_routes: usize,
    pub relocated_routes: usize,
    pub tail_bytes: usize,
    pub changed_bytes: usize,
}

/// 검토안 580개를 113개 lossless 문서에 적용한 기술 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestManualStreamWriterAnalysis {
    pub sequences: Vec<QuestManualSequenceWrite>,
    pub selected_routes: usize,
    pub retained_source_routes: usize,
    pub inline_routes: usize,
    pub relocated_routes: usize,
    pub return_records: usize,
    pub source_bytes: usize,
    pub output_bytes: usize,
    pub changed_bytes: usize,
    pub retained_source_exact: bool,
    pub opaque_exact: bool,
    pub internal_addresses_exact: bool,
    pub non_target_exact: bool,
    pub control_order_exact: bool,
    pub explicit_layout_routes: usize,
    pub translation_approved_routes: usize,
    pub layout_approved_routes: usize,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestManualStreamWriterError(String);

impl fmt::Display for QuestManualStreamWriterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestManualStreamWriterError {}

pub(super) fn error(message: impl Into<String>) -> QuestManualStreamWriterError {
    QuestManualStreamWriterError(message.into())
}
