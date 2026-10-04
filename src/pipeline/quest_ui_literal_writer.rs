//! QUEST 지명·공용 선택지를 원래 고정 slot에 쓰는 개발 writer.
//!
//! 지명·선택지·상태·장비·도구의 수동 composition을 소비하며 자동 개행·축약·주소 재배치를 하지
//! 않는다. 각 문구를 공용 item page code로 직렬화하고 남는 slot은 NUL로
//! 채운다. 번역·geometry·runtime·제품 승인은 별도 gate로 남긴다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::analysis::QuestUiLiteralUnitAnalysis;
use crate::coords::ExeMap;
use crate::disasm::{QuestUiLiteralRole, QuestUiLiteralSurface};
use crate::font::QuestCodeTable;

use super::quest_fixed_slot_writer::encode_quest_fixed_slot_text;
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_manual_stream_writer::quest_page_code_maps;
use super::quest_translation_encoding::QuestTranslationEncodingPlan;
use super::quest_ui_literal_compositions::{
    QuestUiLiteralCompositionAnalysis, QuestUiLiteralCompositionStatus,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestUiLiteralWrite {
    pub unit_id: String,
    pub role: QuestUiLiteralRole,
    pub surface: QuestUiLiteralSurface,
    pub runtime: u32,
    pub end_runtime: u32,
    pub text_offset: usize,
    pub page_id: u8,
    pub source_text: String,
    pub selected_text: String,
    pub source: Vec<u8>,
    pub output: Vec<u8>,
    pub source_sha256: String,
    pub output_sha256: String,
    pub glyph_cells: usize,
    pub status: QuestUiLiteralCompositionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestUiLiteralWriterAnalysis {
    pub writes: Vec<QuestUiLiteralWrite>,
    pub page_id: u8,
    pub units: usize,
    pub world_map_labels: usize,
    pub binary_choices: usize,
    pub source_bytes: usize,
    pub output_bytes: usize,
    pub glyph_cells: usize,
    pub used_glyphs: String,
    pub source_exact: bool,
    pub nul_terminal_exact: bool,
    pub japanese_glyphs_remaining: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
    code_supply_sha256: String,
    writer_provenance_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestUiLiteralWriterError(String);

impl fmt::Display for QuestUiLiteralWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestUiLiteralWriterError {}

fn error(message: impl Into<String>) -> QuestUiLiteralWriterError {
    QuestUiLiteralWriterError(message.into())
}

fn sha256_hex(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn update_len_prefixed(digest: &mut Sha256, raw: &[u8]) {
    digest.update(u64::try_from(raw.len()).unwrap_or(u64::MAX).to_le_bytes());
    digest.update(raw);
}

fn code_supply_sha256(
    page_id: u8,
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"quest-ui-literal-code-supply\0");
    digest.update([page_id]);
    update_len_prefixed(&mut digest, package.sha256.as_bytes());
    update_len_prefixed(&mut digest, encoding.repertoire.as_bytes());
    for page in &encoding.pages {
        digest.update([page.page_id]);
        update_len_prefixed(&mut digest, page.glyphs.as_bytes());
    }
    for code in code_table.codes() {
        digest.update(code);
    }
    format!("{:x}", digest.finalize())
}

fn writer_provenance_sha256(writer: &QuestUiLiteralWriterAnalysis) -> String {
    let mut digest = Sha256::new();
    digest.update(b"quest-ui-literal-writer\0");
    digest.update([writer.page_id]);
    update_len_prefixed(&mut digest, writer.code_supply_sha256.as_bytes());
    for value in [
        writer.units,
        writer.world_map_labels,
        writer.binary_choices,
        writer.source_bytes,
        writer.output_bytes,
        writer.glyph_cells,
        writer.japanese_glyphs_remaining,
    ] {
        digest.update(u64::try_from(value).unwrap_or(u64::MAX).to_le_bytes());
    }
    update_len_prefixed(&mut digest, writer.used_glyphs.as_bytes());
    digest.update([
        writer.source_exact.into(),
        writer.nul_terminal_exact.into(),
        writer.translation_approved.into(),
        writer.layout_approved.into(),
        writer.runtime_verified.into(),
        writer.product_eligible.into(),
    ]);
    for write in &writer.writes {
        update_len_prefixed(&mut digest, write.unit_id.as_bytes());
        update_len_prefixed(&mut digest, write.role.label().as_bytes());
        update_len_prefixed(&mut digest, write.surface.label().as_bytes());
        digest.update(write.runtime.to_le_bytes());
        digest.update(write.end_runtime.to_le_bytes());
        digest.update(
            u64::try_from(write.text_offset)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        digest.update([write.page_id]);
        update_len_prefixed(&mut digest, write.source_text.as_bytes());
        update_len_prefixed(&mut digest, write.selected_text.as_bytes());
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
    }
    format!("{:x}", digest.finalize())
}

pub(super) fn verify_quest_ui_literal_writer_code_supply(
    writer: &QuestUiLiteralWriterAnalysis,
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> Result<(), QuestUiLiteralWriterError> {
    verify_quest_ui_literal_writer_provenance(writer)?;
    if writer.code_supply_sha256
        != code_supply_sha256(writer.page_id, encoding, package, code_table)
    {
        return Err(error(
            "QUEST UI literal writer provenance/code supply 불일치",
        ));
    }
    Ok(())
}

pub(super) fn verify_quest_ui_literal_writer_provenance(
    writer: &QuestUiLiteralWriterAnalysis,
) -> Result<(), QuestUiLiteralWriterError> {
    if writer.writer_provenance_sha256 != writer_provenance_sha256(writer) {
        return Err(error("QUEST UI literal writer provenance 불일치"));
    }
    Ok(())
}

/// source-bound UI literal 전수를 지정 item page와 원래 EXE slot에 결속한다.
#[allow(clippy::too_many_arguments)]
pub fn audit_quest_ui_literal_writer(
    map: &ExeMap,
    executable_text: &[u8],
    units: &QuestUiLiteralUnitAnalysis,
    compositions: &QuestUiLiteralCompositionAnalysis,
    page_id: u8,
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> Result<QuestUiLiteralWriterAnalysis, QuestUiLiteralWriterError> {
    if map.name != "QUEST.EXE"
        || executable_text.len() != map.text_len as usize
        || page_id == 0
        || usize::from(page_id) > package.plan.pages
        || units.world_map_labels != 12
        || units.binary_choices != 2
        || compositions.translation_approved
        || compositions.layout_approved
        || compositions.runtime_verified
        || compositions.product_eligible
    {
        return Err(error("QUEST UI literal writer 개발 입력 경계 drift"));
    }
    let page_codes = quest_page_code_maps(encoding, package, code_table)
        .map_err(|source| error(source.to_string()))?;
    let composition_by_id = compositions
        .compositions
        .iter()
        .map(|composition| (composition.unit_id.as_str(), composition))
        .collect::<BTreeMap<_, _>>();
    if composition_by_id.len() != units.units.len()
        || composition_by_id.len() != compositions.compositions.len()
    {
        return Err(error(
            "QUEST UI literal writer source/composition 분모 불일치",
        ));
    }

    let mut writes = Vec::with_capacity(units.units.len());
    let mut used_glyphs = BTreeSet::new();
    for unit in &units.units {
        let composition = composition_by_id.get(unit.id.as_str()).ok_or_else(|| {
            error(format!(
                "{}: QUEST UI literal writer composition 누락",
                unit.id
            ))
        })?;
        if composition.source_sha256 != unit.source_sha256
            || composition.status
                != QuestUiLiteralCompositionStatus::ManualDraftNeedsRuntimeGeometry
            || unit.surface != unit.role.surface()
        {
            return Err(error(format!(
                "{}: QUEST UI literal writer source/status/surface 불일치",
                unit.id
            )));
        }
        let text_offset = map
            .text_offset(unit.runtime)
            .ok_or_else(|| error(format!("{}: QUEST UI literal runtime 사상 실패", unit.id)))?;
        let end = text_offset
            .checked_add(unit.source_slot.len())
            .ok_or_else(|| error(format!("{}: source slot overflow", unit.id)))?;
        let expected_span = unit
            .end_runtime
            .checked_sub(unit.runtime)
            .and_then(|value| usize::try_from(value).ok());
        let actual_source_sha256: [u8; 32] = Sha256::digest(&unit.source_slot).into();
        if expected_span != Some(unit.source_slot.len())
            || executable_text.get(text_offset..end) != Some(unit.source_slot.as_slice())
            || actual_source_sha256 != unit.source_sha256
        {
            return Err(error(format!(
                "{}: QUEST UI literal writer 원본 byte/span/digest 불일치",
                unit.id
            )));
        }
        let encoded = encode_quest_fixed_slot_text(
            &composition.selected_text,
            unit.source_slot.len(),
            page_id,
            &page_codes,
            code_table,
        )
        .map_err(|source| error(format!("{}: {source}", unit.id)))?;
        if unit.surface == QuestUiLiteralSurface::WorldMapLabel && encoded.glyph_cells >= 16 {
            return Err(error(format!(
                "{}: 월드 지명 유한 탐색 범위에 NUL이 들어오지 않음",
                unit.id
            )));
        }
        used_glyphs.extend(encoded.normalized_text.chars());
        writes.push(QuestUiLiteralWrite {
            unit_id: unit.id.clone(),
            role: unit.role,
            surface: unit.surface,
            runtime: unit.runtime,
            end_runtime: unit.end_runtime,
            text_offset,
            page_id,
            source_text: unit.source_text.clone(),
            selected_text: encoded.normalized_text,
            source: unit.source_slot.clone(),
            source_sha256: sha256_hex(&unit.source_slot),
            output_sha256: sha256_hex(&encoded.output),
            output: encoded.output,
            glyph_cells: encoded.glyph_cells,
            status: composition.status,
        });
    }

    let used_glyphs = used_glyphs.into_iter().collect::<String>();
    let declared_used_glyphs = compositions
        .used_glyphs
        .chars()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<String>();
    if used_glyphs != declared_used_glyphs {
        return Err(error("QUEST UI literal writer 사용 glyph 원장 불일치"));
    }
    let source_bytes = writes.iter().map(|write| write.source.len()).sum();
    let output_bytes = writes.iter().map(|write| write.output.len()).sum();
    let glyph_cells = writes.iter().map(|write| write.glyph_cells).sum();
    let mut writer = QuestUiLiteralWriterAnalysis {
        writes,
        page_id,
        units: units.units.len(),
        world_map_labels: units.world_map_labels,
        binary_choices: units.binary_choices,
        source_bytes,
        output_bytes,
        glyph_cells,
        used_glyphs,
        source_exact: source_bytes == units.source_bytes,
        nul_terminal_exact: true,
        japanese_glyphs_remaining: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
        code_supply_sha256: code_supply_sha256(page_id, encoding, package, code_table),
        writer_provenance_sha256: String::new(),
    };
    if !writer.source_exact
        || writer.units != crate::analysis::BOX_QUEST_UI_LITERAL_UNITS
        || writer.source_bytes != writer.output_bytes
        || writer.writes.iter().any(|write| {
            write.output.get(write.glyph_cells * 2) != Some(&0)
                || write.output[write.glyph_cells * 2..]
                    .iter()
                    .any(|byte| *byte != 0)
        })
        || writer
            .writes
            .iter()
            .all(|write| write.source == write.output)
    {
        return Err(error("QUEST UI literal writer 최종 결속 drift"));
    }
    writer.writer_provenance_sha256 = writer_provenance_sha256(&writer);
    Ok(writer)
}
