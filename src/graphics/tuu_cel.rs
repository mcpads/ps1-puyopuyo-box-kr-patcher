//! `TUU_SEQ.VDD`의 CEL 물리 레코드.
//!
//! 지원 원본의 CEL 49개는 모두 CNX `cel` 해제 결과가 20바이트 단위로
//! 닫히며, 각 레코드는 u16 LE 값 10개다. 값의 의미는 소비 코드로 아직
//! 확정하지 않았으므로 이 모듈은 물리 경계와 원본 왕복만 소유한다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::archive::VddArchive;
use crate::compress::{decompress, parse_header, CnxError};

pub const TUU_SEQ_PATH: &str = "/GOCHA1/TUU_SEQ.VDD;1";
const RECORD_FIELDS: usize = 10;
const RECORD_LEN: usize = RECORD_FIELDS * 2;
const MAX_VALUE: u16 = 0x01FF;

/// CEL의 20바이트 물리 레코드.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TuuCelRecord {
    values: [u16; RECORD_FIELDS],
}

impl TuuCelRecord {
    pub const fn values(&self) -> &[u16; RECORD_FIELDS] {
        &self.values
    }
}

/// CEL 파일 하나의 검증된 레코드 배열.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TuuCelTable {
    records: Vec<TuuCelRecord>,
}

impl TuuCelTable {
    /// 비어 있거나 20바이트로 나누어지지 않는 입력을 거부한다.
    pub fn parse(raw: &[u8]) -> Result<Self, TuuCelError> {
        if raw.is_empty() {
            return Err(TuuCelError::Empty);
        }
        if !raw.len().is_multiple_of(RECORD_LEN) {
            return Err(TuuCelError::LengthNotMultiple {
                actual: raw.len(),
                record_len: RECORD_LEN,
            });
        }

        let records = raw
            .chunks_exact(RECORD_LEN)
            .map(|chunk| {
                let mut values = [0u16; RECORD_FIELDS];
                for (index, pair) in chunk.chunks_exact(2).enumerate() {
                    values[index] = u16::from_le_bytes([pair[0], pair[1]]);
                }
                TuuCelRecord { values }
            })
            .collect();
        Ok(Self { records })
    }

    pub fn records(&self) -> &[TuuCelRecord] {
        &self.records
    }

    /// 해석하지 않은 각 u16 값을 그대로 직렬화한다.
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.records.len() * RECORD_LEN);
        for record in &self.records {
            for value in record.values {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
        out
    }
}

/// CEL 물리 경계 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuuCelError {
    Empty,
    LengthNotMultiple { actual: usize, record_len: usize },
}

impl fmt::Display for TuuCelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "TUU CEL 레코드가 비어 있음"),
            Self::LengthNotMultiple { actual, record_len } => write!(
                f,
                "TUU CEL 길이가 레코드 경계와 불일치: actual={actual}, record={record_len}"
            ),
        }
    }
}

impl Error for TuuCelError {}

/// `TUU_SEQ.VDD` 전수 구조 요약.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TuuCelInventorySummary {
    pub entries: usize,
    pub cel_tables: usize,
    pub seq_entries: usize,
    pub records: usize,
    pub unique_values: usize,
    pub maximum_value: u16,
}

pub const BOX_EXPECTED_TUU_CEL_INVENTORY: TuuCelInventorySummary = TuuCelInventorySummary {
    entries: 98,
    cel_tables: 49,
    seq_entries: 49,
    records: 2_206,
    unique_values: 512,
    maximum_value: MAX_VALUE,
};

/// `TUU_SEQ.VDD` 전수 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuuCelInventoryError {
    UnsupportedEntry(String),
    Cnx {
        name: String,
        source: CnxError,
    },
    Subtype {
        name: String,
        expected: [u8; 3],
        actual: [u8; 3],
    },
    Cel {
        name: String,
        source: TuuCelError,
    },
    RoundtripMismatch(String),
    ValueOutOfRange {
        name: String,
        record: usize,
        field: usize,
        value: u16,
    },
    MissingPair {
        extension: &'static str,
        stem: String,
    },
    SummaryMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for TuuCelInventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedEntry(name) => {
                write!(f, "TUU_SEQ에 지원하지 않는 엔트리: {name}")
            }
            Self::Cnx { name, source } => write!(f, "{name}: {source}"),
            Self::Subtype {
                name,
                expected,
                actual,
            } => write!(
                f,
                "{name}: CNX subtype 불일치: expected={}, actual={}",
                String::from_utf8_lossy(expected),
                String::from_utf8_lossy(actual)
            ),
            Self::Cel { name, source } => write!(f, "{name}: {source}"),
            Self::RoundtripMismatch(name) => {
                write!(f, "{name}: CEL 직렬화가 해제 원본과 다름")
            }
            Self::ValueOutOfRange {
                name,
                record,
                field,
                value,
            } => write!(
                f,
                "{name}: CEL 값 범위 이탈: record={record}, field={field}, value=0x{value:04X}"
            ),
            Self::MissingPair { extension, stem } => {
                write!(f, "TUU_SEQ {stem}.{extension} 짝이 없음")
            }
            Self::SummaryMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "TUU CEL {field} 불일치: expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for TuuCelInventoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cnx { source, .. } => Some(source),
            Self::Cel { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// 지원 원본 `TUU_SEQ.VDD`의 CEL/SEQ 쌍과 모든 CEL 물리 레코드를 검증한다.
pub fn verify_box_tuu_cel_inventory(
    archive: &VddArchive,
) -> Result<TuuCelInventorySummary, TuuCelInventoryError> {
    let mut cel_stems = BTreeSet::new();
    let mut seq_stems = BTreeSet::new();
    let mut values = BTreeSet::new();
    let mut records = 0usize;

    for entry in archive.entries() {
        let name = entry.name().to_ascii_uppercase();
        let (stem, extension) = name
            .rsplit_once('.')
            .ok_or_else(|| TuuCelInventoryError::UnsupportedEntry(name.clone()))?;
        let compressed = archive.read_file(entry);
        let header = parse_header(compressed).map_err(|source| TuuCelInventoryError::Cnx {
            name: name.clone(),
            source,
        })?;
        match extension {
            "CEL" => {
                require_subtype(&name, b"cel", header.subtype)?;
                let raw = decompress(compressed).map_err(|source| TuuCelInventoryError::Cnx {
                    name: name.clone(),
                    source,
                })?;
                let table =
                    TuuCelTable::parse(&raw).map_err(|source| TuuCelInventoryError::Cel {
                        name: name.clone(),
                        source,
                    })?;
                if table.serialize() != raw {
                    return Err(TuuCelInventoryError::RoundtripMismatch(name));
                }
                for (record_index, record) in table.records().iter().enumerate() {
                    for (field_index, &value) in record.values().iter().enumerate() {
                        if value > MAX_VALUE {
                            return Err(TuuCelInventoryError::ValueOutOfRange {
                                name,
                                record: record_index,
                                field: field_index,
                                value,
                            });
                        }
                        values.insert(value);
                    }
                }
                records += table.records().len();
                cel_stems.insert(stem.to_string());
            }
            "SEQ" => {
                require_subtype(&name, b"bin", header.subtype)?;
                seq_stems.insert(stem.to_string());
            }
            _ => return Err(TuuCelInventoryError::UnsupportedEntry(name)),
        }
    }

    if let Some(stem) = cel_stems.difference(&seq_stems).next() {
        return Err(TuuCelInventoryError::MissingPair {
            extension: "SEQ",
            stem: stem.clone(),
        });
    }
    if let Some(stem) = seq_stems.difference(&cel_stems).next() {
        return Err(TuuCelInventoryError::MissingPair {
            extension: "CEL",
            stem: stem.clone(),
        });
    }

    let summary = TuuCelInventorySummary {
        entries: archive.entry_count(),
        cel_tables: cel_stems.len(),
        seq_entries: seq_stems.len(),
        records,
        unique_values: values.len(),
        maximum_value: values.last().copied().unwrap_or(0),
    };
    require_summary(
        "엔트리 수",
        BOX_EXPECTED_TUU_CEL_INVENTORY.entries,
        summary.entries,
    )?;
    require_summary(
        "CEL 수",
        BOX_EXPECTED_TUU_CEL_INVENTORY.cel_tables,
        summary.cel_tables,
    )?;
    require_summary(
        "SEQ 수",
        BOX_EXPECTED_TUU_CEL_INVENTORY.seq_entries,
        summary.seq_entries,
    )?;
    require_summary(
        "레코드 수",
        BOX_EXPECTED_TUU_CEL_INVENTORY.records,
        summary.records,
    )?;
    require_summary(
        "고유 값 수",
        BOX_EXPECTED_TUU_CEL_INVENTORY.unique_values,
        summary.unique_values,
    )?;
    require_summary(
        "최대 값",
        BOX_EXPECTED_TUU_CEL_INVENTORY.maximum_value as usize,
        summary.maximum_value as usize,
    )?;
    Ok(summary)
}

fn require_subtype(
    name: &str,
    expected: &[u8; 3],
    actual: [u8; 3],
) -> Result<(), TuuCelInventoryError> {
    if &actual != expected {
        return Err(TuuCelInventoryError::Subtype {
            name: name.to_string(),
            expected: *expected,
            actual,
        });
    }
    Ok(())
}

fn require_summary(
    field: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), TuuCelInventoryError> {
    if expected != actual {
        return Err(TuuCelInventoryError::SummaryMismatch {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "tuu_cel_tests.rs"]
mod tuu_cel_tests;
