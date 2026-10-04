//! `RPG_SEQ.VDD` 전체의 OP21 기반 text stream 정적 후보.
//!
//! base opcode `0x21`은 interpreter active word를 1로 만들고 현재 stream을
//! 4바이트 전진시킨다. 이후 `FF03`은 active word를 0으로 되돌리고,
//! `FF00`/`FF1A`는 interpreter를 해제한다. 이 모듈은 4바이트 정렬
//! `0x21` shape에서 시작해 위 lifecycle 종단까지 확인된 layout만으로
//! token을 읽고, 모든 glyph가 `QUEST.EXE` 코드표에 있는 경우만 stream
//! 후보로 보존한다.
//!
//! 정적 shape 자체는 실행 도달 증거가 아니다. 따라서 결과는 번역 모집단이
//! 아니라 기존 코드표 run 후보를 좁히는 source-shape 하한이다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::font::QuestCodeTable;
use crate::text::decode_sjis;

use super::quest::{parse_quest_token, QuestToken};
use super::quest_seq_inventory::{BOX_QUEST_SEQ_BYTES, BOX_QUEST_SEQ_SEQUENCES};
use super::quest_text_candidates::{
    scan_quest_code_table_text_runs, TOWN00_TEXT_CANDIDATE_MIN_CODE_UNITS,
};

pub const QUEST_SEQ_TEXT_MODE_OPCODE: u8 = 0x21;
pub const QUEST_SEQ_TEXT_MODE_RECORD_BYTES: usize = 4;
pub const QUEST_SEQ_TEXT_STREAM_SCHEMA_VERSION: &str = "quest-seq-text-streams-v1";
pub const QUEST_SEQ_ROOTED_TEXT_RUN_SCHEMA_VERSION: &str = "quest-seq-rooted-text-runs-v1";
pub const BOX_QUEST_SEQ_OP21_SHAPES: usize = 752;
pub const BOX_QUEST_SEQ_PARSEABLE_TEXT_STREAMS: usize = 605;
pub const BOX_QUEST_SEQ_REJECTED_OP21_SHAPES: usize = 147;
pub const BOX_QUEST_SEQ_NONCANONICAL_OP21_SHAPES: usize = 145;
pub const BOX_QUEST_SEQ_CANONICAL_INVALID_TEXT_STREAMS: usize = 2;
pub const BOX_QUEST_SEQ_TEXT_STREAM_FILES: usize = 16;
pub const BOX_QUEST_SEQ_ROOTED_TEXT_FILES: usize = 12;
pub const BOX_QUEST_SEQ_ROOTED_TEXT_RUNS: usize = 3_304;
pub const BOX_QUEST_SEQ_UNROOTED_CODE_TABLE_RUNS: usize = 416;
pub const BOX_QUEST_SEQ_ROOTED_TEXT_CODE_UNITS: usize = 31_140;
pub const BOX_QUEST_SEQ_ROOTED_TEXT_UNIQUE_CHARACTERS: usize = 273;
pub const BOX_QUEST_SEQ_TEXT_STREAM_REPORT_SHA256: &str =
    "ba8c35d413ed3d4d3952f3ecdd50028efb86c151e22b8d8b6df1e08449db1bbe";
pub const BOX_QUEST_SEQ_ROOTED_TEXT_REPORT_SHA256: &str =
    "afa28fadecbe9e48c9e33cda7417f6e23da778b8faa413e6bfc84324dd8a2c6c";

/// OP21 shape가 있는 파일의 고정 지원 원본 분모.
///
/// `(이름, shape, parseable, rejected, rooted run, rooted code unit)` 순서다.
pub const BOX_QUEST_SEQ_TEXT_STREAM_FILE_EXPECTATIONS: [(&str, usize, usize, usize, usize, usize);
    BOX_QUEST_SEQ_TEXT_STREAM_FILES] = [
    ("ALLMAP.SEQ", 5, 0, 5, 0, 0),
    ("DRAK.CEL", 5, 0, 5, 0, 0),
    ("HASI.SEQ", 25, 18, 7, 109, 875),
    ("ISEKI.SEQ", 43, 37, 6, 215, 1_879),
    ("ITEM_DAT.SEQ", 4, 0, 4, 0, 0),
    ("KAWA.SEQ", 75, 66, 9, 638, 5_643),
    ("MCRD1.SEQ", 5, 0, 5, 0, 0),
    ("MITI.SEQ", 16, 11, 5, 117, 1_053),
    ("MORI.SEQ", 65, 43, 22, 188, 1_536),
    ("SHEZ.SEQ", 39, 29, 10, 155, 1_619),
    ("SUKE.SEQ", 25, 16, 9, 80, 727),
    ("TOU.SEQ", 56, 51, 5, 342, 3_112),
    ("TOWN00.SEQ", 125, 110, 15, 465, 4_848),
    ("TOWN10.SEQ", 129, 109, 20, 568, 5_746),
    ("TOWN20.SEQ", 95, 81, 14, 288, 2_928),
    ("TOWN30.SEQ", 40, 34, 6, 139, 1_174),
];

/// text mode를 끝내는 확인된 control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuestTextTerminal {
    /// `FF00`: interpreter 해제와 dispatch yield.
    ReleaseInterpreter,
    /// `FF03`: interpreter 비활성화와 stream 4바이트 정렬.
    DeactivateAndAlign,
    /// `FF1A`: parent 통지 뒤 interpreter 해제와 dispatch yield.
    NotifyParentAndRelease,
}

impl QuestTextTerminal {
    pub const fn from_control(code: u8) -> Option<Self> {
        match code {
            0x00 => Some(Self::ReleaseInterpreter),
            0x03 => Some(Self::DeactivateAndAlign),
            0x1A => Some(Self::NotifyParentAndRelease),
            _ => None,
        }
    }

    pub const fn code(self) -> u8 {
        match self {
            Self::ReleaseInterpreter => 0x00,
            Self::DeactivateAndAlign => 0x03,
            Self::NotifyParentAndRelease => 0x1A,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::ReleaseInterpreter => "ff00-release",
            Self::DeactivateAndAlign => "ff03-deactivate-align",
            Self::NotifyParentAndRelease => "ff1a-notify-release",
        }
    }
}

/// 4바이트 정렬 OP21 shape 하나의 파싱 판정.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextStreamMarker {
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub record_offset: usize,
    pub record_raw: [u8; QUEST_SEQ_TEXT_MODE_RECORD_BYTES],
    pub stream_offset: usize,
    pub end_offset: Option<usize>,
    pub terminal: Option<QuestTextTerminal>,
    pub token_count: usize,
    pub glyph_count: usize,
    pub control_count: usize,
    pub rooted_run_count: usize,
    pub error: Option<String>,
}

impl QuestSeqTextStreamMarker {
    pub const fn is_parseable(&self) -> bool {
        self.error.is_none()
    }
}

/// parseable OP21 stream 안에서 token 경계와 일치한 연속 일본어 glyph run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqRootedTextRun {
    pub sequence: String,
    pub sequence_sha256: [u8; 32],
    pub offset: usize,
    pub raw: Vec<u8>,
    pub source: String,
    pub japanese_code_units: usize,
    pub owner_record_offsets: Vec<usize>,
}

impl QuestSeqRootedTextRun {
    pub fn id(&self) -> String {
        format!(
            "QUEST-SEQ-ROOTED-TEXT:{}:0x{:04X}",
            self.sequence, self.offset
        )
    }

    pub const fn byte_len(&self) -> usize {
        self.raw.len()
    }

    pub const fn code_units(&self) -> usize {
        self.raw.len() / 2
    }
}

/// 후보가 없는 SEQ도 보존하는 파일별 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextStreamSummary {
    pub sequence: String,
    pub bytes: usize,
    pub sha256: [u8; 32],
    pub op21_shapes: usize,
    pub parseable_streams: usize,
    pub rejected_shapes: usize,
    pub rooted_runs: usize,
    pub rooted_code_units: usize,
}

/// 113개 SEQ의 OP21 stream 후보 분석.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextStreamAnalysis {
    pub summaries: Vec<QuestSeqTextStreamSummary>,
    pub markers: Vec<QuestSeqTextStreamMarker>,
    pub rooted_runs: Vec<QuestSeqRootedTextRun>,
    pub code_table_run_candidates: usize,
    pub code_table_candidates_with_rooted_run: usize,
    pub exact_code_table_candidate_matches: usize,
    pub rooted_code_units: usize,
    pub unique_source_characters: usize,
}

/// 입력·token·코드표·결정적 분모 불일치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqTextStreamError(String);

impl fmt::Display for QuestSeqTextStreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for QuestSeqTextStreamError {}

fn error(message: impl Into<String>) -> QuestSeqTextStreamError {
    QuestSeqTextStreamError(message.into())
}

fn is_japanese_character(character: char) -> bool {
    matches!(
        character,
        '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'
    )
}

#[derive(Debug)]
pub(crate) struct ParsedQuestTextStream {
    pub(crate) end_offset: usize,
    pub(crate) terminal: QuestTextTerminal,
    pub(crate) token_count: usize,
    pub(crate) glyph_count: usize,
    pub(crate) control_count: usize,
    pub(crate) runs: Vec<(usize, Vec<u8>, String, usize)>,
}

fn finish_run(
    runs: &mut Vec<(usize, Vec<u8>, String, usize)>,
    offset: &mut Option<usize>,
    raw: &mut Vec<u8>,
    source: &mut String,
    japanese_code_units: &mut usize,
) {
    if raw.len() / 2 >= TOWN00_TEXT_CANDIDATE_MIN_CODE_UNITS && *japanese_code_units > 0 {
        runs.push((
            offset.expect("비어 있지 않은 glyph run에는 시작 offset이 있음"),
            std::mem::take(raw),
            std::mem::take(source),
            *japanese_code_units,
        ));
    } else {
        raw.clear();
        source.clear();
    }
    *offset = None;
    *japanese_code_units = 0;
}

pub(crate) fn parse_quest_text_stream(
    sequence: &str,
    raw: &[u8],
    stream_offset: usize,
    code_table: &QuestCodeTable,
) -> Result<ParsedQuestTextStream, String> {
    let mut cursor = stream_offset;
    let mut token_count = 0usize;
    let mut glyph_count = 0usize;
    let mut control_count = 0usize;
    let mut runs = Vec::new();
    let mut run_offset = None;
    let mut run_raw = Vec::new();
    let mut run_source = String::new();
    let mut run_japanese_code_units = 0usize;

    while cursor < raw.len() {
        let address = u32::try_from(cursor)
            .map_err(|_| format!("{sequence}: text stream offset이 u32 범위를 넘음"))?;
        let token = parse_quest_token(&raw[cursor..], address)
            .map_err(|source| format!("token 0x{cursor:04X}: {source}"))?;
        let token_len = token.raw().len();
        token_count += 1;

        match token {
            QuestToken::Glyph(pair) => {
                let code = [pair[0], pair[1]];
                if code_table.index_of(code).is_none() {
                    return Err(format!(
                        "glyph 0x{cursor:04X}가 QUEST 코드표에 없음: {:02X}{:02X}",
                        code[0], code[1]
                    ));
                }
                let decoded_character = decode_sjis(&code).ok().and_then(|decoded| {
                    let mut characters = decoded.chars();
                    let character = characters.next()?;
                    characters.next().is_none().then_some(character)
                });
                if let Some(character) = decoded_character {
                    run_offset.get_or_insert(cursor);
                    run_raw.extend_from_slice(pair);
                    run_source.push(character);
                    run_japanese_code_units += usize::from(is_japanese_character(character));
                } else {
                    // renderer는 Unicode가 아니라 코드표 index를 소비한다. 따라서
                    // 단일 Shift-JIS 문자로 표현할 수 없는 등록 glyph도 stream
                    // 실패로 만들지 않고 review 가능한 문자 run만 여기서 끊는다.
                    finish_run(
                        &mut runs,
                        &mut run_offset,
                        &mut run_raw,
                        &mut run_source,
                        &mut run_japanese_code_units,
                    );
                }
                glyph_count += 1;
            }
            QuestToken::Control { spec, .. } => {
                finish_run(
                    &mut runs,
                    &mut run_offset,
                    &mut run_raw,
                    &mut run_source,
                    &mut run_japanese_code_units,
                );
                control_count += 1;
                if let Some(terminal) = QuestTextTerminal::from_control(spec.code) {
                    let end_offset = cursor
                        .checked_add(token_len)
                        .ok_or_else(|| format!("{sequence}: text stream end overflow"))?;
                    return Ok(ParsedQuestTextStream {
                        end_offset,
                        terminal,
                        token_count,
                        glyph_count,
                        control_count,
                        runs,
                    });
                }
            }
        }
        cursor = cursor
            .checked_add(token_len)
            .ok_or_else(|| format!("{sequence}: text stream cursor overflow"))?;
    }

    Err(format!(
        "stream 0x{stream_offset:04X}가 lifecycle 종단 FF00/FF03/FF1A 없이 끝남"
    ))
}

#[derive(Debug)]
struct RootedRunBuilder {
    sequence_sha256: [u8; 32],
    raw: Vec<u8>,
    source: String,
    japanese_code_units: usize,
    owner_record_offsets: BTreeSet<usize>,
}

/// 임의 SEQ 집합에서 OP21 stream 후보와 token-rooted run을 만든다.
pub fn audit_quest_seq_text_streams<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    code_table: &QuestCodeTable,
) -> Result<QuestSeqTextStreamAnalysis, QuestSeqTextStreamError> {
    let mut input = BTreeMap::<String, &'a [u8]>::new();
    for (name, raw) in sequences {
        if name.is_empty() {
            return Err(error("QUEST SEQ text stream 입력 이름이 비어 있음"));
        }
        if input.insert(name.to_string(), raw).is_some() {
            return Err(error(format!("QUEST SEQ text stream 입력 중복: {name}")));
        }
    }
    if input.is_empty() {
        return Err(error("QUEST SEQ text stream 입력이 없음"));
    }

    let mut summaries = Vec::with_capacity(input.len());
    let mut markers = Vec::new();
    let mut rooted = BTreeMap::<(String, usize), RootedRunBuilder>::new();
    let mut code_table_run_candidates = 0usize;
    let mut covered_code_table_candidates = BTreeSet::<(String, usize)>::new();
    let mut exact_code_table_candidates = BTreeSet::<(String, usize)>::new();

    for (sequence, raw) in input {
        let sequence_sha256: [u8; 32] = Sha256::digest(raw).into();
        let code_table_runs = scan_quest_code_table_text_runs(raw, code_table);
        code_table_run_candidates += code_table_runs.len();
        let marker_start = markers.len();

        for record_offset in (0..raw.len()).step_by(QUEST_SEQ_TEXT_MODE_RECORD_BYTES) {
            if raw.get(record_offset) != Some(&QUEST_SEQ_TEXT_MODE_OPCODE) {
                continue;
            }
            let Some(record_end) = record_offset.checked_add(QUEST_SEQ_TEXT_MODE_RECORD_BYTES)
            else {
                continue;
            };
            let Some(record_slice) = raw.get(record_offset..record_end) else {
                continue;
            };
            let record_raw: [u8; QUEST_SEQ_TEXT_MODE_RECORD_BYTES] = record_slice
                .try_into()
                .expect("정확한 OP21 record slice 길이");

            let parsed = if record_raw[1..] == [0, 0, 0] {
                parse_quest_text_stream(&sequence, raw, record_end, code_table)
            } else {
                Err(format!(
                    "OP21 미소비 byte가 canonical zero가 아님: {:02X}{:02X}{:02X}",
                    record_raw[1], record_raw[2], record_raw[3]
                ))
            };
            match parsed {
                Ok(parsed) => {
                    for (offset, run_raw, source, japanese_code_units) in &parsed.runs {
                        let run_end = offset.checked_add(run_raw.len()).ok_or_else(|| {
                            error(format!("{sequence}: rooted run 범위 overflow"))
                        })?;
                        let containing_candidates = code_table_runs
                            .iter()
                            .filter(|candidate| {
                                let Some(candidate_end) =
                                    candidate.offset.checked_add(candidate.raw.len())
                                else {
                                    return false;
                                };
                                if candidate.offset > *offset || candidate_end < run_end {
                                    return false;
                                }
                                let relative = *offset - candidate.offset;
                                candidate.raw.get(relative..relative + run_raw.len())
                                    == Some(run_raw.as_slice())
                            })
                            .collect::<Vec<_>>();
                        let [containing_candidate] = containing_candidates.as_slice() else {
                            return Err(error(format!(
                                "{sequence}: OP21 rooted run을 포함하는 코드표 후보가 \
                                 정확히 하나가 아님: offset=0x{offset:04X}, actual={}",
                                containing_candidates.len()
                            )));
                        };
                        covered_code_table_candidates
                            .insert((sequence.clone(), containing_candidate.offset));
                        if containing_candidate.offset == *offset
                            && containing_candidate.raw == *run_raw
                            && containing_candidate.source == *source
                            && containing_candidate.japanese_code_units == *japanese_code_units
                        {
                            exact_code_table_candidates
                                .insert((sequence.clone(), containing_candidate.offset));
                        }
                        let key = (sequence.clone(), *offset);
                        match rooted.entry(key) {
                            std::collections::btree_map::Entry::Vacant(entry) => {
                                entry.insert(RootedRunBuilder {
                                    sequence_sha256,
                                    raw: run_raw.clone(),
                                    source: source.clone(),
                                    japanese_code_units: *japanese_code_units,
                                    owner_record_offsets: BTreeSet::from([record_offset]),
                                });
                            }
                            std::collections::btree_map::Entry::Occupied(mut entry) => {
                                let existing = entry.get_mut();
                                if existing.raw != *run_raw
                                    || existing.source != *source
                                    || existing.japanese_code_units != *japanese_code_units
                                {
                                    return Err(error(format!(
                                        "{sequence}: rooted run 0x{offset:04X} 내용 충돌"
                                    )));
                                }
                                existing.owner_record_offsets.insert(record_offset);
                            }
                        }
                    }
                    markers.push(QuestSeqTextStreamMarker {
                        sequence: sequence.clone(),
                        sequence_sha256,
                        record_offset,
                        record_raw,
                        stream_offset: record_end,
                        end_offset: Some(parsed.end_offset),
                        terminal: Some(parsed.terminal),
                        token_count: parsed.token_count,
                        glyph_count: parsed.glyph_count,
                        control_count: parsed.control_count,
                        rooted_run_count: parsed.runs.len(),
                        error: None,
                    });
                }
                Err(parse_error) => markers.push(QuestSeqTextStreamMarker {
                    sequence: sequence.clone(),
                    sequence_sha256,
                    record_offset,
                    record_raw,
                    stream_offset: record_end,
                    end_offset: None,
                    terminal: None,
                    token_count: 0,
                    glyph_count: 0,
                    control_count: 0,
                    rooted_run_count: 0,
                    error: Some(parse_error),
                }),
            }
        }

        let sequence_markers = &markers[marker_start..];
        let sequence_rooted = rooted
            .keys()
            .filter(|(candidate_sequence, _)| candidate_sequence == &sequence)
            .count();
        let rooted_code_units = rooted
            .iter()
            .filter(|((candidate_sequence, _), _)| candidate_sequence == &sequence)
            .map(|(_, run)| run.raw.len() / 2)
            .sum();
        let parseable_streams = sequence_markers
            .iter()
            .filter(|marker| marker.is_parseable())
            .count();
        summaries.push(QuestSeqTextStreamSummary {
            sequence,
            bytes: raw.len(),
            sha256: sequence_sha256,
            op21_shapes: sequence_markers.len(),
            parseable_streams,
            rejected_shapes: sequence_markers.len() - parseable_streams,
            rooted_runs: sequence_rooted,
            rooted_code_units,
        });
    }

    let rooted_runs = rooted
        .into_iter()
        .map(|((sequence, offset), run)| QuestSeqRootedTextRun {
            sequence,
            sequence_sha256: run.sequence_sha256,
            offset,
            raw: run.raw,
            source: run.source,
            japanese_code_units: run.japanese_code_units,
            owner_record_offsets: run.owner_record_offsets.into_iter().collect(),
        })
        .collect::<Vec<_>>();
    let rooted_code_units = rooted_runs
        .iter()
        .map(QuestSeqRootedTextRun::code_units)
        .sum();
    let unique_source_characters = rooted_runs
        .iter()
        .flat_map(|run| run.source.chars())
        .collect::<BTreeSet<_>>()
        .len();

    Ok(QuestSeqTextStreamAnalysis {
        code_table_candidates_with_rooted_run: covered_code_table_candidates.len(),
        exact_code_table_candidate_matches: exact_code_table_candidates.len(),
        summaries,
        markers,
        rooted_runs,
        code_table_run_candidates,
        rooted_code_units,
        unique_source_characters,
    })
}

/// 고정 지원 원본의 OP21 stream 분모와 두 report digest를 검증한다.
pub fn verify_box_quest_seq_text_streams<'a>(
    sequences: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    code_table: &QuestCodeTable,
) -> Result<QuestSeqTextStreamAnalysis, QuestSeqTextStreamError> {
    let analysis = audit_quest_seq_text_streams(sequences, code_table)?;
    let parseable = analysis
        .markers
        .iter()
        .filter(|marker| marker.is_parseable())
        .count();
    let noncanonical = analysis
        .markers
        .iter()
        .filter(|marker| marker.record_raw[1..] != [0, 0, 0])
        .count();
    let canonical_invalid = analysis
        .markers
        .iter()
        .filter(|marker| marker.record_raw[1..] == [0, 0, 0] && !marker.is_parseable())
        .count();
    let terminal_counts = [
        QuestTextTerminal::ReleaseInterpreter,
        QuestTextTerminal::DeactivateAndAlign,
        QuestTextTerminal::NotifyParentAndRelease,
    ]
    .map(|terminal| {
        analysis
            .markers
            .iter()
            .filter(|marker| marker.terminal == Some(terminal))
            .count()
    });
    let rooted_files = analysis
        .summaries
        .iter()
        .filter(|summary| summary.rooted_runs > 0)
        .count();
    let actual = [
        analysis.summaries.len(),
        analysis.summaries.iter().map(|summary| summary.bytes).sum(),
        analysis
            .summaries
            .iter()
            .filter(|summary| summary.op21_shapes > 0)
            .count(),
        rooted_files,
        analysis.markers.len(),
        parseable,
        analysis.markers.len() - parseable,
        noncanonical,
        canonical_invalid,
        analysis.code_table_run_candidates,
        analysis.code_table_candidates_with_rooted_run,
        analysis.exact_code_table_candidate_matches,
        analysis.rooted_runs.len(),
        analysis.rooted_code_units,
        analysis.unique_source_characters,
    ];
    let expected = [
        BOX_QUEST_SEQ_SEQUENCES,
        BOX_QUEST_SEQ_BYTES,
        BOX_QUEST_SEQ_TEXT_STREAM_FILES,
        BOX_QUEST_SEQ_ROOTED_TEXT_FILES,
        BOX_QUEST_SEQ_OP21_SHAPES,
        BOX_QUEST_SEQ_PARSEABLE_TEXT_STREAMS,
        BOX_QUEST_SEQ_REJECTED_OP21_SHAPES,
        BOX_QUEST_SEQ_NONCANONICAL_OP21_SHAPES,
        BOX_QUEST_SEQ_CANONICAL_INVALID_TEXT_STREAMS,
        BOX_QUEST_SEQ_ROOTED_TEXT_RUNS + BOX_QUEST_SEQ_UNROOTED_CODE_TABLE_RUNS,
        BOX_QUEST_SEQ_ROOTED_TEXT_RUNS,
        BOX_QUEST_SEQ_ROOTED_TEXT_RUNS,
        BOX_QUEST_SEQ_ROOTED_TEXT_RUNS,
        BOX_QUEST_SEQ_ROOTED_TEXT_CODE_UNITS,
        BOX_QUEST_SEQ_ROOTED_TEXT_UNIQUE_CHARACTERS,
    ];
    if actual != expected {
        return Err(error(format!(
            "QUEST SEQ OP21 text stream 분모 불일치: expected={expected:?}, actual={actual:?}"
        )));
    }
    if terminal_counts != [2, 603, 0] {
        return Err(error(format!(
            "QUEST SEQ OP21 text stream 종단 분모 불일치: \
             expected=[2, 603, 0], actual={terminal_counts:?}"
        )));
    }

    let actual_files = analysis
        .summaries
        .iter()
        .filter(|summary| summary.op21_shapes > 0)
        .map(|summary| {
            (
                summary.sequence.as_str(),
                summary.op21_shapes,
                summary.parseable_streams,
                summary.rejected_shapes,
                summary.rooted_runs,
                summary.rooted_code_units,
            )
        })
        .collect::<Vec<_>>();
    if actual_files != BOX_QUEST_SEQ_TEXT_STREAM_FILE_EXPECTATIONS {
        return Err(error(format!(
            "QUEST SEQ OP21 text stream 파일별 분모 불일치: \
             expected={BOX_QUEST_SEQ_TEXT_STREAM_FILE_EXPECTATIONS:?}, actual={actual_files:?}"
        )));
    }

    let stream_report = render_quest_seq_text_streams_tsv(&analysis);
    let stream_digest = format!("{:x}", Sha256::digest(stream_report.as_bytes()));
    if stream_digest != BOX_QUEST_SEQ_TEXT_STREAM_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST SEQ OP21 text stream report SHA-256 불일치: \
             expected={BOX_QUEST_SEQ_TEXT_STREAM_REPORT_SHA256}, actual={stream_digest}"
        )));
    }
    let rooted_report = render_quest_seq_rooted_text_runs_tsv(&analysis);
    let rooted_digest = format!("{:x}", Sha256::digest(rooted_report.as_bytes()));
    if rooted_digest != BOX_QUEST_SEQ_ROOTED_TEXT_REPORT_SHA256 {
        return Err(error(format!(
            "QUEST SEQ rooted text report SHA-256 불일치: \
             expected={BOX_QUEST_SEQ_ROOTED_TEXT_REPORT_SHA256}, actual={rooted_digest}"
        )));
    }
    Ok(analysis)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

/// OP21 shape의 성공·기각 판정을 모두 보존하는 결정적 TSV.
pub fn render_quest_seq_text_streams_tsv(analysis: &QuestSeqTextStreamAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tarchive_path\tarchive_sequence_count\tsequence_name\t\
         sequence_sha256\trecord_offset\trecord_raw_hex\tstream_offset\tend_offset\t\
         status\tterminal\ttoken_count\tglyph_count\tcontrol_count\trooted_run_count\terror\n",
    );
    for marker in &analysis.markers {
        let end_offset = marker
            .end_offset
            .map(|offset| format!("0x{offset:04X}"))
            .unwrap_or_default();
        let status = if marker.is_parseable() {
            "parseable-op21-text-stream-candidate"
        } else {
            "rejected-op21-shape"
        };
        let terminal = marker
            .terminal
            .map(QuestTextTerminal::label)
            .unwrap_or_default();
        writeln!(
            output,
            "{}\t/QUEST/RPG_SEQ.VDD;1\t{}\t{}\t{}\t0x{:04X}\t{}\t0x{:04X}\t{}\t\
             {}\t{}\t{}\t{}\t{}\t{}\t{}",
            QUEST_SEQ_TEXT_STREAM_SCHEMA_VERSION,
            analysis.summaries.len(),
            marker.sequence,
            hex(&marker.sequence_sha256),
            marker.record_offset,
            hex(&marker.record_raw),
            marker.stream_offset,
            end_offset,
            status,
            terminal,
            marker.token_count,
            marker.glyph_count,
            marker.control_count,
            marker.rooted_run_count,
            escape_tsv(marker.error.as_deref().unwrap_or_default()),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

/// parseable OP21 stream 안의 일본어 glyph run만 내보내는 결정적 TSV.
pub fn render_quest_seq_rooted_text_runs_tsv(analysis: &QuestSeqTextStreamAnalysis) -> String {
    let mut output = String::from(
        "schema_version\tarchive_path\tarchive_sequence_count\tsequence_name\t\
         sequence_sha256\tcandidate_id\toffset\traw_hex\tbyte_len\tcode_units\t\
         japanese_code_units\towner_op21_count\towner_op21_offsets\tevidence\tsource\n",
    );
    for run in &analysis.rooted_runs {
        let owner_offsets = run
            .owner_record_offsets
            .iter()
            .map(|offset| format!("0x{offset:04X}"))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(
            output,
            "{}\t/QUEST/RPG_SEQ.VDD;1\t{}\t{}\t{}\t{}\t0x{:04X}\t{}\t{}\t{}\t{}\t\
             {}\t{}\top21-token-rooted-static-candidate\t{}",
            QUEST_SEQ_ROOTED_TEXT_RUN_SCHEMA_VERSION,
            analysis.summaries.len(),
            run.sequence,
            hex(&run.sequence_sha256),
            run.id(),
            run.offset,
            hex(&run.raw),
            run.byte_len(),
            run.code_units(),
            run.japanese_code_units,
            run.owner_record_offsets.len(),
            owner_offsets,
            escape_tsv(&run.source),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_seq_text_streams_tests.rs"]
mod tests;
