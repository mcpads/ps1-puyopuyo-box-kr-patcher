//! `MENU1.SEQ`가 `MENU06.BTX` glyph cell을 소비하는 u16 text stream.
//!
//! 이 경계는 두 `MENU1.SEQ`의 정적 pointer chain과 `MENU06.BTX` cell
//! setup을 연결한다. 실제 장면 실행·primitive draw·화면 표시는 별도
//! runtime 증거가 필요하며, 이 모듈의 결과만으로 닫지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

pub const MENU06_TEXT_REVIEW_SCHEMA_VERSION: &str = "menu06-text-review-v1";
pub const MENU06_TEXT_REVIEW_SHA256: &str =
    "62418b8b62a63040f2a03f03ec668f4d94668499452f9a2eea7965a7fd4a4138";

const MENU1_SEQUENCE: &str = "MENU1.SEQ";
const MENU1_BYTES: usize = 0x47EC;
const MENU1_INITIALIZED_BYTES: usize = 0x47C0;
const MENU1_LOAD_BASE: u32 = 0x801A_6270;
const ROOT_POINTER_FIELD_OFFSET: usize = 0x0084;
const ROOT_OFFSET: usize = 0x2D44;
const CHILD_POINTER_FIELD_OFFSET: usize = 0x2D5C;
const CHILD_OFFSET: usize = 0x2D84;
const STREAM_POINTER_FIELD_OFFSET: usize = 0x2D98;
pub const MENU06_STREAM_OFFSET: usize = 0x2DC0;
pub const MENU06_STREAM_END_OFFSET: usize = 0x30D8;
const STREAM_OFFSET: usize = MENU06_STREAM_OFFSET;
const STREAM_END_OFFSET: usize = MENU06_STREAM_END_OFFSET;
const STREAM_BYTES: usize = STREAM_END_OFFSET - STREAM_OFFSET;
const SETUP_OFFSET: usize = 0x2D00;
const CELL_TABLE_OFFSET: usize = 0x3B34;
const GLYPH_CELL_RECORDS: usize = 98;
const STATIC_CELL_PREFIX_RECORDS: usize = 153;
const CELL_HEADER_BYTES: usize = 12;
const CELL_RECORD_BYTES: usize = 12;
const STATIC_CELL_PREFIX_END: usize =
    CELL_TABLE_OFFSET + CELL_HEADER_BYTES + STATIC_CELL_PREFIX_RECORDS * CELL_RECORD_BYTES;
const PALETTE_SENTINEL_BYTES: usize = 32;

const STREAM_SHA256: &str = "bfb11ff68af8c4686c0e391512cd52c804ada2cf8f0282bedcaf800a3e50063e";
const GLYPH_TABLE_SHA256: &str = "20ea7c97e8aaa06f3d45983572df8bd7532215a73837db3ae21661a449e85b3f";
const STATIC_CELL_PREFIX_SHA256: &str =
    "f6ccae84dedbb0577210cedd7f8742c2dfd14c37312c8ab9c59e75e365ca484a";
const PALETTE_SENTINEL_SHA256: &str =
    "a03fc7d2f9fa68dd500fdd6fd03f170fbbabe4a34d49081ea7f6add54077b0fc";

const SOURCE_SPECS: [Menu06SourceSpec; 2] = [
    Menu06SourceSpec {
        archive: "/PUYO1/PP1_SEQ.VDD;1",
        sequence_sha256: "2dabfced36c1b47fc61a965ddfb23f38e86e102eaac6c58e24a7ddf778bde025",
        resource_id: 114,
    },
    Menu06SourceSpec {
        archive: "/PUYO2/PP2_SEQ.VDD;1",
        sequence_sha256: "af67a76295a8f0d97ef6984f6dbd55171ed4df85c756ea5148e8d8384ab91733",
        resource_id: 82,
    },
];

/// `MENU06.BTX` cell 0–97의 사람이 판독한 보호 원문.
///
/// cell 0은 8px 공백이고 나머지는 16×16 glyph다. 중복 `ぞ` 두 장도 cell
/// identity가 다르므로 합치지 않는다.
pub const MENU06_GLYPH_SOURCES: [&str; GLYPH_CELL_RECORDS] = [
    " ", "あ", "い", "う", "え", "お", "く", "こ", "し", "す", "せ", "そ", "ち", "つ", "て", "と",
    "な", "に", "ね", "の", "は", "ひ", "ほ", "ま", "み", "も", "や", "よ", "ら", "り", "る", "れ",
    "ろ", "わ", "を", "ん", "が", "げ", "じ", "ず", "ぞ", "だ", "ぢ", "で", "ば", "ぷ", "オ", "タ",
    "フ", "ム", "モ", "ル", "レ", "ン", "ゲ", "ド", "バ", "ボ", "ぁ", "ぞ", "ゃ", "っ", "ィ", "ー",
    "、", "。", "・", "！", "1", "2", "3", "4", "右", "左", "下", "入", "落", "速", "色", "匹",
    "消", "上", "積", "連", "鎖", "高", "得", "点", "対", "戦", "相", "手", "側", "降", "回", "覚",
    "応", "用",
];

#[derive(Debug, Clone, Copy)]
struct Menu06SourceSpec {
    archive: &'static str,
    sequence_sha256: &'static str,
    resource_id: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06SourceEvidence {
    pub archive: &'static str,
    pub sequence_sha256: String,
    pub resource_id: u16,
    pub root_pointer_runtime: u32,
    pub child_pointer_runtime: u32,
    pub stream_pointer_runtime: u32,
    pub cell_table_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06TextSegment {
    pub index: usize,
    pub start_offset: usize,
    pub end_offset: usize,
    pub glyph_cells: Vec<u16>,
    pub source: String,
}

/// `MENU06` stream에서 경계를 확정한 u16 token.
///
/// offset은 `MENU1.SEQ` 논리 파일 기준이다. control 의미는 아직 확정하지
/// 않았으므로 code와 원본 인자만 보존한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Menu06TextToken {
    Glyph {
        offset: usize,
        cell: u16,
    },
    Control {
        offset: usize,
        code: u16,
        arguments: Vec<u16>,
    },
}

impl Menu06TextToken {
    const fn offset(&self) -> usize {
        match self {
            Self::Glyph { offset, .. } | Self::Control { offset, .. } => *offset,
        }
    }
}

/// control과 뒤 padding을 고정했을 때의 보수적인 제자리 재작성 용량.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Menu06RewriteCapacity {
    pub stream_words: usize,
    pub glyph_positions: usize,
    pub control_words: usize,
    pub control_argument_words: usize,
    pub terminator_padding_words: usize,
    pub slack_words: usize,
    pub repertoire_slots: usize,
    pub used_repertoire_slots: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06TextAnalysis {
    pub sources: Vec<Menu06SourceEvidence>,
    pub segments: Vec<Menu06TextSegment>,
    pub tokens: Vec<Menu06TextToken>,
    pub glyph_tokens: usize,
    pub distinct_glyph_cells: usize,
    pub control_tokens: BTreeMap<u16, usize>,
    pub stream_sha256: String,
    pub static_cell_prefix_records: usize,
    pub static_valid_cells: usize,
    pub static_empty_cells: usize,
    pub rewrite_capacity: Menu06RewriteCapacity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu06TextError(String);

impl fmt::Display for Menu06TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Menu06TextError {}

fn error(message: impl Into<String>) -> Menu06TextError {
    Menu06TextError(message.into())
}

fn read_u16(raw: &[u8], offset: usize) -> Result<u16, Menu06TextError> {
    let bytes = raw
        .get(offset..offset + 2)
        .ok_or_else(|| error(format!("MENU06 u16 범위 이탈: +0x{offset:04X}")))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_i16(raw: &[u8], offset: usize) -> Result<i16, Menu06TextError> {
    Ok(read_u16(raw, offset)? as i16)
}

fn read_u32(raw: &[u8], offset: usize) -> Result<u32, Menu06TextError> {
    let bytes = raw
        .get(offset..offset + 4)
        .ok_or_else(|| error(format!("MENU06 u32 범위 이탈: +0x{offset:04X}")))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("4바이트 slice")))
}

fn sha256(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}

fn control_arity(code: u16) -> Option<usize> {
    match code {
        0xFF00 | 0xFF02 | 0xFF0A => Some(0),
        0xFF04 | 0xFF17 => Some(1),
        0xFF13 => Some(2),
        _ => None,
    }
}

fn finish_segment(
    segments: &mut Vec<Menu06TextSegment>,
    cells: &mut Vec<u16>,
    start_offset: &mut Option<usize>,
    end_offset: usize,
) -> Result<(), Menu06TextError> {
    if cells.is_empty() {
        return Err(error(format!(
            "MENU06 FF02 구간이 비어 있음: +0x{end_offset:04X}"
        )));
    }
    let source = cells
        .iter()
        .map(|cell| MENU06_GLYPH_SOURCES[usize::from(*cell)])
        .collect::<String>();
    segments.push(Menu06TextSegment {
        index: segments.len(),
        start_offset: start_offset
            .take()
            .ok_or_else(|| error("MENU06 구간 시작 offset 누락"))?,
        end_offset,
        glyph_cells: std::mem::take(cells),
        source,
    });
    Ok(())
}

fn parse_stream(raw: &[u8]) -> Result<Menu06TextAnalysis, Menu06TextError> {
    if raw.len() != STREAM_BYTES {
        return Err(error(format!(
            "MENU06 text stream 길이 불일치: expected={STREAM_BYTES}, actual={}",
            raw.len()
        )));
    }
    let mut offset = 0usize;
    let mut segments = Vec::new();
    let mut tokens = Vec::new();
    let mut cells = Vec::new();
    let mut segment_start = None;
    let mut glyph_tokens = 0usize;
    let mut glyph_cells = BTreeSet::new();
    let mut control_tokens = BTreeMap::<u16, usize>::new();
    let mut terminated = false;

    while offset < raw.len() {
        let token_offset = offset;
        let value = read_u16(raw, offset)?;
        offset += 2;
        if let Some(arity) = control_arity(value) {
            *control_tokens.entry(value).or_default() += 1;
            let argument_bytes = arity
                .checked_mul(2)
                .ok_or_else(|| error("MENU06 control argument 길이 overflow"))?;
            if offset + argument_bytes > raw.len() {
                return Err(error(format!(
                    "MENU06 control 인자 범위 이탈: code=0x{value:04X}, +0x{token_offset:04X}"
                )));
            }
            let arguments = (0..arity)
                .map(|index| read_u16(raw, offset + index * 2))
                .collect::<Result<Vec<_>, _>>()?;
            tokens.push(Menu06TextToken::Control {
                offset: STREAM_OFFSET + token_offset,
                code: value,
                arguments,
            });
            if value == 0xFF02 {
                finish_segment(
                    &mut segments,
                    &mut cells,
                    &mut segment_start,
                    STREAM_OFFSET + token_offset,
                )?;
            }
            if value == 0xFF00 {
                finish_segment(
                    &mut segments,
                    &mut cells,
                    &mut segment_start,
                    STREAM_OFFSET + token_offset,
                )?;
                let padding = &raw[offset..];
                if padding != [0; 6] {
                    return Err(error(format!(
                        "MENU06 FF00 뒤 padding 불일치: {:02X?}",
                        padding
                    )));
                }
                terminated = true;
                break;
            }
            offset += argument_bytes;
            continue;
        }
        if usize::from(value) >= MENU06_GLYPH_SOURCES.len() {
            return Err(error(format!(
                "MENU06 미등록 token: +0x{token_offset:04X}=0x{value:04X}"
            )));
        }
        segment_start.get_or_insert(STREAM_OFFSET + token_offset);
        tokens.push(Menu06TextToken::Glyph {
            offset: STREAM_OFFSET + token_offset,
            cell: value,
        });
        cells.push(value);
        glyph_tokens += 1;
        glyph_cells.insert(value);
    }
    if !terminated {
        return Err(error("MENU06 text stream에 FF00 종료가 없음"));
    }
    let control_argument_words = tokens
        .iter()
        .map(|token| match token {
            Menu06TextToken::Control { arguments, .. } => arguments.len(),
            Menu06TextToken::Glyph { .. } => 0,
        })
        .sum::<usize>();
    let control_words = control_tokens.values().sum::<usize>();
    let terminator_padding_words = 3;
    let used_words = glyph_tokens
        .checked_add(control_words)
        .and_then(|value| value.checked_add(control_argument_words))
        .and_then(|value| value.checked_add(terminator_padding_words))
        .ok_or_else(|| error("MENU06 stream word 합계 overflow"))?;
    let stream_words = STREAM_BYTES / 2;
    let slack_words = stream_words
        .checked_sub(used_words)
        .ok_or_else(|| error("MENU06 stream word 합계가 고정 span을 초과함"))?;
    Ok(Menu06TextAnalysis {
        sources: Vec::new(),
        segments,
        tokens,
        glyph_tokens,
        distinct_glyph_cells: glyph_cells.len(),
        control_tokens,
        stream_sha256: sha256(raw),
        static_cell_prefix_records: 0,
        static_valid_cells: 0,
        static_empty_cells: 0,
        rewrite_capacity: Menu06RewriteCapacity {
            stream_words,
            glyph_positions: glyph_tokens,
            control_words,
            control_argument_words,
            terminator_padding_words,
            slack_words,
            repertoire_slots: GLYPH_CELL_RECORDS,
            used_repertoire_slots: glyph_cells.len(),
        },
    })
}

/// typed token을 고정 792B stream으로 직렬화한다.
///
/// token offset, control arity, glyph cell 범위, 단일 종결자와 6B padding을
/// 모두 검증해 실패 뒤 부분 결과를 반환하지 않는다.
pub fn serialize_menu06_text_tokens(
    tokens: &[Menu06TextToken],
) -> Result<Vec<u8>, Menu06TextError> {
    let mut output = Vec::with_capacity(STREAM_BYTES);
    let mut terminated = false;
    for token in tokens {
        if terminated {
            return Err(error("MENU06 FF00 뒤 token이 존재함"));
        }
        let expected_offset = STREAM_OFFSET
            .checked_add(output.len())
            .ok_or_else(|| error("MENU06 token offset overflow"))?;
        if token.offset() != expected_offset {
            return Err(error(format!(
                "MENU06 token offset 불일치: expected=+0x{expected_offset:04X}, actual=+0x{:04X}",
                token.offset()
            )));
        }
        match token {
            Menu06TextToken::Glyph { cell, .. } => {
                if usize::from(*cell) >= GLYPH_CELL_RECORDS {
                    return Err(error(format!("MENU06 glyph cell 범위 이탈: {cell}")));
                }
                output.extend_from_slice(&cell.to_le_bytes());
            }
            Menu06TextToken::Control {
                code, arguments, ..
            } => {
                let arity = control_arity(*code)
                    .ok_or_else(|| error(format!("MENU06 미등록 control: 0x{code:04X}")))?;
                if arguments.len() != arity {
                    return Err(error(format!(
                        "MENU06 control arity 불일치: code=0x{code:04X}, expected={arity}, actual={}",
                        arguments.len()
                    )));
                }
                output.extend_from_slice(&code.to_le_bytes());
                for argument in arguments {
                    output.extend_from_slice(&argument.to_le_bytes());
                }
                if *code == 0xFF00 {
                    output.extend_from_slice(&[0; 6]);
                    terminated = true;
                }
            }
        }
    }
    if !terminated {
        return Err(error("MENU06 typed stream에 FF00 종료가 없음"));
    }
    if output.len() != STREAM_BYTES {
        return Err(error(format!(
            "MENU06 직렬화 길이 불일치: expected={STREAM_BYTES}, actual={}",
            output.len()
        )));
    }
    Ok(output)
}

/// 원본 control·인자·token 위치를 바꾸지 않는 fixed-slot rewrite를 검증한다.
pub fn serialize_menu06_control_preserving_rewrite(
    source: &[Menu06TextToken],
    replacement: &[Menu06TextToken],
) -> Result<Vec<u8>, Menu06TextError> {
    if source.len() != replacement.len() {
        return Err(error(format!(
            "MENU06 rewrite token 수 불일치: source={}, replacement={}",
            source.len(),
            replacement.len()
        )));
    }
    for (index, (source_token, replacement_token)) in source.iter().zip(replacement).enumerate() {
        match (source_token, replacement_token) {
            (
                Menu06TextToken::Glyph {
                    offset: source_offset,
                    ..
                },
                Menu06TextToken::Glyph {
                    offset: replacement_offset,
                    ..
                },
            ) if source_offset == replacement_offset => {}
            (
                Menu06TextToken::Control {
                    offset: source_offset,
                    code: source_code,
                    arguments: source_arguments,
                },
                Menu06TextToken::Control {
                    offset: replacement_offset,
                    code: replacement_code,
                    arguments: replacement_arguments,
                },
            ) if source_offset == replacement_offset
                && source_code == replacement_code
                && source_arguments == replacement_arguments => {}
            _ => {
                return Err(error(format!(
                    "MENU06 rewrite가 보호 token을 변경함: index={index}"
                )));
            }
        }
    }
    serialize_menu06_text_tokens(replacement)
}

fn verify_cell_table(raw: &[u8]) -> Result<(usize, usize), Menu06TextError> {
    let header = [
        read_u16(raw, CELL_TABLE_OFFSET)?,
        read_u16(raw, CELL_TABLE_OFFSET + 2)?,
        read_u16(raw, CELL_TABLE_OFFSET + 4)?,
        read_u16(raw, CELL_TABLE_OFFSET + 6)?,
        read_u16(raw, CELL_TABLE_OFFSET + 8)?,
        read_u16(raw, CELL_TABLE_OFFSET + 10)?,
    ];
    if header != [0, 0, 512, 256, 0, 0] {
        return Err(error(format!("MENU06 cell header 불일치: {header:?}")));
    }
    let glyph_end = CELL_TABLE_OFFSET + CELL_HEADER_BYTES + GLYPH_CELL_RECORDS * CELL_RECORD_BYTES;
    if sha256(
        raw.get(CELL_TABLE_OFFSET..glyph_end)
            .ok_or_else(|| error("MENU06 glyph table 범위 이탈"))?,
    ) != GLYPH_TABLE_SHA256
    {
        return Err(error("MENU06 glyph table SHA-256 불일치"));
    }
    if sha256(
        raw.get(CELL_TABLE_OFFSET..STATIC_CELL_PREFIX_END)
            .ok_or_else(|| error("MENU06 static cell prefix 범위 이탈"))?,
    ) != STATIC_CELL_PREFIX_SHA256
    {
        return Err(error("MENU06 static cell prefix SHA-256 불일치"));
    }
    let palette_end = STATIC_CELL_PREFIX_END + PALETTE_SENTINEL_BYTES;
    if sha256(
        raw.get(STATIC_CELL_PREFIX_END..palette_end)
            .ok_or_else(|| error("MENU06 palette sentinel 범위 이탈"))?,
    ) != PALETTE_SENTINEL_SHA256
    {
        return Err(error("MENU06 cell 뒤 palette sentinel SHA-256 불일치"));
    }

    let mut valid = 0usize;
    let mut empty = 0usize;
    for index in 0..STATIC_CELL_PREFIX_RECORDS {
        let offset = CELL_TABLE_OFFSET + CELL_HEADER_BYTES + index * CELL_RECORD_BYTES;
        let x = read_u16(raw, offset)?;
        let y = read_u16(raw, offset + 2)?;
        let width = read_u16(raw, offset + 4)?;
        let height = read_u16(raw, offset + 6)?;
        let anchor_x = read_i16(raw, offset + 8)?;
        let anchor_y = read_i16(raw, offset + 10)?;
        if width == 0 || height == 0 {
            empty += 1;
        } else {
            if u32::from(x) + u32::from(width) > 512 || u32::from(y) + u32::from(height) > 256 {
                return Err(error(format!(
                    "MENU06 static cell이 atlas 밖임: index={index}, ({x},{y},{width},{height})"
                )));
            }
            valid += 1;
        }
        if anchor_x != 0 || anchor_y != 0 {
            return Err(error(format!(
                "MENU06 static cell anchor 불일치: index={index}, ({anchor_x},{anchor_y})"
            )));
        }
        if index < GLYPH_CELL_RECORDS {
            let expected = if index == 0 {
                (240, 0, 8, 8)
            } else {
                let glyph = index - 1;
                (
                    u16::try_from((glyph % 15) * 16).expect("glyph x는 u16"),
                    u16::try_from((glyph / 15) * 16).expect("glyph y는 u16"),
                    16,
                    16,
                )
            };
            if (x, y, width, height) != expected {
                return Err(error(format!(
                    "MENU06 glyph cell 격자 불일치: index={index}, expected={expected:?}, actual=({x},{y},{width},{height})"
                )));
            }
        }
    }
    Ok((valid, empty))
}

fn verify_source(
    spec: Menu06SourceSpec,
    raw: &[u8],
) -> Result<Menu06SourceEvidence, Menu06TextError> {
    if raw.len() != MENU1_BYTES || sha256(raw) != spec.sequence_sha256 {
        return Err(error(format!(
            "{}:{MENU1_SEQUENCE} 원본 계약 불일치",
            spec.archive
        )));
    }
    let footer_offset = raw.len() - 44;
    if footer_offset != MENU1_INITIALIZED_BYTES
        || read_u32(raw, footer_offset + 12)? != MENU1_LOAD_BASE
        || read_u32(raw, footer_offset + 16)? != MENU1_INITIALIZED_BYTES as u32
    {
        return Err(error(format!(
            "{}:{MENU1_SEQUENCE} overlay footer 불일치",
            spec.archive
        )));
    }
    let root_pointer_runtime = MENU1_LOAD_BASE + ROOT_OFFSET as u32;
    let child_pointer_runtime = MENU1_LOAD_BASE + CHILD_OFFSET as u32;
    let stream_pointer_runtime = MENU1_LOAD_BASE + STREAM_OFFSET as u32;
    let cell_table_runtime = MENU1_LOAD_BASE + CELL_TABLE_OFFSET as u32;
    for (field, expected) in [
        (ROOT_POINTER_FIELD_OFFSET, root_pointer_runtime),
        (CHILD_POINTER_FIELD_OFFSET, child_pointer_runtime),
        (STREAM_POINTER_FIELD_OFFSET, stream_pointer_runtime),
    ] {
        if read_u32(raw, field)? != expected {
            return Err(error(format!(
                "{}:{MENU1_SEQUENCE} pointer chain 불일치: +0x{field:04X}",
                spec.archive
            )));
        }
    }
    if raw.get(SETUP_OFFSET..SETUP_OFFSET + 4) != Some(&[0x28, 0x12, 0x97, 0x00])
        || read_u16(raw, SETUP_OFFSET + 4)? != spec.resource_id
        || read_u16(raw, SETUP_OFFSET + 6)? != 0
        || read_u32(raw, SETUP_OFFSET + 8)? != 0
        || read_u32(raw, SETUP_OFFSET + 12)? != cell_table_runtime
    {
        return Err(error(format!(
            "{}:{MENU1_SEQUENCE} MENU06 setup 불일치",
            spec.archive
        )));
    }
    if sha256(
        raw.get(STREAM_OFFSET..STREAM_END_OFFSET)
            .ok_or_else(|| error("MENU06 text stream 범위 이탈"))?,
    ) != STREAM_SHA256
    {
        return Err(error(format!(
            "{}:{MENU1_SEQUENCE} MENU06 stream SHA-256 불일치",
            spec.archive
        )));
    }
    Ok(Menu06SourceEvidence {
        archive: spec.archive,
        sequence_sha256: spec.sequence_sha256.to_string(),
        resource_id: spec.resource_id,
        root_pointer_runtime,
        child_pointer_runtime,
        stream_pointer_runtime,
        cell_table_runtime,
    })
}

/// 두 MENU1 overlay의 pointer chain·u16 stream·MENU06 cell 공급을 검증한다.
pub fn verify_box_menu06_text<'a>(
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
) -> Result<Menu06TextAnalysis, Menu06TextError> {
    let mut source_map = BTreeMap::new();
    for (archive, sequence, raw) in sources {
        if sequence != MENU1_SEQUENCE {
            continue;
        }
        if source_map.insert(archive, raw).is_some() {
            return Err(error(format!(
                "MENU06 source 중복: {archive}:{MENU1_SEQUENCE}"
            )));
        }
    }
    let canonical = source_map
        .get(SOURCE_SPECS[0].archive)
        .copied()
        .ok_or_else(|| error("PUYO1 MENU1.SEQ source 누락"))?;
    let mut analysis = parse_stream(&canonical[STREAM_OFFSET..STREAM_END_OFFSET])?;
    let serialized = serialize_menu06_text_tokens(&analysis.tokens)?;
    if serialized != canonical[STREAM_OFFSET..STREAM_END_OFFSET] {
        return Err(error("PUYO1 MENU06 typed stream 왕복 불일치"));
    }
    let mut evidence = Vec::new();
    for spec in SOURCE_SPECS {
        let raw = source_map
            .get(spec.archive)
            .copied()
            .ok_or_else(|| error(format!("{}:{MENU1_SEQUENCE} source 누락", spec.archive)))?;
        evidence.push(verify_source(spec, raw)?);
        let parsed = parse_stream(&raw[STREAM_OFFSET..STREAM_END_OFFSET])?;
        if parsed.segments != analysis.segments
            || parsed.tokens != analysis.tokens
            || parsed.glyph_tokens != analysis.glyph_tokens
            || parsed.control_tokens != analysis.control_tokens
        {
            return Err(error(format!(
                "{}:{MENU1_SEQUENCE} MENU06 stream 의미 불일치",
                spec.archive
            )));
        }
    }
    let (valid, empty) = verify_cell_table(canonical)?;
    if analysis.segments.len() != 29
        || analysis.glyph_tokens != 297
        || analysis.distinct_glyph_cells != GLYPH_CELL_RECORDS
        || analysis.control_tokens
            != BTreeMap::from([
                (0xFF00, 1),
                (0xFF02, 28),
                (0xFF04, 23),
                (0xFF0A, 16),
                (0xFF13, 1),
                (0xFF17, 1),
            ])
        || valid != 143
        || empty != 10
        || analysis.rewrite_capacity
            != (Menu06RewriteCapacity {
                stream_words: 396,
                glyph_positions: 297,
                control_words: 70,
                control_argument_words: 26,
                terminator_padding_words: 3,
                slack_words: 0,
                repertoire_slots: 98,
                used_repertoire_slots: 98,
            })
    {
        return Err(error(format!(
            "지원 원본 MENU06 분모 불일치: segments={}, glyph={}/{}, controls={:?}, cells={valid}/{empty}",
            analysis.segments.len(),
            analysis.glyph_tokens,
            analysis.distinct_glyph_cells,
            analysis.control_tokens,
        )));
    }
    analysis.sources = evidence;
    analysis.static_cell_prefix_records = STATIC_CELL_PREFIX_RECORDS;
    analysis.static_valid_cells = valid;
    analysis.static_empty_cells = empty;
    let review = render_menu06_text_review_tsv(&analysis);
    if sha256(review.as_bytes()) != MENU06_TEXT_REVIEW_SHA256 {
        return Err(error("MENU06 text review TSV SHA-256 불일치"));
    }
    Ok(analysis)
}

/// `FF02` 경계별 보호 원문과 실제 cell ID를 사람이 대조할 review TSV.
pub fn render_menu06_text_review_tsv(analysis: &Menu06TextAnalysis) -> String {
    let archives = analysis
        .sources
        .iter()
        .map(|source| source.archive)
        .collect::<Vec<_>>()
        .join("|");
    let mut output = String::from(
        "schema_version\tsegment_id\tarchives\tsequence\tstart_offset\tend_offset\t\
         glyph_tokens\tglyph_cells\tprotected_source\ttranslation\treview_status\tclaim_boundary\n",
    );
    for segment in &analysis.segments {
        let cells = segment
            .glyph_cells
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(",");
        output.push_str(&format!(
            "{MENU06_TEXT_REVIEW_SCHEMA_VERSION}\tMENU06-TXT-{:04}\t{archives}\t{MENU1_SEQUENCE}\t\
             0x{:04X}\t0x{:04X}\t{}\t{}\t{}\t\tunmanaged\t\
             static-pointer-chain-code-to-cell-runtime-execution-draw-screen-unverified\n",
            segment.index + 1,
            segment.start_offset,
            segment.end_offset,
            segment.glyph_cells.len(),
            cells,
            segment.source,
        ));
    }
    output
}

#[cfg(test)]
#[path = "menu06_text_tests.rs"]
mod tests;
