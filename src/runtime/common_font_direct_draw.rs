//! QUEST `COM_FONT.BTX` chain label의 직접 primitive command 관측.
//!
//! 실제 연쇄 object의 cell/resource writer, callback handoff와 command count
//! commit을 같은 RAM object·cell record·8바이트 command slot으로 결속한다.
//! 화면 PNG는 관측 장면에 귀속하지만 최종 framebuffer pixel attribution은
//! 별도 증거가 없으므로 주장하지 않는다.

use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use serde_json::Value;

use crate::coords::{psx_main_ram_offset, PSX_MAIN_RAM_LEN};
use crate::disasm::CommonFontDirectDrawSite;
use crate::font::{parse_runtime_cell_table, RuntimeCell, RuntimeCellTableError};
use crate::isa::Reg;

use super::common_font_draw::{digest, event_register, event_u32, png_shape_exact};

pub const COMMON_FONT_DIRECT_DRAW_SCHEMA_VERSION: &str = "common-font-direct-draw-runtime-v1";

const CELL_TABLE_HEADER_BYTES: u32 = 12;
const CELL_RECORD_BYTES: u32 = 12;
const COMMAND_BYTES: usize = 8;
const OBJECT_DRAW_FLAGS_OFFSET: u32 = 0x006E;

/// 실제 chain label object가 만든 direct draw command의 검증 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontDirectDrawReport {
    pub schema_version: &'static str,
    pub exe: &'static str,
    pub archive: &'static str,
    pub surface_id: &'static str,
    pub source_text: &'static str,
    pub callback_source_sha256: String,
    pub ram_sha256: String,
    pub writer_event_sha256: String,
    pub post_store_event_sha256: String,
    pub handoff_event_sha256: String,
    pub commit_event_sha256: String,
    pub screen_sha256: String,
    pub descriptor_runtime: u32,
    pub descriptor_root_runtime: u32,
    pub table_runtime: u32,
    pub object_runtime: u32,
    pub cell_record_runtime: u32,
    pub resource: u16,
    pub cell: RuntimeCell,
    pub command_buffer_runtime: u32,
    pub command_count_runtime: u32,
    pub command_slot_runtime: u32,
    pub previous_command_count: u32,
    pub committed_command_count: u32,
    pub command_x: u16,
    pub command_y: u16,
    pub command_cell: u16,
    pub command_reserved: u8,
    pub command_flags: u8,
    pub object_fields_verified: bool,
    pub descriptor_chain_verified: bool,
    pub command_verified: bool,
    pub screen_capture_shape_exact: bool,
    pub final_pixel_attribution_verified: bool,
    pub runtime_verified: bool,
    pub release_eligible: bool,
}

/// 원본 source에서 검증한 site와 같은 실행 세대의 관측 입력.
pub struct CommonFontDirectDrawObservationInput<'a> {
    pub site: &'a CommonFontDirectDrawSite,
    pub ram: &'a [u8],
    pub writer_events_json: &'a [u8],
    pub post_store_events_json: &'a [u8],
    pub handoff_events_json: &'a [u8],
    pub commit_events_json: &'a [u8],
    pub screen_png: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontDirectDrawError(String);

impl fmt::Display for CommonFontDirectDrawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for CommonFontDirectDrawError {}

impl From<RuntimeCellTableError> for CommonFontDirectDrawError {
    fn from(source: RuntimeCellTableError) -> Self {
        error(format!(
            "QUEST COM_FONT direct draw cell table 오류: {source}"
        ))
    }
}

fn error(message: impl Into<String>) -> CommonFontDirectDrawError {
    CommonFontDirectDrawError(message.into())
}

fn runtime_slice<'a>(
    ram: &'a [u8],
    runtime: u32,
    len: usize,
    label: &str,
) -> Result<&'a [u8], CommonFontDirectDrawError> {
    let offset = psx_main_ram_offset(runtime, len).ok_or_else(|| {
        error(format!(
            "QUEST COM_FONT direct draw {label} RAM 범위 이탈: 0x{runtime:08X}+{len}"
        ))
    })?;
    Ok(&ram[offset..offset + len])
}

fn runtime_u8(ram: &[u8], runtime: u32, label: &str) -> Result<u8, CommonFontDirectDrawError> {
    Ok(runtime_slice(ram, runtime, 1, label)?[0])
}

fn runtime_u16(ram: &[u8], runtime: u32, label: &str) -> Result<u16, CommonFontDirectDrawError> {
    let raw: [u8; 2] = runtime_slice(ram, runtime, 2, label)?
        .try_into()
        .expect("길이를 검증했다");
    Ok(u16::from_le_bytes(raw))
}

fn runtime_u32(ram: &[u8], runtime: u32, label: &str) -> Result<u32, CommonFontDirectDrawError> {
    let raw: [u8; 4] = runtime_slice(ram, runtime, 4, label)?
        .try_into()
        .expect("길이를 검증했다");
    Ok(u32::from_le_bytes(raw))
}

fn event_value(
    event: &Value,
    register: Reg,
    label: &str,
) -> Result<u32, CommonFontDirectDrawError> {
    event_register(event, register, label).map_err(|source| error(source.to_string()))
}

fn parse_single_event(
    raw: &[u8],
    expected_pc: u32,
    label: &str,
) -> Result<Value, CommonFontDirectDrawError> {
    let document: Value = serde_json::from_slice(raw).map_err(|source| {
        error(format!(
            "QUEST COM_FONT direct draw {label} event JSON 해석 실패: {source}"
        ))
    })?;
    let dropped = document
        .get("dropped")
        .and_then(Value::as_u64)
        .ok_or_else(|| error(format!("QUEST COM_FONT direct draw {label} dropped가 없음")))?;
    if dropped != 0 {
        return Err(error(format!(
            "QUEST COM_FONT direct draw {label} dropped={dropped}"
        )));
    }
    let events = document
        .get("events")
        .and_then(Value::as_array)
        .ok_or_else(|| error(format!("QUEST COM_FONT direct draw {label} events가 없음")))?;
    let [event] = events.as_slice() else {
        return Err(error(format!(
            "QUEST COM_FONT direct draw {label} event 수 불일치: expected=1, actual={}",
            events.len()
        )));
    };
    let pc = event_u32(event, &["pc"], label).map_err(|source| error(source.to_string()))?;
    if pc != expected_pc {
        return Err(error(format!(
            "QUEST COM_FONT direct draw {label} PC 불일치: expected=0x{expected_pc:08X}, actual=0x{pc:08X}"
        )));
    }
    Ok(event.clone())
}

fn require_equal(actual: u32, expected: u32, label: &str) -> Result<(), CommonFontDirectDrawError> {
    if actual != expected {
        return Err(error(format!(
            "QUEST COM_FONT direct draw {label} 불일치: expected=0x{expected:08X}, actual=0x{actual:08X}"
        )));
    }
    Ok(())
}

fn add_nonnegative_offset(
    base: u32,
    offset: i16,
    label: &str,
) -> Result<u32, CommonFontDirectDrawError> {
    let offset = u32::try_from(i32::from(offset)).map_err(|_| {
        error(format!(
            "QUEST COM_FONT direct draw {label} offset이 음수임: {offset}"
        ))
    })?;
    base.checked_add(offset)
        .ok_or_else(|| error(format!("QUEST COM_FONT direct draw {label} 주소 overflow")))
}

/// writer부터 command count commit까지 같은 object의 한 경로로 결속한다.
pub fn inspect_common_font_direct_draw(
    input: CommonFontDirectDrawObservationInput<'_>,
) -> Result<CommonFontDirectDrawReport, CommonFontDirectDrawError> {
    if input.ram.len() != PSX_MAIN_RAM_LEN {
        return Err(error(format!(
            "QUEST COM_FONT direct draw RAM 길이 불일치: expected={PSX_MAIN_RAM_LEN}, actual={}",
            input.ram.len()
        )));
    }

    let callback_source = runtime_slice(
        input.ram,
        input.site.function_runtime,
        input.site.source_bytes,
        "callback source",
    )?;
    let callback_source_sha256 = digest(callback_source);
    let expected_callback_sha256 = input
        .site
        .source_sha256
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if callback_source_sha256 != expected_callback_sha256 {
        return Err(error(
            "QUEST COM_FONT direct draw runtime callback이 지원 원본과 다름",
        ));
    }

    let writer = parse_single_event(
        input.writer_events_json,
        input.site.writer_runtime,
        "writer",
    )?;
    let post_store = parse_single_event(
        input.post_store_events_json,
        input.site.writer_post_runtime,
        "post-store",
    )?;
    let handoff = parse_single_event(
        input.handoff_events_json,
        input.site.handoff_runtime,
        "callback handoff",
    )?;
    let commit = parse_single_event(
        input.commit_events_json,
        input.site.commit_runtime,
        "command commit",
    )?;

    let object_runtime = event_value(&writer, Reg::A2, "writer object")?;
    require_equal(
        event_value(&writer, Reg::V0, "writer cell")?,
        u32::from(input.site.cell),
        "writer cell",
    )?;
    require_equal(
        event_value(&writer, Reg::S2, "writer resource")?,
        u32::from(input.site.resource),
        "writer resource",
    )?;
    require_equal(
        event_value(&post_store, Reg::A2, "post-store object")?,
        object_runtime,
        "post-store object",
    )?;
    require_equal(
        event_value(&post_store, Reg::V0, "post-store cell")?,
        u32::from(input.site.cell),
        "post-store cell",
    )?;
    require_equal(
        event_value(&post_store, Reg::S2, "post-store resource")?,
        u32::from(input.site.resource),
        "post-store resource",
    )?;

    let cell_field_runtime =
        add_nonnegative_offset(object_runtime, input.site.cell_field_offset, "object cell")?;
    let resource_field_runtime = add_nonnegative_offset(
        object_runtime,
        input.site.resource_field_offset,
        "object resource",
    )?;
    let callback_field_runtime = add_nonnegative_offset(
        object_runtime,
        input.site.callback_field_offset,
        "object callback",
    )?;
    if runtime_u16(input.ram, cell_field_runtime, "object cell")? != input.site.cell
        || runtime_u16(input.ram, resource_field_runtime, "object resource")? != input.site.resource
    {
        return Err(error(
            "QUEST COM_FONT direct draw object의 종단 cell/resource 불일치",
        ));
    }
    require_equal(
        runtime_u32(input.ram, callback_field_runtime, "object callback")?,
        input.site.function_runtime,
        "object callback",
    )?;

    require_equal(
        event_value(&handoff, Reg::A0, "callback object argument")?,
        object_runtime,
        "callback object argument",
    )?;
    require_equal(
        event_value(&handoff, Reg::T0, "callback object handoff")?,
        object_runtime,
        "callback object handoff",
    )?;

    let descriptor_root_runtime = runtime_u32(
        input.ram,
        input.site.descriptor_root_pointer_runtime,
        "descriptor root pointer",
    )?;
    let table_pointer_runtime = descriptor_root_runtime
        .checked_add(input.site.descriptor_table_pointer_offset)
        .ok_or_else(|| error("QUEST COM_FONT direct draw table pointer 주소 overflow"))?;
    let descriptor_table_pointer_runtime = input
        .site
        .descriptor_runtime
        .checked_add(28)
        .ok_or_else(|| error("QUEST COM_FONT direct draw descriptor pointer 주소 overflow"))?;
    require_equal(
        table_pointer_runtime,
        descriptor_table_pointer_runtime,
        "descriptor table pointer field",
    )?;
    let table_from_root = runtime_u32(input.ram, table_pointer_runtime, "cell table pointer")?;
    let table = parse_runtime_cell_table(
        input.ram,
        input.site.descriptor_runtime,
        input.site.cell_count,
    )?;
    require_equal(
        table.table_runtime,
        table_from_root,
        "cell table pointer chain",
    )?;
    let cell = table
        .cells
        .get(usize::from(input.site.cell))
        .copied()
        .ok_or_else(|| error("QUEST COM_FONT direct draw target cell이 table 밖임"))?;
    let cell_record_runtime = table
        .table_runtime
        .checked_add(CELL_TABLE_HEADER_BYTES)
        .and_then(|runtime| runtime.checked_add(u32::from(input.site.cell) * CELL_RECORD_BYTES))
        .ok_or_else(|| error("QUEST COM_FONT direct draw cell record 주소 overflow"))?;

    require_equal(
        event_value(&commit, Reg::T0, "commit object")?,
        object_runtime,
        "commit object",
    )?;
    require_equal(
        event_value(&commit, Reg::A1, "commit cell")?,
        u32::from(input.site.cell),
        "commit cell",
    )?;
    require_equal(
        event_value(&commit, Reg::A2, "commit cell record")?,
        cell_record_runtime,
        "commit cell record",
    )?;
    require_equal(
        event_value(&commit, Reg::A0, "commit command buffer")?,
        input.site.command_buffer_runtime,
        "commit command buffer",
    )?;

    let gp_runtime = event_value(&commit, Reg::GP, "commit gp")?;
    let command_count_runtime = add_nonnegative_offset(
        gp_runtime,
        input.site.command_count_gp_offset,
        "command count",
    )?;
    let previous_command_count = runtime_u32(input.ram, command_count_runtime, "command count")?;
    let committed_command_count = previous_command_count
        .checked_add(1)
        .ok_or_else(|| error("QUEST COM_FONT direct draw command count overflow"))?;
    require_equal(
        event_value(&commit, Reg::V0, "committed command count")?,
        committed_command_count,
        "committed command count",
    )?;
    let command_slot_runtime = input
        .site
        .command_buffer_runtime
        .checked_add(
            previous_command_count
                .checked_mul(COMMAND_BYTES as u32)
                .ok_or_else(|| error("QUEST COM_FONT direct draw command slot stride overflow"))?,
        )
        .ok_or_else(|| error("QUEST COM_FONT direct draw command slot 주소 overflow"))?;
    require_equal(
        event_value(&commit, Reg::V1, "commit command slot")?,
        command_slot_runtime,
        "commit command slot",
    )?;

    let command = runtime_slice(
        input.ram,
        command_slot_runtime,
        COMMAND_BYTES,
        "command slot",
    )?;
    let command_x = u16::from_le_bytes(command[0..2].try_into().expect("길이를 검증했다"));
    let command_y = u16::from_le_bytes(command[2..4].try_into().expect("길이를 검증했다"));
    let command_cell = u16::from_le_bytes(command[4..6].try_into().expect("길이를 검증했다"));
    let command_reserved = command[6];
    let command_flags = command[7];
    if command_cell != input.site.cell || command_reserved != 0 {
        return Err(error(format!(
            "QUEST COM_FONT direct draw command cell/reserved 불일치: cell={command_cell}, reserved={command_reserved}"
        )));
    }
    let object_draw_flags_runtime = object_runtime
        .checked_add(OBJECT_DRAW_FLAGS_OFFSET)
        .ok_or_else(|| error("QUEST COM_FONT direct draw object flags 주소 overflow"))?;
    let expected_flags = runtime_u8(input.ram, object_draw_flags_runtime, "object draw flags")? & 7;
    if command_flags != expected_flags {
        return Err(error(format!(
            "QUEST COM_FONT direct draw command flags 불일치: expected={expected_flags}, actual={command_flags}"
        )));
    }
    require_equal(
        u32::from(command_y),
        event_value(&commit, Reg::A3, "commit command y")? & 0xFFFF,
        "command y",
    )?;

    let screen_capture_shape_exact = png_shape_exact(input.screen_png);
    if !screen_capture_shape_exact {
        return Err(error("QUEST COM_FONT direct draw 화면 PNG shape 불일치"));
    }

    Ok(CommonFontDirectDrawReport {
        schema_version: COMMON_FONT_DIRECT_DRAW_SCHEMA_VERSION,
        exe: input.site.exe,
        archive: input.site.archive,
        surface_id: input.site.surface_id,
        source_text: input.site.source_text,
        callback_source_sha256,
        ram_sha256: digest(input.ram),
        writer_event_sha256: digest(input.writer_events_json),
        post_store_event_sha256: digest(input.post_store_events_json),
        handoff_event_sha256: digest(input.handoff_events_json),
        commit_event_sha256: digest(input.commit_events_json),
        screen_sha256: digest(input.screen_png),
        descriptor_runtime: input.site.descriptor_runtime,
        descriptor_root_runtime,
        table_runtime: table.table_runtime,
        object_runtime,
        cell_record_runtime,
        resource: input.site.resource,
        cell,
        command_buffer_runtime: input.site.command_buffer_runtime,
        command_count_runtime,
        command_slot_runtime,
        previous_command_count,
        committed_command_count,
        command_x,
        command_y,
        command_cell,
        command_reserved,
        command_flags,
        object_fields_verified: true,
        descriptor_chain_verified: true,
        command_verified: true,
        screen_capture_shape_exact,
        final_pixel_attribution_verified: false,
        runtime_verified: true,
        release_eligible: false,
    })
}

/// direct draw 한 건을 원본·event 해시와 command field가 있는 TSV로 만든다.
pub fn render_common_font_direct_draw_tsv(report: &CommonFontDirectDrawReport) -> String {
    let mut output = String::from(
        "schema_version\texe\tarchive\tsurface_id\tsource_text\tcallback_source_sha256\t\
         ram_sha256\twriter_event_sha256\tpost_store_event_sha256\thandoff_event_sha256\t\
         commit_event_sha256\tscreen_sha256\tdescriptor_runtime\tdescriptor_root_runtime\t\
         table_runtime\tobject_runtime\tcell_record_runtime\tresource\tcell\tcell_x\tcell_y\t\
         cell_width\tcell_height\tcommand_buffer_runtime\tcommand_count_runtime\t\
         command_slot_runtime\tprevious_command_count\tcommitted_command_count\tcommand_x\t\
         command_y\tcommand_cell\tcommand_reserved\tcommand_flags\tobject_fields_verified\t\
         descriptor_chain_verified\tcommand_verified\tscreen_capture_shape_exact\t\
         final_pixel_attribution_verified\truntime_verified\trelease_eligible\n",
    );
    writeln!(
        output,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t\
         0x{:08X}\t0x{:08X}\t0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t0x{:08X}\t0x{:08X}\t\
         0x{:08X}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        report.schema_version,
        report.exe,
        report.archive,
        report.surface_id,
        report.source_text.replace('\n', "\\n"),
        report.callback_source_sha256,
        report.ram_sha256,
        report.writer_event_sha256,
        report.post_store_event_sha256,
        report.handoff_event_sha256,
        report.commit_event_sha256,
        report.screen_sha256,
        report.descriptor_runtime,
        report.descriptor_root_runtime,
        report.table_runtime,
        report.object_runtime,
        report.cell_record_runtime,
        report.resource,
        report.cell.index,
        report.cell.x,
        report.cell.y,
        report.cell.width,
        report.cell.height,
        report.command_buffer_runtime,
        report.command_count_runtime,
        report.command_slot_runtime,
        report.previous_command_count,
        report.committed_command_count,
        report.command_x,
        report.command_y,
        report.command_cell,
        report.command_reserved,
        report.command_flags,
        report.object_fields_verified,
        report.descriptor_chain_verified,
        report.command_verified,
        report.screen_capture_shape_exact,
        report.final_pixel_attribution_verified,
        report.runtime_verified,
        report.release_eligible,
    )
    .expect("String 쓰기는 실패하지 않는다");
    output
}

#[cfg(test)]
#[path = "common_font_direct_draw_tests.rs"]
mod common_font_direct_draw_tests;
