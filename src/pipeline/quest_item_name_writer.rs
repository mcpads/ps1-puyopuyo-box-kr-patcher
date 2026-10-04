//! `ITEM_DAT.SEQ`의 수동 한국어 NUL 이름을 page code로 만드는 개발 writer.
//!
//! 이름은 사람이 작성한 source-bound 초안만 소비한다. writer는 glyph code와
//! NUL 종단을 만들고, 기존 page-0 relink가 111개 이름 pointer와 보호된 설명
//! pointer를 한 결과로 합성하게 한다. 번역·조판·runtime·제품 승인은 올리지
//! 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::font::QuestCodeTable;
use crate::script::{
    QuestItemNameDraftAnalysis, QuestItemNameDraftStatus, QuestItemTextRole,
    QuestItemTextUnitAnalysis,
};

use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_item_text_relink::{
    verify_box_quest_item_text_relink_plan_with_names, QuestItemTextRelinkPlan,
};
use super::quest_manual_stream_writer::{encode_quest_page_text, quest_page_code_maps};
use super::quest_translation_encoding::{
    normalize_quest_translation_text, QuestTranslationEncodingPlan,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemNameWrite {
    pub unit_id: String,
    pub record_offset: usize,
    pub pointer_field_offset: usize,
    pub source_text: String,
    pub draft_korean: String,
    pub source: Vec<u8>,
    pub output: Vec<u8>,
    pub source_sha256: String,
    pub output_sha256: String,
    pub glyph_cells: usize,
    pub relocated_text_offset: usize,
    pub relocated_pointer: u32,
    pub status: QuestItemNameDraftStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemNameWriterAnalysis {
    pub page_id: u8,
    pub writes: Vec<QuestItemNameWrite>,
    pub relink: QuestItemTextRelinkPlan,
    pub name_units: usize,
    pub description_units: usize,
    pub pointer_writes: usize,
    pub glyph_cells: usize,
    pub source_name_bytes: usize,
    pub output_name_bytes: usize,
    pub protected_description_bytes: usize,
    pub active_window_remaining: usize,
    pub source_sequence_sha256: String,
    pub output_sequence_sha256: String,
    pub source_exact: bool,
    pub pointer_readback_exact: bool,
    pub nul_terminal_exact: bool,
    pub descriptions_exact: bool,
    pub source_exact_outside_relinked_fields: bool,
    pub japanese_glyphs_remaining: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
    code_supply_sha256: String,
    writer_provenance_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemNameWriterError(String);

impl fmt::Display for QuestItemNameWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemNameWriterError {}

fn error(message: impl Into<String>) -> QuestItemNameWriterError {
    QuestItemNameWriterError(message.into())
}

fn sha256(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn update_len_prefixed(digest: &mut Sha256, raw: &[u8]) {
    digest.update(u64::try_from(raw.len()).unwrap_or(u64::MAX).to_le_bytes());
    digest.update(raw);
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

fn item_name_code_supply_sha256(
    page_id: u8,
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"quest-item-name-code-supply\0");
    digest.update([page_id]);
    update_len_prefixed(&mut digest, package.sha256.as_bytes());
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

fn item_name_writer_provenance_sha256(writer: &QuestItemNameWriterAnalysis) -> String {
    let mut digest = Sha256::new();
    digest.update(b"quest-item-name-writer\0");
    digest.update([writer.page_id]);
    update_len_prefixed(&mut digest, writer.code_supply_sha256.as_bytes());
    for value in [
        writer.name_units,
        writer.description_units,
        writer.pointer_writes,
        writer.glyph_cells,
        writer.source_name_bytes,
        writer.output_name_bytes,
        writer.protected_description_bytes,
        writer.active_window_remaining,
        writer.japanese_glyphs_remaining,
    ] {
        digest.update(u64::try_from(value).unwrap_or(u64::MAX).to_le_bytes());
    }
    digest.update([
        writer.source_exact.into(),
        writer.pointer_readback_exact.into(),
        writer.nul_terminal_exact.into(),
        writer.descriptions_exact.into(),
        writer.source_exact_outside_relinked_fields.into(),
        writer.translation_approved.into(),
        writer.layout_approved.into(),
        writer.runtime_verified.into(),
        writer.product_eligible.into(),
    ]);
    update_len_prefixed(&mut digest, writer.source_sequence_sha256.as_bytes());
    update_len_prefixed(&mut digest, writer.output_sequence_sha256.as_bytes());
    for value in [
        writer.relink.control.source_bytes,
        writer.relink.control.output_bytes,
        writer.relink.control.appended_bytes,
        writer.relink.control.active_window_remaining,
        writer.relink.control.pointer_writes,
        writer.relink.control.name_units,
        writer.relink.control.description_units,
        writer.relink.control.description_op21_records,
    ] {
        digest.update(u64::try_from(value).unwrap_or(u64::MAX).to_le_bytes());
    }
    digest.update([writer
        .relink
        .control
        .source_exact_outside_relinked_fields
        .into()]);
    update_len_prefixed(&mut digest, &writer.relink.control.output);
    update_len_prefixed(&mut digest, writer.relink.verified_active_store.bytes());
    for relocated in &writer.relink.control.relocated_units {
        update_len_prefixed(&mut digest, relocated.id.as_bytes());
        update_len_prefixed(&mut digest, relocated.role.label().as_bytes());
        for value in [
            relocated.pointer_field_offset,
            relocated.original_text_offset,
            relocated.relocated_record_offset.unwrap_or(usize::MAX),
            relocated.relocated_text_offset,
            relocated.source_bytes,
            relocated.protected_bytes,
        ] {
            digest.update(u64::try_from(value).unwrap_or(u64::MAX).to_le_bytes());
        }
        digest.update(relocated.relocated_pointer.to_le_bytes());
    }
    for write in &writer.writes {
        update_len_prefixed(&mut digest, write.unit_id.as_bytes());
        digest.update(
            u64::try_from(write.record_offset)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        digest.update(
            u64::try_from(write.pointer_field_offset)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        update_len_prefixed(&mut digest, write.source_text.as_bytes());
        update_len_prefixed(&mut digest, write.draft_korean.as_bytes());
        update_len_prefixed(&mut digest, &write.source);
        update_len_prefixed(&mut digest, &write.output);
        update_len_prefixed(&mut digest, write.source_sha256.as_bytes());
        update_len_prefixed(&mut digest, write.output_sha256.as_bytes());
        digest.update(
            u64::try_from(write.glyph_cells)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        digest.update(
            u64::try_from(write.relocated_text_offset)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        digest.update(write.relocated_pointer.to_le_bytes());
        update_len_prefixed(&mut digest, write.status.label().as_bytes());
    }
    format!("{:x}", digest.finalize())
}

pub(super) fn verify_quest_item_name_writer_provenance(
    writer: &QuestItemNameWriterAnalysis,
) -> Result<(), QuestItemNameWriterError> {
    if writer.code_supply_sha256.len() != 64
        || writer.writer_provenance_sha256 != item_name_writer_provenance_sha256(writer)
    {
        return Err(error("QUEST item name writer provenance 불일치"));
    }
    Ok(())
}

pub(super) fn verify_quest_item_name_writer_code_supply(
    writer: &QuestItemNameWriterAnalysis,
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> Result<(), QuestItemNameWriterError> {
    verify_quest_item_name_writer_provenance(writer)?;
    quest_page_code_maps(encoding, package, code_table)
        .map_err(|source| error(source.to_string()))?;
    if writer.code_supply_sha256
        != item_name_code_supply_sha256(writer.page_id, encoding, package, code_table)
    {
        return Err(error("QUEST item name writer code supply 결속 불일치"));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn build_quest_item_name_writer_archive_fixture(
    source: &[u8],
    output: Vec<u8>,
) -> QuestItemNameWriterAnalysis {
    use crate::isa::{verify_bytes, IsaProfile};

    let code =
        verify_bytes(IsaProfile::DECLARED, 0x8001_0000, &[0, 0, 0, 0]).expect("fixture typed code");
    let relink = QuestItemTextRelinkPlan {
        control: super::quest_item_text_relink::QuestItemTextRelinkControl {
            source_bytes: source.len(),
            output_bytes: output.len(),
            appended_bytes: output.len().saturating_sub(source.len()),
            active_window_remaining: 0,
            pointer_writes: 0,
            name_units: 0,
            description_units: 0,
            description_op21_records: 0,
            source_exact_outside_relinked_fields: true,
            relocated_units: Vec::new(),
            output,
        },
        verified_active_store: code,
    };
    let mut writer = QuestItemNameWriterAnalysis {
        page_id: 1,
        writes: Vec::new(),
        name_units: 0,
        description_units: 0,
        pointer_writes: 0,
        glyph_cells: 0,
        source_name_bytes: 0,
        output_name_bytes: 0,
        protected_description_bytes: 0,
        active_window_remaining: 0,
        source_sequence_sha256: sha256(source),
        output_sequence_sha256: sha256(&relink.control.output),
        source_exact: true,
        pointer_readback_exact: true,
        nul_terminal_exact: true,
        descriptions_exact: true,
        source_exact_outside_relinked_fields: true,
        japanese_glyphs_remaining: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
        code_supply_sha256: "0".repeat(64),
        writer_provenance_sha256: String::new(),
        relink,
    };
    writer.writer_provenance_sha256 = item_name_writer_provenance_sha256(&writer);
    writer
}

struct ItemNameWriteInput<'a> {
    map: &'a ExeMap,
    executable_text: &'a [u8],
    raw: &'a [u8],
    units: &'a QuestItemTextUnitAnalysis,
    drafts: &'a QuestItemNameDraftAnalysis,
    page_id: u8,
    page_codes: &'a BTreeMap<u8, BTreeMap<char, [u8; 2]>>,
    code_table: &'a QuestCodeTable,
    code_supply_sha256: String,
}

fn encode_item_name(
    draft_korean: &str,
    page_id: u8,
    page_codes: &BTreeMap<u8, BTreeMap<char, [u8; 2]>>,
    code_table: &QuestCodeTable,
) -> Result<(String, usize, Vec<u8>), QuestItemNameWriterError> {
    if draft_korean.is_empty() || contains_japanese(draft_korean) {
        return Err(error(
            "QUEST item name writer 이름이 비었거나 일본어가 남음",
        ));
    }
    let normalized = normalize_quest_translation_text(draft_korean)
        .map_err(|source| error(source.to_string()))?;
    let codes = encode_quest_page_text(&normalized, page_id, page_codes, code_table)
        .map_err(|source| error(source.to_string()))?;
    let glyph_cells = codes.len();
    let mut output = codes.into_iter().flatten().collect::<Vec<_>>();
    output.extend_from_slice(&[0, 0]);
    if output.len() != glyph_cells * 2 + 2 || !output.ends_with(&[0, 0]) {
        return Err(error("QUEST item name writer NUL 직렬화 불일치"));
    }
    Ok((normalized, glyph_cells, output))
}

fn write_item_names(
    input: ItemNameWriteInput<'_>,
) -> Result<QuestItemNameWriterAnalysis, QuestItemNameWriterError> {
    let ItemNameWriteInput {
        map,
        executable_text,
        raw,
        units,
        drafts,
        page_id,
        page_codes,
        code_table,
        code_supply_sha256,
    } = input;
    if page_id == 0
        || drafts.translation_approved
        || drafts.layout_approved
        || drafts.product_eligible
        || drafts.asset_sha256.len() != 64
    {
        return Err(error("QUEST item name writer 개발 입력 승인 경계 drift"));
    }
    let source_names = units
        .units
        .iter()
        .filter(|unit| unit.role == QuestItemTextRole::Name)
        .map(|unit| (unit.id.as_str(), unit))
        .collect::<BTreeMap<_, _>>();
    let descriptions = units
        .units
        .iter()
        .filter(|unit| unit.role == QuestItemTextRole::Description)
        .collect::<Vec<_>>();
    if source_names.len() != units.name_units
        || source_names.len() != units.unique_records
        || descriptions.len() != units.description_units
        || descriptions.len() != units.unique_records
        || drafts.entries.len() != source_names.len()
    {
        return Err(error("QUEST item name writer source/draft 분모 불일치"));
    }

    let mut draft_ids = BTreeSet::new();
    let mut used_glyphs = BTreeSet::new();
    let mut independent_review = 0usize;
    let mut human_choice = 0usize;
    let mut replacements = BTreeMap::new();
    let mut pending_writes = Vec::with_capacity(drafts.entries.len());
    for draft in &drafts.entries {
        if !draft_ids.insert(draft.unit_id.as_str()) {
            return Err(error(format!(
                "QUEST item name writer draft ID 중복: {}",
                draft.unit_id
            )));
        }
        let source = source_names.get(draft.unit_id.as_str()).ok_or_else(|| {
            error(format!(
                "QUEST item name writer 비대상 draft: {}",
                draft.unit_id
            ))
        })?;
        if draft.protected_source != source.protected_source
            || draft.draft_korean.is_empty()
            || draft.note.is_empty()
            || contains_japanese(&draft.draft_korean)
        {
            return Err(error(format!(
                "QUEST item name writer draft/source 불일치: {}",
                draft.unit_id
            )));
        }
        let (normalized, glyph_cells, output) =
            encode_item_name(&draft.draft_korean, page_id, page_codes, code_table)?;
        used_glyphs.extend(normalized.chars());
        match draft.status {
            QuestItemNameDraftStatus::DraftNeedsIndependentReview => independent_review += 1,
            QuestItemNameDraftStatus::DraftNeedsHumanChoice => human_choice += 1,
        }
        replacements.insert(draft.unit_id.clone(), output.clone());
        pending_writes.push((source, draft, normalized, glyph_cells, output));
    }
    let expected_used_glyphs = used_glyphs.into_iter().collect::<String>();
    let declared_used_glyphs = normalize_quest_translation_text(&drafts.used_glyphs)
        .map_err(|source| error(source.to_string()))?
        .chars()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<String>();
    if draft_ids != source_names.keys().copied().collect::<BTreeSet<_>>()
        || declared_used_glyphs != expected_used_glyphs
        || independent_review != drafts.draft_needs_independent_review
        || human_choice != drafts.draft_needs_human_choice
        || independent_review + human_choice != drafts.entries.len()
    {
        return Err(error("QUEST item name writer draft 집계 drift"));
    }

    let relink = verify_box_quest_item_text_relink_plan_with_names(
        map,
        executable_text,
        raw,
        units,
        &replacements,
    )
    .map_err(|source| error(source.to_string()))?;
    let relocated_names = relink
        .control
        .relocated_units
        .iter()
        .filter(|unit| unit.role == QuestItemTextRole::Name)
        .map(|unit| (unit.id.as_str(), unit))
        .collect::<BTreeMap<_, _>>();
    let mut writes = Vec::with_capacity(pending_writes.len());
    for (source, draft, normalized, glyph_cells, output) in pending_writes {
        let relocated = relocated_names.get(draft.unit_id.as_str()).ok_or_else(|| {
            error(format!(
                "QUEST item name writer relocated unit 누락: {}",
                draft.unit_id
            ))
        })?;
        let end = relocated
            .relocated_text_offset
            .checked_add(output.len())
            .ok_or_else(|| error("QUEST item name writer readback 범위 overflow"))?;
        if relink
            .control
            .output
            .get(relocated.relocated_text_offset..end)
            != Some(output.as_slice())
            || relocated.protected_bytes != output.len()
            || relocated.source_bytes != source.protected_raw.len()
        {
            return Err(error(format!(
                "QUEST item name writer relocated readback 불일치: {}",
                draft.unit_id
            )));
        }
        writes.push(QuestItemNameWrite {
            unit_id: draft.unit_id.clone(),
            record_offset: source.record_offset,
            pointer_field_offset: source.pointer_field_offset,
            source_text: source.protected_source.clone(),
            draft_korean: normalized,
            source: source.protected_raw.clone(),
            output_sha256: sha256(&output),
            source_sha256: sha256(&source.protected_raw),
            output,
            glyph_cells,
            relocated_text_offset: relocated.relocated_text_offset,
            relocated_pointer: relocated.relocated_pointer,
            status: draft.status,
        });
    }
    writes.sort_by_key(|write| (write.record_offset, write.unit_id.clone()));

    let protected_description_bytes = descriptions
        .iter()
        .map(|unit| unit.protected_raw.len())
        .sum::<usize>();
    let descriptions_exact = relink
        .control
        .relocated_units
        .iter()
        .filter(|unit| unit.role == QuestItemTextRole::Description)
        .all(|relocated| {
            descriptions
                .iter()
                .find(|unit| unit.id == relocated.id)
                .is_some_and(|source| {
                    relocated
                        .relocated_text_offset
                        .checked_add(source.protected_raw.len())
                        .and_then(|end| {
                            relink
                                .control
                                .output
                                .get(relocated.relocated_text_offset..end)
                        })
                        == Some(source.protected_raw.as_slice())
                })
        })
        && relink
            .control
            .relocated_units
            .iter()
            .filter(|unit| unit.role == QuestItemTextRole::Description)
            .count()
            == descriptions.len();
    let mut writer = QuestItemNameWriterAnalysis {
        page_id,
        name_units: writes.len(),
        description_units: descriptions.len(),
        pointer_writes: relink.control.pointer_writes,
        glyph_cells: writes.iter().map(|write| write.glyph_cells).sum(),
        source_name_bytes: writes.iter().map(|write| write.source.len()).sum(),
        output_name_bytes: writes.iter().map(|write| write.output.len()).sum(),
        protected_description_bytes,
        active_window_remaining: relink.control.active_window_remaining,
        source_sequence_sha256: sha256(raw),
        output_sequence_sha256: sha256(&relink.control.output),
        source_exact: true,
        pointer_readback_exact: relocated_names.len() == writes.len(),
        nul_terminal_exact: writes.iter().all(|write| {
            write.output.ends_with(&[0, 0])
                && !write.output[..write.output.len() - 2]
                    .chunks_exact(2)
                    .any(|pair| pair == [0, 0])
        }),
        descriptions_exact,
        source_exact_outside_relinked_fields: relink.control.source_exact_outside_relinked_fields,
        japanese_glyphs_remaining: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
        code_supply_sha256,
        writer_provenance_sha256: String::new(),
        writes,
        relink,
    };
    writer.writer_provenance_sha256 = item_name_writer_provenance_sha256(&writer);
    if writer.name_units != units.name_units
        || writer.description_units != units.description_units
        || writer.pointer_writes != units.units.len()
        || !writer.source_exact
        || !writer.pointer_readback_exact
        || !writer.nul_terminal_exact
        || !writer.descriptions_exact
        || !writer.source_exact_outside_relinked_fields
        || writer.japanese_glyphs_remaining != 0
        || writer.translation_approved
        || writer.layout_approved
        || writer.runtime_verified
        || writer.product_eligible
        || verify_quest_item_name_writer_provenance(&writer).is_err()
    {
        return Err(error("QUEST item name writer 최종 gate 실패"));
    }
    Ok(writer)
}

/// 수동 이름 초안 전수를 supplemental page와 page-0 relink에 결속한다.
#[allow(clippy::too_many_arguments)]
pub fn audit_quest_item_name_writer(
    map: &ExeMap,
    executable_text: &[u8],
    raw: &[u8],
    units: &QuestItemTextUnitAnalysis,
    drafts: &QuestItemNameDraftAnalysis,
    page_id: u8,
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> Result<QuestItemNameWriterAnalysis, QuestItemNameWriterError> {
    let page_codes = quest_page_code_maps(encoding, package, code_table)
        .map_err(|source| error(source.to_string()))?;
    let code_supply_sha256 = item_name_code_supply_sha256(page_id, encoding, package, code_table);
    let writer = write_item_names(ItemNameWriteInput {
        map,
        executable_text,
        raw,
        units,
        drafts,
        page_id,
        page_codes: &page_codes,
        code_table,
        code_supply_sha256,
    })?;
    verify_quest_item_name_writer_code_supply(&writer, encoding, package, code_table)?;
    Ok(writer)
}

#[cfg(test)]
#[path = "quest_item_name_writer_tests.rs"]
mod tests;
