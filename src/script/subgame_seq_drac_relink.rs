//! DRAC message record의 제자리 재포장·pointer relink 정적 경계.
//!
//! 두 `DRAC_SEQ`의 repertoire 뒤에는 다섯 message record가 연속으로 있고,
//! 각 record는 길이 필드가 아니라 `65 65` 종결쌍으로 끝난다. 이 모듈은
//! record 뒤의 exact zero reserve와 번역 영역을 가리키는 모든 32-bit
//! pointer를 byte 단위로 전수 검사한다. 이 결과는 제자리 재포장 전략의
//! 정적 허용 범위이며 실제 번역문, UX와 runtime 표시는 승인하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::RuntimeAssetMap;

use super::{
    SubgameSeqDracTextAnalysis, SubgameSeqDracTextKind, SubgameSeqLayoutAnalysis,
    SUBGAME_SEQ_OVERLAY_FOOTER_LEN,
};

pub const SUBGAME_SEQ_DRAC_RELINK_SCHEMA_VERSION: &str = "subgame-seq-drac-relink-v1";
pub const BOX_SUBGAME_SEQ_DRAC_RELINK_ROWS: usize = 2;
pub const BOX_SUBGAME_SEQ_DRAC_RELINK_POINTERS_PER_COPY: usize = 6;
pub const BOX_SUBGAME_SEQ_DRAC_RELINK_MESSAGE_RECORDS: usize = 5;
pub const BOX_SUBGAME_SEQ_DRAC_RELINK_ORIGINAL_MESSAGE_BYTES: usize = 240;
pub const BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_CAPACITY_BYTES: usize = 270;
pub const BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_SLACK_BYTES: usize = 30;
pub const BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_EXTRA_TWO_BYTE_GLYPHS: usize = 15;
const LOAD_BASE: u32 = 0x801B_5270;
const REPERTOIRE_BYTES: usize = 112;
const SEPARATOR_BYTES: usize = 2;
const TERMINATOR: [u8; 2] = [0x65, 0x65];

/// 지원 원본에서 생성한 2행 TSV의 SHA-256.
pub const BOX_SUBGAME_SEQ_DRAC_RELINK_REPORT_SHA256: [u8; 32] = [
    0xEC, 0x83, 0x0A, 0x34, 0xE8, 0x0F, 0xCE, 0xC3, 0xBB, 0xD0, 0x8D, 0xF8, 0x68, 0xBC, 0x85, 0xC4,
    0x70, 0x95, 0xE1, 0x4B, 0x80, 0x18, 0xCC, 0xFD, 0xAA, 0xC2, 0xB5, 0x1B, 0x79, 0x44, 0x49, 0xD3,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqDracRelinkSpec {
    pub scope: &'static str,
    pub archive: &'static str,
    pub sequence: &'static str,
    pub repertoire_offset: usize,
    pub message_start_offset: usize,
    pub original_message_end_offset: usize,
    pub capacity_end_offset: usize,
    pub footer_offset: usize,
    pub pointer_fields: [usize; BOX_SUBGAME_SEQ_DRAC_RELINK_POINTERS_PER_COPY],
}

pub const BOX_SUBGAME_SEQ_DRAC_RELINK_SPECS: [SubgameSeqDracRelinkSpec; 2] = [
    SubgameSeqDracRelinkSpec {
        scope: "PUYO1",
        archive: "/PUYO1/PP1_SEQ.VDD;1",
        sequence: "DRAC_SEQ.SEQ",
        repertoire_offset: 0x1AAC,
        message_start_offset: 0x1B1E,
        original_message_end_offset: 0x1C0E,
        capacity_end_offset: 0x1C2C,
        footer_offset: 0x1DC0,
        pointer_fields: [0x0B68, 0x0D0C, 0x0D28, 0x0D48, 0x0D68, 0x0D84],
    },
    SubgameSeqDracRelinkSpec {
        scope: "PUYO2",
        archive: "/PUYO2/PP2_SEQ.VDD;1",
        sequence: "DRAC_SEQ.SEQ",
        repertoire_offset: 0x1A18,
        message_start_offset: 0x1A8A,
        original_message_end_offset: 0x1B7A,
        capacity_end_offset: 0x1B9C,
        footer_offset: 0x1D20,
        pointer_fields: [0x0B68, 0x0C80, 0x0C9C, 0x0CBC, 0x0CDC, 0x0CF8],
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracRelinkPointer {
    pub field_offset: usize,
    pub field_runtime: u32,
    pub target_offset: usize,
    pub target_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracRelinkRow {
    pub scope: &'static str,
    pub archive: &'static str,
    pub sequence: &'static str,
    pub repertoire_offset: usize,
    pub repertoire_bytes: usize,
    pub message_start_offset: usize,
    pub original_message_end_offset: usize,
    pub capacity_end_offset: usize,
    pub original_message_bytes: usize,
    pub capacity_bytes: usize,
    pub slack_bytes: usize,
    pub max_extra_two_byte_glyphs: usize,
    pub record_bytes: Vec<usize>,
    pub pointers: Vec<SubgameSeqDracRelinkPointer>,
    pub footer_offset: usize,
    pub protected_tail_bytes: usize,
    pub protected_tail_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracRelinkAnalysis {
    pub rows: Vec<SubgameSeqDracRelinkRow>,
    pub common_capacity_bytes: usize,
    pub common_slack_bytes: usize,
    pub common_extra_two_byte_glyphs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracRepack {
    pub scope: &'static str,
    pub sequence: Vec<u8>,
    pub record_offsets: Vec<usize>,
    pub record_bytes: Vec<usize>,
    pub used_bytes: usize,
    pub remaining_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracRelinkError(String);

impl fmt::Display for SubgameSeqDracRelinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqDracRelinkError {}

fn error(message: impl Into<String>) -> SubgameSeqDracRelinkError {
    SubgameSeqDracRelinkError(message.into())
}

fn read_u32(raw: &[u8], offset: usize) -> Option<u32> {
    raw.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
}

fn control_signature(
    record: &[u8],
    repertoire: &BTreeSet<[u8; 2]>,
) -> Result<Vec<Vec<u8>>, SubgameSeqDracRelinkError> {
    if record.len() < 6 {
        return Err(error("DRAC record가 header와 종결쌍보다 짧음"));
    }
    let mut controls = Vec::new();
    let mut cursor = 4usize;
    while cursor < record.len() {
        if record.get(cursor..cursor + 2) == Some(TERMINATOR.as_slice()) {
            if cursor + 2 != record.len() {
                return Err(error("DRAC record 종결 뒤 trailing byte가 있음"));
            }
            return Ok(controls);
        }
        match record[cursor] {
            code @ (0x71 | 0x77) => {
                let argument = *record
                    .get(cursor + 1)
                    .ok_or_else(|| error(format!("DRAC control 0x{code:02X} 인자가 잘림")))?;
                controls.push(vec![code, argument]);
                cursor += 2;
            }
            code @ (0x72 | 0x73) => {
                controls.push(vec![code]);
                cursor += 1;
            }
            _ => {
                let pair: [u8; 2] = record
                    .get(cursor..cursor + 2)
                    .ok_or_else(|| error("DRAC record glyph pair가 잘림"))?
                    .try_into()
                    .expect("2바이트 slice");
                if !repertoire.contains(&pair) {
                    return Err(error(format!(
                        "DRAC record glyph pair가 repertoire에 없음: {:02X}{:02X}",
                        pair[0], pair[1]
                    )));
                }
                cursor += 2;
            }
        }
    }
    Err(error("DRAC record 종결쌍이 없음"))
}

fn repack_records(
    spec: SubgameSeqDracRelinkSpec,
    raw: &[u8],
    original_records: &[Vec<u8>],
    replacement_records: &[Vec<u8>],
) -> Result<SubgameSeqDracRepack, SubgameSeqDracRelinkError> {
    if original_records.len() != BOX_SUBGAME_SEQ_DRAC_RELINK_MESSAGE_RECORDS
        || replacement_records.len() != BOX_SUBGAME_SEQ_DRAC_RELINK_MESSAGE_RECORDS
    {
        return Err(error("DRAC repack message record 분모 불일치"));
    }
    let expected_len = spec
        .footer_offset
        .checked_add(SUBGAME_SEQ_OVERLAY_FOOTER_LEN)
        .ok_or_else(|| error("DRAC repack 원본 길이 overflow"))?;
    if raw.len() != expected_len {
        return Err(error(format!(
            "{} DRAC repack 원본 길이 불일치",
            spec.scope
        )));
    }
    let repertoire_end = spec
        .repertoire_offset
        .checked_add(REPERTOIRE_BYTES)
        .ok_or_else(|| error("DRAC repack repertoire 끝 overflow"))?;
    let repertoire_raw = raw
        .get(spec.repertoire_offset..repertoire_end)
        .ok_or_else(|| error("DRAC repack repertoire 범위 이탈"))?;
    let repertoire = repertoire_raw
        .chunks_exact(2)
        .map(|pair| <[u8; 2]>::try_from(pair).expect("2바이트 pair"))
        .collect::<BTreeSet<_>>();
    if repertoire_raw.len() != REPERTOIRE_BYTES || repertoire.len() < 2 {
        return Err(error("DRAC repack repertoire 분모 불일치"));
    }

    let mut original_cursor = spec.message_start_offset;
    let mut original_signatures = Vec::with_capacity(original_records.len());
    for (index, record) in original_records.iter().enumerate() {
        let original_end = original_cursor
            .checked_add(record.len())
            .ok_or_else(|| error("DRAC original record 끝 overflow"))?;
        if raw.get(original_cursor..original_end) != Some(record.as_slice()) {
            return Err(error(format!(
                "{} DRAC message {} 원본 전제 불일치",
                spec.scope,
                index + 1
            )));
        }
        let original_offset = u32::try_from(original_cursor)
            .map_err(|_| error("DRAC original pointer offset이 u32를 넘음"))?;
        let expected_runtime = LOAD_BASE
            .checked_add(original_offset)
            .ok_or_else(|| error("DRAC original pointer runtime overflow"))?;
        if read_u32(raw, spec.pointer_fields[index + 1]) != Some(expected_runtime) {
            return Err(error(format!(
                "{} DRAC message {} original pointer 불일치",
                spec.scope,
                index + 1
            )));
        }
        original_signatures.push(control_signature(record, &repertoire)?);
        original_cursor = original_end;
    }
    if original_cursor != spec.original_message_end_offset {
        return Err(error(format!(
            "{} DRAC original message 끝 불일치",
            spec.scope
        )));
    }

    let mut replacement_cursor = spec.message_start_offset;
    let mut record_offsets = Vec::with_capacity(replacement_records.len());
    for (index, record) in replacement_records.iter().enumerate() {
        if record.get(..4) != original_records[index].get(..4) {
            return Err(error(format!(
                "{} DRAC message {} header 변경",
                spec.scope,
                index + 1
            )));
        }
        if control_signature(record, &repertoire)? != original_signatures[index] {
            return Err(error(format!(
                "{} DRAC message {} control sequence 변경",
                spec.scope,
                index + 1
            )));
        }
        let replacement_end = replacement_cursor
            .checked_add(record.len())
            .ok_or_else(|| error("DRAC replacement record cursor overflow"))?;
        record_offsets.push(replacement_cursor);
        replacement_cursor = replacement_end;
    }
    if replacement_cursor > spec.capacity_end_offset {
        return Err(error(format!(
            "{} DRAC replacement capacity 초과: used={}, capacity={}",
            spec.scope,
            replacement_cursor - spec.message_start_offset,
            spec.capacity_end_offset - spec.message_start_offset
        )));
    }

    let mut sequence = raw.to_vec();
    sequence[spec.message_start_offset..spec.capacity_end_offset].fill(0);
    for ((index, record), &offset) in replacement_records.iter().enumerate().zip(&record_offsets) {
        let record_end = offset
            .checked_add(record.len())
            .ok_or_else(|| error("DRAC replacement write 끝 overflow"))?;
        sequence[offset..record_end].copy_from_slice(record);
        let target_offset = u32::try_from(offset)
            .map_err(|_| error("DRAC replacement pointer offset이 u32를 넘음"))?;
        let target = LOAD_BASE
            .checked_add(target_offset)
            .ok_or_else(|| error("DRAC replacement pointer runtime overflow"))?;
        let field = spec.pointer_fields[index + 1];
        let field_end = field
            .checked_add(4)
            .ok_or_else(|| error("DRAC replacement pointer field 끝 overflow"))?;
        sequence
            .get_mut(field..field_end)
            .ok_or_else(|| error("DRAC replacement pointer field 범위 이탈"))?
            .copy_from_slice(&target.to_le_bytes());
    }
    let record_bytes = replacement_records.iter().map(Vec::len).collect::<Vec<_>>();
    let used_bytes = replacement_cursor - spec.message_start_offset;
    Ok(SubgameSeqDracRepack {
        scope: spec.scope,
        sequence,
        record_offsets,
        record_bytes,
        used_bytes,
        remaining_bytes: spec.capacity_end_offset - replacement_cursor,
    })
}

fn scan_region_pointers(
    raw: &[u8],
    initialized_len: usize,
    map: RuntimeAssetMap,
    region_start: usize,
    region_end: usize,
) -> Result<Vec<SubgameSeqDracRelinkPointer>, SubgameSeqDracRelinkError> {
    let scan_end = initialized_len
        .checked_sub(3)
        .ok_or_else(|| error("DRAC initialized 영역이 pointer보다 짧음"))?;
    let mut pointers = Vec::new();
    for field_offset in 0..scan_end {
        let value =
            read_u32(raw, field_offset).ok_or_else(|| error("DRAC pointer scan 범위 이탈"))?;
        let Some(target_offset) = map.file_offset(value) else {
            continue;
        };
        if !(region_start..region_end).contains(&target_offset) {
            continue;
        }
        let field_runtime = map
            .runtime_addr(field_offset)
            .ok_or_else(|| error("DRAC pointer field runtime 변환 실패"))?;
        pointers.push(SubgameSeqDracRelinkPointer {
            field_offset,
            field_runtime,
            target_offset,
            target_runtime: value,
        });
    }
    Ok(pointers)
}

fn audit_subgame_seq_drac_relinks<'a>(
    specs: &[SubgameSeqDracRelinkSpec],
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    drac_text: &SubgameSeqDracTextAnalysis,
    layouts: &SubgameSeqLayoutAnalysis,
) -> Result<SubgameSeqDracRelinkAnalysis, SubgameSeqDracRelinkError> {
    if specs.is_empty() {
        return Err(error("DRAC relink spec이 없음"));
    }
    let mut spec_keys = BTreeSet::new();
    for spec in specs {
        if !spec_keys.insert((spec.scope, spec.archive, spec.sequence)) {
            return Err(error(format!("{} DRAC relink spec 중복", spec.scope)));
        }
    }
    let mut source_map = BTreeMap::new();
    for (archive, sequence, raw) in sources {
        if source_map.insert((archive, sequence), raw).is_some() {
            return Err(error(format!(
                "{archive}:{sequence} DRAC relink source 중복"
            )));
        }
    }

    let mut rows = Vec::with_capacity(specs.len());
    for &spec in specs {
        let raw = source_map
            .get(&(spec.archive, spec.sequence))
            .copied()
            .ok_or_else(|| error(format!("{} DRAC relink source 누락", spec.scope)))?;
        let layout = layouts
            .rows
            .iter()
            .find(|row| row.archive == spec.archive && row.sequence == spec.sequence)
            .ok_or_else(|| error(format!("{} DRAC relink layout 누락", spec.scope)))?;
        let footer = layout
            .footer
            .ok_or_else(|| error(format!("{} DRAC relink footer 누락", spec.scope)))?;
        if footer.load_base != LOAD_BASE
            || footer.footer_offset != spec.footer_offset
            || footer.initialized_len as usize != spec.footer_offset
            || raw.len() != spec.footer_offset + SUBGAME_SEQ_OVERLAY_FOOTER_LEN
        {
            return Err(error(format!(
                "{} DRAC relink footer 경계 불일치",
                spec.scope
            )));
        }
        let raw_len = u32::try_from(raw.len()).map_err(|_| error("DRAC SEQ 길이가 u32를 넘음"))?;
        let map = RuntimeAssetMap::new(LOAD_BASE, raw_len)
            .ok_or_else(|| error("DRAC relink runtime 사상 overflow"))?;

        let repertoire = drac_text
            .rows
            .iter()
            .find(|row| row.scope == spec.scope && row.kind == SubgameSeqDracTextKind::Repertoire)
            .ok_or_else(|| error(format!("{} DRAC repertoire 행 누락", spec.scope)))?;
        if repertoire.record_offset != spec.repertoire_offset
            || repertoire.raw.len() != REPERTOIRE_BYTES
            || repertoire.pointer_field_offset != spec.pointer_fields[0]
        {
            return Err(error(format!(
                "{} DRAC repertoire relink 경계 불일치",
                spec.scope
            )));
        }
        let separator_start = spec.repertoire_offset + REPERTOIRE_BYTES;
        if separator_start + SEPARATOR_BYTES != spec.message_start_offset
            || raw.get(separator_start..spec.message_start_offset) != Some(TERMINATOR.as_slice())
        {
            return Err(error(format!(
                "{} DRAC repertoire separator 불일치",
                spec.scope
            )));
        }

        let mut messages = drac_text
            .rows
            .iter()
            .filter(|row| row.scope == spec.scope && row.kind == SubgameSeqDracTextKind::Message)
            .collect::<Vec<_>>();
        messages.sort_by_key(|row| row.index);
        if messages.len() != BOX_SUBGAME_SEQ_DRAC_RELINK_MESSAGE_RECORDS
            || messages
                .iter()
                .enumerate()
                .any(|(index, row)| row.index != index + 1)
        {
            return Err(error(format!(
                "{} DRAC message relink 행 분모 불일치",
                spec.scope
            )));
        }
        let mut cursor = spec.message_start_offset;
        let mut record_bytes = Vec::with_capacity(messages.len());
        for (index, row) in messages.iter().enumerate() {
            if row.record_offset != cursor
                || row.pointer_field_offset != spec.pointer_fields[index + 1]
                || row.raw.len() < 6
                || !row.raw.ends_with(&TERMINATOR)
                || row
                    .raw
                    .windows(2)
                    .filter(|pair| *pair == TERMINATOR)
                    .count()
                    != 1
                || raw.get(cursor..cursor + row.raw.len()) != Some(row.raw.as_slice())
            {
                return Err(error(format!(
                    "{} DRAC message {} 연속/종결 경계 불일치",
                    spec.scope,
                    index + 1
                )));
            }
            cursor = cursor
                .checked_add(row.raw.len())
                .ok_or_else(|| error("DRAC message cursor overflow"))?;
            record_bytes.push(row.raw.len());
        }
        if cursor != spec.original_message_end_offset
            || cursor - spec.message_start_offset
                != BOX_SUBGAME_SEQ_DRAC_RELINK_ORIGINAL_MESSAGE_BYTES
            || spec.capacity_end_offset < cursor
            || raw
                .get(cursor..spec.capacity_end_offset)
                .ok_or_else(|| error("DRAC message zero reserve 범위 이탈"))?
                .iter()
                .any(|&byte| byte != 0)
        {
            return Err(error(format!(
                "{} DRAC message capacity 경계 불일치",
                spec.scope
            )));
        }

        let pointers = scan_region_pointers(
            raw,
            spec.footer_offset,
            map,
            spec.repertoire_offset,
            spec.capacity_end_offset,
        )?;
        let expected_targets = std::iter::once(repertoire)
            .chain(messages.iter().copied())
            .map(|row| (row.pointer_field_offset, row.record_offset))
            .collect::<Vec<_>>();
        let actual_targets = pointers
            .iter()
            .map(|pointer| (pointer.field_offset, pointer.target_offset))
            .collect::<Vec<_>>();
        if actual_targets != expected_targets
            || pointers
                .iter()
                .any(|pointer| !pointer.field_offset.is_multiple_of(4))
            || pointers.len() != BOX_SUBGAME_SEQ_DRAC_RELINK_POINTERS_PER_COPY
        {
            return Err(error(format!(
                "{} DRAC relink pointer 전수 불일치: expected={expected_targets:?}, actual={actual_targets:?}",
                spec.scope
            )));
        }

        let capacity_bytes = spec.capacity_end_offset - spec.message_start_offset;
        let original_message_bytes = spec.original_message_end_offset - spec.message_start_offset;
        let slack_bytes = capacity_bytes - original_message_bytes;
        let protected_tail = raw
            .get(spec.capacity_end_offset..spec.footer_offset)
            .ok_or_else(|| error("DRAC protected tail 범위 이탈"))?;
        rows.push(SubgameSeqDracRelinkRow {
            scope: spec.scope,
            archive: spec.archive,
            sequence: spec.sequence,
            repertoire_offset: spec.repertoire_offset,
            repertoire_bytes: REPERTOIRE_BYTES,
            message_start_offset: spec.message_start_offset,
            original_message_end_offset: spec.original_message_end_offset,
            capacity_end_offset: spec.capacity_end_offset,
            original_message_bytes,
            capacity_bytes,
            slack_bytes,
            max_extra_two_byte_glyphs: slack_bytes / 2,
            record_bytes,
            pointers,
            footer_offset: spec.footer_offset,
            protected_tail_bytes: protected_tail.len(),
            protected_tail_sha256: hex(&Sha256::digest(protected_tail)),
        });
    }
    rows.sort_by_key(|row| row.scope);
    let common_capacity_bytes = rows
        .iter()
        .map(|row| row.capacity_bytes)
        .min()
        .ok_or_else(|| error("DRAC common capacity 행이 없음"))?;
    let common_slack_bytes = rows
        .iter()
        .map(|row| row.slack_bytes)
        .min()
        .ok_or_else(|| error("DRAC common slack 행이 없음"))?;
    Ok(SubgameSeqDracRelinkAnalysis {
        rows,
        common_capacity_bytes,
        common_slack_bytes,
        common_extra_two_byte_glyphs: common_slack_bytes / 2,
    })
}

/// 두 DRAC 사본의 제자리 message 재포장·pointer relink 정적 경계를 검증한다.
pub fn verify_box_subgame_seq_drac_relinks<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    drac_text: &SubgameSeqDracTextAnalysis,
    layouts: &SubgameSeqLayoutAnalysis,
) -> Result<SubgameSeqDracRelinkAnalysis, SubgameSeqDracRelinkError> {
    let sources = sources.into_iter().collect::<Vec<_>>();
    let analysis = audit_subgame_seq_drac_relinks(
        &BOX_SUBGAME_SEQ_DRAC_RELINK_SPECS,
        sources.iter().copied(),
        drac_text,
        layouts,
    )?;
    let actual = [
        analysis.rows.len(),
        analysis.common_capacity_bytes,
        analysis.common_slack_bytes,
        analysis.common_extra_two_byte_glyphs,
    ];
    let expected = [
        BOX_SUBGAME_SEQ_DRAC_RELINK_ROWS,
        BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_CAPACITY_BYTES,
        BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_SLACK_BYTES,
        BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_EXTRA_TWO_BYTE_GLYPHS,
    ];
    if actual != expected
        || analysis.rows.iter().any(|row| {
            row.original_message_bytes != BOX_SUBGAME_SEQ_DRAC_RELINK_ORIGINAL_MESSAGE_BYTES
                || row.pointers.len() != BOX_SUBGAME_SEQ_DRAC_RELINK_POINTERS_PER_COPY
        })
    {
        return Err(error(format!(
            "DRAC relink 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }
    let report = render_subgame_seq_drac_relinks_tsv(&analysis);
    let digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if digest != BOX_SUBGAME_SEQ_DRAC_RELINK_REPORT_SHA256 {
        return Err(error(format!(
            "DRAC relink report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_DRAC_RELINK_REPORT_SHA256),
            hex(&digest)
        )));
    }
    for spec in BOX_SUBGAME_SEQ_DRAC_RELINK_SPECS {
        let raw = sources
            .iter()
            .find(|(archive, sequence, _)| *archive == spec.archive && *sequence == spec.sequence)
            .map(|(_, _, raw)| *raw)
            .ok_or_else(|| error(format!("{} DRAC repack source 누락", spec.scope)))?;
        let mut records = drac_text
            .rows
            .iter()
            .filter(|row| row.scope == spec.scope && row.kind == SubgameSeqDracTextKind::Message)
            .collect::<Vec<_>>();
        records.sort_by_key(|row| row.index);
        let records = records
            .into_iter()
            .map(|row| row.raw.clone())
            .collect::<Vec<_>>();
        let repack =
            repack_box_subgame_seq_drac_messages(spec.scope, raw, &records, drac_text, &analysis)?;
        if repack.sequence != raw
            || repack.used_bytes != BOX_SUBGAME_SEQ_DRAC_RELINK_ORIGINAL_MESSAGE_BYTES
            || repack.remaining_bytes != BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_SLACK_BYTES
        {
            return Err(error(format!(
                "{} DRAC 원문 no-op repack 검산 불일치",
                spec.scope
            )));
        }
    }
    Ok(analysis)
}

/// 검증된 한 DRAC 사본의 다섯 message record를 공통 270바이트 안에서 재포장한다.
///
/// replacement는 원본과 같은 4바이트 header와 control 순서를 유지해야 하며,
/// 본문 glyph pair는 해당 장면의 112바이트 repertoire 안에 있어야 한다.
/// 반환 전 원본 record, direct pointer 전수, zero reserve와 보호 tail을 다시
/// 검증하고 message 영역과 다섯 pointer field 외의 byte가 같은지도 확인한다.
pub fn repack_box_subgame_seq_drac_messages(
    scope: &str,
    raw: &[u8],
    replacement_records: &[Vec<u8>],
    drac_text: &SubgameSeqDracTextAnalysis,
    relinks: &SubgameSeqDracRelinkAnalysis,
) -> Result<SubgameSeqDracRepack, SubgameSeqDracRelinkError> {
    let spec = BOX_SUBGAME_SEQ_DRAC_RELINK_SPECS
        .iter()
        .copied()
        .find(|spec| spec.scope == scope)
        .ok_or_else(|| error(format!("지원하지 않는 DRAC repack scope: {scope}")))?;
    if relinks.common_capacity_bytes != BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_CAPACITY_BYTES
        || relinks.common_slack_bytes != BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_SLACK_BYTES
        || relinks.common_extra_two_byte_glyphs
            != BOX_SUBGAME_SEQ_DRAC_RELINK_COMMON_EXTRA_TWO_BYTE_GLYPHS
    {
        return Err(error("DRAC repack 공통 용량 분석 불일치"));
    }
    let matching_relinks = relinks
        .rows
        .iter()
        .filter(|row| row.scope == spec.scope)
        .collect::<Vec<_>>();
    if matching_relinks.len() != 1 {
        return Err(error(format!(
            "{} DRAC repack relink 행 분모 불일치",
            spec.scope
        )));
    }
    let relink = matching_relinks[0];

    let matching_repertoires = drac_text
        .rows
        .iter()
        .filter(|row| row.scope == spec.scope && row.kind == SubgameSeqDracTextKind::Repertoire)
        .collect::<Vec<_>>();
    if matching_repertoires.len() != 1 {
        return Err(error(format!(
            "{} DRAC repack repertoire 행 분모 불일치",
            spec.scope
        )));
    }
    let repertoire = matching_repertoires[0];
    let mut messages = drac_text
        .rows
        .iter()
        .filter(|row| row.scope == spec.scope && row.kind == SubgameSeqDracTextKind::Message)
        .collect::<Vec<_>>();
    messages.sort_by_key(|row| row.index);
    if messages.len() != BOX_SUBGAME_SEQ_DRAC_RELINK_MESSAGE_RECORDS
        || messages
            .iter()
            .enumerate()
            .any(|(index, row)| row.index != index + 1)
    {
        return Err(error(format!(
            "{} DRAC repack message 행 분모 불일치",
            spec.scope
        )));
    }
    let original_records = messages
        .iter()
        .map(|row| row.raw.clone())
        .collect::<Vec<_>>();
    let record_bytes = original_records.iter().map(Vec::len).collect::<Vec<_>>();

    let capacity_bytes = spec
        .capacity_end_offset
        .checked_sub(spec.message_start_offset)
        .ok_or_else(|| error("DRAC repack capacity 경계 역전"))?;
    let original_message_bytes = spec
        .original_message_end_offset
        .checked_sub(spec.message_start_offset)
        .ok_or_else(|| error("DRAC repack original message 경계 역전"))?;
    let slack_bytes = capacity_bytes
        .checked_sub(original_message_bytes)
        .ok_or_else(|| error("DRAC repack zero reserve 경계 역전"))?;
    let protected_tail_bytes = spec
        .footer_offset
        .checked_sub(spec.capacity_end_offset)
        .ok_or_else(|| error("DRAC repack 보호 tail 경계 역전"))?;
    if relink.archive != spec.archive
        || relink.sequence != spec.sequence
        || relink.repertoire_offset != spec.repertoire_offset
        || relink.repertoire_bytes != REPERTOIRE_BYTES
        || relink.message_start_offset != spec.message_start_offset
        || relink.original_message_end_offset != spec.original_message_end_offset
        || relink.capacity_end_offset != spec.capacity_end_offset
        || relink.original_message_bytes != original_message_bytes
        || relink.capacity_bytes != capacity_bytes
        || relink.slack_bytes != slack_bytes
        || relink.max_extra_two_byte_glyphs != slack_bytes / 2
        || relink.record_bytes != record_bytes
        || relink.footer_offset != spec.footer_offset
        || relink.protected_tail_bytes != protected_tail_bytes
    {
        return Err(error(format!(
            "{} DRAC repack relink 정적 경계 불일치",
            spec.scope
        )));
    }
    if repertoire.archive != spec.archive
        || repertoire.sequence != spec.sequence
        || repertoire.index != 0
        || repertoire.record_offset != spec.repertoire_offset
        || repertoire.pointer_field_offset != spec.pointer_fields[0]
        || repertoire.raw.len() != REPERTOIRE_BYTES
    {
        return Err(error(format!(
            "{} DRAC repack repertoire 정적 경계 불일치",
            spec.scope
        )));
    }
    let repertoire_end = spec
        .repertoire_offset
        .checked_add(REPERTOIRE_BYTES)
        .ok_or_else(|| error("DRAC repack repertoire 끝 overflow"))?;
    if raw.get(spec.repertoire_offset..repertoire_end) != Some(repertoire.raw.as_slice())
        || raw.get(repertoire_end..spec.message_start_offset) != Some(TERMINATOR.as_slice())
    {
        return Err(error(format!(
            "{} DRAC repack repertoire 원본 전제 불일치",
            spec.scope
        )));
    }
    if raw
        .get(spec.original_message_end_offset..spec.capacity_end_offset)
        .ok_or_else(|| error("DRAC repack zero reserve 범위 이탈"))?
        .iter()
        .any(|&byte| byte != 0)
    {
        return Err(error(format!(
            "{} DRAC repack zero reserve 원본 전제 불일치",
            spec.scope
        )));
    }
    let protected_tail = raw
        .get(spec.capacity_end_offset..spec.footer_offset)
        .ok_or_else(|| error("DRAC repack 보호 tail 범위 이탈"))?;
    if hex(&Sha256::digest(protected_tail)) != relink.protected_tail_sha256 {
        return Err(error(format!(
            "{} DRAC repack 보호 tail digest 불일치",
            spec.scope
        )));
    }

    let raw_len = u32::try_from(raw.len()).map_err(|_| error("DRAC SEQ 길이가 u32를 넘음"))?;
    let map = RuntimeAssetMap::new(LOAD_BASE, raw_len)
        .ok_or_else(|| error("DRAC repack runtime 사상 overflow"))?;
    let pointers = scan_region_pointers(
        raw,
        spec.footer_offset,
        map,
        spec.repertoire_offset,
        spec.capacity_end_offset,
    )?;
    if pointers != relink.pointers {
        return Err(error(format!(
            "{} DRAC repack direct pointer 전수 불일치",
            spec.scope
        )));
    }

    let common_capacity_end = spec
        .message_start_offset
        .checked_add(relinks.common_capacity_bytes)
        .ok_or_else(|| error("DRAC repack 공통 capacity 끝 overflow"))?;
    if common_capacity_end > spec.capacity_end_offset {
        return Err(error("DRAC repack 공통 capacity가 사본 범위를 넘음"));
    }
    let mut common_spec = spec;
    common_spec.capacity_end_offset = common_capacity_end;
    let repack = repack_records(common_spec, raw, &original_records, replacement_records)?;

    for (offset, (&before, &after)) in raw.iter().zip(&repack.sequence).enumerate() {
        let is_message = (spec.message_start_offset..common_capacity_end).contains(&offset);
        let is_message_pointer = spec.pointer_fields[1..].iter().any(|&field| {
            field
                .checked_add(4)
                .is_some_and(|field_end| (field..field_end).contains(&offset))
        });
        if !is_message && !is_message_pointer && before != after {
            return Err(error(format!(
                "{} DRAC repack 비대상 byte 변경: +0x{offset:04X}",
                spec.scope
            )));
        }
    }
    Ok(repack)
}

pub fn render_subgame_seq_drac_relinks_tsv(analysis: &SubgameSeqDracRelinkAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tscope\tarchive\tsequence\trepertoire_offset\trepertoire_bytes\t\
         message_start_offset\toriginal_message_end_offset\tcapacity_end_offset\t\
         original_message_bytes\tcapacity_bytes\tslack_bytes\tmax_extra_two_byte_glyphs\t\
         record_bytes\tpointer_fields\tpointer_targets\tdirect_pointer_count\t\
         footer_offset\tprotected_tail_bytes\tprotected_tail_sha256\tcommon_capacity_bytes\t\
         common_slack_bytes\tcommon_extra_two_byte_glyphs\tstatic_status\truntime_status\n",
    );
    for row in &analysis.rows {
        let record_bytes = row
            .record_bytes
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let pointer_fields = row
            .pointers
            .iter()
            .map(|pointer| format!("0x{:04X}", pointer.field_offset))
            .collect::<Vec<_>>()
            .join(",");
        let pointer_targets = row
            .pointers
            .iter()
            .map(|pointer| format!("0x{:04X}", pointer.target_offset))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t0x{:04X}\t{}\t0x{:04X}\t0x{:04X}\t0x{:04X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t{}\t{}\t{}\t{}\t{}\tpacked-records-five-pointer-relink-within-zero-reserve-static\ttranslation-ux-runtime-unverified",
            SUBGAME_SEQ_DRAC_RELINK_SCHEMA_VERSION,
            row.scope,
            row.archive,
            row.sequence,
            row.repertoire_offset,
            row.repertoire_bytes,
            row.message_start_offset,
            row.original_message_end_offset,
            row.capacity_end_offset,
            row.original_message_bytes,
            row.capacity_bytes,
            row.slack_bytes,
            row.max_extra_two_byte_glyphs,
            record_bytes,
            pointer_fields,
            pointer_targets,
            row.pointers.len(),
            row.footer_offset,
            row.protected_tail_bytes,
            row.protected_tail_sha256,
            analysis.common_capacity_bytes,
            analysis.common_slack_bytes,
            analysis.common_extra_two_byte_glyphs,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_drac_relink_tests.rs"]
mod subgame_seq_drac_relink_tests;
