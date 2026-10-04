//! `QUEST.EXE` 상주 전투 이름 분기표와 고정 slot.
//!
//! `0x80084D44`의 `OP09`는 interpreter object `+0x34`의 byte를 index로
//! 사용해 바로 뒤 pointer table에서 상대별 stream을 고른다. 각 stream의
//! `OP11/0x40` index `0x38`은 `FF1E(mode 0)`가 읽는 전역 word에 이름
//! pointer를 대입한다. index `0x16`의 다음 payload pointer는 이름 slot의
//! 끝을 제공한다. 문자열 모양 검색이나 다음 0-byte 추측으로 모집단·용량을
//! 만들지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::disasm::{
    parse_quest_seq_global_word_assignment, QUEST_FIXED_TEXT_POINTER_RUNTIME,
    QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_BYTES, QUEST_SEQ_GLOBAL_WORD_STATE_BASE_RUNTIME,
};
use crate::font::{decode_quest_source_glyph, QuestCodeTable};

pub const QUEST_BATTLE_NAME_SELECTOR_RUNTIME: u32 = 0x8008_4D44;
pub const QUEST_BATTLE_NAME_SELECTOR_FIELD_OFFSET: u8 = 0x34;
pub const QUEST_BATTLE_NAME_STATE_INDEX: u8 = 0x38;
pub const QUEST_BATTLE_NAME_SLOT_END_STATE_INDEX: u8 = 0x16;

const BOX_QUEST_BATTLE_NAMES: [&str; 59] = [
    "ドラコ",
    "スケルトンＴ",
    "アウルベア",
    "ハーピー",
    "セリリ",
    "すけとうだら",
    "トリオ・ザ・バンシー",
    "ぞう大魔王",
    "ミノタウロス",
    "ルルー",
    "サタン",
    "ミニゾンビ",
    "ウィル・オー・ウィスプ",
    "ナスグレイブ",
    "サムライモール",
    "マミー",
    "バロメッツ",
    "ふたごのケットシー",
    "おおかみおとこ",
    "アーちゃん",
    "スキヤポデス",
    "やきナスグレイブ",
    "ハニービー",
    "コドモドラゴン",
    "グレイウルフ",
    "ゾンビ",
    "スキュラ",
    "バーニングソウル",
    "のみ",
    "ウッドハンター",
    "つじぎりモール",
    "コカトリス",
    "まもの",
    "ドラゴン",
    "シェゾ",
    "キキーモラ",
    "ちょっぷん",
    "ドラコ",
    "カーバンクル",
    "インキュバス",
    "パノッティ",
    "ウィッチ",
    "ラグナス",
    "ミニゾンビ",
    "ぞう大魔王",
    "ももも",
    "のほほ",
    "パキスタ",
    "パララ",
    "ふふふ",
    "さそりまん",
    "サキュバス",
    "ドッペルアルル",
    "マスクドサタン",
    "アルル",
    "チコ",
    "ドラゴン４",
    "メロウ",
    "むほほ",
];

pub const BOX_QUEST_BATTLE_NAME_ENTRIES: usize = BOX_QUEST_BATTLE_NAMES.len();

/// 분기표 entry 하나가 소유하는 이름 pointer와 덮어쓸 수 있는 slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestBattleNameEntry {
    pub id: String,
    pub index: usize,
    pub branch_pointer_runtime: u32,
    pub stream_runtime: u32,
    pub name_assignment_runtime: u32,
    pub name_runtime: u32,
    pub slot_end_assignment_runtime: u32,
    pub slot_end_runtime: u32,
    pub name_text_offset: usize,
    pub slot_end_text_offset: usize,
    pub source: String,
    /// NUL 앞의 원본 u16 glyph code.
    pub source_raw: Vec<u8>,
    /// 이름 시작부터 index `0x16` pointer가 가리키는 경계 직전까지.
    pub protected_slot: Vec<u8>,
    pub protected_slot_sha256: [u8; 32],
    pub glyph_occurrences: usize,
    /// 끝의 u16 NUL을 제외하고 넣을 수 있는 최대 glyph 수.
    pub slot_capacity_glyphs: usize,
}

/// 소비자에서 역산한 전투 이름 모집단.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestBattleNameTableAnalysis {
    pub selector_runtime: u32,
    pub selector_field_offset: u8,
    pub pointer_table_runtime: u32,
    pub pointer_table_end_runtime: u32,
    pub fixed_text_state_index: u8,
    pub fixed_text_state_runtime: u32,
    pub entries: Vec<QuestBattleNameEntry>,
    pub unique_sources: usize,
    pub glyph_occurrences: usize,
    pub slot_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestBattleNameTableError(String);

impl fmt::Display for QuestBattleNameTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestBattleNameTableError {}

fn error(message: impl Into<String>) -> QuestBattleNameTableError {
    QuestBattleNameTableError(message.into())
}

fn read_u32(text: &[u8], offset: usize, role: &str) -> Result<u32, QuestBattleNameTableError> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| error(format!("QUEST battle name {role} 범위 overflow")))?;
    let raw = text
        .get(offset..end)
        .ok_or_else(|| error(format!("QUEST battle name {role}가 잘림")))?;
    Ok(u32::from_le_bytes(
        raw.try_into().expect("4바이트 범위를 확인함"),
    ))
}

fn runtime_at(map: &ExeMap, offset: usize, role: &str) -> Result<u32, QuestBattleNameTableError> {
    map.text_runtime_addr(offset).ok_or_else(|| {
        error(format!(
            "QUEST battle name {role}가 EXE text 밖임: +0x{offset:X}"
        ))
    })
}

fn decode_name_slot(
    code_table: &QuestCodeTable,
    slot: &[u8],
    index: usize,
) -> Result<(String, Vec<u8>, usize), QuestBattleNameTableError> {
    if slot.len() < 2 || !slot.len().is_multiple_of(2) {
        return Err(error(format!(
            "QUEST battle name #{index} slot 폭이 u16 경계가 아님: {}",
            slot.len()
        )));
    }
    let terminator = slot
        .chunks_exact(2)
        .position(|pair| pair == [0, 0])
        .ok_or_else(|| error(format!("QUEST battle name #{index} u16 NUL이 없음")))?;
    let source_end = terminator
        .checked_mul(2)
        .ok_or_else(|| error("QUEST battle name source 끝 overflow"))?;
    if slot[source_end + 2..].iter().any(|byte| *byte != 0) {
        return Err(error(format!(
            "QUEST battle name #{index} NUL 뒤 slot padding이 zero가 아님"
        )));
    }
    let source_raw = slot[..source_end].to_vec();
    if source_raw.is_empty() {
        return Err(error(format!("QUEST battle name #{index} source가 비었음")));
    }
    let mut source = String::new();
    for pair in source_raw.chunks_exact(2) {
        let code = [pair[0], pair[1]];
        if code_table.index_of(code).is_none() {
            return Err(error(format!(
                "QUEST battle name #{index} glyph가 코드표에 없음: {:02X}{:02X}",
                code[0], code[1]
            )));
        }
        let character = decode_quest_source_glyph(code).ok_or_else(|| {
            error(format!(
                "QUEST battle name #{index} glyph를 한 문자로 decode할 수 없음: {:02X}{:02X}",
                code[0], code[1]
            ))
        })?;
        source.push(character);
    }
    Ok((source, source_raw, terminator))
}

/// `OP09` pointer table과 각 대상의 `OP11`을 따라 이름 slot을 전수 추출한다.
pub fn audit_quest_battle_name_table(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
    selector_runtime: u32,
) -> Result<QuestBattleNameTableAnalysis, QuestBattleNameTableError> {
    if map.name != "QUEST.EXE" || text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST battle name 입력 불일치: name={}, text={}/{}",
            map.name,
            text.len(),
            map.text_len,
        )));
    }
    let selector_offset = map.text_offset(selector_runtime).ok_or_else(|| {
        error(format!(
            "QUEST battle name selector가 EXE text 밖임: 0x{selector_runtime:08X}"
        ))
    })?;
    let selector = text
        .get(selector_offset..selector_offset + 4)
        .ok_or_else(|| error("QUEST battle name OP09 selector가 잘림"))?;
    if selector[0] != 0x09 {
        return Err(error(format!(
            "QUEST battle name selector가 OP09가 아님: {:02X?}",
            selector
        )));
    }
    let pointer_table_runtime = selector_runtime
        .checked_add(4)
        .ok_or_else(|| error("QUEST battle name pointer table runtime overflow"))?;
    let first_target = read_u32(text, selector_offset + 4, "첫 branch pointer")?;
    let pointer_bytes = first_target
        .checked_sub(pointer_table_runtime)
        .ok_or_else(|| error("QUEST battle name 첫 target이 pointer table보다 앞섬"))?;
    if pointer_bytes == 0 || !pointer_bytes.is_multiple_of(4) {
        return Err(error(format!(
            "QUEST battle name pointer table 끝을 첫 target에서 유도할 수 없음: {pointer_bytes}B"
        )));
    }
    let entry_count = usize::try_from(pointer_bytes / 4)
        .map_err(|_| error("QUEST battle name entry count 변환 overflow"))?;
    if entry_count > 256 {
        return Err(error(format!(
            "QUEST battle name pointer table이 object byte index 범위를 넘음: {entry_count}"
        )));
    }
    let pointer_table_end_runtime = first_target;
    let mut targets = Vec::with_capacity(entry_count);
    for index in 0..entry_count {
        let pointer_offset = selector_offset
            .checked_add(4 + index * 4)
            .ok_or_else(|| error("QUEST battle name pointer field overflow"))?;
        let target = read_u32(text, pointer_offset, "branch pointer")?;
        map.text_offset(target).ok_or_else(|| {
            error(format!(
                "QUEST battle name #{index} branch target이 EXE text 밖임: 0x{target:08X}"
            ))
        })?;
        targets.push((runtime_at(map, pointer_offset, "branch pointer")?, target));
    }
    if targets.first().map(|(_, target)| *target) != Some(pointer_table_end_runtime) {
        return Err(error(
            "QUEST battle name pointer table 끝과 첫 target이 다름",
        ));
    }
    if targets
        .iter()
        .map(|(_, target)| *target)
        .collect::<BTreeSet<_>>()
        .len()
        != targets.len()
    {
        return Err(error("QUEST battle name branch target이 중복됨"));
    }

    let mut entries = Vec::with_capacity(entry_count);
    for (index, (branch_pointer_runtime, stream_runtime)) in targets.into_iter().enumerate() {
        let mut cursor = map
            .text_offset(stream_runtime)
            .expect("target 범위를 확인함");
        let mut assignments = BTreeMap::new();
        for _ in 0..64 {
            let Some(assignment) = parse_quest_seq_global_word_assignment(
                text.get(cursor..)
                    .ok_or_else(|| error("QUEST battle name assignment 입력이 잘림"))?,
            ) else {
                break;
            };
            if assignment.unconsumed_parameter != 0 {
                return Err(error(format!(
                    "QUEST battle name #{index} OP11 미소비 byte가 zero가 아님: index=0x{:02X}, value=0x{:02X}",
                    assignment.state_index, assignment.unconsumed_parameter,
                )));
            }
            let assignment_runtime = runtime_at(map, cursor, "assignment")?;
            if assignments
                .insert(
                    assignment.state_index,
                    (assignment_runtime, assignment.value),
                )
                .is_some()
            {
                return Err(error(format!(
                    "QUEST battle name #{index} global state index가 중복됨: 0x{:02X}",
                    assignment.state_index
                )));
            }
            cursor = cursor
                .checked_add(QUEST_SEQ_GLOBAL_WORD_ASSIGNMENT_BYTES)
                .ok_or_else(|| error("QUEST battle name assignment cursor overflow"))?;
        }
        let (name_assignment_runtime, name_runtime) = assignments
            .get(&QUEST_BATTLE_NAME_STATE_INDEX)
            .copied()
            .ok_or_else(|| error(format!("QUEST battle name #{index} index 0x38 대입이 없음")))?;
        let (slot_end_assignment_runtime, slot_end_runtime) = assignments
            .get(&QUEST_BATTLE_NAME_SLOT_END_STATE_INDEX)
            .copied()
            .ok_or_else(|| error(format!("QUEST battle name #{index} index 0x16 대입이 없음")))?;
        let name_text_offset = map.text_offset(name_runtime).ok_or_else(|| {
            error(format!(
                "QUEST battle name #{index} source pointer가 EXE text 밖임: 0x{name_runtime:08X}"
            ))
        })?;
        let slot_end_text_offset = map.text_offset(slot_end_runtime).ok_or_else(|| {
            error(format!(
                "QUEST battle name #{index} slot end pointer가 EXE text 밖임: 0x{slot_end_runtime:08X}"
            ))
        })?;
        let protected_slot = text
            .get(name_text_offset..slot_end_text_offset)
            .ok_or_else(|| {
                error(format!(
                    "QUEST battle name #{index} slot이 역행하거나 EXE text 밖임: 0x{name_runtime:08X}..0x{slot_end_runtime:08X}"
                ))
            })?
            .to_vec();
        let (source, source_raw, glyph_occurrences) =
            decode_name_slot(code_table, &protected_slot, index)?;
        let slot_capacity_glyphs = protected_slot
            .len()
            .checked_div(2)
            .and_then(|units| units.checked_sub(1))
            .ok_or_else(|| error("QUEST battle name slot capacity underflow"))?;
        entries.push(QuestBattleNameEntry {
            id: format!("QUEST-EXE-BATTLE-NAME:{index:02}:0x{name_runtime:08X}"),
            index,
            branch_pointer_runtime,
            stream_runtime,
            name_assignment_runtime,
            name_runtime,
            slot_end_assignment_runtime,
            slot_end_runtime,
            name_text_offset,
            slot_end_text_offset,
            source,
            source_raw,
            protected_slot_sha256: Sha256::digest(&protected_slot).into(),
            protected_slot,
            glyph_occurrences,
            slot_capacity_glyphs,
        });
    }
    for entry in &entries {
        let pointer_raw = entry.name_runtime.to_le_bytes();
        let pointer_hits = text
            .windows(pointer_raw.len())
            .enumerate()
            .filter(|(_, raw)| *raw == pointer_raw)
            .map(|(offset, _)| runtime_at(map, offset, "name pointer 참조"))
            .collect::<Result<Vec<_>, _>>()?;
        let expected_pointer_runtime = entry
            .name_assignment_runtime
            .checked_add(4)
            .ok_or_else(|| error("QUEST battle name assignment payload runtime overflow"))?;
        if pointer_hits != [expected_pointer_runtime] {
            return Err(error(format!(
                "QUEST battle name #{} 직접 pointer 참조 분모 drift: expected=[0x{expected_pointer_runtime:08X}], actual={pointer_hits:08X?}",
                entry.index
            )));
        }
    }
    let mut ranges = entries
        .iter()
        .map(|entry| {
            (
                entry.name_text_offset,
                entry.slot_end_text_offset,
                entry.index,
            )
        })
        .collect::<Vec<_>>();
    ranges.sort_unstable();
    for pair in ranges.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err(error(format!(
                "QUEST battle name slot이 겹침: #{}와 #{}",
                pair[0].2, pair[1].2
            )));
        }
    }

    let fixed_text_state_runtime = QUEST_SEQ_GLOBAL_WORD_STATE_BASE_RUNTIME
        .checked_add(u32::from(QUEST_BATTLE_NAME_STATE_INDEX) * 4)
        .ok_or_else(|| error("QUEST battle name state runtime overflow"))?;
    if fixed_text_state_runtime != QUEST_FIXED_TEXT_POINTER_RUNTIME {
        return Err(error(format!(
            "QUEST battle name state와 FF1E fixed-text pointer가 다름: 0x{fixed_text_state_runtime:08X}/0x{QUEST_FIXED_TEXT_POINTER_RUNTIME:08X}"
        )));
    }
    Ok(QuestBattleNameTableAnalysis {
        selector_runtime,
        selector_field_offset: selector[1],
        pointer_table_runtime,
        pointer_table_end_runtime,
        fixed_text_state_index: QUEST_BATTLE_NAME_STATE_INDEX,
        fixed_text_state_runtime,
        unique_sources: entries
            .iter()
            .map(|entry| entry.source.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        glyph_occurrences: entries.iter().map(|entry| entry.glyph_occurrences).sum(),
        slot_bytes: entries.iter().map(|entry| entry.protected_slot.len()).sum(),
        entries,
    })
}

/// 지원 원본의 `OP09` 분기 순서와 59개 보호 이름을 fail closed로 검증한다.
pub fn verify_box_quest_battle_name_table(
    map: &ExeMap,
    text: &[u8],
    code_table: &QuestCodeTable,
) -> Result<QuestBattleNameTableAnalysis, QuestBattleNameTableError> {
    let analysis =
        audit_quest_battle_name_table(map, text, code_table, QUEST_BATTLE_NAME_SELECTOR_RUNTIME)?;
    let selector_offset = map
        .text_offset(QUEST_BATTLE_NAME_SELECTOR_RUNTIME)
        .expect("audit에서 selector 범위를 확인함");
    if text.get(selector_offset..selector_offset + 4) != Some(&[0x09, 0x34, 0x39, 0x00])
        || analysis.selector_field_offset != QUEST_BATTLE_NAME_SELECTOR_FIELD_OFFSET
        || analysis.entries.len() != BOX_QUEST_BATTLE_NAME_ENTRIES
    {
        return Err(error(format!(
            "QUEST battle name selector/분모 drift: selector={:02X?}, field=0x{:02X}, entries={}",
            text.get(selector_offset..selector_offset + 4),
            analysis.selector_field_offset,
            analysis.entries.len(),
        )));
    }
    let actual = analysis
        .entries
        .iter()
        .map(|entry| entry.source.as_str())
        .collect::<Vec<_>>();
    if actual != BOX_QUEST_BATTLE_NAMES {
        return Err(error(format!(
            "QUEST battle name 보호 원문/순서 drift: {actual:?}"
        )));
    }
    Ok(analysis)
}

fn hex_bytes(raw: &[u8]) -> String {
    raw.iter().map(|byte| format!("{byte:02X}")).collect()
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
}

/// 원본 이름과 각 물리 slot의 주소·용량·digest를 순서대로 내보낸다.
///
/// 이 보고서는 번역·표시 승인을 뜻하지 않는다. 59개 물리 entry를 모두
/// 남겨 같은 원문이 여러 slot에 있는 경우에도 최종 writer가 각 쓰기를
/// source-bound로 검증할 수 있게 한다.
pub fn render_quest_battle_name_table_tsv(analysis: &QuestBattleNameTableAnalysis) -> String {
    let mut output = String::from(
        "unit_id\tindex\tsource\tsource_raw_hex\tprotected_slot_sha256\t\
         branch_pointer_runtime\tstream_runtime\tname_assignment_runtime\tname_runtime\t\
         slot_end_assignment_runtime\tslot_end_runtime\tprotected_slot_bytes\t\
         slot_capacity_glyphs\tglyph_occurrences\tfixed_text_state_runtime\tevidence\n",
    );
    for entry in &analysis.entries {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t\
             0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t0x{:08X}\t\
             source-consumer-bound;runtime-population-not-exhaustive",
            entry.id,
            entry.index,
            escape_tsv(&entry.source),
            hex_bytes(&entry.source_raw),
            hex_bytes(&entry.protected_slot_sha256),
            entry.branch_pointer_runtime,
            entry.stream_runtime,
            entry.name_assignment_runtime,
            entry.name_runtime,
            entry.slot_end_assignment_runtime,
            entry.slot_end_runtime,
            entry.protected_slot.len(),
            entry.slot_capacity_glyphs,
            entry.glyph_occurrences,
            analysis.fixed_text_state_runtime,
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_battle_name_table_tests.rs"]
mod tests;
