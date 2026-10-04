//! scene-local repertoire와 E0 message record의 공용 정적 경계.
//!
//! DRAC에서 확인한 형식을 다른 장면에 이름만으로 이식하지 않는다. spec은
//! 각 사본의 원본 digest, repertoire와 pointer field, message record 범위를
//! 독립적으로 선언한다. 공용 분석기는 같은 overlay 내부 pointer, `E0`
//! command, `65 65` 종결, Shift-JIS span과 repertoire 포함 관계를 다시
//! 검증한다. runtime 실행과 화면 표시, control 의미, 재포장은 별도 gate다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::text::decode_sjis;

use super::{
    SubgameSeqLayoutAnalysis, SubgameSeqTextCandidateAnalysis, SubgameSeqTextRootAnalysis,
};

pub const SUBGAME_SEQ_SCENE_TEXT_SCHEMA_VERSION: &str = "subgame-seq-scene-text-v1";

pub const BOX_SUBGAME_SEQ_MIN_TEXT_ROWS: usize = 17;
pub const BOX_SUBGAME_SEQ_MIN_MESSAGE_RECORDS: usize = 15;
pub const BOX_SUBGAME_SEQ_MIN_MESSAGE_SPANS: usize = 24;
pub const BOX_SUBGAME_SEQ_MIN_MESSAGE_TERMINATORS: usize = 16;
pub const BOX_SUBGAME_SEQ_MIN_MESSAGE_CODE_UNITS: usize = 174;
pub const BOX_SUBGAME_SEQ_MIN_MESSAGE_PROTECTED_BYTES: usize = 136;
pub const BOX_SUBGAME_SEQ_MIN_REPERTOIRE_CODE_UNITS: usize = 98;
pub const BOX_SUBGAME_SEQ_MIN_DIRECT_POINTERS: usize = 17;
pub const BOX_SUBGAME_SEQ_MIN_ZERO_RESERVE_BYTES: usize = 4;
pub const BOX_SUBGAME_SEQ_MIN_TEXT_REPORT_SHA256: [u8; 32] = [
    0x15, 0xDA, 0xE9, 0x31, 0x04, 0xB2, 0xBE, 0x62, 0x56, 0xF7, 0xFE, 0x8C, 0x58, 0xED, 0xAD, 0x6A,
    0x54, 0x22, 0x4A, 0x36, 0x5B, 0x24, 0x60, 0x61, 0xF0, 0x84, 0xDF, 0x99, 0x7A, 0x1D, 0xA2, 0x38,
];

const PP1_MIN_POINTER_FIELDS: [usize; 7] = [0x0CE8, 0x0CFC, 0x0D14, 0x0D34, 0x0D5C, 0x0D74, 0x0D8C];
const PP1_MIN_MESSAGE_OFFSETS: [usize; 7] =
    [0x18E0, 0x18F4, 0x195A, 0x1970, 0x1990, 0x19A0, 0x19DA];
const PP2_MIN_POINTER_FIELDS: [usize; 8] = [
    0x0C5C, 0x0C70, 0x0C88, 0x0CA0, 0x0CC0, 0x0CE8, 0x0D00, 0x0D18,
];
const PP2_MIN_MESSAGE_OFFSETS: [usize; 8] = [
    0x17DC, 0x17F0, 0x1804, 0x182E, 0x1844, 0x1864, 0x1874, 0x18AE,
];

#[derive(Debug, Clone, Copy)]
pub struct SubgameSeqSceneTextSpec<'a> {
    pub scope: &'static str,
    pub archive: &'static str,
    pub sequence: &'static str,
    pub expected_sequence_sha256: [u8; 32],
    pub expected_bytes: usize,
    pub repertoire_pointer_field: usize,
    pub repertoire_offset: usize,
    pub repertoire_bytes: usize,
    pub message_pointer_fields: &'a [usize],
    pub message_offsets: &'a [usize],
    pub message_end_offset: usize,
    pub padding_end_offset: usize,
}

pub const BOX_SUBGAME_SEQ_MIN_TEXT_SPECS: [SubgameSeqSceneTextSpec<'static>; 2] = [
    SubgameSeqSceneTextSpec {
        scope: "PUYO1",
        archive: "/PUYO1/PP1_SEQ.VDD;1",
        sequence: "MIN_SEQ.SEQ",
        expected_sequence_sha256: [
            0x5E, 0x7A, 0x88, 0x3E, 0x7D, 0xD5, 0x62, 0x43, 0xB3, 0x8C, 0x0B, 0xC0, 0x2A, 0xE9,
            0x7F, 0x11, 0xA4, 0x40, 0xA0, 0x20, 0x21, 0xB9, 0x49, 0xF5, 0x30, 0xC7, 0xF2, 0x4E,
            0x4F, 0x05, 0xD5, 0x28,
        ],
        expected_bytes: 0x1BA4,
        repertoire_pointer_field: 0x0B44,
        repertoire_offset: 0x187C,
        repertoire_bytes: 98,
        message_pointer_fields: &PP1_MIN_POINTER_FIELDS,
        message_offsets: &PP1_MIN_MESSAGE_OFFSETS,
        message_end_offset: 0x19E6,
        padding_end_offset: 0x19E8,
    },
    SubgameSeqSceneTextSpec {
        scope: "PUYO2",
        archive: "/PUYO2/PP2_SEQ.VDD;1",
        sequence: "MIN_SEQ.SEQ",
        expected_sequence_sha256: [
            0xFC, 0x08, 0x4D, 0x0A, 0x5F, 0x58, 0xC9, 0x62, 0x2A, 0x2F, 0x97, 0x5E, 0x55, 0xDC,
            0xB0, 0xE6, 0x9D, 0x4D, 0x7B, 0x57, 0x6A, 0x60, 0xE9, 0x9D, 0xF7, 0xF9, 0xB7, 0xE5,
            0xA2, 0xE8, 0xCF, 0x50,
        ],
        expected_bytes: 0x1A6C,
        repertoire_pointer_field: 0x0B44,
        repertoire_offset: 0x1778,
        repertoire_bytes: 98,
        message_pointer_fields: &PP2_MIN_POINTER_FIELDS,
        message_offsets: &PP2_MIN_MESSAGE_OFFSETS,
        message_end_offset: 0x18BA,
        padding_end_offset: 0x18BC,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubgameSeqSceneTextKind {
    Repertoire,
    Message,
}

impl SubgameSeqSceneTextKind {
    fn label(self) -> &'static str {
        match self {
            Self::Repertoire => "repertoire",
            Self::Message => "message",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneSjisSpan {
    pub offset: usize,
    pub raw: Vec<u8>,
    pub decoded: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneTextRow {
    pub scope: &'static str,
    pub archive: &'static str,
    pub sequence: &'static str,
    pub kind: SubgameSeqSceneTextKind,
    pub index: usize,
    pub command_variant: Option<u8>,
    pub pointer_field_offset: usize,
    pub pointer_field_runtime: u32,
    pub record_offset: usize,
    pub record_runtime: u32,
    pub raw: Vec<u8>,
    pub raw_sha256: [u8; 32],
    pub spans: Vec<SubgameSeqSceneSjisSpan>,
    pub protected_bytes: usize,
    pub terminators: usize,
    pub kana_candidate_overlaps: usize,
    pub missed_sjis_spans: usize,
}

impl SubgameSeqSceneTextRow {
    pub fn code_units(&self) -> usize {
        self.spans
            .iter()
            .map(|span| span.decoded.chars().count())
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneTextAnalysis {
    pub rows: Vec<SubgameSeqSceneTextRow>,
    pub copies: usize,
    pub repertoires: usize,
    pub repertoire_code_units: usize,
    pub message_records: usize,
    pub message_spans: usize,
    pub message_code_units: usize,
    pub message_protected_bytes: usize,
    pub message_terminators: usize,
    pub kana_candidates_in_messages: usize,
    pub missed_message_spans: usize,
    pub command_variant_records: usize,
    pub direct_pointers: usize,
    pub zero_reserve_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqSceneTextError(String);

impl fmt::Display for SubgameSeqSceneTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqSceneTextError {}

fn error(message: impl Into<String>) -> SubgameSeqSceneTextError {
    SubgameSeqSceneTextError(message.into())
}

fn is_sjis_lead(byte: u8) -> bool {
    matches!(byte, 0x81..=0x9F | 0xE0..=0xEF)
}

fn scan_sjis_spans(raw: &[u8], base_offset: usize) -> Vec<SubgameSeqSceneSjisSpan> {
    let mut spans = Vec::new();
    let mut cursor = 0usize;
    while cursor + 1 < raw.len() {
        if !is_sjis_lead(raw[cursor]) || decode_sjis(&raw[cursor..cursor + 2]).is_err() {
            cursor += 1;
            continue;
        }
        let start = cursor;
        cursor += 2;
        while cursor + 1 < raw.len()
            && is_sjis_lead(raw[cursor])
            && decode_sjis(&raw[cursor..cursor + 2]).is_ok()
        {
            cursor += 2;
        }
        let span_raw = raw[start..cursor].to_vec();
        let decoded = decode_sjis(&span_raw).expect("검증한 Shift-JIS span");
        spans.push(SubgameSeqSceneSjisSpan {
            offset: base_offset + start,
            raw: span_raw,
            decoded,
        });
    }
    spans
}

fn read_u32(raw: &[u8], offset: usize) -> Result<u32, SubgameSeqSceneTextError> {
    let bytes = raw
        .get(offset..offset + 4)
        .ok_or_else(|| error(format!("scene text u32 범위 이탈: +0x{offset:04X}")))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 u32")))
}

fn candidate_overlaps(
    text: &SubgameSeqTextCandidateAnalysis,
    spec: SubgameSeqSceneTextSpec<'_>,
    start: usize,
    end: usize,
) -> usize {
    text.candidates
        .iter()
        .filter(|candidate| {
            candidate.archive == spec.archive
                && candidate.sequence == spec.sequence
                && candidate.offset >= start
                && candidate
                    .offset
                    .checked_add(candidate.raw.len())
                    .is_some_and(|candidate_end| candidate_end <= end)
        })
        .count()
}

fn missed_spans(
    text: &SubgameSeqTextCandidateAnalysis,
    spec: SubgameSeqSceneTextSpec<'_>,
    spans: &[SubgameSeqSceneSjisSpan],
) -> usize {
    spans
        .iter()
        .filter(|span| {
            !text.candidates.iter().any(|candidate| {
                candidate.archive == spec.archive
                    && candidate.sequence == spec.sequence
                    && candidate.offset == span.offset
                    && candidate.raw == span.raw
            })
        })
        .count()
}

/// 선언한 scene text 사본을 공용 형식으로 검증한다.
pub fn audit_subgame_seq_scene_text<'a>(
    specs: &[SubgameSeqSceneTextSpec<'_>],
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    text: &SubgameSeqTextCandidateAnalysis,
    layouts: &SubgameSeqLayoutAnalysis,
    roots: &SubgameSeqTextRootAnalysis,
) -> Result<SubgameSeqSceneTextAnalysis, SubgameSeqSceneTextError> {
    if specs.is_empty() {
        return Err(error("scene text spec이 없음"));
    }
    let mut source_map = BTreeMap::new();
    for (archive, sequence, raw) in sources {
        let key = (archive.to_string(), sequence.to_string());
        if source_map.insert(key.clone(), raw).is_some() {
            return Err(error(format!("scene text 입력 중복: {}:{}", key.0, key.1)));
        }
    }
    let mut spec_keys = BTreeSet::new();
    let mut rows = Vec::new();
    let mut direct_pointers = 0usize;
    let mut zero_reserve_bytes = 0usize;
    let mut repertoire_baseline: Option<Vec<u8>> = None;

    for &spec in specs {
        let key = (spec.archive.to_string(), spec.sequence.to_string());
        if !spec_keys.insert(key.clone()) {
            return Err(error(format!(
                "scene text spec 중복: {}:{}",
                spec.archive, spec.sequence
            )));
        }
        if spec.message_pointer_fields.len() != spec.message_offsets.len()
            || spec.message_offsets.is_empty()
            || spec.repertoire_bytes == 0
            || !spec.repertoire_bytes.is_multiple_of(2)
        {
            return Err(error(format!("{} scene text spec 분모 불일치", spec.scope)));
        }
        let raw = source_map.get(&key).ok_or_else(|| {
            error(format!(
                "scene text 원본 누락: {}:{}",
                spec.archive, spec.sequence
            ))
        })?;
        let sequence_sha256: [u8; 32] = Sha256::digest(raw).into();
        if raw.len() != spec.expected_bytes || sequence_sha256 != spec.expected_sequence_sha256 {
            return Err(error(format!(
                "{} scene text 원본 계약 불일치: expected_bytes=0x{:X}, actual_bytes=0x{:X}, expected_sha256={}, actual_sha256={}",
                spec.scope,
                spec.expected_bytes,
                raw.len(),
                hex(&spec.expected_sequence_sha256),
                hex(&sequence_sha256),
            )));
        }
        let layout = layouts
            .rows
            .iter()
            .find(|row| row.archive == spec.archive && row.sequence == spec.sequence)
            .ok_or_else(|| error(format!("{} scene text layout 누락", spec.scope)))?;
        if layout.sequence_sha256 != sequence_sha256 || layout.bytes != raw.len() {
            return Err(error(format!(
                "{} scene text layout 원본 불일치",
                spec.scope
            )));
        }
        let footer = layout
            .footer
            .ok_or_else(|| error(format!("{} scene text overlay footer 누락", spec.scope)))?;
        let map = footer
            .runtime_map()
            .ok_or_else(|| error("scene text runtime 사상 overflow"))?;

        let root = roots
            .rows
            .iter()
            .find(|row| {
                row.archive == spec.archive
                    && row.sequence == spec.sequence
                    && row.candidate_offset == spec.repertoire_offset
            })
            .ok_or_else(|| error(format!("{} scene text repertoire root 누락", spec.scope)))?;
        if !root.roots.iter().any(|pointer| {
            pointer.pointer_field_offset == spec.repertoire_pointer_field
                && pointer.target_offset == spec.repertoire_offset
                && pointer.candidate_delta == 0
        }) {
            return Err(error(format!(
                "{} scene text repertoire exact pointer 누락",
                spec.scope
            )));
        }

        let repertoire_end = spec
            .repertoire_offset
            .checked_add(spec.repertoire_bytes)
            .ok_or_else(|| error("scene text repertoire 끝 overflow"))?;
        let message_start = repertoire_end
            .checked_add(2)
            .ok_or_else(|| error("scene text message 시작 overflow"))?;
        if spec.message_offsets[0] != message_start
            || raw.get(repertoire_end..message_start) != Some([0x65, 0x65].as_slice())
            || spec.message_end_offset > spec.padding_end_offset
            || raw
                .get(spec.message_end_offset..spec.padding_end_offset)
                .ok_or_else(|| error("scene text zero reserve 범위 이탈"))?
                .iter()
                .any(|&byte| byte != 0)
        {
            return Err(error(format!(
                "{} scene text block 경계 불일치",
                spec.scope
            )));
        }
        let repertoire_raw = raw
            .get(spec.repertoire_offset..repertoire_end)
            .ok_or_else(|| error("scene text repertoire 범위 이탈"))?
            .to_vec();
        let repertoire_text = decode_sjis(&repertoire_raw)
            .map_err(|source| error(format!("scene text repertoire decode 실패: {source}")))?;
        if repertoire_text.chars().count() != spec.repertoire_bytes / 2 {
            return Err(error("scene text repertoire code unit 불일치"));
        }
        if let Some(baseline) = &repertoire_baseline {
            if baseline != &repertoire_raw {
                return Err(error("scene text 사본별 repertoire 불일치"));
            }
        } else {
            repertoire_baseline = Some(repertoire_raw.clone());
        }
        let repertoire_pairs = repertoire_raw
            .chunks_exact(2)
            .map(|pair| <[u8; 2]>::try_from(pair).expect("2바이트 repertoire pair"))
            .collect::<BTreeSet<_>>();
        let repertoire_spans = vec![SubgameSeqSceneSjisSpan {
            offset: spec.repertoire_offset,
            raw: repertoire_raw.clone(),
            decoded: repertoire_text,
        }];
        rows.push(SubgameSeqSceneTextRow {
            scope: spec.scope,
            archive: spec.archive,
            sequence: spec.sequence,
            kind: SubgameSeqSceneTextKind::Repertoire,
            index: 0,
            command_variant: None,
            pointer_field_offset: spec.repertoire_pointer_field,
            pointer_field_runtime: map
                .runtime_addr(spec.repertoire_pointer_field)
                .ok_or_else(|| error("repertoire pointer runtime 변환 실패"))?,
            record_offset: spec.repertoire_offset,
            record_runtime: map
                .runtime_addr(spec.repertoire_offset)
                .ok_or_else(|| error("repertoire runtime 변환 실패"))?,
            raw_sha256: Sha256::digest(&repertoire_raw).into(),
            raw: repertoire_raw,
            spans: repertoire_spans,
            protected_bytes: 0,
            terminators: 0,
            kana_candidate_overlaps: candidate_overlaps(
                text,
                spec,
                spec.repertoire_offset,
                repertoire_end,
            ),
            missed_sjis_spans: 0,
        });

        let mut expected_pointer_fields = BTreeSet::from([spec.repertoire_pointer_field]);
        for (index, (&pointer_field, &start)) in spec
            .message_pointer_fields
            .iter()
            .zip(spec.message_offsets)
            .enumerate()
        {
            if !expected_pointer_fields.insert(pointer_field) {
                return Err(error(format!(
                    "{} scene text pointer field 중복",
                    spec.scope
                )));
            }
            let end = spec
                .message_offsets
                .get(index + 1)
                .copied()
                .unwrap_or(spec.message_end_offset);
            if start >= end {
                return Err(error(format!(
                    "{} scene text message 순서 불일치",
                    spec.scope
                )));
            }
            let command_start = pointer_field
                .checked_sub(4)
                .ok_or_else(|| error("scene text E0 command offset underflow"))?;
            let command = raw
                .get(command_start..pointer_field)
                .ok_or_else(|| error("scene text E0 command 범위 이탈"))?;
            if command[0] != 0xE0 || command[2..4] != [0, 0] || command[1] > 1 {
                return Err(error(format!(
                    "{} scene text E0 command 불일치: message={}",
                    spec.scope,
                    index + 1
                )));
            }
            let target_runtime = read_u32(raw, pointer_field)?;
            if map.file_offset(target_runtime) != Some(start) {
                return Err(error(format!(
                    "{} scene text message pointer target 불일치: message={}",
                    spec.scope,
                    index + 1
                )));
            }
            let record_raw = raw
                .get(start..end)
                .ok_or_else(|| error("scene text message 범위 이탈"))?
                .to_vec();
            let terminators = record_raw
                .windows(2)
                .filter(|window| *window == [0x65, 0x65])
                .count();
            if record_raw.len() < 6 || terminators == 0 || !record_raw.ends_with(&[0x65, 0x65]) {
                return Err(error(format!(
                    "{} scene text message 종결 불일치: message={}",
                    spec.scope,
                    index + 1
                )));
            }
            let spans = scan_sjis_spans(&record_raw[4..], start + 4);
            for span in &spans {
                for pair in span.raw.chunks_exact(2) {
                    let pair: [u8; 2] = pair.try_into().expect("2바이트 message pair");
                    if !repertoire_pairs.contains(&pair) {
                        return Err(error(format!(
                            "{} scene text message glyph가 repertoire에 없음: message={} pair={:02X}{:02X}",
                            spec.scope,
                            index + 1,
                            pair[0],
                            pair[1]
                        )));
                    }
                }
            }
            let sjis_bytes = spans.iter().map(|span| span.raw.len()).sum::<usize>();
            let protected_bytes = record_raw
                .len()
                .checked_sub(sjis_bytes)
                .ok_or_else(|| error("scene text 보호 byte 계산 underflow"))?;
            let overlaps = candidate_overlaps(text, spec, start, end);
            let missed = missed_spans(text, spec, &spans);
            rows.push(SubgameSeqSceneTextRow {
                scope: spec.scope,
                archive: spec.archive,
                sequence: spec.sequence,
                kind: SubgameSeqSceneTextKind::Message,
                index: index + 1,
                command_variant: Some(command[1]),
                pointer_field_offset: pointer_field,
                pointer_field_runtime: map
                    .runtime_addr(pointer_field)
                    .ok_or_else(|| error("message pointer runtime 변환 실패"))?,
                record_offset: start,
                record_runtime: target_runtime,
                raw_sha256: Sha256::digest(&record_raw).into(),
                raw: record_raw,
                spans,
                protected_bytes,
                terminators,
                kana_candidate_overlaps: overlaps,
                missed_sjis_spans: missed,
            });
        }

        let initialized_len = usize::try_from(footer.initialized_len)
            .map_err(|_| error("scene text initialized 길이 변환 실패"))?;
        let mut actual_pointer_fields = BTreeSet::new();
        for field_offset in 0..=initialized_len.saturating_sub(4) {
            let value = u32::from_le_bytes(
                raw[field_offset..field_offset + 4]
                    .try_into()
                    .expect("4바이트 pointer scan"),
            );
            let Some(target_offset) = map.file_offset(value) else {
                continue;
            };
            if (spec.repertoire_offset..spec.padding_end_offset).contains(&target_offset) {
                actual_pointer_fields.insert(field_offset);
            }
        }
        if actual_pointer_fields != expected_pointer_fields {
            return Err(error(format!(
                "{} scene text direct pointer 전수 불일치: expected={expected_pointer_fields:X?}, actual={actual_pointer_fields:X?}",
                spec.scope
            )));
        }
        direct_pointers += actual_pointer_fields.len();
        zero_reserve_bytes += spec.padding_end_offset - spec.message_end_offset;
    }

    let repertoires = rows
        .iter()
        .filter(|row| row.kind == SubgameSeqSceneTextKind::Repertoire)
        .count();
    let message_rows = rows
        .iter()
        .filter(|row| row.kind == SubgameSeqSceneTextKind::Message)
        .collect::<Vec<_>>();
    Ok(SubgameSeqSceneTextAnalysis {
        copies: specs.len(),
        repertoires,
        repertoire_code_units: rows
            .iter()
            .filter(|row| row.kind == SubgameSeqSceneTextKind::Repertoire)
            .map(SubgameSeqSceneTextRow::code_units)
            .sum(),
        message_records: message_rows.len(),
        message_spans: message_rows.iter().map(|row| row.spans.len()).sum(),
        message_code_units: message_rows.iter().map(|row| row.code_units()).sum(),
        message_protected_bytes: message_rows.iter().map(|row| row.protected_bytes).sum(),
        message_terminators: message_rows.iter().map(|row| row.terminators).sum(),
        kana_candidates_in_messages: message_rows
            .iter()
            .map(|row| row.kana_candidate_overlaps)
            .sum(),
        missed_message_spans: message_rows.iter().map(|row| row.missed_sjis_spans).sum(),
        command_variant_records: message_rows
            .iter()
            .filter(|row| row.command_variant == Some(1))
            .count(),
        direct_pointers,
        zero_reserve_bytes,
        rows,
    })
}

/// 지원 원본 `MIN_SEQ` 두 사본의 공용 scene text 경계를 고정한다.
pub fn verify_box_subgame_seq_min_text<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    text: &SubgameSeqTextCandidateAnalysis,
    layouts: &SubgameSeqLayoutAnalysis,
    roots: &SubgameSeqTextRootAnalysis,
) -> Result<SubgameSeqSceneTextAnalysis, SubgameSeqSceneTextError> {
    let analysis = audit_subgame_seq_scene_text(
        &BOX_SUBGAME_SEQ_MIN_TEXT_SPECS,
        sources,
        text,
        layouts,
        roots,
    )?;
    let actual = [
        analysis.rows.len(),
        analysis.message_records,
        analysis.message_spans,
        analysis.message_terminators,
        analysis.message_code_units,
        analysis.message_protected_bytes,
        analysis.repertoire_code_units,
        analysis.direct_pointers,
        analysis.zero_reserve_bytes,
    ];
    let expected = [
        BOX_SUBGAME_SEQ_MIN_TEXT_ROWS,
        BOX_SUBGAME_SEQ_MIN_MESSAGE_RECORDS,
        BOX_SUBGAME_SEQ_MIN_MESSAGE_SPANS,
        BOX_SUBGAME_SEQ_MIN_MESSAGE_TERMINATORS,
        BOX_SUBGAME_SEQ_MIN_MESSAGE_CODE_UNITS,
        BOX_SUBGAME_SEQ_MIN_MESSAGE_PROTECTED_BYTES,
        BOX_SUBGAME_SEQ_MIN_REPERTOIRE_CODE_UNITS,
        BOX_SUBGAME_SEQ_MIN_DIRECT_POINTERS,
        BOX_SUBGAME_SEQ_MIN_ZERO_RESERVE_BYTES,
    ];
    if actual != expected {
        return Err(error(format!(
            "MIN scene text 분모 불일치: expected={expected:?}, actual={actual:?}; candidates/missed/variants={}/{}/{}",
            analysis.kana_candidates_in_messages,
            analysis.missed_message_spans,
            analysis.command_variant_records,
        )));
    }
    let report = render_subgame_seq_scene_text_tsv(&analysis);
    let report_sha256: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if report_sha256 != BOX_SUBGAME_SEQ_MIN_TEXT_REPORT_SHA256 {
        return Err(error(format!(
            "MIN scene text report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_MIN_TEXT_REPORT_SHA256),
            hex(&report_sha256)
        )));
    }
    Ok(analysis)
}

pub fn render_subgame_seq_scene_text_tsv(analysis: &SubgameSeqSceneTextAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tscope\tarchive\tsequence\tkind\tindex\tcommand_variant\t\
         pointer_field_offset\tpointer_field_runtime\trecord_offset\trecord_runtime\t\
         raw_sha256\traw_bytes\tprotected_bytes\tterminators\tsjis_spans\tcode_units\t\
         kana_candidate_overlaps\tmissed_sjis_spans\tconsumer_contract\tpatch_strategy\ttext\n",
    );
    for row in &analysis.rows {
        let text = row
            .spans
            .iter()
            .map(|span| span.decoded.as_str())
            .collect::<Vec<_>>()
            .join("|");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t0x{:08X}\t0x{:04X}\t\
             0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\
             preserve-no-write-until-control-layout-and-runtime-scene-contract\t{}",
            SUBGAME_SEQ_SCENE_TEXT_SCHEMA_VERSION,
            row.scope,
            row.archive,
            row.sequence,
            row.kind.label(),
            row.index,
            row.command_variant
                .map(|variant| variant.to_string())
                .unwrap_or_default(),
            row.pointer_field_offset,
            row.pointer_field_runtime,
            row.record_offset,
            row.record_runtime,
            hex(&row.raw_sha256),
            row.raw.len(),
            row.protected_bytes,
            row.terminators,
            row.spans.len(),
            row.code_units(),
            row.kana_candidate_overlaps,
            row.missed_sjis_spans,
            if row.kind == SubgameSeqSceneTextKind::Message {
                "shared-e0-message-consumer-static"
            } else {
                "scene-local-repertoire-static"
            },
            escape_tsv(&text),
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
#[path = "subgame_seq_scene_text_tests.rs"]
mod subgame_seq_scene_text_tests;
