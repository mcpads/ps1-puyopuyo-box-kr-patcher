//! QUEST `QMP` tile map.
//!
//! 파일 전체는 20바이트 header와 `width × height`개의 u16 LE tile ID다.
//! 별도 문자열 section은 없다. tile ID의 바이트가 우연히 Shift-JIS처럼
//! 보이는 것은 표시 텍스트 후보로 세지 않는다.

use std::error::Error;
use std::fmt;

use crate::archive::VddArchive;

const MAGIC: &[u8; 8] = b"PBXQMAP\0";
/// `PBXQMAP` header 길이. runtime loader도 cell payload를 이만큼 건너뛴다.
pub const QMP_HEADER_LEN: usize = 20;

/// QMP tile map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QmpMap {
    width: u32,
    height: u32,
    cells: Vec<u16>,
}

impl QmpMap {
    /// header와 전체 cell payload 길이를 검증한다.
    pub fn parse(raw: &[u8]) -> Result<Self, QmpError> {
        if raw.len() < QMP_HEADER_LEN {
            return Err(QmpError::Truncated(raw.len()));
        }
        if &raw[..8] != MAGIC {
            return Err(QmpError::InvalidMagic);
        }
        let header_len = read_u32(raw, 8);
        if header_len != QMP_HEADER_LEN as u32 {
            return Err(QmpError::InvalidHeaderLength(header_len));
        }
        let width = read_u32(raw, 12);
        let height = read_u32(raw, 16);
        if width == 0 || height == 0 {
            return Err(QmpError::EmptyDimensions { width, height });
        }
        let cell_count = width
            .checked_mul(height)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or(QmpError::SizeOverflow)?;
        let expected = cell_count
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(QMP_HEADER_LEN))
            .ok_or(QmpError::SizeOverflow)?;
        if raw.len() != expected {
            return Err(QmpError::LengthMismatch {
                expected,
                actual: raw.len(),
            });
        }
        let cells = raw[QMP_HEADER_LEN..]
            .chunks_exact(2)
            .map(|cell| u16::from_le_bytes([cell[0], cell[1]]))
            .collect();
        Ok(Self {
            width,
            height,
            cells,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn cells(&self) -> &[u16] {
        &self.cells
    }

    /// 검증된 구조를 canonical bytes로 직렬화한다.
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(QMP_HEADER_LEN + self.cells.len() * 2);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&(QMP_HEADER_LEN as u32).to_le_bytes());
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        for &cell in &self.cells {
            out.extend_from_slice(&cell.to_le_bytes());
        }
        out
    }
}

/// QMP 파싱 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QmpError {
    Truncated(usize),
    InvalidMagic,
    InvalidHeaderLength(u32),
    EmptyDimensions { width: u32, height: u32 },
    SizeOverflow,
    LengthMismatch { expected: usize, actual: usize },
}

impl fmt::Display for QmpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated(actual) => {
                write!(
                    f,
                    "QMP header 잘림: expected>={QMP_HEADER_LEN}, actual={actual}"
                )
            }
            Self::InvalidMagic => write!(f, "QMP magic 불일치"),
            Self::InvalidHeaderLength(actual) => write!(
                f,
                "QMP header 길이 불일치: expected={QMP_HEADER_LEN}, actual={actual}"
            ),
            Self::EmptyDimensions { width, height } => {
                write!(f, "QMP 빈 크기: {width}x{height}")
            }
            Self::SizeOverflow => write!(f, "QMP cell 크기 계산 overflow"),
            Self::LengthMismatch { expected, actual } => write!(
                f,
                "QMP 파일 길이 불일치: expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for QmpError {}

/// `RPG_MAP.VDD` QMP 구조 분모.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QmpInventorySummary {
    pub maps: usize,
    pub cells: usize,
    pub dimensions: usize,
    pub unique_tile_ids: usize,
}

pub const BOX_EXPECTED_QMP_INVENTORY: QmpInventorySummary = QmpInventorySummary {
    maps: 135,
    cells: 395_478,
    dimensions: 29,
    unique_tile_ids: 814,
};

/// QMP archive 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QmpInventoryError {
    WrongEntryCount {
        expected: usize,
        actual: usize,
    },
    UnsupportedEntry(String),
    Parse {
        name: String,
        source: QmpError,
    },
    RoundtripMismatch(String),
    SummaryMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for QmpInventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongEntryCount { expected, actual } => write!(
                f,
                "RPG_MAP QMP 수 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnsupportedEntry(name) => {
                write!(f, "RPG_MAP에 QMP가 아닌 엔트리: {name}")
            }
            Self::Parse { name, source } => write!(f, "{name}: {source}"),
            Self::RoundtripMismatch(name) => {
                write!(f, "{name}: QMP 직렬화가 원본과 다름")
            }
            Self::SummaryMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "RPG_MAP QMP {field} 불일치: expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for QmpInventoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Parse { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// 지원 원본 `RPG_MAP.VDD`의 모든 QMP를 파싱·왕복하고 분모를 검증한다.
pub fn verify_box_qmp_inventory(
    archive: &VddArchive,
) -> Result<QmpInventorySummary, QmpInventoryError> {
    if archive.entry_count() != BOX_EXPECTED_QMP_INVENTORY.maps {
        return Err(QmpInventoryError::WrongEntryCount {
            expected: BOX_EXPECTED_QMP_INVENTORY.maps,
            actual: archive.entry_count(),
        });
    }
    let mut cells = 0usize;
    let mut dimensions = std::collections::BTreeSet::new();
    let mut tile_ids = std::collections::BTreeSet::new();
    for entry in archive.entries() {
        if !entry.name().to_ascii_uppercase().ends_with(".QMP") {
            return Err(QmpInventoryError::UnsupportedEntry(
                entry.name().to_string(),
            ));
        }
        let raw = archive.read_file(entry);
        let map = QmpMap::parse(raw).map_err(|source| QmpInventoryError::Parse {
            name: entry.name().to_string(),
            source,
        })?;
        if map.serialize() != raw {
            return Err(QmpInventoryError::RoundtripMismatch(
                entry.name().to_string(),
            ));
        }
        cells += map.cells().len();
        dimensions.insert((map.width(), map.height()));
        tile_ids.extend(map.cells().iter().copied());
    }
    let summary = QmpInventorySummary {
        maps: archive.entry_count(),
        cells,
        dimensions: dimensions.len(),
        unique_tile_ids: tile_ids.len(),
    };
    require_field("cell 수", BOX_EXPECTED_QMP_INVENTORY.cells, summary.cells)?;
    require_field(
        "크기 종류",
        BOX_EXPECTED_QMP_INVENTORY.dimensions,
        summary.dimensions,
    )?;
    require_field(
        "고유 tile ID 수",
        BOX_EXPECTED_QMP_INVENTORY.unique_tile_ids,
        summary.unique_tile_ids,
    )?;
    Ok(summary)
}

fn require_field(
    field: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), QmpInventoryError> {
    if expected != actual {
        return Err(QmpInventoryError::SummaryMismatch {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

fn read_u32(raw: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap())
}

#[cfg(test)]
#[path = "qmp_tests.rs"]
mod qmp_tests;
