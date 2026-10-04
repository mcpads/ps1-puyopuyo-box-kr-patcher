//! 17개 PUYO1/PUYO2 scene family의 E0 message 모집단.
//!
//! E0 command는 같은 overlay 안의 4바이트 header+token body를 직접 가리킨다.
//! 이 모듈은 정렬된 E0 record를 전수 순회하고, 공유 DRAC consumer에서 typed
//! MIPS로 검증한 token 범위·추가 인자 폭만 사용해 record를 빈틈없이 나눈다.
//! 의미를 확정하지 않은 `0x63`과 PUYO1 전용 `0x78`은 1바이트 `forbidden`
//! token으로 원위치 보존한다. runtime 실행·화면 표시와 가변 길이 재배치는
//! 별도 gate다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::disasm::{
    SubgameSeqDracConsumerAnalysis, BOX_SUBGAME_SEQ_DRAC_ARGUMENT_TOKENS,
    BOX_SUBGAME_SEQ_DRAC_TOKEN_FIRST,
};
use crate::script::SubgameSeqLayoutAnalysis;
use crate::text::decode_sjis;

use super::SubgameSeqSceneGlyphAnalysis;

pub const SUBGAME_SEQ_SCENE_MESSAGE_SCHEMA_VERSION: &str = "subgame-seq-scene-messages-v1";

pub const BOX_SUBGAME_SEQ_SCENE_MESSAGE_ROWS: usize = 194;
pub const BOX_SUBGAME_SEQ_SCENE_MESSAGE_FAMILIES: usize = 17;
pub const BOX_SUBGAME_SEQ_SCENE_PHYSICAL_MESSAGES: usize = 194;
pub const BOX_SUBGAME_SEQ_SCENE_MESSAGE_GLYPH_TOKENS: usize = 2_772;
pub const BOX_SUBGAME_SEQ_SCENE_MESSAGE_CONTROL_TOKENS: usize = 615;
pub const BOX_SUBGAME_SEQ_SCENE_MESSAGE_ARGUMENT_CONTROLS: usize = 203;
pub const BOX_SUBGAME_SEQ_SCENE_MESSAGE_TERMINATORS: usize = 194;
pub const BOX_SUBGAME_SEQ_SCENE_MESSAGE_VARIANT_RECORDS: usize = 81;
pub const BOX_SUBGAME_SEQ_SCENE_MESSAGE_DIRECT_POINTERS: usize = 194;
pub const BOX_SUBGAME_SEQ_SCENE_MESSAGE_REFERENCED_BYTES: usize = 7_526;
pub const BOX_SUBGAME_SEQ_SCENE_MESSAGE_UNIQUE_BYTES: usize = 7_526;
pub const BOX_SUBGAME_SEQ_SCENE_MESSAGE_UNIQUE_PAYLOADS: usize = 100;
pub const BOX_SUBGAME_SEQ_SCENE_MESSAGE_CROSS_SCOPE_PAIRS: usize = 94;

/// 지원 원본에서 만든 전체 E0 message TSV의 SHA-256.
pub const BOX_SUBGAME_SEQ_SCENE_MESSAGE_REPORT_SHA256: [u8; 32] = [
    0x74, 0x6D, 0xEB, 0x61, 0x83, 0x18, 0xA6, 0x8A, 0x8B, 0x13, 0x76, 0x36, 0xC5, 0x53, 0xFE, 0x2D,
    0xDC, 0x41, 0xF8, 0xC3, 0x9B, 0xE8, 0x8B, 0xDB, 0x6F, 0x18, 0xAC, 0x1A, 0xDC, 0xA8, 0x9C, 0xFF,
];

const BOX_SUBGAME_SEQ_SCENE_MESSAGE_CONTROL_COUNTS: [(u8, usize); 6] = [
    (0x63, 8),
    (0x71, 63),
    (0x72, 344),
    (0x73, 56),
    (0x77, 140),
    (0x78, 4),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneMessageRow {
    pub scope: &'static str,
    pub exe: &'static str,
    pub archive: &'static str,
    pub sequence: &'static str,
    pub sequence_sha256: [u8; 32],
    pub index: usize,
    pub command_offset: usize,
    pub pointer_field_offset: usize,
    pub pointer_field_runtime: u32,
    pub record_offset: usize,
    pub record_runtime: u32,
    pub record_end_offset: usize,
    pub variant: u8,
    pub header: [u8; 4],
    pub raw: Vec<u8>,
    pub raw_sha256: [u8; 32],
    pub payload_copy_count: usize,
    pub counterpart_scope: Option<&'static str>,
    pub counterpart_index: Option<usize>,
    pub glyph_tokens: usize,
    pub control_tokens: usize,
    pub argument_controls: usize,
    pub unique_glyph_cells: usize,
    pub maximum_glyph_cell: usize,
    pub control_counts: BTreeMap<u8, usize>,
    pub source_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneMessageAnalysis {
    pub rows: Vec<SubgameSeqSceneMessageRow>,
    pub families: usize,
    pub physical_messages: usize,
    pub glyph_tokens: usize,
    pub control_tokens: usize,
    pub argument_controls: usize,
    pub terminators: usize,
    pub variant_records: usize,
    pub direct_pointers: usize,
    pub referenced_message_bytes: usize,
    pub unique_message_bytes: usize,
    pub unique_payloads: usize,
    pub cross_scope_pairs: usize,
    pub control_counts: BTreeMap<u8, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneMessageError(String);

impl fmt::Display for SubgameSeqSceneMessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqSceneMessageError {}

fn error(message: impl Into<String>) -> SubgameSeqSceneMessageError {
    SubgameSeqSceneMessageError(message.into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedMessage {
    end_offset: usize,
    header: [u8; 4],
    raw: Vec<u8>,
    glyph_tokens: usize,
    control_tokens: usize,
    argument_controls: usize,
    glyph_cells: BTreeSet<usize>,
    control_counts: BTreeMap<u8, usize>,
    source_text: String,
}

fn is_sjis_lead(byte: u8) -> bool {
    matches!(byte, 0x81..=0x9F | 0xE0..=0xEF)
}

fn read_u32(raw: &[u8], offset: usize) -> Result<u32, SubgameSeqSceneMessageError> {
    raw.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
        .ok_or_else(|| error(format!("scene message word 범위 이탈: +0x{offset:X}")))
}

fn parse_message(
    raw: &[u8],
    record_offset: usize,
    canonical_pairs: &[[u8; 2]],
    dispatch_first: u8,
    dispatch_last: u8,
    argument_tokens: &[u8],
) -> Result<ParsedMessage, SubgameSeqSceneMessageError> {
    let header: [u8; 4] = raw
        .get(record_offset..record_offset + 4)
        .ok_or_else(|| error("scene message header 범위 이탈"))?
        .try_into()
        .expect("4바이트 header");
    let mut cursor = record_offset + 4;
    let mut glyph_tokens = 0usize;
    let mut control_tokens = 0usize;
    let mut argument_controls = 0usize;
    let mut glyph_cells = BTreeSet::new();
    let mut control_counts = BTreeMap::new();
    let mut source_text = String::new();

    loop {
        if raw.get(cursor..cursor + 2) == Some([0x65, 0x65].as_slice()) {
            source_text.push_str("<6565>");
            let end_offset = cursor + 2;
            let message_raw = raw
                .get(record_offset..end_offset)
                .ok_or_else(|| error("scene message raw 범위 이탈"))?
                .to_vec();
            return Ok(ParsedMessage {
                end_offset,
                header,
                raw: message_raw,
                glyph_tokens,
                control_tokens,
                argument_controls,
                glyph_cells,
                control_counts,
                source_text,
            });
        }
        let first = *raw
            .get(cursor)
            .ok_or_else(|| error("scene message 65 65 종결 누락"))?;
        if is_sjis_lead(first) {
            let pair: [u8; 2] = raw
                .get(cursor..cursor + 2)
                .ok_or_else(|| error("scene message Shift-JIS pair 잘림"))?
                .try_into()
                .expect("2바이트 pair");
            let decoded = decode_sjis(&pair)
                .map_err(|source| error(format!("scene message glyph decode 실패: {source}")))?;
            if decoded.chars().count() != 1 {
                return Err(error("scene message glyph가 한 code unit이 아님"));
            }
            let cell = canonical_pairs
                .iter()
                .position(|candidate| *candidate == pair)
                .ok_or_else(|| {
                    error(format!(
                        "scene message glyph가 canonical repertoire에 없음: {:02X}{:02X}",
                        pair[0], pair[1]
                    ))
                })?;
            glyph_tokens += 1;
            glyph_cells.insert(cell);
            source_text.push_str(&decoded);
            cursor += 2;
            continue;
        }
        if !(dispatch_first..=dispatch_last).contains(&first) {
            return Err(error(format!(
                "scene message의 미지원 byte token: +0x{cursor:X}=0x{first:02X}"
            )));
        }
        control_tokens += 1;
        *control_counts.entry(first).or_default() += 1;
        if argument_tokens.contains(&first) {
            let argument = *raw
                .get(cursor + 1)
                .ok_or_else(|| error(format!("scene control 0x{first:02X} 인자가 잘림")))?;
            write!(source_text, "<{first:02X}:{argument:02X}>")
                .expect("String 쓰기는 실패하지 않는다");
            argument_controls += 1;
            cursor += 2;
        } else {
            write!(source_text, "<{first:02X}>").expect("String 쓰기는 실패하지 않는다");
            cursor += 1;
        }
    }
}

/// 34개 scene-local SEQ에서 정렬된 E0 pointer record를 전수 분석한다.
pub fn audit_subgame_seq_scene_messages<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    layouts: &SubgameSeqLayoutAnalysis,
    glyphs: &SubgameSeqSceneGlyphAnalysis,
    consumers: &SubgameSeqDracConsumerAnalysis,
) -> Result<SubgameSeqSceneMessageAnalysis, SubgameSeqSceneMessageError> {
    let mut source_map = BTreeMap::new();
    for (archive, sequence, raw) in sources {
        if source_map.insert((archive, sequence), raw).is_some() {
            return Err(error(format!(
                "scene message source 입력 중복: {archive}:{sequence}"
            )));
        }
    }
    if glyphs.rows.is_empty() || consumers.rows.is_empty() {
        return Err(error("scene message 선행 glyph/consumer 계약이 비어 있음"));
    }

    let mut rows = Vec::new();
    let mut family_keys = BTreeSet::new();
    let mut global_physical = BTreeSet::new();
    let mut unique_message_bytes = 0usize;

    for glyph in &glyphs.rows {
        family_keys.insert(glyph.sequence);
        let raw = source_map
            .get(&(glyph.seq_archive, glyph.sequence))
            .copied()
            .ok_or_else(|| {
                error(format!(
                    "scene message 원본 누락: {}:{}",
                    glyph.seq_archive, glyph.sequence
                ))
            })?;
        let sequence_sha256: [u8; 32] = Sha256::digest(raw).into();
        if sequence_sha256 != glyph.sequence_sha256 {
            return Err(error(format!(
                "{}:{} scene message 원본 digest 불일치",
                glyph.seq_archive, glyph.sequence
            )));
        }
        let layout = layouts
            .rows
            .iter()
            .find(|row| row.archive == glyph.seq_archive && row.sequence == glyph.sequence)
            .ok_or_else(|| error("scene message overlay layout 누락"))?;
        let footer = layout
            .footer
            .ok_or_else(|| error("scene message overlay footer 누락"))?;
        let runtime_map = footer
            .runtime_map()
            .ok_or_else(|| error("scene message runtime 사상 overflow"))?;
        let initialized_len = usize::try_from(footer.initialized_len)
            .map_err(|_| error("scene message initialized 길이 변환 실패"))?;
        if initialized_len > raw.len() {
            return Err(error("scene message initialized 범위가 원본 밖임"));
        }
        let consumer = consumers
            .rows
            .iter()
            .find(|row| row.scope == glyph.scope && row.exe == glyph.exe)
            .ok_or_else(|| error(format!("{} scene message consumer 누락", glyph.scope)))?;
        if consumer.dispatch_first_token != BOX_SUBGAME_SEQ_DRAC_TOKEN_FIRST
            || consumer.argument_tokens != BOX_SUBGAME_SEQ_DRAC_ARGUMENT_TOKENS
            || !matches!(
                (glyph.scope, consumer.dispatch_last_token),
                ("PUYO1", 0x78) | ("PUYO2", 0x77)
            )
        {
            return Err(error(format!(
                "{} scene message token 소비 계약 불일치",
                glyph.scope
            )));
        }

        let canonical_end = glyph
            .repertoire_offset
            .checked_add(glyph.canonical_glyph_slots * 2)
            .ok_or_else(|| error("scene canonical repertoire 끝 overflow"))?;
        let canonical_raw = raw
            .get(glyph.repertoire_offset..canonical_end)
            .ok_or_else(|| error("scene canonical repertoire 범위 이탈"))?;
        let canonical_pairs = canonical_raw
            .chunks_exact(2)
            .map(|pair| [pair[0], pair[1]])
            .collect::<Vec<_>>();
        if canonical_pairs.len() != glyph.canonical_glyph_slots
            || canonical_pairs
                .iter()
                .any(|pair| !decode_sjis(pair).is_ok_and(|decoded| decoded.chars().count() == 1))
        {
            return Err(error(format!(
                "{}:{} canonical repertoire가 엄격한 2바이트 glyph가 아님",
                glyph.scope, glyph.sequence
            )));
        }

        let mut pending = Vec::new();
        for command_offset in (0..=initialized_len.saturating_sub(8)).step_by(4) {
            let record = &raw[command_offset..command_offset + 8];
            if record[0] != 0xE0 || record[1] > 1 || record[2..4] != [0, 0] {
                continue;
            }
            let pointer_field_offset = command_offset + 4;
            let record_runtime = read_u32(raw, pointer_field_offset)?;
            let record_offset = runtime_map.file_offset(record_runtime).ok_or_else(|| {
                error(format!(
                    "{}:{} E0 target이 overlay 밖임: command=+0x{command_offset:X}, target=0x{record_runtime:08X}",
                    glyph.scope, glyph.sequence
                ))
            })?;
            let parsed = parse_message(
                raw,
                record_offset,
                &canonical_pairs,
                consumer.dispatch_first_token,
                consumer.dispatch_last_token,
                &consumer.argument_tokens,
            )?;
            if parsed.end_offset > initialized_len {
                return Err(error(format!(
                    "{}:{} E0 message가 initialized 범위를 넘음",
                    glyph.scope, glyph.sequence
                )));
            }
            pending.push((
                command_offset,
                pointer_field_offset,
                record_runtime,
                record_offset,
                record[1],
                parsed,
            ));
        }
        if pending.is_empty() {
            return Err(error(format!(
                "{}:{} 정렬 E0 message가 없음",
                glyph.scope, glyph.sequence
            )));
        }

        let mut target_refs = BTreeMap::<usize, Vec<usize>>::new();
        let mut target_ends = BTreeMap::<usize, usize>::new();
        for (_, pointer_field, _, target, _, parsed) in &pending {
            target_refs.entry(*target).or_default().push(*pointer_field);
            if target_ends
                .insert(*target, parsed.end_offset)
                .is_some_and(|end| end != parsed.end_offset)
            {
                return Err(error("같은 scene message target의 종결이 서로 다름"));
            }
        }
        let physical_ranges = target_ends.iter().collect::<Vec<_>>();
        for pair in physical_ranges.windows(2) {
            let (&left_start, &left_end) = pair[0];
            let (&right_start, _) = pair[1];
            if left_end > right_start {
                return Err(error(format!(
                    "{}:{} scene message physical range 겹침: +0x{left_start:X}..+0x{left_end:X}, +0x{right_start:X}",
                    glyph.scope, glyph.sequence
                )));
            }
        }
        for (&target, expected_fields) in &target_refs {
            let pointer = runtime_map
                .runtime_addr(target)
                .ok_or_else(|| error("scene message target runtime 역변환 실패"))?;
            let pointer_bytes = pointer.to_le_bytes();
            let actual_fields = raw[..initialized_len]
                .windows(4)
                .enumerate()
                .filter_map(|(offset, bytes)| (bytes == pointer_bytes).then_some(offset))
                .collect::<Vec<_>>();
            if &actual_fields != expected_fields {
                return Err(error(format!(
                    "{}:{} scene message direct pointer 전수 불일치: target=+0x{target:X}, expected={expected_fields:X?}, actual={actual_fields:X?}",
                    glyph.scope, glyph.sequence
                )));
            }
        }

        let first_row = rows.len();
        for (index, (command, pointer_field, record_runtime, record_offset, variant, parsed)) in
            pending.into_iter().enumerate()
        {
            let pointer_field_runtime = runtime_map
                .runtime_addr(pointer_field)
                .ok_or_else(|| error("scene message pointer field runtime 변환 실패"))?;
            let maximum_glyph_cell = parsed.glyph_cells.iter().next_back().copied().unwrap_or(0);
            rows.push(SubgameSeqSceneMessageRow {
                scope: glyph.scope,
                exe: glyph.exe,
                archive: glyph.seq_archive,
                sequence: glyph.sequence,
                sequence_sha256: glyph.sequence_sha256,
                index: index + 1,
                command_offset: command,
                pointer_field_offset: pointer_field,
                pointer_field_runtime,
                record_offset,
                record_runtime,
                record_end_offset: parsed.end_offset,
                variant,
                header: parsed.header,
                raw_sha256: Sha256::digest(&parsed.raw).into(),
                raw: parsed.raw,
                payload_copy_count: 0,
                counterpart_scope: None,
                counterpart_index: None,
                glyph_tokens: parsed.glyph_tokens,
                control_tokens: parsed.control_tokens,
                argument_controls: parsed.argument_controls,
                unique_glyph_cells: parsed.glyph_cells.len(),
                maximum_glyph_cell,
                control_counts: parsed.control_counts,
                source_text: parsed.source_text,
            });
        }

        let family_rows = &rows[first_row..];
        if glyph.compound_message_records != 0 {
            let actual = (
                family_rows.len(),
                family_rows
                    .iter()
                    .map(|row| row.glyph_tokens)
                    .sum::<usize>(),
                family_rows
                    .iter()
                    .map(|row| row.control_tokens)
                    .sum::<usize>(),
                family_rows.iter().filter(|row| row.variant == 1).count(),
            );
            let expected = (
                glyph.compound_message_records,
                glyph.compound_glyph_tokens,
                glyph.compound_control_tokens,
                glyph.compound_variant_records,
            );
            if actual != expected {
                return Err(error(format!(
                    "{}:{} compound/full E0 교차검산 불일치: expected={expected:?}, actual={actual:?}",
                    glyph.scope, glyph.sequence
                )));
            }
        }
        for (&target, &end) in &target_ends {
            if global_physical.insert((glyph.scope, glyph.sequence, target)) {
                unique_message_bytes += end - target;
            }
        }
    }

    let physical_messages = global_physical.len();
    let mut payload_groups = BTreeMap::<[u8; 32], Vec<usize>>::new();
    for (index, row) in rows.iter().enumerate() {
        payload_groups
            .entry(row.raw_sha256)
            .or_default()
            .push(index);
    }
    let mut cross_scope_pairs = 0usize;
    for indexes in payload_groups.values() {
        match indexes.as_slice() {
            [index] => {
                rows[*index].payload_copy_count = 1;
            }
            [left, right] => {
                let left_scope = rows[*left].scope;
                let left_sequence = rows[*left].sequence;
                let left_index = rows[*left].index;
                let right_scope = rows[*right].scope;
                let right_sequence = rows[*right].sequence;
                let right_index = rows[*right].index;
                if left_scope == right_scope || left_sequence != right_sequence {
                    return Err(error(format!(
                        "scene message 동일 payload가 같은 scope 또는 다른 family에 걸침: {left_scope}:{left_sequence}/{right_scope}:{right_sequence}"
                    )));
                }
                rows[*left].payload_copy_count = 2;
                rows[*left].counterpart_scope = Some(right_scope);
                rows[*left].counterpart_index = Some(right_index);
                rows[*right].payload_copy_count = 2;
                rows[*right].counterpart_scope = Some(left_scope);
                rows[*right].counterpart_index = Some(left_index);
                cross_scope_pairs += 1;
            }
            _ => {
                return Err(error(format!(
                    "scene message 동일 payload 사본이 2개를 넘음: {}",
                    indexes.len()
                )));
            }
        }
    }
    let unique_payloads = payload_groups.len();
    let mut control_counts = BTreeMap::new();
    for row in &rows {
        for (&code, &count) in &row.control_counts {
            *control_counts.entry(code).or_default() += count;
        }
    }
    Ok(SubgameSeqSceneMessageAnalysis {
        families: family_keys.len(),
        physical_messages,
        glyph_tokens: rows.iter().map(|row| row.glyph_tokens).sum(),
        control_tokens: rows.iter().map(|row| row.control_tokens).sum(),
        argument_controls: rows.iter().map(|row| row.argument_controls).sum(),
        terminators: rows.len(),
        variant_records: rows.iter().filter(|row| row.variant == 1).count(),
        direct_pointers: rows.len(),
        referenced_message_bytes: rows.iter().map(|row| row.raw.len()).sum(),
        unique_message_bytes,
        unique_payloads,
        cross_scope_pairs,
        control_counts,
        rows,
    })
}

/// 지원 원본의 34개 scene 사본 전체 E0 message 분모를 고정한다.
pub fn verify_box_subgame_seq_scene_messages<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    layouts: &SubgameSeqLayoutAnalysis,
    glyphs: &SubgameSeqSceneGlyphAnalysis,
    consumers: &SubgameSeqDracConsumerAnalysis,
) -> Result<SubgameSeqSceneMessageAnalysis, SubgameSeqSceneMessageError> {
    let analysis = audit_subgame_seq_scene_messages(sources, layouts, glyphs, consumers)?;
    let actual = [
        analysis.rows.len(),
        analysis.physical_messages,
        analysis.glyph_tokens,
        analysis.control_tokens,
        analysis.argument_controls,
        analysis.terminators,
        analysis.variant_records,
        analysis.direct_pointers,
        analysis.referenced_message_bytes,
        analysis.unique_message_bytes,
        analysis.unique_payloads,
        analysis.cross_scope_pairs,
    ];
    let expected = [
        BOX_SUBGAME_SEQ_SCENE_MESSAGE_ROWS,
        BOX_SUBGAME_SEQ_SCENE_PHYSICAL_MESSAGES,
        BOX_SUBGAME_SEQ_SCENE_MESSAGE_GLYPH_TOKENS,
        BOX_SUBGAME_SEQ_SCENE_MESSAGE_CONTROL_TOKENS,
        BOX_SUBGAME_SEQ_SCENE_MESSAGE_ARGUMENT_CONTROLS,
        BOX_SUBGAME_SEQ_SCENE_MESSAGE_TERMINATORS,
        BOX_SUBGAME_SEQ_SCENE_MESSAGE_VARIANT_RECORDS,
        BOX_SUBGAME_SEQ_SCENE_MESSAGE_DIRECT_POINTERS,
        BOX_SUBGAME_SEQ_SCENE_MESSAGE_REFERENCED_BYTES,
        BOX_SUBGAME_SEQ_SCENE_MESSAGE_UNIQUE_BYTES,
        BOX_SUBGAME_SEQ_SCENE_MESSAGE_UNIQUE_PAYLOADS,
        BOX_SUBGAME_SEQ_SCENE_MESSAGE_CROSS_SCOPE_PAIRS,
    ];
    let expected_control_counts = BOX_SUBGAME_SEQ_SCENE_MESSAGE_CONTROL_COUNTS
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    if actual != expected
        || analysis.families != BOX_SUBGAME_SEQ_SCENE_MESSAGE_FAMILIES
        || analysis.control_counts != expected_control_counts
    {
        return Err(error(format!(
            "scene message 분모 불일치: expected={expected:?}/{}/{expected_control_counts:02X?}, actual={actual:?}/{}/{:02X?}",
            BOX_SUBGAME_SEQ_SCENE_MESSAGE_FAMILIES,
            analysis.families,
            analysis.control_counts,
        )));
    }
    let report = render_subgame_seq_scene_messages_tsv(&analysis);
    let digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if digest != BOX_SUBGAME_SEQ_SCENE_MESSAGE_REPORT_SHA256 {
        return Err(error(format!(
            "scene message report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_SCENE_MESSAGE_REPORT_SHA256),
            hex(&digest)
        )));
    }
    Ok(analysis)
}

pub fn render_subgame_seq_scene_messages_tsv(analysis: &SubgameSeqSceneMessageAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tscope\texe\tarchive\tsequence\tsequence_sha256\tindex\t\
         command_offset\tpointer_field_offset\tpointer_field_runtime\trecord_offset\t\
         record_runtime\trecord_end_offset\tvariant\theader\traw_sha256\tpayload_copy_count\t\
         counterpart\traw_bytes\t\
         glyph_tokens\tcontrol_tokens\targument_controls\tunique_glyph_cells\t\
         maximum_glyph_cell\tcontrol_counts\ttoken_policy\tconsumer_contract\t\
         runtime_verdict\tpatch_strategy\tsource_text\n",
    );
    for row in &analysis.rows {
        let control_counts = row
            .control_counts
            .iter()
            .map(|(code, count)| format!("{code:02X}:{count}"))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t0x{:04X}\t0x{:08X}\t\
             0x{:04X}\t0x{:08X}\t0x{:04X}\t{}\t{:02X}{:02X}{:02X}{:02X}\t{}\t{}\t{}\t{}\t\
             {}\t{}\t{}\t{}\t{}\t{}\t63/78=forbidden-1-byte;71/74/77=preserve-arg1;6565=preserve-terminator\t\
             shared-e0-header-token-consumer-static\tunverified\t\
             preserve-no-write-until-relink-layout-and-runtime-contract\t{}",
            SUBGAME_SEQ_SCENE_MESSAGE_SCHEMA_VERSION,
            row.scope,
            row.exe,
            row.archive,
            row.sequence,
            hex(&row.sequence_sha256),
            row.index,
            row.command_offset,
            row.pointer_field_offset,
            row.pointer_field_runtime,
            row.record_offset,
            row.record_runtime,
            row.record_end_offset,
            row.variant,
            row.header[0],
            row.header[1],
            row.header[2],
            row.header[3],
            hex(&row.raw_sha256),
            row.payload_copy_count,
            row.counterpart_scope
                .zip(row.counterpart_index)
                .map_or_else(String::new, |(scope, index)| format!("{scope}:{index}")),
            row.raw.len(),
            row.glyph_tokens,
            row.control_tokens,
            row.argument_controls,
            row.unique_glyph_cells,
            row.maximum_glyph_cell,
            control_counts,
            escape_tsv(&row.source_text),
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn escape_tsv(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_scene_messages_tests.rs"]
mod subgame_seq_scene_messages_tests;
