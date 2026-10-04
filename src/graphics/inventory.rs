//! BOX GRP 아카이브의 BTX 구조 분모 검증.
//!
//! 사람의 픽셀 텍스트 판정은 `docs/btx_catalog.md`가 소유한다. 이 모듈은
//! Rust 제품 경로가 의존할 구조 사실만 검증한다: GRP·엔트리·BTX 수,
//! CNX subtype별 픽셀 형식, 해제 성공과 고유 해제 내용 수다.

use std::collections::{BTreeSet, HashSet};
use std::error::Error;
use std::fmt;

use crate::archive::VddArchive;
use crate::compress::{decompress, is_cnx, parse_header, CnxError};

use crate::btx::PixelFormat;

/// BTX 구조 분모.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventorySummary {
    pub archives: usize,
    pub entries: usize,
    pub physical_btx: usize,
    pub indexed4: usize,
    pub indexed8: usize,
    pub unique_content: usize,
    pub other_entries: usize,
}

/// 고정 지원 원본에서 기대하는 BTX 구조 분모.
pub const BOX_EXPECTED_INVENTORY: InventorySummary = InventorySummary {
    archives: 11,
    entries: 744,
    physical_btx: 732,
    indexed4: 685,
    indexed8: 47,
    unique_content: 475,
    other_entries: 12,
};

/// BTX 인벤토리 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryError {
    EmptyInput,
    EmptyArchiveId,
    DuplicateArchiveId(String),
    NonCnxBtx {
        archive: String,
        name: String,
    },
    CnxHeader {
        archive: String,
        name: String,
        source: CnxError,
    },
    UnsupportedFormat {
        archive: String,
        name: String,
        subtype: [u8; 3],
    },
    CnxDecompress {
        archive: String,
        name: String,
        source: CnxError,
    },
    EmptyPixels {
        archive: String,
        name: String,
    },
    SummaryMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for InventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "BTX 인벤토리에 GRP 아카이브가 없음"),
            Self::EmptyArchiveId => write!(f, "BTX 인벤토리의 GRP 식별자가 비어 있음"),
            Self::DuplicateArchiveId(id) => {
                write!(f, "BTX 인벤토리의 GRP 식별자 중복: {id}")
            }
            Self::NonCnxBtx { archive, name } => {
                write!(f, "{archive}:{name}: BTX가 CNX v2가 아님")
            }
            Self::CnxHeader {
                archive,
                name,
                source,
            } => write!(f, "{archive}:{name}: CNX 헤더 실패: {source}"),
            Self::UnsupportedFormat {
                archive,
                name,
                subtype,
            } => write!(
                f,
                "{archive}:{name}: BTX에서 지원하지 않는 CNX subtype {:02X} {:02X} {:02X}",
                subtype[0], subtype[1], subtype[2]
            ),
            Self::CnxDecompress {
                archive,
                name,
                source,
            } => write!(f, "{archive}:{name}: CNX 해제 실패: {source}"),
            Self::EmptyPixels { archive, name } => {
                write!(f, "{archive}:{name}: BTX 해제 픽셀이 비어 있음")
            }
            Self::SummaryMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "BTX 인벤토리 {field} 불일치: expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for InventoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CnxHeader { source, .. } | Self::CnxDecompress { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// 여러 GRP VDD의 BTX 구조 분모를 계산한다.
///
/// 같은 archive ID 중복, raw BTX, 알 수 없는 subtype, CNX 해제 실패와 빈
/// 픽셀은 일부 집계를 반환하지 않고 오류로 처리한다.
pub fn audit_archives<'a>(
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
) -> Result<InventorySummary, InventoryError> {
    let mut archive_ids = BTreeSet::new();
    let mut unique_content = HashSet::new();
    let mut summary = InventorySummary {
        archives: 0,
        entries: 0,
        physical_btx: 0,
        indexed4: 0,
        indexed8: 0,
        unique_content: 0,
        other_entries: 0,
    };

    for (archive_id, archive) in archives {
        if archive_id.trim().is_empty() {
            return Err(InventoryError::EmptyArchiveId);
        }
        if !archive_ids.insert(archive_id.to_string()) {
            return Err(InventoryError::DuplicateArchiveId(archive_id.to_string()));
        }
        summary.archives += 1;
        summary.entries += archive.entry_count();

        for entry in archive.entries() {
            if !entry.name().to_ascii_uppercase().ends_with(".BTX") {
                summary.other_entries += 1;
                continue;
            }

            let raw = archive.read_file(entry);
            if !is_cnx(raw) {
                return Err(InventoryError::NonCnxBtx {
                    archive: archive_id.to_string(),
                    name: entry.name().to_string(),
                });
            }
            let header = parse_header(raw).map_err(|source| InventoryError::CnxHeader {
                archive: archive_id.to_string(),
                name: entry.name().to_string(),
                source,
            })?;
            let format = PixelFormat::from_cnx_subtype(header.subtype).map_err(|_| {
                InventoryError::UnsupportedFormat {
                    archive: archive_id.to_string(),
                    name: entry.name().to_string(),
                    subtype: header.subtype,
                }
            })?;
            let pixels = decompress(raw).map_err(|source| InventoryError::CnxDecompress {
                archive: archive_id.to_string(),
                name: entry.name().to_string(),
                source,
            })?;
            if pixels.is_empty() {
                return Err(InventoryError::EmptyPixels {
                    archive: archive_id.to_string(),
                    name: entry.name().to_string(),
                });
            }

            summary.physical_btx += 1;
            match format {
                PixelFormat::Indexed4 => summary.indexed4 += 1,
                PixelFormat::Indexed8 => summary.indexed8 += 1,
            }
            unique_content.insert((format, pixels));
        }
    }

    if summary.archives == 0 {
        return Err(InventoryError::EmptyInput);
    }
    summary.unique_content = unique_content.len();
    Ok(summary)
}

fn require_field(
    field: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), InventoryError> {
    if expected != actual {
        return Err(InventoryError::SummaryMismatch {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

/// 계산한 분모를 고정 지원 원본의 BOX 계약과 대조한다.
pub fn verify_box_inventory(summary: InventorySummary) -> Result<(), InventoryError> {
    require_field(
        "GRP 아카이브 수",
        BOX_EXPECTED_INVENTORY.archives,
        summary.archives,
    )?;
    require_field(
        "전체 엔트리 수",
        BOX_EXPECTED_INVENTORY.entries,
        summary.entries,
    )?;
    require_field(
        "물리 BTX 수",
        BOX_EXPECTED_INVENTORY.physical_btx,
        summary.physical_btx,
    )?;
    require_field(
        "indexed 4bpp 수",
        BOX_EXPECTED_INVENTORY.indexed4,
        summary.indexed4,
    )?;
    require_field(
        "indexed 8bpp 수",
        BOX_EXPECTED_INVENTORY.indexed8,
        summary.indexed8,
    )?;
    require_field(
        "고유 해제 내용 수",
        BOX_EXPECTED_INVENTORY.unique_content,
        summary.unique_content,
    )?;
    require_field(
        "비 BTX 엔트리 수",
        BOX_EXPECTED_INVENTORY.other_entries,
        summary.other_entries,
    )
}

#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
