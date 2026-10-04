//! 개발 CNX 한 엔트리의 불변 원본 결속과 재압축 경계.
//!
//! BTX와 SEQ처럼 VDD 안에서 CNX로 저장되는 자산은 이 경로에서 원본 subtype,
//! plain 크기, 실제 변경과 재압축 readback을 공통 검증한다. VDD 안의 여러
//! 엔트리를 합치는 책임은 `cnx_development_archive_replacement`가 소유한다.

use std::error::Error;
use std::fmt;

use crate::archive::VddArchive;
use crate::compress::{compress, decompress, parse_header};
use crate::disc::{Iso9660, VerifiedSourceImage};

/// 한 CNX 엔트리의 불변 archive 위치와 Expected Write 소유자.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CnxDevelopmentEntrySpec {
    pub iso_path: &'static str,
    pub entry_name: &'static str,
    pub expected_cnx_subtype: [u8; 3],
    pub patch_id: &'static str,
    pub actor: &'static str,
}

/// plain 합성과 재압축을 마친 한 엔트리의 저장 판정.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CnxDevelopmentEntryStorageReport {
    pub original_cnx_bytes: usize,
    pub replacement_cnx_bytes: usize,
    pub plain_bytes: usize,
    pub cnx_roundtrip_exact: bool,
}

/// 불변 원본 VDD에 결속된 한 CNX replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreparedCnxDevelopmentEntry {
    pub(super) spec: CnxDevelopmentEntrySpec,
    pub(super) original_vdd: Vec<u8>,
    pub(super) replacement_cnx: Vec<u8>,
    pub(super) patched_plain: Vec<u8>,
    pub storage: CnxDevelopmentEntryStorageReport,
}

impl PreparedCnxDevelopmentEntry {
    pub fn patched_plain(&self) -> &[u8] {
        &self.patched_plain
    }
}

/// 원본·CNX·plain 변환 경계 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CnxDevelopmentEntryError(String);

impl fmt::Display for CnxDevelopmentEntryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for CnxDevelopmentEntryError {}

fn error(message: impl Into<String>) -> CnxDevelopmentEntryError {
    CnxDevelopmentEntryError(message.into())
}

/// 지원 원본의 한 CNX를 자산별 함수로 합성하고 재압축 결과를 준비한다.
pub(super) fn prepare_cnx_development_entry<R>(
    source: &VerifiedSourceImage,
    spec: CnxDevelopmentEntrySpec,
    patch_plain: impl FnOnce(&[u8]) -> Result<(Vec<u8>, R), String>,
) -> Result<(PreparedCnxDevelopmentEntry, R), CnxDevelopmentEntryError> {
    let iso = Iso9660::parse(source.image()).map_err(|source| error(source.to_string()))?;
    let original_vdd = iso
        .read_file(spec.iso_path)
        .map_err(|source| error(source.to_string()))?;
    prepare_cnx_development_entry_from_vdd(spec, original_vdd, patch_plain)
}

/// 이미 한 번 읽은 불변 VDD에서 CNX 엔트리 합성과 재압축을 준비한다.
pub(super) fn prepare_cnx_development_entry_from_vdd<R>(
    spec: CnxDevelopmentEntrySpec,
    original_vdd: Vec<u8>,
    patch_plain: impl FnOnce(&[u8]) -> Result<(Vec<u8>, R), String>,
) -> Result<(PreparedCnxDevelopmentEntry, R), CnxDevelopmentEntryError> {
    let archive = VddArchive::parse(&original_vdd).map_err(|source| error(source.to_string()))?;
    let entry = archive
        .find(spec.entry_name)
        .ok_or_else(|| error(format!("{}에 {}가 없음", spec.iso_path, spec.entry_name)))?;
    let original_cnx = archive.read_file(entry);
    let header = parse_header(original_cnx).map_err(|source| error(source.to_string()))?;
    if header.subtype != spec.expected_cnx_subtype {
        return Err(error(format!(
            "{}:{} CNX subtype 불일치: expected={:?}, actual={:?}",
            spec.iso_path, spec.entry_name, spec.expected_cnx_subtype, header.subtype
        )));
    }
    let original_plain = decompress(original_cnx).map_err(|source| error(source.to_string()))?;
    let (patched_plain, patch_report) = patch_plain(&original_plain).map_err(error)?;
    if patched_plain.len() != original_plain.len() {
        return Err(error(format!(
            "{} plain 크기가 바뀜: original={}, replacement={}",
            spec.entry_name,
            original_plain.len(),
            patched_plain.len()
        )));
    }
    if patched_plain == original_plain {
        return Err(error(format!(
            "{} 개발 합성 결과가 원본과 같음",
            spec.entry_name
        )));
    }
    let replacement_cnx =
        compress(&patched_plain, header.subtype).map_err(|source| error(source.to_string()))?;
    let cnx_roundtrip_exact =
        decompress(&replacement_cnx).map_err(|source| error(source.to_string()))? == patched_plain;
    let storage = CnxDevelopmentEntryStorageReport {
        original_cnx_bytes: original_cnx.len(),
        replacement_cnx_bytes: replacement_cnx.len(),
        plain_bytes: patched_plain.len(),
        cnx_roundtrip_exact,
    };
    if storage.original_cnx_bytes == 0
        || storage.replacement_cnx_bytes == 0
        || storage.plain_bytes == 0
        || !storage.cnx_roundtrip_exact
    {
        return Err(error(format!(
            "{} 개발 entry 저장 판정 drift",
            spec.entry_name
        )));
    }
    Ok((
        PreparedCnxDevelopmentEntry {
            spec,
            original_vdd,
            replacement_cnx,
            patched_plain,
            storage,
        },
        patch_report,
    ))
}

#[cfg(test)]
#[path = "cnx_development_entry_tests.rs"]
mod tests;
