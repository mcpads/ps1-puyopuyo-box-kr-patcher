//! BOX SEQ·GRP VDD의 CNX subtype 분모.
//!
//! CNX codec 자체는 subtype을 불투명한 3바이트로 보존한다. 이 모듈은
//! 고정 지원 원본에서 확인한 archive·확장자·subtype 대응을 별도 사양으로
//! 검증한다. 새 subtype이나 경로를 자동으로 받아들이지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::archive::VddArchive;

use super::{is_cnx, parse_header, CnxError};

/// VDD 하나의 CNX subtype 수.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveCnxInventory {
    pub path: &'static str,
    pub entries: usize,
    pub bin: usize,
    pub bi_: usize,
    pub bit: usize,
    pub cel: usize,
    pub raw: usize,
}

impl ArchiveCnxInventory {
    const fn empty(path: &'static str) -> Self {
        Self {
            path,
            entries: 0,
            bin: 0,
            bi_: 0,
            bit: 0,
            cel: 0,
            raw: 0,
        }
    }

    const fn cnx(self) -> usize {
        self.bin + self.bi_ + self.bit + self.cel
    }
}

/// 전체 SEQ·GRP VDD의 CNX subtype 수.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CnxInventorySummary {
    pub archives: usize,
    pub entries: usize,
    pub cnx: usize,
    pub bin: usize,
    pub bi_: usize,
    pub bit: usize,
    pub cel: usize,
    pub raw: usize,
}

/// 고정 지원 원본의 18개 SEQ·GRP VDD별 정확한 계약.
pub const BOX_CNX_ARCHIVES: [ArchiveCnxInventory; 18] = [
    ArchiveCnxInventory {
        path: "/FIL_GRP.VDD;1",
        entries: 11,
        bin: 0,
        bi_: 6,
        bit: 5,
        cel: 0,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/FIL_SEQ.VDD;1",
        entries: 1,
        bin: 1,
        bi_: 0,
        bit: 0,
        cel: 0,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/GOCHA1/TUU_GRP.VDD;1",
        entries: 168,
        bin: 0,
        bi_: 162,
        bit: 6,
        cel: 0,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/GOCHA1/TUU_SEQ.VDD;1",
        entries: 98,
        bin: 49,
        bi_: 0,
        bit: 0,
        cel: 49,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/GOCHA2/GOTY_GRP.VDD;1",
        entries: 17,
        bin: 0,
        bi_: 16,
        bit: 1,
        cel: 0,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/HORI2/HORI2GRP.VDD;1",
        entries: 12,
        bin: 0,
        bi_: 11,
        bit: 1,
        cel: 0,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/HORI4/HORI4GRP.VDD;1",
        entries: 12,
        bin: 0,
        bi_: 11,
        bit: 1,
        cel: 0,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/KOUSIKI/KOU_GRP.VDD;1",
        entries: 10,
        bin: 0,
        bi_: 9,
        bit: 1,
        cel: 0,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/KOUSIKI/KOU_SEQ.VDD;1",
        entries: 2,
        bin: 2,
        bi_: 0,
        bit: 0,
        cel: 0,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/MINPUYO/MIN_GRP.VDD;1",
        entries: 16,
        bin: 0,
        bi_: 15,
        bit: 1,
        cel: 0,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/PCOMI/PC1_GRP.VDD;1",
        entries: 15,
        bin: 2,
        bi_: 4,
        bit: 1,
        cel: 0,
        raw: 8,
    },
    ArchiveCnxInventory {
        path: "/PCOMI/PC1_SEQ.VDD;1",
        entries: 2,
        bin: 2,
        bi_: 0,
        bit: 0,
        cel: 0,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/PUYO1/PP1_GRP.VDD;1",
        entries: 143,
        bin: 0,
        bi_: 135,
        bit: 8,
        cel: 0,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/PUYO1/PP1_SEQ.VDD;1",
        entries: 136,
        bin: 119,
        bi_: 0,
        bit: 0,
        cel: 17,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/PUYO2/PP2_GRP.VDD;1",
        entries: 217,
        bin: 2,
        bi_: 208,
        bit: 7,
        cel: 0,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/PUYO2/PP2_SEQ.VDD;1",
        entries: 201,
        bin: 151,
        bi_: 0,
        bit: 0,
        cel: 50,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/QUEST/RPG_GRP.VDD;1",
        entries: 123,
        bin: 0,
        bi_: 108,
        bit: 15,
        cel: 0,
        raw: 0,
    },
    ArchiveCnxInventory {
        path: "/QUEST/RPG_SEQ.VDD;1",
        entries: 113,
        bin: 64,
        bi_: 0,
        bit: 0,
        cel: 49,
        raw: 0,
    },
];

/// 고정 지원 원본의 전체 CNX 분모.
pub const BOX_CNX_INVENTORY: CnxInventorySummary = CnxInventorySummary {
    archives: 18,
    entries: 1_297,
    cnx: 1_289,
    bin: 392,
    bi_: 685,
    bit: 47,
    cel: 165,
    raw: 8,
};

/// CNX 분모 검증 실패.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CnxInventoryError {
    EmptyInput,
    EmptyArchiveId,
    DuplicateArchiveId(String),
    CnxHeader {
        archive: String,
        name: String,
        source: CnxError,
    },
    UnknownSubtype {
        archive: String,
        name: String,
        subtype: [u8; 3],
    },
    ExtensionMismatch {
        archive: String,
        name: String,
        subtype: [u8; 3],
    },
    UnexpectedArchive(String),
    MissingArchive(&'static str),
    ArchiveMismatch {
        archive: String,
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    SummaryMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for CnxInventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "CNX 인벤토리에 VDD 아카이브가 없음"),
            Self::EmptyArchiveId => write!(f, "CNX 인벤토리의 VDD 식별자가 비어 있음"),
            Self::DuplicateArchiveId(id) => write!(f, "CNX 인벤토리 VDD 식별자 중복: {id}"),
            Self::CnxHeader {
                archive,
                name,
                source,
            } => write!(f, "{archive}:{name}: CNX 헤더 실패: {source}"),
            Self::UnknownSubtype {
                archive,
                name,
                subtype,
            } => write!(
                f,
                "{archive}:{name}: 미등록 CNX subtype {:02X} {:02X} {:02X}",
                subtype[0], subtype[1], subtype[2]
            ),
            Self::ExtensionMismatch {
                archive,
                name,
                subtype,
            } => write!(
                f,
                "{archive}:{name}: 확장자와 CNX subtype {:02X} {:02X} {:02X} 불일치",
                subtype[0], subtype[1], subtype[2]
            ),
            Self::UnexpectedArchive(archive) => {
                write!(f, "BOX CNX 인벤토리에 미등록 VDD: {archive}")
            }
            Self::MissingArchive(archive) => {
                write!(f, "BOX CNX 인벤토리에 필요한 VDD 누락: {archive}")
            }
            Self::ArchiveMismatch {
                archive,
                field,
                expected,
                actual,
            } => write!(
                f,
                "{archive}: CNX 인벤토리 {field} 불일치: expected={expected}, actual={actual}"
            ),
            Self::SummaryMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "BOX CNX 인벤토리 {field} 불일치: expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for CnxInventoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CnxHeader { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn extension_matches(name: &str, subtype: [u8; 3]) -> bool {
    let upper = name.to_ascii_uppercase();
    match &subtype {
        b"bin" => upper.ends_with(".SEQ") || upper.ends_with(".BX"),
        b"bi_" | b"bit" => upper.ends_with(".BTX"),
        b"cel" => upper.ends_with(".CEL"),
        _ => false,
    }
}

fn audit_archive(
    archive_id: &str,
    archive: &VddArchive,
) -> Result<ArchiveCnxInventory, CnxInventoryError> {
    let static_id = BOX_CNX_ARCHIVES
        .iter()
        .find(|spec| spec.path == archive_id)
        .map(|spec| spec.path)
        .unwrap_or("");
    let mut result = ArchiveCnxInventory::empty(static_id);
    result.entries = archive.entry_count();
    for entry in archive.entries() {
        let raw = archive.read_file(entry);
        if !is_cnx(raw) {
            result.raw += 1;
            continue;
        }
        let header = parse_header(raw).map_err(|source| CnxInventoryError::CnxHeader {
            archive: archive_id.to_string(),
            name: entry.name().to_string(),
            source,
        })?;
        if !matches!(&header.subtype, b"bin" | b"bi_" | b"bit" | b"cel") {
            return Err(CnxInventoryError::UnknownSubtype {
                archive: archive_id.to_string(),
                name: entry.name().to_string(),
                subtype: header.subtype,
            });
        }
        if !extension_matches(entry.name(), header.subtype) {
            return Err(CnxInventoryError::ExtensionMismatch {
                archive: archive_id.to_string(),
                name: entry.name().to_string(),
                subtype: header.subtype,
            });
        }
        match &header.subtype {
            b"bin" => result.bin += 1,
            b"bi_" => result.bi_ += 1,
            b"bit" => result.bit += 1,
            b"cel" => result.cel += 1,
            _ => unreachable!("위에서 등록된 subtype만 허용함"),
        }
    }
    Ok(result)
}

/// 여러 VDD에서 CNX subtype 분모를 계산한다.
pub fn audit_archives<'a>(
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
) -> Result<CnxInventorySummary, CnxInventoryError> {
    let mut ids = BTreeSet::new();
    let mut summary = CnxInventorySummary {
        archives: 0,
        entries: 0,
        cnx: 0,
        bin: 0,
        bi_: 0,
        bit: 0,
        cel: 0,
        raw: 0,
    };
    for (archive_id, archive) in archives {
        if archive_id.trim().is_empty() {
            return Err(CnxInventoryError::EmptyArchiveId);
        }
        if !ids.insert(archive_id.to_string()) {
            return Err(CnxInventoryError::DuplicateArchiveId(
                archive_id.to_string(),
            ));
        }
        let item = audit_archive(archive_id, archive)?;
        summary.archives += 1;
        summary.entries += item.entries;
        summary.cnx += item.cnx();
        summary.bin += item.bin;
        summary.bi_ += item.bi_;
        summary.bit += item.bit;
        summary.cel += item.cel;
        summary.raw += item.raw;
    }
    if summary.archives == 0 {
        return Err(CnxInventoryError::EmptyInput);
    }
    Ok(summary)
}

fn require_archive_field(
    archive: &str,
    field: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), CnxInventoryError> {
    if expected != actual {
        return Err(CnxInventoryError::ArchiveMismatch {
            archive: archive.to_string(),
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

fn require_summary_field(
    field: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), CnxInventoryError> {
    if expected != actual {
        return Err(CnxInventoryError::SummaryMismatch {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

/// 18개 archive별 분모와 전체 합계를 BOX 원본 계약에 대조한다.
pub fn verify_box_inventory<'a>(
    archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
) -> Result<CnxInventorySummary, CnxInventoryError> {
    let mut provided = BTreeMap::new();
    for (archive_id, archive) in archives {
        if provided.insert(archive_id, archive).is_some() {
            return Err(CnxInventoryError::DuplicateArchiveId(
                archive_id.to_string(),
            ));
        }
    }
    for archive_id in provided.keys() {
        if !BOX_CNX_ARCHIVES.iter().any(|spec| spec.path == *archive_id) {
            return Err(CnxInventoryError::UnexpectedArchive(
                (*archive_id).to_string(),
            ));
        }
    }

    for expected in BOX_CNX_ARCHIVES {
        let archive = provided
            .get(expected.path)
            .ok_or(CnxInventoryError::MissingArchive(expected.path))?;
        let actual = audit_archive(expected.path, archive)?;
        for (field, expected_value, actual_value) in [
            ("엔트리 수", expected.entries, actual.entries),
            ("bin 수", expected.bin, actual.bin),
            ("bi_ 수", expected.bi_, actual.bi_),
            ("bit 수", expected.bit, actual.bit),
            ("cel 수", expected.cel, actual.cel),
            ("raw 수", expected.raw, actual.raw),
        ] {
            require_archive_field(expected.path, field, expected_value, actual_value)?;
        }
    }

    let summary = audit_archives(provided)?;
    for (field, expected, actual) in [
        ("VDD 수", BOX_CNX_INVENTORY.archives, summary.archives),
        ("엔트리 수", BOX_CNX_INVENTORY.entries, summary.entries),
        ("CNX 수", BOX_CNX_INVENTORY.cnx, summary.cnx),
        ("bin 수", BOX_CNX_INVENTORY.bin, summary.bin),
        ("bi_ 수", BOX_CNX_INVENTORY.bi_, summary.bi_),
        ("bit 수", BOX_CNX_INVENTORY.bit, summary.bit),
        ("cel 수", BOX_CNX_INVENTORY.cel, summary.cel),
        ("raw 수", BOX_CNX_INVENTORY.raw, summary.raw),
    ] {
        require_summary_field(field, expected, actual)?;
    }
    Ok(summary)
}

#[cfg(test)]
#[path = "inventory_tests.rs"]
mod tests;
