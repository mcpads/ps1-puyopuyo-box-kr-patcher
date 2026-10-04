//! DRAC E0 message record와 논리 layout을 해석한다.

use std::collections::BTreeMap;

use crate::text::decode_sjis;

use super::{
    error, SubgameSeqDracConsumerError, SubgameSeqDracMessageLayout,
    BOX_SUBGAME_SEQ_DRAC_COMMAND_OPCODE,
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct DracRecordStats {
    pub(super) header_bytes: usize,
    pub(super) body_bytes: usize,
    pub(super) glyph_tokens: usize,
    pub(super) argument_controls: usize,
    pub(super) state_controls: usize,
    pub(super) terminators: usize,
    pub(super) control_counts: BTreeMap<u8, usize>,
}

pub(super) fn parse_message_record(
    raw: &[u8],
) -> Result<DracRecordStats, SubgameSeqDracConsumerError> {
    if raw.len() < 6 {
        return Err(error(
            "DRAC message record가 4바이트 header와 종결쌍보다 짧음",
        ));
    }
    let mut stats = DracRecordStats {
        header_bytes: 4,
        body_bytes: raw.len() - 4,
        ..DracRecordStats::default()
    };
    let mut cursor = 4usize;
    while cursor < raw.len() {
        if raw.get(cursor..cursor + 2) == Some([0x65, 0x65].as_slice()) {
            if cursor + 2 != raw.len() {
                return Err(error("DRAC message 65 65 종결 뒤 trailing byte가 있음"));
            }
            stats.terminators += 1;
            *stats.control_counts.entry(0x65).or_default() += 1;
            cursor += 2;
            continue;
        }
        match raw[cursor] {
            code @ (0x71 | 0x77) => {
                raw.get(cursor + 1)
                    .ok_or_else(|| error(format!("DRAC control 0x{code:02X} 인자가 잘림")))?;
                stats.argument_controls += 1;
                *stats.control_counts.entry(code).or_default() += 1;
                cursor += 2;
            }
            code @ (0x72 | 0x73) => {
                stats.state_controls += 1;
                *stats.control_counts.entry(code).or_default() += 1;
                cursor += 1;
            }
            _ => {
                let pair = raw
                    .get(cursor..cursor + 2)
                    .ok_or_else(|| error("DRAC Shift-JIS pair가 잘림"))?;
                let decoded = decode_sjis(pair).map_err(|decode_error| {
                    error(format!("DRAC glyph decode 실패: {decode_error}"))
                })?;
                if decoded.chars().count() != 1 {
                    return Err(error("DRAC glyph pair가 한 code unit이 아님"));
                }
                stats.glyph_tokens += 1;
                cursor += 2;
            }
        }
    }
    if stats.terminators != 1 {
        return Err(error(format!(
            "DRAC message 종결쌍 수 불일치: expected=1, actual={}",
            stats.terminators
        )));
    }
    Ok(stats)
}

pub(super) fn analyze_message_layout(
    raw: &[u8],
    index: usize,
    variant: u8,
) -> Result<SubgameSeqDracMessageLayout, SubgameSeqDracConsumerError> {
    let header: [u8; 4] = raw
        .get(..4)
        .ok_or_else(|| error("DRAC layout header가 4바이트보다 짧음"))?
        .try_into()
        .expect("4바이트 slice");
    let mut cursor = 4usize;
    let mut column = 0usize;
    let mut maximum_columns = 0usize;
    let mut rows = 1usize;
    let mut line_break_pairs = 0usize;
    let mut column_advances = 0usize;
    let mut end_waits = 0usize;
    let mut current_waits = 0usize;
    let mut terminated = false;

    while cursor < raw.len() {
        if raw.get(cursor..cursor + 2) == Some([0x65, 0x65].as_slice()) {
            if cursor + 2 != raw.len() {
                return Err(error("DRAC layout 종결 뒤 trailing byte가 있음"));
            }
            terminated = true;
            cursor += 2;
            continue;
        }
        match raw[cursor] {
            0x71 => {
                raw.get(cursor + 1)
                    .ok_or_else(|| error("DRAC layout 0x71 인자가 잘림"))?;
                end_waits += 1;
                cursor += 2;
            }
            0x72 => {
                if raw.get(cursor + 1) != Some(&0x72) {
                    return Err(error("DRAC layout 0x72가 pair가 아님"));
                }
                maximum_columns = maximum_columns.max(column);
                column = 0;
                rows += 1;
                line_break_pairs += 1;
                cursor += 2;
            }
            0x73 => {
                column = column
                    .checked_add(1)
                    .ok_or_else(|| error("DRAC layout column overflow"))?;
                column_advances += 1;
                cursor += 1;
            }
            0x77 => {
                raw.get(cursor + 1)
                    .ok_or_else(|| error("DRAC layout 0x77 인자가 잘림"))?;
                current_waits += 1;
                cursor += 2;
            }
            _ => {
                let pair = raw
                    .get(cursor..cursor + 2)
                    .ok_or_else(|| error("DRAC layout glyph pair가 잘림"))?;
                let decoded = decode_sjis(pair).map_err(|decode_error| {
                    error(format!("DRAC layout glyph decode 실패: {decode_error}"))
                })?;
                if decoded.chars().count() != 1 {
                    return Err(error("DRAC layout glyph가 한 code unit이 아님"));
                }
                column = column
                    .checked_add(1)
                    .ok_or_else(|| error("DRAC layout column overflow"))?;
                cursor += 2;
            }
        }
    }
    if !terminated {
        return Err(error("DRAC layout 종결쌍이 없음"));
    }
    maximum_columns = maximum_columns.max(column);

    Ok(SubgameSeqDracMessageLayout {
        index,
        variant,
        header_x_units: header[0],
        header_y_units: header[1],
        header_columns: header[2],
        header_rows: header[3],
        derived_columns: maximum_columns,
        derived_rows: rows,
        line_break_pairs,
        column_advances,
        end_waits,
        current_waits,
    })
}

pub(super) fn parse_message_command_variant(
    raw: &[u8],
    pointer_field_offset: usize,
) -> Result<u8, SubgameSeqDracConsumerError> {
    let command_offset = pointer_field_offset
        .checked_sub(4)
        .ok_or_else(|| error("DRAC message pointer 앞 command offset underflow"))?;
    let command = raw
        .get(command_offset..pointer_field_offset)
        .ok_or_else(|| error("DRAC message command 범위 이탈"))?;
    let [opcode, variant, reserved_0, reserved_1] = command else {
        return Err(error("DRAC message command 길이 불일치"));
    };
    if *opcode != BOX_SUBGAME_SEQ_DRAC_COMMAND_OPCODE
        || !matches!(*variant, 0 | 1)
        || *reserved_0 != 0
        || *reserved_1 != 0
    {
        return Err(error(format!(
            "DRAC message command 원본 불일치: {:02X?}",
            command
        )));
    }
    Ok(*variant)
}
