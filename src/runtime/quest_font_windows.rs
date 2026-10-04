//! QUEST text window state와 parent geometry의 exact runtime census.
//!
//! 장면별 font page 교체의 필요조건은 살아 있는 text window가 0개인
//! 것이다. 원본에는 이 조건을 대신하는 단일 count가 확인되지 않았으므로
//! 10개 state를 전수 읽는다. parent가 연결된 칸은 `FF07`·`FF08`의 실제
//! 소비 필드와 수동 조판에 필요한 cursor·cell·spacing 상태도 함께 읽는다.
//! 이 스냅샷만으로 target 장면, portrait 경계나 layout 승인을 주장하지 않는다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use crate::coords::{psx_main_ram_offset, PSX_MAIN_RAM_LEN};
use crate::disasm::{
    QUEST_FONT_STATE_BASE_RUNTIME, QUEST_FONT_STATE_RECORD_COUNT, QUEST_FONT_STATE_RECORD_STRIDE,
    QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET, QUEST_TEXT_WINDOW_CELL_STATE_OFFSET,
    QUEST_TEXT_WINDOW_FRAME_OUTSET_PX, QUEST_TEXT_WINDOW_PARENT_HEIGHT_OFFSET,
    QUEST_TEXT_WINDOW_PARENT_STATE_OFFSET, QUEST_TEXT_WINDOW_PARENT_WIDTH_OFFSET,
    QUEST_TEXT_WINDOW_PARENT_X_FIXED12_OFFSET, QUEST_TEXT_WINDOW_PARENT_Y_FIXED12_OFFSET,
    QUEST_TEXT_WINDOW_POSITION_FRACTION_BITS, QUEST_TEXT_WINDOW_RESOURCE_STATE_OFFSET,
};

use super::{MednafenPsxState, MednafenStateError};

pub const QUEST_FONT_WINDOW_ORIGIN_X_OFFSET: u32 = 0x06;
pub const QUEST_FONT_WINDOW_ORIGIN_Y_OFFSET: u32 = 0x08;
pub const QUEST_FONT_WINDOW_CURSOR_X_OFFSET: u32 = 0x0A;
pub const QUEST_FONT_WINDOW_CURSOR_Y_OFFSET: u32 = 0x0C;
pub const QUEST_FONT_WINDOW_CELL_WIDTH_OFFSET: u32 = 0x12;
pub const QUEST_FONT_WINDOW_CELL_HEIGHT_OFFSET: u32 = 0x13;
pub const QUEST_FONT_WINDOW_LINE_HEIGHT_OFFSET: u32 = 0x14;
pub const QUEST_FONT_WINDOW_GLYPH_SPACING_OFFSET: u32 = 0x16;
pub const QUEST_FONT_WINDOW_LINE_SPACING_OFFSET: u32 = 0x17;
pub const QUEST_FONT_WINDOW_ACTIVE_OFFSET: u32 = 0x1F;
pub const QUEST_FONT_WINDOW_INPUT_INDICATOR_OFFSET: u32 = 0x64;
pub const QUEST_FONT_WINDOW_SELECTION_RENDERER_OFFSET: u32 = 0x68;
pub const QUEST_FONT_WINDOW_GEOMETRY_SCHEMA_VERSION: &str = "quest-font-window-geometry-v1";

/// parent 객체에서 읽은 draw geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestFontWindowGeometry {
    pub parent_x_fixed12: i32,
    pub parent_y_fixed12: i32,
    pub parent_x_px: i32,
    pub parent_y_px: i32,
    pub parent_width_px: i32,
    pub parent_height_px: i32,
    pub frame_origin_x_px: i32,
    pub frame_origin_y_px: i32,
}

/// runtime에서 읽은 text window state 한 칸.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestFontWindowSlot {
    pub index: usize,
    pub state_runtime: u32,
    pub parent_runtime: u32,
    pub resource_selector: u8,
    pub window_resource: u8,
    pub window_cell: u8,
    pub active: u8,
    pub origin_x: u16,
    pub origin_y: u16,
    pub cursor_x: u16,
    pub cursor_y: u16,
    pub cell_width: u8,
    pub cell_height: u8,
    pub line_height: u8,
    pub glyph_spacing: i8,
    pub line_spacing: i8,
    pub input_indicator_runtime: u32,
    pub selection_renderer_runtime: u32,
    pub geometry: Option<QuestFontWindowGeometry>,
}

/// 10개 state를 빠짐없이 읽은 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestFontWindowCensus {
    pub slots: Vec<QuestFontWindowSlot>,
    pub active_windows: usize,
    pub selection_renderer_slots: usize,
    pub geometry_slots: usize,
}

impl QuestFontWindowCensus {
    /// font page 교체의 필요조건인 무창 상태만 판정한다.
    ///
    /// fresh source, scene binding과 `LoadImage` 완료는 별도 게이트다.
    pub const fn has_no_active_windows(&self) -> bool {
        self.active_windows == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestFontWindowCensusError {
    CoordinateOverflow,
    WrongRamLength { expected: usize, actual: usize },
    RamAddressOutOfRange { runtime: u32, bytes: usize },
    State(MednafenStateError),
}

impl fmt::Display for QuestFontWindowCensusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoordinateOverflow => f.write_str("QUEST font window state 좌표 overflow"),
            Self::WrongRamLength { expected, actual } => write!(
                f,
                "QUEST font window RAM 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::RamAddressOutOfRange { runtime, bytes } => write!(
                f,
                "QUEST font window RAM 주소 범위 이탈: 0x{runtime:08X}+{bytes}B"
            ),
            Self::State(source) => write!(f, "QUEST font window state 읽기 실패: {source}"),
        }
    }
}

impl Error for QuestFontWindowCensusError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State(source) => Some(source),
            Self::CoordinateOverflow
            | Self::WrongRamLength { .. }
            | Self::RamAddressOutOfRange { .. } => None,
        }
    }
}

impl From<MednafenStateError> for QuestFontWindowCensusError {
    fn from(value: MednafenStateError) -> Self {
        Self::State(value)
    }
}

trait QuestFontMemory {
    fn read_u8(&self, runtime: u32) -> Result<u8, QuestFontWindowCensusError>;
    fn read_u16(&self, runtime: u32) -> Result<u16, QuestFontWindowCensusError>;
    fn read_u32(&self, runtime: u32) -> Result<u32, QuestFontWindowCensusError>;
}

impl QuestFontMemory for MednafenPsxState<'_> {
    fn read_u8(&self, runtime: u32) -> Result<u8, QuestFontWindowCensusError> {
        Ok(self.read_u8(runtime)?)
    }

    fn read_u16(&self, runtime: u32) -> Result<u16, QuestFontWindowCensusError> {
        Ok(self.read_u16_le(runtime)?)
    }

    fn read_u32(&self, runtime: u32) -> Result<u32, QuestFontWindowCensusError> {
        Ok(self.read_u32_le(runtime)?)
    }
}

struct RawRam<'a>(&'a [u8]);

impl RawRam<'_> {
    fn range(&self, runtime: u32, bytes: usize) -> Result<&[u8], QuestFontWindowCensusError> {
        let offset = psx_main_ram_offset(runtime, bytes)
            .ok_or(QuestFontWindowCensusError::RamAddressOutOfRange { runtime, bytes })?;
        self.0
            .get(offset..offset + bytes)
            .ok_or(QuestFontWindowCensusError::RamAddressOutOfRange { runtime, bytes })
    }
}

impl QuestFontMemory for RawRam<'_> {
    fn read_u8(&self, runtime: u32) -> Result<u8, QuestFontWindowCensusError> {
        Ok(self.range(runtime, 1)?[0])
    }

    fn read_u16(&self, runtime: u32) -> Result<u16, QuestFontWindowCensusError> {
        Ok(u16::from_le_bytes(
            self.range(runtime, 2)?
                .try_into()
                .expect("2바이트 RAM 범위를 확인함"),
        ))
    }

    fn read_u32(&self, runtime: u32) -> Result<u32, QuestFontWindowCensusError> {
        Ok(u32::from_le_bytes(
            self.range(runtime, 4)?
                .try_into()
                .expect("4바이트 RAM 범위를 확인함"),
        ))
    }
}

fn field_runtime(base: u32, offset: u32) -> Result<u32, QuestFontWindowCensusError> {
    base.checked_add(offset)
        .ok_or(QuestFontWindowCensusError::CoordinateOverflow)
}

fn signed_offset(offset: i16) -> Result<u32, QuestFontWindowCensusError> {
    u32::try_from(offset).map_err(|_| QuestFontWindowCensusError::CoordinateOverflow)
}

fn read_parent_geometry(
    memory: &impl QuestFontMemory,
    parent_runtime: u32,
) -> Result<Option<QuestFontWindowGeometry>, QuestFontWindowCensusError> {
    if parent_runtime == 0 {
        return Ok(None);
    }
    let read_i32 = |offset: i16| {
        memory
            .read_u32(field_runtime(parent_runtime, signed_offset(offset)?)?)
            .map(|value| value as i32)
    };
    let parent_x_fixed12 = read_i32(QUEST_TEXT_WINDOW_PARENT_X_FIXED12_OFFSET)?;
    let parent_y_fixed12 = read_i32(QUEST_TEXT_WINDOW_PARENT_Y_FIXED12_OFFSET)?;
    let parent_width_px = read_i32(QUEST_TEXT_WINDOW_PARENT_WIDTH_OFFSET)?;
    let parent_height_px = read_i32(QUEST_TEXT_WINDOW_PARENT_HEIGHT_OFFSET)?;
    let scale = 1i32 << QUEST_TEXT_WINDOW_POSITION_FRACTION_BITS;
    let parent_x_px = parent_x_fixed12 / scale;
    let parent_y_px = parent_y_fixed12 / scale;
    let outset = i32::from(QUEST_TEXT_WINDOW_FRAME_OUTSET_PX);
    Ok(Some(QuestFontWindowGeometry {
        parent_x_fixed12,
        parent_y_fixed12,
        parent_x_px,
        parent_y_px,
        parent_width_px,
        parent_height_px,
        frame_origin_x_px: parent_x_px - outset,
        frame_origin_y_px: parent_y_px - outset,
    }))
}

fn inspect_memory(
    memory: &impl QuestFontMemory,
) -> Result<QuestFontWindowCensus, QuestFontWindowCensusError> {
    let mut slots = Vec::with_capacity(QUEST_FONT_STATE_RECORD_COUNT);
    for index in 0..QUEST_FONT_STATE_RECORD_COUNT {
        let delta = index
            .checked_mul(QUEST_FONT_STATE_RECORD_STRIDE)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(QuestFontWindowCensusError::CoordinateOverflow)?;
        let state_runtime = QUEST_FONT_STATE_BASE_RUNTIME
            .checked_add(delta)
            .ok_or(QuestFontWindowCensusError::CoordinateOverflow)?;
        let parent_runtime = memory.read_u32(field_runtime(
            state_runtime,
            signed_offset(QUEST_TEXT_WINDOW_PARENT_STATE_OFFSET)?,
        )?)?;
        let read_u8 = |offset| memory.read_u8(field_runtime(state_runtime, offset)?);
        let read_u16 = |offset| memory.read_u16(field_runtime(state_runtime, offset)?);
        slots.push(QuestFontWindowSlot {
            index,
            state_runtime,
            parent_runtime,
            resource_selector: read_u8(signed_offset(QUEST_GLYPH_RESOURCE_SELECTOR_STATE_OFFSET)?)?,
            window_resource: read_u8(signed_offset(QUEST_TEXT_WINDOW_RESOURCE_STATE_OFFSET)?)?,
            window_cell: read_u8(signed_offset(QUEST_TEXT_WINDOW_CELL_STATE_OFFSET)?)?,
            active: read_u8(QUEST_FONT_WINDOW_ACTIVE_OFFSET)?,
            origin_x: read_u16(QUEST_FONT_WINDOW_ORIGIN_X_OFFSET)?,
            origin_y: read_u16(QUEST_FONT_WINDOW_ORIGIN_Y_OFFSET)?,
            cursor_x: read_u16(QUEST_FONT_WINDOW_CURSOR_X_OFFSET)?,
            cursor_y: read_u16(QUEST_FONT_WINDOW_CURSOR_Y_OFFSET)?,
            cell_width: read_u8(QUEST_FONT_WINDOW_CELL_WIDTH_OFFSET)?,
            cell_height: read_u8(QUEST_FONT_WINDOW_CELL_HEIGHT_OFFSET)?,
            line_height: read_u8(QUEST_FONT_WINDOW_LINE_HEIGHT_OFFSET)?,
            glyph_spacing: read_u8(QUEST_FONT_WINDOW_GLYPH_SPACING_OFFSET)? as i8,
            line_spacing: read_u8(QUEST_FONT_WINDOW_LINE_SPACING_OFFSET)? as i8,
            input_indicator_runtime: memory.read_u32(field_runtime(
                state_runtime,
                QUEST_FONT_WINDOW_INPUT_INDICATOR_OFFSET,
            )?)?,
            selection_renderer_runtime: memory.read_u32(field_runtime(
                state_runtime,
                QUEST_FONT_WINDOW_SELECTION_RENDERER_OFFSET,
            )?)?,
            geometry: read_parent_geometry(memory, parent_runtime)?,
        });
    }

    let active_windows = slots.iter().filter(|slot| slot.active != 0).count();
    let selection_renderer_slots = slots
        .iter()
        .filter(|slot| slot.selection_renderer_runtime != 0)
        .count();
    let geometry_slots = slots.iter().filter(|slot| slot.geometry.is_some()).count();
    Ok(QuestFontWindowCensus {
        slots,
        active_windows,
        selection_renderer_slots,
        geometry_slots,
    })
}

/// exact Mednafen state에서 QUEST text window 10개를 전수 읽는다.
pub fn inspect_quest_font_windows(
    state: &MednafenPsxState<'_>,
) -> Result<QuestFontWindowCensus, QuestFontWindowCensusError> {
    inspect_memory(state)
}

/// exact 2 MiB PS1 Main RAM dump에서 QUEST text window 10개를 전수 읽는다.
pub fn inspect_quest_font_windows_ram(
    ram: &[u8],
) -> Result<QuestFontWindowCensus, QuestFontWindowCensusError> {
    if ram.len() != PSX_MAIN_RAM_LEN {
        return Err(QuestFontWindowCensusError::WrongRamLength {
            expected: PSX_MAIN_RAM_LEN,
            actual: ram.len(),
        });
    }
    inspect_memory(&RawRam(ram))
}

/// source·RAM digest에 귀속한 window geometry TSV를 만든다.
pub fn render_quest_font_window_geometry_tsv(
    census: &QuestFontWindowCensus,
    source_sha256: &[u8; 32],
    ram_sha256: &[u8; 32],
) -> String {
    let source_sha256 = source_sha256
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let ram_sha256 = ram_sha256
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let mut output = String::from(
        "schema_version\tsource_sha256\tram_sha256\tindex\tstate_runtime\tparent_runtime\t\
         geometry_status\tactive\tresource_selector\twindow_resource\twindow_cell\t\
         origin_x\torigin_y\tcursor_x\tcursor_y\tcell_width\tcell_height\tline_height\t\
         glyph_spacing\tline_spacing\tinput_indicator_runtime\tselection_renderer_runtime\t\
         parent_x_fixed12\tparent_y_fixed12\tparent_x_px\tparent_y_px\tparent_width_px\t\
         parent_height_px\tframe_origin_x_px\tframe_origin_y_px\n",
    );
    for slot in &census.slots {
        let (status, geometry) = match slot.geometry {
            Some(geometry) => ("observed-parent", Some(geometry)),
            None => ("parent-not-bound", None),
        };
        let field = |value: Option<i32>| value.map(|value| value.to_string()).unwrap_or_default();
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            QUEST_FONT_WINDOW_GEOMETRY_SCHEMA_VERSION,
            source_sha256,
            ram_sha256,
            slot.index,
            slot.state_runtime,
            slot.parent_runtime,
            status,
            slot.active,
            slot.resource_selector,
            slot.window_resource,
            slot.window_cell,
            slot.origin_x,
            slot.origin_y,
            slot.cursor_x,
            slot.cursor_y,
            slot.cell_width,
            slot.cell_height,
            slot.line_height,
            slot.glyph_spacing,
            slot.line_spacing,
            slot.input_indicator_runtime,
            slot.selection_renderer_runtime,
            field(geometry.map(|value| value.parent_x_fixed12)),
            field(geometry.map(|value| value.parent_y_fixed12)),
            field(geometry.map(|value| value.parent_x_px)),
            field(geometry.map(|value| value.parent_y_px)),
            field(geometry.map(|value| value.parent_width_px)),
            field(geometry.map(|value| value.parent_height_px)),
            field(geometry.map(|value| value.frame_origin_x_px)),
            field(geometry.map(|value| value.frame_origin_y_px)),
        )
        .expect("String write는 실패하지 않음");
    }
    output
}

#[cfg(test)]
#[path = "quest_font_windows_tests.rs"]
mod tests;
