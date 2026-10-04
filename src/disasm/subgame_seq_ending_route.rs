//! PUYO2 결과·엔딩 SEQ의 canonical 정적 route 검증.
//!
//! 지원 원본의 `PP2_ALL`은 `PP2_TTL`의 credits export 3과 final ending
//! export 4를 호출한다. 각 export에는 `P2_END10.BTX`와
//! `P2_END15/16.BTX` setup이 들어 있다. 이 모듈은 그 source route만
//! 검증한다. setup 실행, VRAM upload, draw와 화면 표시는 별도 runtime
//! 증거가 소유한다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::compress::decompress;

pub const BOX_PUYO2_ENDING_ROUTE_ARCHIVE: &str = "/PUYO2/PP2_SEQ.VDD;1";
pub const BOX_PUYO2_ENDING_ROUTE_ARCHIVE_ENTRIES: usize = 201;

const PP2_ALL_INDEX: usize = 131;
const PP2_ALL_NAME: &str = "PP2_ALL.SEQ";
const PP2_ALL_LEN: usize = 4_472;
const PP2_ALL_SHA256: &str = "5d15a0a1d8e38c76756b1e4201cae7920e4efbcc0172e3a299f0552956252b40";

const PP2_TTL_INDEX: usize = 133;
const PP2_TTL_NAME: &str = "PP2_TTL.SEQ";
const PP2_TTL_LEN: usize = 18_440;
const PP2_TTL_SHA256: &str = "59a7d64838d17f2ca018a3c3f8d67978a60a23e23b7cf46385c1c232eb9f2951";
const PP2_TTL_RESOURCE: u16 = 0x0089;
const PP2_TTL_RUNTIME_BASE: u32 = 0x801B_5270;

const PP2_TTL_SELECTION_OFFSETS: [usize; 5] = [0x00AC, 0x01F4, 0x032C, 0x0374, 0x039C];
const PP2_TTL_CALLS: [(usize, u32); 4] = [(0x00D0, 0), (0x0344, 1), (0x038C, 4), (0x03B4, 3)];

const PP2_TTL_EXPORT_RECORDS: [usize; 5] = [0x00BC, 0x00CC, 0x00E8, 0x00F8, 0x0108];
const PP2_TTL_EXPORT_STREAMS: [usize; 5] = [0x0118, 0x0DC4, 0x1644, 0x331C, 0x4520];
const CREDITS_EXPORT_INDEX: u32 = 3;
const FINAL_ENDING_EXPORT_INDEX: u32 = 4;

const P2_END10_RESOURCE: u16 = 0x0047;
const P2_END10_SETUP_OFFSET: usize = 0x358C;
const P2_END10_TABLE_OFFSET: usize = 0x3AA8;
const P2_END10_UPLOAD_PARAMETER: u32 = 0x03C0;

const P2_END15_RESOURCE: u16 = 0x004C;
const P2_END15_SETUP_OFFSET: usize = 0x4570;
const P2_END15_TABLE_OFFSET: usize = 0x408C;
const P2_END15_UPLOAD_PARAMETER: u32 = 0x0180;

const P2_END16_RESOURCE: u16 = 0x004D;
const P2_END16_SETUP_OFFSET: usize = 0x45A0;
const P2_END16_TABLE_OFFSET: usize = 0x40C0;
const P2_END16_UPLOAD_PARAMETER: u32 = 0x01C0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2EndingRouteAnalysis {
    pub archive_entries: usize,
    pub pp2_all_bytes: usize,
    pub pp2_ttl_bytes: usize,
    pub pp2_ttl_selection_records: usize,
    pub pp2_ttl_call_records: usize,
    pub credits_export_index: u32,
    pub credits_export_offset: usize,
    pub final_ending_export_index: u32,
    pub final_ending_export_offset: usize,
    pub end10_resource: u16,
    pub end10_setup_offset: usize,
    pub end15_resource: u16,
    pub end15_setup_offset: usize,
    pub end16_resource: u16,
    pub end16_setup_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puyo2EndingRouteError(String);

impl fmt::Display for Puyo2EndingRouteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Puyo2EndingRouteError {}

fn error(message: impl Into<String>) -> Puyo2EndingRouteError {
    Puyo2EndingRouteError(message.into())
}

fn require_bytes(
    raw: &[u8],
    sequence: &str,
    offset: usize,
    expected: &[u8],
) -> Result<(), Puyo2EndingRouteError> {
    let actual = raw.get(offset..offset + expected.len()).ok_or_else(|| {
        error(format!(
            "{sequence} ending route 범위 이탈: +0x{offset:X}, bytes={}",
            expected.len()
        ))
    })?;
    if actual != expected {
        return Err(error(format!(
            "{sequence} ending route 불일치: +0x{offset:X}, expected={expected:02X?}, actual={actual:02X?}"
        )));
    }
    Ok(())
}

fn runtime_pointer(offset: usize) -> Result<[u8; 4], Puyo2EndingRouteError> {
    let offset = u32::try_from(offset).map_err(|_| error("PP2_TTL offset이 u32 범위를 넘음"))?;
    PP2_TTL_RUNTIME_BASE
        .checked_add(offset)
        .map(u32::to_le_bytes)
        .ok_or_else(|| error("PP2_TTL runtime pointer 계산 overflow"))
}

fn selection_record(flags: u8) -> [u8; 8] {
    [
        0x28,
        0x10,
        flags,
        0x01,
        PP2_TTL_RESOURCE as u8,
        (PP2_TTL_RESOURCE >> 8) as u8,
        0,
        0,
    ]
}

fn call_record(index: u32) -> [u8; 8] {
    let mut record = [0x39, 0x01, PP2_TTL_RESOURCE as u8, 0, 0, 0, 0, 0];
    record[4..8].copy_from_slice(&index.to_le_bytes());
    record
}

fn setup_record(
    resource: u16,
    table_offset: usize,
    upload_parameter: u32,
) -> Result<[u8; 48], Puyo2EndingRouteError> {
    let mut record = [0u8; 48];
    record[0..8].copy_from_slice(&[
        0x28,
        0x12,
        0x97,
        0,
        resource as u8,
        (resource >> 8) as u8,
        0,
        0,
    ]);
    record[12..16].copy_from_slice(&runtime_pointer(table_offset)?);
    record[16] = 0x28;
    record[24..32].copy_from_slice(&[
        0x28,
        0x12,
        0x88,
        0,
        resource as u8,
        (resource >> 8) as u8,
        0,
        0,
    ]);
    record[32..36].copy_from_slice(&upload_parameter.to_le_bytes());
    record[40] = 0x28;
    Ok(record)
}

fn count_aligned_records(raw: &[u8], expected: &[u8]) -> usize {
    if raw.len() < expected.len() {
        return 0;
    }
    (0..=raw.len() - expected.len())
        .step_by(4)
        .filter(|&offset| &raw[offset..offset + expected.len()] == expected)
        .count()
}

fn verify_route_sources(
    pp2_all: &[u8],
    pp2_ttl: &[u8],
) -> Result<Puyo2EndingRouteAnalysis, Puyo2EndingRouteError> {
    let expected_resource = u16::try_from(PP2_TTL_INDEX)
        .ok()
        .and_then(|index| index.checked_add(4))
        .ok_or_else(|| error("PP2_TTL resource 계산 overflow"))?;
    if PP2_TTL_RESOURCE != expected_resource {
        return Err(error(format!(
            "PP2_TTL VDD index/resource 계약 불일치: index={PP2_TTL_INDEX}, resource=0x{PP2_TTL_RESOURCE:04X}"
        )));
    }

    for (index, &offset) in PP2_TTL_SELECTION_OFFSETS.iter().enumerate() {
        let flags = if index < 2 { 0x97 } else { 0x17 };
        require_bytes(pp2_all, PP2_ALL_NAME, offset, &selection_record(flags))?;
    }
    let selection_count = count_aligned_records(pp2_all, &selection_record(0x97))
        + count_aligned_records(pp2_all, &selection_record(0x17));
    if selection_count != PP2_TTL_SELECTION_OFFSETS.len() {
        return Err(error(format!(
            "PP2_ALL PP2_TTL selection 분모 불일치: expected={}, actual={selection_count}",
            PP2_TTL_SELECTION_OFFSETS.len()
        )));
    }

    for &(offset, index) in &PP2_TTL_CALLS {
        require_bytes(pp2_all, PP2_ALL_NAME, offset, &call_record(index))?;
    }
    let calls = if pp2_all.len() < 8 {
        Vec::new()
    } else {
        (0..=pp2_all.len() - 8)
            .step_by(4)
            .filter_map(|offset| {
                let record = &pp2_all[offset..offset + 8];
                (record[0..4] == [0x39, 0x01, PP2_TTL_RESOURCE as u8, 0]).then(|| {
                    (
                        offset,
                        u32::from_le_bytes(record[4..8].try_into().expect("OP39 index word")),
                    )
                })
            })
            .collect::<Vec<_>>()
    };
    if calls != PP2_TTL_CALLS {
        return Err(error(format!(
            "PP2_ALL PP2_TTL call route 불일치: expected={PP2_TTL_CALLS:04X?}, actual={calls:04X?}"
        )));
    }
    let call_count = calls.len();

    for (index, (&record_offset, &stream_offset)) in PP2_TTL_EXPORT_RECORDS
        .iter()
        .zip(PP2_TTL_EXPORT_STREAMS.iter())
        .enumerate()
    {
        let export_offset = index * 8;
        let mut export = [0x03, 0, 0, 0, 0, 0, 0, 0];
        export[4..8].copy_from_slice(&runtime_pointer(record_offset)?);
        require_bytes(pp2_ttl, PP2_TTL_NAME, export_offset, &export)?;

        let mut target = [0x1C, 0x02, 0, 0, 0, 0, 0, 0];
        target[4..8].copy_from_slice(&runtime_pointer(stream_offset)?);
        require_bytes(pp2_ttl, PP2_TTL_NAME, record_offset, &target)?;
    }

    require_bytes(
        pp2_ttl,
        PP2_TTL_NAME,
        P2_END10_SETUP_OFFSET,
        &setup_record(
            P2_END10_RESOURCE,
            P2_END10_TABLE_OFFSET,
            P2_END10_UPLOAD_PARAMETER,
        )?,
    )?;
    require_bytes(
        pp2_ttl,
        PP2_TTL_NAME,
        P2_END15_SETUP_OFFSET,
        &setup_record(
            P2_END15_RESOURCE,
            P2_END15_TABLE_OFFSET,
            P2_END15_UPLOAD_PARAMETER,
        )?,
    )?;
    require_bytes(
        pp2_ttl,
        PP2_TTL_NAME,
        P2_END16_SETUP_OFFSET,
        &setup_record(
            P2_END16_RESOURCE,
            P2_END16_TABLE_OFFSET,
            P2_END16_UPLOAD_PARAMETER,
        )?,
    )?;

    let credits_export_offset = PP2_TTL_EXPORT_STREAMS[CREDITS_EXPORT_INDEX as usize];
    if !(credits_export_offset <= P2_END10_SETUP_OFFSET
        && P2_END10_SETUP_OFFSET < P2_END10_TABLE_OFFSET)
    {
        return Err(error(
            "PP2_TTL credits export와 END10 setup/table 순서 불일치",
        ));
    }

    let final_ending_export_offset = PP2_TTL_EXPORT_STREAMS[FINAL_ENDING_EXPORT_INDEX as usize];
    if !(final_ending_export_offset <= P2_END15_SETUP_OFFSET
        && P2_END15_SETUP_OFFSET < P2_END16_SETUP_OFFSET)
    {
        return Err(error(
            "PP2_TTL final ending export와 END15/16 setup 순서 불일치",
        ));
    }

    Ok(Puyo2EndingRouteAnalysis {
        archive_entries: BOX_PUYO2_ENDING_ROUTE_ARCHIVE_ENTRIES,
        pp2_all_bytes: pp2_all.len(),
        pp2_ttl_bytes: pp2_ttl.len(),
        pp2_ttl_selection_records: selection_count,
        pp2_ttl_call_records: call_count,
        credits_export_index: CREDITS_EXPORT_INDEX,
        credits_export_offset,
        final_ending_export_index: FINAL_ENDING_EXPORT_INDEX,
        final_ending_export_offset,
        end10_resource: P2_END10_RESOURCE,
        end10_setup_offset: P2_END10_SETUP_OFFSET,
        end15_resource: P2_END15_RESOURCE,
        end15_setup_offset: P2_END15_SETUP_OFFSET,
        end16_resource: P2_END16_RESOURCE,
        end16_setup_offset: P2_END16_SETUP_OFFSET,
    })
}

fn read_source(
    archive: &VddArchive,
    index: usize,
    name: &str,
    expected_len: usize,
    expected_sha256: &str,
) -> Result<Vec<u8>, Puyo2EndingRouteError> {
    let entry = archive
        .entries()
        .get(index)
        .ok_or_else(|| error(format!("{name} VDD index 범위 이탈: {index}")))?;
    if entry.name() != name {
        return Err(error(format!(
            "PUYO2 ending route VDD 이름 불일치: index={index}, expected={name}, actual={}",
            entry.name()
        )));
    }
    let raw = decompress(archive.read_file(entry))
        .map_err(|cause| error(format!("{name} CNX 해제 실패: {cause}")))?;
    if raw.len() != expected_len {
        return Err(error(format!(
            "{name} 해제 길이 불일치: expected={expected_len}, actual={}",
            raw.len()
        )));
    }
    let actual_sha256 = format!("{:x}", Sha256::digest(&raw));
    if actual_sha256 != expected_sha256 {
        return Err(error(format!(
            "{name} SHA-256 불일치: expected={expected_sha256}, actual={actual_sha256}"
        )));
    }
    Ok(raw)
}

/// 지원 원본의 canonical `PP2_ALL → PP2_TTL export 3/4 → END10/15/16` route를 검증한다.
pub fn verify_box_puyo2_ending_route(
    archive: &VddArchive,
) -> Result<Puyo2EndingRouteAnalysis, Puyo2EndingRouteError> {
    if archive.entry_count() != BOX_PUYO2_ENDING_ROUTE_ARCHIVE_ENTRIES {
        return Err(error(format!(
            "{} 엔트리 수 불일치: expected={}, actual={}",
            BOX_PUYO2_ENDING_ROUTE_ARCHIVE,
            BOX_PUYO2_ENDING_ROUTE_ARCHIVE_ENTRIES,
            archive.entry_count()
        )));
    }
    let pp2_all = read_source(
        archive,
        PP2_ALL_INDEX,
        PP2_ALL_NAME,
        PP2_ALL_LEN,
        PP2_ALL_SHA256,
    )?;
    let pp2_ttl = read_source(
        archive,
        PP2_TTL_INDEX,
        PP2_TTL_NAME,
        PP2_TTL_LEN,
        PP2_TTL_SHA256,
    )?;
    verify_route_sources(&pp2_all, &pp2_ttl)
}

#[cfg(test)]
#[path = "subgame_seq_ending_route_tests.rs"]
mod subgame_seq_ending_route_tests;
