//! DRAC message record의 header·본문 token 소비 경계.
//!
//! `script::subgame_seq_drac_text`가 고정한 5개 record를 PUYO1/PUYO2의
//! 독립 코드와 연결한다. 다섯 pointer 앞 record는 `E0 <variant> 00 00`이고,
//! EXE별 `E0..FF` 등록 table의 `E0` slot이 message consumer를 가리킨다.
//! 각 message의 4바이트 header는 `-1,-1,+1,+1`로 변환돼 4인자 함수로
//! 전달되고, 본문은 `65 65`까지 복사된 뒤 byte token table로 바뀐다. 이
//! 모듈은 실제로 관측된 `65/71/72/73/77`의 dispatch와 message object field
//! 효과, header·logical layout·glyph object 좌표식을 typed MIPS로 검증한다.
//! 실제 화면에서의 좌표 단위·시간 단위, variant의 연출 의미와 정상
//! 실행·화면 표시는 여전히 runtime 판정이다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::{SubgameSeqDracTextAnalysis, SubgameSeqDracTextKind};

use super::linear;

mod command_dispatch;
mod handler_effects;
mod layout_dataflow;
mod message_records;
mod record_loader;
mod report;
mod token_dispatch;

use command_dispatch::verify_command_dispatch;
use handler_effects::{verify_forbidden_state_handlers, verify_observed_handler_effects};
use layout_dataflow::verify_layout_dataflow;
use message_records::{
    analyze_message_layout, parse_message_command_variant, parse_message_record, DracRecordStats,
};
use record_loader::verify_record_loader;
pub use report::render_subgame_seq_drac_consumers_tsv;
use token_dispatch::verify_token_dispatch;

pub const SUBGAME_SEQ_DRAC_CONSUMER_SCHEMA_VERSION: &str = "subgame-seq-drac-consumers-v5";
pub const BOX_SUBGAME_SEQ_DRAC_CONSUMER_ROWS: usize = 2;
pub const BOX_SUBGAME_SEQ_DRAC_RECORDS_PER_EXE: usize = 5;
pub const BOX_SUBGAME_SEQ_DRAC_HEADER_BYTES_PER_EXE: usize = 20;
pub const BOX_SUBGAME_SEQ_DRAC_BODY_BYTES_PER_EXE: usize = 220;
pub const BOX_SUBGAME_SEQ_DRAC_GLYPH_TOKENS_PER_EXE: usize = 89;
pub const BOX_SUBGAME_SEQ_DRAC_ARGUMENT_CONTROLS_PER_EXE: usize = 9;
pub const BOX_SUBGAME_SEQ_DRAC_STATE_CONTROLS_PER_EXE: usize = 14;
pub const BOX_SUBGAME_SEQ_DRAC_TERMINATORS_PER_EXE: usize = 5;
pub const BOX_SUBGAME_SEQ_DRAC_COMMAND_TABLE_ENTRIES: usize = 32;
pub const BOX_SUBGAME_SEQ_DRAC_COMMAND_OPCODE: u8 = 0xE0;
pub const BOX_SUBGAME_SEQ_DRAC_TOKEN_FIRST: u8 = 0x63;
pub const BOX_SUBGAME_SEQ_DRAC_ARGUMENT_TOKENS: [u8; 3] = [0x71, 0x74, 0x77];
pub const BOX_SUBGAME_SEQ_DRAC_VARIANT_RECORDS_PER_EXE: usize = 1;
pub const BOX_SUBGAME_SEQ_DRAC_LAYOUT_MATCHES_PER_EXE: usize = 5;
pub const BOX_SUBGAME_SEQ_DRAC_LINE_BREAK_PAIRS_PER_EXE: usize = 6;
pub const BOX_SUBGAME_SEQ_DRAC_COLUMN_ADVANCES_PER_EXE: usize = 2;
pub const BOX_SUBGAME_SEQ_DRAC_END_WAITS_PER_EXE: usize = 2;
pub const BOX_SUBGAME_SEQ_DRAC_CURRENT_WAITS_PER_EXE: usize = 7;
pub const BOX_SUBGAME_SEQ_DRAC_HEADER_PROFILES: [[u8; 4]; 5] = [
    [0x1A, 0x0F, 0x04, 0x01],
    [0x12, 0x09, 0x09, 0x03],
    [0x06, 0x0C, 0x0A, 0x02],
    [0x0C, 0x0C, 0x0C, 0x02],
    [0x12, 0x09, 0x09, 0x03],
];

/// 지원 원본에서 생성한 2행 TSV의 SHA-256.
pub const BOX_SUBGAME_SEQ_DRAC_CONSUMER_REPORT_SHA256: [u8; 32] = [
    0x1C, 0xAA, 0xB2, 0xB0, 0xD7, 0xC7, 0x11, 0x36, 0x96, 0x34, 0x52, 0x45, 0xF7, 0xE5, 0xD3, 0xFB,
    0x58, 0xCD, 0x3A, 0xCA, 0xC1, 0x24, 0xC0, 0x3F, 0x22, 0x74, 0x5D, 0x9A, 0x09, 0xEF, 0x29, 0x24,
];

const OBSERVED_CONTROL_COUNTS: [(u8, usize); 5] =
    [(0x65, 5), (0x71, 2), (0x72, 12), (0x73, 2), (0x77, 7)];

const PUYO1_DISPATCH_TARGETS: [u32; 22] = [
    0x8001_77A8,
    0x8001_77F0,
    0x8001_7798,
    0x8001_77F0,
    0x8001_77F0,
    0x8001_77F0,
    0x8001_77F0,
    0x8001_77F0,
    0x8001_77F0,
    0x8001_77F0,
    0x8001_77F0,
    0x8001_77F0,
    0x8001_77F0,
    0x8001_77F0,
    0x8001_7790,
    0x8001_776C,
    0x8001_77D8,
    0x8001_7780,
    0x8001_77F0,
    0x8001_77F0,
    0x8001_7788,
    0x8001_77C4,
];

const PUYO2_DISPATCH_TARGETS: [u32; 21] = [
    0x8001_6100,
    0x8001_6150,
    0x8001_60F0,
    0x8001_6150,
    0x8001_6150,
    0x8001_6150,
    0x8001_6150,
    0x8001_6150,
    0x8001_6150,
    0x8001_6150,
    0x8001_611C,
    0x8001_6150,
    0x8001_6150,
    0x8001_6150,
    0x8001_60E8,
    0x8001_60C4,
    0x8001_6138,
    0x8001_60D8,
    0x8001_6150,
    0x8001_6150,
    0x8001_60E0,
];

const PUYO1_MESSAGE_ALLOCATION_SITES: [(u32, u32); 2] =
    [(0x8001_8054, 0x8001_805C), (0x8001_8A24, 0x8001_8A30)];
const PUYO2_MESSAGE_ALLOCATION_SITES: [(u32, u32); 2] =
    [(0x8001_6978, 0x8001_6980), (0x8001_7474, 0x8001_7480)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqDracConsumerSpec {
    pub scope: &'static str,
    pub exe: &'static str,
    pub current_record_load_runtime: u32,
    pub message_pointer_load_runtime: u32,
    pub header_load_runtime: u32,
    pub header_call_runtime: u32,
    pub header_consumer_runtime: u32,
    pub body_first_load_runtime: u32,
    pub tokenizer_call_runtime: u32,
    pub tokenizer_runtime: u32,
    pub token_cursor_load_runtime: u32,
    pub token_table_runtime: u32,
    pub dispatch_table_runtime: u32,
    pub dispatch_targets: &'static [u32],
    pub token_63_global_gp_offset: i16,
    pub token_78_handler_runtime: Option<u32>,
    pub end_handler_runtime: u32,
    pub end_value_store_runtime: u32,
    pub default_value_handler_runtime: u32,
    pub line_break_handler_runtime: u32,
    pub column_advance_handler_runtime: u32,
    pub current_value_handler_runtime: u32,
    pub variant_state_runtime: u32,
    pub variant_store_runtime: u32,
    pub header_state_store_runtime: u32,
    pub header_width_gp_offset: i16,
    pub header_rows_gp_offset: i16,
    pub header_x_gp_offset: i16,
    pub header_y_gp_offset: i16,
    pub glyph_handler_runtime: u32,
    pub glyph_header_x_load_runtime: u32,
    pub glyph_column_load_runtime: u32,
    pub glyph_header_y_load_runtime: u32,
    pub glyph_x_scale_runtime: u32,
    pub glyph_x_store_runtime: u32,
    pub glyph_row_load_runtime: u32,
    pub glyph_row_scale_runtime: u32,
    pub glyph_y_store_runtime: u32,
    pub glyph_column_reg: Reg,
    pub glyph_header_y_reg: Reg,
    pub glyph_row_reg: Reg,
    pub message_update_runtime: u32,
    pub message_allocator_runtime: u32,
    pub message_allocator_callback_store_runtime: u32,
    pub message_allocation_sites: &'static [(u32, u32)],
    pub command_registration_runtime: u32,
    pub command_registration_call_runtime: u32,
    pub command_registration_target_runtime: u32,
    pub command_copy_words_runtime: u32,
    pub command_source_table_runtime: u32,
    pub command_dispatcher_runtime: u32,
    pub command_dispatch_table_runtime: u32,
    pub message_handler_runtime: u32,
}

pub const BOX_SUBGAME_SEQ_DRAC_CONSUMER_SPECS: [SubgameSeqDracConsumerSpec; 2] = [
    SubgameSeqDracConsumerSpec {
        scope: "PUYO1",
        exe: "PUYO1.EXE",
        current_record_load_runtime: 0x8001_7F68,
        message_pointer_load_runtime: 0x8001_7F70,
        header_load_runtime: 0x8001_7F78,
        header_call_runtime: 0x8001_7F94,
        header_consumer_runtime: 0x8001_7B24,
        body_first_load_runtime: 0x8001_7FAC,
        tokenizer_call_runtime: 0x8001_804C,
        tokenizer_runtime: 0x8001_78E4,
        token_cursor_load_runtime: 0x8001_76E0,
        token_table_runtime: 0x800B_3000,
        dispatch_table_runtime: 0x8001_02FC,
        dispatch_targets: &PUYO1_DISPATCH_TARGETS,
        token_63_global_gp_offset: 0x0408,
        token_78_handler_runtime: Some(0x8001_77C4),
        end_handler_runtime: 0x8001_7798,
        end_value_store_runtime: 0x8001_77C0,
        default_value_handler_runtime: 0x8001_7790,
        line_break_handler_runtime: 0x8001_776C,
        column_advance_handler_runtime: 0x8001_77D8,
        current_value_handler_runtime: 0x8001_7788,
        variant_state_runtime: 0x800B_47CC,
        variant_store_runtime: 0x8001_7F64,
        header_state_store_runtime: 0x8001_7F9C,
        header_width_gp_offset: 0x0404,
        header_rows_gp_offset: 0x040C,
        header_x_gp_offset: 0x0410,
        header_y_gp_offset: 0x0414,
        glyph_handler_runtime: 0x8001_77F0,
        glyph_header_x_load_runtime: 0x8001_7810,
        glyph_column_load_runtime: 0x8001_7814,
        glyph_header_y_load_runtime: 0x8001_7818,
        glyph_x_scale_runtime: 0x8001_781C,
        glyph_x_store_runtime: 0x8001_782C,
        glyph_row_load_runtime: 0x8001_7830,
        glyph_row_scale_runtime: 0x8001_7850,
        glyph_y_store_runtime: 0x8001_7868,
        glyph_column_reg: Reg::A1,
        glyph_header_y_reg: Reg::A2,
        glyph_row_reg: Reg::A1,
        message_update_runtime: 0x8001_764C,
        message_allocator_runtime: 0x8001_53EC,
        message_allocator_callback_store_runtime: 0x8001_5408,
        message_allocation_sites: &PUYO1_MESSAGE_ALLOCATION_SITES,
        command_registration_runtime: 0x8001_880C,
        command_registration_call_runtime: 0x8001_8820,
        command_registration_target_runtime: 0x8002_3840,
        command_copy_words_runtime: 0x8006_3FDC,
        command_source_table_runtime: 0x8008_9310,
        command_dispatcher_runtime: 0x8002_37D4,
        command_dispatch_table_runtime: 0x8008_96F0,
        message_handler_runtime: 0x8001_7F2C,
    },
    SubgameSeqDracConsumerSpec {
        scope: "PUYO2",
        exe: "PUYO2.EXE",
        current_record_load_runtime: 0x8001_688C,
        message_pointer_load_runtime: 0x8001_6894,
        header_load_runtime: 0x8001_689C,
        header_call_runtime: 0x8001_68B8,
        header_consumer_runtime: 0x8001_6448,
        body_first_load_runtime: 0x8001_68D0,
        tokenizer_call_runtime: 0x8001_6970,
        tokenizer_runtime: 0x8001_6210,
        token_cursor_load_runtime: 0x8001_6038,
        token_table_runtime: 0x800B_53E0,
        dispatch_table_runtime: 0x8001_0324,
        dispatch_targets: &PUYO2_DISPATCH_TARGETS,
        token_63_global_gp_offset: 0x0414,
        token_78_handler_runtime: None,
        end_handler_runtime: 0x8001_60F0,
        end_value_store_runtime: 0x8001_6118,
        default_value_handler_runtime: 0x8001_60E8,
        line_break_handler_runtime: 0x8001_60C4,
        column_advance_handler_runtime: 0x8001_6138,
        current_value_handler_runtime: 0x8001_60E0,
        variant_state_runtime: 0x800B_6BAC,
        variant_store_runtime: 0x8001_6888,
        header_state_store_runtime: 0x8001_68C0,
        header_width_gp_offset: 0x0410,
        header_rows_gp_offset: 0x0418,
        header_x_gp_offset: 0x041C,
        header_y_gp_offset: 0x0420,
        glyph_handler_runtime: 0x8001_6150,
        glyph_header_x_load_runtime: 0x8001_6160,
        glyph_column_load_runtime: 0x8001_616C,
        glyph_header_y_load_runtime: 0x8001_6170,
        glyph_x_scale_runtime: 0x8001_6174,
        glyph_x_store_runtime: 0x8001_6188,
        glyph_row_load_runtime: 0x8001_6190,
        glyph_row_scale_runtime: 0x8001_61AC,
        glyph_y_store_runtime: 0x8001_61C4,
        glyph_column_reg: Reg::A0,
        glyph_header_y_reg: Reg::A3,
        glyph_row_reg: Reg::A1,
        message_update_runtime: 0x8001_5FA4,
        message_allocator_runtime: 0x8001_4CF0,
        message_allocator_callback_store_runtime: 0x8001_4D0C,
        message_allocation_sites: &PUYO2_MESSAGE_ALLOCATION_SITES,
        command_registration_runtime: 0x8001_7210,
        command_registration_call_runtime: 0x8001_7224,
        command_registration_target_runtime: 0x8002_2890,
        command_copy_words_runtime: 0x8006_0264,
        command_source_table_runtime: 0x8008_B360,
        command_dispatcher_runtime: 0x8002_2824,
        command_dispatch_table_runtime: 0x8008_B740,
        message_handler_runtime: 0x8001_6850,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqDracMessageLayout {
    pub index: usize,
    pub variant: u8,
    pub header_x_units: u8,
    pub header_y_units: u8,
    pub header_columns: u8,
    pub header_rows: u8,
    pub derived_columns: usize,
    pub derived_rows: usize,
    pub line_break_pairs: usize,
    pub column_advances: usize,
    pub end_waits: usize,
    pub current_waits: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracConsumerRow {
    pub scope: &'static str,
    pub exe: &'static str,
    pub current_record_load_runtime: u32,
    pub header_consumer_runtime: u32,
    pub body_tokenizer_runtime: u32,
    pub token_cursor_load_runtime: u32,
    pub token_table_runtime: u32,
    pub dispatch_table_runtime: u32,
    pub dispatch_entries: usize,
    pub dispatch_first_token: u8,
    pub dispatch_last_token: u8,
    pub argument_tokens: [u8; 3],
    pub forbidden_state_tokens: Vec<u8>,
    pub records: usize,
    pub header_bytes: usize,
    pub body_bytes: usize,
    pub glyph_tokens: usize,
    pub argument_controls: usize,
    pub state_controls: usize,
    pub terminators: usize,
    pub control_counts: BTreeMap<u8, usize>,
    pub command_registration_runtime: u32,
    pub command_source_table_runtime: u32,
    pub command_copy_words_runtime: u32,
    pub command_dispatcher_runtime: u32,
    pub command_dispatch_table_runtime: u32,
    pub message_handler_runtime: u32,
    pub command_records: usize,
    pub variant_records: usize,
    pub variant_state_runtime: u32,
    pub header_layout_matches: usize,
    pub line_break_pairs: usize,
    pub column_advances: usize,
    pub end_waits: usize,
    pub current_waits: usize,
    pub message_layouts: Vec<SubgameSeqDracMessageLayout>,
    pub glyph_handler_runtime: u32,
    pub message_update_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracConsumerAnalysis {
    pub rows: Vec<SubgameSeqDracConsumerRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracConsumerError(String);

impl fmt::Display for SubgameSeqDracConsumerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqDracConsumerError {}

fn error(message: impl Into<String>) -> SubgameSeqDracConsumerError {
    SubgameSeqDracConsumerError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, SubgameSeqDracConsumerError> {
    instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| error(format!("DRAC consumer 명령 주소 없음: 0x{address:08X}")))
}

fn require(
    instructions: &[Instruction],
    spec: SubgameSeqDracConsumerSpec,
    address: u32,
    expected: K,
) -> Result<(), SubgameSeqDracConsumerError> {
    let actual = instruction_at(instructions, address)?;
    if actual.kind != expected {
        return Err(error(format!(
            "{} DRAC consumer 명령 불일치: address=0x{address:08X}, expected={expected:?}, actual={:?}",
            spec.exe, actual.kind
        )));
    }
    Ok(())
}

fn high_low(address: u32) -> (u16, i16) {
    (
        ((address.wrapping_add(0x8000)) >> 16) as u16,
        address as u16 as i16,
    )
}

fn audit_subgame_seq_drac_consumers<'a>(
    specs: &[SubgameSeqDracConsumerSpec],
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    drac_text: &SubgameSeqDracTextAnalysis,
) -> Result<SubgameSeqDracConsumerAnalysis, SubgameSeqDracConsumerError> {
    let mut executable_map = BTreeMap::new();
    for (map, text) in executables {
        if executable_map.insert(map.name, (map, text)).is_some() {
            return Err(error(format!("{} DRAC consumer EXE 입력 중복", map.name)));
        }
    }
    let mut spec_exes = BTreeSet::new();
    for spec in specs {
        if !spec_exes.insert(spec.exe) {
            return Err(error(format!("{} DRAC consumer spec 중복", spec.exe)));
        }
    }
    if specs.len() != 2 || executable_map.len() != 2 {
        return Err(error("DRAC consumer EXE/spec 집합 불일치"));
    }
    let mut source_map = BTreeMap::new();
    for (archive, sequence, raw) in sources {
        if source_map.insert((archive, sequence), raw).is_some() {
            return Err(error(format!(
                "DRAC consumer source 입력 중복: {archive}:{sequence}"
            )));
        }
    }

    let expected_control_counts = OBSERVED_CONTROL_COUNTS
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let mut rows = Vec::with_capacity(specs.len());
    for &spec in specs {
        let &(map, text) = executable_map
            .get(spec.exe)
            .ok_or_else(|| error(format!("{} DRAC consumer EXE 입력 누락", spec.exe)))?;
        if text.len() != map.text_len as usize {
            return Err(error(format!(
                "{} DRAC consumer text 길이 불일치",
                spec.exe
            )));
        }
        let instructions = linear::sweep(map, text)
            .ok_or_else(|| error(format!("{} DRAC consumer text 정렬 불일치", spec.exe)))?;
        verify_command_dispatch(map, text, &instructions, spec)?;
        verify_record_loader(&instructions, spec)?;
        verify_token_dispatch(map, text, &instructions, spec)?;
        verify_forbidden_state_handlers(&instructions, spec)?;
        verify_observed_handler_effects(&instructions, spec)?;
        verify_layout_dataflow(&instructions, spec)?;

        let mut message_rows = drac_text
            .rows
            .iter()
            .filter(|row| row.scope == spec.scope && row.kind == SubgameSeqDracTextKind::Message)
            .collect::<Vec<_>>();
        message_rows.sort_by_key(|row| row.index);
        if message_rows.len() != BOX_SUBGAME_SEQ_DRAC_RECORDS_PER_EXE {
            return Err(error(format!(
                "{} DRAC message record 수 불일치: expected={}, actual={}",
                spec.scope,
                BOX_SUBGAME_SEQ_DRAC_RECORDS_PER_EXE,
                message_rows.len()
            )));
        }
        let mut totals = DracRecordStats::default();
        let mut command_records = 0usize;
        let mut variant_records = 0usize;
        let mut message_layouts = Vec::with_capacity(message_rows.len());
        for row in message_rows {
            let source = source_map
                .get(&(row.archive, row.sequence))
                .ok_or_else(|| {
                    error(format!(
                        "{}:{} DRAC consumer source 입력 누락",
                        row.archive, row.sequence
                    ))
                })?;
            let pointer_bytes = source
                .get(row.pointer_field_offset..row.pointer_field_offset + 4)
                .ok_or_else(|| error("DRAC message pointer field 범위 이탈"))?;
            let pointer = u32::from_le_bytes(pointer_bytes.try_into().expect("4바이트 word"));
            if pointer != row.record_runtime {
                return Err(error(format!(
                    "{} DRAC message pointer/record runtime 불일치: field=+0x{:04X}",
                    row.scope, row.pointer_field_offset
                )));
            }
            let variant = parse_message_command_variant(source, row.pointer_field_offset)?;
            command_records += 1;
            variant_records += usize::from(variant == 1);
            let stats = parse_message_record(&row.raw)?;
            let layout = analyze_message_layout(&row.raw, row.index, variant)?;
            let expected_header = BOX_SUBGAME_SEQ_DRAC_HEADER_PROFILES
                .get(
                    row.index
                        .checked_sub(1)
                        .ok_or_else(|| error("DRAC message index가 0"))?,
                )
                .ok_or_else(|| error("DRAC message index가 header 기준선 밖임"))?;
            let actual_header = [
                layout.header_x_units,
                layout.header_y_units,
                layout.header_columns,
                layout.header_rows,
            ];
            if actual_header != *expected_header
                || usize::from(layout.header_columns) != layout.derived_columns
                || usize::from(layout.header_rows) != layout.derived_rows
                || variant != u8::from(row.index == 3)
            {
                return Err(error(format!(
                    "{} DRAC message {} header/layout 기준선 불일치: header={actual_header:02X?}, derived={}/{}, variant={variant}",
                    spec.scope, row.index, layout.derived_columns, layout.derived_rows
                )));
            }
            message_layouts.push(layout);
            totals.header_bytes += stats.header_bytes;
            totals.body_bytes += stats.body_bytes;
            totals.glyph_tokens += stats.glyph_tokens;
            totals.argument_controls += stats.argument_controls;
            totals.state_controls += stats.state_controls;
            totals.terminators += stats.terminators;
            for (code, count) in stats.control_counts {
                *totals.control_counts.entry(code).or_default() += count;
            }
        }
        let actual = [
            totals.header_bytes,
            totals.body_bytes,
            totals.glyph_tokens,
            totals.argument_controls,
            totals.state_controls,
            totals.terminators,
        ];
        let expected = [
            BOX_SUBGAME_SEQ_DRAC_HEADER_BYTES_PER_EXE,
            BOX_SUBGAME_SEQ_DRAC_BODY_BYTES_PER_EXE,
            BOX_SUBGAME_SEQ_DRAC_GLYPH_TOKENS_PER_EXE,
            BOX_SUBGAME_SEQ_DRAC_ARGUMENT_CONTROLS_PER_EXE,
            BOX_SUBGAME_SEQ_DRAC_STATE_CONTROLS_PER_EXE,
            BOX_SUBGAME_SEQ_DRAC_TERMINATORS_PER_EXE,
        ];
        if actual != expected || totals.control_counts != expected_control_counts {
            return Err(error(format!(
                "{} DRAC token 분모 불일치: expected={expected:?}/{expected_control_counts:?}, actual={actual:?}/{:?}",
                spec.scope, totals.control_counts
            )));
        }
        if command_records != BOX_SUBGAME_SEQ_DRAC_RECORDS_PER_EXE
            || variant_records != BOX_SUBGAME_SEQ_DRAC_VARIANT_RECORDS_PER_EXE
        {
            return Err(error(format!(
                "{} DRAC E0 command 분모 불일치: records={command_records}, variants={variant_records}",
                spec.scope
            )));
        }
        let header_layout_matches = message_layouts
            .iter()
            .filter(|layout| {
                usize::from(layout.header_columns) == layout.derived_columns
                    && usize::from(layout.header_rows) == layout.derived_rows
            })
            .count();
        let line_break_pairs = message_layouts
            .iter()
            .map(|layout| layout.line_break_pairs)
            .sum::<usize>();
        let column_advances = message_layouts
            .iter()
            .map(|layout| layout.column_advances)
            .sum::<usize>();
        let end_waits = message_layouts
            .iter()
            .map(|layout| layout.end_waits)
            .sum::<usize>();
        let current_waits = message_layouts
            .iter()
            .map(|layout| layout.current_waits)
            .sum::<usize>();
        if header_layout_matches != BOX_SUBGAME_SEQ_DRAC_LAYOUT_MATCHES_PER_EXE
            || line_break_pairs != BOX_SUBGAME_SEQ_DRAC_LINE_BREAK_PAIRS_PER_EXE
            || column_advances != BOX_SUBGAME_SEQ_DRAC_COLUMN_ADVANCES_PER_EXE
            || end_waits != BOX_SUBGAME_SEQ_DRAC_END_WAITS_PER_EXE
            || current_waits != BOX_SUBGAME_SEQ_DRAC_CURRENT_WAITS_PER_EXE
        {
            return Err(error(format!(
                "{} DRAC layout/control 역할 기준선 불일치: matches={header_layout_matches}, line-breaks={line_break_pairs}, columns={column_advances}, end-waits={end_waits}, current-waits={current_waits}",
                spec.scope
            )));
        }
        rows.push(SubgameSeqDracConsumerRow {
            scope: spec.scope,
            exe: spec.exe,
            current_record_load_runtime: spec.current_record_load_runtime,
            header_consumer_runtime: spec.header_consumer_runtime,
            body_tokenizer_runtime: spec.tokenizer_runtime,
            token_cursor_load_runtime: spec.token_cursor_load_runtime,
            token_table_runtime: spec.token_table_runtime,
            dispatch_table_runtime: spec.dispatch_table_runtime,
            dispatch_entries: spec.dispatch_targets.len(),
            dispatch_first_token: BOX_SUBGAME_SEQ_DRAC_TOKEN_FIRST,
            dispatch_last_token: BOX_SUBGAME_SEQ_DRAC_TOKEN_FIRST
                + u8::try_from(spec.dispatch_targets.len() - 1)
                    .map_err(|_| error("DRAC dispatch 끝 token 변환 실패"))?,
            argument_tokens: BOX_SUBGAME_SEQ_DRAC_ARGUMENT_TOKENS,
            forbidden_state_tokens: if spec.token_78_handler_runtime.is_some() {
                vec![0x63, 0x78]
            } else {
                vec![0x63]
            },
            records: BOX_SUBGAME_SEQ_DRAC_RECORDS_PER_EXE,
            header_bytes: totals.header_bytes,
            body_bytes: totals.body_bytes,
            glyph_tokens: totals.glyph_tokens,
            argument_controls: totals.argument_controls,
            state_controls: totals.state_controls,
            terminators: totals.terminators,
            control_counts: totals.control_counts,
            command_registration_runtime: spec.command_registration_runtime,
            command_source_table_runtime: spec.command_source_table_runtime,
            command_copy_words_runtime: spec.command_copy_words_runtime,
            command_dispatcher_runtime: spec.command_dispatcher_runtime,
            command_dispatch_table_runtime: spec.command_dispatch_table_runtime,
            message_handler_runtime: spec.message_handler_runtime,
            command_records,
            variant_records,
            variant_state_runtime: spec.variant_state_runtime,
            header_layout_matches,
            line_break_pairs,
            column_advances,
            end_waits,
            current_waits,
            message_layouts,
            glyph_handler_runtime: spec.glyph_handler_runtime,
            message_update_runtime: spec.message_update_runtime,
        });
    }
    if rows.len() == 2 && rows[0].message_layouts != rows[1].message_layouts {
        return Err(error("PUYO1/PUYO2 DRAC message layout이 서로 다름"));
    }
    Ok(SubgameSeqDracConsumerAnalysis { rows })
}

/// 두 EXE의 DRAC record→token dispatcher와 지원 원본 token 분모를 검증한다.
pub fn verify_box_subgame_seq_drac_consumers<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    drac_text: &SubgameSeqDracTextAnalysis,
) -> Result<SubgameSeqDracConsumerAnalysis, SubgameSeqDracConsumerError> {
    let analysis = audit_subgame_seq_drac_consumers(
        &BOX_SUBGAME_SEQ_DRAC_CONSUMER_SPECS,
        executables,
        sources,
        drac_text,
    )?;
    if analysis.rows.len() != BOX_SUBGAME_SEQ_DRAC_CONSUMER_ROWS {
        return Err(error(format!(
            "DRAC consumer 행 수 불일치: expected={}, actual={}",
            BOX_SUBGAME_SEQ_DRAC_CONSUMER_ROWS,
            analysis.rows.len()
        )));
    }
    let report = render_subgame_seq_drac_consumers_tsv(&analysis);
    let digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if digest != BOX_SUBGAME_SEQ_DRAC_CONSUMER_REPORT_SHA256 {
        return Err(error(format!(
            "DRAC consumer report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_DRAC_CONSUMER_REPORT_SHA256),
            hex(&digest)
        )));
    }
    Ok(analysis)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_drac_consumer_tests.rs"]
mod subgame_seq_drac_consumer_tests;
