//! QUEST `OP83` child actor의 위치·접촉 영역 record.
//!
//! `OP82`가 만든 child는 parent의 `+0x34`에 남고, 뒤따르는 `OP83`
//! subtype 0/3/2가 각각 절대 위치, 상대 이동, 접촉 사각형을 설정한다.
//! 이 모듈은 그 연속 세 record만 구조화한다. 실행 도달이나 실제 접촉은
//! 주장하지 않는다.

use std::error::Error;
use std::fmt;

pub const QUEST_SEQ_CHILD_CONFIGURATION_OPCODE: u8 = 0x83;
pub const QUEST_SEQ_CHILD_CONFIGURATION_RECORD_BYTES: usize = 12;
pub const QUEST_SEQ_CHILD_CONTACT_RECORDS_BYTES: usize =
    QUEST_SEQ_CHILD_CONFIGURATION_RECORD_BYTES * 3;

const ABSOLUTE_POSITION_SUBTYPE: u8 = 0;
const CONTACT_BOUNDS_SUBTYPE: u8 = 2;
const RELATIVE_MOTION_SUBTYPE: u8 = 3;
const FIXED_POINT_SHIFT: u32 = 12;

/// subtype 0/3/2 연속 record가 만드는 child actor의 정적 위치와 접촉 영역.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqChildContact {
    pub anchor_x_fixed: u32,
    pub anchor_y_fixed: u32,
    pub anchor_x_pixels: i32,
    pub anchor_y_pixels: i32,
    pub layer: u16,
    pub velocity_x_fixed: i32,
    pub velocity_y_fixed: i32,
    pub bounds_left: i16,
    pub bounds_top: i16,
    pub bounds_width: i16,
    pub bounds_height: i16,
    pub contact_left_pixels: i32,
    pub contact_top_pixels: i32,
    pub contact_right_pixels: i32,
    pub contact_bottom_pixels: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestSeqChildContactError(String);

impl fmt::Display for QuestSeqChildContactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for QuestSeqChildContactError {}

fn error(message: impl Into<String>) -> QuestSeqChildContactError {
    QuestSeqChildContactError(message.into())
}

fn record(
    raw: &[u8],
    index: usize,
    expected_subtype: u8,
) -> Result<&[u8], QuestSeqChildContactError> {
    let start = index
        .checked_mul(QUEST_SEQ_CHILD_CONFIGURATION_RECORD_BYTES)
        .ok_or_else(|| error("QUEST OP83 record offset overflow"))?;
    let end = start
        .checked_add(QUEST_SEQ_CHILD_CONFIGURATION_RECORD_BYTES)
        .ok_or_else(|| error("QUEST OP83 record 끝 offset overflow"))?;
    let record = raw
        .get(start..end)
        .ok_or_else(|| error(format!("QUEST OP83 record {index}가 잘림")))?;
    if record[0] != QUEST_SEQ_CHILD_CONFIGURATION_OPCODE || record[1] != expected_subtype {
        return Err(error(format!(
            "QUEST OP83 record {index} opcode/subtype 불일치: \
             expected=83/{expected_subtype:02X}, actual={:02X}/{:02X}",
            record[0], record[1]
        )));
    }
    Ok(record)
}

fn read_u16(record: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        record[offset..offset + 2]
            .try_into()
            .expect("12바이트 OP83 record 안의 halfword"),
    )
}

fn read_i16(record: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes(
        record[offset..offset + 2]
            .try_into()
            .expect("12바이트 OP83 record 안의 signed halfword"),
    )
}

fn absolute_coordinate_fixed(value: u16) -> u32 {
    u32::from(value)
        .wrapping_shl(4)
        .wrapping_add(16)
        .wrapping_shl(FIXED_POINT_SHIFT)
}

fn add_relative_fixed(absolute: u32, relative: i16) -> u32 {
    absolute.wrapping_add((i32::from(relative) << FIXED_POINT_SHIFT) as u32)
}

fn fixed_to_pixels(value: u32) -> i32 {
    (value as i32) >> FIXED_POINT_SHIFT
}

/// 연속된 `OP83` subtype 0/3/2 record를 child 접촉 영역으로 해석한다.
pub fn parse_quest_seq_child_contact(
    raw: &[u8],
) -> Result<QuestSeqChildContact, QuestSeqChildContactError> {
    if raw.len() != QUEST_SEQ_CHILD_CONTACT_RECORDS_BYTES {
        return Err(error(format!(
            "QUEST child contact record 길이 불일치: expected={}, actual={}",
            QUEST_SEQ_CHILD_CONTACT_RECORDS_BYTES,
            raw.len()
        )));
    }

    let position = record(raw, 0, ABSOLUTE_POSITION_SUBTYPE)?;
    let motion = record(raw, 1, RELATIVE_MOTION_SUBTYPE)?;
    let bounds = record(raw, 2, CONTACT_BOUNDS_SUBTYPE)?;

    let anchor_x_fixed = add_relative_fixed(
        absolute_coordinate_fixed(read_u16(position, 2)),
        read_i16(motion, 2),
    );
    let anchor_y_fixed = add_relative_fixed(
        absolute_coordinate_fixed(read_u16(position, 4)),
        read_i16(motion, 4),
    );
    let anchor_x_pixels = fixed_to_pixels(anchor_x_fixed);
    let anchor_y_pixels = fixed_to_pixels(anchor_y_fixed);
    let bounds_left = read_i16(bounds, 2);
    let bounds_top = read_i16(bounds, 4);
    let bounds_width = read_i16(bounds, 6);
    let bounds_height = read_i16(bounds, 8);
    if bounds_width < 0 || bounds_height < 0 {
        return Err(error("QUEST child contact 폭·높이가 음수임"));
    }
    let contact_left_pixels = anchor_x_pixels.wrapping_add(i32::from(bounds_left));
    let contact_top_pixels = anchor_y_pixels.wrapping_add(i32::from(bounds_top));
    let contact_right_pixels = contact_left_pixels.wrapping_add(i32::from(bounds_width));
    let contact_bottom_pixels = contact_top_pixels.wrapping_add(i32::from(bounds_height));

    Ok(QuestSeqChildContact {
        anchor_x_fixed,
        anchor_y_fixed,
        anchor_x_pixels,
        anchor_y_pixels,
        layer: read_u16(position, 6),
        velocity_x_fixed: i32::from(read_i16(motion, 6)) << FIXED_POINT_SHIFT,
        velocity_y_fixed: i32::from(read_i16(motion, 8)) << FIXED_POINT_SHIFT,
        bounds_left,
        bounds_top,
        bounds_width,
        bounds_height,
        contact_left_pixels,
        contact_top_pixels,
        contact_right_pixels,
        contact_bottom_pixels,
    })
}

#[cfg(test)]
#[path = "quest_seq_child_contact_tests.rs"]
mod tests;
