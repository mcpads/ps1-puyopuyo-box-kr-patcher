//! QUEST 자연 조우의 전투 이름 고정 slot 개발 writer.
//!
//! consumer에서 증명한 59개 물리 slot과 source-bound 수동 composition만
//! 받는다. 각 개발 이름에 수동으로 선언한 주격 조사 역할을 결속하고 조우
//! 공통 page code로 직렬화한 뒤, 첫 u16 NUL부터 slot 끝까지 0으로 채운다.
//! 자동 음역·축약·개행은 하지 않으며 번역·geometry·runtime·제품 승인을
//! 올리지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::analysis::QuestBattleNameTableAnalysis;
use crate::coords::ExeMap;
use crate::font::QuestCodeTable;
use crate::text::select_subject_particle;

use super::quest_battle_name_compositions::{
    QuestBattleNameCompositionAnalysis, QuestBattleNameDecisionStatus,
};
use super::quest_fixed_slot_writer::encode_quest_fixed_slot_text;
use super::quest_glyph_cache_package::QuestGlyphCachePackage;
use super::quest_manual_stream_writer::quest_page_code_maps;
use super::quest_translation_encoding::QuestTranslationEncodingPlan;

/// 원래 물리 slot에 적용할 검증된 전투 이름 Expected Write 재료.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestBattleNameWrite {
    pub unit_id: String,
    pub index: usize,
    pub runtime: u32,
    pub end_runtime: u32,
    pub text_offset: usize,
    pub page_id: u8,
    pub source_text: String,
    pub development_text: String,
    pub source: Vec<u8>,
    pub output: Vec<u8>,
    pub source_sha256: String,
    pub output_sha256: String,
    pub glyph_cells: usize,
    pub slot_capacity_glyphs: usize,
    pub decision_status: QuestBattleNameDecisionStatus,
}

/// 59개 전투 이름 slot이 모두 통과했을 때만 반환하는 writer 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestBattleNameWriterAnalysis {
    pub writes: Vec<QuestBattleNameWrite>,
    pub page_id: u8,
    pub physical_names: usize,
    pub unique_sources: usize,
    pub source_bytes: usize,
    pub output_bytes: usize,
    pub glyph_cells: usize,
    pub used_glyphs: String,
    pub selected_drafts: usize,
    pub needs_human_choice: usize,
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
pub struct QuestBattleNameWriterError(String);

impl fmt::Display for QuestBattleNameWriterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestBattleNameWriterError {}

fn error(message: impl Into<String>) -> QuestBattleNameWriterError {
    QuestBattleNameWriterError(message.into())
}

fn sha256_hex(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn update_len_prefixed(digest: &mut Sha256, raw: &[u8]) {
    digest.update(u64::try_from(raw.len()).unwrap_or(u64::MAX).to_le_bytes());
    digest.update(raw);
}

fn battle_name_code_supply_sha256(
    page_id: u8,
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"quest-battle-name-code-supply\0");
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

fn battle_name_writer_provenance_sha256(writer: &QuestBattleNameWriterAnalysis) -> String {
    let mut digest = Sha256::new();
    digest.update(b"quest-battle-name-writer\0");
    digest.update([writer.page_id]);
    update_len_prefixed(&mut digest, writer.code_supply_sha256.as_bytes());
    for value in [
        writer.physical_names,
        writer.unique_sources,
        writer.source_bytes,
        writer.output_bytes,
        writer.glyph_cells,
        writer.selected_drafts,
        writer.needs_human_choice,
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
        digest.update(u64::try_from(write.index).unwrap_or(u64::MAX).to_le_bytes());
        digest.update(write.runtime.to_le_bytes());
        digest.update(write.end_runtime.to_le_bytes());
        digest.update(
            u64::try_from(write.text_offset)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        digest.update([write.page_id]);
        update_len_prefixed(&mut digest, write.source_text.as_bytes());
        update_len_prefixed(&mut digest, write.development_text.as_bytes());
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
            u64::try_from(write.slot_capacity_glyphs)
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        update_len_prefixed(&mut digest, write.decision_status.label().as_bytes());
    }
    format!("{:x}", digest.finalize())
}

pub(super) fn verify_quest_battle_name_writer_provenance(
    writer: &QuestBattleNameWriterAnalysis,
) -> Result<(), QuestBattleNameWriterError> {
    if writer.code_supply_sha256.len() != 64
        || writer.writer_provenance_sha256 != battle_name_writer_provenance_sha256(writer)
    {
        return Err(error("QUEST battle name writer provenance 불일치"));
    }
    Ok(())
}

pub(super) fn verify_quest_battle_name_writer_code_supply(
    writer: &QuestBattleNameWriterAnalysis,
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> Result<(), QuestBattleNameWriterError> {
    verify_quest_battle_name_writer_provenance(writer)?;
    quest_page_code_maps(encoding, package, code_table)
        .map_err(|source| error(source.to_string()))?;
    if writer.code_supply_sha256
        != battle_name_code_supply_sha256(writer.page_id, encoding, package, code_table)
    {
        return Err(error("QUEST battle name writer code supply 결속 불일치"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_battle_name_slots(
    map: &ExeMap,
    executable_text: &[u8],
    table: &QuestBattleNameTableAnalysis,
    compositions: &QuestBattleNameCompositionAnalysis,
    page_id: u8,
    page_codes: &BTreeMap<u8, BTreeMap<char, [u8; 2]>>,
    code_table: &QuestCodeTable,
    code_supply_sha256: String,
) -> Result<QuestBattleNameWriterAnalysis, QuestBattleNameWriterError> {
    if map.name != "QUEST.EXE"
        || executable_text.len() != map.text_len as usize
        || page_id == 0
        || table.entries.is_empty()
        || compositions.translation_approved
        || compositions.layout_approved
        || compositions.runtime_verified
        || compositions.product_eligible
    {
        return Err(error("QUEST battle name writer 개발 입력 경계 drift"));
    }
    let mut composition_by_id = BTreeMap::new();
    for composition in &compositions.compositions {
        if composition_by_id
            .insert(composition.unit_id.as_str(), composition)
            .is_some()
        {
            return Err(error(format!(
                "QUEST battle name writer composition ID 중복: {}",
                composition.unit_id
            )));
        }
    }
    if composition_by_id.len() != table.entries.len() {
        return Err(error(
            "QUEST battle name writer source/composition 분모 불일치",
        ));
    }

    let mut writes = Vec::with_capacity(table.entries.len());
    let mut used_glyphs = BTreeSet::new();
    let mut ranges = Vec::with_capacity(table.entries.len());
    for entry in &table.entries {
        let composition = composition_by_id
            .remove(entry.id.as_str())
            .ok_or_else(|| error(format!("{}: battle name composition 누락", entry.id)))?;
        let text_offset = map
            .text_offset(entry.name_runtime)
            .ok_or_else(|| error(format!("{}: name runtime 사상 실패", entry.id)))?;
        let slot_end = text_offset
            .checked_add(entry.protected_slot.len())
            .ok_or_else(|| error(format!("{}: slot 끝 overflow", entry.id)))?;
        let runtime_span = u32::try_from(entry.protected_slot.len())
            .ok()
            .and_then(|span| entry.name_runtime.checked_add(span));
        let source_glyph_bytes = entry
            .glyph_occurrences
            .checked_mul(2)
            .ok_or_else(|| error(format!("{}: source glyph byte overflow", entry.id)))?;
        let source_nul_end = source_glyph_bytes
            .checked_add(2)
            .ok_or_else(|| error(format!("{}: source NUL 끝 overflow", entry.id)))?;
        let actual_sha256: [u8; 32] = Sha256::digest(&entry.protected_slot).into();
        if entry.name_text_offset != text_offset
            || entry.slot_end_text_offset != slot_end
            || runtime_span != Some(entry.slot_end_runtime)
            || executable_text.get(text_offset..slot_end) != Some(entry.protected_slot.as_slice())
            || actual_sha256 != entry.protected_slot_sha256
            || entry.source.chars().count() != entry.glyph_occurrences
            || entry.source_raw.len() != source_glyph_bytes
            || entry.protected_slot.get(..source_glyph_bytes) != Some(entry.source_raw.as_slice())
            || entry.protected_slot.get(source_glyph_bytes..source_nul_end)
                != Some([0, 0].as_slice())
            || entry.protected_slot[source_nul_end..]
                .iter()
                .any(|byte| *byte != 0)
            || entry.protected_slot.len()
                != entry
                    .slot_capacity_glyphs
                    .checked_add(1)
                    .and_then(|cells| cells.checked_mul(2))
                    .unwrap_or(usize::MAX)
            || entry
                .source_raw
                .chunks_exact(2)
                .any(|pair| code_table.index_of([pair[0], pair[1]]).is_none())
        {
            return Err(error(format!(
                "{}: battle name 원본 byte/span/NUL/digest 불일치",
                entry.id
            )));
        }
        if composition.index != entry.index
            || composition.protected_source != entry.source
            || composition.protected_slot_sha256 != entry.protected_slot_sha256
            || composition.slot_capacity_glyphs != entry.slot_capacity_glyphs
        {
            return Err(error(format!(
                "{}: battle name composition/source 결속 불일치",
                entry.id
            )));
        }
        if select_subject_particle(composition.pronunciation_tail)
            != Ok(composition.subject_particle)
        {
            return Err(error(format!(
                "{}: battle name 주격 조사 결속 불일치",
                entry.id
            )));
        }
        let rendered_text = format!(
            "{}{}",
            composition.development_text, composition.subject_particle
        );
        let encoded = encode_quest_fixed_slot_text(
            &rendered_text,
            entry.protected_slot.len(),
            page_id,
            page_codes,
            code_table,
        )
        .map_err(|source| error(format!("{}: {source}", entry.id)))?;
        if encoded.glyph_cells > entry.slot_capacity_glyphs {
            return Err(error(format!(
                "{}: battle name output이 slot glyph 용량을 넘음",
                entry.id
            )));
        }
        used_glyphs.extend(encoded.normalized_text.chars());
        ranges.push((text_offset, slot_end, entry.id.as_str()));
        writes.push(QuestBattleNameWrite {
            unit_id: entry.id.clone(),
            index: entry.index,
            runtime: entry.name_runtime,
            end_runtime: entry.slot_end_runtime,
            text_offset,
            page_id,
            source_text: entry.source.clone(),
            development_text: encoded.normalized_text,
            source: entry.protected_slot.clone(),
            source_sha256: sha256_hex(&entry.protected_slot),
            output_sha256: sha256_hex(&encoded.output),
            output: encoded.output,
            glyph_cells: encoded.glyph_cells,
            slot_capacity_glyphs: entry.slot_capacity_glyphs,
            decision_status: composition.decision_status,
        });
    }
    if !composition_by_id.is_empty() {
        return Err(error(
            "QUEST battle name writer에 미소비 composition이 남음",
        ));
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(error("QUEST battle name writer 물리 slot이 겹침"));
    }

    let used_glyphs = used_glyphs.into_iter().collect::<String>();
    let declared_used_glyphs = compositions
        .used_glyphs
        .chars()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<String>();
    let source_bytes = writes.iter().map(|write| write.source.len()).sum();
    let output_bytes = writes.iter().map(|write| write.output.len()).sum();
    let glyph_cells = writes.iter().map(|write| write.glyph_cells).sum();
    let selected_drafts = writes
        .iter()
        .filter(|write| {
            write.decision_status == QuestBattleNameDecisionStatus::DraftNeedsIndependentReview
        })
        .count();
    let needs_human_choice = writes
        .iter()
        .filter(|write| {
            write.decision_status == QuestBattleNameDecisionStatus::DraftNeedsHumanChoice
        })
        .count();
    let source_exact = source_bytes == table.slot_bytes;
    let nul_terminal_exact = writes.iter().all(|write| {
        let nul_offset = write.glyph_cells * 2;
        write.output.get(nul_offset..nul_offset + 2) == Some([0, 0].as_slice())
            && write.output[nul_offset..].iter().all(|byte| *byte == 0)
    });
    let mut writer = QuestBattleNameWriterAnalysis {
        physical_names: writes.len(),
        unique_sources: table.unique_sources,
        source_bytes,
        output_bytes,
        glyph_cells,
        page_id,
        used_glyphs,
        selected_drafts,
        needs_human_choice,
        source_exact,
        nul_terminal_exact,
        japanese_glyphs_remaining: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
        code_supply_sha256,
        writer_provenance_sha256: String::new(),
        writes,
    };
    if writer.used_glyphs != declared_used_glyphs
        || writer.physical_names != table.entries.len()
        || writer.selected_drafts != compositions.selected_drafts
        || writer.needs_human_choice != compositions.needs_human_choice
        || writer.source_bytes != writer.output_bytes
        || !writer.source_exact
        || !writer.nul_terminal_exact
        || writer
            .writes
            .iter()
            .all(|write| write.source == write.output)
    {
        return Err(error("QUEST battle name writer 최종 결속 drift"));
    }
    writer.writer_provenance_sha256 = battle_name_writer_provenance_sha256(&writer);
    verify_quest_battle_name_writer_provenance(&writer)?;
    Ok(writer)
}

/// source-bound 전투 이름 전수를 조우 공통 page와 원래 EXE slot에 결속한다.
#[allow(clippy::too_many_arguments)]
pub fn audit_quest_battle_name_writer(
    map: &ExeMap,
    executable_text: &[u8],
    table: &QuestBattleNameTableAnalysis,
    compositions: &QuestBattleNameCompositionAnalysis,
    page_id: u8,
    encoding: &QuestTranslationEncodingPlan,
    package: &QuestGlyphCachePackage,
    code_table: &QuestCodeTable,
) -> Result<QuestBattleNameWriterAnalysis, QuestBattleNameWriterError> {
    if usize::from(page_id) > package.plan.pages {
        return Err(error("QUEST battle name writer page가 package 범위 밖임"));
    }
    let page_codes = quest_page_code_maps(encoding, package, code_table)
        .map_err(|source| error(source.to_string()))?;
    write_battle_name_slots(
        map,
        executable_text,
        table,
        compositions,
        page_id,
        &page_codes,
        code_table,
        battle_name_code_supply_sha256(page_id, encoding, package, code_table),
    )
}

#[cfg(test)]
pub(super) fn build_quest_battle_name_writer_fixture(
    writes: Vec<QuestBattleNameWrite>,
) -> QuestBattleNameWriterAnalysis {
    let page_id = writes.first().expect("battle name fixture write").page_id;
    let source_bytes = writes.iter().map(|write| write.source.len()).sum();
    let output_bytes = writes.iter().map(|write| write.output.len()).sum();
    let glyph_cells = writes.iter().map(|write| write.glyph_cells).sum();
    let used_glyphs = writes
        .iter()
        .flat_map(|write| write.development_text.chars())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let unique_sources = writes
        .iter()
        .map(|write| write.source_text.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let selected_drafts = writes
        .iter()
        .filter(|write| {
            write.decision_status == QuestBattleNameDecisionStatus::DraftNeedsIndependentReview
        })
        .count();
    let needs_human_choice = writes
        .iter()
        .filter(|write| {
            write.decision_status == QuestBattleNameDecisionStatus::DraftNeedsHumanChoice
        })
        .count();
    let mut writer = QuestBattleNameWriterAnalysis {
        physical_names: writes.len(),
        unique_sources,
        source_bytes,
        output_bytes,
        glyph_cells,
        page_id,
        used_glyphs,
        selected_drafts,
        needs_human_choice,
        source_exact: true,
        nul_terminal_exact: true,
        japanese_glyphs_remaining: 0,
        translation_approved: false,
        layout_approved: false,
        runtime_verified: false,
        product_eligible: false,
        code_supply_sha256: "0".repeat(64),
        writer_provenance_sha256: String::new(),
        writes,
    };
    writer.writer_provenance_sha256 = battle_name_writer_provenance_sha256(&writer);
    writer
}

#[cfg(test)]
#[path = "quest_battle_name_writer_tests.rs"]
mod tests;
