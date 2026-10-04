//! 비-QUEST SEQ의 custom byte text 저장 구조.
//!
//! 가나 휴리스틱은 `PCOMI1.SEQ:0x4C0E`의 6바이트를 Shift-JIS `蔓吊ひ`로
//! 읽지만 이는 1-byte code 여섯 개를 우연히 2바이트씩 묶은 오독이다. 실제
//! owner는 root pointer가 가리키는 11개 고정 길이 NUL slot이고, 별도 root
//! pointer는 편집 화면의 1-byte repertoire를 가리킨다. 이 모듈은 root,
//! pointer table, slot array, repertoire와 후보 귀속을 원본 계약으로 고정한다.
//! 이 저장 모델은 code→glyph·consumer 의미를 소유하지 않는다. 별도
//! `disasm` gate가 consumer와 atlas를 검증하며, slot별 UX 의미와 플레이어
//! 표시 여부가 확인되기 전까지 쓰기는 금지한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::{
    SubgameSeqLayoutAnalysis, SubgameSeqLayoutKind, SubgameSeqTextCandidate,
    SubgameSeqTextCandidateAnalysis,
};

pub const SUBGAME_SEQ_CUSTOM_BYTE_RECORD_SCHEMA_VERSION: &str =
    "subgame-seq-custom-byte-storage-v1";
pub const BOX_SUBGAME_SEQ_CUSTOM_BYTE_RECORDS: usize = 1;
pub const BOX_SUBGAME_SEQ_CUSTOM_BYTE_SLOTS: usize = 11;
pub const BOX_SUBGAME_SEQ_CUSTOM_BYTE_SLOT_BYTES: usize = 0xD9;
pub const BOX_SUBGAME_SEQ_CUSTOM_BYTE_SLOT_PAYLOAD_BYTES: usize = 0xD8;
pub const BOX_SUBGAME_SEQ_CUSTOM_BYTE_REPERTOIRE_BYTES: usize = 0x150;
pub const BOX_SUBGAME_SEQ_CUSTOM_BYTE_REPERTOIRE_CODES: usize = 251;
pub const BOX_SUBGAME_SEQ_CUSTOM_BYTE_USED_CODES: usize = 179;
pub const BOX_SUBGAME_SEQ_CUSTOM_BYTE_CANDIDATE_ID: &str =
    "SUBGAME-SEQ-TEXT:PCOMI:PCOMI1.SEQ:0x4C0E";
pub const BOX_SUBGAME_SEQ_CUSTOM_BYTE_REPORT_SHA256: [u8; 32] = [
    0x72, 0x05, 0xDA, 0xAF, 0x0E, 0x46, 0xC1, 0x04, 0x09, 0x3D, 0xBE, 0x2E, 0x04, 0xB5, 0x59, 0xDF,
    0x02, 0x63, 0x32, 0xBA, 0x9D, 0xCE, 0x1C, 0x52, 0xBB, 0x95, 0xBF, 0x96, 0x74, 0xFD, 0xF5, 0x80,
];

#[derive(Debug, Clone, Copy)]
struct SubgameSeqCustomByteRecordSpec {
    scope: &'static str,
    archive: &'static str,
    sequence: &'static str,
    sequence_bytes: usize,
    sequence_sha256: [u8; 32],
    load_base: u32,
    initialized_len: usize,
    repertoire_root_field_offset: usize,
    repertoire_offset: usize,
    repertoire_bytes: usize,
    repertoire_sha256: [u8; 32],
    repertoire_codes: usize,
    slot_table_root_field_offset: usize,
    slot_table_offset: usize,
    slot_count: usize,
    slot_stride: usize,
    slot_payload_bytes: usize,
    slot_array_start_offset: usize,
    slot_array_end_offset: usize,
    slot_array_sha256: [u8; 32],
    used_codes: usize,
    target_slot_index: usize,
    pointer_field_offset: usize,
    next_pointer_field_offset: usize,
    record_start_offset: usize,
    record_end_offset: usize,
    record_sha256: [u8; 32],
    candidate_offset: usize,
    candidate_raw: &'static [u8],
    candidate_heuristic_sjis: &'static str,
    embedded_ascii_offset: usize,
    embedded_ascii: &'static [u8],
    known_ascii_literals: &'static [(usize, &'static [u8])],
}

const PCOMI1_KNOWN_ASCII_LITERALS: [(usize, &[u8]); 4] = [
    (0x4C35, b"USA"),
    (0x4DFE, b"HEY YO>"),
    (0x4ED7, b"2001; 7<20="),
    (0x4FE6, b"http:"),
];

const PCOMI1_CUSTOM_BYTE_RECORD: SubgameSeqCustomByteRecordSpec = SubgameSeqCustomByteRecordSpec {
    scope: "PCOMI",
    archive: "/PCOMI/PC1_SEQ.VDD;1",
    sequence: "PCOMI1.SEQ",
    sequence_bytes: 0x72C8,
    sequence_sha256: [
        0xFB, 0x55, 0x56, 0xB9, 0x8A, 0x57, 0x61, 0x89, 0x28, 0x23, 0x2B, 0xF4, 0xCD, 0xE8, 0x3C,
        0xFD, 0x03, 0x43, 0x2E, 0x61, 0x00, 0x6F, 0xB8, 0x0B, 0xD4, 0x0F, 0x3C, 0xE8, 0x74, 0x3F,
        0x9F, 0x8F,
    ],
    load_base: 0x801A_6270,
    initialized_len: 0x729C,
    repertoire_root_field_offset: 0x0000,
    repertoire_offset: 0x1ED0,
    repertoire_bytes: BOX_SUBGAME_SEQ_CUSTOM_BYTE_REPERTOIRE_BYTES,
    repertoire_sha256: [
        0x1A, 0xA5, 0x96, 0x2A, 0x7B, 0x2D, 0x5F, 0x60, 0x41, 0x5A, 0x6C, 0x20, 0x67, 0x64, 0x85,
        0x84, 0xC2, 0x06, 0xBC, 0xF2, 0x62, 0xFF, 0x35, 0x36, 0xCA, 0x7F, 0x5E, 0x6B, 0x70, 0x27,
        0x3B, 0xAC,
    ],
    repertoire_codes: BOX_SUBGAME_SEQ_CUSTOM_BYTE_REPERTOIRE_CODES,
    slot_table_root_field_offset: 0x002C,
    slot_table_offset: 0x2CD0,
    slot_count: BOX_SUBGAME_SEQ_CUSTOM_BYTE_SLOTS,
    slot_stride: BOX_SUBGAME_SEQ_CUSTOM_BYTE_SLOT_BYTES,
    slot_payload_bytes: BOX_SUBGAME_SEQ_CUSTOM_BYTE_SLOT_PAYLOAD_BYTES,
    slot_array_start_offset: 0x4736,
    slot_array_end_offset: 0x5089,
    slot_array_sha256: [
        0x01, 0x78, 0x95, 0x7B, 0xE3, 0x9C, 0x50, 0xAB, 0xAA, 0x55, 0xA4, 0x83, 0x77, 0xC2, 0x4D,
        0xB6, 0x1B, 0x47, 0x48, 0xB6, 0xFE, 0x9C, 0x15, 0xD6, 0x82, 0xD3, 0xC2, 0x91, 0x8B, 0xD2,
        0x52, 0x24,
    ],
    used_codes: BOX_SUBGAME_SEQ_CUSTOM_BYTE_USED_CODES,
    target_slot_index: 5,
    pointer_field_offset: 0x2CE4,
    next_pointer_field_offset: 0x2CE8,
    record_start_offset: 0x4B73,
    record_end_offset: 0x4C4C,
    record_sha256: [
        0x40, 0xF3, 0x8B, 0x42, 0x8B, 0xDF, 0xDE, 0x2E, 0x66, 0x25, 0x06, 0x6C, 0x78, 0x63, 0x3C,
        0x92, 0x26, 0xC6, 0x1F, 0xDB, 0x03, 0x35, 0xBC, 0xC8, 0x4C, 0xA0, 0x1E, 0xE6, 0x85, 0x8E,
        0xD3, 0x25,
    ],
    candidate_offset: 0x4C0E,
    candidate_raw: &[0x96, 0xA0, 0x92, 0xDD, 0x82, 0xD0],
    candidate_heuristic_sjis: "蔓吊ひ",
    embedded_ascii_offset: 0x4C35,
    embedded_ascii: b"USA",
    known_ascii_literals: &PCOMI1_KNOWN_ASCII_LITERALS,
};

const BOX_SUBGAME_SEQ_CUSTOM_BYTE_RECORD_SPECS: [SubgameSeqCustomByteRecordSpec; 1] =
    [PCOMI1_CUSTOM_BYTE_RECORD];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqCustomByteRecordRow {
    pub candidate_id: String,
    pub scope: &'static str,
    pub archive: String,
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub repertoire_root_field_offset: usize,
    pub repertoire_root_field_runtime: u32,
    pub repertoire_offset: usize,
    pub repertoire_runtime: u32,
    pub repertoire_bytes: usize,
    pub repertoire_sha256: [u8; 32],
    pub repertoire_codes: usize,
    pub slot_table_root_field_offset: usize,
    pub slot_table_root_field_runtime: u32,
    pub slot_table_offset: usize,
    pub slot_table_runtime: u32,
    pub slot_count: usize,
    pub slot_stride: usize,
    pub slot_payload_bytes: usize,
    pub slot_array_start_offset: usize,
    pub slot_array_start_runtime: u32,
    pub slot_array_end_offset: usize,
    pub slot_array_end_runtime: u32,
    pub slot_array_sha256: [u8; 32],
    pub used_codes: usize,
    pub target_slot_index: usize,
    pub pointer_field_offset: usize,
    pub pointer_field_runtime: u32,
    pub next_pointer_field_offset: usize,
    pub next_pointer_field_runtime: u32,
    pub record_start_offset: usize,
    pub record_start_runtime: u32,
    pub record_end_offset: usize,
    pub record_end_runtime: u32,
    pub record_sha256: [u8; 32],
    pub candidate_offset: usize,
    pub candidate_runtime: u32,
    pub candidate_delta: usize,
    pub candidate_raw: Vec<u8>,
    pub embedded_ascii_offset: usize,
    pub embedded_ascii: Vec<u8>,
    pub known_ascii_literals: usize,
}

impl SubgameSeqCustomByteRecordRow {
    pub fn owner_id(&self) -> String {
        format!(
            "{}:{}:CUSTOM-BYTE-RECORD@0x{:04X}",
            self.scope, self.sequence, self.record_start_offset
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqCustomByteRecordAnalysis {
    pub rows: Vec<SubgameSeqCustomByteRecordRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqCustomByteRecordError(String);

impl fmt::Display for SubgameSeqCustomByteRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqCustomByteRecordError {}

fn error(message: impl Into<String>) -> SubgameSeqCustomByteRecordError {
    SubgameSeqCustomByteRecordError(message.into())
}

fn read_u32(raw: &[u8], offset: usize) -> Result<u32, SubgameSeqCustomByteRecordError> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| error("custom byte pointer field 범위 overflow"))?;
    let bytes = raw.get(offset..end).ok_or_else(|| {
        error(format!(
            "custom byte pointer field가 원본 밖임: 0x{offset:04X}"
        ))
    })?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
}

fn find_candidate(
    candidates: &SubgameSeqTextCandidateAnalysis,
    spec: SubgameSeqCustomByteRecordSpec,
) -> Result<&SubgameSeqTextCandidate, SubgameSeqCustomByteRecordError> {
    let matches = candidates
        .candidates
        .iter()
        .filter(|candidate| {
            candidate.scope == spec.scope
                && candidate.archive == spec.archive
                && candidate.sequence == spec.sequence
                && candidate.offset == spec.candidate_offset
        })
        .collect::<Vec<_>>();
    let [candidate] = matches.as_slice() else {
        return Err(error(format!(
            "custom byte candidate 분모 불일치: {}:{}:0x{:04X} actual={}",
            spec.archive,
            spec.sequence,
            spec.candidate_offset,
            matches.len()
        )));
    };
    Ok(candidate)
}

fn audit_record(
    raw: &[u8],
    candidate: &SubgameSeqTextCandidate,
    spec: SubgameSeqCustomByteRecordSpec,
) -> Result<SubgameSeqCustomByteRecordRow, SubgameSeqCustomByteRecordError> {
    if raw.len() != spec.sequence_bytes || Sha256::digest(raw).as_slice() != spec.sequence_sha256 {
        return Err(error(format!(
            "{}:{} custom byte 원본 digest 불일치",
            spec.archive, spec.sequence
        )));
    }
    if candidate.sequence_sha256 != spec.sequence_sha256
        || candidate.raw != spec.candidate_raw
        || candidate.decoded.as_deref() != Some(spec.candidate_heuristic_sjis)
    {
        return Err(error(format!(
            "{} custom byte 후보 원문 불일치",
            candidate.id()
        )));
    }

    let runtime = |offset: usize| {
        spec.load_base
            .checked_add(u32::try_from(offset).map_err(|_| error("offset 변환 실패"))?)
            .ok_or_else(|| error("custom byte runtime 주소 overflow"))
    };

    if read_u32(raw, spec.repertoire_root_field_offset)? != runtime(spec.repertoire_offset)? {
        return Err(error("custom byte repertoire root pointer 불일치"));
    }
    let repertoire_end = spec
        .repertoire_offset
        .checked_add(spec.repertoire_bytes)
        .ok_or_else(|| error("custom byte repertoire 범위 overflow"))?;
    let repertoire = raw
        .get(spec.repertoire_offset..repertoire_end)
        .ok_or_else(|| error("custom byte repertoire가 원본 밖임"))?;
    let repertoire_codes = repertoire.iter().copied().collect::<BTreeSet<_>>();
    if repertoire.contains(&0)
        || raw.get(repertoire_end) != Some(&0)
        || Sha256::digest(repertoire).as_slice() != spec.repertoire_sha256
        || repertoire_codes.len() != spec.repertoire_codes
    {
        return Err(error(
            "custom byte repertoire digest, NUL 또는 code 분모 불일치",
        ));
    }

    if read_u32(raw, spec.slot_table_root_field_offset)? != runtime(spec.slot_table_offset)? {
        return Err(error("custom byte slot table root pointer 불일치"));
    }
    let expected_array_bytes = spec
        .slot_count
        .checked_mul(spec.slot_stride)
        .ok_or_else(|| error("custom byte slot array 범위 overflow"))?;
    if spec.slot_payload_bytes.checked_add(1) != Some(spec.slot_stride)
        || spec
            .slot_array_start_offset
            .checked_add(expected_array_bytes)
            != Some(spec.slot_array_end_offset)
    {
        return Err(error("custom byte slot 고정 길이 계약 불일치"));
    }
    let slot_array = raw
        .get(spec.slot_array_start_offset..spec.slot_array_end_offset)
        .ok_or_else(|| error("custom byte slot array가 원본 밖임"))?;
    if Sha256::digest(slot_array).as_slice() != spec.slot_array_sha256 {
        return Err(error("custom byte slot array digest 불일치"));
    }
    let mut used_codes = BTreeSet::new();
    for slot_index in 0..spec.slot_count {
        let slot_start = spec
            .slot_array_start_offset
            .checked_add(
                slot_index
                    .checked_mul(spec.slot_stride)
                    .ok_or_else(|| error("custom byte slot offset overflow"))?,
            )
            .ok_or_else(|| error("custom byte slot offset overflow"))?;
        let pointer_field = spec
            .slot_table_offset
            .checked_add(
                slot_index
                    .checked_mul(4)
                    .ok_or_else(|| error("custom byte slot pointer offset overflow"))?,
            )
            .ok_or_else(|| error("custom byte slot pointer offset overflow"))?;
        if read_u32(raw, pointer_field)? != runtime(slot_start)? {
            return Err(error(format!(
                "custom byte slot pointer 불일치: index={slot_index}"
            )));
        }
        let slot = raw
            .get(slot_start..slot_start + spec.slot_stride)
            .ok_or_else(|| error("custom byte slot이 원본 밖임"))?;
        if slot.iter().position(|&byte| byte == 0) != Some(spec.slot_payload_bytes) {
            return Err(error(format!(
                "custom byte slot NUL 위치 불일치: index={slot_index}"
            )));
        }
        used_codes.extend(slot[..spec.slot_payload_bytes].iter().copied());
    }
    if used_codes.len() != spec.used_codes || !used_codes.is_subset(&repertoire_codes) {
        return Err(error("custom byte slot code가 repertoire 계약과 불일치"));
    }
    if spec.pointer_field_offset != spec.slot_table_offset + spec.target_slot_index * 4
        || spec.record_start_offset
            != spec.slot_array_start_offset + spec.target_slot_index * spec.slot_stride
        || spec.record_end_offset != spec.record_start_offset + spec.slot_stride
    {
        return Err(error("custom byte 후보 slot 귀속 계약 불일치"));
    }

    let record = raw
        .get(spec.record_start_offset..spec.record_end_offset)
        .ok_or_else(|| error("custom byte record 범위가 원본 밖임"))?;
    if record.is_empty()
        || record.last() != Some(&0)
        || Sha256::digest(record).as_slice() != spec.record_sha256
    {
        return Err(error("custom byte record digest 또는 NUL 종결 불일치"));
    }
    let candidate_end = spec
        .candidate_offset
        .checked_add(spec.candidate_raw.len())
        .ok_or_else(|| error("custom byte candidate 범위 overflow"))?;
    if spec.candidate_offset < spec.record_start_offset
        || candidate_end > spec.record_end_offset
        || raw.get(spec.candidate_offset..candidate_end) != Some(spec.candidate_raw)
    {
        return Err(error("custom byte candidate가 record 안의 원문과 불일치"));
    }
    let ascii_end = spec
        .embedded_ascii_offset
        .checked_add(spec.embedded_ascii.len())
        .ok_or_else(|| error("custom byte ASCII 범위 overflow"))?;
    if spec.embedded_ascii_offset < spec.record_start_offset
        || ascii_end > spec.record_end_offset
        || raw.get(spec.embedded_ascii_offset..ascii_end) != Some(spec.embedded_ascii)
    {
        return Err(error("custom byte record의 embedded ASCII 불일치"));
    }
    for &(offset, literal) in spec.known_ascii_literals {
        let end = offset
            .checked_add(literal.len())
            .ok_or_else(|| error("custom byte ASCII literal 범위 overflow"))?;
        if offset < spec.slot_array_start_offset
            || end > spec.slot_array_end_offset
            || raw.get(offset..end) != Some(literal)
        {
            return Err(error(format!(
                "custom byte ASCII literal 불일치: offset=0x{offset:04X}"
            )));
        }
    }

    let expected_start = spec
        .load_base
        .checked_add(
            u32::try_from(spec.record_start_offset).map_err(|_| error("offset 변환 실패"))?,
        )
        .ok_or_else(|| error("custom byte record 시작 runtime overflow"))?;
    let expected_end = spec
        .load_base
        .checked_add(u32::try_from(spec.record_end_offset).map_err(|_| error("offset 변환 실패"))?)
        .ok_or_else(|| error("custom byte record 끝 runtime overflow"))?;
    if read_u32(raw, spec.pointer_field_offset)? != expected_start
        || read_u32(raw, spec.next_pointer_field_offset)? != expected_end
    {
        return Err(error("custom byte record pointer pair 불일치"));
    }
    Ok(SubgameSeqCustomByteRecordRow {
        candidate_id: candidate.id(),
        scope: spec.scope,
        archive: spec.archive.to_string(),
        sequence: spec.sequence.to_string(),
        sequence_sha256: spec.sequence_sha256,
        repertoire_root_field_offset: spec.repertoire_root_field_offset,
        repertoire_root_field_runtime: runtime(spec.repertoire_root_field_offset)?,
        repertoire_offset: spec.repertoire_offset,
        repertoire_runtime: runtime(spec.repertoire_offset)?,
        repertoire_bytes: spec.repertoire_bytes,
        repertoire_sha256: spec.repertoire_sha256,
        repertoire_codes: spec.repertoire_codes,
        slot_table_root_field_offset: spec.slot_table_root_field_offset,
        slot_table_root_field_runtime: runtime(spec.slot_table_root_field_offset)?,
        slot_table_offset: spec.slot_table_offset,
        slot_table_runtime: runtime(spec.slot_table_offset)?,
        slot_count: spec.slot_count,
        slot_stride: spec.slot_stride,
        slot_payload_bytes: spec.slot_payload_bytes,
        slot_array_start_offset: spec.slot_array_start_offset,
        slot_array_start_runtime: runtime(spec.slot_array_start_offset)?,
        slot_array_end_offset: spec.slot_array_end_offset,
        slot_array_end_runtime: runtime(spec.slot_array_end_offset)?,
        slot_array_sha256: spec.slot_array_sha256,
        used_codes: spec.used_codes,
        target_slot_index: spec.target_slot_index,
        pointer_field_offset: spec.pointer_field_offset,
        pointer_field_runtime: runtime(spec.pointer_field_offset)?,
        next_pointer_field_offset: spec.next_pointer_field_offset,
        next_pointer_field_runtime: runtime(spec.next_pointer_field_offset)?,
        record_start_offset: spec.record_start_offset,
        record_start_runtime: runtime(spec.record_start_offset)?,
        record_end_offset: spec.record_end_offset,
        record_end_runtime: runtime(spec.record_end_offset)?,
        record_sha256: spec.record_sha256,
        candidate_offset: spec.candidate_offset,
        candidate_runtime: runtime(spec.candidate_offset)?,
        candidate_delta: spec.candidate_offset - spec.record_start_offset,
        candidate_raw: spec.candidate_raw.to_vec(),
        embedded_ascii_offset: spec.embedded_ascii_offset,
        embedded_ascii: spec.embedded_ascii.to_vec(),
        known_ascii_literals: spec.known_ascii_literals.len(),
    })
}

fn audit_subgame_seq_custom_byte_records<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    candidates: &SubgameSeqTextCandidateAnalysis,
    layouts: &SubgameSeqLayoutAnalysis,
    specs: &[SubgameSeqCustomByteRecordSpec],
) -> Result<SubgameSeqCustomByteRecordAnalysis, SubgameSeqCustomByteRecordError> {
    let mut source_index = BTreeMap::new();
    for (archive, sequence, raw) in sources {
        if source_index.insert((archive, sequence), raw).is_some() {
            return Err(error(format!(
                "custom byte 원본이 중복됨: {archive}:{sequence}"
            )));
        }
    }
    let mut rows = Vec::with_capacity(specs.len());
    for &spec in specs {
        let raw = source_index
            .get(&(spec.archive, spec.sequence))
            .ok_or_else(|| {
                error(format!(
                    "custom byte 원본 없음: {}:{}",
                    spec.archive, spec.sequence
                ))
            })?;
        let layout = layouts
            .rows
            .iter()
            .find(|row| row.archive == spec.archive && row.sequence == spec.sequence)
            .ok_or_else(|| error("custom byte overlay layout 없음"))?;
        let footer = layout
            .footer
            .ok_or_else(|| error("custom byte source가 overlay가 아님"))?;
        if layout.kind != SubgameSeqLayoutKind::Overlay
            || layout.bytes != spec.sequence_bytes
            || layout.sequence_sha256 != spec.sequence_sha256
            || footer.load_base != spec.load_base
            || usize::try_from(footer.initialized_len).ok() != Some(spec.initialized_len)
            || spec.record_end_offset > spec.initialized_len
        {
            return Err(error("custom byte overlay 계약 불일치"));
        }
        rows.push(audit_record(raw, find_candidate(candidates, spec)?, spec)?);
    }
    rows.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    Ok(SubgameSeqCustomByteRecordAnalysis { rows })
}

/// 지원 원본에서 heuristic-SJIS 후보 한 건의 실제 custom byte 저장 owner를 검증한다.
pub fn verify_box_subgame_seq_custom_byte_records<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    candidates: &SubgameSeqTextCandidateAnalysis,
    layouts: &SubgameSeqLayoutAnalysis,
) -> Result<SubgameSeqCustomByteRecordAnalysis, SubgameSeqCustomByteRecordError> {
    let analysis = audit_subgame_seq_custom_byte_records(
        sources,
        candidates,
        layouts,
        &BOX_SUBGAME_SEQ_CUSTOM_BYTE_RECORD_SPECS,
    )?;
    if analysis.rows.len() != BOX_SUBGAME_SEQ_CUSTOM_BYTE_RECORDS
        || analysis.rows.first().map(|row| row.candidate_id.as_str())
            != Some(BOX_SUBGAME_SEQ_CUSTOM_BYTE_CANDIDATE_ID)
    {
        return Err(error("지원 원본 custom byte record 분모 또는 ID 불일치"));
    }
    let report = render_subgame_seq_custom_byte_records_tsv(&analysis);
    let actual: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if actual != BOX_SUBGAME_SEQ_CUSTOM_BYTE_REPORT_SHA256 {
        return Err(error(format!(
            "지원 원본 custom byte record TSV SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_CUSTOM_BYTE_REPORT_SHA256),
            hex(&actual)
        )));
    }
    Ok(analysis)
}

/// custom byte storage owner와 별도 consumer/atlas 소유 경계를 검토하는 TSV.
pub fn render_subgame_seq_custom_byte_records_tsv(
    analysis: &SubgameSeqCustomByteRecordAnalysis,
) -> String {
    let mut output = String::from(
        "schema_version\tcandidate_id\tscope\tarchive\tsequence\tsequence_sha256\t\
         repertoire_root_field_offset\trepertoire_root_field_runtime\trepertoire_offset\t\
         repertoire_runtime\trepertoire_bytes\trepertoire_sha256\trepertoire_codes\t\
         slot_table_root_field_offset\tslot_table_root_field_runtime\tslot_table_offset\t\
         slot_table_runtime\tslot_count\tslot_stride\tslot_payload_bytes\t\
         slot_array_start_offset\tslot_array_start_runtime\tslot_array_end_offset\t\
         slot_array_end_runtime\tslot_array_sha256\tused_codes\ttarget_slot_index\t\
         pointer_field_offset\tpointer_field_runtime\tnext_pointer_field_offset\t\
         next_pointer_field_runtime\trecord_start_offset\trecord_start_runtime\t\
         record_end_offset\trecord_end_runtime\trecord_bytes\trecord_sha256\t\
         candidate_offset\tcandidate_runtime\tcandidate_delta\tcandidate_raw\t\
         embedded_ascii_offset\tembedded_ascii\tknown_ascii_literals\tterminator\t\
         heuristic_sjis_interpretation\tconsumer_contract\tpatch_strategy\tstatus\n",
    );
    for row in &analysis.rows {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t0x{:08X}\t0x{:04X}\t0x{:08X}\t{}\t{}\t{}\t\
             0x{:04X}\t0x{:08X}\t0x{:04X}\t0x{:08X}\t{}\t{}\t{}\t0x{:04X}\t0x{:08X}\t\
             0x{:04X}\t0x{:08X}\t{}\t{}\t{}\t\
             0x{:04X}\t0x{:08X}\t0x{:04X}\t0x{:08X}\t\
             0x{:04X}\t0x{:08X}\t0x{:04X}\t0x{:08X}\t{}\t{}\t0x{:04X}\t0x{:08X}\t\
             {}\t{}\t0x{:04X}\t{}\t{}\tNUL\trejected-accidental-two-byte-pairing\t\
             owned-by-separate-consumer-and-atlas-gates\t\
             protected-no-write-until-runtime-meaning-and-human-approval\tanalysis-required",
            SUBGAME_SEQ_CUSTOM_BYTE_RECORD_SCHEMA_VERSION,
            row.candidate_id,
            row.scope,
            row.archive,
            row.sequence,
            hex(&row.sequence_sha256),
            row.repertoire_root_field_offset,
            row.repertoire_root_field_runtime,
            row.repertoire_offset,
            row.repertoire_runtime,
            row.repertoire_bytes,
            hex(&row.repertoire_sha256),
            row.repertoire_codes,
            row.slot_table_root_field_offset,
            row.slot_table_root_field_runtime,
            row.slot_table_offset,
            row.slot_table_runtime,
            row.slot_count,
            row.slot_stride,
            row.slot_payload_bytes,
            row.slot_array_start_offset,
            row.slot_array_start_runtime,
            row.slot_array_end_offset,
            row.slot_array_end_runtime,
            hex(&row.slot_array_sha256),
            row.used_codes,
            row.target_slot_index,
            row.pointer_field_offset,
            row.pointer_field_runtime,
            row.next_pointer_field_offset,
            row.next_pointer_field_runtime,
            row.record_start_offset,
            row.record_start_runtime,
            row.record_end_offset,
            row.record_end_runtime,
            row.record_end_offset - row.record_start_offset,
            hex(&row.record_sha256),
            row.candidate_offset,
            row.candidate_runtime,
            row.candidate_delta,
            hex(&row.candidate_raw),
            row.embedded_ascii_offset,
            String::from_utf8_lossy(&row.embedded_ascii),
            row.known_ascii_literals,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_custom_byte_record_tests.rs"]
mod tests;
