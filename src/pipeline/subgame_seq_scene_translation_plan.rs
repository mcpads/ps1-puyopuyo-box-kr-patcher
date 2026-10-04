//! scene-local E0 Unicode 번역의 pair 배정·물리 record 합성.
//!
//! 논리 manifest의 annotated Unicode를 family별 canonical repertoire pair로
//! 컴파일한다. PUYO1/PUYO2 사본은 같은 논리 문장을 공유해도 각자의 원본
//! header와 물리 ID를 다시 대조한다. 결과 record는 bounded scene relinker에
//! 넘겨 실제 block 용량과 모든 incoming pointer를 검산한다. BTX cell 픽셀
//! 합성은 mapping을 소비하는 다음 계층의 책임이다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::analysis::{
    SubgameSeqSceneGlyphAnalysis, SubgameSeqSceneGlyphRow, SubgameSeqSceneMessageAnalysis,
};
use crate::text::decode_sjis;

use super::{
    repack_box_subgame_seq_scene_messages, SubgameSeqSceneRelinkAnalysis, SubgameSeqSceneRepack,
    SubgameSeqSceneReplacement, SubgameSeqSceneTranslationManifestAnalysis,
    SubgameSeqSceneTranslationStatus,
};

pub const SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_SCHEMA_VERSION: &str =
    "subgame-seq-scene-translation-plan-v1";
pub const BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_FAMILIES: usize = 17;
pub const BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_PHYSICAL_MESSAGES: usize = 194;
pub const BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_SYNTHETIC_GLYPHS: usize = 17;
pub const BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_CAPACITY_BYTES: usize = 7_526;
pub const BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_USED_BYTES: usize = 2_806;
pub const BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_REMAINING_BYTES: usize = 4_720;
pub const BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_REPORT_SHA256: &str =
    "98cb12fc4e30effc325294ac91eac65aff03ef5b323d82a2fbdc1f450c2fd133";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneTranslation {
    pub logical_id: String,
    pub annotated: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneGlyphMapping {
    pub sequence: String,
    pub glyph: char,
    pub repertoire_index: usize,
    pub pair: [u8; 2],
}

impl SubgameSeqSceneGlyphMapping {
    pub fn cell_index(&self) -> usize {
        self.repertoire_index + 30
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneTranslationPlanRow {
    pub scope: &'static str,
    pub sequence: &'static str,
    pub messages: usize,
    pub mappings: usize,
    pub capacity_bytes: usize,
    pub used_bytes: usize,
    pub remaining_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneTranslationPlan {
    pub replacements: Vec<SubgameSeqSceneReplacement>,
    pub glyph_mappings: Vec<SubgameSeqSceneGlyphMapping>,
    pub repacks: Vec<SubgameSeqSceneRepack>,
    pub rows: Vec<SubgameSeqSceneTranslationPlanRow>,
    pub families: usize,
    pub physical_messages: usize,
    pub capacity_bytes: usize,
    pub used_bytes: usize,
    pub remaining_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneTranslationPlanError(String);

impl fmt::Display for SubgameSeqSceneTranslationPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqSceneTranslationPlanError {}

fn error(message: impl Into<String>) -> SubgameSeqSceneTranslationPlanError {
    SubgameSeqSceneTranslationPlanError(message.into())
}

/// 한 family의 사람이 작성한 초안 또는 승인문을 일반 scene compiler 입력으로 모은다.
pub fn collect_subgame_seq_scene_family_translations(
    manifest: &SubgameSeqSceneTranslationManifestAnalysis,
    sequence: &str,
) -> Result<Vec<SubgameSeqSceneTranslation>, SubgameSeqSceneTranslationPlanError> {
    let entries = manifest
        .entries
        .iter()
        .filter(|entry| entry.sequence == sequence)
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return Err(error(format!(
            "{sequence} scene translation family가 manifest에 없음"
        )));
    }
    if let Some(entry) = entries
        .iter()
        .find(|entry| entry.status == SubgameSeqSceneTranslationStatus::PendingHumanTranslation)
    {
        return Err(error(format!(
            "{} scene translation family에 미작성 행이 있음",
            entry.logical_id
        )));
    }
    Ok(entries
        .into_iter()
        .map(|entry| SubgameSeqSceneTranslation {
            logical_id: entry.logical_id.clone(),
            annotated: entry.ko.clone(),
        })
        .collect())
}

/// 번역이 시작된 family는 전 행을 요구하고, 전부 pending인 family는 보존한다.
pub fn collect_authored_subgame_seq_scene_translations(
    manifest: &SubgameSeqSceneTranslationManifestAnalysis,
) -> Result<Vec<SubgameSeqSceneTranslation>, SubgameSeqSceneTranslationPlanError> {
    let mut translations = Vec::new();
    for sequence in manifest
        .entries
        .iter()
        .map(|entry| entry.sequence.as_str())
        .collect::<BTreeSet<_>>()
    {
        let entries = manifest
            .entries
            .iter()
            .filter(|entry| entry.sequence == sequence)
            .collect::<Vec<_>>();
        let authored = entries
            .iter()
            .filter(|entry| {
                entry.status != SubgameSeqSceneTranslationStatus::PendingHumanTranslation
            })
            .count();
        if authored == 0 {
            continue;
        }
        if authored != entries.len() {
            return Err(error(format!(
                "{sequence} scene translation family가 일부 행만 작성됨"
            )));
        }
        translations.extend(entries.into_iter().map(|entry| SubgameSeqSceneTranslation {
            logical_id: entry.logical_id.clone(),
            annotated: entry.ko.clone(),
        }));
    }
    Ok(translations)
}

fn select_complete_translation_families<'a>(
    translation_ids: &BTreeSet<&str>,
    manifest: &'a SubgameSeqSceneTranslationManifestAnalysis,
) -> Result<
    Vec<&'a super::SubgameSeqSceneTranslationManifestEntry>,
    SubgameSeqSceneTranslationPlanError,
> {
    if translation_ids.is_empty() {
        return Err(error("scene translation 입력이 비어 있음"));
    }
    let manifest_ids = manifest
        .entries
        .iter()
        .map(|entry| entry.logical_id.as_str())
        .collect::<BTreeSet<_>>();
    if let Some(unknown) = translation_ids.difference(&manifest_ids).next() {
        return Err(error(format!(
            "scene translation manifest에 없는 논리 ID: {unknown}"
        )));
    }
    let selected_sequences = manifest
        .entries
        .iter()
        .filter(|entry| translation_ids.contains(entry.logical_id.as_str()))
        .map(|entry| entry.sequence.as_str())
        .collect::<BTreeSet<_>>();
    let selected_entries = manifest
        .entries
        .iter()
        .filter(|entry| selected_sequences.contains(entry.sequence.as_str()))
        .collect::<Vec<_>>();
    if let Some(missing) = selected_entries
        .iter()
        .find(|entry| !translation_ids.contains(entry.logical_id.as_str()))
    {
        return Err(error(format!(
            "{} scene translation family가 일부 행만 선택됨: missing={}",
            missing.sequence, missing.logical_id
        )));
    }
    Ok(selected_entries)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AnnotatedPart {
    Text(String),
    Control(Vec<u8>),
}

fn parse_hex_byte(raw: &str) -> Option<u8> {
    (raw.len() == 2)
        .then(|| u8::from_str_radix(raw, 16).ok())
        .flatten()
}

fn parse_annotated(raw: &str) -> Result<Vec<AnnotatedPart>, SubgameSeqSceneTranslationPlanError> {
    if raw.is_empty() || raw.contains(['\t', '\r', '\n']) {
        return Err(error(
            "scene translation annotated text가 비었거나 개행/tab을 포함함",
        ));
    }
    let mut parts = Vec::new();
    let mut rest = raw;
    while let Some(start) = rest.find('<') {
        if rest[..start].contains('>') {
            return Err(error("scene translation placeholder 괄호 순서 불일치"));
        }
        if start != 0 {
            parts.push(AnnotatedPart::Text(rest[..start].to_string()));
        }
        let after_start = &rest[start + 1..];
        let end = after_start
            .find('>')
            .ok_or_else(|| error("scene translation placeholder 종결 누락"))?;
        let body = &after_start[..end];
        let control = if body.len() == 4 && !body.contains(':') {
            vec![
                parse_hex_byte(&body[..2])
                    .ok_or_else(|| error("scene translation placeholder hex 불일치"))?,
                parse_hex_byte(&body[2..])
                    .ok_or_else(|| error("scene translation placeholder hex 불일치"))?,
            ]
        } else if let Some((code, argument)) = body.split_once(':') {
            vec![
                parse_hex_byte(code)
                    .ok_or_else(|| error("scene translation control code hex 불일치"))?,
                parse_hex_byte(argument)
                    .ok_or_else(|| error("scene translation control argument hex 불일치"))?,
            ]
        } else {
            vec![parse_hex_byte(body)
                .ok_or_else(|| error("scene translation control code hex 불일치"))?]
        };
        parts.push(AnnotatedPart::Control(control));
        rest = &after_start[end + 1..];
    }
    if rest.contains('>') {
        return Err(error("scene translation placeholder 닫힘 괄호만 있음"));
    }
    if !rest.is_empty() {
        parts.push(AnnotatedPart::Text(rest.to_string()));
    }
    if !matches!(parts.last(), Some(AnnotatedPart::Control(raw)) if raw == &[0x65, 0x65]) {
        return Err(error("scene translation 65 65 종결 누락"));
    }
    if parts[..parts.len() - 1]
        .iter()
        .any(|part| matches!(part, AnnotatedPart::Control(raw) if raw == &[0x65, 0x65]))
    {
        return Err(error("scene translation 65 65 종결이 중간에 있음"));
    }
    Ok(parts)
}

fn controls(parts: &[AnnotatedPart]) -> Vec<&[u8]> {
    parts
        .iter()
        .filter_map(|part| match part {
            AnnotatedPart::Control(raw) => Some(raw.as_slice()),
            AnnotatedPart::Text(_) => None,
        })
        .collect()
}

fn is_hangul_syllable(glyph: char) -> bool {
    matches!(glyph, '\u{ac00}'..='\u{d7a3}')
}

fn is_japanese_text_character(glyph: char) -> bool {
    matches!(
        glyph,
        '\u{3040}'..='\u{30ff}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{ff65}'..='\u{ff9f}'
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RepertoireSlot {
    index: usize,
    pair: [u8; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Repertoire {
    by_character: BTreeMap<char, RepertoireSlot>,
    unique_slots: Vec<RepertoireSlot>,
}

fn parse_repertoire(raw: &[u8]) -> Result<Repertoire, SubgameSeqSceneTranslationPlanError> {
    if raw.is_empty() || !raw.len().is_multiple_of(2) {
        return Err(error(
            "scene translation repertoire 길이가 2바이트 정렬이 아님",
        ));
    }
    let mut by_character = BTreeMap::new();
    let mut unique_pairs = BTreeSet::new();
    let mut unique_slots = Vec::new();
    for (index, pair) in raw.chunks_exact(2).enumerate() {
        let pair: [u8; 2] = pair.try_into().expect("2바이트 repertoire pair");
        let decoded = decode_sjis(&pair)
            .map_err(|source| error(format!("scene repertoire decode 실패: {source}")))?;
        let mut characters = decoded.chars();
        let glyph = characters
            .next()
            .ok_or_else(|| error("scene repertoire가 빈 문자로 decode됨"))?;
        if characters.next().is_some() {
            return Err(error("scene repertoire pair가 한 문자가 아님"));
        }
        let slot = RepertoireSlot { index, pair };
        by_character.entry(glyph).or_insert(slot);
        if unique_pairs.insert(pair) {
            unique_slots.push(slot);
        }
    }
    Ok(Repertoire {
        by_character,
        unique_slots,
    })
}

fn canonical_raw<'a>(
    row: &SubgameSeqSceneGlyphRow,
    sources: &BTreeMap<(&str, &str), &'a [u8]>,
) -> Result<&'a [u8], SubgameSeqSceneTranslationPlanError> {
    let raw = sources
        .get(&(row.seq_archive, row.sequence))
        .copied()
        .ok_or_else(|| error(format!("{}:{} scene source 누락", row.scope, row.sequence)))?;
    if <[u8; 32]>::from(Sha256::digest(raw)) != row.sequence_sha256 {
        return Err(error(format!(
            "{}:{} scene source digest 불일치",
            row.scope, row.sequence
        )));
    }
    let end = row
        .repertoire_offset
        .checked_add(row.canonical_glyph_slots * 2)
        .ok_or_else(|| error("scene canonical repertoire 끝 overflow"))?;
    raw.get(row.repertoire_offset..end)
        .ok_or_else(|| error("scene canonical repertoire 범위 이탈"))
}

fn physical_id(scope: &str, sequence: &str, command_offset: usize) -> String {
    format!("{scope}:{sequence}:0x{command_offset:04X}")
}

fn compile_record(
    source_raw: &[u8],
    source_annotated: &str,
    translated: &[AnnotatedPart],
    codes: &BTreeMap<char, [u8; 2]>,
) -> Result<Vec<u8>, SubgameSeqSceneTranslationPlanError> {
    let source_parts = parse_annotated(source_annotated)?;
    if controls(&source_parts) != controls(translated) {
        return Err(error("scene translation control/argument 순서 변경"));
    }
    let header = source_raw
        .get(..4)
        .ok_or_else(|| error("scene translation 원본 header 누락"))?;
    let mut output = header.to_vec();
    for part in translated {
        match part {
            AnnotatedPart::Text(text) => {
                for glyph in text.chars() {
                    output.extend_from_slice(codes.get(&glyph).ok_or_else(|| {
                        error(format!("scene translation glyph pair 누락: {glyph}"))
                    })?);
                }
            }
            AnnotatedPart::Control(raw) => output.extend_from_slice(raw),
        }
    }
    Ok(output)
}

/// 논리 Unicode 입력 전체를 family별 pair와 물리 record로 컴파일한다.
pub fn plan_subgame_seq_scene_translation<'a>(
    translations: &[SubgameSeqSceneTranslation],
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    manifest: &SubgameSeqSceneTranslationManifestAnalysis,
    messages: &SubgameSeqSceneMessageAnalysis,
    glyphs: &SubgameSeqSceneGlyphAnalysis,
    relinks: &SubgameSeqSceneRelinkAnalysis,
) -> Result<SubgameSeqSceneTranslationPlan, SubgameSeqSceneTranslationPlanError> {
    let mut source_map = BTreeMap::new();
    for (archive, sequence, raw) in sources {
        if source_map.insert((archive, sequence), raw).is_some() {
            return Err(error("scene translation source 입력 중복"));
        }
    }
    let translation_map = translations
        .iter()
        .map(|translation| {
            (
                translation.logical_id.as_str(),
                translation.annotated.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if translation_map.len() != translations.len() {
        return Err(error("scene translation 논리 ID 중복"));
    }
    let translation_ids = translation_map.keys().copied().collect::<BTreeSet<_>>();
    let selected_entries = select_complete_translation_families(&translation_ids, manifest)?;
    let selected_sequences = selected_entries
        .iter()
        .map(|entry| entry.sequence.as_str())
        .collect::<BTreeSet<_>>();

    let physical_rows = messages
        .rows
        .iter()
        .map(|row| {
            (
                physical_id(row.scope, row.sequence, row.command_offset),
                row,
            )
        })
        .collect::<BTreeMap<_, _>>();
    if physical_rows.len() != messages.rows.len() {
        return Err(error("scene translation 물리 message ID 중복"));
    }

    let mut repertoire_by_sequence = BTreeMap::<&str, Repertoire>::new();
    for &sequence in &selected_sequences {
        let rows = glyphs
            .rows
            .iter()
            .filter(|row| row.sequence == sequence)
            .collect::<Vec<_>>();
        let [left, right] = rows.as_slice() else {
            return Err(error(format!(
                "{sequence} scene translation glyph 사본 분모 불일치"
            )));
        };
        let left_raw = canonical_raw(left, &source_map)?;
        let right_raw = canonical_raw(right, &source_map)?;
        if left_raw != right_raw || left.canonical_glyph_slots != right.canonical_glyph_slots {
            return Err(error(format!(
                "{sequence} scene translation 사본 repertoire 불일치"
            )));
        }
        repertoire_by_sequence.insert(sequence, parse_repertoire(left_raw)?);
    }

    let mut parsed = BTreeMap::<&str, Vec<AnnotatedPart>>::new();
    let mut requested = BTreeMap::<&str, Vec<char>>::new();
    for entry in &selected_entries {
        let annotated = translation_map
            .get(entry.logical_id.as_str())
            .copied()
            .ok_or_else(|| error("scene translation 논리 ID 누락"))?;
        let parts = parse_annotated(annotated)?;
        let source_parts = parse_annotated(&entry.protected_source)?;
        if controls(&parts) != controls(&source_parts) {
            return Err(error(format!(
                "{} scene translation control placeholder drift",
                entry.logical_id
            )));
        }
        let sequence_requested = requested.entry(entry.sequence.as_str()).or_default();
        for glyph in parts
            .iter()
            .filter_map(|part| match part {
                AnnotatedPart::Text(text) => Some(text.chars()),
                AnnotatedPart::Control(_) => None,
            })
            .flatten()
        {
            if !sequence_requested.contains(&glyph) {
                sequence_requested.push(glyph);
            }
        }
        parsed.insert(entry.logical_id.as_str(), parts);
    }

    let mut glyph_mappings = Vec::new();
    let mut codes_by_sequence = BTreeMap::<&str, BTreeMap<char, [u8; 2]>>::new();
    for (&sequence, requested) in &requested {
        let repertoire = repertoire_by_sequence
            .get(sequence)
            .ok_or_else(|| error("scene translation repertoire 누락"))?;
        let mut retained_slots = BTreeSet::new();
        let mut allocated_glyphs = Vec::new();
        for &glyph in requested {
            if is_hangul_syllable(glyph) || glyph == ' ' {
                // 사람이 쓴 공백도 제어코드로 바꾸지 않고 빈 glyph cell 하나를
                // 배정한다. 따라서 원문 control 순서와 인자는 그대로 유지된다.
                allocated_glyphs.push(glyph);
            } else if is_japanese_text_character(glyph) {
                return Err(error(format!(
                    "{sequence} scene translation에 일본어 잔존: {glyph} (U+{:04X})",
                    glyph as u32
                )));
            } else if let Some(slot) = repertoire.by_character.get(&glyph) {
                retained_slots.insert(slot.index);
            } else {
                return Err(error(format!(
                    "{sequence} scene translation repertoire mapping 누락: {glyph} (U+{:04X})",
                    glyph as u32
                )));
            }
        }
        let available = repertoire
            .unique_slots
            .iter()
            .copied()
            .filter(|slot| !retained_slots.contains(&slot.index))
            .collect::<Vec<_>>();
        if allocated_glyphs.len() > available.len() {
            return Err(error(format!(
                "{sequence} scene translation repertoire 용량 초과: required={}, available={}",
                allocated_glyphs.len(),
                available.len()
            )));
        }
        let mut codes = repertoire
            .by_character
            .iter()
            .map(|(&glyph, slot)| (glyph, slot.pair))
            .collect::<BTreeMap<_, _>>();
        for (glyph, slot) in allocated_glyphs.into_iter().zip(available) {
            codes.insert(glyph, slot.pair);
            glyph_mappings.push(SubgameSeqSceneGlyphMapping {
                sequence: sequence.to_string(),
                glyph,
                repertoire_index: slot.index,
                pair: slot.pair,
            });
        }
        codes_by_sequence.insert(sequence, codes);
    }

    let mut replacement_map = BTreeMap::<String, Vec<u8>>::new();
    for entry in &selected_entries {
        let parts = parsed
            .get(entry.logical_id.as_str())
            .ok_or_else(|| error("scene translation parsed 입력 누락"))?;
        let codes = codes_by_sequence
            .get(entry.sequence.as_str())
            .ok_or_else(|| error("scene translation code map 누락"))?;
        let ids = [
            entry.puyo1_message_id.as_deref(),
            entry.puyo2_message_id.as_deref(),
        ];
        for id in ids.into_iter().flatten() {
            let source = physical_rows
                .get(id)
                .copied()
                .ok_or_else(|| error(format!("scene translation 물리 ID 누락: {id}")))?;
            let source_digest = source
                .raw_sha256
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            if source.sequence != entry.sequence
                || source_digest != entry.raw_sha256
                || source.source_text != entry.protected_source
            {
                return Err(error(format!(
                    "{} scene translation manifest/물리 원본 계약 불일치: {id}",
                    entry.logical_id
                )));
            }
            let replacement = compile_record(&source.raw, &entry.protected_source, parts, codes)?;
            if replacement_map
                .insert(id.to_string(), replacement)
                .is_some()
            {
                return Err(error(format!("scene translation replacement 중복: {id}")));
            }
        }
    }
    let expected_physical_messages = selected_entries
        .iter()
        .map(|entry| entry.copy_count)
        .sum::<usize>();
    if replacement_map.len() != expected_physical_messages {
        return Err(error("scene translation 물리 replacement 분모 불일치"));
    }
    let replacements = replacement_map
        .iter()
        .map(|(message_id, raw)| SubgameSeqSceneReplacement {
            message_id: message_id.clone(),
            raw: raw.clone(),
        })
        .collect::<Vec<_>>();

    let selected_relinks = relinks
        .rows
        .iter()
        .filter(|relink| selected_sequences.contains(relink.sequence))
        .collect::<Vec<_>>();
    let mut repacks = Vec::with_capacity(selected_relinks.len());
    let mut rows = Vec::with_capacity(selected_relinks.len());
    for relink in selected_relinks {
        let raw = source_map
            .get(&(relink.archive, relink.sequence))
            .copied()
            .ok_or_else(|| error("scene translation repack source 누락"))?;
        let scene_replacements = messages
            .rows
            .iter()
            .filter(|row| row.scope == relink.scope && row.sequence == relink.sequence)
            .map(|row| {
                let id = physical_id(row.scope, row.sequence, row.command_offset);
                let replacement = replacement_map
                    .get(&id)
                    .ok_or_else(|| error("scene translation scene replacement 누락"))?;
                Ok(SubgameSeqSceneReplacement {
                    message_id: id,
                    raw: replacement.clone(),
                })
            })
            .collect::<Result<Vec<_>, SubgameSeqSceneTranslationPlanError>>()?;
        let repack = repack_box_subgame_seq_scene_messages(
            relink.scope,
            relink.sequence,
            raw,
            &scene_replacements,
            messages,
            glyphs,
            relinks,
        )
        .map_err(|source| error(source.to_string()))?;
        let capacity_bytes = relink.blocks.iter().map(|block| block.capacity_bytes).sum();
        let mappings = glyph_mappings
            .iter()
            .filter(|mapping| mapping.sequence == relink.sequence)
            .count();
        rows.push(SubgameSeqSceneTranslationPlanRow {
            scope: relink.scope,
            sequence: relink.sequence,
            messages: scene_replacements.len(),
            mappings,
            capacity_bytes,
            used_bytes: repack.used_bytes,
            remaining_bytes: repack.remaining_bytes,
            output_sha256: format!("{:x}", Sha256::digest(&repack.sequence)),
        });
        repacks.push(repack);
    }
    let capacity_bytes = rows.iter().map(|row| row.capacity_bytes).sum();
    let used_bytes = rows.iter().map(|row| row.used_bytes).sum();
    let remaining_bytes = rows.iter().map(|row| row.remaining_bytes).sum();
    Ok(SubgameSeqSceneTranslationPlan {
        families: selected_sequences.len(),
        physical_messages: replacements.len(),
        replacements,
        glyph_mappings,
        repacks,
        rows,
        capacity_bytes,
        used_bytes,
        remaining_bytes,
    })
}

fn synthetic_annotated(source: &str) -> Result<String, SubgameSeqSceneTranslationPlanError> {
    let parts = parse_annotated(source)?;
    let mut output = String::new();
    for part in parts {
        match part {
            AnnotatedPart::Text(_) => output.push('가'),
            AnnotatedPart::Control(raw) if raw.len() == 1 => {
                write!(output, "<{:02X}>", raw[0]).expect("String 쓰기는 실패하지 않는다");
            }
            AnnotatedPart::Control(raw) if raw == [0x65, 0x65] => {
                output.push_str("<6565>");
            }
            AnnotatedPart::Control(raw) if raw.len() == 2 => {
                write!(output, "<{:02X}:{:02X}>", raw[0], raw[1])
                    .expect("String 쓰기는 실패하지 않는다");
            }
            AnnotatedPart::Control(_) => {
                return Err(error("scene synthetic control 폭 미지원"));
            }
        }
    }
    Ok(output)
}

/// 지원 원본 전체를 family당 한글 한 글자로 축약해 정적 compiler를 검산한다.
pub fn verify_box_subgame_seq_scene_translation_plan<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    manifest: &SubgameSeqSceneTranslationManifestAnalysis,
    messages: &SubgameSeqSceneMessageAnalysis,
    glyphs: &SubgameSeqSceneGlyphAnalysis,
    relinks: &SubgameSeqSceneRelinkAnalysis,
) -> Result<SubgameSeqSceneTranslationPlan, SubgameSeqSceneTranslationPlanError> {
    let translations = manifest
        .entries
        .iter()
        .map(|entry| {
            Ok(SubgameSeqSceneTranslation {
                logical_id: entry.logical_id.clone(),
                annotated: synthetic_annotated(&entry.protected_source)?,
            })
        })
        .collect::<Result<Vec<_>, SubgameSeqSceneTranslationPlanError>>()?;
    let plan = plan_subgame_seq_scene_translation(
        &translations,
        sources,
        manifest,
        messages,
        glyphs,
        relinks,
    )?;
    let actual = (
        plan.families,
        plan.physical_messages,
        plan.glyph_mappings.len(),
        plan.capacity_bytes,
        plan.used_bytes,
        plan.remaining_bytes,
    );
    let expected = (
        BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_FAMILIES,
        BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_PHYSICAL_MESSAGES,
        BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_SYNTHETIC_GLYPHS,
        BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_CAPACITY_BYTES,
        BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_USED_BYTES,
        BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_REMAINING_BYTES,
    );
    if actual != expected {
        return Err(error(format!(
            "scene translation synthetic 기준선 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }
    let report = render_subgame_seq_scene_translation_plan_tsv(&plan);
    let digest = format!("{:x}", Sha256::digest(report.as_bytes()));
    if digest != BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_REPORT_SHA256 {
        return Err(error(format!(
            "scene translation plan report SHA-256 불일치: expected={}, actual={digest}",
            BOX_SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_REPORT_SHA256
        )));
    }
    Ok(plan)
}

pub fn render_subgame_seq_scene_translation_plan_tsv(
    plan: &SubgameSeqSceneTranslationPlan,
) -> String {
    let mut output = String::from(
        "schema_version\tscope\tsequence\tmessages\tglyph_mappings\tcapacity_bytes\tused_bytes\tremaining_bytes\toutput_sha256\tstatic_status\n",
    );
    for row in &plan.rows {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tsynthetic-one-glyph-per-source-run-bounded-repack-exact-btx-runtime-unverified",
            SUBGAME_SEQ_SCENE_TRANSLATION_PLAN_SCHEMA_VERSION,
            row.scope,
            row.sequence,
            row.messages,
            row.mappings,
            row.capacity_bytes,
            row.used_bytes,
            row.remaining_bytes,
            row.output_sha256,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

#[cfg(test)]
#[path = "subgame_seq_scene_translation_plan_tests.rs"]
mod tests;
