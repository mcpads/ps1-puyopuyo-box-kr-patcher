//! scene-local E0 message의 bounded block 재포장·pointer relink.
//!
//! 인접 message 사이의 exact zero gap만 같은 block의 slack으로 인정한다.
//! 마지막 record 뒤의 반복 zero는 소유권이 없으므로 용량에 넣지 않는다.
//! block 내부를 가리키는 모든 u32를 byte 단위로 전수 검사하고, E0 pointer
//! field 이외의 참조가 있으면 실패한다. replacement는 header와 control
//! 순서·인자를 원본 그대로 보존하고 canonical repertoire pair만 쓸 수 있다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::analysis::{
    SubgameSeqSceneGlyphAnalysis, SubgameSeqSceneMessageAnalysis, SubgameSeqSceneMessageRow,
};
use crate::coords::RuntimeAssetMap;
use crate::script::SubgameSeqLayoutAnalysis;

pub const SUBGAME_SEQ_SCENE_RELINK_SCHEMA_VERSION: &str = "subgame-seq-scene-relink-v1";
pub const BOX_SUBGAME_SEQ_SCENE_RELINK_ROWS: usize = 34;
pub const BOX_SUBGAME_SEQ_SCENE_RELINK_BLOCKS: usize = 35;
pub const BOX_SUBGAME_SEQ_SCENE_RELINK_MESSAGES: usize = 194;
pub const BOX_SUBGAME_SEQ_SCENE_RELINK_CAPACITY_BYTES: usize = 7_526;
pub const BOX_SUBGAME_SEQ_SCENE_RELINK_ORIGINAL_BYTES: usize = 7_526;
pub const BOX_SUBGAME_SEQ_SCENE_RELINK_SLACK_BYTES: usize = 0;
pub const BOX_SUBGAME_SEQ_SCENE_RELINK_ZERO_SLACK_BLOCKS: usize = 35;

/// 지원 원본에서 만든 block/relink TSV의 SHA-256.
pub const BOX_SUBGAME_SEQ_SCENE_RELINK_REPORT_SHA256: [u8; 32] = [
    0x1A, 0x4E, 0xA8, 0xB3, 0xBF, 0x23, 0xE7, 0xE9, 0x9A, 0x6C, 0x50, 0x7B, 0x85, 0xB3, 0x8B, 0xE9,
    0xE5, 0x68, 0xB3, 0xF8, 0x3A, 0xA9, 0xFA, 0x73, 0x21, 0x0D, 0x2D, 0x92, 0x1C, 0x1D, 0xCC, 0x4C,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneRelinkPointer {
    pub field_offset: usize,
    pub field_runtime: u32,
    pub target_offset: usize,
    pub target_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneRelinkBlock {
    pub index: usize,
    pub start_offset: usize,
    pub end_offset: usize,
    pub capacity_bytes: usize,
    pub original_message_bytes: usize,
    pub slack_bytes: usize,
    pub message_ids: Vec<String>,
    pub original_offsets: Vec<usize>,
    pub pointers: Vec<SubgameSeqSceneRelinkPointer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneRelinkRow {
    pub scope: &'static str,
    pub exe: &'static str,
    pub archive: &'static str,
    pub sequence: &'static str,
    pub sequence_sha256: [u8; 32],
    pub load_base: u32,
    pub initialized_bytes: usize,
    pub messages: usize,
    pub blocks: Vec<SubgameSeqSceneRelinkBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneRelinkAnalysis {
    pub rows: Vec<SubgameSeqSceneRelinkRow>,
    pub blocks: usize,
    pub messages: usize,
    pub capacity_bytes: usize,
    pub original_message_bytes: usize,
    pub slack_bytes: usize,
    pub zero_slack_blocks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneReplacement {
    pub message_id: String,
    pub raw: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneRepack {
    pub scope: &'static str,
    pub sequence_name: &'static str,
    pub sequence: Vec<u8>,
    pub record_offsets: BTreeMap<String, usize>,
    pub used_bytes: usize,
    pub remaining_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneRelinkError(String);

impl fmt::Display for SubgameSeqSceneRelinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqSceneRelinkError {}

fn error(message: impl Into<String>) -> SubgameSeqSceneRelinkError {
    SubgameSeqSceneRelinkError(message.into())
}

fn message_id(row: &SubgameSeqSceneMessageRow) -> String {
    format!(
        "{}:{}:0x{:04X}",
        row.scope, row.sequence, row.command_offset
    )
}

fn read_u32(raw: &[u8], offset: usize) -> Option<u32> {
    raw.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
}

fn control_signature(
    record: &[u8],
    canonical_pairs: &BTreeSet<[u8; 2]>,
    dispatch_last: u8,
) -> Result<Vec<Vec<u8>>, SubgameSeqSceneRelinkError> {
    if record.len() < 6 {
        return Err(error("scene relink record가 header와 종결보다 짧음"));
    }
    let mut controls = Vec::new();
    let mut cursor = 4usize;
    loop {
        if record.get(cursor..cursor + 2) == Some([0x65, 0x65].as_slice()) {
            if cursor + 2 != record.len() {
                return Err(error("scene relink record 종결 뒤 trailing byte가 있음"));
            }
            return Ok(controls);
        }
        let first = *record
            .get(cursor)
            .ok_or_else(|| error("scene relink record 종결 누락"))?;
        if matches!(first, 0x81..=0x9F | 0xE0..=0xEF) {
            let pair: [u8; 2] = record
                .get(cursor..cursor + 2)
                .ok_or_else(|| error("scene relink glyph pair 잘림"))?
                .try_into()
                .expect("2바이트 pair");
            if !canonical_pairs.contains(&pair) {
                return Err(error(format!(
                    "scene relink glyph가 canonical repertoire에 없음: {:02X}{:02X}",
                    pair[0], pair[1]
                )));
            }
            cursor += 2;
            continue;
        }
        if !(0x63..=dispatch_last).contains(&first) {
            return Err(error(format!("scene relink 미지원 token: 0x{first:02X}")));
        }
        if matches!(first, 0x71 | 0x74 | 0x77) {
            let argument = *record
                .get(cursor + 1)
                .ok_or_else(|| error("scene relink argument control 잘림"))?;
            controls.push(vec![first, argument]);
            cursor += 2;
        } else {
            controls.push(vec![first]);
            cursor += 1;
        }
    }
}

fn scan_block_pointers(
    raw: &[u8],
    initialized_len: usize,
    map: RuntimeAssetMap,
    start: usize,
    end: usize,
) -> Result<Vec<SubgameSeqSceneRelinkPointer>, SubgameSeqSceneRelinkError> {
    let scan_end = initialized_len
        .checked_sub(3)
        .ok_or_else(|| error("scene relink initialized 영역이 pointer보다 짧음"))?;
    let mut pointers = Vec::new();
    for field_offset in 0..scan_end {
        let value = read_u32(raw, field_offset)
            .ok_or_else(|| error("scene relink pointer scan 범위 이탈"))?;
        let Some(target_offset) = map.file_offset(value) else {
            continue;
        };
        if !(start..end).contains(&target_offset) {
            continue;
        }
        pointers.push(SubgameSeqSceneRelinkPointer {
            field_offset,
            field_runtime: map
                .runtime_addr(field_offset)
                .ok_or_else(|| error("scene relink pointer field runtime 변환 실패"))?,
            target_offset,
            target_runtime: value,
        });
    }
    Ok(pointers)
}

fn discover_blocks(
    raw: &[u8],
    initialized_len: usize,
    map: RuntimeAssetMap,
    messages: &[&SubgameSeqSceneMessageRow],
) -> Result<Vec<SubgameSeqSceneRelinkBlock>, SubgameSeqSceneRelinkError> {
    if messages.is_empty() {
        return Err(error("scene relink message 행이 없음"));
    }
    let mut by_offset = messages.to_vec();
    by_offset.sort_by_key(|row| row.record_offset);
    let mut groups = Vec::<Vec<&SubgameSeqSceneMessageRow>>::new();
    for message in by_offset {
        let merge = groups.last().is_some_and(|group| {
            let previous = group.last().expect("비어 있지 않은 block");
            previous.record_end_offset <= message.record_offset
                && raw[previous.record_end_offset..message.record_offset]
                    .iter()
                    .all(|&byte| byte == 0)
        });
        if merge {
            groups
                .last_mut()
                .expect("비어 있지 않은 block")
                .push(message);
        } else {
            groups.push(vec![message]);
        }
    }

    let mut blocks = Vec::with_capacity(groups.len());
    for (index, group) in groups.into_iter().enumerate() {
        let start = group.first().expect("비어 있지 않은 block").record_offset;
        let end = group
            .last()
            .expect("비어 있지 않은 block")
            .record_end_offset;
        let original_message_bytes = group.iter().map(|row| row.raw.len()).sum::<usize>();
        let capacity_bytes = end - start;
        let slack_bytes = capacity_bytes
            .checked_sub(original_message_bytes)
            .ok_or_else(|| error("scene relink block slack underflow"))?;
        let pointers = scan_block_pointers(raw, initialized_len, map, start, end)?;
        let expected = group
            .iter()
            .map(|row| (row.pointer_field_offset, row.record_offset))
            .collect::<Vec<_>>();
        let actual = pointers
            .iter()
            .map(|pointer| (pointer.field_offset, pointer.target_offset))
            .collect::<Vec<_>>();
        if actual != expected {
            return Err(error(format!(
                "scene relink block incoming pointer 전수 불일치: +0x{start:X}..+0x{end:X}, expected={expected:X?}, actual={actual:X?}"
            )));
        }
        if pointers
            .iter()
            .any(|pointer| (start..end).contains(&pointer.field_offset))
        {
            return Err(error("scene relink block 안에 자기 pointer field가 있음"));
        }
        blocks.push(SubgameSeqSceneRelinkBlock {
            index: index + 1,
            start_offset: start,
            end_offset: end,
            capacity_bytes,
            original_message_bytes,
            slack_bytes,
            message_ids: group.iter().map(|row| message_id(row)).collect(),
            original_offsets: group.iter().map(|row| row.record_offset).collect(),
            pointers,
        });
    }
    Ok(blocks)
}

pub fn audit_subgame_seq_scene_relinks<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    layouts: &SubgameSeqLayoutAnalysis,
    messages: &SubgameSeqSceneMessageAnalysis,
) -> Result<SubgameSeqSceneRelinkAnalysis, SubgameSeqSceneRelinkError> {
    let mut source_map = BTreeMap::new();
    for (archive, sequence, raw) in sources {
        if source_map.insert((archive, sequence), raw).is_some() {
            return Err(error("scene relink source 입력 중복"));
        }
    }
    let keys = messages
        .rows
        .iter()
        .map(|row| (row.scope, row.archive, row.sequence))
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::with_capacity(keys.len());
    for (scope, archive, sequence) in keys {
        let raw = source_map
            .get(&(archive, sequence))
            .copied()
            .ok_or_else(|| error(format!("{scope}:{sequence} scene relink source 누락")))?;
        let layout = layouts
            .rows
            .iter()
            .find(|row| row.archive == archive && row.sequence == sequence)
            .ok_or_else(|| error("scene relink layout 누락"))?;
        let footer = layout
            .footer
            .ok_or_else(|| error("scene relink footer 누락"))?;
        let initialized_len = usize::try_from(footer.initialized_len)
            .map_err(|_| error("scene relink initialized 길이 변환 실패"))?;
        let map = footer
            .runtime_map()
            .ok_or_else(|| error("scene relink runtime 사상 overflow"))?;
        let mut scene_messages = messages
            .rows
            .iter()
            .filter(|row| row.scope == scope && row.archive == archive && row.sequence == sequence)
            .collect::<Vec<_>>();
        scene_messages.sort_by_key(|row| row.pointer_field_offset);
        let exe = scene_messages
            .first()
            .map(|row| row.exe)
            .ok_or_else(|| error("scene relink message 행 누락"))?;
        let sequence_sha256: [u8; 32] = Sha256::digest(raw).into();
        if scene_messages
            .iter()
            .any(|row| row.exe != exe || row.sequence_sha256 != sequence_sha256)
        {
            return Err(error("scene relink message/source 계약 불일치"));
        }
        let blocks = discover_blocks(raw, initialized_len, map, &scene_messages)?;
        rows.push(SubgameSeqSceneRelinkRow {
            scope,
            exe,
            archive,
            sequence,
            sequence_sha256,
            load_base: footer.load_base,
            initialized_bytes: initialized_len,
            messages: scene_messages.len(),
            blocks,
        });
    }
    rows.sort_by_key(|row| (row.scope, row.sequence));
    let blocks = rows.iter().map(|row| row.blocks.len()).sum();
    let capacity_bytes = rows
        .iter()
        .flat_map(|row| &row.blocks)
        .map(|block| block.capacity_bytes)
        .sum();
    let original_message_bytes = rows
        .iter()
        .flat_map(|row| &row.blocks)
        .map(|block| block.original_message_bytes)
        .sum();
    let slack_bytes = rows
        .iter()
        .flat_map(|row| &row.blocks)
        .map(|block| block.slack_bytes)
        .sum();
    let zero_slack_blocks = rows
        .iter()
        .flat_map(|row| &row.blocks)
        .filter(|block| block.slack_bytes == 0)
        .count();
    Ok(SubgameSeqSceneRelinkAnalysis {
        rows,
        blocks,
        messages: messages.rows.len(),
        capacity_bytes,
        original_message_bytes,
        slack_bytes,
        zero_slack_blocks,
    })
}

pub fn verify_box_subgame_seq_scene_relinks<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    layouts: &SubgameSeqLayoutAnalysis,
    messages: &SubgameSeqSceneMessageAnalysis,
    glyphs: &SubgameSeqSceneGlyphAnalysis,
) -> Result<SubgameSeqSceneRelinkAnalysis, SubgameSeqSceneRelinkError> {
    let sources = sources.into_iter().collect::<Vec<_>>();
    let analysis = audit_subgame_seq_scene_relinks(sources.iter().copied(), layouts, messages)?;
    let actual = [
        analysis.rows.len(),
        analysis.blocks,
        analysis.messages,
        analysis.capacity_bytes,
        analysis.original_message_bytes,
        analysis.slack_bytes,
    ];
    let expected = [
        BOX_SUBGAME_SEQ_SCENE_RELINK_ROWS,
        BOX_SUBGAME_SEQ_SCENE_RELINK_BLOCKS,
        BOX_SUBGAME_SEQ_SCENE_RELINK_MESSAGES,
        BOX_SUBGAME_SEQ_SCENE_RELINK_CAPACITY_BYTES,
        BOX_SUBGAME_SEQ_SCENE_RELINK_ORIGINAL_BYTES,
        BOX_SUBGAME_SEQ_SCENE_RELINK_SLACK_BYTES,
    ];
    if actual != expected
        || analysis.zero_slack_blocks != BOX_SUBGAME_SEQ_SCENE_RELINK_ZERO_SLACK_BLOCKS
    {
        return Err(error(format!(
            "scene relink 분모 불일치: expected={expected:?}, actual={actual:?}, zero_slack_blocks={}",
            analysis.zero_slack_blocks
        )));
    }
    let report = render_subgame_seq_scene_relinks_tsv(&analysis);
    let digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if digest != BOX_SUBGAME_SEQ_SCENE_RELINK_REPORT_SHA256 {
        return Err(error(format!(
            "scene relink report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_SCENE_RELINK_REPORT_SHA256),
            hex(&digest)
        )));
    }
    for row in &analysis.rows {
        let raw = sources
            .iter()
            .find(|(archive, sequence, _)| *archive == row.archive && *sequence == row.sequence)
            .map(|(_, _, raw)| *raw)
            .ok_or_else(|| error("scene relink no-op source 누락"))?;
        let replacements = messages
            .rows
            .iter()
            .filter(|message| message.scope == row.scope && message.sequence == row.sequence)
            .map(|message| SubgameSeqSceneReplacement {
                message_id: message_id(message),
                raw: message.raw.clone(),
            })
            .collect::<Vec<_>>();
        let repack = repack_box_subgame_seq_scene_messages(
            row.scope,
            row.sequence,
            raw,
            &replacements,
            messages,
            glyphs,
            &analysis,
        )?;
        if repack.sequence != raw
            || repack.used_bytes
                != row
                    .blocks
                    .iter()
                    .map(|block| block.original_message_bytes)
                    .sum::<usize>()
            || repack.remaining_bytes
                != row
                    .blocks
                    .iter()
                    .map(|block| block.slack_bytes)
                    .sum::<usize>()
        {
            return Err(error(format!(
                "{}:{} scene relink 원문 no-op 불일치",
                row.scope, row.sequence
            )));
        }
    }
    Ok(analysis)
}

/// 한 scene 사본의 message를 검증된 bounded block 안에서 다시 이어 붙인다.
pub fn repack_box_subgame_seq_scene_messages(
    scope: &str,
    sequence_name: &str,
    raw: &[u8],
    replacements: &[SubgameSeqSceneReplacement],
    messages: &SubgameSeqSceneMessageAnalysis,
    glyphs: &SubgameSeqSceneGlyphAnalysis,
    relinks: &SubgameSeqSceneRelinkAnalysis,
) -> Result<SubgameSeqSceneRepack, SubgameSeqSceneRelinkError> {
    let relink_rows = relinks
        .rows
        .iter()
        .filter(|row| row.scope == scope && row.sequence == sequence_name)
        .collect::<Vec<_>>();
    let [relink] = relink_rows.as_slice() else {
        return Err(error("scene repack relink 행 수가 1이 아님"));
    };
    if <[u8; 32]>::from(Sha256::digest(raw)) != relink.sequence_sha256 {
        return Err(error("scene repack 원본 SHA-256 불일치"));
    }
    if relink.initialized_bytes > raw.len() {
        return Err(error("scene repack initialized 영역이 원본 밖임"));
    }
    let glyph_rows = glyphs
        .rows
        .iter()
        .filter(|row| row.scope == scope && row.sequence == sequence_name)
        .collect::<Vec<_>>();
    let [glyph] = glyph_rows.as_slice() else {
        return Err(error("scene repack glyph 행 수가 1이 아님"));
    };
    let mut source_messages = messages
        .rows
        .iter()
        .filter(|row| row.scope == scope && row.sequence == sequence_name)
        .collect::<Vec<_>>();
    source_messages.sort_by_key(|row| row.pointer_field_offset);
    for source in &source_messages {
        let source_raw = raw
            .get(source.record_offset..source.record_end_offset)
            .ok_or_else(|| error("scene repack 원문 record 범위 이탈"))?;
        if source_raw != source.raw {
            return Err(error(format!(
                "scene repack 원문 record 불일치: {}",
                message_id(source)
            )));
        }
        if read_u32(raw, source.pointer_field_offset) != Some(source.record_runtime) {
            return Err(error(format!(
                "scene repack 원문 E0 pointer 불일치: {}",
                message_id(source)
            )));
        }
    }
    let source_by_id = source_messages
        .iter()
        .map(|row| (message_id(row), *row))
        .collect::<BTreeMap<_, _>>();
    let replacement_map = replacements
        .iter()
        .map(|replacement| (replacement.message_id.as_str(), replacement.raw.as_slice()))
        .collect::<BTreeMap<_, _>>();
    if replacement_map.len() != replacements.len()
        || replacement_map.len() != source_by_id.len()
        || replacement_map.keys().copied().collect::<Vec<_>>()
            != source_by_id.keys().map(String::as_str).collect::<Vec<_>>()
    {
        return Err(error("scene repack replacement ID 분모 불일치"));
    }
    let canonical_end = glyph.repertoire_offset + glyph.canonical_glyph_slots * 2;
    let canonical_pairs = raw
        .get(glyph.repertoire_offset..canonical_end)
        .ok_or_else(|| error("scene repack canonical repertoire 범위 이탈"))?
        .chunks_exact(2)
        .map(|pair| [pair[0], pair[1]])
        .collect::<BTreeSet<_>>();
    let dispatch_last = if scope == "PUYO1" { 0x78 } else { 0x77 };
    let mut output = raw.to_vec();
    let runtime_map = RuntimeAssetMap::new(
        relink.load_base,
        u32::try_from(relink.initialized_bytes)
            .map_err(|_| error("scene repack initialized 길이가 u32를 넘음"))?,
    )
    .ok_or_else(|| error("scene repack runtime 사상 overflow"))?;
    let mut record_offsets = BTreeMap::new();
    let mut used_bytes = 0usize;
    let mut remaining_bytes = 0usize;

    for block in &relink.blocks {
        let mut cursor = block.start_offset;
        let mut block_records = Vec::with_capacity(block.message_ids.len());
        for id in &block.message_ids {
            let source = source_by_id
                .get(id)
                .copied()
                .ok_or_else(|| error("scene repack source message ID 누락"))?;
            let replacement = replacement_map
                .get(id.as_str())
                .copied()
                .ok_or_else(|| error("scene repack replacement message ID 누락"))?;
            if replacement.get(..4) != source.raw.get(..4) {
                return Err(error(format!("scene repack header 변경: {id}")));
            }
            let original_signature =
                control_signature(&source.raw, &canonical_pairs, dispatch_last)?;
            let replacement_signature =
                control_signature(replacement, &canonical_pairs, dispatch_last)?;
            if original_signature != replacement_signature {
                return Err(error(format!("scene repack control sequence 변경: {id}")));
            }
            let end = cursor
                .checked_add(replacement.len())
                .ok_or_else(|| error("scene repack record 끝 overflow"))?;
            if end > block.end_offset {
                return Err(error(format!(
                    "scene repack block capacity 초과: block={}, used={}, capacity={}",
                    block.index,
                    end - block.start_offset,
                    block.capacity_bytes
                )));
            }
            block_records.push((id, source, replacement, cursor, end));
            cursor = end;
        }
        output[block.start_offset..block.end_offset].fill(0);
        for (id, source, replacement, start, end) in block_records {
            output[start..end].copy_from_slice(replacement);
            let target = runtime_map
                .runtime_addr(start)
                .ok_or_else(|| error("scene repack target runtime 변환 실패"))?;
            output[source.pointer_field_offset..source.pointer_field_offset + 4]
                .copy_from_slice(&target.to_le_bytes());
            record_offsets.insert(id.clone(), start);
            used_bytes += replacement.len();
        }
        remaining_bytes += block.end_offset - cursor;

        let mut expected_pointers = block
            .message_ids
            .iter()
            .map(|id| {
                let source = source_by_id
                    .get(id)
                    .copied()
                    .ok_or_else(|| error("scene repack post-check source ID 누락"))?;
                let target_offset = *record_offsets
                    .get(id)
                    .ok_or_else(|| error("scene repack post-check record offset 누락"))?;
                Ok((source.pointer_field_offset, target_offset))
            })
            .collect::<Result<Vec<_>, SubgameSeqSceneRelinkError>>()?;
        expected_pointers.sort_unstable();
        let actual_pointers = scan_block_pointers(
            &output,
            relink.initialized_bytes,
            runtime_map,
            block.start_offset,
            block.end_offset,
        )?
        .into_iter()
        .map(|pointer| (pointer.field_offset, pointer.target_offset))
        .collect::<Vec<_>>();
        if actual_pointers != expected_pointers {
            return Err(error(format!(
                "scene repack 이후 incoming pointer 전수 불일치: block={}, expected={expected_pointers:X?}, actual={actual_pointers:X?}",
                block.index
            )));
        }
    }

    let allowed_ranges = relink
        .blocks
        .iter()
        .map(|block| block.start_offset..block.end_offset)
        .collect::<Vec<_>>();
    let pointer_fields = source_messages
        .iter()
        .flat_map(|row| row.pointer_field_offset..row.pointer_field_offset + 4)
        .collect::<BTreeSet<_>>();
    for (offset, (&before, &after)) in raw.iter().zip(&output).enumerate() {
        if before != after
            && !allowed_ranges.iter().any(|range| range.contains(&offset))
            && !pointer_fields.contains(&offset)
        {
            return Err(error(format!(
                "scene repack 허용 밖 byte 변경: +0x{offset:X}"
            )));
        }
    }
    Ok(SubgameSeqSceneRepack {
        scope: relink.scope,
        sequence_name: relink.sequence,
        sequence: output,
        record_offsets,
        used_bytes,
        remaining_bytes,
    })
}

pub fn render_subgame_seq_scene_relinks_tsv(analysis: &SubgameSeqSceneRelinkAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tscope\texe\tarchive\tsequence\tsequence_sha256\tload_base\tinitialized_bytes\t\
         block_index\tblock_start\tblock_end\tcapacity_bytes\toriginal_message_bytes\t\
         slack_bytes\tmessage_count\tmessage_ids\toriginal_offsets\tpointer_fields\t\
         reference_contract\tcapacity_policy\tstatic_status\n",
    );
    for row in &analysis.rows {
        for block in &row.blocks {
            let offsets = block
                .original_offsets
                .iter()
                .map(|offset| format!("0x{offset:04X}"))
                .collect::<Vec<_>>()
                .join(",");
            let pointer_fields = block
                .pointers
                .iter()
                .map(|pointer| {
                    format!(
                        "0x{:04X}->0x{:04X}",
                        pointer.field_offset, pointer.target_offset
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            writeln!(
                output,
                "{}\t{}\t{}\t{}\t{}\t{}\t0x{:08X}\t{}\t{}\t0x{:04X}\t0x{:04X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
                 bytewise-all-u32-into-block-equals-e0-fields\tinter-record-zero-only-no-trailing-zero-claim\t\
                 bounded-no-op-repack-exact-runtime-unverified",
                SUBGAME_SEQ_SCENE_RELINK_SCHEMA_VERSION,
                row.scope,
                row.exe,
                row.archive,
                row.sequence,
                hex(&row.sequence_sha256),
                row.load_base,
                row.initialized_bytes,
                block.index,
                block.start_offset,
                block.end_offset,
                block.capacity_bytes,
                block.original_message_bytes,
                block.slack_bytes,
                block.message_ids.len(),
                block.message_ids.join(","),
                offsets,
                pointer_fields,
            )
            .expect("String 쓰기는 실패하지 않는다");
        }
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_scene_relink_tests.rs"]
mod subgame_seq_scene_relink_tests;
