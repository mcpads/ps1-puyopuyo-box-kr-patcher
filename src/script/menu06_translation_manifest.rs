//! `MENU06` 보호 원문 대조형 한국어 번역 manifest.
//!
//! 29개 `FF02` review 구간을 버전 관리 입력과 전수 대조하고, 원본 control과
//! token 위치를 보존하는 fixed-slot stream을 만든다. 한국어가 짧은 구간은
//! 별도 16×16 blank cell로 채우며 일반 8px space와 섞지 않는다. 모든 행이
//! 사람 승인되기 전에는 제품용 plan을 반환하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::{serialize_menu06_control_preserving_rewrite, Menu06TextAnalysis, Menu06TextToken};

pub const MENU06_TRANSLATION_MANIFEST_SCHEMA_VERSION: &str = "menu06-translation-manifest-v1";

const MANIFEST: &str = include_str!("../../assets/translations/menu06.tsv");
const HEADER: &str = "schema_version\tstatus\tsegment_id\tstart_offset\tend_offset\tprotected_source\tko\tpadding_cells\tnotes";
const REPERTOIRE_SLOTS: usize = 98;
const NARROW_SPACE_CELL: u16 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu06TranslationStatus {
    DraftNeedsHumanReview,
    Approved,
}

impl Menu06TranslationStatus {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "draft-needs-human-review" => Some(Self::DraftNeedsHumanReview),
            "approved" => Some(Self::Approved),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::DraftNeedsHumanReview => "draft-needs-human-review",
            Self::Approved => "approved",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06TranslationManifestEntry {
    pub status: Menu06TranslationStatus,
    pub segment_id: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub protected_source: String,
    pub ko: String,
    pub padding_cells: usize,
    pub notes: String,
}

/// 한 cell에 그릴 문자. `glyph=None`은 16×16 full-width blank다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Menu06GlyphMapping {
    pub cell: u16,
    pub glyph: Option<char>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06TranslationPlan {
    pub replacement_tokens: Vec<Menu06TextToken>,
    pub rewritten_stream: Vec<u8>,
    pub rewritten_stream_sha256: String,
    pub glyph_mappings: Vec<Menu06GlyphMapping>,
    pub padding_cell: u16,
    pub used_slots: usize,
    pub remaining_slots: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06TranslationManifestAnalysis {
    pub entries: Vec<Menu06TranslationManifestEntry>,
    pub plan: Menu06TranslationPlan,
    pub protected_source_code_units: usize,
    pub translated_code_units: usize,
    pub padding_cells: usize,
    pub draft_needs_human_review: usize,
    pub approved: usize,
    pub product_eligible: bool,
    pub unique_translation_glyphs: usize,
    pub manifest_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06TranslationManifestError(String);

impl fmt::Display for Menu06TranslationManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Menu06TranslationManifestError {}

fn error(message: impl Into<String>) -> Menu06TranslationManifestError {
    Menu06TranslationManifestError(message.into())
}

fn digest(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn parse_offset(
    raw: &str,
    field: &str,
    line: usize,
) -> Result<usize, Menu06TranslationManifestError> {
    raw.strip_prefix("0x")
        .and_then(|digits| usize::from_str_radix(digits, 16).ok())
        .ok_or_else(|| error(format!("MENU06 manifest {field} 오류: line={line}")))
}

fn parse_usize(
    raw: &str,
    field: &str,
    line: usize,
) -> Result<usize, Menu06TranslationManifestError> {
    raw.parse::<usize>()
        .map_err(|_| error(format!("MENU06 manifest {field} 숫자 아님: line={line}")))
}

fn contains_japanese(raw: &str) -> bool {
    raw.chars().any(|glyph| {
        matches!(
            glyph,
            '\u{3040}'..='\u{30ff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{4e00}'..='\u{9fff}'
                | '\u{ff65}'..='\u{ff9f}'
        )
    })
}

fn contains_hangul(raw: &str) -> bool {
    raw.chars()
        .any(|glyph| matches!(glyph, '\u{ac00}'..='\u{d7a3}'))
}

fn parse_manifest(
    raw: &str,
) -> Result<Vec<Menu06TranslationManifestEntry>, Menu06TranslationManifestError> {
    let mut lines = raw.lines();
    if lines.next().unwrap_or_default() != HEADER {
        return Err(error("MENU06 translation manifest header 불일치"));
    }
    let mut entries = Vec::new();
    let mut ids = BTreeSet::new();
    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        if line.is_empty() {
            return Err(error(format!(
                "MENU06 translation manifest 빈 행: line={line_number}"
            )));
        }
        let columns = line.split('\t').collect::<Vec<_>>();
        let [schema, status, segment_id, start_offset, end_offset, protected_source, ko, padding_cells, notes] =
            columns.as_slice()
        else {
            return Err(error(format!(
                "MENU06 translation manifest 열 수 불일치: line={line_number}"
            )));
        };
        if *schema != MENU06_TRANSLATION_MANIFEST_SCHEMA_VERSION {
            return Err(error(format!(
                "MENU06 translation manifest schema 불일치: line={line_number}"
            )));
        }
        let status = Menu06TranslationStatus::parse(status).ok_or_else(|| {
            error(format!(
                "MENU06 translation manifest status 미지원: line={line_number}, value={status}"
            ))
        })?;
        if segment_id.is_empty()
            || protected_source.is_empty()
            || ko.is_empty()
            || notes.is_empty()
            || contains_japanese(ko)
            || !contains_hangul(ko)
        {
            return Err(error(format!(
                "MENU06 translation manifest 필수값/한국어 오류: line={line_number}"
            )));
        }
        if !ids.insert(*segment_id) {
            return Err(error(format!(
                "MENU06 translation manifest segment 중복: {segment_id}"
            )));
        }
        entries.push(Menu06TranslationManifestEntry {
            status,
            segment_id: (*segment_id).to_string(),
            start_offset: parse_offset(start_offset, "start_offset", line_number)?,
            end_offset: parse_offset(end_offset, "end_offset", line_number)?,
            protected_source: (*protected_source).to_string(),
            ko: (*ko).to_string(),
            padding_cells: parse_usize(padding_cells, "padding_cells", line_number)?,
            notes: (*notes).to_string(),
        });
    }
    if entries.is_empty() {
        return Err(error("MENU06 translation manifest가 비어 있음"));
    }
    Ok(entries)
}

fn build_plan(
    entries: &[Menu06TranslationManifestEntry],
    text: &Menu06TextAnalysis,
) -> Result<Menu06TranslationPlan, Menu06TranslationManifestError> {
    let glyphs = entries
        .iter()
        .flat_map(|entry| entry.ko.chars())
        .collect::<BTreeSet<_>>();
    if !glyphs.contains(&' ') {
        return Err(error("MENU06 한국어 manifest에 8px space가 없음"));
    }
    let used_slots = glyphs
        .len()
        .checked_add(1)
        .ok_or_else(|| error("MENU06 glyph slot 합계 overflow"))?;
    if used_slots > REPERTOIRE_SLOTS {
        return Err(error(format!(
            "MENU06 glyph repertoire 초과: visible={}, padding=1, capacity={REPERTOIRE_SLOTS}",
            glyphs.len()
        )));
    }
    let mut by_glyph = BTreeMap::from([(' ', NARROW_SPACE_CELL)]);
    let mut mappings = vec![Menu06GlyphMapping {
        cell: NARROW_SPACE_CELL,
        glyph: Some(' '),
    }];
    let mut next_cell = 1u16;
    for glyph in glyphs.iter().copied().filter(|glyph| *glyph != ' ') {
        by_glyph.insert(glyph, next_cell);
        mappings.push(Menu06GlyphMapping {
            cell: next_cell,
            glyph: Some(glyph),
        });
        next_cell = next_cell
            .checked_add(1)
            .ok_or_else(|| error("MENU06 glyph cell overflow"))?;
    }
    let padding_cell = next_cell;
    mappings.push(Menu06GlyphMapping {
        cell: padding_cell,
        glyph: None,
    });

    let replacement_cells = entries
        .iter()
        .flat_map(|entry| {
            entry
                .ko
                .chars()
                .map(|glyph| by_glyph[&glyph])
                .chain(std::iter::repeat_n(padding_cell, entry.padding_cells))
        })
        .collect::<Vec<_>>();
    if replacement_cells.len() != text.glyph_tokens {
        return Err(error(format!(
            "MENU06 replacement glyph 분모 불일치: expected={}, actual={}",
            text.glyph_tokens,
            replacement_cells.len()
        )));
    }
    let mut replacement_tokens = text.tokens.clone();
    let mut cells = replacement_cells.into_iter();
    for token in &mut replacement_tokens {
        if let Menu06TextToken::Glyph { cell, .. } = token {
            *cell = cells
                .next()
                .ok_or_else(|| error("MENU06 replacement glyph 조기 소진"))?;
        }
    }
    if cells.next().is_some() {
        return Err(error("MENU06 replacement glyph 잔여 발생"));
    }
    let rewritten_stream =
        serialize_menu06_control_preserving_rewrite(&text.tokens, &replacement_tokens)
            .map_err(|source| error(source.to_string()))?;
    Ok(Menu06TranslationPlan {
        rewritten_stream_sha256: digest(&rewritten_stream),
        replacement_tokens,
        rewritten_stream,
        glyph_mappings: mappings,
        padding_cell,
        used_slots,
        remaining_slots: REPERTOIRE_SLOTS - used_slots,
    })
}

fn verify_manifest(
    raw: &str,
    text: &Menu06TextAnalysis,
) -> Result<Menu06TranslationManifestAnalysis, Menu06TranslationManifestError> {
    let manifest_sha256 = digest(raw.as_bytes());
    let entries = parse_manifest(raw)?;
    if entries.len() != text.segments.len() {
        return Err(error(format!(
            "MENU06 translation manifest segment 분모 불일치: expected={}, actual={}",
            text.segments.len(),
            entries.len()
        )));
    }
    for (entry, segment) in entries.iter().zip(&text.segments) {
        let expected_id = format!("MENU06-TXT-{:04}", segment.index + 1);
        if entry.segment_id != expected_id
            || entry.start_offset != segment.start_offset
            || entry.end_offset != segment.end_offset
            || entry.protected_source != segment.source
        {
            return Err(error(format!(
                "MENU06 translation manifest 보호 원문/identity drift: {}",
                entry.segment_id
            )));
        }
        let occupied = entry
            .ko
            .chars()
            .count()
            .checked_add(entry.padding_cells)
            .ok_or_else(|| error("MENU06 segment 길이 overflow"))?;
        if occupied != segment.glyph_cells.len() {
            return Err(error(format!(
                "MENU06 fixed-slot 길이 불일치: {}, expected={}, actual={occupied}",
                entry.segment_id,
                segment.glyph_cells.len()
            )));
        }
    }
    let plan = build_plan(&entries, text)?;
    let draft_needs_human_review = entries
        .iter()
        .filter(|entry| entry.status == Menu06TranslationStatus::DraftNeedsHumanReview)
        .count();
    let approved = entries.len() - draft_needs_human_review;
    let analysis = Menu06TranslationManifestAnalysis {
        protected_source_code_units: entries
            .iter()
            .map(|entry| entry.protected_source.chars().count())
            .sum(),
        translated_code_units: entries.iter().map(|entry| entry.ko.chars().count()).sum(),
        padding_cells: entries.iter().map(|entry| entry.padding_cells).sum(),
        product_eligible: approved == entries.len(),
        unique_translation_glyphs: plan
            .glyph_mappings
            .iter()
            .filter(|mapping| mapping.glyph.is_some())
            .count(),
        entries,
        plan,
        draft_needs_human_review,
        approved,
        manifest_sha256,
    };
    Ok(analysis)
}

/// 지원 원본 분석과 version 관리 MENU06 manifest를 전수 대조한다.
pub fn verify_box_menu06_translation_manifest(
    text: &Menu06TextAnalysis,
) -> Result<Menu06TranslationManifestAnalysis, Menu06TranslationManifestError> {
    let analysis = verify_manifest(MANIFEST, text)?;
    if render_box_menu06_translation_manifest_tsv(&analysis) != MANIFEST {
        return Err(error(
            "MENU06 translation manifest가 canonical 순서/표현이 아님",
        ));
    }
    Ok(analysis)
}

/// 정적 glyph·fixed-slot 검토에만 쓰는 미승인 plan을 반환한다.
pub fn review_box_menu06_translation_plan(
    text: &Menu06TextAnalysis,
) -> Result<Menu06TranslationPlan, Menu06TranslationManifestError> {
    Ok(verify_box_menu06_translation_manifest(text)?.plan)
}

/// 29개 구간이 모두 사람 승인된 경우에만 제품용 plan을 반환한다.
pub fn approved_box_menu06_translation_plan(
    text: &Menu06TextAnalysis,
) -> Result<Menu06TranslationPlan, Menu06TranslationManifestError> {
    let analysis = verify_box_menu06_translation_manifest(text)?;
    if !analysis.product_eligible {
        return Err(error(format!(
            "MENU06 translation manifest 사람 승인 미완료: approved={}, total={}",
            analysis.approved,
            analysis.entries.len()
        )));
    }
    Ok(analysis.plan)
}

pub fn render_box_menu06_translation_manifest_tsv(
    analysis: &Menu06TranslationManifestAnalysis,
) -> String {
    let mut output = format!("{HEADER}\n");
    for entry in &analysis.entries {
        writeln!(
            output,
            "{}\t{}\t{}\t0x{:04X}\t0x{:04X}\t{}\t{}\t{}\t{}",
            MENU06_TRANSLATION_MANIFEST_SCHEMA_VERSION,
            entry.status.label(),
            entry.segment_id,
            entry.start_offset,
            entry.end_offset,
            entry.protected_source,
            entry.ko,
            entry.padding_cells,
            entry.notes,
        )
        .expect("String 쓰기는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "menu06_translation_manifest_tests.rs"]
mod tests;
