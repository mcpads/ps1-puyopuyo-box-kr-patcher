//! DRAC repertoire의 code pair -> BTX cell 소비 경계.
//!
//! `E1`은 SEQ의 56개 Shift-JIS pair를 EXE별 상주 buffer로 복사하고,
//! tokenizer는 message의 pair를 그 buffer에서 찾아 `pair offset / 2`로
//! 바꾼다. glyph 생성기는 그 값에 30을 더해 cell `30..=85`를 만들고
//! `E2`가 기록한 resource를 object `+0x6C`에 넣는다. short renderer는
//! GRP descriptor root에서 `resource * 36`, 이어서 `cell * 12`를 적용한다.
//! `DRAC_SEQ`의 `0x28/0x12` record는 같은 장면 안의 cell table pointer를
//! 선택 descriptor `+0x1C`에 기록한다. 이 모듈은 그 전체 정적 연결과
//! 실제 BTX 원문 및 cell `30..=85`의 정확한 사각형을 검증한다. CLUT,
//! VRAM upload와 정상 장면의 draw hit는 별도 동적 판정으로 남긴다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::archive::VddArchive;
use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K};
use crate::script::{SubgameSeqDracTextAnalysis, SubgameSeqDracTextKind};

use super::{
    linear, BtxCellAnalysis, BtxNameAnalysis, BtxRendererFamily, SUBGAME_SEQ_LOAD_DESTINATION,
};

mod assets;
mod cell_setup;
mod command_handlers;
mod descriptor_root;
mod glyph_object_route;
mod renderer_binding;
mod report;
mod resource_consumers;
mod sequence_records;

#[cfg(test)]
use assets::equal_glyph_cells;
use assets::verify_assets;
use cell_setup::{verify_cell_setup_dispatch, verify_cell_setup_record_and_layout};
use command_handlers::{verify_repertoire_handler, verify_resource_handler};
use descriptor_root::verify_descriptor_root_initialization;
use glyph_object_route::{verify_main_glyph_object_route, verify_tokenizer};
use renderer_binding::verify_renderer;
pub use report::render_subgame_seq_drac_glyphs_tsv;
#[cfg(test)]
use resource_consumers::memory_base_and_offset;
use resource_consumers::{
    gp_runtime_address, verify_auxiliary_consumers, verify_main_resource_consumers,
};
use sequence_records::{
    verify_command_table, verify_message_subset, verify_same_named_resource_record,
    verify_setup_records,
};

pub const SUBGAME_SEQ_DRAC_GLYPH_SCHEMA_VERSION: &str = "subgame-seq-drac-glyph-v6";
pub const BOX_SUBGAME_SEQ_DRAC_GLYPH_ROWS: usize = 2;
pub const BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS: usize = 56;
pub const BOX_SUBGAME_SEQ_DRAC_SOURCE_UNIQUE_PAIRS: usize = 52;
pub const BOX_SUBGAME_SEQ_DRAC_SOURCE_DUPLICATE_SLOTS: usize = 4;
pub const BOX_SUBGAME_SEQ_DRAC_SPACE_PAIR_INDICES: [usize; 5] = [27, 28, 29, 30, 31];
pub const BOX_SUBGAME_SEQ_DRAC_CELL_BASE: u16 = 30;
pub const BOX_SUBGAME_SEQ_DRAC_CELL_LAST: u16 = 85;
pub const BOX_SUBGAME_SEQ_DRAC_DESCRIPTOR_STRIDE: u32 = 36;
pub const BOX_SUBGAME_SEQ_DRAC_CELL_STRIDE: u32 = 12;
pub const BOX_SUBGAME_SEQ_DRAC_AUXILIARY_GP_ACCESSES: usize = 4;
pub const BOX_SUBGAME_SEQ_DRAC_AUXILIARY_OBJECT_SITES: usize = 2;
pub const BOX_SUBGAME_SEQ_DRAC_AUXILIARY_CELL: u16 = 31;
pub const BOX_SUBGAME_SEQ_DRAC_AUXILIARY_NEXT_CELL: u16 = 32;
pub const BOX_SUBGAME_SEQ_DRAC_AUXILIARY_CELL_TABLE_OFFSET: i16 = 0x0174;
pub const BOX_SUBGAME_SEQ_DRAC_AUXILIARY_NEXT_CELL_TABLE_OFFSET: i16 = 0x0180;
pub const BOX_SUBGAME_SEQ_DRAC_MAIN_RESOURCE_BRANCH_SITES: usize = 0;
pub const BOX_SUBGAME_SEQ_DRAC_MAIN_GP_ACCESSES: usize = 2;
pub const PUYO1_DRAC_GP_RUNTIME: u32 = 0x8009_56C4;
pub const PUYO1_DRAC_MAIN_RESOURCE_GLOBAL_RUNTIME: u32 = 0x8009_5AC0;
pub const PUYO1_DRAC_AUXILIARY_RESOURCE_GLOBAL_RUNTIME: u32 = 0x8009_5AC4;
pub const PUYO2_DRAC_GP_RUNTIME: u32 = 0x8009_7950;
pub const PUYO2_DRAC_MAIN_RESOURCE_GLOBAL_RUNTIME: u32 = 0x8009_7D58;
pub const PUYO2_DRAC_AUXILIARY_RESOURCE_GLOBAL_RUNTIME: u32 = 0x8009_7D5C;
pub const BOX_SUBGAME_SEQ_DRAC_ATLAS_BYTES: usize = 131_072;
pub const BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH: u16 = 1_024;
pub const BOX_SUBGAME_SEQ_DRAC_ATLAS_HEIGHT: u16 = 256;
pub const BOX_SUBGAME_SEQ_DRAC_ATLAS_PIXELS: usize =
    BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH as usize * BOX_SUBGAME_SEQ_DRAC_ATLAS_HEIGHT as usize;
pub const BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET: usize = 0x039C;
pub const BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_RUNTIME: u32 = 0x801B_560C;
pub const BOX_SUBGAME_SEQ_DRAC_CELL_HEADER_BYTES: usize = 12;
pub const BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH: u16 = 16;
pub const BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT: u16 = 16;
pub const BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X: u16 = 640;
pub const BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y: u16 = 0;
pub const BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS: usize = 8;
pub const BOX_SUBGAME_SEQ_DRAC_CELL_GRID_ROWS: usize = 7;
pub const BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_PREFIX_END: usize = 0x07B0;
pub const BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_PREFIX_SHA256: &str =
    "2fdbc15d6c60c01308311392c66e084362e4bbfe9f09cba72e7acbfd54cd9f88";

/// 지원 원본에서 생성한 2행 TSV의 SHA-256.
pub const BOX_SUBGAME_SEQ_DRAC_GLYPH_REPORT_SHA256: [u8; 32] = [
    0xDC, 0xFB, 0x36, 0x79, 0xC2, 0x1B, 0x40, 0xB6, 0x56, 0x5C, 0xE5, 0x0A, 0x9D, 0x6F, 0x36, 0x8C,
    0xDB, 0x29, 0x5C, 0x99, 0x20, 0x0C, 0x60, 0x0F, 0xA7, 0x7A, 0x85, 0x71, 0xC0, 0xF5, 0x93, 0x39,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqDracGlyphSpec {
    pub scope: &'static str,
    pub exe: &'static str,
    pub seq_archive: &'static str,
    pub grp_archive: &'static str,
    pub sequence: &'static str,
    pub e2_record_offset: usize,
    pub e1_record_offset: usize,
    pub resource_id: u16,
    pub auxiliary_id: u16,
    pub asset: &'static str,
    pub auxiliary_asset: &'static str,
    pub asset_vdd_index: usize,
    pub asset_stored_bytes: u32,
    pub asset_plain_sha256: &'static str,
    pub same_named_asset: &'static str,
    pub same_named_resource_id: u16,
    pub same_named_asset_vdd_index: usize,
    pub same_named_asset_stored_bytes: u32,
    pub same_named_asset_plain_sha256: &'static str,
    pub selected_same_named_equal_cells: usize,
    pub same_named_e2_sequence: &'static str,
    pub same_named_e2_record_offset: usize,
    pub same_named_e2_auxiliary_id: u16,
    pub command_source_table_runtime: u32,
    pub e1_handler_runtime: u32,
    pub e2_handler_runtime: u32,
    pub repertoire_buffer_runtime: u32,
    pub message_buffer_runtime: u32,
    pub token_table_runtime: u32,
    pub tokenizer_runtime: u32,
    pub pair_lookup_runtime: u32,
    pub pair_index_store_runtime: u32,
    pub cell_index_runtime: u32,
    pub cell_index_add_runtime: u32,
    pub cell_index_store_runtime: u32,
    pub glyph_object_allocator_runtime: u32,
    pub glyph_object_callback_runtime: u32,
    pub resource_global_load_runtime: u32,
    pub resource_object_store_runtime: u32,
    pub resource_gp_offset: i16,
    pub auxiliary_gp_offset: i16,
    pub gp_runtime: u32,
    pub gp_lui_runtime: u32,
    pub gp_addiu_runtime: u32,
    pub auxiliary_descriptor_load_runtime: u32,
    pub auxiliary_cell_store_runtime: u32,
    pub auxiliary_next_cell_store_runtime: u32,
    pub auxiliary_object_load_runtimes: [u32; BOX_SUBGAME_SEQ_DRAC_AUXILIARY_OBJECT_SITES],
    pub auxiliary_object_cell_store_runtimes: [u32; BOX_SUBGAME_SEQ_DRAC_AUXILIARY_OBJECT_SITES],
    pub auxiliary_object_resource_store_runtimes:
        [u32; BOX_SUBGAME_SEQ_DRAC_AUXILIARY_OBJECT_SITES],
    pub auxiliary_object_allocator_runtimes: [u32; BOX_SUBGAME_SEQ_DRAC_AUXILIARY_OBJECT_SITES],
    pub auxiliary_object_allocator_target: u32,
    pub renderer_runtime: u32,
    pub renderer_resource_load_runtime: u32,
    pub renderer_cell_load_runtime: u32,
    pub renderer_descriptor_root_load_runtime: u32,
    pub renderer_cell_table_load_runtime: u32,
    pub renderer_cell_entry_runtime: u32,
    pub descriptor_root_runtime: u32,
    pub descriptor_base_runtime: u32,
    pub descriptor_base_lui_runtime: u32,
    pub descriptor_base_addiu_runtime: u32,
    pub descriptor_root_lui_runtime: u32,
    pub descriptor_root_store_runtime: u32,
    pub opcode_28_slot_runtime: u32,
    pub opcode_28_wrapper_runtime: u32,
    pub subdispatcher_runtime: u32,
    pub subdispatch_table_runtime: u32,
    pub cell_setup_handler_runtime: u32,
    pub cell_setup_record_offset: usize,
    pub cell_table_pointer_load_runtime: u32,
    pub descriptor_cell_table_store_runtime: u32,
    pub cell_count_load_runtime: u32,
    pub descriptor_cell_count_store_runtime: u32,
}

pub const BOX_SUBGAME_SEQ_DRAC_GLYPH_SPECS: [SubgameSeqDracGlyphSpec; 2] = [
    SubgameSeqDracGlyphSpec {
        scope: "PUYO1",
        exe: "PUYO1.EXE",
        seq_archive: "/PUYO1/PP1_SEQ.VDD;1",
        grp_archive: "/PUYO1/PP1_GRP.VDD;1",
        sequence: "DRAC_SEQ.SEQ",
        e2_record_offset: 0x0B5C,
        e1_record_offset: 0x0B64,
        resource_id: 0x005D,
        auxiliary_id: 0x0059,
        asset: "MZ_DRA.BTX",
        auxiliary_asset: "MZ_ARLE.BTX",
        asset_vdd_index: 55,
        asset_stored_bytes: 30_584,
        asset_plain_sha256: "74d42b1d57583f0225ad04e8c5aa03a284742664272b197779450231c35de7b0",
        same_named_asset: "MZ_DRA.BTX",
        same_named_resource_id: 0x005D,
        same_named_asset_vdd_index: 55,
        same_named_asset_stored_bytes: 30_584,
        same_named_asset_plain_sha256:
            "74d42b1d57583f0225ad04e8c5aa03a284742664272b197779450231c35de7b0",
        selected_same_named_equal_cells: BOX_SUBGAME_SEQ_DRAC_REPERTOIRE_GLYPHS,
        same_named_e2_sequence: "DRAC_SEQ.SEQ",
        same_named_e2_record_offset: 0x0B5C,
        same_named_e2_auxiliary_id: 0x0059,
        command_source_table_runtime: 0x8008_9310,
        e1_handler_runtime: 0x8001_8B80,
        e2_handler_runtime: 0x8001_8C58,
        repertoire_buffer_runtime: 0x800B_2F38,
        message_buffer_runtime: 0x800B_2ED0,
        token_table_runtime: 0x800B_3000,
        tokenizer_runtime: 0x8001_78E4,
        pair_lookup_runtime: 0x8001_7998,
        pair_index_store_runtime: 0x8001_7A08,
        cell_index_runtime: 0x8001_783C,
        cell_index_add_runtime: 0x8001_7840,
        cell_index_store_runtime: 0x8001_7844,
        glyph_object_allocator_runtime: 0x8001_77F8,
        glyph_object_callback_runtime: 0x8001_89B0,
        resource_global_load_runtime: 0x8001_7860,
        resource_object_store_runtime: 0x8001_786C,
        resource_gp_offset: 0x03FC,
        auxiliary_gp_offset: 0x0400,
        gp_runtime: PUYO1_DRAC_GP_RUNTIME,
        gp_lui_runtime: 0x8001_5298,
        gp_addiu_runtime: 0x8001_529C,
        auxiliary_descriptor_load_runtime: 0x8001_7E0C,
        auxiliary_cell_store_runtime: 0x8001_7E64,
        auxiliary_next_cell_store_runtime: 0x8001_7E34,
        auxiliary_object_load_runtimes: [0x8001_7ED4, 0x8001_8B3C],
        auxiliary_object_cell_store_runtimes: [0x8001_7EDC, 0x8001_8B44],
        auxiliary_object_resource_store_runtimes: [0x8001_7EF4, 0x8001_8B5C],
        auxiliary_object_allocator_runtimes: [0x8001_7EC4, 0x8001_8B2C],
        auxiliary_object_allocator_target: 0x8001_53EC,
        renderer_runtime: 0x8002_4AB4,
        renderer_resource_load_runtime: 0x8002_4AE8,
        renderer_cell_load_runtime: 0x8002_4B00,
        renderer_descriptor_root_load_runtime: 0x8002_4AFC,
        renderer_cell_table_load_runtime: 0x8002_4B10,
        renderer_cell_entry_runtime: 0x8002_4B18,
        descriptor_root_runtime: 0x800B_4330,
        descriptor_base_runtime: 0x8006_7E48,
        descriptor_base_lui_runtime: 0x8002_A51C,
        descriptor_base_addiu_runtime: 0x8002_A520,
        descriptor_root_lui_runtime: 0x8002_A518,
        descriptor_root_store_runtime: 0x8002_A528,
        opcode_28_slot_runtime: 0x8008_9790,
        opcode_28_wrapper_runtime: 0x8002_4314,
        subdispatcher_runtime: 0x8001_C6F8,
        subdispatch_table_runtime: 0x8008_954C,
        cell_setup_handler_runtime: 0x8001_AEA4,
        cell_setup_record_offset: 0x139C,
        cell_table_pointer_load_runtime: 0x8001_B168,
        descriptor_cell_table_store_runtime: 0x8001_B170,
        cell_count_load_runtime: 0x8001_B17C,
        descriptor_cell_count_store_runtime: 0x8001_B184,
    },
    SubgameSeqDracGlyphSpec {
        scope: "PUYO2",
        exe: "PUYO2.EXE",
        seq_archive: "/PUYO2/PP2_SEQ.VDD;1",
        grp_archive: "/PUYO2/PP2_GRP.VDD;1",
        sequence: "DRAC_SEQ.SEQ",
        e2_record_offset: 0x0B5C,
        e1_record_offset: 0x0B64,
        resource_id: 0x008C,
        auxiliary_id: 0x0088,
        asset: "MZ_MUMY.BTX",
        auxiliary_asset: "DEMOFONT.BTX",
        asset_vdd_index: 137,
        asset_stored_bytes: 30_021,
        asset_plain_sha256: "c0c1ee748a88305c267ddd378d8d260396e0fd4e8ac7719fc2c4b281d3e61185",
        same_named_asset: "MZ_DRA.BTX",
        same_named_resource_id: 0x008D,
        same_named_asset_vdd_index: 138,
        same_named_asset_stored_bytes: 30_584,
        same_named_asset_plain_sha256:
            "74d42b1d57583f0225ad04e8c5aa03a284742664272b197779450231c35de7b0",
        selected_same_named_equal_cells: 0,
        same_named_e2_sequence: "TARA_SEQ.SEQ",
        same_named_e2_record_offset: 0x0B44,
        same_named_e2_auxiliary_id: 0x0088,
        command_source_table_runtime: 0x8008_B360,
        e1_handler_runtime: 0x8001_75CC,
        e2_handler_runtime: 0x8001_76A4,
        repertoire_buffer_runtime: 0x800B_52D8,
        message_buffer_runtime: 0x800B_51A8,
        token_table_runtime: 0x800B_53E0,
        tokenizer_runtime: 0x8001_6210,
        pair_lookup_runtime: 0x8001_62BC,
        pair_index_store_runtime: 0x8001_632C,
        cell_index_runtime: 0x8001_618C,
        cell_index_add_runtime: 0x8001_6194,
        cell_index_store_runtime: 0x8001_6198,
        glyph_object_allocator_runtime: 0x8001_6158,
        glyph_object_callback_runtime: 0x8001_7400,
        resource_global_load_runtime: 0x8001_61BC,
        resource_object_store_runtime: 0x8001_61C8,
        resource_gp_offset: 0x0408,
        auxiliary_gp_offset: 0x040C,
        gp_runtime: PUYO2_DRAC_GP_RUNTIME,
        gp_lui_runtime: 0x8001_4AB8,
        gp_addiu_runtime: 0x8001_4ABC,
        auxiliary_descriptor_load_runtime: 0x8001_6730,
        auxiliary_cell_store_runtime: 0x8001_6788,
        auxiliary_next_cell_store_runtime: 0x8001_6758,
        auxiliary_object_load_runtimes: [0x8001_67F8, 0x8001_7588],
        auxiliary_object_cell_store_runtimes: [0x8001_6800, 0x8001_7590],
        auxiliary_object_resource_store_runtimes: [0x8001_6818, 0x8001_75A8],
        auxiliary_object_allocator_runtimes: [0x8001_67E8, 0x8001_7578],
        auxiliary_object_allocator_target: 0x8001_4CF0,
        renderer_runtime: 0x8002_3AF0,
        renderer_resource_load_runtime: 0x8002_3B24,
        renderer_cell_load_runtime: 0x8002_3B3C,
        renderer_descriptor_root_load_runtime: 0x8002_3B38,
        renderer_cell_table_load_runtime: 0x8002_3B4C,
        renderer_cell_entry_runtime: 0x8002_3B54,
        descriptor_root_runtime: 0x800B_6710,
        descriptor_base_runtime: 0x8006_3E9C,
        descriptor_base_lui_runtime: 0x8002_27E0,
        descriptor_base_addiu_runtime: 0x8002_27E4,
        descriptor_root_lui_runtime: 0x8002_27D8,
        descriptor_root_store_runtime: 0x8002_27F0,
        opcode_28_slot_runtime: 0x8008_B7E0,
        opcode_28_wrapper_runtime: 0x8002_3364,
        subdispatcher_runtime: 0x8001_B184,
        subdispatch_table_runtime: 0x8008_B59C,
        cell_setup_handler_runtime: 0x8001_9930,
        cell_setup_record_offset: 0x1308,
        cell_table_pointer_load_runtime: 0x8001_9BF4,
        descriptor_cell_table_store_runtime: 0x8001_9BFC,
        cell_count_load_runtime: 0x8001_9C08,
        descriptor_cell_count_store_runtime: 0x8001_9C10,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracGlyphRow {
    pub scope: &'static str,
    pub exe: &'static str,
    pub seq_archive: &'static str,
    pub grp_archive: &'static str,
    pub sequence: &'static str,
    pub e2_record_offset: usize,
    pub e1_record_offset: usize,
    pub e1_record_runtime: u32,
    pub repertoire_runtime: u32,
    pub e1_handler_runtime: u32,
    pub e2_handler_runtime: u32,
    pub repertoire_buffer_runtime: u32,
    pub tokenizer_runtime: u32,
    pub pair_lookup_runtime: u32,
    pub pair_index_store_runtime: u32,
    pub repertoire_glyphs: usize,
    pub source_unique_pairs: usize,
    pub source_duplicate_slots: usize,
    pub cell_base: u16,
    pub cell_last: u16,
    pub glyph_object_allocator_runtime: u32,
    pub glyph_object_callback_runtime: u32,
    pub main_resource_branch_sites: usize,
    pub main_gp_accesses: usize,
    pub gp_runtime: u32,
    pub main_resource_global_runtime: u32,
    pub auxiliary_resource_global_runtime: u32,
    pub resource_id: u16,
    pub auxiliary_id: u16,
    pub auxiliary_asset: &'static str,
    pub auxiliary_descriptor_runtime: u32,
    pub auxiliary_gp_accesses: usize,
    pub auxiliary_descriptor_load_runtime: u32,
    pub auxiliary_cell_store_runtime: u32,
    pub auxiliary_next_cell_store_runtime: u32,
    pub auxiliary_object_load_runtimes: [u32; BOX_SUBGAME_SEQ_DRAC_AUXILIARY_OBJECT_SITES],
    pub auxiliary_object_cell_store_runtimes: [u32; BOX_SUBGAME_SEQ_DRAC_AUXILIARY_OBJECT_SITES],
    pub auxiliary_object_resource_store_runtimes:
        [u32; BOX_SUBGAME_SEQ_DRAC_AUXILIARY_OBJECT_SITES],
    pub cell_setup_record_offset: usize,
    pub cell_setup_record_runtime: u32,
    pub cell_setup_handler_runtime: u32,
    pub cell_table_offset: usize,
    pub cell_table_runtime: u32,
    pub cell_table_prefix_sha256: String,
    pub atlas_width: u16,
    pub atlas_height: u16,
    pub cell_grid_x: u16,
    pub cell_grid_y: u16,
    pub cell_grid_columns: usize,
    pub cell_grid_rows: usize,
    pub cell_width: u16,
    pub cell_height: u16,
    pub renderer_runtime: u32,
    pub renderer_resource_load_runtime: u32,
    pub renderer_cell_load_runtime: u32,
    pub descriptor_root_runtime: u32,
    pub descriptor_runtime: u32,
    pub asset: &'static str,
    pub asset_vdd_index: usize,
    pub asset_stored_bytes: u32,
    pub asset_plain_bytes: usize,
    pub asset_plain_sha256: String,
    pub same_named_asset: &'static str,
    pub same_named_resource_id: u16,
    pub same_named_descriptor_runtime: u32,
    pub same_named_asset_vdd_index: usize,
    pub same_named_asset_stored_bytes: u32,
    pub same_named_asset_plain_sha256: String,
    pub selected_same_named_equal_cells: usize,
    pub same_named_e2_sequence: &'static str,
    pub same_named_e2_record_offset: usize,
    pub atlas_pixels: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracGlyphAnalysis {
    pub rows: Vec<SubgameSeqDracGlyphRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqDracGlyphError(String);

impl fmt::Display for SubgameSeqDracGlyphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqDracGlyphError {}

fn error(message: impl Into<String>) -> SubgameSeqDracGlyphError {
    SubgameSeqDracGlyphError(message.into())
}

fn high_low(address: u32) -> (u16, i16) {
    (
        ((address.wrapping_add(0x8000)) >> 16) as u16,
        address as u16 as i16,
    )
}

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, SubgameSeqDracGlyphError> {
    instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or_else(|| error(format!("DRAC glyph 명령 주소 없음: 0x{address:08X}")))
}

fn require(
    instructions: &[Instruction],
    spec: SubgameSeqDracGlyphSpec,
    address: u32,
    expected: K,
) -> Result<(), SubgameSeqDracGlyphError> {
    let actual = instruction_at(instructions, address)?;
    if actual.kind != expected {
        return Err(error(format!(
            "{} DRAC glyph 명령 불일치: address=0x{address:08X}, expected={expected:?}, actual={:?}",
            spec.exe, actual.kind
        )));
    }
    Ok(())
}

fn read_u32(raw: &[u8], offset: usize) -> Result<u32, SubgameSeqDracGlyphError> {
    raw.get(offset..offset + 4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4바이트 word")))
        .ok_or_else(|| error(format!("DRAC glyph word 범위 이탈: +0x{offset:X}")))
}

fn read_u16(raw: &[u8], offset: usize) -> Result<u16, SubgameSeqDracGlyphError> {
    raw.get(offset..offset + 2)
        .map(|bytes| u16::from_le_bytes(bytes.try_into().expect("2바이트 halfword")))
        .ok_or_else(|| error(format!("DRAC glyph halfword 범위 이탈: +0x{offset:X}")))
}

fn read_i16(raw: &[u8], offset: usize) -> Result<i16, SubgameSeqDracGlyphError> {
    read_u16(raw, offset).map(|value| value as i16)
}

fn read_exe_word(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    label: &str,
) -> Result<u32, SubgameSeqDracGlyphError> {
    let offset = map
        .text_offset(runtime)
        .ok_or_else(|| error(format!("{}가 EXE 밖임: 0x{runtime:08X}", label)))?;
    read_u32(text, offset)
}

fn audit_subgame_seq_drac_glyphs<'a>(
    specs: &[SubgameSeqDracGlyphSpec],
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    grp_archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
    drac_text: &SubgameSeqDracTextAnalysis,
    btx_names: &BtxNameAnalysis,
    btx_cells: &BtxCellAnalysis,
) -> Result<SubgameSeqDracGlyphAnalysis, SubgameSeqDracGlyphError> {
    if specs.is_empty() {
        return Err(error("DRAC glyph spec이 없음"));
    }
    let mut spec_keys = BTreeSet::new();
    for spec in specs {
        if !spec_keys.insert((spec.exe, spec.seq_archive, spec.grp_archive, spec.sequence)) {
            return Err(error(format!("{} DRAC glyph spec 중복", spec.exe)));
        }
    }

    let mut executable_map = BTreeMap::new();
    for (map, text) in executables {
        if executable_map.insert(map.name, (map, text)).is_some() {
            return Err(error(format!("{} DRAC glyph EXE 입력 중복", map.name)));
        }
    }
    let mut source_map = BTreeMap::new();
    for (archive, sequence, raw) in sources {
        if source_map.insert((archive, sequence), raw).is_some() {
            return Err(error(format!(
                "{archive}:{sequence} DRAC glyph source 입력 중복"
            )));
        }
    }
    let mut archive_map = BTreeMap::new();
    for (archive, value) in grp_archives {
        if archive_map.insert(archive, value).is_some() {
            return Err(error(format!("{archive} DRAC glyph GRP 입력 중복")));
        }
    }
    if executable_map.len() != specs.len() || archive_map.len() != specs.len() {
        return Err(error("DRAC glyph EXE/GRP 입력 집합 불일치"));
    }

    let mut rows = Vec::with_capacity(specs.len());
    for &spec in specs {
        let &(map, text) = executable_map
            .get(spec.exe)
            .ok_or_else(|| error(format!("{} DRAC glyph EXE 누락", spec.exe)))?;
        if text.len() != map.text_len as usize {
            return Err(error(format!("{} DRAC glyph text 길이 불일치", spec.exe)));
        }
        let raw = source_map
            .get(&(spec.seq_archive, spec.sequence))
            .ok_or_else(|| {
                error(format!(
                    "{}:{} DRAC glyph source 누락",
                    spec.seq_archive, spec.sequence
                ))
            })?;
        let archive = archive_map
            .get(spec.grp_archive)
            .copied()
            .ok_or_else(|| error(format!("{} DRAC glyph GRP 누락", spec.grp_archive)))?;
        let repertoire = drac_text
            .rows
            .iter()
            .find(|row| row.scope == spec.scope && row.kind == SubgameSeqDracTextKind::Repertoire)
            .ok_or_else(|| error(format!("{} DRAC repertoire 누락", spec.scope)))?;
        if repertoire.pointer_field_offset != spec.e1_record_offset + 4 {
            return Err(error(format!(
                "{} DRAC E1 pointer field 위치 불일치",
                spec.scope
            )));
        }
        verify_setup_records(raw, spec, repertoire.record_runtime)?;
        let same_named_e2_raw = source_map
            .get(&(spec.seq_archive, spec.same_named_e2_sequence))
            .ok_or_else(|| {
                error(format!(
                    "{}:{} same-named asset E2 source 누락",
                    spec.seq_archive, spec.same_named_e2_sequence
                ))
            })?;
        verify_same_named_resource_record(same_named_e2_raw, spec)?;
        let cell_layout = verify_cell_setup_record_and_layout(raw, spec)?;
        let (repertoire_glyphs, source_unique_pairs) = verify_message_subset(drac_text, spec)?;

        let instructions = linear::sweep(map, text)
            .ok_or_else(|| error(format!("{} DRAC glyph text 정렬 불일치", spec.exe)))?;
        verify_command_table(map, text, spec)?;
        verify_repertoire_handler(&instructions, spec)?;
        verify_resource_handler(&instructions, spec)?;
        verify_main_resource_consumers(&instructions, spec)?;
        verify_auxiliary_consumers(&instructions, spec)?;
        verify_tokenizer(&instructions, spec)?;
        verify_main_glyph_object_route(&instructions, spec)?;
        verify_cell_setup_dispatch(map, text, &instructions, spec)?;
        verify_descriptor_root_initialization(&instructions, spec)?;
        verify_renderer(btx_cells, spec)?;
        let assets = verify_assets(map, text, archive, btx_names, spec)?;

        let e1_record_runtime = repertoire
            .pointer_field_runtime
            .checked_sub(4)
            .ok_or_else(|| error("DRAC E1 record runtime underflow"))?;
        rows.push(SubgameSeqDracGlyphRow {
            scope: spec.scope,
            exe: spec.exe,
            seq_archive: spec.seq_archive,
            grp_archive: spec.grp_archive,
            sequence: spec.sequence,
            e2_record_offset: spec.e2_record_offset,
            e1_record_offset: spec.e1_record_offset,
            e1_record_runtime,
            repertoire_runtime: repertoire.record_runtime,
            e1_handler_runtime: spec.e1_handler_runtime,
            e2_handler_runtime: spec.e2_handler_runtime,
            repertoire_buffer_runtime: spec.repertoire_buffer_runtime,
            tokenizer_runtime: spec.tokenizer_runtime,
            pair_lookup_runtime: spec.pair_lookup_runtime,
            pair_index_store_runtime: spec.pair_index_store_runtime,
            repertoire_glyphs,
            source_unique_pairs,
            source_duplicate_slots: repertoire_glyphs - source_unique_pairs,
            cell_base: BOX_SUBGAME_SEQ_DRAC_CELL_BASE,
            cell_last: BOX_SUBGAME_SEQ_DRAC_CELL_LAST,
            glyph_object_allocator_runtime: spec.glyph_object_allocator_runtime,
            glyph_object_callback_runtime: spec.glyph_object_callback_runtime,
            main_resource_branch_sites: BOX_SUBGAME_SEQ_DRAC_MAIN_RESOURCE_BRANCH_SITES,
            main_gp_accesses: BOX_SUBGAME_SEQ_DRAC_MAIN_GP_ACCESSES,
            gp_runtime: spec.gp_runtime,
            main_resource_global_runtime: gp_runtime_address(
                spec.gp_runtime,
                spec.resource_gp_offset,
            )
            .ok_or_else(|| error("DRAC main resource global 주소 overflow"))?,
            auxiliary_resource_global_runtime: gp_runtime_address(
                spec.gp_runtime,
                spec.auxiliary_gp_offset,
            )
            .ok_or_else(|| error("DRAC auxiliary resource global 주소 overflow"))?,
            resource_id: spec.resource_id,
            auxiliary_id: spec.auxiliary_id,
            auxiliary_asset: spec.auxiliary_asset,
            auxiliary_descriptor_runtime: assets.auxiliary_descriptor_runtime,
            auxiliary_gp_accesses: BOX_SUBGAME_SEQ_DRAC_AUXILIARY_GP_ACCESSES,
            auxiliary_descriptor_load_runtime: spec.auxiliary_descriptor_load_runtime,
            auxiliary_cell_store_runtime: spec.auxiliary_cell_store_runtime,
            auxiliary_next_cell_store_runtime: spec.auxiliary_next_cell_store_runtime,
            auxiliary_object_load_runtimes: spec.auxiliary_object_load_runtimes,
            auxiliary_object_cell_store_runtimes: spec.auxiliary_object_cell_store_runtimes,
            auxiliary_object_resource_store_runtimes: spec.auxiliary_object_resource_store_runtimes,
            cell_setup_record_offset: spec.cell_setup_record_offset,
            cell_setup_record_runtime: cell_layout.setup_record_runtime,
            cell_setup_handler_runtime: spec.cell_setup_handler_runtime,
            cell_table_offset: BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_OFFSET,
            cell_table_runtime: BOX_SUBGAME_SEQ_DRAC_CELL_TABLE_RUNTIME,
            cell_table_prefix_sha256: cell_layout.table_prefix_sha256,
            atlas_width: BOX_SUBGAME_SEQ_DRAC_ATLAS_WIDTH,
            atlas_height: BOX_SUBGAME_SEQ_DRAC_ATLAS_HEIGHT,
            cell_grid_x: BOX_SUBGAME_SEQ_DRAC_CELL_GRID_X,
            cell_grid_y: BOX_SUBGAME_SEQ_DRAC_CELL_GRID_Y,
            cell_grid_columns: BOX_SUBGAME_SEQ_DRAC_CELL_GRID_COLUMNS,
            cell_grid_rows: BOX_SUBGAME_SEQ_DRAC_CELL_GRID_ROWS,
            cell_width: BOX_SUBGAME_SEQ_DRAC_CELL_WIDTH,
            cell_height: BOX_SUBGAME_SEQ_DRAC_CELL_HEIGHT,
            renderer_runtime: spec.renderer_runtime,
            renderer_resource_load_runtime: spec.renderer_resource_load_runtime,
            renderer_cell_load_runtime: spec.renderer_cell_load_runtime,
            descriptor_root_runtime: spec.descriptor_root_runtime,
            descriptor_runtime: assets.descriptor_runtime,
            asset: spec.asset,
            asset_vdd_index: spec.asset_vdd_index,
            asset_stored_bytes: spec.asset_stored_bytes,
            asset_plain_bytes: assets.selected_plain.len(),
            asset_plain_sha256: assets.selected_plain_sha256,
            same_named_asset: spec.same_named_asset,
            same_named_resource_id: spec.same_named_resource_id,
            same_named_descriptor_runtime: assets.same_named_descriptor_runtime,
            same_named_asset_vdd_index: spec.same_named_asset_vdd_index,
            same_named_asset_stored_bytes: spec.same_named_asset_stored_bytes,
            same_named_asset_plain_sha256: assets.same_named_plain_sha256,
            selected_same_named_equal_cells: assets.equal_cells,
            same_named_e2_sequence: spec.same_named_e2_sequence,
            same_named_e2_record_offset: spec.same_named_e2_record_offset,
            atlas_pixels: assets.selected_plain.len() * 2,
        });
    }
    rows.sort_by_key(|row| row.scope);
    Ok(SubgameSeqDracGlyphAnalysis { rows })
}

/// 두 DRAC 사본의 repertoire -> token -> cell -> BTX 정적 연결을 검증한다.
pub fn verify_box_subgame_seq_drac_glyphs<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sources: impl IntoIterator<Item = (&'a str, &'a str, &'a [u8])>,
    grp_archives: impl IntoIterator<Item = (&'a str, &'a VddArchive)>,
    drac_text: &SubgameSeqDracTextAnalysis,
    btx_names: &BtxNameAnalysis,
    btx_cells: &BtxCellAnalysis,
) -> Result<SubgameSeqDracGlyphAnalysis, SubgameSeqDracGlyphError> {
    let analysis = audit_subgame_seq_drac_glyphs(
        &BOX_SUBGAME_SEQ_DRAC_GLYPH_SPECS,
        executables,
        sources,
        grp_archives,
        drac_text,
        btx_names,
        btx_cells,
    )?;
    if analysis.rows.len() != BOX_SUBGAME_SEQ_DRAC_GLYPH_ROWS {
        return Err(error(format!(
            "DRAC glyph 행 수 불일치: expected={}, actual={}",
            BOX_SUBGAME_SEQ_DRAC_GLYPH_ROWS,
            analysis.rows.len()
        )));
    }
    let report = render_subgame_seq_drac_glyphs_tsv(&analysis);
    let digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if digest != BOX_SUBGAME_SEQ_DRAC_GLYPH_REPORT_SHA256 {
        return Err(error(format!(
            "DRAC glyph report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_DRAC_GLYPH_REPORT_SHA256),
            hex(&digest)
        )));
    }
    Ok(analysis)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_drac_glyph_tests.rs"]
mod subgame_seq_drac_glyph_tests;
