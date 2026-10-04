//! PCOMI의 u16 glyph code를 cell로 바꾸는 전용 분기.
//!
//! 공용 SEQ consumer가 control이 아닌 token을 glyph 경로로 넘긴 뒤,
//! `PCOMI.EXE`는 세 경로를 고른다. `0x1000` 미만은 직접 cell, low byte가
//! `_` 또는 `*`인 값은 96-entry suffix table, 나머지는 BIOS glyph cache다.
//! 이 모듈은 그 프로젝트 고유 의미와 재삽입 admission을 소유한다. shared
//! ISA에는 이 정책을 올리지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{linear, SubgameTextConsumerAnalysis};

pub const PCOMI_GLYPH_DIRECT_LIMIT: u16 = 0x1000;
pub const PCOMI_GLYPH_CELL_BASE: u16 = 0x0096;
pub const PCOMI_GLYPH_OBJECT_CELL_OFFSET: i16 = 0x006A;
pub const PCOMI_GLYPH_SUFFIX_HIGH_MIN: u8 = 0x20;
pub const PCOMI_GLYPH_SUFFIX_HIGH_MAX: u8 = 0x7F;
pub const PCOMI_GLYPH_SUFFIX_TABLE_ENTRIES: usize = 96;
pub const PCOMI_GLYPH_UNDERSCORE_TABLE_RUNTIME: u32 = 0x8005_0704;
pub const PCOMI_GLYPH_STAR_TABLE_RUNTIME: u32 = 0x8005_07C4;
pub const PCOMI_GLYPH_FALLBACK_RUNTIME: u32 = 0x8001_BA4C;

const PCOMI_GLYPH_UNDERSCORE_TABLE_SHA256: [u8; 32] = [
    0x3f, 0x0f, 0x2a, 0x3c, 0xfe, 0x87, 0x94, 0xce, 0xb5, 0x71, 0x15, 0xdf, 0x1f, 0x7f, 0x55, 0x40,
    0x9d, 0x10, 0xcc, 0xf2, 0x49, 0xe4, 0x36, 0xd7, 0x8f, 0x70, 0x91, 0x76, 0xbd, 0xff, 0xe7, 0xa1,
];
const PCOMI_GLYPH_STAR_TABLE_SHA256: [u8; 32] = [
    0xb0, 0xa2, 0xfa, 0xaf, 0xf8, 0x86, 0xc9, 0x70, 0x73, 0xee, 0x09, 0x22, 0xe4, 0x9f, 0x60, 0xa7,
    0x1c, 0x47, 0x4b, 0xc9, 0x6c, 0x78, 0x17, 0xc6, 0x61, 0x43, 0xcf, 0x9a, 0x43, 0x5e, 0x6d, 0x7e,
];

#[derive(Debug, Clone, Copy)]
struct TableSpec {
    suffix: PcomiGlyphSuffix,
    runtime: u32,
    sha256: [u8; 32],
    nonzero_entries: usize,
    unique_entries: usize,
    unique_nonzero_entries: usize,
    maximum_entry: u16,
}

const TABLE_SPECS: [TableSpec; 2] = [
    TableSpec {
        suffix: PcomiGlyphSuffix::Underscore,
        runtime: PCOMI_GLYPH_UNDERSCORE_TABLE_RUNTIME,
        sha256: PCOMI_GLYPH_UNDERSCORE_TABLE_SHA256,
        nonzero_entries: 56,
        unique_entries: 31,
        unique_nonzero_entries: 30,
        maximum_entry: 284,
    },
    TableSpec {
        suffix: PcomiGlyphSuffix::Star,
        runtime: PCOMI_GLYPH_STAR_TABLE_RUNTIME,
        sha256: PCOMI_GLYPH_STAR_TABLE_SHA256,
        nonzero_entries: 8,
        unique_entries: 5,
        unique_nonzero_entries: 4,
        maximum_entry: 8,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcomiGlyphSuffix {
    Underscore,
    Star,
}

impl PcomiGlyphSuffix {
    pub const fn byte(self) -> u8 {
        match self {
            Self::Underscore => b'_',
            Self::Star => b'*',
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Underscore => "underscore",
            Self::Star => "star",
        }
    }
}

/// PCOMI code 한 개가 선택하는 정적 glyph 경로.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcomiGlyphCodeRoute {
    DirectCell {
        cell: u16,
    },
    SuffixTable {
        suffix: PcomiGlyphSuffix,
        high: u8,
        cell: u16,
    },
    BiosGlyphCache,
}

/// 검산된 소비 함수 한 곳의 분기 주소.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcomiGlyphCodeConsumer {
    pub role: &'static str,
    pub code_register: Reg,
    pub threshold_runtime: u32,
    pub direct_store_runtime: u32,
    pub underscore_branch_runtime: u32,
    pub star_branch_runtime: u32,
    pub fallback_call_runtime: u32,
    pub fallback_store_runtime: u32,
    pub consumer_entry_reached: bool,
}

/// suffix table 원본의 물리·내용 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcomiGlyphSuffixTable {
    pub suffix: PcomiGlyphSuffix,
    pub runtime: u32,
    pub entries: Vec<u16>,
    pub nonzero_entries: usize,
    pub unique_entries: usize,
    pub unique_nonzero_entries: usize,
    pub maximum_entry: u16,
    pub sha256: [u8; 32],
}

/// PCOMI 두 소비 함수와 두 suffix table의 정적 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcomiGlyphCodeAnalysis {
    pub consumers: Vec<PcomiGlyphCodeConsumer>,
    pub tables: Vec<PcomiGlyphSuffixTable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcomiGlyphCodeError(String);

impl fmt::Display for PcomiGlyphCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for PcomiGlyphCodeError {}

fn error(message: impl Into<String>) -> PcomiGlyphCodeError {
    PcomiGlyphCodeError(message.into())
}

#[derive(Debug, Clone, Copy)]
struct ConsumerSpec {
    role: &'static str,
    code_register: Reg,
    threshold_runtime: u32,
    direct_store_runtime: u32,
    underscore_branch_runtime: u32,
    star_branch_runtime: u32,
    fallback_call_runtime: u32,
    fallback_store_runtime: u32,
    instructions: &'static [(u32, K)],
}

const OBJECT_CONSUMER_INSTRUCTIONS: &[(u32, K)] = &[
    (
        0x8001_BD78,
        K::Lui {
            rt: Reg::V0,
            imm: 0x8005,
        },
    ),
    (
        0x8001_BD80,
        K::Addiu {
            rt: Reg::S5,
            rs: Reg::V0,
            imm: 0x0704,
        },
    ),
    (
        0x8001_BE4C,
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::S1,
            imm: PCOMI_GLYPH_DIRECT_LIMIT as i16,
        },
    ),
    (
        0x8001_BE50,
        K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 33,
        },
    ),
    (
        0x8001_BE54,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::S1,
            imm: PCOMI_GLYPH_CELL_BASE as i16,
        },
    ),
    (
        0x8001_BE58,
        K::Andi {
            rt: Reg::V1,
            rs: Reg::S1,
            imm: 0x00FF,
        },
    ),
    (
        0x8001_BE5C,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: b'_' as i16,
        },
    ),
    (
        0x8001_BE60,
        K::Bne {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 10,
        },
    ),
    (
        0x8001_BE64,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: b'*' as i16,
        },
    ),
    (
        0x8001_BE68,
        K::Srl {
            rd: Reg::V0,
            rt: Reg::S1,
            sa: 8,
        },
    ),
    (
        0x8001_BE6C,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: -(PCOMI_GLYPH_SUFFIX_HIGH_MIN as i16),
        },
    ),
    (
        0x8001_BE70,
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 1,
        },
    ),
    (
        0x8001_BE74,
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::S5,
        },
    ),
    (
        0x8001_BE78,
        K::Lhu {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0,
        },
    ),
    (
        0x8001_BE80,
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V1,
            imm: PCOMI_GLYPH_CELL_BASE as i16,
        },
    ),
    (
        0x8001_BE88,
        K::Sh {
            rt: Reg::V1,
            base: Reg::S0,
            offset: PCOMI_GLYPH_OBJECT_CELL_OFFSET,
        },
    ),
    (
        0x8001_BE8C,
        K::Bne {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 11,
        },
    ),
    (
        0x8001_BE90,
        K::Lui {
            rt: Reg::V0,
            imm: 0x8005,
        },
    ),
    (
        0x8001_BE94,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0x07C4,
        },
    ),
    (
        0x8001_BE98,
        K::Srl {
            rd: Reg::V1,
            rt: Reg::S1,
            sa: 8,
        },
    ),
    (
        0x8001_BE9C,
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V1,
            imm: -(PCOMI_GLYPH_SUFFIX_HIGH_MIN as i16),
        },
    ),
    (
        0x8001_BEA0,
        K::Sll {
            rd: Reg::V1,
            rt: Reg::V1,
            sa: 1,
        },
    ),
    (
        0x8001_BEA4,
        K::Addu {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::V0,
        },
    ),
    (
        0x8001_BEA8,
        K::Lhu {
            rt: Reg::A0,
            base: Reg::V1,
            offset: 0,
        },
    ),
    (
        0x8001_BEB0,
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: PCOMI_GLYPH_CELL_BASE as i16,
        },
    ),
    (
        0x8001_BEB8,
        K::Sh {
            rt: Reg::A0,
            base: Reg::S0,
            offset: PCOMI_GLYPH_OBJECT_CELL_OFFSET,
        },
    ),
    (
        0x8001_BEBC,
        K::Jal {
            target: PCOMI_GLYPH_FALLBACK_RUNTIME,
        },
    ),
    (
        0x8001_BEC4,
        K::Sh {
            rt: Reg::V0,
            base: Reg::S0,
            offset: PCOMI_GLYPH_OBJECT_CELL_OFFSET,
        },
    ),
    (
        0x8001_BED8,
        K::Sh {
            rt: Reg::V0,
            base: Reg::S0,
            offset: PCOMI_GLYPH_OBJECT_CELL_OFFSET,
        },
    ),
];

const ARGUMENT_CONSUMER_INSTRUCTIONS: &[(u32, K)] = &[
    (
        0x8001_C35C,
        K::Lui {
            rt: Reg::V0,
            imm: 0x8005,
        },
    ),
    (
        0x8001_C360,
        K::Addiu {
            rt: Reg::S5,
            rs: Reg::V0,
            imm: 0x0704,
        },
    ),
    (
        0x8001_C38C,
        K::Lui {
            rt: Reg::V0,
            imm: 0x8005,
        },
    ),
    (
        0x8001_C398,
        K::Addiu {
            rt: Reg::S4,
            rs: Reg::V0,
            imm: 0x07C4,
        },
    ),
    (
        0x8001_C454,
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::A1,
            imm: PCOMI_GLYPH_DIRECT_LIMIT as i16,
        },
    ),
    (
        0x8001_C458,
        K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 27,
        },
    ),
    (
        0x8001_C45C,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::A1,
            imm: PCOMI_GLYPH_CELL_BASE as i16,
        },
    ),
    (
        0x8001_C460,
        K::Andi {
            rt: Reg::V1,
            rs: Reg::A1,
            imm: 0x00FF,
        },
    ),
    (
        0x8001_C464,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: b'_' as i16,
        },
    ),
    (
        0x8001_C468,
        K::Bne {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 5,
        },
    ),
    (
        0x8001_C46C,
        K::Srl {
            rd: Reg::V0,
            rt: Reg::A1,
            sa: 8,
        },
    ),
    (
        0x8001_C470,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: -(PCOMI_GLYPH_SUFFIX_HIGH_MIN as i16),
        },
    ),
    (
        0x8001_C474,
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 1,
        },
    ),
    (
        0x8001_C47C,
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::S5,
        },
    ),
    (
        0x8001_C480,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: b'*' as i16,
        },
    ),
    (
        0x8001_C484,
        K::Bne {
            rs: Reg::V1,
            rt: Reg::V0,
            offset: 9,
        },
    ),
    (
        0x8001_C488,
        K::Srl {
            rd: Reg::V0,
            rt: Reg::A1,
            sa: 8,
        },
    ),
    (
        0x8001_C48C,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: -(PCOMI_GLYPH_SUFFIX_HIGH_MIN as i16),
        },
    ),
    (
        0x8001_C490,
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 1,
        },
    ),
    (
        0x8001_C494,
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::S4,
        },
    ),
    (
        0x8001_C498,
        K::Lhu {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0,
        },
    ),
    (
        0x8001_C4A0,
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V1,
            imm: PCOMI_GLYPH_CELL_BASE as i16,
        },
    ),
    (
        0x8001_C4A8,
        K::Sh {
            rt: Reg::V1,
            base: Reg::S0,
            offset: PCOMI_GLYPH_OBJECT_CELL_OFFSET,
        },
    ),
    (
        0x8001_C4AC,
        K::Jal {
            target: PCOMI_GLYPH_FALLBACK_RUNTIME,
        },
    ),
    (
        0x8001_C4B4,
        K::Sh {
            rt: Reg::V0,
            base: Reg::S0,
            offset: PCOMI_GLYPH_OBJECT_CELL_OFFSET,
        },
    ),
    (
        0x8001_C4C8,
        K::Sh {
            rt: Reg::V0,
            base: Reg::S0,
            offset: PCOMI_GLYPH_OBJECT_CELL_OFFSET,
        },
    ),
];

const CONSUMER_SPECS: [ConsumerSpec; 2] = [
    ConsumerSpec {
        role: "object-loop",
        code_register: Reg::S1,
        threshold_runtime: 0x8001_BE4C,
        direct_store_runtime: 0x8001_BED8,
        underscore_branch_runtime: 0x8001_BE60,
        star_branch_runtime: 0x8001_BE8C,
        fallback_call_runtime: 0x8001_BEBC,
        fallback_store_runtime: 0x8001_BEC4,
        instructions: OBJECT_CONSUMER_INSTRUCTIONS,
    },
    ConsumerSpec {
        role: "argument-immediate",
        code_register: Reg::A1,
        threshold_runtime: 0x8001_C454,
        direct_store_runtime: 0x8001_C4C8,
        underscore_branch_runtime: 0x8001_C468,
        star_branch_runtime: 0x8001_C484,
        fallback_call_runtime: 0x8001_C4AC,
        fallback_store_runtime: 0x8001_C4B4,
        instructions: ARGUMENT_CONSUMER_INSTRUCTIONS,
    },
];

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, PcomiGlyphCodeError> {
    instructions
        .binary_search_by_key(&address, |instruction| instruction.addr)
        .ok()
        .map(|index| &instructions[index])
        .ok_or_else(|| error(format!("PCOMI glyph code 명령 주소 없음: 0x{address:08X}")))
}

fn verify_instructions(
    instructions: &[Instruction],
    spec: ConsumerSpec,
) -> Result<(), PcomiGlyphCodeError> {
    for &(address, expected) in spec.instructions {
        let actual = instruction_at(instructions, address)?;
        if actual.kind != expected {
            return Err(error(format!(
                "PCOMI {} glyph code 명령 불일치: address=0x{address:08X}, \
                 expected={expected:?}, actual={:?}",
                spec.role, actual.kind
            )));
        }
    }
    Ok(())
}

fn read_table(
    map: &ExeMap,
    text: &[u8],
    spec: TableSpec,
) -> Result<PcomiGlyphSuffixTable, PcomiGlyphCodeError> {
    let offset = map.text_offset(spec.runtime).ok_or_else(|| {
        error(format!(
            "PCOMI {} suffix table이 EXE text 밖임: 0x{:08X}",
            spec.suffix.label(),
            spec.runtime,
        ))
    })?;
    let byte_len = PCOMI_GLYPH_SUFFIX_TABLE_ENTRIES * 2;
    let bytes = text.get(offset..offset + byte_len).ok_or_else(|| {
        error(format!(
            "PCOMI {} suffix table 범위 이탈: 0x{:08X}",
            spec.suffix.label(),
            spec.runtime,
        ))
    })?;
    let entries = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let nonzero_entries = entries.iter().filter(|&&entry| entry != 0).count();
    let unique = entries.iter().copied().collect::<BTreeSet<_>>();
    let unique_nonzero_entries = unique.iter().filter(|&&entry| entry != 0).count();
    let maximum_entry = entries.iter().copied().max().unwrap_or(0);
    let sha256: [u8; 32] = Sha256::digest(bytes).into();
    if sha256 != spec.sha256
        || nonzero_entries != spec.nonzero_entries
        || unique.len() != spec.unique_entries
        || unique_nonzero_entries != spec.unique_nonzero_entries
        || maximum_entry != spec.maximum_entry
    {
        return Err(error(format!(
            "PCOMI {} suffix table 기준선 불일치: nonzero expected={}, actual={}; \
             unique expected={}, actual={}; unique-nonzero expected={}, actual={}; \
             max expected={}, actual={}; SHA-256 expected={}, actual={}",
            spec.suffix.label(),
            spec.nonzero_entries,
            nonzero_entries,
            spec.unique_entries,
            unique.len(),
            spec.unique_nonzero_entries,
            unique_nonzero_entries,
            spec.maximum_entry,
            maximum_entry,
            hex_digest(&spec.sha256),
            hex_digest(&sha256),
        )));
    }
    Ok(PcomiGlyphSuffixTable {
        suffix: spec.suffix,
        runtime: spec.runtime,
        entries,
        nonzero_entries,
        unique_entries: unique.len(),
        unique_nonzero_entries,
        maximum_entry,
        sha256,
    })
}

/// 지원 원본 `PCOMI.EXE`의 direct/suffix/BIOS glyph code 분기를 검산한다.
pub fn verify_box_pcomi_glyph_codes(
    map: &'static ExeMap,
    text: &[u8],
    text_consumers: &SubgameTextConsumerAnalysis,
) -> Result<PcomiGlyphCodeAnalysis, PcomiGlyphCodeError> {
    if map.name != "PCOMI.EXE" {
        return Err(error(format!(
            "PCOMI glyph code EXE 사상 불일치: actual={}",
            map.name
        )));
    }
    if text.len() != map.text_len as usize {
        return Err(error(format!(
            "PCOMI glyph code text 길이 불일치: expected={}, actual={}",
            map.text_len,
            text.len()
        )));
    }
    let instructions =
        linear::sweep(map, text).ok_or_else(|| error("PCOMI.EXE text가 4바이트 정렬이 아님"))?;
    let mut consumers = Vec::with_capacity(CONSUMER_SPECS.len());
    for spec in CONSUMER_SPECS {
        verify_instructions(&instructions, spec)?;
        let shared = text_consumers
            .consumers
            .iter()
            .find(|consumer| consumer.exe == map.name && consumer.role == spec.role)
            .ok_or_else(|| error(format!("PCOMI {} 공용 text consumer 결과 누락", spec.role)))?;
        if shared.glyph_cache_call_runtime != spec.fallback_call_runtime
            || shared.control_table_runtime != 0x8005_08E4
        {
            return Err(error(format!(
                "PCOMI {} 공용 text consumer와 전용 glyph 분기 불일치",
                spec.role
            )));
        }
        consumers.push(PcomiGlyphCodeConsumer {
            role: spec.role,
            code_register: spec.code_register,
            threshold_runtime: spec.threshold_runtime,
            direct_store_runtime: spec.direct_store_runtime,
            underscore_branch_runtime: spec.underscore_branch_runtime,
            star_branch_runtime: spec.star_branch_runtime,
            fallback_call_runtime: spec.fallback_call_runtime,
            fallback_store_runtime: spec.fallback_store_runtime,
            consumer_entry_reached: shared.entry_reached,
        });
    }
    let tables = TABLE_SPECS
        .into_iter()
        .map(|spec| read_table(map, text, spec))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PcomiGlyphCodeAnalysis { consumers, tables })
}

/// 한 code를 실제 분기 순서대로 분류한다.
///
/// suffix form은 EXE에 상한 guard가 없으므로 table 안인 high byte만 받는다.
/// 범위 밖 suffix를 BIOS fallback으로 바꾸지 않고 재삽입 오류로 돌려준다.
pub fn classify_pcomi_glyph_code(
    analysis: &PcomiGlyphCodeAnalysis,
    code: u16,
) -> Result<PcomiGlyphCodeRoute, PcomiGlyphCodeError> {
    if code < PCOMI_GLYPH_DIRECT_LIMIT {
        return Ok(PcomiGlyphCodeRoute::DirectCell {
            cell: code + PCOMI_GLYPH_CELL_BASE,
        });
    }
    let low = code as u8;
    let suffix = match low {
        b'_' => PcomiGlyphSuffix::Underscore,
        b'*' => PcomiGlyphSuffix::Star,
        _ => return Ok(PcomiGlyphCodeRoute::BiosGlyphCache),
    };
    let high = (code >> 8) as u8;
    if !(PCOMI_GLYPH_SUFFIX_HIGH_MIN..=PCOMI_GLYPH_SUFFIX_HIGH_MAX).contains(&high) {
        return Err(error(format!(
            "PCOMI {} suffix high byte 범위 이탈: code=0x{code:04X}, allowed=0x20..0x7F",
            suffix.label()
        )));
    }
    let table = analysis
        .tables
        .iter()
        .find(|table| table.suffix == suffix)
        .ok_or_else(|| error(format!("PCOMI {} suffix table 결과 누락", suffix.label())))?;
    let index = usize::from(high - PCOMI_GLYPH_SUFFIX_HIGH_MIN);
    let mapped = *table
        .entries
        .get(index)
        .ok_or_else(|| error("PCOMI suffix table index 범위 이탈"))?;
    Ok(PcomiGlyphCodeRoute::SuffixTable {
        suffix,
        high,
        cell: mapped + PCOMI_GLYPH_CELL_BASE,
    })
}

/// 전용 분기와 table을 결정적 TSV로 만든다.
pub fn render_pcomi_glyph_codes_tsv(analysis: &PcomiGlyphCodeAnalysis) -> String {
    let mut out = String::from(
        "kind\trole\tcode_register\tthreshold_runtime\tdirect_store_runtime\t\
         underscore_branch_runtime\tstar_branch_runtime\tfallback_call_runtime\t\
         fallback_store_runtime\tconsumer_entry_reached\tsuffix\ttable_runtime\tentries\t\
         nonzero_entries\tunique_entries\tunique_nonzero_entries\tmaximum_entry\tsha256\n",
    );
    for consumer in &analysis.consumers {
        writeln!(
            out,
            "consumer\t{}\t{}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t\
             0x{:08X}\t0x{:08X}\t{}\t\t\t\t\t\t\t\t",
            consumer.role,
            consumer.code_register.name(),
            consumer.threshold_runtime,
            consumer.direct_store_runtime,
            consumer.underscore_branch_runtime,
            consumer.star_branch_runtime,
            consumer.fallback_call_runtime,
            consumer.fallback_store_runtime,
            consumer.consumer_entry_reached,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    for table in &analysis.tables {
        writeln!(
            out,
            "table\t\t\t\t\t\t\t\t\t\t{}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}",
            table.suffix.label(),
            table.runtime,
            table.entries.len(),
            table.nonzero_entries,
            table.unique_entries,
            table.unique_nonzero_entries,
            table.maximum_entry,
            hex_digest(&table.sha256),
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    out
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_pcomi_glyph_code_tests.rs"]
mod subgame_seq_pcomi_glyph_code_tests;
