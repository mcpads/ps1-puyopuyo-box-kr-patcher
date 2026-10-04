//! `E6` direct text command와 전역 code table 소비 경계.
//!
//! 가나 run의 exact pointer를 출발점으로 삼되, 최종 분모는 PUYO1/PUYO2의
//! 모든 overlay를 4바이트 정렬로 훑어 찾은 canonical `E6 00 00 00 + u32`
//! record다. `E6` handler는 `record+4`의 pointer를 2바이트 pair parser에
//! 넘기고 record를 8바이트 전진한다. parser는 `65 65`에서 끝나며 관측
//! control `70 70`, `72 72`, `77 24`를 glyph와 분리한다. 두 EXE는 같은
//! 256-entry Shift-JIS code table을 쓰지만 glyph resource는 PUYO1 `CHR.BTX`,
//! PUYO2 `DEMOFONT.BTX`로 다르다. 정상 실행과 PUYO1의 cell table·화면
//! 표시는 별도 runtime gate다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::{SubgameSeqLayoutAnalysis, SubgameSeqLayoutKind, SubgameSeqTextRootAnalysis};
use crate::text::decode_sjis;

use super::{linear, BtxNameAnalysis};

pub const SUBGAME_SEQ_E6_TEXT_SCHEMA_VERSION: &str = "subgame-seq-e6-text-v1";
pub const BOX_SUBGAME_SEQ_E6_TEXT_ROWS: usize = 118;
pub const BOX_SUBGAME_SEQ_E6_TEXT_PHYSICAL_SEQUENCES: usize = 66;
pub const BOX_SUBGAME_SEQ_E6_TEXT_LOGICAL_SEQUENCES: usize = 33;
pub const BOX_SUBGAME_SEQ_E6_TEXT_ROWS_PER_EXE: usize = 59;
pub const BOX_SUBGAME_SEQ_E6_TEXT_SPANS: usize = 266;
pub const BOX_SUBGAME_SEQ_E6_TEXT_CODE_UNITS: usize = 2_567;
pub const BOX_SUBGAME_SEQ_E6_TEXT_UNIQUE_PAIRS: usize = 210;
pub const BOX_SUBGAME_SEQ_E6_TEXT_CODE_TABLE_ENTRIES: usize = 256;
pub const BOX_SUBGAME_SEQ_E6_TEXT_CODE_TABLE_UNIQUE_PAIRS: usize = 252;
pub const BOX_SUBGAME_SEQ_E6_TEXT_EXACT_ROOTS: usize = 118;
pub const BOX_SUBGAME_SEQ_E1_REPERTOIRE_EXACT_ROOTS: usize = 34;
pub const BOX_SUBGAME_SEQ_E6_TEXT_CONTROL_70: usize = 66;
pub const BOX_SUBGAME_SEQ_E6_TEXT_CONTROL_72: usize = 148;
pub const BOX_SUBGAME_SEQ_E6_TEXT_CONTROL_77: usize = 59;
pub const BOX_SUBGAME_SEQ_E6_TEXT_TERMINATORS: usize = 118;
pub const BOX_SUBGAME_SEQ_E6_TEXT_MIN_PAIRS: usize = 10;
pub const BOX_SUBGAME_SEQ_E6_TEXT_MAX_PAIRS: usize = 41;
pub const BOX_SUBGAME_SEQ_E6_TEXT_MAX_EMITTED_TOKENS: usize = 44;
pub const BOX_SUBGAME_SEQ_E6_TEXT_COUNTERPART_EQUAL: usize = 51;
pub const BOX_SUBGAME_SEQ_E6_TEXT_COUNTERPART_DIFFERENT: usize = 8;
pub const BOX_SUBGAME_SEQ_E6_TEXT_REPORT_SHA256: [u8; 32] = [
    0xA5, 0xCA, 0x0F, 0xA5, 0xD1, 0xFB, 0xBA, 0x43, 0xDB, 0x4A, 0xD7, 0x97, 0x82, 0x56, 0x90, 0x31,
    0x7F, 0x00, 0x55, 0x95, 0xD4, 0x0B, 0x11, 0xE7, 0x24, 0x17, 0x85, 0x3E, 0xA2, 0x30, 0x5C, 0x5D,
];

const E6_COMMAND: [u8; 4] = [0xE6, 0, 0, 0];
const E1_COMMAND: [u8; 4] = [0xE1, 0, 0, 0];
const TERMINATOR: [u8; 2] = [0x65, 0x65];
const CONTROL_70: [u8; 2] = [0x70, 0x70];
const CONTROL_72: [u8; 2] = [0x72, 0x72];
const CONTROL_77: [u8; 2] = [0x77, 0x24];
const CODE_TABLE_RUNTIME: u32 = 0x8001_00FC;
const CODE_TABLE_BYTES: usize = BOX_SUBGAME_SEQ_E6_TEXT_CODE_TABLE_ENTRIES * 2;
const CODE_TABLE_SHA256: [u8; 32] = [
    0x41, 0x98, 0xAB, 0x2F, 0xB9, 0x6A, 0xFE, 0xEC, 0xAD, 0xE0, 0xB1, 0x79, 0x23, 0x2E, 0x39, 0xA4,
    0xC9, 0x99, 0xE2, 0x15, 0xDB, 0x1D, 0x81, 0xD9, 0x9E, 0x86, 0x00, 0x33, 0xBE, 0xFD, 0x51, 0x2C,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqE6TextSpec {
    pub scope: &'static str,
    pub exe: &'static str,
    pub seq_archive: &'static str,
    pub grp_archive: &'static str,
    pub command_source_table_runtime: u32,
    pub command_handler_runtime: u32,
    pub active_flag_runtime: u32,
    pub parser_runtime: u32,
    pub token_table_runtime: u32,
    pub resource_table_runtime: u32,
    pub glyph_resource: u16,
    pub glyph_asset: &'static str,
    pub glyph_token_load_runtime: u32,
    pub glyph_limit_runtime: Option<u32>,
    pub glyph_resource_load_runtime: u32,
    pub glyph_resource_store_runtime: u32,
    pub glyph_cell_store_runtime: u32,
}

pub const BOX_SUBGAME_SEQ_E6_TEXT_SPECS: [SubgameSeqE6TextSpec; 2] = [
    SubgameSeqE6TextSpec {
        scope: "PUYO1",
        exe: "PUYO1.EXE",
        seq_archive: "/PUYO1/PP1_SEQ.VDD;1",
        grp_archive: "/PUYO1/PP1_GRP.VDD;1",
        command_source_table_runtime: 0x8008_9310,
        command_handler_runtime: 0x8001_9024,
        active_flag_runtime: 0x800B_47C8,
        parser_runtime: 0x8001_852C,
        token_table_runtime: 0x800B_3000,
        resource_table_runtime: 0x8006_7E48,
        glyph_resource: 0x006D,
        glyph_asset: "CHR.BTX",
        glyph_token_load_runtime: 0x8001_8E4C,
        glyph_limit_runtime: None,
        glyph_resource_load_runtime: 0x8001_8EC8,
        glyph_resource_store_runtime: 0x8001_8ECC,
        glyph_cell_store_runtime: 0x8001_8ED4,
    },
    SubgameSeqE6TextSpec {
        scope: "PUYO2",
        exe: "PUYO2.EXE",
        seq_archive: "/PUYO2/PP2_SEQ.VDD;1",
        grp_archive: "/PUYO2/PP2_GRP.VDD;1",
        command_source_table_runtime: 0x8008_B360,
        command_handler_runtime: 0x8001_7A5C,
        active_flag_runtime: 0x800B_6BA8,
        parser_runtime: 0x8001_6F30,
        token_table_runtime: 0x800B_53E0,
        resource_table_runtime: 0x8006_3E9C,
        glyph_resource: 0x0088,
        glyph_asset: "DEMOFONT.BTX",
        glyph_token_load_runtime: 0x8001_6E24,
        glyph_limit_runtime: Some(0x8001_6E2C),
        glyph_resource_load_runtime: 0x8001_6E7C,
        glyph_resource_store_runtime: 0x8001_6E84,
        glyph_cell_store_runtime: 0x8001_6E88,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6TextSpan {
    pub offset: usize,
    pub raw: Vec<u8>,
    pub decoded: String,
    pub candidate_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6TextRow {
    pub scope: &'static str,
    pub archive: String,
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub sequence_index: usize,
    pub command_offset: usize,
    pub command_runtime: u32,
    pub pointer_field_offset: usize,
    pub pointer_field_runtime: u32,
    pub stream_offset: usize,
    pub stream_runtime: u32,
    pub raw: Vec<u8>,
    pub raw_sha256: [u8; 32],
    pub spans: Vec<SubgameSeqE6TextSpan>,
    pub control_70: usize,
    pub control_72: usize,
    pub control_77: usize,
    pub emitted_tokens: usize,
    pub counterpart_raw_equal: bool,
    pub glyph_resource: u16,
    pub glyph_asset: &'static str,
}

impl SubgameSeqE6TextRow {
    pub fn code_units(&self) -> usize {
        self.spans
            .iter()
            .map(|span| span.decoded.chars().count())
            .sum()
    }

    pub fn pairs(&self) -> usize {
        self.raw.len() / 2
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6TextAnalysis {
    pub rows: Vec<SubgameSeqE6TextRow>,
    pub consumers: usize,
    pub physical_sequences: usize,
    pub logical_sequences: usize,
    pub spans: usize,
    pub code_units: usize,
    pub unique_glyph_pairs: usize,
    pub candidate_spans: usize,
    pub exact_e6_roots: usize,
    pub exact_e1_roots: usize,
    pub control_70: usize,
    pub control_72: usize,
    pub control_77: usize,
    pub terminators: usize,
    pub min_pairs: usize,
    pub max_pairs: usize,
    pub max_emitted_tokens: usize,
    pub counterpart_equal: usize,
    pub counterpart_different: usize,
    pub code_table_unique_pairs: usize,
    pub resource_links: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6TextError(String);

impl fmt::Display for SubgameSeqE6TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqE6TextError {}

fn error(message: impl Into<String>) -> SubgameSeqE6TextError {
    SubgameSeqE6TextError(message.into())
}

fn runtime_slice<'a>(
    map: &ExeMap,
    text: &'a [u8],
    runtime: u32,
    len: usize,
) -> Result<&'a [u8], SubgameSeqE6TextError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "{} E6 runtime이 EXE 밖임: 0x{runtime:08X}",
            map.name
        ))
    })?;
    text.get(offset..offset + len).ok_or_else(|| {
        error(format!(
            "{} E6 runtime 범위 이탈: 0x{runtime:08X}",
            map.name
        ))
    })
}

fn require(
    instructions: &[Instruction],
    map: &ExeMap,
    runtime: u32,
    expected: K,
) -> Result<(), SubgameSeqE6TextError> {
    let offset = map
        .text_offset(runtime)
        .ok_or_else(|| error(format!("{} E6 명령이 EXE 밖임: 0x{runtime:08X}", map.name)))?;
    let actual = instructions
        .get(offset / 4)
        .filter(|instruction| instruction.addr == runtime)
        .ok_or_else(|| error(format!("{} E6 명령 누락: 0x{runtime:08X}", map.name)))?;
    if actual.kind != expected {
        return Err(error(format!(
            "{} E6 명령 불일치: 0x{runtime:08X}, expected={expected:?}, actual={:?}",
            map.name, actual.kind
        )));
    }
    Ok(())
}

fn verify_consumer(
    map: &ExeMap,
    text: &[u8],
    spec: SubgameSeqE6TextSpec,
) -> Result<BTreeSet<[u8; 2]>, SubgameSeqE6TextError> {
    if map.name != spec.exe || text.len() != map.text_len as usize {
        return Err(error(format!("{} E6 EXE 계약 불일치", spec.exe)));
    }
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error(format!("{} E6 EXE text 정렬 불일치", spec.exe)))?;
    let table = runtime_slice(map, text, spec.command_source_table_runtime, 32 * 4)?;
    let e6_handler = u32::from_le_bytes(
        table[6 * 4..7 * 4]
            .try_into()
            .expect("E6 command table word"),
    );
    if e6_handler != spec.command_handler_runtime {
        return Err(error(format!(
            "{} E6 command table handler 불일치: expected=0x{:08X}, actual=0x{e6_handler:08X}",
            spec.exe, spec.command_handler_runtime
        )));
    }

    let handler = spec.command_handler_runtime;
    require(
        &instructions,
        map,
        handler + 0x08,
        K::Addu {
            rd: Reg::S0,
            rs: Reg::A0,
            rt: Reg::ZERO,
        },
    )?;
    require(
        &instructions,
        map,
        handler + 0x10,
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 1,
        },
    )?;
    let flag_high = (spec.active_flag_runtime >> 16) as u16;
    let flag_low = spec.active_flag_runtime as u16 as i16;
    require(
        &instructions,
        map,
        handler + 0x0C,
        K::Lui {
            rt: Reg::V1,
            imm: flag_high,
        },
    )?;
    require(
        &instructions,
        map,
        handler + 0x18,
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: flag_low,
        },
    )?;
    for (delta, expected) in [
        (
            0x1C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x1C,
            },
        ),
        (
            0x24,
            K::Lw {
                rt: Reg::A0,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x28,
            K::Jal {
                target: spec.parser_runtime,
            },
        ),
        (
            0x30,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x1C,
            },
        ),
        (
            0x38,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 8,
            },
        ),
        (
            0x3C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x1C,
            },
        ),
    ] {
        require(&instructions, map, handler + delta, expected)?;
    }

    let parser = spec.parser_runtime;
    for (delta, expected) in [
        (
            0x0C,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8001,
            },
        ),
        (
            0x10,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V0,
                imm: 0x00FC,
            },
        ),
        (
            0x20,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 0x0200,
            },
        ),
        (
            0x78,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 0x0200,
            },
        ),
        (
            0xC0,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 0x63,
            },
        ),
        (
            0xE4,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::T2,
                offset: 0,
            },
        ),
        (
            0xE8,
            K::Addiu {
                rt: Reg::T2,
                rs: Reg::T2,
                imm: 1,
            },
        ),
        (
            0xF0,
            K::Lbu {
                rt: Reg::A0,
                base: Reg::T2,
                offset: 0,
            },
        ),
        (
            0xF4,
            K::Addiu {
                rt: Reg::T2,
                rs: Reg::T2,
                imm: 1,
            },
        ),
        (
            0xFC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x65,
            },
        ),
        (
            0x160,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x72,
            },
        ),
        (
            0x168,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x77,
            },
        ),
        (
            0x190,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x77,
            },
        ),
        (
            0x198,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: 0x70,
            },
        ),
        (
            0x26C,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 2,
            },
        ),
        (
            0x27C,
            K::Srl {
                rd: Reg::V0,
                rt: Reg::A1,
                sa: 31,
            },
        ),
        (
            0x280,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::A1,
                rt: Reg::V0,
            },
        ),
        (
            0x284,
            K::Sra {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 1,
            },
        ),
    ] {
        require(&instructions, map, parser + delta, expected)?;
    }

    let token_high = (spec.token_table_runtime >> 16) as u16;
    let token_low = spec.token_table_runtime as u16 as i16;
    for (delta, expected) in [
        (
            0xC4,
            K::Lui {
                rt: Reg::V0,
                imm: token_high,
            },
        ),
        (
            0xC8,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: token_low,
            },
        ),
        (
            0xCC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 99 * 4,
            },
        ),
        (
            0xD0,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0xD4,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: -1,
            },
        ),
        (
            0xD8,
            K::Bgez {
                rs: Reg::V1,
                offset: -3,
            },
        ),
        (
            0xDC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -4,
            },
        ),
    ] {
        require(&instructions, map, parser + delta, expected)?;
    }

    let code_table = runtime_slice(map, text, CODE_TABLE_RUNTIME, CODE_TABLE_BYTES)?;
    let digest: [u8; 32] = Sha256::digest(code_table).into();
    if digest != CODE_TABLE_SHA256 {
        return Err(error(format!("{} E6 code table digest 불일치", spec.exe)));
    }
    let mut pairs = BTreeSet::new();
    for pair in code_table.chunks_exact(2) {
        let pair: [u8; 2] = pair.try_into().expect("2바이트 code table pair");
        let decoded = decode_sjis(&pair)
            .map_err(|source| error(format!("{} E6 code table decode 실패: {source}", spec.exe)))?;
        if decoded.chars().count() != 1 {
            return Err(error(format!("{} E6 code table 다문자 entry", spec.exe)));
        }
        pairs.insert(pair);
    }

    require(
        &instructions,
        map,
        spec.glyph_token_load_runtime,
        K::Lw {
            rt: Reg::S1,
            base: Reg::V0,
            offset: 0,
        },
    )?;
    if let Some(runtime) = spec.glyph_limit_runtime {
        require(
            &instructions,
            map,
            runtime,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::S1,
                imm: 0x0100,
            },
        )?;
    }
    require(
        &instructions,
        map,
        spec.glyph_resource_load_runtime,
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: spec.glyph_resource as i16,
        },
    )?;
    require(
        &instructions,
        map,
        spec.glyph_resource_store_runtime,
        K::Sh {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x6C,
        },
    )?;
    require(
        &instructions,
        map,
        spec.glyph_cell_store_runtime,
        K::Sh {
            rt: Reg::S1,
            base: Reg::V0,
            offset: 0x6A,
        },
    )?;
    Ok(pairs)
}

#[derive(Debug)]
struct ParsedStream {
    raw: Vec<u8>,
    spans: Vec<(usize, Vec<u8>, String)>,
    control_70: usize,
    control_72: usize,
    control_77: usize,
    glyph_pairs: Vec<[u8; 2]>,
}

fn parse_stream(
    raw: &[u8],
    start: usize,
    end: usize,
    code_table: &BTreeSet<[u8; 2]>,
) -> Result<ParsedStream, SubgameSeqE6TextError> {
    if start >= end || end > raw.len() || !start.is_multiple_of(2) || !end.is_multiple_of(2) {
        return Err(error("E6 stream 시작/끝 경계 불일치"));
    }
    let mut cursor = start;
    let mut spans = Vec::new();
    let mut span_start = None;
    let mut span_raw = Vec::new();
    let mut glyph_pairs = Vec::new();
    let mut control_70 = 0;
    let mut control_72 = 0;
    let mut control_77 = 0;

    let flush = |spans: &mut Vec<(usize, Vec<u8>, String)>,
                 span_start: &mut Option<usize>,
                 span_raw: &mut Vec<u8>|
     -> Result<(), SubgameSeqE6TextError> {
        if let Some(offset) = span_start.take() {
            let decoded = decode_sjis(span_raw)
                .map_err(|source| error(format!("E6 glyph span decode 실패: {source}")))?;
            spans.push((offset, std::mem::take(span_raw), decoded));
        }
        Ok(())
    };

    loop {
        let pair_end = cursor
            .checked_add(2)
            .filter(|pair_end| *pair_end <= end)
            .ok_or_else(|| error(format!("E6 stream terminator 없음: +0x{start:04X}")))?;
        let pair_raw = &raw[cursor..pair_end];
        let pair: [u8; 2] = pair_raw.try_into().expect("2바이트 E6 pair");
        cursor += 2;
        if pair == TERMINATOR {
            flush(&mut spans, &mut span_start, &mut span_raw)?;
            break;
        }
        if pair == CONTROL_70 || pair == CONTROL_72 || pair == CONTROL_77 {
            flush(&mut spans, &mut span_start, &mut span_raw)?;
            match pair {
                CONTROL_70 => control_70 += 1,
                CONTROL_72 => control_72 += 1,
                CONTROL_77 => control_77 += 1,
                _ => unreachable!(),
            }
            continue;
        }
        if !code_table.contains(&pair) {
            return Err(error(format!(
                "E6 glyph pair가 code table에 없음: +0x{:04X} {:02X}{:02X}",
                cursor - 2,
                pair[0],
                pair[1]
            )));
        }
        let decoded = decode_sjis(&pair)
            .map_err(|source| error(format!("E6 glyph pair decode 실패: {source}")))?;
        if decoded.chars().count() != 1 {
            return Err(error("E6 glyph pair가 단일 문자가 아님"));
        }
        if span_start.is_none() {
            span_start = Some(cursor - 2);
        }
        span_raw.extend_from_slice(&pair);
        glyph_pairs.push(pair);
    }
    Ok(ParsedStream {
        raw: raw[start..cursor].to_vec(),
        spans,
        control_70,
        control_72,
        control_77,
        glyph_pairs,
    })
}

/// 지원 원본의 두 EXE와 모든 PP1/PP2 overlay에서 `E6` text를 검증한다.
pub fn audit_subgame_seq_e6_text<'a>(
    executables: impl IntoIterator<Item = (&'a ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    layouts: &SubgameSeqLayoutAnalysis,
    roots: &SubgameSeqTextRootAnalysis,
    btx_names: &BtxNameAnalysis,
) -> Result<SubgameSeqE6TextAnalysis, SubgameSeqE6TextError> {
    let mut executable_map = BTreeMap::new();
    for (map, text) in executables {
        if executable_map.insert(map.name, (map, text)).is_some() {
            return Err(error(format!("{} E6 EXE 입력 중복", map.name)));
        }
    }
    if executable_map.len() != BOX_SUBGAME_SEQ_E6_TEXT_SPECS.len()
        || executable_map.keys().any(|exe| {
            !BOX_SUBGAME_SEQ_E6_TEXT_SPECS
                .iter()
                .any(|spec| spec.exe == *exe)
        })
    {
        return Err(error("E6 EXE 입력 분모 불일치"));
    }

    let mut source_map = BTreeMap::new();
    for (archive, sequence, raw) in sources {
        let key = (archive.to_string(), sequence.to_string());
        if source_map.insert(key, raw).is_some() {
            return Err(error(format!("E6 SEQ 입력 중복: {archive}:{sequence}")));
        }
    }
    if source_map.len() != layouts.rows.len() {
        return Err(error("E6 source/layout 분모 불일치"));
    }

    let mut code_tables = BTreeMap::new();
    let mut resource_links = 0usize;
    for &spec in &BOX_SUBGAME_SEQ_E6_TEXT_SPECS {
        let &(map, text) = executable_map
            .get(spec.exe)
            .ok_or_else(|| error(format!("{} E6 EXE 누락", spec.exe)))?;
        let pairs = verify_consumer(map, text, spec)?;
        code_tables.insert(spec.scope, pairs);

        let descriptor_pointer_runtime = spec
            .resource_table_runtime
            .checked_add(u32::from(spec.glyph_resource) * 36 + 4)
            .ok_or_else(|| error("E6 glyph descriptor pointer overflow"))?;
        let links = btx_names
            .links
            .iter()
            .filter(|link| {
                link.exe == spec.exe
                    && link.archive == spec.grp_archive
                    && link.asset_name == spec.glyph_asset
                    && link.descriptor_pointer_runtime == descriptor_pointer_runtime
            })
            .count();
        if links != 1 {
            return Err(error(format!(
                "{} E6 glyph resource/name 연결 불일치: resource={} asset={} links={links}",
                spec.exe, spec.glyph_resource, spec.glyph_asset
            )));
        }
        resource_links += links;
    }
    if code_tables.get("PUYO1") != code_tables.get("PUYO2") {
        return Err(error("PUYO1/PUYO2 E6 code table 불일치"));
    }

    let mut exact_e6_roots = 0usize;
    let mut exact_e1_roots = 0usize;
    for row in &roots.rows {
        let raw = source_map
            .get(&(row.archive.clone(), row.sequence.clone()))
            .ok_or_else(|| error("E6 root 원본 누락"))?;
        for root in row.roots.iter().filter(|root| root.candidate_delta == 0) {
            let command_offset = root
                .pointer_field_offset
                .checked_sub(4)
                .ok_or_else(|| error("exact root command offset underflow"))?;
            match raw.get(command_offset..root.pointer_field_offset) {
                Some(command) if command == E6_COMMAND => exact_e6_roots += 1,
                Some(command) if command == E1_COMMAND => exact_e1_roots += 1,
                Some(command) => {
                    return Err(error(format!(
                        "exact root command 미분류: {}:{}:+0x{command_offset:04X} {command:02X?}",
                        row.archive, row.sequence
                    )));
                }
                None => return Err(error("exact root command 범위 이탈")),
            }
        }
    }

    let mut rows = Vec::new();
    let mut all_glyph_pairs = BTreeSet::new();
    for &spec in &BOX_SUBGAME_SEQ_E6_TEXT_SPECS {
        let code_table = code_tables
            .get(spec.scope)
            .ok_or_else(|| error("E6 scope code table 누락"))?;
        for layout in layouts.rows.iter().filter(|layout| {
            layout.archive == spec.seq_archive && layout.kind == SubgameSeqLayoutKind::Overlay
        }) {
            let raw = source_map
                .get(&(layout.archive.clone(), layout.sequence.clone()))
                .ok_or_else(|| error("E6 layout 원본 누락"))?;
            let footer = layout
                .footer
                .ok_or_else(|| error("E6 overlay footer 누락"))?;
            let map = footer
                .runtime_map()
                .ok_or_else(|| error("E6 overlay runtime 사상 overflow"))?;
            let initialized_len = usize::try_from(footer.initialized_len)
                .map_err(|_| error("E6 initialized 길이 변환 실패"))?;
            for command_offset in (0..=initialized_len.saturating_sub(8)).step_by(4) {
                if raw.get(command_offset..command_offset + 4) != Some(E6_COMMAND.as_slice()) {
                    continue;
                }
                let pointer_field_offset = command_offset + 4;
                let stream_runtime = u32::from_le_bytes(
                    raw[pointer_field_offset..pointer_field_offset + 4]
                        .try_into()
                        .expect("E6 pointer word"),
                );
                let stream_offset = map.file_offset(stream_runtime).ok_or_else(|| {
                    error(format!(
                        "{}:{} canonical E6의 pointer가 자기 overlay 밖임: +0x{command_offset:04X}",
                        spec.scope, layout.sequence
                    ))
                })?;
                let parsed = parse_stream(raw, stream_offset, initialized_len, code_table)?;
                let mut spans = Vec::new();
                for (offset, span_raw, decoded) in parsed.spans {
                    let matches = roots
                        .rows
                        .iter()
                        .filter(|root| {
                            root.archive == layout.archive
                                && root.sequence == layout.sequence
                                && root.candidate_offset == offset
                                && root.candidate_bytes == span_raw.len()
                                && root.source.as_deref() == Some(decoded.as_str())
                        })
                        .collect::<Vec<_>>();
                    let [candidate] = matches.as_slice() else {
                        return Err(error(format!(
                            "E6 glyph span/candidate 연결 불일치: {}:{}:+0x{offset:04X} matches={}",
                            layout.archive,
                            layout.sequence,
                            matches.len()
                        )));
                    };
                    spans.push(SubgameSeqE6TextSpan {
                        offset,
                        raw: span_raw,
                        decoded,
                        candidate_id: candidate.candidate_id.clone(),
                    });
                }
                let exact = roots
                    .rows
                    .iter()
                    .filter(|row| {
                        row.archive == layout.archive
                            && row.sequence == layout.sequence
                            && row.candidate_offset == stream_offset
                            && row.roots.iter().any(|root| {
                                root.candidate_delta == 0
                                    && root.pointer_field_offset == pointer_field_offset
                            })
                    })
                    .count();
                if exact != 1 {
                    return Err(error(format!(
                        "E6 command/exact root 연결 불일치: {}:{}:+0x{command_offset:04X} exact={exact}",
                        layout.archive, layout.sequence
                    )));
                }
                all_glyph_pairs.extend(parsed.glyph_pairs);
                let pair_count = parsed.raw.len() / 2;
                let control_tokens = parsed.control_70 + parsed.control_72 + parsed.control_77;
                let glyph_tokens = spans
                    .iter()
                    .map(|span| span.decoded.chars().count())
                    .sum::<usize>();
                let emitted_tokens = glyph_tokens + control_tokens + 4;
                rows.push(SubgameSeqE6TextRow {
                    scope: spec.scope,
                    archive: layout.archive.clone(),
                    sequence: layout.sequence.clone(),
                    sequence_sha256: layout.sequence_sha256,
                    sequence_index: 0,
                    command_offset,
                    command_runtime: map
                        .runtime_addr(command_offset)
                        .ok_or_else(|| error("E6 command runtime 변환 실패"))?,
                    pointer_field_offset,
                    pointer_field_runtime: map
                        .runtime_addr(pointer_field_offset)
                        .ok_or_else(|| error("E6 pointer runtime 변환 실패"))?,
                    stream_offset,
                    stream_runtime,
                    raw_sha256: Sha256::digest(&parsed.raw).into(),
                    raw: parsed.raw,
                    spans,
                    control_70: parsed.control_70,
                    control_72: parsed.control_72,
                    control_77: parsed.control_77,
                    emitted_tokens,
                    counterpart_raw_equal: false,
                    glyph_resource: spec.glyph_resource,
                    glyph_asset: spec.glyph_asset,
                });
                if pair_count > 100 || emitted_tokens > 100 {
                    return Err(error("E6 parser 100-word token table 용량 초과"));
                }
            }
        }
    }

    rows.sort_by(|left, right| {
        (left.scope, left.sequence.as_str(), left.command_offset).cmp(&(
            right.scope,
            right.sequence.as_str(),
            right.command_offset,
        ))
    });
    let mut per_sequence = BTreeMap::<(&str, String), usize>::new();
    for row in &mut rows {
        let next = per_sequence
            .entry((row.scope, row.sequence.clone()))
            .or_default();
        *next += 1;
        row.sequence_index = *next;
    }
    let mut counterpart_groups = BTreeMap::<(String, usize), Vec<usize>>::new();
    for (index, row) in rows.iter().enumerate() {
        counterpart_groups
            .entry((row.sequence.clone(), row.sequence_index))
            .or_default()
            .push(index);
    }
    let mut counterpart_equal = 0usize;
    let mut counterpart_different = 0usize;
    for ((sequence, sequence_index), indexes) in counterpart_groups {
        let [left, right] = indexes.as_slice() else {
            return Err(error(format!(
                "E6 counterpart 분모 불일치: {sequence}#{sequence_index} copies={}",
                indexes.len()
            )));
        };
        let equal = rows[*left].raw == rows[*right].raw;
        rows[*left].counterpart_raw_equal = equal;
        rows[*right].counterpart_raw_equal = equal;
        if equal {
            counterpart_equal += 1;
        } else {
            counterpart_different += 1;
        }
    }

    let physical_sequences = rows
        .iter()
        .map(|row| (row.scope, row.sequence.as_str()))
        .collect::<BTreeSet<_>>()
        .len();
    let logical_sequences = rows
        .iter()
        .map(|row| row.sequence.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let min_pairs = rows
        .iter()
        .map(SubgameSeqE6TextRow::pairs)
        .min()
        .unwrap_or(0);
    let max_pairs = rows
        .iter()
        .map(SubgameSeqE6TextRow::pairs)
        .max()
        .unwrap_or(0);
    let max_emitted_tokens = rows.iter().map(|row| row.emitted_tokens).max().unwrap_or(0);
    Ok(SubgameSeqE6TextAnalysis {
        consumers: BOX_SUBGAME_SEQ_E6_TEXT_SPECS.len(),
        physical_sequences,
        logical_sequences,
        spans: rows.iter().map(|row| row.spans.len()).sum(),
        code_units: rows.iter().map(SubgameSeqE6TextRow::code_units).sum(),
        unique_glyph_pairs: all_glyph_pairs.len(),
        candidate_spans: rows.iter().map(|row| row.spans.len()).sum(),
        exact_e6_roots,
        exact_e1_roots,
        control_70: rows.iter().map(|row| row.control_70).sum(),
        control_72: rows.iter().map(|row| row.control_72).sum(),
        control_77: rows.iter().map(|row| row.control_77).sum(),
        terminators: rows.len(),
        min_pairs,
        max_pairs,
        max_emitted_tokens,
        counterpart_equal,
        counterpart_different,
        code_table_unique_pairs: code_tables.get("PUYO1").map_or(0, BTreeSet::len),
        resource_links,
        rows,
    })
}

/// 지원 원본의 E6 direct text 분모와 결정적 report를 고정한다.
pub fn verify_box_subgame_seq_e6_text<'a>(
    executables: impl IntoIterator<Item = (&'a ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    layouts: &SubgameSeqLayoutAnalysis,
    roots: &SubgameSeqTextRootAnalysis,
    btx_names: &BtxNameAnalysis,
) -> Result<SubgameSeqE6TextAnalysis, SubgameSeqE6TextError> {
    let analysis = audit_subgame_seq_e6_text(executables, sources, layouts, roots, btx_names)?;
    let per_scope = BOX_SUBGAME_SEQ_E6_TEXT_SPECS.map(|spec| {
        analysis
            .rows
            .iter()
            .filter(|row| row.scope == spec.scope)
            .count()
    });
    let actual = [
        analysis.rows.len(),
        analysis.physical_sequences,
        analysis.logical_sequences,
        per_scope[0],
        per_scope[1],
        analysis.spans,
        analysis.code_units,
        analysis.unique_glyph_pairs,
        analysis.candidate_spans,
        analysis.exact_e6_roots,
        analysis.exact_e1_roots,
        analysis.control_70,
        analysis.control_72,
        analysis.control_77,
        analysis.terminators,
        analysis.min_pairs,
        analysis.max_pairs,
        analysis.max_emitted_tokens,
        analysis.counterpart_equal,
        analysis.counterpart_different,
        analysis.code_table_unique_pairs,
        analysis.resource_links,
    ];
    let expected = [
        BOX_SUBGAME_SEQ_E6_TEXT_ROWS,
        BOX_SUBGAME_SEQ_E6_TEXT_PHYSICAL_SEQUENCES,
        BOX_SUBGAME_SEQ_E6_TEXT_LOGICAL_SEQUENCES,
        BOX_SUBGAME_SEQ_E6_TEXT_ROWS_PER_EXE,
        BOX_SUBGAME_SEQ_E6_TEXT_ROWS_PER_EXE,
        BOX_SUBGAME_SEQ_E6_TEXT_SPANS,
        BOX_SUBGAME_SEQ_E6_TEXT_CODE_UNITS,
        BOX_SUBGAME_SEQ_E6_TEXT_UNIQUE_PAIRS,
        BOX_SUBGAME_SEQ_E6_TEXT_SPANS,
        BOX_SUBGAME_SEQ_E6_TEXT_EXACT_ROOTS,
        BOX_SUBGAME_SEQ_E1_REPERTOIRE_EXACT_ROOTS,
        BOX_SUBGAME_SEQ_E6_TEXT_CONTROL_70,
        BOX_SUBGAME_SEQ_E6_TEXT_CONTROL_72,
        BOX_SUBGAME_SEQ_E6_TEXT_CONTROL_77,
        BOX_SUBGAME_SEQ_E6_TEXT_TERMINATORS,
        BOX_SUBGAME_SEQ_E6_TEXT_MIN_PAIRS,
        BOX_SUBGAME_SEQ_E6_TEXT_MAX_PAIRS,
        BOX_SUBGAME_SEQ_E6_TEXT_MAX_EMITTED_TOKENS,
        BOX_SUBGAME_SEQ_E6_TEXT_COUNTERPART_EQUAL,
        BOX_SUBGAME_SEQ_E6_TEXT_COUNTERPART_DIFFERENT,
        BOX_SUBGAME_SEQ_E6_TEXT_CODE_TABLE_UNIQUE_PAIRS,
        BOX_SUBGAME_SEQ_E6_TEXT_SPECS.len(),
    ];
    if actual != expected {
        return Err(error(format!(
            "E6 direct text 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }
    let report = render_subgame_seq_e6_text_tsv(&analysis);
    let digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if digest != BOX_SUBGAME_SEQ_E6_TEXT_REPORT_SHA256 {
        return Err(error(format!(
            "E6 direct text report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_E6_TEXT_REPORT_SHA256),
            hex(&digest)
        )));
    }
    Ok(analysis)
}

pub fn render_subgame_seq_e6_text_tsv(analysis: &SubgameSeqE6TextAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tscope\tarchive\tsequence\tsequence_sha256\tsequence_index\t\
         command_offset\tcommand_runtime\tpointer_field_offset\tpointer_field_runtime\t\
         stream_offset\tstream_runtime\traw_sha256\traw_bytes\tpairs\temitted_tokens\t\
         spans\tcode_units\tcontrol_70\tcontrol_72\tcontrol_77\tterminators\t\
         counterpart_raw_equal\tglyph_resource\tglyph_asset\tcandidate_ids\t\
         consumer_contract\tpatch_strategy\ttext\n",
    );
    for row in &analysis.rows {
        let text = row
            .spans
            .iter()
            .map(|span| span.decoded.as_str())
            .collect::<Vec<_>>()
            .join("|");
        let candidate_ids = row
            .spans
            .iter()
            .map(|span| span.candidate_id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t0x{:08X}\t0x{:04X}\t0x{:08X}\t\
             0x{:04X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t1\t{}\t\
             0x{:04X}\t{}\t{}\te6-direct-pair-parser-static-runtime-unverified\t\
             preserve-no-write-until-resource-layout-and-runtime-display-contract\t{}",
            SUBGAME_SEQ_E6_TEXT_SCHEMA_VERSION,
            row.scope,
            row.archive,
            row.sequence,
            hex(&row.sequence_sha256),
            row.sequence_index,
            row.command_offset,
            row.command_runtime,
            row.pointer_field_offset,
            row.pointer_field_runtime,
            row.stream_offset,
            row.stream_runtime,
            hex(&row.raw_sha256),
            row.raw.len(),
            row.pairs(),
            row.emitted_tokens,
            row.spans.len(),
            row.code_units(),
            row.control_70,
            row.control_72,
            row.control_77,
            row.counterpart_raw_equal,
            row.glyph_resource,
            row.glyph_asset,
            candidate_ids,
            escape_tsv(&text),
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    output
}

fn escape_tsv(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_e6_text_tests.rs"]
mod subgame_seq_e6_text_tests;
