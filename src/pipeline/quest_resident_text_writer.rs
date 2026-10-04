//! `QUEST.EXE` 상주 OP21 원문의 고정 span 개발 writer.
//!
//! VDD 번역 stream과 섞지 않고 source graph에서 검증한 resident
//! unit만 받는다. 사람이 각 glyph run의 문구와 앞·뒤 pad cell을
//! 직접 정하며, writer는 원본 span을 clone한 뒤 OP21 page byte와 glyph
//! pair만 바꾼다. control·FF03·span 길이는 원래 좌표와 raw를 그대로
//! 보존한다. 이 산출물은 geometry·번역·runtime 승인을 올리지 않는
//! 비배포 development 입력이다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::analysis::{
    QuestResidentTextTokenKind, QuestResidentTextUnit, QuestResidentTextUnitAnalysis,
};
use crate::font::QuestCodeTable;
use crate::script::QuestTextTerminal;

use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_manual_stream_writer::{
    encode_quest_page_text, quest_page_code_maps, QuestManualStreamWriterError,
};
use super::quest_translation_encoding::{
    normalize_quest_translation_text, QuestTranslationEncodingPlan,
};

pub const QUEST_RESIDENT_TEXT_COMPOSITION_STATUS: &str = "manual-draft-needs-geometry";

/// 상주 문구의 사람 작성 상태. 자동 승인 상태는 제공하지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestResidentTextCompositionStatus {
    ManualDraftNeedsGeometry,
}

impl QuestResidentTextCompositionStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ManualDraftNeedsGeometry => QUEST_RESIDENT_TEXT_COMPOSITION_STATUS,
        }
    }
}

/// control 앞의 glyph run 하나에 대한 수동 문구와 고정 cell pad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextRunComposition {
    pub run_index: usize,
    pub text: String,
    pub leading_pad_cells: usize,
    pub trailing_pad_cells: usize,
}

/// source span SHA에 결속한 resident unit 하나의 수동 composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextComposition {
    pub unit_id: String,
    pub source_sha256: [u8; 32],
    pub status: QuestResidentTextCompositionStatus,
    pub runs: Vec<QuestResidentTextRunComposition>,
}

/// 기술 계획이 외부에서 파생해 넘긴 resident unit별 page ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextPageAssignment {
    pub unit_id: String,
    pub page_id: u8,
}

/// 검증된 `QUEST.EXE` text span Expected Write 재료.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextWrite {
    pub unit_id: String,
    pub record_runtime: u32,
    pub end_runtime: u32,
    pub record_text_offset: usize,
    pub end_text_offset: usize,
    pub page_id: u8,
    pub source: Vec<u8>,
    pub output: Vec<u8>,
    pub source_sha256: String,
    pub output_sha256: String,
    pub glyph_cells: usize,
    pub status: QuestResidentTextCompositionStatus,
    pub source_exact: bool,
    pub controls_exact: bool,
    pub terminal_exact: bool,
    pub japanese_glyphs_remaining: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
}

/// 입력 전체가 통과했을 때만 반환하는 resident writer 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextWriterAnalysis {
    pub writes: Vec<QuestResidentTextWrite>,
    pub units: usize,
    pub glyph_cells: usize,
    pub source_exact: bool,
    pub controls_exact: bool,
    pub terminal_exact: bool,
    pub japanese_glyphs_remaining: usize,
    pub status: QuestResidentTextCompositionStatus,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
    code_supply_sha256: String,
    writer_provenance_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextWriterError(String);

impl fmt::Display for QuestResidentTextWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentTextWriterError {}

fn error(message: impl Into<String>) -> QuestResidentTextWriterError {
    QuestResidentTextWriterError(message.into())
}

fn map_encoding_error(source: QuestManualStreamWriterError) -> QuestResidentTextWriterError {
    error(source.to_string())
}

fn sha256(raw: &[u8]) -> [u8; 32] {
    Sha256::digest(raw).into()
}

fn sha256_hex(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn update_len_prefixed(digest: &mut Sha256, raw: &[u8]) {
    digest.update(u64::try_from(raw.len()).unwrap_or(u64::MAX).to_le_bytes());
    digest.update(raw);
}

fn quest_resident_code_supply_sha256(
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"quest-resident-code-supply\0");
    update_len_prefixed(&mut digest, package.sha256.as_bytes());
    digest.update(
        u64::try_from(encoding.selected_routes)
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    digest.update(
        u64::try_from(encoding.retained_source_routes)
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    update_len_prefixed(&mut digest, encoding.repertoire.as_bytes());
    digest.update(
        u64::try_from(encoding.pages.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for page in &encoding.pages {
        digest.update([page.page_id]);
        update_len_prefixed(&mut digest, page.glyphs.as_bytes());
    }
    digest.update(
        u64::try_from(code_table.codes().len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for code in code_table.codes() {
        digest.update(code);
    }
    format!("{:x}", digest.finalize())
}

fn quest_resident_writer_provenance_sha256(writer: &QuestResidentTextWriterAnalysis) -> String {
    let mut digest = Sha256::new();
    digest.update(b"quest-resident-text-writer\0");
    update_len_prefixed(&mut digest, writer.code_supply_sha256.as_bytes());
    for value in [
        writer.units,
        writer.glyph_cells,
        writer.japanese_glyphs_remaining,
    ] {
        digest.update(u64::try_from(value).unwrap_or(u64::MAX).to_le_bytes());
    }
    digest.update([
        writer.source_exact.into(),
        writer.controls_exact.into(),
        writer.terminal_exact.into(),
        writer.translation_approved.into(),
        writer.layout_approved.into(),
        writer.runtime_verified.into(),
        writer.product_eligible.into(),
    ]);
    update_len_prefixed(&mut digest, writer.status.label().as_bytes());
    digest.update(
        u64::try_from(writer.writes.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for write in &writer.writes {
        update_len_prefixed(&mut digest, write.unit_id.as_bytes());
        digest.update(write.record_runtime.to_le_bytes());
        digest.update(write.end_runtime.to_le_bytes());
        digest.update(
            u64::try_from(write.record_text_offset)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        digest.update(
            u64::try_from(write.end_text_offset)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        digest.update([write.page_id]);
        update_len_prefixed(&mut digest, &write.source);
        update_len_prefixed(&mut digest, &write.output);
        update_len_prefixed(&mut digest, write.source_sha256.as_bytes());
        update_len_prefixed(&mut digest, write.output_sha256.as_bytes());
        digest.update(
            u64::try_from(write.glyph_cells)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        update_len_prefixed(&mut digest, write.status.label().as_bytes());
        digest.update([
            write.source_exact.into(),
            write.controls_exact.into(),
            write.terminal_exact.into(),
            (write.japanese_glyphs_remaining == 0).into(),
            write.translation_approved.into(),
            write.layout_approved.into(),
            write.runtime_verified.into(),
            write.product_eligible.into(),
        ]);
        digest.update(
            u64::try_from(write.japanese_glyphs_remaining)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
    }
    format!("{:x}", digest.finalize())
}

pub(super) fn verify_quest_resident_text_writer_provenance(
    writer: &QuestResidentTextWriterAnalysis,
) -> Result<(), QuestResidentTextWriterError> {
    if writer.code_supply_sha256.len() != 64
        || writer.writer_provenance_sha256 != quest_resident_writer_provenance_sha256(writer)
    {
        return Err(error("QUEST resident writer provenance 불일치"));
    }
    Ok(())
}

pub(super) fn verify_quest_resident_text_writer_code_supply(
    writer: &QuestResidentTextWriterAnalysis,
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> Result<(), QuestResidentTextWriterError> {
    verify_quest_resident_text_writer_provenance(writer)?;
    quest_page_code_maps(encoding, package, code_table).map_err(map_encoding_error)?;
    if writer.code_supply_sha256 != quest_resident_code_supply_sha256(encoding, package, code_table)
    {
        return Err(error("QUEST resident writer code supply 결속 불일치"));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn build_quest_resident_text_writer_fixture(
    writes: Vec<QuestResidentTextWrite>,
) -> QuestResidentTextWriterAnalysis {
    let mut writer = QuestResidentTextWriterAnalysis {
        units: writes.len(),
        glyph_cells: writes.iter().map(|write| write.glyph_cells).sum(),
        source_exact: writes.iter().all(|write| write.source_exact),
        controls_exact: writes.iter().all(|write| write.controls_exact),
        terminal_exact: writes.iter().all(|write| write.terminal_exact),
        japanese_glyphs_remaining: writes
            .iter()
            .map(|write| write.japanese_glyphs_remaining)
            .sum(),
        status: QuestResidentTextCompositionStatus::ManualDraftNeedsGeometry,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
        code_supply_sha256: sha256_hex(b"fixture-code-supply"),
        writer_provenance_sha256: String::new(),
        writes,
    };
    writer.writer_provenance_sha256 = quest_resident_writer_provenance_sha256(&writer);
    writer
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

fn relative_offset(
    unit: &QuestResidentTextUnit,
    text_offset: usize,
    label: &str,
) -> Result<usize, QuestResidentTextWriterError> {
    text_offset
        .checked_sub(unit.record_text_offset)
        .ok_or_else(|| {
            error(format!(
                "{}: {label} text offset이 record 앞임: +0x{text_offset:X}",
                unit.id
            ))
        })
}

fn validate_source_unit(
    unit: &QuestResidentTextUnit,
    code_table: &QuestCodeTable,
) -> Result<Vec<bool>, QuestResidentTextWriterError> {
    let text_span = unit
        .end_text_offset
        .checked_sub(unit.record_text_offset)
        .ok_or_else(|| error(format!("{}: text span 역전", unit.id)))?;
    let runtime_span = unit
        .end_runtime
        .checked_sub(unit.record_runtime)
        .ok_or_else(|| error(format!("{}: runtime span 역전", unit.id)))?;
    if text_span != unit.protected_raw.len()
        || usize::try_from(runtime_span).ok() != Some(text_span)
        || unit.record_text_offset.checked_add(4) != Some(unit.body_text_offset)
        || unit.record_runtime.checked_add(4) != Some(unit.body_runtime)
        || unit.record_raw != [0x21, 0, 0, 0]
        || unit.protected_raw.get(..4) != Some(unit.record_raw.as_slice())
        || sha256(&unit.protected_raw) != unit.expected_raw_sha256
        || unit.terminal != QuestTextTerminal::DeactivateAndAlign
    {
        return Err(error(format!(
            "{}: resident source span 결속 불일치",
            unit.id
        )));
    }

    let mut glyph_bytes = vec![false; unit.protected_raw.len()];
    let mut glyph_cells = 0usize;
    for (expected_index, run) in unit.runs.iter().enumerate() {
        if run.index != expected_index
            || run.glyph_occurrences.checked_mul(2) != Some(run.raw.len())
            || run.source.chars().count() != run.glyph_occurrences
        {
            return Err(error(format!(
                "{}: source run 구조 불일치: {expected_index}",
                unit.id
            )));
        }
        let offset = relative_offset(unit, run.text_offset, "run")?;
        let end = offset
            .checked_add(run.raw.len())
            .ok_or_else(|| error(format!("{}: source run 범위 overflow", unit.id)))?;
        if unit.protected_raw.get(offset..end) != Some(run.raw.as_slice()) {
            return Err(error(format!(
                "{}: source run raw 불일치: {}",
                unit.id, run.index
            )));
        }
        let claims = glyph_bytes
            .get_mut(offset..end)
            .ok_or_else(|| error(format!("{}: source run이 span 밖임", unit.id)))?;
        if claims.iter().any(|claimed| *claimed) {
            return Err(error(format!(
                "{}: source run 겹침: {}",
                unit.id, run.index
            )));
        }
        claims.fill(true);
        for pair in run.raw.chunks_exact(2) {
            let code = [pair[0], pair[1]];
            if code_table.index_of(code).is_none() {
                return Err(error(format!(
                    "{}: source run glyph가 code table에 없음: {code:02X?}",
                    unit.id
                )));
            }
        }
        glyph_cells = glyph_cells
            .checked_add(run.glyph_occurrences)
            .ok_or_else(|| error(format!("{}: source glyph 합계 overflow", unit.id)))?;
    }
    if glyph_cells != unit.glyph_occurrences || unit.japanese_glyph_occurrences == 0 {
        return Err(error(format!("{}: source glyph 분모 불일치", unit.id)));
    }

    let mut controls = 0usize;
    let mut terminal_tokens = 0usize;
    for (expected_index, token) in unit.tokens.iter().enumerate() {
        if token.index != expected_index {
            return Err(error(format!("{}: token index 불일치", unit.id)));
        }
        let offset = relative_offset(unit, token.text_offset, "token")?;
        let runtime_delta = token
            .runtime
            .checked_sub(unit.record_runtime)
            .and_then(|value| usize::try_from(value).ok());
        let end = offset
            .checked_add(token.raw.len())
            .ok_or_else(|| error(format!("{}: token 범위 overflow", unit.id)))?;
        if runtime_delta != Some(offset)
            || unit.protected_raw.get(offset..end) != Some(token.raw.as_slice())
        {
            return Err(error(format!("{}: token 좌표/raw 불일치", unit.id)));
        }
        match token.kind {
            QuestResidentTextTokenKind::Glyph => {
                if token.raw.len() != 2
                    || !glyph_bytes
                        .get(offset..end)
                        .is_some_and(|claims| claims.iter().all(|claimed| *claimed))
                {
                    return Err(error(format!(
                        "{}: glyph token이 run에 결속되지 않음",
                        unit.id
                    )));
                }
            }
            QuestResidentTextTokenKind::Control => {
                if glyph_bytes
                    .get(offset..end)
                    .is_some_and(|claims| claims.iter().any(|claimed| *claimed))
                {
                    return Err(error(format!("{}: control과 glyph run이 겹침", unit.id)));
                }
                controls += 1;
                if token.control_code == Some(0x03) {
                    terminal_tokens += 1;
                    if expected_index + 1 != unit.tokens.len()
                        || token.raw.len() < 6
                        || token.raw.get(..2) != Some([0x03, 0xFF].as_slice())
                    {
                        return Err(error(format!("{}: FF03 종단 위치/raw 불일치", unit.id)));
                    }
                }
            }
        }
    }
    if controls != unit.control_occurrences || terminal_tokens != 1 {
        return Err(error(format!("{}: control/FF03 분모 불일치", unit.id)));
    }
    Ok(glyph_bytes)
}

fn write_unit(
    unit: &QuestResidentTextUnit,
    composition: &QuestResidentTextComposition,
    page_id: u8,
    page_codes: &BTreeMap<u8, BTreeMap<char, [u8; 2]>>,
    code_table: &QuestCodeTable,
) -> Result<QuestResidentTextWrite, QuestResidentTextWriterError> {
    let glyph_bytes = validate_source_unit(unit, code_table)?;
    if composition.unit_id != unit.id
        || composition.source_sha256 != unit.expected_raw_sha256
        || composition.status != QuestResidentTextCompositionStatus::ManualDraftNeedsGeometry
        || page_id == 0
    {
        return Err(error(format!(
            "{}: composition/source/page 결속 불일치",
            unit.id
        )));
    }
    let mut compositions = BTreeMap::new();
    for run in &composition.runs {
        if compositions.insert(run.run_index, run).is_some() {
            return Err(error(format!(
                "{}: composition run index 중복: {}",
                unit.id, run.run_index
            )));
        }
    }
    if compositions.len() != unit.runs.len() {
        return Err(error(format!("{}: composition run 누락", unit.id)));
    }

    let mut output = unit.protected_raw.clone();
    output[..4].copy_from_slice(&[0x21, page_id, 0, 0]);
    let mut written_cells = 0usize;
    for source_run in &unit.runs {
        let run = compositions.remove(&source_run.index).ok_or_else(|| {
            error(format!(
                "{}: composition run 누락: {}",
                unit.id, source_run.index
            ))
        })?;
        if run.text.chars().any(is_japanese) {
            return Err(error(format!(
                "{}: composition에 일본어 잔존: run {}",
                unit.id, source_run.index
            )));
        }
        let normalized = normalize_quest_translation_text(&run.text)
            .map_err(|source| error(source.to_string()))?;
        let cells = run
            .leading_pad_cells
            .checked_add(normalized.chars().count())
            .and_then(|value| value.checked_add(run.trailing_pad_cells))
            .ok_or_else(|| error(format!("{}: composition cell 합계 overflow", unit.id)))?;
        if cells != source_run.glyph_occurrences {
            return Err(error(format!(
                "{}: composition run cell 수 불일치: run {}, expected={}, actual={cells}",
                unit.id, source_run.index, source_run.glyph_occurrences
            )));
        }
        let padded = std::iter::repeat_n('\u{3000}', run.leading_pad_cells)
            .chain(normalized.chars())
            .chain(std::iter::repeat_n('\u{3000}', run.trailing_pad_cells))
            .collect::<String>();
        let codes = encode_quest_page_text(&padded, page_id, page_codes, code_table)
            .map_err(map_encoding_error)?;
        if codes.len() != source_run.glyph_occurrences {
            return Err(error(format!("{}: encoded run cell 수 불일치", unit.id)));
        }
        let offset = relative_offset(unit, source_run.text_offset, "composition run")?;
        let replacement = codes.into_iter().flatten().collect::<Vec<_>>();
        let end = offset
            .checked_add(replacement.len())
            .ok_or_else(|| error(format!("{}: composition run 범위 overflow", unit.id)))?;
        output
            .get_mut(offset..end)
            .ok_or_else(|| error(format!("{}: composition run이 span 밖임", unit.id)))?
            .copy_from_slice(&replacement);
        written_cells = written_cells
            .checked_add(cells)
            .ok_or_else(|| error(format!("{}: written cell 합계 overflow", unit.id)))?;
    }
    if !compositions.is_empty() || written_cells != unit.glyph_occurrences {
        return Err(error(format!(
            "{}: composition 미소비 run/cell이 남음",
            unit.id
        )));
    }

    let controls_exact = unit
        .tokens
        .iter()
        .filter(|token| token.kind == QuestResidentTextTokenKind::Control)
        .all(|token| {
            relative_offset(unit, token.text_offset, "control")
                .ok()
                .and_then(|offset| output.get(offset..offset + token.raw.len()))
                == Some(token.raw.as_slice())
        });
    let terminal_exact = unit
        .tokens
        .last()
        .filter(|token| token.control_code == Some(0x03))
        .is_some_and(|token| {
            relative_offset(unit, token.text_offset, "FF03")
                .ok()
                .and_then(|offset| output.get(offset..offset + token.raw.len()))
                == Some(token.raw.as_slice())
        });
    if output.len() != unit.protected_raw.len()
        || output[0] != 0x21
        || output[1] != page_id
        || output[2..4] != [0, 0]
        || !controls_exact
        || !terminal_exact
    {
        return Err(error(format!("{}: record/control/FF03 보존 실패", unit.id)));
    }
    for (offset, (source, final_byte)) in unit.protected_raw.iter().zip(&output).enumerate() {
        if source != final_byte && offset != 1 && !glyph_bytes[offset] {
            return Err(error(format!(
                "{}: 미등록 최종 diff: span +0x{offset:X}",
                unit.id
            )));
        }
    }

    Ok(QuestResidentTextWrite {
        unit_id: unit.id.clone(),
        record_runtime: unit.record_runtime,
        end_runtime: unit.end_runtime,
        record_text_offset: unit.record_text_offset,
        end_text_offset: unit.end_text_offset,
        page_id,
        source_sha256: sha256_hex(&unit.protected_raw),
        output_sha256: sha256_hex(&output),
        source: unit.protected_raw.clone(),
        output,
        glyph_cells: written_cells,
        status: composition.status,
        source_exact: true,
        controls_exact,
        terminal_exact,
        japanese_glyphs_remaining: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
    })
}

fn write_with_page_codes(
    units: &[QuestResidentTextUnit],
    compositions: &[QuestResidentTextComposition],
    page_assignments: &[QuestResidentTextPageAssignment],
    page_codes: &BTreeMap<u8, BTreeMap<char, [u8; 2]>>,
    code_table: &QuestCodeTable,
    code_supply_sha256: &str,
) -> Result<QuestResidentTextWriterAnalysis, QuestResidentTextWriterError> {
    let unit_by_id = units
        .iter()
        .map(|unit| (unit.id.as_str(), unit))
        .collect::<BTreeMap<_, _>>();
    if unit_by_id.len() != units.len() {
        return Err(error("QUEST resident writer unit ID 중복"));
    }
    let mut composition_by_id = BTreeMap::new();
    for composition in compositions {
        if composition_by_id
            .insert(composition.unit_id.as_str(), composition)
            .is_some()
        {
            return Err(error(format!(
                "QUEST resident writer composition ID 중복: {}",
                composition.unit_id
            )));
        }
    }
    let mut page_by_id = BTreeMap::new();
    for assignment in page_assignments {
        if page_by_id
            .insert(assignment.unit_id.as_str(), assignment.page_id)
            .is_some()
        {
            return Err(error(format!(
                "QUEST resident writer page assignment ID 중복: {}",
                assignment.unit_id
            )));
        }
    }
    if composition_by_id.len() != units.len() || page_by_id.len() != units.len() {
        return Err(error(
            "QUEST resident writer unit/composition/page assignment 분모 불일치",
        ));
    }

    let mut writes = Vec::with_capacity(units.len());
    for unit in units {
        let composition = composition_by_id
            .remove(unit.id.as_str())
            .ok_or_else(|| error(format!("{}: resident composition 누락", unit.id)))?;
        let page_id = page_by_id
            .remove(unit.id.as_str())
            .ok_or_else(|| error(format!("{}: resident page assignment 누락", unit.id)))?;
        writes.push(write_unit(
            unit,
            composition,
            page_id,
            page_codes,
            code_table,
        )?);
    }
    if !composition_by_id.is_empty() || !page_by_id.is_empty() {
        return Err(error(
            "QUEST resident writer에 미등록 composition/page assignment가 남음",
        ));
    }
    let mut analysis = QuestResidentTextWriterAnalysis {
        units: writes.len(),
        glyph_cells: writes.iter().map(|write| write.glyph_cells).sum(),
        source_exact: writes.iter().all(|write| write.source_exact),
        controls_exact: writes.iter().all(|write| write.controls_exact),
        terminal_exact: writes.iter().all(|write| write.terminal_exact),
        japanese_glyphs_remaining: writes
            .iter()
            .map(|write| write.japanese_glyphs_remaining)
            .sum(),
        status: QuestResidentTextCompositionStatus::ManualDraftNeedsGeometry,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
        code_supply_sha256: code_supply_sha256.to_owned(),
        writer_provenance_sha256: String::new(),
        writes,
    };
    analysis.writer_provenance_sha256 = quest_resident_writer_provenance_sha256(&analysis);
    if !analysis.source_exact
        || !analysis.controls_exact
        || !analysis.terminal_exact
        || analysis.japanese_glyphs_remaining != 0
        || analysis.translation_approved
        || analysis.layout_approved
        || analysis.runtime_verified
        || analysis.product_eligible
        || verify_quest_resident_text_writer_provenance(&analysis).is_err()
    {
        return Err(error("QUEST resident writer 최종 gate 실패"));
    }
    Ok(analysis)
}

/// 수동 composition을 외부 page 파생과 package code map에 결속해 고정 span을 만든다.
pub fn audit_quest_resident_text_writer(
    units: &QuestResidentTextUnitAnalysis,
    compositions: &[QuestResidentTextComposition],
    page_assignments: &[QuestResidentTextPageAssignment],
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> Result<QuestResidentTextWriterAnalysis, QuestResidentTextWriterError> {
    let derived = [
        units.units.iter().map(|unit| unit.glyph_occurrences).sum(),
        units
            .units
            .iter()
            .map(|unit| unit.japanese_glyph_occurrences)
            .sum(),
        units
            .units
            .iter()
            .map(|unit| unit.control_occurrences)
            .sum(),
        units.units.iter().map(|unit| unit.runs.len()).sum(),
        units
            .units
            .iter()
            .filter(|unit| unit.runtime_evidence.is_some())
            .count(),
    ];
    let declared = [
        units.glyph_occurrences,
        units.japanese_glyph_occurrences,
        units.control_occurrences,
        units.runs,
        units.runtime_observed_units,
    ];
    if derived != declared {
        return Err(error(format!(
            "QUEST resident writer unit analysis 분모 불일치: declared={declared:?}, derived={derived:?}"
        )));
    }
    let page_codes =
        quest_page_code_maps(encoding, package, code_table).map_err(map_encoding_error)?;
    let code_supply_sha256 = quest_resident_code_supply_sha256(encoding, package, code_table);
    write_with_page_codes(
        &units.units,
        compositions,
        page_assignments,
        &page_codes,
        code_table,
        &code_supply_sha256,
    )
}

#[cfg(test)]
#[path = "quest_resident_text_writer_tests.rs"]
mod tests;
