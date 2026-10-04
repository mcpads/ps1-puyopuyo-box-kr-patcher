//! OP21 token 원장을 사람이 검토할 stream 단위로 묶는다.
//!
//! 한 행은 canonical OP21 stream 하나이며 glyph 원문과 control raw
//! signature를 분리한다. 일본어 glyph가 하나라도 있는 stream만 번역
//! 후보이고, 영문·숫자 등 비일본어 stream은 원본 보존 참고로 남긴다.
//! 이 파생 원장은 제품 입력이 아니며 사람 승인과 runtime 도달을 대신하지
//! 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::quest_seq_text_extraction::{
    serialize_quest_seq_extracted_text_stream, QuestSeqExtractedTokenKind,
    QuestSeqTextExtractionAnalysis, BOX_QUEST_SEQ_EXTRACTED_TEXT_STREAMS,
};
use super::quest_seq_text_runtime::QUEST_SEQ_TEXT_RUNTIME_EVIDENCE;

pub const QUEST_SEQ_TRANSLATION_UNIT_SCHEMA_VERSION: &str = "quest-seq-translation-units-v2";
pub const BOX_QUEST_SEQ_TRANSLATION_UNITS: usize = 605;
pub const BOX_QUEST_SEQ_JAPANESE_TRANSLATION_CANDIDATES: usize = 580;
pub const BOX_QUEST_SEQ_NON_JAPANESE_REFERENCE_UNITS: usize = 0;
pub const BOX_QUEST_SEQ_PROTECTED_RAW_REVIEW_UNITS: usize = 0;
pub const BOX_QUEST_SEQ_CONTROL_ONLY_REFERENCE_UNITS: usize = 25;
pub const BOX_QUEST_SEQ_RUNTIME_OBSERVED_TRANSLATION_UNITS: usize = 3;
pub const BOX_QUEST_SEQ_TRANSLATION_UNIT_REPORT_SHA256: &str =
    "c94207802811dde4b629f30321ee84abe008ac5c771eebed4a1f36d6d384853c";

/// 사람 검토 시 적용할 stream 단위 정책.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestSeqTranslationUnitPolicy {
    /// 일본어 glyph가 있어 번역 후보로 검토한다.
    JapaneseTranslationCandidate,
    /// 일본어가 없으므로 영문·숫자·기호 원본 보존 참고로 남긴다.
    NonJapaneseReference,
    /// 코드표에는 있으나 엄격한 Shift-JIS 한 문자로 decode되지 않는 raw가 있다.
    ProtectedRawReview,
    /// glyph 없이 control만 있는 stream으로 원본 보존 참고다.
    ControlOnlyReference,
}

impl QuestSeqTranslationUnitPolicy {
    pub const fn label(self) -> &'static str {
        match self {
            Self::JapaneseTranslationCandidate => "japanese-translation-candidate",
            Self::NonJapaneseReference => "non-japanese-reference",
            Self::ProtectedRawReview => "protected-raw-review",
            Self::ControlOnlyReference => "control-only-reference",
        }
    }
}

/// 한 canonical OP21 stream의 사람 검토 단위.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTranslationUnit {
    pub id: String,
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub record_offset: usize,
    pub end_offset: usize,
    pub stream_sha256: [u8; 32],
    pub policy: QuestSeqTranslationUnitPolicy,
    pub protected_source: String,
    pub glyph_occurrences: usize,
    pub japanese_glyph_occurrences: usize,
    pub protected_raw_glyph_occurrences: usize,
    pub control_occurrences: usize,
    pub control_signature: String,
    pub runtime_evidence: Option<&'static str>,
}

/// 605개 stream을 빠짐없이 분류한 검토 원장.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTranslationUnitAnalysis {
    pub units: Vec<QuestSeqTranslationUnit>,
    pub japanese_translation_candidates: usize,
    pub non_japanese_reference_units: usize,
    pub protected_raw_review_units: usize,
    pub control_only_reference_units: usize,
    pub runtime_observed_units: usize,
}

/// glyph decode·stream identity·runtime 연결·결정적 분모 불일치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTranslationUnitError(String);

impl fmt::Display for QuestSeqTranslationUnitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestSeqTranslationUnitError {}

fn error(message: impl Into<String>) -> QuestSeqTranslationUnitError {
    QuestSeqTranslationUnitError(message.into())
}

fn is_japanese(character: char) -> bool {
    matches!(
        character,
        '\u{3040}'..='\u{30ff}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{ff66}'..='\u{ff9f}'
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

/// 추출 stream을 보호 원문과 control signature가 분리된 검토 단위로 묶는다.
pub fn audit_quest_seq_translation_units(
    extraction: &QuestSeqTextExtractionAnalysis,
) -> Result<QuestSeqTranslationUnitAnalysis, QuestSeqTranslationUnitError> {
    let mut runtime_by_id = BTreeMap::new();
    for evidence in QUEST_SEQ_TEXT_RUNTIME_EVIDENCE {
        let id = evidence.stream_id();
        if runtime_by_id
            .insert(
                id.clone(),
                (evidence.expected_stream_sha256, evidence.level.label()),
            )
            .is_some()
        {
            return Err(error(format!("runtime 번역 검토 stream ID 중복: {id}")));
        }
    }

    let mut ids = BTreeSet::new();
    let mut units = Vec::with_capacity(extraction.streams.len());
    for stream in &extraction.streams {
        let id = stream.id();
        if !ids.insert(id.clone()) {
            return Err(error(format!("QUEST 번역 검토 stream ID 중복: {id}")));
        }
        serialize_quest_seq_extracted_text_stream(stream)
            .map_err(|source| error(format!("{id}: 보호 stream 검증 실패: {source}")))?;

        let mut protected_source = String::new();
        let mut glyph_occurrences = 0usize;
        let mut japanese_glyph_occurrences = 0usize;
        let mut protected_raw_glyph_occurrences = 0usize;
        let mut control_occurrences = 0usize;
        let mut control_signature = Vec::new();
        for token in &stream.tokens {
            match token.kind {
                QuestSeqExtractedTokenKind::Glyph => {
                    glyph_occurrences += 1;
                    if let Some(source) = token.source.as_deref() {
                        let mut characters = source.chars();
                        let character = characters
                            .next()
                            .ok_or_else(|| error(format!("{id}: 빈 glyph source")))?;
                        if characters.next().is_some() {
                            return Err(error(format!("{id}: glyph source가 한 문자가 아님")));
                        }
                        japanese_glyph_occurrences += usize::from(is_japanese(character));
                        protected_source.push(character);
                    } else {
                        protected_raw_glyph_occurrences += 1;
                        write!(protected_source, "<RAW:{}>", hex(&token.raw))
                            .expect("String write는 실패하지 않음");
                    }
                }
                QuestSeqExtractedTokenKind::Control => {
                    let code = token.control_code.ok_or_else(|| {
                        error(format!("{id}: control code 누락: token={}", token.index))
                    })?;
                    let meaning = token.control_meaning.ok_or_else(|| {
                        error(format!("{id}: control 의미 누락: token={}", token.index))
                    })?;
                    control_occurrences += 1;
                    control_signature.push(format!(
                        "{}:FF{code:02X}:{}:{meaning:?}",
                        token.index,
                        hex(&token.raw)
                    ));
                }
            }
        }
        if glyph_occurrences + control_occurrences != stream.tokens.len() {
            return Err(error(format!("{id}: token 분모 불일치")));
        }

        let runtime_evidence =
            if let Some((expected_stream_sha256, evidence_level)) = runtime_by_id.remove(&id) {
                if expected_stream_sha256 != hex(&stream.expected_raw_sha256) {
                    return Err(error(format!("{id}: runtime stream SHA-256 불일치")));
                }
                Some(evidence_level)
            } else {
                None
            };
        let policy = if glyph_occurrences == 0 {
            QuestSeqTranslationUnitPolicy::ControlOnlyReference
        } else if protected_raw_glyph_occurrences > 0 {
            QuestSeqTranslationUnitPolicy::ProtectedRawReview
        } else if japanese_glyph_occurrences > 0 {
            QuestSeqTranslationUnitPolicy::JapaneseTranslationCandidate
        } else {
            QuestSeqTranslationUnitPolicy::NonJapaneseReference
        };
        units.push(QuestSeqTranslationUnit {
            id,
            sequence: stream.sequence.clone(),
            sequence_sha256: stream.sequence_sha256,
            record_offset: stream.record_offset,
            end_offset: stream.end_offset,
            stream_sha256: stream.expected_raw_sha256,
            policy,
            protected_source,
            glyph_occurrences,
            japanese_glyph_occurrences,
            protected_raw_glyph_occurrences,
            control_occurrences,
            control_signature: control_signature.join(","),
            runtime_evidence,
        });
    }
    units.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then(left.record_offset.cmp(&right.record_offset))
    });
    let japanese_translation_candidates = units
        .iter()
        .filter(|unit| unit.policy == QuestSeqTranslationUnitPolicy::JapaneseTranslationCandidate)
        .count();
    let protected_raw_review_units = units
        .iter()
        .filter(|unit| unit.policy == QuestSeqTranslationUnitPolicy::ProtectedRawReview)
        .count();
    let control_only_reference_units = units
        .iter()
        .filter(|unit| unit.policy == QuestSeqTranslationUnitPolicy::ControlOnlyReference)
        .count();
    let non_japanese_reference_units = units.len()
        - japanese_translation_candidates
        - protected_raw_review_units
        - control_only_reference_units;
    let runtime_observed_units = units
        .iter()
        .filter(|unit| unit.runtime_evidence.is_some())
        .count();
    Ok(QuestSeqTranslationUnitAnalysis {
        units,
        japanese_translation_candidates,
        non_japanese_reference_units,
        protected_raw_review_units,
        control_only_reference_units,
        runtime_observed_units,
    })
}

/// 고정 지원 원본의 stream 검토 분모와 report digest를 검증한다.
pub fn verify_box_quest_seq_translation_units(
    extraction: &QuestSeqTextExtractionAnalysis,
) -> Result<QuestSeqTranslationUnitAnalysis, QuestSeqTranslationUnitError> {
    let analysis = audit_quest_seq_translation_units(extraction)?;
    let actual = [
        analysis.units.len(),
        analysis.japanese_translation_candidates,
        analysis.non_japanese_reference_units,
        analysis.protected_raw_review_units,
        analysis.control_only_reference_units,
        analysis.runtime_observed_units,
    ];
    let expected = [
        BOX_QUEST_SEQ_TRANSLATION_UNITS,
        BOX_QUEST_SEQ_JAPANESE_TRANSLATION_CANDIDATES,
        BOX_QUEST_SEQ_NON_JAPANESE_REFERENCE_UNITS,
        BOX_QUEST_SEQ_PROTECTED_RAW_REVIEW_UNITS,
        BOX_QUEST_SEQ_CONTROL_ONLY_REFERENCE_UNITS,
        BOX_QUEST_SEQ_RUNTIME_OBSERVED_TRANSLATION_UNITS,
    ];
    if actual != expected {
        return Err(error(format!(
            "QUEST 번역 검토 단위 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }
    if analysis.units.len() != BOX_QUEST_SEQ_EXTRACTED_TEXT_STREAMS {
        return Err(error("QUEST 추출 stream과 번역 검토 단위 분모 불일치"));
    }

    let report = render_quest_seq_translation_units_tsv(&analysis);
    let actual_digest = format!("{:x}", Sha256::digest(report.as_bytes()));
    if actual_digest != BOX_QUEST_SEQ_TRANSLATION_UNIT_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST 번역 검토 단위 TSV SHA-256 불일치: \
             expected={BOX_QUEST_SEQ_TRANSLATION_UNIT_REPORT_SHA256}, actual={actual_digest}"
        )));
    }
    Ok(analysis)
}

/// stream 원문과 control raw를 분리한 결정적 사람 검토 TSV.
pub fn render_quest_seq_translation_units_tsv(
    analysis: &QuestSeqTranslationUnitAnalysis,
) -> String {
    let mut output = String::from(
        "schema_version\tarchive_path\tsequence_name\tsequence_sha256\tstream_id\t\
         record_offset\tend_offset\tstream_sha256\treview_policy\t\
         glyph_occurrences\tjapanese_glyph_occurrences\tprotected_raw_glyph_occurrences\t\
         control_occurrences\tprotected_source\tcontrol_signature\truntime_evidence\n",
    );
    for unit in &analysis.units {
        writeln!(
            output,
            "{}\t/QUEST/RPG_SEQ.VDD;1\t{}\t{}\t{}\t0x{:04X}\t0x{:04X}\t{}\t\
             {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            QUEST_SEQ_TRANSLATION_UNIT_SCHEMA_VERSION,
            unit.sequence,
            hex(&unit.sequence_sha256),
            unit.id,
            unit.record_offset,
            unit.end_offset,
            hex(&unit.stream_sha256),
            unit.policy.label(),
            unit.glyph_occurrences,
            unit.japanese_glyph_occurrences,
            unit.protected_raw_glyph_occurrences,
            unit.control_occurrences,
            escape_tsv(&unit.protected_source),
            unit.control_signature,
            unit.runtime_evidence.unwrap_or("static-only"),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_seq_translation_units_tests.rs"]
mod tests;
