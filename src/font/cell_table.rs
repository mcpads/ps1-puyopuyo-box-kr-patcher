//! runtime glyph/sprite cell metadata 표.
//!
//! `COM_FONT.BTX`처럼 개별 glyph와 큰 조립 단위가 섞인 자산은 픽셀
//! connected component만으로 소비 사각형을 확정할 수 없다. 이 모듈은
//! runtime descriptor가 가리키는 12바이트 cell record를 명시한 분모만큼
//! 읽고, atlas 경계를 벗어나는 record를 거부한다. 통합 sprite 표처럼
//! 빈 slot과 다른 page record가 섞인 경우는 명시한 index만 검사한다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use crate::coords::normalize_kseg;

const DESCRIPTOR_BYTES: usize = 36;
const CELL_RECORD_BYTES: usize = 12;
const TABLE_HEADER_BYTES: usize = 12;

/// runtime cell 사각형 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCell {
    pub index: usize,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub anchor_x: i16,
    pub anchor_y: i16,
}

/// descriptor와 cell metadata 표의 검증 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCellTable {
    pub descriptor_runtime: u32,
    pub table_runtime: u32,
    pub atlas_width: u16,
    pub atlas_height: u16,
    pub cells: Vec<RuntimeCell>,
}

impl RuntimeCellTable {
    pub fn large_cells(&self) -> usize {
        self.cells
            .iter()
            .filter(|cell| cell.width > 8 || cell.height > 16)
            .count()
    }

    pub fn distinct_sizes(&self) -> usize {
        self.cells
            .iter()
            .map(|cell| (cell.width, cell.height))
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    }
}

/// runtime cell 표 파싱 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCellTableError {
    EmptyTable,
    EmptySelection,
    DuplicateCellIndex(usize),
    CellIndexOutsideTable {
        index: usize,
        record_count: usize,
    },
    AddressOutsideRam {
        address: u32,
        len: usize,
    },
    InvalidDescriptorKind(u32),
    NullTablePointer,
    InvalidHeader {
        origin_x: u16,
        origin_y: u16,
        width: u16,
        height: u16,
        anchor_x: i16,
        anchor_y: i16,
    },
    EmptyCell(usize),
    CellOutsideAtlas {
        index: usize,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        atlas_width: u16,
        atlas_height: u16,
    },
}

impl fmt::Display for RuntimeCellTableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTable => write!(f, "runtime cell table 분모가 0임"),
            Self::EmptySelection => write!(f, "runtime cell selection이 비어 있음"),
            Self::DuplicateCellIndex(index) => {
                write!(f, "runtime cell selection index 중복: {index}")
            }
            Self::CellIndexOutsideTable {
                index,
                record_count,
            } => write!(
                f,
                "runtime cell selection index가 분모 밖임: index={index}, record_count={record_count}"
            ),
            Self::AddressOutsideRam { address, len } => write!(
                f,
                "runtime cell table RAM 범위 이탈: address=0x{address:08X}, len={len}"
            ),
            Self::InvalidDescriptorKind(actual) => write!(
                f,
                "runtime cell descriptor kind 불일치: expected low16=0x0001, \
                 actual=0x{actual:08X}"
            ),
            Self::NullTablePointer => write!(f, "runtime cell descriptor의 table pointer가 0임"),
            Self::InvalidHeader {
                origin_x,
                origin_y,
                width,
                height,
                anchor_x,
                anchor_y,
            } => write!(
                f,
                "runtime cell table header 불일치: origin=({origin_x},{origin_y}), \
                 size={width}x{height}, anchor=({anchor_x},{anchor_y})"
            ),
            Self::EmptyCell(index) => write!(f, "runtime cell {index}의 크기가 0임"),
            Self::CellOutsideAtlas {
                index,
                x,
                y,
                width,
                height,
                atlas_width,
                atlas_height,
            } => write!(
                f,
                "runtime cell {index}가 atlas 밖임: ({x},{y}) {width}x{height}, \
                 atlas={atlas_width}x{atlas_height}"
            ),
        }
    }
}

impl Error for RuntimeCellTableError {}

/// RAM dump에서 descriptor와 명시한 수의 cell record를 읽는다.
pub fn parse_runtime_cell_table(
    ram: &[u8],
    descriptor_runtime: u32,
    record_count: usize,
) -> Result<RuntimeCellTable, RuntimeCellTableError> {
    if record_count == 0 {
        return Err(RuntimeCellTableError::EmptyTable);
    }
    let indices = (0..record_count).collect::<Vec<_>>();
    parse_runtime_cell_indices(ram, descriptor_runtime, record_count, &indices)
}

/// 통합 sprite 표에서 명시한 cell index만 atlas 경계와 대조한다.
///
/// `record_count`는 전체 표의 분모를 검증하기 위한 외부 근거이며, 선택하지
/// 않은 빈 slot이나 다른 texture page record를 승인하지 않는다.
pub fn parse_runtime_cell_selection(
    ram: &[u8],
    descriptor_runtime: u32,
    record_count: usize,
    selected_indices: &[usize],
) -> Result<RuntimeCellTable, RuntimeCellTableError> {
    if record_count == 0 {
        return Err(RuntimeCellTableError::EmptyTable);
    }
    if selected_indices.is_empty() {
        return Err(RuntimeCellTableError::EmptySelection);
    }
    let mut unique = std::collections::BTreeSet::new();
    for &index in selected_indices {
        if index >= record_count {
            return Err(RuntimeCellTableError::CellIndexOutsideTable {
                index,
                record_count,
            });
        }
        if !unique.insert(index) {
            return Err(RuntimeCellTableError::DuplicateCellIndex(index));
        }
    }
    parse_runtime_cell_indices(ram, descriptor_runtime, record_count, selected_indices)
}

fn parse_runtime_cell_indices(
    ram: &[u8],
    descriptor_runtime: u32,
    record_count: usize,
    selected_indices: &[usize],
) -> Result<RuntimeCellTable, RuntimeCellTableError> {
    let descriptor = ram_slice(ram, descriptor_runtime, DESCRIPTOR_BYTES)?;
    let kind = read_u32(descriptor, 0);
    if kind & 0xFFFF != 1 {
        return Err(RuntimeCellTableError::InvalidDescriptorKind(kind));
    }
    let table_runtime = read_u32(descriptor, 28);
    if table_runtime == 0 {
        return Err(RuntimeCellTableError::NullTablePointer);
    }
    let table_bytes = TABLE_HEADER_BYTES
        .checked_add(record_count.checked_mul(CELL_RECORD_BYTES).ok_or(
            RuntimeCellTableError::AddressOutsideRam {
                address: table_runtime,
                len: usize::MAX,
            },
        )?)
        .ok_or(RuntimeCellTableError::AddressOutsideRam {
            address: table_runtime,
            len: usize::MAX,
        })?;
    let table = ram_slice(ram, table_runtime, table_bytes)?;
    let origin_x = read_u16(table, 0);
    let origin_y = read_u16(table, 2);
    let atlas_width = read_u16(table, 4);
    let atlas_height = read_u16(table, 6);
    let anchor_x = read_i16(table, 8);
    let anchor_y = read_i16(table, 10);
    if origin_x != 0
        || origin_y != 0
        || atlas_width == 0
        || atlas_height == 0
        || anchor_x != 0
        || anchor_y != 0
    {
        return Err(RuntimeCellTableError::InvalidHeader {
            origin_x,
            origin_y,
            width: atlas_width,
            height: atlas_height,
            anchor_x,
            anchor_y,
        });
    }

    let mut cells = Vec::with_capacity(selected_indices.len());
    for &index in selected_indices {
        let offset = TABLE_HEADER_BYTES + index * CELL_RECORD_BYTES;
        let cell = RuntimeCell {
            index,
            x: read_u16(table, offset),
            y: read_u16(table, offset + 2),
            width: read_u16(table, offset + 4),
            height: read_u16(table, offset + 6),
            anchor_x: read_i16(table, offset + 8),
            anchor_y: read_i16(table, offset + 10),
        };
        if cell.width == 0 || cell.height == 0 {
            return Err(RuntimeCellTableError::EmptyCell(index));
        }
        if u32::from(cell.x) + u32::from(cell.width) > u32::from(atlas_width)
            || u32::from(cell.y) + u32::from(cell.height) > u32::from(atlas_height)
        {
            return Err(RuntimeCellTableError::CellOutsideAtlas {
                index,
                x: cell.x,
                y: cell.y,
                width: cell.width,
                height: cell.height,
                atlas_width,
                atlas_height,
            });
        }
        cells.push(cell);
    }

    Ok(RuntimeCellTable {
        descriptor_runtime,
        table_runtime,
        atlas_width,
        atlas_height,
        cells,
    })
}

/// 검증된 cell record를 결정적 TSV로 직렬화한다.
pub fn render_runtime_cell_table_tsv(table: &RuntimeCellTable) -> String {
    let mut output = String::from(
        "descriptor_runtime\ttable_runtime\tatlas_width\tatlas_height\tindex\tx\ty\twidth\theight\tanchor_x\tanchor_y\n",
    );
    for cell in &table.cells {
        writeln!(
            output,
            "0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            table.descriptor_runtime,
            table.table_runtime,
            table.atlas_width,
            table.atlas_height,
            cell.index,
            cell.x,
            cell.y,
            cell.width,
            cell.height,
            cell.anchor_x,
            cell.anchor_y,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

fn ram_slice(ram: &[u8], runtime_address: u32, len: usize) -> Result<&[u8], RuntimeCellTableError> {
    let normalized = normalize_kseg(runtime_address);
    let ram_address = if (0x8000_0000..0xA000_0000).contains(&normalized) {
        normalized & 0x1FFF_FFFF
    } else {
        normalized
    };
    let offset =
        usize::try_from(ram_address).map_err(|_| RuntimeCellTableError::AddressOutsideRam {
            address: runtime_address,
            len,
        })?;
    let end = offset
        .checked_add(len)
        .ok_or(RuntimeCellTableError::AddressOutsideRam {
            address: runtime_address,
            len,
        })?;
    ram.get(offset..end)
        .ok_or(RuntimeCellTableError::AddressOutsideRam {
            address: runtime_address,
            len,
        })
}

fn read_u16(raw: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([raw[offset], raw[offset + 1]])
}

fn read_i16(raw: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([raw[offset], raw[offset + 1]])
}

fn read_u32(raw: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        raw[offset],
        raw[offset + 1],
        raw[offset + 2],
        raw[offset + 3],
    ])
}

#[cfg(test)]
#[path = "cell_table_tests.rs"]
mod cell_table_tests;
