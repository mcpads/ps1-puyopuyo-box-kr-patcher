//! `PUYO2.EXE`의 `DEMOFONT.BTX` 문자 code → cell 소비 경계.
//!
//! 원본의 Shift-JIS 코드표, resource descriptor, parser와 glyph object
//! 기록 명령을 함께 검증한다. 이 모듈이 고정하는 범위는 code `0..=255`가
//! `DEMOFONT.BTX`의 같은 번호 cell로 이어진다는 사실까지다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::font::DemoFontSummary;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::text::decode_sjis;

use super::linear;

pub const DEMO_FONT_CODE_TABLE_RUNTIME: u32 = 0x8001_00FC;
pub const DEMO_FONT_CODE_TABLE_ENTRIES: usize = 256;
pub const DEMO_FONT_CODE_TABLE_SHA256: &str =
    "4198ab2fb96afeecade0b179232e39a4c999e215db1d81d99e860033befd512c";
pub const DEMO_FONT_RESOURCE_TABLE_RUNTIME: u32 = 0x8006_3E9C;
pub const DEMO_FONT_RESOURCE_INDEX: u16 = 0x0088;
pub const DEMO_FONT_DESCRIPTOR_RUNTIME: u32 = 0x8006_51BC;

const RESOURCE_RECORD_BYTES: u32 = 36;
const DESCRIPTOR_KIND: u32 = 0x0001_0000;
const DESCRIPTOR_NAME_RUNTIME: u32 = 0x8006_64CD;
const DESCRIPTOR_RAW_BYTES: u32 = 0x0001_0000;
const CODE_TABLE_BYTES: usize = DEMO_FONT_CODE_TABLE_ENTRIES * 2;

/// `DEMOFONT.BTX` 소비 경계 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoFontConsumerSummary {
    pub code_table_runtime: u32,
    pub code_table_entries: usize,
    pub resource_table_runtime: u32,
    pub resource_index: u16,
    pub descriptor_runtime: u32,
    pub mapped_cells: usize,
    pub physical_empty_cells: usize,
    pub mapped_empty_cells: usize,
    pub unregistered_empty_cells: usize,
}

/// `DEMOFONT.BTX` 소비 경계 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DemoFontConsumerError {
    WrongExecutable(&'static str),
    TextLengthMismatch {
        expected: usize,
        actual: usize,
    },
    UnalignedText,
    RangeOutsideText {
        address: u32,
        len: usize,
    },
    WordMismatch {
        address: u32,
        field: &'static str,
        expected: u32,
        actual: u32,
    },
    NameMismatch(Vec<u8>),
    CodeTableDigestMismatch {
        expected: String,
        actual: String,
    },
    InvalidCodeTableEntry(usize),
    MissingInstruction(u32),
    InstructionMismatch {
        address: u32,
        expected: &'static str,
        actual: K,
    },
    PhysicalSlotCount {
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for DemoFontConsumerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(f, "DEMOFONT 소비 검증 대상이 PUYO2.EXE가 아님: {exe}")
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                f,
                "PUYO2.EXE text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText => write!(f, "PUYO2.EXE text 길이가 4바이트 정렬이 아님"),
            Self::RangeOutsideText { address, len } => write!(
                f,
                "PUYO2.EXE runtime 범위가 text 밖임: address=0x{address:08X}, len={len}"
            ),
            Self::WordMismatch {
                address,
                field,
                expected,
                actual,
            } => write!(
                f,
                "DEMOFONT descriptor {field} 불일치: address=0x{address:08X}, \
                 expected=0x{expected:08X}, actual=0x{actual:08X}"
            ),
            Self::NameMismatch(actual) => {
                write!(f, "DEMOFONT descriptor 이름 불일치: actual={actual:02X?}")
            }
            Self::CodeTableDigestMismatch { expected, actual } => write!(
                f,
                "DEMOFONT code table SHA-256 불일치: expected={expected}, actual={actual}"
            ),
            Self::InvalidCodeTableEntry(index) => {
                write!(
                    f,
                    "DEMOFONT code table의 {index}번 pair가 단일 Shift-JIS 문자가 아님"
                )
            }
            Self::MissingInstruction(address) => {
                write!(f, "DEMOFONT 소비 명령이 text 밖임: 0x{address:08X}")
            }
            Self::InstructionMismatch {
                address,
                expected,
                actual,
            } => write!(
                f,
                "DEMOFONT 소비 명령 불일치: address=0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
            Self::PhysicalSlotCount { expected, actual } => write!(
                f,
                "DEMOFONT 물리 slot 수 불일치: expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for DemoFontConsumerError {}

/// 지원 원본 `PUYO2.EXE`와 물리 atlas의 code → cell 경계를 검증한다.
pub fn verify_box_demo_font_consumer(
    map: &ExeMap,
    text: &[u8],
    font: &DemoFontSummary,
) -> Result<DemoFontConsumerSummary, DemoFontConsumerError> {
    if map.name != "PUYO2.EXE" {
        return Err(DemoFontConsumerError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(DemoFontConsumerError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }

    let descriptor_runtime = DEMO_FONT_RESOURCE_TABLE_RUNTIME
        + u32::from(DEMO_FONT_RESOURCE_INDEX) * RESOURCE_RECORD_BYTES;
    require_word(map, text, descriptor_runtime, "kind", DESCRIPTOR_KIND)?;
    require_word(
        map,
        text,
        descriptor_runtime + 4,
        "name pointer",
        DESCRIPTOR_NAME_RUNTIME,
    )?;
    require_word(
        map,
        text,
        descriptor_runtime + 12,
        "raw bytes",
        DESCRIPTOR_RAW_BYTES,
    )?;
    if descriptor_runtime != DEMO_FONT_DESCRIPTOR_RUNTIME {
        return Err(DemoFontConsumerError::WordMismatch {
            address: descriptor_runtime,
            field: "resource index 환산",
            expected: DEMO_FONT_DESCRIPTOR_RUNTIME,
            actual: descriptor_runtime,
        });
    }
    let name = runtime_slice(map, text, DESCRIPTOR_NAME_RUNTIME, b"DEMOFONT.BTX\0".len())?;
    if name != b"DEMOFONT.BTX\0" {
        return Err(DemoFontConsumerError::NameMismatch(name.to_vec()));
    }

    let code_table = runtime_slice(map, text, DEMO_FONT_CODE_TABLE_RUNTIME, CODE_TABLE_BYTES)?;
    let actual_digest = format!("{:x}", Sha256::digest(code_table));
    if actual_digest != DEMO_FONT_CODE_TABLE_SHA256 {
        return Err(DemoFontConsumerError::CodeTableDigestMismatch {
            expected: DEMO_FONT_CODE_TABLE_SHA256.to_string(),
            actual: actual_digest,
        });
    }
    verify_code_table_shape(code_table)?;

    let instructions = linear::sweep(map, text).ok_or(DemoFontConsumerError::UnalignedText)?;
    verify_resource_table_initialization(&instructions, map)?;
    verify_code_parser(&instructions, map)?;
    verify_cell_consumer(&instructions, map)?;
    let mapped_empty_cells = verify_physical_cells(font)?;

    Ok(DemoFontConsumerSummary {
        code_table_runtime: DEMO_FONT_CODE_TABLE_RUNTIME,
        code_table_entries: DEMO_FONT_CODE_TABLE_ENTRIES,
        resource_table_runtime: DEMO_FONT_RESOURCE_TABLE_RUNTIME,
        resource_index: DEMO_FONT_RESOURCE_INDEX,
        descriptor_runtime,
        mapped_cells: DEMO_FONT_CODE_TABLE_ENTRIES,
        physical_empty_cells: font.empty_slots.len(),
        mapped_empty_cells,
        unregistered_empty_cells: font.empty_slots.len() - mapped_empty_cells,
    })
}

fn verify_resource_table_initialization(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<(), DemoFontConsumerError> {
    require(
        instructions,
        map,
        0x8002_27D8,
        "lui v1, 0x800B",
        K::Lui {
            rt: Reg::V1,
            imm: 0x800B,
        },
    )?;
    require(
        instructions,
        map,
        0x8002_27E0,
        "lui v0, 0x8006",
        K::Lui {
            rt: Reg::V0,
            imm: 0x8006,
        },
    )?;
    require(
        instructions,
        map,
        0x8002_27E4,
        "addiu v0, v0, 0x3E9C",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0x3E9C,
        },
    )?;
    require(
        instructions,
        map,
        0x8002_27EC,
        "jal 0x80028780",
        K::Jal {
            target: 0x8002_8780,
        },
    )?;
    require(
        instructions,
        map,
        0x8002_27F0,
        "sw v0, 0x6710(v1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x6710,
        },
    )
}

fn verify_code_parser(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<(), DemoFontConsumerError> {
    require(
        instructions,
        map,
        0x8001_6F3C,
        "lui v0, 0x8001",
        K::Lui {
            rt: Reg::V0,
            imm: 0x8001,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_6F40,
        "addiu v1, v0, 0x00FC",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V0,
            imm: 0x00FC,
        },
    )?;
    for address in [0x8001_6F50, 0x8001_6FA8] {
        require(
            instructions,
            map,
            address,
            "addiu v0, v1, 0x0200",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 0x0200,
            },
        )?;
    }
    require(
        instructions,
        map,
        0x8001_719C,
        "addiu a1, a1, 2",
        K::Addiu {
            rt: Reg::A1,
            rs: Reg::A1,
            imm: 2,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_71AC,
        "srl v0, a1, 31",
        K::Srl {
            rd: Reg::V0,
            rt: Reg::A1,
            sa: 31,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_71B0,
        "addu v0, a1, v0",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::A1,
            rt: Reg::V0,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_71B4,
        "sra v0, v0, 1",
        K::Sra {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 1,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_71B8,
        "sw v0, 0(v1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0,
        },
    )
}

fn verify_cell_consumer(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<(), DemoFontConsumerError> {
    let s1 = Reg(17);
    require(
        instructions,
        map,
        0x8001_6E24,
        "lw s1, 0(v0)",
        K::Lw {
            rt: s1,
            base: Reg::V0,
            offset: 0,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_6E2C,
        "sltiu v0, s1, 0x100",
        K::Sltiu {
            rt: Reg::V0,
            rs: s1,
            imm: 0x0100,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_6E30,
        "beq v0, zero, 0x80016EB0",
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 31,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_6E7C,
        "addiu v1, zero, 0x88",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: DEMO_FONT_RESOURCE_INDEX as i16,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_6E84,
        "sh v1, 0x6C(v0)",
        K::Sh {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x006C,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_6E88,
        "sh s1, 0x6A(v0)",
        K::Sh {
            rt: s1,
            base: Reg::V0,
            offset: 0x006A,
        },
    )
}

fn verify_code_table_shape(code_table: &[u8]) -> Result<(), DemoFontConsumerError> {
    if code_table.len() != CODE_TABLE_BYTES {
        return Err(DemoFontConsumerError::RangeOutsideText {
            address: DEMO_FONT_CODE_TABLE_RUNTIME,
            len: CODE_TABLE_BYTES,
        });
    }
    for (index, pair) in code_table.chunks_exact(2).enumerate() {
        let decoded =
            decode_sjis(pair).map_err(|_| DemoFontConsumerError::InvalidCodeTableEntry(index))?;
        if decoded.chars().count() != 1 {
            return Err(DemoFontConsumerError::InvalidCodeTableEntry(index));
        }
    }
    Ok(())
}

fn verify_physical_cells(font: &DemoFontSummary) -> Result<usize, DemoFontConsumerError> {
    if font.slots != 512 {
        return Err(DemoFontConsumerError::PhysicalSlotCount {
            expected: 512,
            actual: font.slots,
        });
    }
    Ok(font
        .empty_slots
        .iter()
        .filter(|index| **index < DEMO_FONT_CODE_TABLE_ENTRIES)
        .count())
}

fn runtime_slice<'a>(
    map: &ExeMap,
    text: &'a [u8],
    address: u32,
    len: usize,
) -> Result<&'a [u8], DemoFontConsumerError> {
    let offset = map
        .text_offset(address)
        .ok_or(DemoFontConsumerError::RangeOutsideText { address, len })?;
    let end = offset
        .checked_add(len)
        .ok_or(DemoFontConsumerError::RangeOutsideText { address, len })?;
    text.get(offset..end)
        .ok_or(DemoFontConsumerError::RangeOutsideText { address, len })
}

fn read_word(map: &ExeMap, text: &[u8], address: u32) -> Result<u32, DemoFontConsumerError> {
    let raw = runtime_slice(map, text, address, 4)?;
    Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn require_word(
    map: &ExeMap,
    text: &[u8],
    address: u32,
    field: &'static str,
    expected: u32,
) -> Result<(), DemoFontConsumerError> {
    let actual = read_word(map, text, address)?;
    if actual != expected {
        return Err(DemoFontConsumerError::WordMismatch {
            address,
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

fn require(
    instructions: &[Instruction],
    map: &ExeMap,
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), DemoFontConsumerError> {
    let offset = map
        .text_offset(address)
        .ok_or(DemoFontConsumerError::MissingInstruction(address))?;
    let actual = instructions
        .get(offset / 4)
        .filter(|instruction| instruction.addr == address)
        .ok_or(DemoFontConsumerError::MissingInstruction(address))?;
    if actual.kind != kind {
        return Err(DemoFontConsumerError::InstructionMismatch {
            address,
            expected,
            actual: actual.kind,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "demo_font_tests.rs"]
mod demo_font_tests;
