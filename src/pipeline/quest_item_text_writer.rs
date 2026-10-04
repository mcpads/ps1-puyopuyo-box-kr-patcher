//! `ITEM_DAT.SEQ` 이름과 설명을 한 relink 결과로 합성하는 개발 writer.
//!
//! 이름은 검증된 NUL writer의 출력, 설명은 source-bound 수동 annotation의
//! literal과 control 배치를 소비한다. 설명 literal만 page code로 인코딩하고
//! control raw는 그대로 복제한 뒤, 이름·설명 pointer와 OP21 page record를
//! 하나의 결과로 먼저 합성한다. 번역·조판·runtime·제품 승인은 올리지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::font::QuestCodeTable;
use crate::script::{
    QuestItemDescriptionDraftAnalysis, QuestItemDescriptionDraftPart,
    QuestItemDescriptionDraftStatus, QuestItemNameDraftAnalysis, QuestItemTextRole,
    QuestItemTextTokenKind, QuestItemTextUnitAnalysis,
};

use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_item_name_writer::{
    audit_quest_item_name_writer, verify_quest_item_name_writer_code_supply,
    verify_quest_item_name_writer_provenance, QuestItemNameWriterAnalysis,
};
use super::quest_item_text_glyph_cache_development::QuestItemDescriptionPageAssignment;
use super::quest_item_text_relink::{
    verify_box_quest_item_text_relink_plan_with_replacements,
    QuestItemDescriptionRelinkReplacement, QuestItemTextRelinkPlan,
};
use super::quest_manual_stream_writer::{encode_quest_page_text, quest_page_code_maps};
use super::quest_translation_encoding::{
    normalize_quest_translation_text, QuestTranslationEncodingPlan,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemDescriptionWrite {
    pub unit_id: String,
    pub record_offset: usize,
    pub pointer_field_offset: usize,
    pub page_id: u8,
    pub source_text: String,
    pub draft_annotated: String,
    pub draft_korean: String,
    pub source: Vec<u8>,
    pub output: Vec<u8>,
    pub source_sha256: String,
    pub output_sha256: String,
    pub glyph_cells: usize,
    pub control_occurrences: usize,
    pub relocated_record_offset: usize,
    pub relocated_text_offset: usize,
    pub relocated_pointer: u32,
    pub status: QuestItemDescriptionDraftStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemTextWriterAnalysis {
    pub name: QuestItemNameWriterAnalysis,
    pub description_writes: Vec<QuestItemDescriptionWrite>,
    pub description_page_ids: Vec<u8>,
    pub relink: QuestItemTextRelinkPlan,
    pub name_units: usize,
    pub description_units: usize,
    pub pointer_writes: usize,
    pub description_glyph_cells: usize,
    pub description_controls: usize,
    pub source_description_bytes: usize,
    pub output_description_bytes: usize,
    pub active_window_remaining: usize,
    pub source_sequence_sha256: String,
    pub output_sequence_sha256: String,
    pub source_exact: bool,
    pub pointer_readback_exact: bool,
    pub controls_exact: bool,
    pub terminal_exact: bool,
    pub source_exact_outside_relinked_fields: bool,
    pub japanese_glyphs_remaining: usize,
    pub draft_needs_independent_review: usize,
    pub draft_needs_human_choice: usize,
    pub translation_approved: bool,
    pub layout_approved: bool,
    pub runtime_verified: bool,
    pub product_eligible: bool,
    code_supply_sha256: String,
    writer_provenance_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemTextWriterError(String);

impl fmt::Display for QuestItemTextWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemTextWriterError {}

fn error(message: impl Into<String>) -> QuestItemTextWriterError {
    QuestItemTextWriterError(message.into())
}

fn sha256(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

#[cfg(test)]
pub(super) fn build_quest_item_text_writer_archive_fixture(
    source: &[u8],
    output: Vec<u8>,
) -> QuestItemTextWriterAnalysis {
    let name =
        super::quest_item_name_writer::build_quest_item_name_writer_archive_fixture(source, output);
    let relink = name.relink.clone();
    let mut writer = QuestItemTextWriterAnalysis {
        name,
        description_writes: Vec::new(),
        description_page_ids: vec![2],
        name_units: 0,
        description_units: 0,
        pointer_writes: 0,
        description_glyph_cells: 0,
        description_controls: 0,
        source_description_bytes: 0,
        output_description_bytes: 0,
        active_window_remaining: relink.control.active_window_remaining,
        source_sequence_sha256: sha256(source),
        output_sequence_sha256: sha256(&relink.control.output),
        source_exact: true,
        pointer_readback_exact: true,
        controls_exact: true,
        terminal_exact: true,
        source_exact_outside_relinked_fields: true,
        japanese_glyphs_remaining: 0,
        draft_needs_independent_review: 0,
        draft_needs_human_choice: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
        code_supply_sha256: "0".repeat(64),
        writer_provenance_sha256: String::new(),
        relink,
    };
    writer.writer_provenance_sha256 = item_text_writer_provenance_sha256(&writer);
    writer
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

fn item_text_code_supply_sha256(
    name_page_id: u8,
    description_page_ids: &[u8],
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"quest-item-text-code-supply\0");
    digest.update([name_page_id]);
    digest.update(
        u64::try_from(description_page_ids.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    digest.update(description_page_ids);
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

fn item_text_writer_provenance_sha256(writer: &QuestItemTextWriterAnalysis) -> String {
    let mut digest = Sha256::new();
    digest.update(b"quest-item-text-writer\0");
    update_len_prefixed(&mut digest, writer.code_supply_sha256.as_bytes());
    update_len_prefixed(&mut digest, writer.name.output_sequence_sha256.as_bytes());
    digest.update([writer.name.page_id]);
    digest.update(
        u64::try_from(writer.description_page_ids.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    digest.update(&writer.description_page_ids);
    for value in [
        writer.name_units,
        writer.description_units,
        writer.pointer_writes,
        writer.description_glyph_cells,
        writer.description_controls,
        writer.source_description_bytes,
        writer.output_description_bytes,
        writer.active_window_remaining,
        writer.japanese_glyphs_remaining,
        writer.draft_needs_independent_review,
        writer.draft_needs_human_choice,
    ] {
        digest.update(u64::try_from(value).unwrap_or(u64::MAX).to_le_bytes());
    }
    digest.update([
        writer.source_exact.into(),
        writer.pointer_readback_exact.into(),
        writer.controls_exact.into(),
        writer.terminal_exact.into(),
        writer.source_exact_outside_relinked_fields.into(),
        writer.translation_approved.into(),
        writer.layout_approved.into(),
        writer.runtime_verified.into(),
        writer.product_eligible.into(),
    ]);
    update_len_prefixed(&mut digest, writer.source_sequence_sha256.as_bytes());
    update_len_prefixed(&mut digest, writer.output_sequence_sha256.as_bytes());
    update_len_prefixed(&mut digest, &writer.relink.control.output);
    update_len_prefixed(&mut digest, writer.relink.verified_active_store.bytes());
    for write in &writer.description_writes {
        update_len_prefixed(&mut digest, write.unit_id.as_bytes());
        for value in [
            write.record_offset,
            write.pointer_field_offset,
            write.glyph_cells,
            write.control_occurrences,
            write.relocated_record_offset,
            write.relocated_text_offset,
        ] {
            digest.update(u64::try_from(value).unwrap_or(u64::MAX).to_le_bytes());
        }
        digest.update([write.page_id]);
        digest.update(write.relocated_pointer.to_le_bytes());
        update_len_prefixed(&mut digest, write.source_text.as_bytes());
        update_len_prefixed(&mut digest, write.draft_annotated.as_bytes());
        update_len_prefixed(&mut digest, write.draft_korean.as_bytes());
        update_len_prefixed(&mut digest, &write.source);
        update_len_prefixed(&mut digest, &write.output);
        update_len_prefixed(&mut digest, write.source_sha256.as_bytes());
        update_len_prefixed(&mut digest, write.output_sha256.as_bytes());
        update_len_prefixed(&mut digest, write.status.label().as_bytes());
    }
    format!("{:x}", digest.finalize())
}

pub(super) fn verify_quest_item_text_writer_provenance(
    writer: &QuestItemTextWriterAnalysis,
) -> Result<(), QuestItemTextWriterError> {
    verify_quest_item_name_writer_provenance(&writer.name)
        .map_err(|source| error(source.to_string()))?;
    if writer.code_supply_sha256.len() != 64
        || writer.writer_provenance_sha256 != item_text_writer_provenance_sha256(writer)
    {
        return Err(error("QUEST item text writer provenance 불일치"));
    }
    Ok(())
}

pub(super) fn verify_quest_item_text_writer_code_supply(
    writer: &QuestItemTextWriterAnalysis,
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> Result<(), QuestItemTextWriterError> {
    verify_quest_item_text_writer_provenance(writer)?;
    verify_quest_item_name_writer_code_supply(&writer.name, encoding, package, code_table)
        .map_err(|source| error(source.to_string()))?;
    let page_codes = quest_page_code_maps(encoding, package, code_table)
        .map_err(|source| error(source.to_string()))?;
    for page_id in std::iter::once(&writer.name.page_id).chain(&writer.description_page_ids) {
        for write in &writer.name.writes {
            let tail = super::quest_item_name_particles::pronunciation_tail(
                &write.unit_id,
                &write.draft_korean,
            )
            .map_err(error)?;
            let particle =
                crate::text::select_object_particle(tail).map_err(|e| error(e.to_string()))?;
            let normalized = super::quest_translation_encoding::normalize_quest_translation_text(
                &write.draft_korean,
            )
            .map_err(|e| error(e.to_string()))?;
            let last = normalized
                .chars()
                .last()
                .ok_or_else(|| error("빈 item 이름"))?;
            let code = |glyph| {
                page_codes
                    .get(page_id)
                    .and_then(|page| page.get(&glyph))
                    .copied()
                    .map(u16::from_le_bytes)
                    .ok_or_else(|| error("QUEST item 목적격 조사 code 공급 누락"))
            };
            let pair = (code(last)?, code(particle)?);
            if write.glyph_cells >= 64
                || !package
                    .object_particle_codes
                    .get(usize::from(*page_id) - 1)
                    .is_some_and(|codes| codes.contains(&pair))
            {
                return Err(error(
                    "QUEST item 목적격 조사 mapping 또는 이름 탐색 상한 drift",
                ));
            }
            let base = page_codes
                .get(&writer.name.page_id)
                .ok_or_else(|| error("item name page 누락"))?;
            if base.get(&particle) != page_codes.get(page_id).and_then(|page| page.get(&particle)) {
                return Err(error("QUEST item 목적격 조사 code가 page 사이에서 달라짐"));
            }
        }
    }
    for page_id in &writer.description_page_ids {
        for write in &writer.name.writes {
            let encoded =
                encode_quest_page_text(&write.draft_korean, *page_id, &page_codes, code_table)
                    .map_err(|source| error(source.to_string()))?;
            if write
                .output
                .strip_suffix(&[0u8, 0])
                .unwrap_or(&write.output)
                != encoded.into_iter().flatten().collect::<Vec<_>>()
            {
                return Err(error(format!(
                    "QUEST item name 공통 code가 description page에서 달라짐: page={page_id}, unit={}",
                    write.unit_id
                )));
            }
        }
    }
    if writer.code_supply_sha256
        != item_text_code_supply_sha256(
            writer.name.page_id,
            &writer.description_page_ids,
            encoding,
            package,
            code_table,
        )
    {
        return Err(error("QUEST item text writer code supply 결속 불일치"));
    }
    Ok(())
}

fn encode_description(
    draft_korean: &str,
    parts: &[QuestItemDescriptionDraftPart],
    page_id: u8,
    page_codes: &BTreeMap<u8, BTreeMap<char, [u8; 2]>>,
    code_table: &QuestCodeTable,
) -> Result<(String, usize, usize, Vec<u8>), QuestItemTextWriterError> {
    if page_id == 0 || draft_korean.is_empty() || contains_japanese(draft_korean) {
        return Err(error(
            "QUEST item description writer 문장이 비었거나 일본어/page0가 남음",
        ));
    }
    let mut normalized = String::new();
    let mut output = Vec::new();
    let mut glyph_cells = 0usize;
    let mut controls = 0usize;
    for part in parts {
        match part {
            QuestItemDescriptionDraftPart::Literal(literal) => {
                let literal = normalize_quest_translation_text(literal)
                    .map_err(|source| error(source.to_string()))?;
                let codes = encode_quest_page_text(&literal, page_id, page_codes, code_table)
                    .map_err(|source| error(source.to_string()))?;
                glyph_cells = glyph_cells
                    .checked_add(codes.len())
                    .ok_or_else(|| error("QUEST item description glyph 수 overflow"))?;
                normalized.push_str(&literal);
                output.extend(codes.into_iter().flatten());
            }
            QuestItemDescriptionDraftPart::Control(raw) => {
                if raw.is_empty() {
                    return Err(error("QUEST item description 빈 control part"));
                }
                controls += 1;
                output.extend_from_slice(raw);
            }
        }
    }
    if normalized
        != normalize_quest_translation_text(draft_korean)
            .map_err(|source| error(source.to_string()))?
        || !output.ends_with(&[0x1A, 0xFF, 0, 0, 0, 0])
    {
        return Err(error("QUEST item description 직렬화 literal/종단 불일치"));
    }
    Ok((normalized, glyph_cells, controls, output))
}

fn source_controls(unit: &crate::script::QuestItemTextUnit) -> Vec<Vec<u8>> {
    unit.tokens
        .iter()
        .filter(|token| token.kind == QuestItemTextTokenKind::Control)
        .map(|token| token.raw.clone())
        .collect()
}

/// 이름과 설명 수동 초안을 결합 page들과 하나의 ITEM_DAT relink 결과에 결속한다.
#[allow(clippy::too_many_arguments)]
pub fn audit_quest_item_text_writer(
    map: &ExeMap,
    executable_text: &[u8],
    raw: &[u8],
    units: &QuestItemTextUnitAnalysis,
    name_drafts: &QuestItemNameDraftAnalysis,
    description_drafts: &QuestItemDescriptionDraftAnalysis,
    name_page_id: u8,
    description_page_assignments: &[QuestItemDescriptionPageAssignment],
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> Result<QuestItemTextWriterAnalysis, QuestItemTextWriterError> {
    let description_page_by_unit = description_page_assignments
        .iter()
        .map(|assignment| (assignment.unit_id.as_str(), assignment.page_id))
        .collect::<BTreeMap<_, _>>();
    let description_page_ids = description_page_assignments
        .iter()
        .map(|assignment| assignment.page_id)
        .collect::<BTreeSet<_>>();
    if name_page_id == 0
        || description_page_by_unit.len() != description_page_assignments.len()
        || description_page_ids.is_empty()
        || description_page_ids.contains(&0)
        || description_drafts.translation_approved
        || description_drafts.layout_approved
        || description_drafts.product_eligible
    {
        return Err(error(
            "QUEST item text writer 개발 입력 승인/page 경계 drift",
        ));
    }
    let page_codes = quest_page_code_maps(encoding, package, code_table)
        .map_err(|source| error(source.to_string()))?;
    if !page_codes.contains_key(&name_page_id)
        || description_page_ids
            .iter()
            .any(|page_id| !page_codes.contains_key(page_id))
    {
        return Err(error("QUEST item text writer supplemental page code 누락"));
    }
    let name = audit_quest_item_name_writer(
        map,
        executable_text,
        raw,
        units,
        name_drafts,
        name_page_id,
        encoding,
        package,
        code_table,
    )
    .map_err(|source| error(source.to_string()))?;
    let source_descriptions = units
        .units
        .iter()
        .filter(|unit| unit.role == QuestItemTextRole::Description)
        .map(|unit| (unit.id.as_str(), unit))
        .collect::<BTreeMap<_, _>>();
    if source_descriptions.len() != units.description_units
        || source_descriptions.len() != units.unique_records
        || description_drafts.entries.len() != source_descriptions.len()
    {
        return Err(error(
            "QUEST item description writer source/draft 분모 불일치",
        ));
    }

    let mut draft_ids = BTreeSet::new();
    let mut used_glyphs = BTreeSet::new();
    let mut independent_review = 0usize;
    let mut human_choice = 0usize;
    let mut description_replacements = BTreeMap::new();
    let mut pending = Vec::with_capacity(description_drafts.entries.len());
    for draft in &description_drafts.entries {
        if !draft_ids.insert(draft.unit_id.as_str()) {
            return Err(error(format!(
                "QUEST item description writer draft ID 중복: {}",
                draft.unit_id
            )));
        }
        let source = source_descriptions
            .get(draft.unit_id.as_str())
            .ok_or_else(|| {
                error(format!(
                    "QUEST item description writer 비대상 draft: {}",
                    draft.unit_id
                ))
            })?;
        if draft.protected_source != source.protected_source
            || draft.note.is_empty()
            || draft.control_raw != source_controls(source)
        {
            return Err(error(format!(
                "QUEST item description writer draft/source 불일치: {}",
                draft.unit_id
            )));
        }
        let description_page_id = description_page_by_unit
            .get(draft.unit_id.as_str())
            .copied()
            .ok_or_else(|| {
                error(format!(
                    "QUEST item description writer page assignment 누락: {}",
                    draft.unit_id
                ))
            })?;
        let (normalized, glyph_cells, controls, output) = encode_description(
            &draft.draft_korean,
            &draft.parts,
            description_page_id,
            &page_codes,
            code_table,
        )?;
        used_glyphs.extend(normalized.chars());
        match draft.status {
            QuestItemDescriptionDraftStatus::DraftNeedsIndependentReview => independent_review += 1,
            QuestItemDescriptionDraftStatus::DraftNeedsHumanChoice => human_choice += 1,
        }
        description_replacements.insert(
            draft.unit_id.clone(),
            QuestItemDescriptionRelinkReplacement {
                page_id: description_page_id,
                body: output.clone(),
            },
        );
        pending.push((
            source,
            draft,
            description_page_id,
            normalized,
            glyph_cells,
            controls,
            output,
        ));
    }
    let declared_used_glyphs = normalize_quest_translation_text(&description_drafts.used_glyphs)
        .map_err(|source| error(source.to_string()))?
        .chars()
        .collect::<BTreeSet<_>>();
    if draft_ids != source_descriptions.keys().copied().collect::<BTreeSet<_>>()
        || used_glyphs != declared_used_glyphs
        || description_page_by_unit.len() != source_descriptions.len()
        || independent_review != description_drafts.draft_needs_independent_review
        || human_choice != description_drafts.draft_needs_human_choice
        || independent_review + human_choice != description_drafts.entries.len()
    {
        return Err(error("QUEST item description writer draft 집계 drift"));
    }
    let name_replacements = name
        .writes
        .iter()
        .map(|write| (write.unit_id.clone(), write.output.clone()))
        .collect::<BTreeMap<_, _>>();
    let relink = verify_box_quest_item_text_relink_plan_with_replacements(
        map,
        executable_text,
        raw,
        units,
        &name_replacements,
        &description_replacements,
    )
    .map_err(|source| error(source.to_string()))?;
    let relocated_descriptions = relink
        .control
        .relocated_units
        .iter()
        .filter(|unit| unit.role == QuestItemTextRole::Description)
        .map(|unit| (unit.id.as_str(), unit))
        .collect::<BTreeMap<_, _>>();
    let mut description_writes = Vec::with_capacity(pending.len());
    for (source, draft, description_page_id, normalized, glyph_cells, controls, output) in pending {
        let relocated = relocated_descriptions
            .get(draft.unit_id.as_str())
            .ok_or_else(|| {
                error(format!(
                    "QUEST item description relocated unit 누락: {}",
                    draft.unit_id
                ))
            })?;
        let record = relocated
            .relocated_record_offset
            .ok_or_else(|| error("QUEST item description OP21 record 누락"))?;
        let end = relocated
            .relocated_text_offset
            .checked_add(output.len())
            .ok_or_else(|| error("QUEST item description readback 범위 overflow"))?;
        if relink.control.output.get(record..record + 4)
            != Some([0x21, description_page_id, 0, 0].as_slice())
            || relink
                .control
                .output
                .get(relocated.relocated_text_offset..end)
                != Some(output.as_slice())
            || relocated.protected_bytes != output.len()
            || relocated.source_bytes != source.protected_raw.len()
        {
            return Err(error(format!(
                "QUEST item description relocated readback 불일치: {}",
                draft.unit_id
            )));
        }
        description_writes.push(QuestItemDescriptionWrite {
            unit_id: draft.unit_id.clone(),
            record_offset: source.record_offset,
            pointer_field_offset: source.pointer_field_offset,
            page_id: description_page_id,
            source_text: source.protected_source.clone(),
            draft_annotated: draft.draft_annotated.clone(),
            draft_korean: normalized,
            source: source.protected_raw.clone(),
            source_sha256: sha256(&source.protected_raw),
            output_sha256: sha256(&output),
            output,
            glyph_cells,
            control_occurrences: controls,
            relocated_record_offset: record,
            relocated_text_offset: relocated.relocated_text_offset,
            relocated_pointer: relocated.relocated_pointer,
            status: draft.status,
        });
    }
    description_writes.sort_by_key(|write| (write.record_offset, write.unit_id.clone()));
    let description_page_ids = description_page_ids.into_iter().collect::<Vec<_>>();
    let mut writer = QuestItemTextWriterAnalysis {
        name_units: name.name_units,
        description_units: description_writes.len(),
        pointer_writes: relink.control.pointer_writes,
        description_glyph_cells: description_writes
            .iter()
            .map(|write| write.glyph_cells)
            .sum(),
        description_controls: description_writes
            .iter()
            .map(|write| write.control_occurrences)
            .sum(),
        source_description_bytes: description_writes
            .iter()
            .map(|write| write.source.len())
            .sum(),
        output_description_bytes: description_writes
            .iter()
            .map(|write| write.output.len())
            .sum(),
        active_window_remaining: relink.control.active_window_remaining,
        source_sequence_sha256: sha256(raw),
        output_sequence_sha256: sha256(&relink.control.output),
        source_exact: true,
        pointer_readback_exact: relocated_descriptions.len() == description_writes.len()
            && name_replacements.len() == name.name_units,
        controls_exact: description_writes
            .iter()
            .map(|write| write.control_occurrences)
            .sum::<usize>()
            == description_drafts.control_occurrences,
        terminal_exact: description_writes
            .iter()
            .all(|write| write.output.ends_with(&[0x1A, 0xFF, 0, 0, 0, 0])),
        source_exact_outside_relinked_fields: relink.control.source_exact_outside_relinked_fields,
        japanese_glyphs_remaining: 0,
        draft_needs_independent_review: independent_review,
        draft_needs_human_choice: human_choice,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
        code_supply_sha256: item_text_code_supply_sha256(
            name_page_id,
            &description_page_ids,
            encoding,
            package,
            code_table,
        ),
        writer_provenance_sha256: String::new(),
        name,
        description_writes,
        description_page_ids,
        relink,
    };
    writer.writer_provenance_sha256 = item_text_writer_provenance_sha256(&writer);
    if writer.name_units != units.name_units
        || writer.description_units != units.description_units
        || writer.pointer_writes != units.units.len()
        || !writer.source_exact
        || !writer.pointer_readback_exact
        || !writer.controls_exact
        || !writer.terminal_exact
        || !writer.source_exact_outside_relinked_fields
        || writer.japanese_glyphs_remaining != 0
        || writer.translation_approved
        || writer.layout_approved
        || writer.runtime_verified
        || writer.product_eligible
    {
        return Err(error("QUEST item text writer 최종 gate 실패"));
    }
    verify_quest_item_text_writer_code_supply(&writer, encoding, package, code_table)?;
    Ok(writer)
}

#[cfg(test)]
#[path = "quest_item_text_writer_tests.rs"]
mod tests;
