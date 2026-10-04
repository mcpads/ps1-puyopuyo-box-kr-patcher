//! 아이템 획득 고정문을 원래 NUL span 안에 쓰는 개발 writer.
//!
//! 선택문은 수동 composition만 소비하고 자동 개행·축약·주소 재배치를 하지
//! 않는다. 각 문구를 공용 item page code로 직렬화한 뒤 남는 원문 span을
//! NUL로 채운다. 번역·geometry·runtime·제품 승인은 별도 gate로 남긴다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::analysis::QuestItemAcquisitionTextUnitAnalysis;
use crate::coords::ExeMap;
use crate::disasm::QuestItemAcquisitionMessageRole;
use crate::font::QuestCodeTable;

use super::quest_fixed_slot_writer::encode_quest_fixed_slot_text;
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_item_acquisition_text_compositions::{
    QuestItemAcquisitionTextCompositionAnalysis, QuestItemAcquisitionTextCompositionStatus,
};
use super::quest_manual_stream_writer::quest_page_code_maps;
use super::quest_translation_encoding::QuestTranslationEncodingPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemAcquisitionTextWrite {
    pub unit_id: String,
    pub role: QuestItemAcquisitionMessageRole,
    pub runtime: u32,
    pub text_offset: usize,
    pub page_id: u8,
    pub source_text: String,
    pub selected_text: String,
    pub source: Vec<u8>,
    pub output: Vec<u8>,
    pub source_sha256: String,
    pub output_sha256: String,
    pub glyph_cells: usize,
    pub status: QuestItemAcquisitionTextCompositionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemAcquisitionTextWriterAnalysis {
    pub writes: Vec<QuestItemAcquisitionTextWrite>,
    pub page_id: u8,
    pub units: usize,
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
pub struct QuestItemAcquisitionTextWriterError(String);

impl fmt::Display for QuestItemAcquisitionTextWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemAcquisitionTextWriterError {}

fn error(message: impl Into<String>) -> QuestItemAcquisitionTextWriterError {
    QuestItemAcquisitionTextWriterError(message.into())
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
    digest.update(b"quest-item-acquisition-text-code-supply\0");
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

fn writer_provenance_sha256(writer: &QuestItemAcquisitionTextWriterAnalysis) -> String {
    let mut digest = Sha256::new();
    digest.update(b"quest-item-acquisition-text-writer\0");
    digest.update([writer.page_id]);
    update_len_prefixed(&mut digest, writer.code_supply_sha256.as_bytes());
    for value in [
        writer.units,
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
        digest.update(write.runtime.to_le_bytes());
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

pub(super) fn verify_quest_item_acquisition_text_writer_code_supply(
    writer: &QuestItemAcquisitionTextWriterAnalysis,
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> Result<(), QuestItemAcquisitionTextWriterError> {
    verify_quest_item_acquisition_text_writer_provenance(writer)?;
    if writer.code_supply_sha256
        != code_supply_sha256(writer.page_id, encoding, package, code_table)
    {
        return Err(error(
            "QUEST item acquisition text writer provenance/code supply 불일치",
        ));
    }
    Ok(())
}

pub(super) fn verify_quest_item_acquisition_text_writer_provenance(
    writer: &QuestItemAcquisitionTextWriterAnalysis,
) -> Result<(), QuestItemAcquisitionTextWriterError> {
    if writer.writer_provenance_sha256 != writer_provenance_sha256(writer) {
        return Err(error(
            "QUEST item acquisition text writer provenance 불일치",
        ));
    }
    Ok(())
}

/// source-bound 수동 문구 전수를 item page code와 원래 EXE span에 결속한다.
#[allow(clippy::too_many_arguments)]
pub fn audit_quest_item_acquisition_text_writer(
    map: &ExeMap,
    executable_text: &[u8],
    units: &QuestItemAcquisitionTextUnitAnalysis,
    compositions: &QuestItemAcquisitionTextCompositionAnalysis,
    page_id: u8,
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> Result<QuestItemAcquisitionTextWriterAnalysis, QuestItemAcquisitionTextWriterError> {
    if map.name != "QUEST.EXE"
        || executable_text.len() != map.text_len as usize
        || page_id == 0
        || usize::from(page_id) > package.plan.pages
        || compositions.translation_approved
        || compositions.layout_approved
        || compositions.runtime_verified
        || compositions.product_eligible
    {
        return Err(error("QUEST item acquisition writer 개발 입력 경계 drift"));
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
            "QUEST item acquisition writer source/composition 분모 불일치",
        ));
    }

    let mut writes = Vec::with_capacity(units.units.len());
    let mut used_glyphs = BTreeSet::new();
    for unit in &units.units {
        let composition = composition_by_id.get(unit.id.as_str()).ok_or_else(|| {
            error(format!(
                "{}: QUEST item acquisition writer composition 누락",
                unit.id
            ))
        })?;
        if composition.source_sha256 != unit.source_sha256
            || composition.status
                != QuestItemAcquisitionTextCompositionStatus::ManualDraftNeedsRuntimeGeometry
        {
            return Err(error(format!(
                "{}: QUEST item acquisition writer source/status 불일치",
                unit.id
            )));
        }
        let text_offset = map.text_offset(unit.runtime).ok_or_else(|| {
            error(format!(
                "{}: QUEST item acquisition runtime 사상 실패",
                unit.id
            ))
        })?;
        let end = text_offset
            .checked_add(unit.source.len())
            .ok_or_else(|| error(format!("{}: source span overflow", unit.id)))?;
        let actual_source_sha256: [u8; 32] = Sha256::digest(&unit.source).into();
        if executable_text.get(text_offset..end) != Some(unit.source.as_slice())
            || actual_source_sha256 != unit.source_sha256
        {
            return Err(error(format!(
                "{}: QUEST item acquisition writer 원본 byte/digest 불일치",
                unit.id
            )));
        }
        let encoded = encode_quest_fixed_slot_text(
            &composition.selected_text,
            unit.source.len(),
            page_id,
            &page_codes,
            code_table,
        )
        .map_err(|source| error(source.to_string()))?;
        used_glyphs.extend(encoded.normalized_text.chars());
        writes.push(QuestItemAcquisitionTextWrite {
            unit_id: unit.id.clone(),
            role: unit.role,
            runtime: unit.runtime,
            text_offset,
            page_id,
            source_text: unit.source_text.clone(),
            selected_text: encoded.normalized_text,
            source: unit.source.clone(),
            source_sha256: sha256_hex(&unit.source),
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
        return Err(error(
            "QUEST item acquisition writer 사용 glyph 원장 불일치",
        ));
    }
    let source_bytes = writes.iter().map(|write| write.source.len()).sum();
    let output_bytes = writes.iter().map(|write| write.output.len()).sum();
    let glyph_cells = writes.iter().map(|write| write.glyph_cells).sum();
    let mut writer = QuestItemAcquisitionTextWriterAnalysis {
        writes,
        page_id,
        units: units.units.len(),
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
        || writer.source_bytes != writer.output_bytes
        || writer.writes.len() != writer.units
        || writer
            .writes
            .iter()
            .all(|write| write.source == write.output)
    {
        return Err(error("QUEST item acquisition writer 최종 결속 drift"));
    }
    writer.writer_provenance_sha256 = writer_provenance_sha256(&writer);
    Ok(writer)
}
