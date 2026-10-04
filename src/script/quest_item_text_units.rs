//! `ITEM_DAT.SEQ`의 item 이름과 직접 text-mode 설명 단위.
//!
//! 이 자산은 OP21 record를 거치지 않는다. `QUEST.EXE`의 item detail
//! constructor가 interpreter active word를 1로 만들고 record `+0x10`의
//! stream pointer를 직접 설치한다. 별도 이름 소비자는 record `+0x0C`의
//! NUL 종단 glyph 열을 읽는다. 이 모듈은 160개 table entry를 111개 고유
//! record로 접고 두 pointer field가 소유하는 원본 token을 보존한다.
//!
//! 정적 소비 경로는 실행·표시 전수를 뜻하지 않는다. 번역 승인과 실제 창
//! 조판은 별도 HITL/runtime gate가 소유한다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::font::{decode_quest_source_glyph, QuestCodeTable};

use super::quest::{parse_quest_token, quest_control_meaning, QuestControlMeaning, QuestToken};
use super::quest_seq_text_streams::QuestTextTerminal;
use super::quest_text_candidates::scan_quest_code_table_text_runs;

pub const QUEST_ITEM_TEXT_SEQUENCE: &str = "ITEM_DAT.SEQ";
pub const QUEST_ITEM_TEXT_SEQUENCE_BYTES: usize = 16_776;
pub const QUEST_ITEM_TEXT_SEQUENCE_SHA256: &str =
    "766c0d52364dcc4fcff530e53fc7199e7e31cf2db0a55c724cea5a3fd3d4d40a";
pub const QUEST_ITEM_TEXT_RUNTIME_BASE: u32 = 0x801A_1A70;
pub const BOX_QUEST_ITEM_TABLE_ENTRIES: usize = 160;
pub const BOX_QUEST_ITEM_RECORDS: usize = 111;
pub const BOX_QUEST_ITEM_NAME_UNITS: usize = 111;
pub const BOX_QUEST_ITEM_DESCRIPTION_UNITS: usize = 111;
pub const BOX_QUEST_ITEM_TEXT_UNITS: usize = 222;
pub const BOX_QUEST_ITEM_TEXT_CANDIDATE_RUNS: usize = 416;
pub const BOX_QUEST_ITEM_NAME_CANDIDATE_RUNS: usize = 110;
pub const BOX_QUEST_ITEM_DESCRIPTION_CANDIDATE_RUNS: usize = 306;
pub const BOX_QUEST_ITEM_SHORT_NAME_UNITS: usize = 1;

const ITEM_RECORD_NAME_POINTER_OFFSET: usize = 0x0C;
const ITEM_RECORD_DESCRIPTION_POINTER_OFFSET: usize = 0x10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuestItemTextRole {
    Name,
    Description,
}

impl QuestItemTextRole {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Description => "description",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestItemTextTokenKind {
    Glyph,
    Control,
    NullTerminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestItemTextTerminal {
    Null,
    Text(QuestTextTerminal),
}

impl QuestItemTextTerminal {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Null => "nul",
            Self::Text(QuestTextTerminal::ReleaseInterpreter) => "ff00-release",
            Self::Text(QuestTextTerminal::DeactivateAndAlign) => "ff03-deactivate-align",
            Self::Text(QuestTextTerminal::NotifyParentAndRelease) => "ff1a-notify-release",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemTextToken {
    pub index: usize,
    pub sequence_offset: usize,
    pub runtime: u32,
    pub raw: Vec<u8>,
    pub kind: QuestItemTextTokenKind,
    pub source: Option<char>,
    pub control_code: Option<u8>,
    pub control_meaning: Option<QuestControlMeaning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemTextUnit {
    pub id: String,
    pub role: QuestItemTextRole,
    pub table_indices: Vec<usize>,
    pub record_offset: usize,
    pub record_runtime: u32,
    pub pointer_field_offset: usize,
    pub pointer_field_runtime: u32,
    pub text_offset: usize,
    pub text_runtime: u32,
    pub end_offset: usize,
    pub end_runtime: u32,
    pub protected_raw: Vec<u8>,
    pub protected_source: String,
    pub tokens: Vec<QuestItemTextToken>,
    pub terminal: QuestItemTextTerminal,
    pub glyph_occurrences: usize,
    pub japanese_glyph_occurrences: usize,
    pub control_occurrences: usize,
    /// 정적 source 결속을 실행 관측으로 오인하지 않게 항상 `None`이다.
    pub runtime_evidence: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemTextUnitAnalysis {
    pub units: Vec<QuestItemTextUnit>,
    pub table_entries: usize,
    pub unique_records: usize,
    pub name_units: usize,
    pub description_units: usize,
    pub japanese_units: usize,
    pub glyph_occurrences: usize,
    pub japanese_glyph_occurrences: usize,
    pub control_occurrences: usize,
    pub code_table_candidate_runs: usize,
    pub name_candidate_runs: usize,
    pub description_candidate_runs: usize,
    pub unowned_candidate_runs: usize,
    pub short_name_units: usize,
    pub runtime_observed_units: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestItemTextUnitError(String);

impl fmt::Display for QuestItemTextUnitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestItemTextUnitError {}

fn error(message: impl Into<String>) -> QuestItemTextUnitError {
    QuestItemTextUnitError(message.into())
}

fn is_japanese(character: char) -> bool {
    matches!(
        character,
        '\u{3040}'..='\u{30ff}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{ff66}'..='\u{ff9f}'
    )
}

fn runtime(base: u32, offset: usize, role: &str) -> Result<u32, QuestItemTextUnitError> {
    base.checked_add(
        u32::try_from(offset).map_err(|_| error(format!("QUEST item {role} offset overflow")))?,
    )
    .ok_or_else(|| error(format!("QUEST item {role} runtime overflow")))
}

fn offset_from_pointer(
    raw: &[u8],
    base: u32,
    pointer: u32,
    role: &str,
) -> Result<usize, QuestItemTextUnitError> {
    let offset = pointer
        .checked_sub(base)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            error(format!(
                "QUEST item {role} pointer가 runtime base 앞임: 0x{pointer:08X}"
            ))
        })?;
    if offset >= raw.len() {
        return Err(error(format!(
            "QUEST item {role} pointer가 자산 밖임: 0x{pointer:08X} / +0x{offset:X}"
        )));
    }
    Ok(offset)
}

fn read_pointer(raw: &[u8], offset: usize, role: &str) -> Result<u32, QuestItemTextUnitError> {
    let bytes = raw.get(offset..offset + 4).ok_or_else(|| {
        error(format!(
            "QUEST item {role} pointer field가 잘림: +0x{offset:X}"
        ))
    })?;
    Ok(u32::from_le_bytes(
        bytes.try_into().expect("4바이트 pointer 범위를 검사함"),
    ))
}

fn item_id(record_offset: usize, role: QuestItemTextRole) -> String {
    format!(
        "QUEST-ITEM-TEXT:{QUEST_ITEM_TEXT_SEQUENCE}:record-0x{record_offset:04X}:{}",
        role.label()
    )
}

fn parse_name_unit(
    raw: &[u8],
    base: u32,
    record_offset: usize,
    table_indices: Vec<usize>,
    code_table: &QuestCodeTable,
) -> Result<QuestItemTextUnit, QuestItemTextUnitError> {
    let pointer_field_offset = record_offset + ITEM_RECORD_NAME_POINTER_OFFSET;
    let pointer = read_pointer(raw, pointer_field_offset, "name")?;
    let text_offset = offset_from_pointer(raw, base, pointer, "name")?;
    if !text_offset.is_multiple_of(2) {
        return Err(error(format!(
            "QUEST item name pointer가 u16 정렬이 아님: +0x{text_offset:X}"
        )));
    }

    let mut cursor = text_offset;
    let mut tokens = Vec::new();
    let mut source = String::new();
    let mut japanese_glyph_occurrences = 0usize;
    loop {
        let pair: [u8; 2] = raw
            .get(cursor..cursor + 2)
            .ok_or_else(|| error("QUEST item name NUL 종단이 자산 안에 없음"))?
            .try_into()
            .expect("2바이트 name token 범위를 검사함");
        let token_runtime = runtime(base, cursor, "name token")?;
        if pair == [0, 0] {
            tokens.push(QuestItemTextToken {
                index: tokens.len(),
                sequence_offset: cursor,
                runtime: token_runtime,
                raw: pair.to_vec(),
                kind: QuestItemTextTokenKind::NullTerminal,
                source: None,
                control_code: None,
                control_meaning: None,
            });
            cursor += 2;
            break;
        }
        if code_table.index_of(pair).is_none() {
            return Err(error(format!(
                "QUEST item name glyph가 코드표에 없음: +0x{cursor:X} {:02X}{:02X}",
                pair[0], pair[1]
            )));
        }
        let character = decode_quest_source_glyph(pair).ok_or_else(|| {
            error(format!(
                "QUEST item name glyph를 한 문자로 decode할 수 없음: +0x{cursor:X}"
            ))
        })?;
        source.push(character);
        japanese_glyph_occurrences += usize::from(is_japanese(character));
        tokens.push(QuestItemTextToken {
            index: tokens.len(),
            sequence_offset: cursor,
            runtime: token_runtime,
            raw: pair.to_vec(),
            kind: QuestItemTextTokenKind::Glyph,
            source: Some(character),
            control_code: None,
            control_meaning: None,
        });
        cursor += 2;
    }
    let protected_raw = raw[text_offset..cursor].to_vec();
    Ok(QuestItemTextUnit {
        id: item_id(record_offset, QuestItemTextRole::Name),
        role: QuestItemTextRole::Name,
        table_indices,
        record_offset,
        record_runtime: runtime(base, record_offset, "record")?,
        pointer_field_offset,
        pointer_field_runtime: runtime(base, pointer_field_offset, "name pointer field")?,
        text_offset,
        text_runtime: pointer,
        end_offset: cursor,
        end_runtime: runtime(base, cursor, "name end")?,
        protected_raw,
        protected_source: source,
        glyph_occurrences: tokens.len() - 1,
        japanese_glyph_occurrences,
        control_occurrences: 0,
        tokens,
        terminal: QuestItemTextTerminal::Null,
        runtime_evidence: None,
    })
}

fn parse_description_unit(
    raw: &[u8],
    base: u32,
    record_offset: usize,
    table_indices: Vec<usize>,
    code_table: &QuestCodeTable,
) -> Result<QuestItemTextUnit, QuestItemTextUnitError> {
    let pointer_field_offset = record_offset + ITEM_RECORD_DESCRIPTION_POINTER_OFFSET;
    let pointer = read_pointer(raw, pointer_field_offset, "description")?;
    let text_offset = offset_from_pointer(raw, base, pointer, "description")?;
    let mut cursor = text_offset;
    let mut tokens = Vec::new();
    let mut source = String::new();
    let mut glyph_occurrences = 0usize;
    let mut japanese_glyph_occurrences = 0usize;
    let mut control_occurrences = 0usize;
    let terminal = loop {
        let token_runtime = runtime(base, cursor, "description token")?;
        let parsed = parse_quest_token(
            raw.get(cursor..)
                .ok_or_else(|| error("QUEST item description token 입력이 잘림"))?,
            token_runtime,
        )
        .map_err(|cause| {
            error(format!(
                "QUEST item description token 파싱 실패: +0x{cursor:X}: {cause}"
            ))
        })?;
        let token_raw = parsed.raw().to_vec();
        let token_len = token_raw.len();
        match parsed {
            QuestToken::Glyph(pair) => {
                let code: [u8; 2] = pair.try_into().expect("glyph는 2바이트");
                if code_table.index_of(code).is_none() {
                    return Err(error(format!(
                        "QUEST item description glyph가 코드표에 없음: +0x{cursor:X} {:02X}{:02X}",
                        code[0], code[1]
                    )));
                }
                let character = decode_quest_source_glyph(code).ok_or_else(|| {
                    error(format!(
                        "QUEST item description glyph를 한 문자로 decode할 수 없음: +0x{cursor:X}"
                    ))
                })?;
                source.push(character);
                glyph_occurrences += 1;
                japanese_glyph_occurrences += usize::from(is_japanese(character));
                tokens.push(QuestItemTextToken {
                    index: tokens.len(),
                    sequence_offset: cursor,
                    runtime: token_runtime,
                    raw: token_raw,
                    kind: QuestItemTextTokenKind::Glyph,
                    source: Some(character),
                    control_code: None,
                    control_meaning: None,
                });
            }
            QuestToken::Control { spec, .. } => {
                let meaning = quest_control_meaning(spec.code).ok_or_else(|| {
                    error(format!("QUEST item control 의미 누락: FF{:02X}", spec.code))
                })?;
                control_occurrences += 1;
                tokens.push(QuestItemTextToken {
                    index: tokens.len(),
                    sequence_offset: cursor,
                    runtime: token_runtime,
                    raw: token_raw,
                    kind: QuestItemTextTokenKind::Control,
                    source: None,
                    control_code: Some(spec.code),
                    control_meaning: Some(meaning),
                });
                if let Some(terminal) = QuestTextTerminal::from_control(spec.code) {
                    cursor = cursor
                        .checked_add(token_len)
                        .ok_or_else(|| error("QUEST item description terminal cursor overflow"))?;
                    break terminal;
                }
            }
        }
        cursor = cursor
            .checked_add(token_len)
            .ok_or_else(|| error("QUEST item description token cursor overflow"))?;
    };
    Ok(QuestItemTextUnit {
        id: item_id(record_offset, QuestItemTextRole::Description),
        role: QuestItemTextRole::Description,
        table_indices,
        record_offset,
        record_runtime: runtime(base, record_offset, "record")?,
        pointer_field_offset,
        pointer_field_runtime: runtime(base, pointer_field_offset, "description pointer field")?,
        text_offset,
        text_runtime: pointer,
        end_offset: cursor,
        end_runtime: runtime(base, cursor, "description end")?,
        protected_raw: raw[text_offset..cursor].to_vec(),
        protected_source: source,
        tokens,
        terminal: QuestItemTextTerminal::Text(terminal),
        glyph_occurrences,
        japanese_glyph_occurrences,
        control_occurrences,
        runtime_evidence: None,
    })
}

/// 절대 pointer table과 두 text field를 token 보존형 단위로 추출한다.
pub fn audit_quest_item_text_units(
    raw: &[u8],
    runtime_base: u32,
    table_entries: usize,
    code_table: &QuestCodeTable,
) -> Result<QuestItemTextUnitAnalysis, QuestItemTextUnitError> {
    if raw.is_empty() || table_entries == 0 {
        return Err(error("QUEST item text 입력이나 table이 비어 있음"));
    }
    let table_bytes = table_entries
        .checked_mul(4)
        .ok_or_else(|| error("QUEST item table bytes overflow"))?;
    let table = raw
        .get(..table_bytes)
        .ok_or_else(|| error("QUEST item pointer table이 잘림"))?;
    let mut records = BTreeMap::<usize, Vec<usize>>::new();
    for (index, word) in table.chunks_exact(4).enumerate() {
        let pointer = u32::from_le_bytes(word.try_into().expect("table word는 4바이트"));
        let offset = offset_from_pointer(raw, runtime_base, pointer, "record")?;
        if !offset.is_multiple_of(4) || offset < table_bytes {
            return Err(error(format!(
                "QUEST item record pointer 경계 불일치: index={index}, +0x{offset:X}, table=0x{table_bytes:X}"
            )));
        }
        records.entry(offset).or_default().push(index);
    }

    let mut units = Vec::with_capacity(records.len() * 2);
    for (record_offset, indices) in &records {
        if raw.get(*record_offset..record_offset + 20).is_none() {
            return Err(error(format!(
                "QUEST item record header가 잘림: +0x{record_offset:X}"
            )));
        }
        units.push(parse_name_unit(
            raw,
            runtime_base,
            *record_offset,
            indices.clone(),
            code_table,
        )?);
        units.push(parse_description_unit(
            raw,
            runtime_base,
            *record_offset,
            indices.clone(),
            code_table,
        )?);
    }

    let candidates = scan_quest_code_table_text_runs(raw, code_table);
    let mut name_candidate_runs = 0usize;
    let mut description_candidate_runs = 0usize;
    let mut unowned_candidate_runs = 0usize;
    for candidate in &candidates {
        let owners = units
            .iter()
            .filter(|unit| {
                let content_end = match unit.terminal {
                    QuestItemTextTerminal::Null => unit.end_offset - 2,
                    QuestItemTextTerminal::Text(_) => unit.end_offset,
                };
                unit.text_offset <= candidate.offset
                    && candidate.offset + candidate.byte_len <= content_end
            })
            .collect::<Vec<_>>();
        match owners.as_slice() {
            [] => unowned_candidate_runs += 1,
            [owner] => match owner.role {
                QuestItemTextRole::Name => name_candidate_runs += 1,
                QuestItemTextRole::Description => description_candidate_runs += 1,
            },
            _ => {
                return Err(error(format!(
                    "QUEST item code-table run owner가 겹침: +0x{:X}",
                    candidate.offset
                )));
            }
        }
    }

    let name_units = units
        .iter()
        .filter(|unit| unit.role == QuestItemTextRole::Name)
        .count();
    let description_units = units.len() - name_units;
    let japanese_units = units
        .iter()
        .filter(|unit| unit.japanese_glyph_occurrences > 0)
        .count();
    let short_name_units = units
        .iter()
        .filter(|unit| {
            unit.role == QuestItemTextRole::Name
                && unit.glyph_occurrences < 3
                && unit.japanese_glyph_occurrences > 0
        })
        .count();
    Ok(QuestItemTextUnitAnalysis {
        glyph_occurrences: units.iter().map(|unit| unit.glyph_occurrences).sum(),
        japanese_glyph_occurrences: units
            .iter()
            .map(|unit| unit.japanese_glyph_occurrences)
            .sum(),
        control_occurrences: units.iter().map(|unit| unit.control_occurrences).sum(),
        runtime_observed_units: units
            .iter()
            .filter(|unit| unit.runtime_evidence.is_some())
            .count(),
        units,
        table_entries,
        unique_records: records.len(),
        name_units,
        description_units,
        japanese_units,
        code_table_candidate_runs: candidates.len(),
        name_candidate_runs,
        description_candidate_runs,
        unowned_candidate_runs,
        short_name_units,
    })
}

/// 지원 원본 `ITEM_DAT.SEQ`의 구조와 기존 heuristic 후보 귀속을 고정한다.
pub fn verify_box_quest_item_text_units(
    raw: &[u8],
    code_table: &QuestCodeTable,
) -> Result<QuestItemTextUnitAnalysis, QuestItemTextUnitError> {
    if raw.len() != QUEST_ITEM_TEXT_SEQUENCE_BYTES {
        return Err(error(format!(
            "QUEST ITEM_DAT 길이 불일치: expected={}, actual={}",
            QUEST_ITEM_TEXT_SEQUENCE_BYTES,
            raw.len()
        )));
    }
    let digest = format!("{:x}", Sha256::digest(raw));
    if digest != QUEST_ITEM_TEXT_SEQUENCE_SHA256 {
        return Err(error(format!(
            "QUEST ITEM_DAT SHA-256 불일치: expected={QUEST_ITEM_TEXT_SEQUENCE_SHA256}, actual={digest}"
        )));
    }
    let analysis = audit_quest_item_text_units(
        raw,
        QUEST_ITEM_TEXT_RUNTIME_BASE,
        BOX_QUEST_ITEM_TABLE_ENTRIES,
        code_table,
    )?;
    let actual = [
        analysis.table_entries,
        analysis.unique_records,
        analysis.units.len(),
        analysis.name_units,
        analysis.description_units,
        analysis.japanese_units,
        analysis.code_table_candidate_runs,
        analysis.name_candidate_runs,
        analysis.description_candidate_runs,
        analysis.unowned_candidate_runs,
        analysis.short_name_units,
        analysis.runtime_observed_units,
    ];
    let expected = [
        BOX_QUEST_ITEM_TABLE_ENTRIES,
        BOX_QUEST_ITEM_RECORDS,
        BOX_QUEST_ITEM_TEXT_UNITS,
        BOX_QUEST_ITEM_NAME_UNITS,
        BOX_QUEST_ITEM_DESCRIPTION_UNITS,
        BOX_QUEST_ITEM_TEXT_UNITS,
        BOX_QUEST_ITEM_TEXT_CANDIDATE_RUNS,
        BOX_QUEST_ITEM_NAME_CANDIDATE_RUNS,
        BOX_QUEST_ITEM_DESCRIPTION_CANDIDATE_RUNS,
        0,
        BOX_QUEST_ITEM_SHORT_NAME_UNITS,
        0,
    ];
    if actual != expected {
        return Err(error(format!(
            "QUEST ITEM_DAT text 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }
    if analysis.units.iter().any(|unit| {
        matches!(
            unit.terminal,
            QuestItemTextTerminal::Text(terminal)
                if terminal != QuestTextTerminal::NotifyParentAndRelease
        )
    }) {
        return Err(error("QUEST ITEM_DAT description terminal이 FF1A가 아님"));
    }
    let short_names = analysis
        .units
        .iter()
        .filter(|unit| {
            unit.role == QuestItemTextRole::Name
                && unit.glyph_occurrences < 3
                && unit.japanese_glyph_occurrences > 0
        })
        .map(|unit| unit.protected_source.as_str())
        .collect::<BTreeSet<_>>();
    if short_names != BTreeSet::from(["カギ"]) {
        return Err(error(format!(
            "QUEST ITEM_DAT heuristic 누락 짧은 이름 불일치: {short_names:?}"
        )));
    }
    Ok(analysis)
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02X}").expect("String write는 실패하지 않음");
    }
    output
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

fn annotated_source(unit: &QuestItemTextUnit) -> String {
    let mut output = String::new();
    for token in &unit.tokens {
        match token.kind {
            QuestItemTextTokenKind::Glyph => {
                output.push(token.source.expect("glyph token은 source를 가짐"));
            }
            QuestItemTextTokenKind::Control => {
                write!(
                    output,
                    "{{FF{:02X}:{}}}",
                    token.control_code.expect("control token은 code를 가짐"),
                    hex_bytes(&token.raw)
                )
                .expect("String write는 실패하지 않음");
            }
            QuestItemTextTokenKind::NullTerminal => output.push_str("{NUL}"),
        }
    }
    output
}

fn control_signature(unit: &QuestItemTextUnit) -> String {
    unit.tokens
        .iter()
        .filter(|token| token.kind == QuestItemTextTokenKind::Control)
        .map(|token| {
            format!(
                "FF{:02X}:{}",
                token.control_code.expect("control token은 code를 가짐"),
                hex_bytes(&token.raw)
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// 이름과 설명의 source·control·좌표를 한 행씩 보존한 읽기 전용 TSV.
pub fn render_quest_item_text_units_tsv(analysis: &QuestItemTextUnitAnalysis) -> String {
    let mut output = String::from(
        "archive_path\tsequence_name\tsequence_sha256\tunit_id\trole\ttable_indices\t\
         record_offset\trecord_runtime\tpointer_field_offset\tpointer_field_runtime\t\
         text_offset\ttext_runtime\tend_offset\tend_runtime\tterminal\t\
         glyph_occurrences\tjapanese_glyph_occurrences\tcontrol_occurrences\t\
         protected_source\tannotated_source\tcontrol_signature\tprotected_raw\t\
         runtime_evidence\n",
    );
    for unit in &analysis.units {
        let table_indices = unit
            .table_indices
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            output,
            "/QUEST/RPG_SEQ.VDD;1\t{}\t{}\t{}\t{}\t{}\t0x{:04X}\t0x{:08X}\t\
             0x{:02X}\t0x{:08X}\t0x{:04X}\t0x{:08X}\t0x{:04X}\t0x{:08X}\t{}\t\
             {}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            QUEST_ITEM_TEXT_SEQUENCE,
            QUEST_ITEM_TEXT_SEQUENCE_SHA256,
            unit.id,
            unit.role.label(),
            table_indices,
            unit.record_offset,
            unit.record_runtime,
            unit.pointer_field_offset,
            unit.pointer_field_runtime,
            unit.text_offset,
            unit.text_runtime,
            unit.end_offset,
            unit.end_runtime,
            unit.terminal.label(),
            unit.glyph_occurrences,
            unit.japanese_glyph_occurrences,
            unit.control_occurrences,
            escape_tsv(&unit.protected_source),
            escape_tsv(&annotated_source(unit)),
            control_signature(unit),
            hex_bytes(&unit.protected_raw),
            unit.runtime_evidence.unwrap_or("static-only"),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_item_text_units_tests.rs"]
mod tests;
