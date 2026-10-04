//! `QUEST.EXE` 상주 graph가 소유하는 OP21 원문 단위.
//!
//! `RPG_SEQ.VDD`의 580개 번역 후보와 섞지 않고, 검증된 literal path와
//! OP80 파생 path가 실제로 포함하는 OP21 record만 읽는다. EXE 전체의
//! `0x21` byte를 다시 훑지 않는다. 각 단위는 record·token·glyph run의
//! 원본 좌표와 raw를 보존하며, 지원 원본 검증은 세 span의 SHA-256과
//! 보호 원문을 함께 확인한다.
//!
//! 이 결과는 정적 source graph에 결속한 추출이다. 어느 단위에도 runtime
//! 실행·표시 증거가 없으며, 번역 승인이나 제품 쓰기 자격을 뜻하지 않는다.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::font::{decode_quest_source_glyph, QuestCodeTable};

use super::quest_resident_seq_paths::{
    QuestResidentSeqPathNode, QuestResidentSeqPathSummary, QuestResidentSeqPathTransition,
};
use super::quest_resident_seq_spawn_path::QuestResidentSeqSpawnPathSummary;
use crate::script::quest::{
    parse_quest_token, quest_control_meaning, QuestControlMeaning, QuestToken,
};
use crate::script::quest_seq_text_streams::{
    QuestTextTerminal, QUEST_SEQ_TEXT_MODE_OPCODE, QUEST_SEQ_TEXT_MODE_RECORD_BYTES,
};

pub const BOX_QUEST_RESIDENT_TEXT_UNITS: usize = 3;
pub const BOX_QUEST_RESIDENT_TEXT_GLYPHS: usize = 61;
pub const BOX_QUEST_RESIDENT_JAPANESE_GLYPHS: usize = 46;
pub const BOX_QUEST_RESIDENT_TEXT_CONTROLS: usize = 39;
pub const BOX_QUEST_RESIDENT_TEXT_RUNS: usize = 9;
pub const BOX_QUEST_RESIDENT_RUNTIME_OBSERVED_TEXT_UNITS: usize = 0;

pub const QUEST_RESIDENT_START_MENU_TEXT_ID: &str = "QUEST-EXE-RESIDENT-TEXT:start-menu:0x80089650";
pub const QUEST_RESIDENT_FAREWELL_TEXT_ID: &str = "QUEST-EXE-RESIDENT-TEXT:farewell:0x800897F8";
pub const QUEST_RESIDENT_DEFEAT_TEXT_ID: &str = "QUEST-EXE-RESIDENT-TEXT:defeat:0x8008C4A0";

pub const QUEST_RESIDENT_START_MENU_TEXT_SHA256: &str =
    "8de52b2f5233768348d43c27c5158a27137bb44393a181513730c5e4abb8a497";
pub const QUEST_RESIDENT_FAREWELL_TEXT_SHA256: &str =
    "c096f5b2db4df1b98a2547624dc1bdcad2f81d93541b061faabf142cbebda985";
pub const QUEST_RESIDENT_DEFEAT_TEXT_SHA256: &str =
    "5e43e721413c07dfbe2004cc0bf8b0439568e76e80d469ee46508e9915508803";

const MAX_RESIDENT_TEXT_SPAN_BYTES: usize = 0x400;
const START_MENU_RUN_SOURCES: [&str; 4] = [
    "\u{3000}\u{3000}ぷよぷよクエスト",
    "つづきから",
    "はじめから",
    "もどる",
];
const FAREWELL_RUN_SOURCES: [&str; 2] = [
    "\u{3000}\u{3000}それじゃあ、おわるよ",
    "\u{3000}\u{3000}またね",
];
const DEFEAT_RUN_SOURCES: [&str; 3] = [
    "アルル「",
    "もう\u{3000}ダメェ～",
    "\u{3000}\u{3000}\u{3000}\u{3000}ばたんきゅ～",
];

#[derive(Debug, Clone, Copy)]
struct QuestResidentTextExpectation {
    id: &'static str,
    role: QuestResidentTextRole,
    record_runtime: u32,
    body_runtime: u32,
    end_runtime: u32,
    glyph_occurrences: usize,
    control_occurrences: usize,
    run_sources: &'static [&'static str],
    protected_source: &'static str,
    span_sha256: &'static str,
}

const QUEST_RESIDENT_TEXT_EXPECTATIONS: [QuestResidentTextExpectation;
    BOX_QUEST_RESIDENT_TEXT_UNITS] = [
    QuestResidentTextExpectation {
        id: QUEST_RESIDENT_START_MENU_TEXT_ID,
        role: QuestResidentTextRole::StartMenu,
        record_runtime: 0x8008_9650,
        body_runtime: 0x8008_9654,
        end_runtime: 0x8008_96CC,
        glyph_occurrences: 23,
        control_occurrences: 19,
        run_sources: &START_MENU_RUN_SOURCES,
        protected_source: "\u{3000}\u{3000}ぷよぷよクエストつづきからはじめからもどる",
        span_sha256: QUEST_RESIDENT_START_MENU_TEXT_SHA256,
    },
    QuestResidentTextExpectation {
        id: QUEST_RESIDENT_FAREWELL_TEXT_ID,
        role: QuestResidentTextRole::Farewell,
        record_runtime: 0x8008_97F8,
        body_runtime: 0x8008_97FC,
        end_runtime: 0x8008_9848,
        glyph_occurrences: 17,
        control_occurrences: 10,
        run_sources: &FAREWELL_RUN_SOURCES,
        protected_source: "\u{3000}\u{3000}それじゃあ、おわるよ\u{3000}\u{3000}またね",
        span_sha256: QUEST_RESIDENT_FAREWELL_TEXT_SHA256,
    },
    QuestResidentTextExpectation {
        id: QUEST_RESIDENT_DEFEAT_TEXT_ID,
        role: QuestResidentTextRole::Defeat,
        record_runtime: 0x8008_C4A0,
        body_runtime: 0x8008_C4A4,
        end_runtime: 0x8008_C4F4,
        glyph_occurrences: 21,
        control_occurrences: 10,
        run_sources: &DEFEAT_RUN_SOURCES,
        protected_source:
            "アルル「もう\u{3000}ダメェ～\u{3000}\u{3000}\u{3000}\u{3000}ばたんきゅ～",
        span_sha256: QUEST_RESIDENT_DEFEAT_TEXT_SHA256,
    },
];

/// EXE 상주 원문의 화면 역할. 실행 도달 판정은 포함하지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QuestResidentTextRole {
    StartMenu,
    Farewell,
    Defeat,
    EncounterMenu,
    EscapeAttempt,
    EscapeFailure,
}

impl QuestResidentTextRole {
    pub const fn label(self) -> &'static str {
        match self {
            Self::StartMenu => "start-menu",
            Self::Farewell => "farewell",
            Self::Defeat => "defeat",
            Self::EncounterMenu => "encounter-menu",
            Self::EscapeAttempt => "escape-attempt",
            Self::EscapeFailure => "escape-failure",
        }
    }
}

/// 보호 token의 소비 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestResidentTextTokenKind {
    Glyph,
    Control,
}

/// 상주 OP21 stream의 원본 좌표와 raw를 보존하는 token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextToken {
    pub index: usize,
    pub text_offset: usize,
    pub runtime: u32,
    pub raw: Vec<u8>,
    pub kind: QuestResidentTextTokenKind,
    pub source: Option<String>,
    pub control_code: Option<u8>,
    pub control_meaning: Option<QuestControlMeaning>,
}

/// control 사이에서 연속으로 소비되는 원본 glyph run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextRun {
    pub index: usize,
    pub text_offset: usize,
    pub runtime: u32,
    pub raw: Vec<u8>,
    pub source: String,
    pub glyph_occurrences: usize,
}

/// source graph에 결속한 `QUEST.EXE` 상주 OP21 원문 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextUnit {
    pub id: String,
    pub role: QuestResidentTextRole,
    pub record_runtime: u32,
    pub record_text_offset: usize,
    pub body_runtime: u32,
    pub body_text_offset: usize,
    /// lifecycle 종단 token까지 소비한 배타적 끝 주소.
    pub end_runtime: u32,
    pub end_text_offset: usize,
    pub record_raw: [u8; QUEST_SEQ_TEXT_MODE_RECORD_BYTES],
    pub terminal: QuestTextTerminal,
    pub expected_raw_sha256: [u8; 32],
    /// OP21 record부터 lifecycle 종단까지의 원본 span.
    pub protected_raw: Vec<u8>,
    pub protected_source: String,
    pub tokens: Vec<QuestResidentTextToken>,
    pub runs: Vec<QuestResidentTextRun>,
    pub glyph_occurrences: usize,
    pub japanese_glyph_occurrences: usize,
    pub control_occurrences: usize,
    /// 정적 graph 결속을 runtime 관측으로 오인하지 않게 항상 `None`이다.
    pub runtime_evidence: Option<&'static str>,
}

/// VDD 번역 모집단과 분리한 source-bound 상주 원문 요약.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextUnitAnalysis {
    pub units: Vec<QuestResidentTextUnit>,
    pub glyph_occurrences: usize,
    pub japanese_glyph_occurrences: usize,
    pub control_occurrences: usize,
    pub runs: usize,
    pub runtime_observed_units: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestResidentTextUnitError(String);

impl fmt::Display for QuestResidentTextUnitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestResidentTextUnitError {}

fn error(message: impl Into<String>) -> QuestResidentTextUnitError {
    QuestResidentTextUnitError(message.into())
}

fn checked_unit_sum(
    mut values: impl Iterator<Item = usize>,
    role: &str,
) -> Result<usize, QuestResidentTextUnitError> {
    values.try_fold(0usize, |sum, value| {
        sum.checked_add(value)
            .ok_or_else(|| error(format!("QUEST resident text {role} 합계 overflow")))
    })
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

fn finish_run(
    runs: &mut Vec<QuestResidentTextRun>,
    text_offset: &mut Option<usize>,
    runtime: &mut Option<u32>,
    raw: &mut Vec<u8>,
    source: &mut String,
    glyph_occurrences: &mut usize,
) {
    if let (Some(text_offset), Some(runtime)) = (text_offset.take(), runtime.take()) {
        runs.push(QuestResidentTextRun {
            index: runs.len(),
            text_offset,
            runtime,
            raw: std::mem::take(raw),
            source: std::mem::take(source),
            glyph_occurrences: *glyph_occurrences,
        });
    }
    *glyph_occurrences = 0;
}

fn graph_op21_nodes<'a>(
    paths: &'a QuestResidentSeqPathSummary,
    spawn: &'a QuestResidentSeqSpawnPathSummary,
) -> Result<BTreeMap<u32, &'a QuestResidentSeqPathNode>, QuestResidentTextUnitError> {
    let literal_nodes = paths.paths.iter().flat_map(|path| &path.nodes);
    let spawn_nodes = spawn
        .path
        .nodes
        .iter()
        .chain(spawn.deferred_path.nodes.iter());
    let mut by_runtime = BTreeMap::new();
    for node in literal_nodes.chain(spawn_nodes) {
        if node.opcode != QUEST_SEQ_TEXT_MODE_OPCODE {
            continue;
        }
        if by_runtime.insert(node.runtime, node).is_some() {
            return Err(error(format!(
                "QUEST resident OP21 graph record 중복: 0x{:08X}",
                node.runtime
            )));
        }
    }

    let literal_op21_records = paths
        .paths
        .iter()
        .flat_map(|path| &path.nodes)
        .filter(|node| node.opcode == QUEST_SEQ_TEXT_MODE_OPCODE)
        .count();
    let literal_glyphs = paths
        .paths
        .iter()
        .flat_map(|path| &path.nodes)
        .map(|node| node.text_glyphs)
        .sum::<usize>();
    let literal_controls = paths
        .paths
        .iter()
        .flat_map(|path| &path.nodes)
        .map(|node| node.text_controls)
        .sum::<usize>();
    if literal_op21_records != paths.op21_records
        || literal_glyphs != paths.text_glyphs
        || literal_controls != paths.text_controls
    {
        return Err(error(format!(
            "QUEST resident literal graph 요약 불일치: OP21 {literal_op21_records}/{}, \
             glyph {literal_glyphs}/{}, control {literal_controls}/{}",
            paths.op21_records, paths.text_glyphs, paths.text_controls
        )));
    }
    let spawn_glyphs = spawn
        .path
        .nodes
        .iter()
        .map(|node| node.text_glyphs)
        .sum::<usize>();
    let spawn_controls = spawn
        .path
        .nodes
        .iter()
        .map(|node| node.text_controls)
        .sum::<usize>();
    if spawn_glyphs != spawn.text_glyphs || spawn_controls != spawn.text_controls {
        return Err(error(format!(
            "QUEST resident spawn graph 요약 불일치: glyph {spawn_glyphs}/{}, \
             control {spawn_controls}/{}",
            spawn.text_glyphs, spawn.text_controls
        )));
    }

    let expected = QUEST_RESIDENT_TEXT_EXPECTATIONS
        .iter()
        .map(|expectation| expectation.record_runtime)
        .collect::<Vec<_>>();
    let actual = by_runtime.keys().copied().collect::<Vec<_>>();
    if actual != expected {
        return Err(error(format!(
            "QUEST resident OP21 graph 분모 불일치: expected={expected:08X?}, actual={actual:08X?}"
        )));
    }
    Ok(by_runtime)
}

fn transition_for_terminal(terminal: QuestTextTerminal) -> QuestResidentSeqPathTransition {
    match terminal {
        QuestTextTerminal::ReleaseInterpreter => QuestResidentSeqPathTransition::TextRelease,
        QuestTextTerminal::DeactivateAndAlign => {
            QuestResidentSeqPathTransition::TextDeactivateAndAlign
        }
        QuestTextTerminal::NotifyParentAndRelease => {
            QuestResidentSeqPathTransition::TextNotifyAndRelease
        }
    }
}

pub(super) fn extract_quest_resident_text_unit(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    node: &QuestResidentSeqPathNode,
    id: &str,
    role: QuestResidentTextRole,
) -> Result<QuestResidentTextUnit, QuestResidentTextUnitError> {
    let record_text_offset = map.text_offset(node.runtime).ok_or_else(|| {
        error(format!(
            "QUEST resident OP21 record가 EXE text 밖임: 0x{:08X}",
            node.runtime
        ))
    })?;
    let body_text_offset = record_text_offset
        .checked_add(QUEST_SEQ_TEXT_MODE_RECORD_BYTES)
        .ok_or_else(|| error(format!("{id}: body offset overflow")))?;
    let body_runtime = map.text_runtime_addr(body_text_offset).ok_or_else(|| {
        error(format!(
            "{id}: body가 EXE text 밖임: +0x{body_text_offset:X}"
        ))
    })?;
    let record_end = body_text_offset;
    let record: [u8; QUEST_SEQ_TEXT_MODE_RECORD_BYTES] = text
        .get(record_text_offset..record_end)
        .ok_or_else(|| error(format!("{id}: OP21 record가 잘림")))?
        .try_into()
        .expect("OP21 record 범위를 확인함");
    if record != [QUEST_SEQ_TEXT_MODE_OPCODE, 0, 0, 0] {
        return Err(error(format!(
            "{id}: canonical OP21 record가 아님: {record:02X?}"
        )));
    }

    let mut cursor = body_text_offset;
    let mut tokens = Vec::new();
    let mut runs = Vec::new();
    let mut run_text_offset = None;
    let mut run_runtime = None;
    let mut run_raw = Vec::new();
    let mut run_source = String::new();
    let mut run_glyphs = 0usize;
    let mut protected_source = String::new();
    let mut glyph_occurrences = 0usize;
    let mut japanese_glyph_occurrences = 0usize;
    let mut control_occurrences = 0usize;
    let mut terminal = None;

    loop {
        if cursor
            .checked_sub(record_text_offset)
            .ok_or_else(|| error(format!("{id}: text cursor 역행")))?
            >= MAX_RESIDENT_TEXT_SPAN_BYTES
        {
            return Err(error(format!(
                "{id}: text lifecycle가 0x{MAX_RESIDENT_TEXT_SPAN_BYTES:X}바이트 안에 끝나지 않음"
            )));
        }
        let runtime = map
            .text_runtime_addr(cursor)
            .ok_or_else(|| error(format!("{id}: token이 EXE text 밖임: +0x{cursor:X}")))?;
        let parsed = parse_quest_token(
            text.get(cursor..)
                .ok_or_else(|| error(format!("{id}: token 입력이 잘림")))?,
            runtime,
        )
        .map_err(|cause| error(format!("{id}: token 파싱 실패: {cause}")))?;
        let raw = parsed.raw().to_vec();
        let token_len = raw.len();
        let token_index = tokens.len();
        match parsed {
            QuestToken::Glyph(pair) => {
                let code: [u8; 2] = pair
                    .try_into()
                    .map_err(|_| error(format!("{id}: glyph 폭 불일치")))?;
                if code_table.index_of(code).is_none() {
                    return Err(error(format!(
                        "{id}: glyph가 QUEST 코드표에 없음: {:02X}{:02X}",
                        code[0], code[1]
                    )));
                }
                let character = decode_quest_source_glyph(code).ok_or_else(|| {
                    error(format!(
                        "{id}: glyph 원문을 한 문자로 decode할 수 없음: {:02X}{:02X}",
                        code[0], code[1]
                    ))
                })?;
                run_text_offset.get_or_insert(cursor);
                run_runtime.get_or_insert(runtime);
                run_raw.extend_from_slice(pair);
                run_source.push(character);
                run_glyphs += 1;
                protected_source.push(character);
                glyph_occurrences += 1;
                japanese_glyph_occurrences += usize::from(is_japanese(character));
                tokens.push(QuestResidentTextToken {
                    index: token_index,
                    text_offset: cursor,
                    runtime,
                    raw,
                    kind: QuestResidentTextTokenKind::Glyph,
                    source: Some(character.to_string()),
                    control_code: None,
                    control_meaning: None,
                });
            }
            QuestToken::Control { spec, .. } => {
                finish_run(
                    &mut runs,
                    &mut run_text_offset,
                    &mut run_runtime,
                    &mut run_raw,
                    &mut run_source,
                    &mut run_glyphs,
                );
                let meaning = quest_control_meaning(spec.code).ok_or_else(|| {
                    error(format!("{id}: control 의미 누락: FF{:02X}", spec.code))
                })?;
                control_occurrences += 1;
                tokens.push(QuestResidentTextToken {
                    index: token_index,
                    text_offset: cursor,
                    runtime,
                    raw,
                    kind: QuestResidentTextTokenKind::Control,
                    source: None,
                    control_code: Some(spec.code),
                    control_meaning: Some(meaning),
                });
                terminal = QuestTextTerminal::from_control(spec.code);
            }
        }
        cursor = cursor
            .checked_add(token_len)
            .ok_or_else(|| error(format!("{id}: token cursor overflow")))?;
        if terminal.is_some() {
            break;
        }
    }

    let terminal = terminal.expect("terminal이 있을 때만 loop를 끝냄");
    let end_text_offset = cursor;
    let end_runtime = map.text_runtime_addr(end_text_offset).ok_or_else(|| {
        error(format!(
            "{id}: text 종단이 EXE text 밖임: +0x{end_text_offset:X}"
        ))
    })?;
    if node.transition != transition_for_terminal(terminal)
        || node.next_runtime
            != (terminal == QuestTextTerminal::DeactivateAndAlign).then_some(end_runtime)
        || node.text_glyphs != glyph_occurrences
        || node.text_controls != control_occurrences
    {
        return Err(error(format!(
            "{id}: graph OP21 lifecycle 불일치: transition={:?}/{:?}, next={:08X?}, \
             glyph={}/{}, control={}/{}",
            node.transition,
            transition_for_terminal(terminal),
            node.next_runtime,
            node.text_glyphs,
            glyph_occurrences,
            node.text_controls,
            control_occurrences,
        )));
    }
    if japanese_glyph_occurrences == 0 {
        return Err(error(format!(
            "{id}: 일본어 glyph가 없는 unit을 resident 번역 대상으로 채택할 수 없음"
        )));
    }
    let protected_raw = text
        .get(record_text_offset..end_text_offset)
        .ok_or_else(|| error(format!("{id}: 보호 span이 EXE text 밖임")))?
        .to_vec();
    let expected_raw_sha256: [u8; 32] = Sha256::digest(&protected_raw).into();

    Ok(QuestResidentTextUnit {
        id: id.to_string(),
        role,
        record_runtime: node.runtime,
        record_text_offset,
        body_runtime,
        body_text_offset,
        end_runtime,
        end_text_offset,
        record_raw: record,
        terminal,
        expected_raw_sha256,
        protected_raw,
        protected_source,
        tokens,
        runs,
        glyph_occurrences,
        japanese_glyph_occurrences,
        control_occurrences,
        runtime_evidence: None,
    })
}

/// 서로 다른 source graph가 소유한 resident unit 집합을 하나의 분모로 합친다.
///
/// 입력 순서는 유지하되 unit ID와 보호 text span이 겹치면 거부한다. 합계는
/// 개별 unit에서 다시 계산하므로 호출자가 별도 수치를 주입할 수 없다.
pub fn combine_quest_resident_text_units<'a>(
    groups: impl IntoIterator<Item = &'a [QuestResidentTextUnit]>,
) -> Result<QuestResidentTextUnitAnalysis, QuestResidentTextUnitError> {
    let units = groups
        .into_iter()
        .flat_map(|group| group.iter().cloned())
        .collect::<Vec<_>>();
    if units.is_empty() {
        return Err(error("QUEST resident text 결합 입력이 비었음"));
    }
    let mut ids = BTreeMap::new();
    for (index, unit) in units.iter().enumerate() {
        if let Some(first) = ids.insert(unit.id.as_str(), index) {
            return Err(error(format!(
                "QUEST resident text unit ID 중복: {} at {first}/{index}",
                unit.id
            )));
        }
    }
    let mut ranges = units
        .iter()
        .enumerate()
        .map(|(index, unit)| (unit.record_text_offset, unit.end_text_offset, index))
        .collect::<Vec<_>>();
    ranges.sort_unstable();
    for pair in ranges.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err(error(format!(
                "QUEST resident text 보호 span 겹침: {} / {}",
                units[pair[0].2].id, units[pair[1].2].id
            )));
        }
    }
    Ok(QuestResidentTextUnitAnalysis {
        glyph_occurrences: checked_unit_sum(
            units.iter().map(|unit| unit.glyph_occurrences),
            "glyph",
        )?,
        japanese_glyph_occurrences: checked_unit_sum(
            units.iter().map(|unit| unit.japanese_glyph_occurrences),
            "Japanese glyph",
        )?,
        control_occurrences: checked_unit_sum(
            units.iter().map(|unit| unit.control_occurrences),
            "control",
        )?,
        runs: checked_unit_sum(units.iter().map(|unit| unit.runs.len()), "run")?,
        runtime_observed_units: units
            .iter()
            .filter(|unit| unit.runtime_evidence.is_some())
            .count(),
        units,
    })
}

/// 검증된 resident graph의 세 OP21만 token 보존형 단위로 추출한다.
pub fn audit_quest_resident_text_units(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    paths: &QuestResidentSeqPathSummary,
    spawn: &QuestResidentSeqSpawnPathSummary,
) -> Result<QuestResidentTextUnitAnalysis, QuestResidentTextUnitError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST resident text 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len
        )));
    }
    let mut graph = graph_op21_nodes(paths, spawn)?;
    let mut units = Vec::with_capacity(QUEST_RESIDENT_TEXT_EXPECTATIONS.len());
    for expectation in QUEST_RESIDENT_TEXT_EXPECTATIONS {
        let node = graph.remove(&expectation.record_runtime).ok_or_else(|| {
            error(format!(
                "{}: graph OP21 record 누락: 0x{:08X}",
                expectation.id, expectation.record_runtime
            ))
        })?;
        units.push(extract_quest_resident_text_unit(
            map,
            text,
            code_table,
            node,
            expectation.id,
            expectation.role,
        )?);
    }
    if !graph.is_empty() {
        return Err(error(format!(
            "QUEST resident text에 미등록 graph OP21이 남음: {:08X?}",
            graph.keys().collect::<Vec<_>>()
        )));
    }

    combine_quest_resident_text_units([units.as_slice()])
}

/// 지원 원본의 세 resident 단위와 보호 source를 fail closed로 검증한다.
pub fn verify_box_quest_resident_text_units(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    paths: &QuestResidentSeqPathSummary,
    spawn: &QuestResidentSeqSpawnPathSummary,
) -> Result<QuestResidentTextUnitAnalysis, QuestResidentTextUnitError> {
    let analysis = audit_quest_resident_text_units(map, text, code_table, paths, spawn)?;
    let actual_totals = [
        analysis.units.len(),
        analysis.glyph_occurrences,
        analysis.japanese_glyph_occurrences,
        analysis.control_occurrences,
        analysis.runs,
        analysis.runtime_observed_units,
    ];
    let expected_totals = [
        BOX_QUEST_RESIDENT_TEXT_UNITS,
        BOX_QUEST_RESIDENT_TEXT_GLYPHS,
        BOX_QUEST_RESIDENT_JAPANESE_GLYPHS,
        BOX_QUEST_RESIDENT_TEXT_CONTROLS,
        BOX_QUEST_RESIDENT_TEXT_RUNS,
        BOX_QUEST_RESIDENT_RUNTIME_OBSERVED_TEXT_UNITS,
    ];
    if actual_totals != expected_totals {
        return Err(error(format!(
            "QUEST resident text 분모 불일치: expected={expected_totals:?}, actual={actual_totals:?}"
        )));
    }

    for (unit, expectation) in analysis.units.iter().zip(QUEST_RESIDENT_TEXT_EXPECTATIONS) {
        let actual_digest = format!("{:x}", Sha256::digest(&unit.protected_raw));
        let actual_runs = unit
            .runs
            .iter()
            .map(|run| run.source.as_str())
            .collect::<Vec<_>>();
        if unit.id != expectation.id
            || unit.role != expectation.role
            || unit.record_runtime != expectation.record_runtime
            || unit.body_runtime != expectation.body_runtime
            || unit.end_runtime != expectation.end_runtime
            || unit.glyph_occurrences != expectation.glyph_occurrences
            || unit.control_occurrences != expectation.control_occurrences
            || actual_runs != expectation.run_sources
            || unit.protected_source != expectation.protected_source
            || actual_digest != expectation.span_sha256
            || unit.runtime_evidence.is_some()
        {
            return Err(error(format!(
                "{}: 보호 source drift: role={:?}, record/body/end=0x{:08X}/0x{:08X}/0x{:08X}, \
                 glyph/control={}/{}, runs={actual_runs:?}, source={:?}, SHA-256={actual_digest}, \
                 runtime={:?}",
                expectation.id,
                unit.role,
                unit.record_runtime,
                unit.body_runtime,
                unit.end_runtime,
                unit.glyph_occurrences,
                unit.control_occurrences,
                unit.protected_source,
                unit.runtime_evidence,
            )));
        }
    }
    Ok(analysis)
}

#[cfg(test)]
#[path = "quest_resident_text_units_tests.rs"]
mod tests;
