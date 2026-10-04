//! `E6` direct text의 code index -> BTX cell 전략 경계.
//!
//! 두 overlay 집합의 text와 code table은 거의 같지만 cell 공급은 같지 않다.
//! PUYO2는 각 `E6` overlay가 `DEMOFONT.BTX` descriptor에 256-cell table을
//! 직접 연결한다. PUYO1의 같은 위치 table은 `P2RANK1.BTX`용이며, 실제
//! glyph object가 고르는 `CHR.BTX`와 다르다. PP1 전체에서 `CHR.BTX`를
//! 설정하는 유일한 `CAST_SEQ` table도 E6가 쓰는 index 범위를 덮지 못한다.
//! 이 모듈은 그 차이를 지원 원본에서 다시 검증하고, PUYO2만 정적 재삽입
//! 후보로 승격하며 PUYO1은 runtime 경로가 닫힐 때까지 no-write로 둔다.

use std::error::Error;
use std::fmt;

use crate::isa::Reg;

use super::subgame_seq_descriptor_ingress::DescriptorRootLiteral;

mod audit;
mod baseline;
mod binary_checks;
mod glyph_assets;
mod glyph_consumer;
mod puyo1_descriptor_route;
mod renderer_route;
mod report;
mod sequence_storage;

pub use audit::audit_subgame_seq_e6_glyphs;
pub use baseline::verify_box_subgame_seq_e6_glyphs;
pub use report::render_subgame_seq_e6_glyphs_tsv;

#[cfg(test)]
use crate::coords::ExeMap;
#[cfg(test)]
use crate::isa::{Instruction, InstructionKind as K};
#[cfg(test)]
use glyph_assets::{setup_records, valid_rect, verify_p1_candidate_coverage, verify_p2_table};
#[cfg(test)]
use puyo1_descriptor_route::{
    has_nearby_halfword_store, p1_async_dispatch_incoming_a0_uses, p1_memory_accesses_with_base,
    test_literal_non_stack_halfword_20_stores as literal_non_stack_halfword_20_stores,
    test_literal_resource_006d_loads as literal_resource_006d_loads,
    test_paired_descriptor_cell_count_writers as paired_descriptor_cell_count_writers,
};
#[cfg(test)]
use renderer_route::verify_zero_words_function;
#[cfg(test)]
use sequence_storage::e6_sequence_selection_records;
#[cfg(test)]
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::collections::BTreeSet;

pub const SUBGAME_SEQ_E6_GLYPH_SCHEMA_VERSION: &str = "subgame-seq-e6-glyph-v14";
pub const BOX_SUBGAME_SEQ_E6_GLYPH_ROWS: usize = 2;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_SEQUENCES_PER_SCOPE: usize = 33;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_USED_INDICES: usize = 210;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_MIN_INDEX: usize = 0;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_MAX_INDEX: usize = 246;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO1_TOKENS: usize = 1_284;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO2_TOKENS: usize = 1_283;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO1_ABOVE_CANDIDATE: usize = 124;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO1_CANDIDATE_VALID_PREFIX: usize = 91;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO1_CANDIDATE_VALID_USED: usize = 95;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO1_CANDIDATE_INVALID_USED: usize = 115;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO1_CANDIDATE_NONEMPTY_USED: usize = 89;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO1_CANDIDATE_EMPTY_USED: usize = 6;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO2_CELLS: usize = 256;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO2_NONEMPTY_CELLS: usize = 246;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO2_NON_E6_SETUPS: usize = 17;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO1_STORED_BYTES: usize = 70_363;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO1_PLAIN_BYTES: usize = 194_504;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO2_STORED_BYTES: usize = 77_608;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_PUYO2_PLAIN_BYTES: usize = 207_200;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_LOAD_BASE: u32 = 0x801B_5270;
pub const BOX_SUBGAME_SEQ_E6_GLYPH_REPORT_SHA256: [u8; 32] = [
    0xDE, 0x2A, 0x59, 0x9B, 0x11, 0x7B, 0x2C, 0x20, 0x99, 0xE5, 0x4A, 0xA5, 0x3B, 0x4A, 0x73, 0xF9,
    0xF5, 0x90, 0xD5, 0x04, 0x90, 0x18, 0x1A, 0x56, 0x7C, 0xCD, 0xE2, 0xCB, 0x95, 0x61, 0xDE, 0x52,
];

const E6_SEQUENCE_RESOURCE_FIRST: u16 = 0x003F;
const E6_SEQUENCE_RESOURCE_LAST: u16 = 0x005F;
const P1_E6_SELECTION_SOURCES: &[&str] = &[];
const P2_E6_SELECTION_SOURCES: &[&str] = &["PP2_FUTA.SEQ", "PP2_HITO.SEQ"];
const P1_E6_SELECTION_FLAGS: &[u8] = &[];
const P2_E6_SELECTION_FLAGS: &[u8] = &[0x97];

const DESCRIPTOR_BYTES: u32 = 36;
const CELL_HEADER_BYTES: usize = 12;
const CELL_BYTES: usize = 12;
const P2_CELL_TABLE_BYTES: usize =
    CELL_HEADER_BYTES + BOX_SUBGAME_SEQ_E6_GLYPH_PUYO2_CELLS * CELL_BYTES;
const CODE_TABLE_RUNTIME: u32 = 0x8001_00FC;
const CODE_TABLE_ENTRIES: usize = 256;
const CODE_TABLE_BYTES: usize = CODE_TABLE_ENTRIES * 2;
const SETUP_PREFIX: [u8; 4] = [0x28, 0x12, 0x97, 0x00];
const TERMINATOR: [u8; 2] = [0x65, 0x65];
const CONTROLS: [[u8; 2]; 3] = [[0x70, 0x70], [0x72, 0x72], [0x77, 0x24]];
const P2_TABLE_SHA256: &str = "c8527677c92fdca7425fa0123ff96bf8880e720ad36bb7c5c630e7e1abd92d4c";
const P1_CANDIDATE_PREFIX_SHA256: &str =
    "b773f33dc4717ac1ef5f9cb279ed8caf173e4e901902f48f511dbfff15e9f25d";
const P1_STATIC_INITIALIZER_RUNTIME: u32 = 0x8002_A2BC;
const P1_STATIC_INITIALIZER_CALLBACK_RUNTIME: u32 = 0x8008_9E3C;
const P1_STATIC_INITIALIZER_TABLE_RUNTIME: u32 = 0x8008_A45C;
const P1_STATIC_INITIALIZER_RESOURCE: u16 = 0x00B4;
const P1_STATIC_INITIALIZER_CALL_SITES: [u32; 2] = [0x8002_A524, 0x8002_A790];
const P1_STATIC_INITIALIZER_ENTRY_REACHED_CALL_SITES: [u32; 1] = [0x8002_A524];
const P1_LITERAL_HALFWORD_20_STORES: [u32; 12] = [
    0x8001_B184,
    0x8001_B1AC,
    0x8002_5540,
    0x8002_5BCC,
    0x8002_5DF0,
    0x8002_5E18,
    0x8002_5E48,
    0x8002_5E6C,
    0x8002_627C,
    0x8002_7018,
    0x8002_A2F8,
    0x8007_56A4,
];
const P1_GPU_PRIMITIVE_HALFWORD_20_STORES: [u32; 8] = [
    0x8002_5540,
    0x8002_5BCC,
    0x8002_5DF0,
    0x8002_5E18,
    0x8002_5E48,
    0x8002_5E6C,
    0x8002_627C,
    0x8002_7018,
];
const P1_CACHE_RECORD_HALFWORD_20_STORE: u32 = 0x8007_56A4;
const P1_LITERAL_RESOURCE_006D_LOADS: [u32; 4] =
    [0x8001_84E4, 0x8001_8DCC, 0x8001_8EC8, 0x8005_CF2C];
const P1_DIRECT_CHR_OBJECT_RESOURCE_STORES: [u32; 3] = [0x8001_84E8, 0x8001_8DD0, 0x8001_8ECC];
const P1_NON_OBJECT_LITERAL_006D_LOADS: [u32; 1] = [0x8005_CF2C];
const P1_ALLOCATOR_DIRECT_CALLS: usize = 447;
const P1_ALLOCATOR_IMMEDIATE_TYPE_CALLS: usize = 242;
const P1_ALLOCATOR_DYNAMIC_TYPE_CALLS: usize = 205;
const P1_IMMEDIATE_TYPE9_CALLS: [u32; 4] = [0x8001_84C0, 0x8001_8DA8, 0x8001_8E9C, 0x8001_8F14];
const P1_DIRECT_CHR_OBJECT_ALLOCATOR_CALLS: [u32; 3] = [0x8001_84C0, 0x8001_8DA8, 0x8001_8E9C];
const P1_INACTIVE_TYPE9_CALLS: [u32; 1] = [0x8001_8F14];
const P1_DESCRIPTOR_ROOT_LITERAL_COMBINES: [u32; 15] = [
    0x8001_AD34,
    0x8001_AD38,
    0x8001_AEE4,
    0x8001_AEE8,
    0x8001_B5C0,
    0x8001_C9F0,
    0x8001_CCA0,
    0x8001_CD58,
    0x8001_D5CC,
    0x8001_D69C,
    0x8001_D700,
    0x8001_DACC,
    0x8002_434C,
    0x8002_4528,
    0x8005_8A08,
];
const P1_DESCRIPTOR_ROOT_RESOURCE_X36_SELECTORS: [u32; 11] = [
    0x8001_AD38,
    0x8001_AEE8,
    0x8001_C9F0,
    0x8001_CCA0,
    0x8001_CD58,
    0x8001_D5CC,
    0x8001_D69C,
    0x8001_D700,
    0x8001_DACC,
    0x8002_4528,
    0x8005_8A08,
];
const P1_DESCRIPTOR_ROOT_ALTERNATE_BRANCH_COMBINES: [u32; 2] = [0x8001_AD34, 0x8001_AEE4];
const P1_DESCRIPTOR_ROOT_REGISTRATION_COMBINES: [u32; 2] = [0x8001_B5C0, 0x8002_434C];
const P1_DESCRIPTOR_ROOT_REGISTRATION_STORES: [u32; 2] = [0x8001_B5CC, 0x8002_4360];
const P1_DESCRIPTOR_ROOT_LITERAL: DescriptorRootLiteral =
    DescriptorRootLiteral::new(0x800B, 0x4330);
const P1_DESCRIPTOR_TABLE_RECORDS: usize = 185;
const P1_DESCRIPTOR_TABLE_END_RUNTIME: u32 = 0x8006_984C;
const P1_DESCRIPTOR_TABLE_SHA256: &str =
    "3099a7e30d5d11606c04ebcdbd6c14433efa43e7c6a1e2baceae96fbb7a374cb";
const P1_DESCRIPTOR_TABLE_ADDRESS_LOADS: [u32; 2] = [0x8002_A520, 0x8002_A778];
const P1_DESCRIPTOR_TABLE_RAW_POINTER_LOCATIONS: [u32; 2] = [0x8006_7B4C, 0x8006_7B54];
const P1_DESCRIPTOR_SELECTOR_DIRECT_CALLEE_CALLS: [u32; 7] = [
    0x8001_AD60,
    0x8001_AF94,
    0x8001_CD8C,
    0x8001_D5F0,
    0x8001_D6C0,
    0x8001_D724,
    0x8005_8A34,
];
const P1_DESCRIPTOR_SELECTOR_LOCAL_CONSUMERS: [u32; 4] =
    [0x8001_CA20, 0x8001_CCCC, 0x8001_DAF8, 0x8002_4558];
const P1_DESCRIPTOR_SELECTOR_UNIQUE_CALLEES: [u32; 5] = [
    0x8001_C77C,
    0x8001_C968,
    0x8001_CC50,
    0x8001_CD28,
    0x8001_D2CC,
];
const P1_DESCRIPTOR_FORWARDING_CALLS: [u32; 4] =
    [0x8001_C7A0, 0x8001_C7F8, 0x8001_C834, 0x8001_C850];
const P1_DESCRIPTOR_FORWARDING_UNIQUE_CALLEES: [u32; 4] =
    [0x8001_C824, 0x8001_CC50, 0x8001_D25C, 0x8001_D3BC];
const P1_DESCRIPTOR_SYNCHRONOUS_FIELD_READS: [u32; 25] = [
    0x8001_ADF0,
    0x8001_AE38,
    0x8001_B04C,
    0x8001_B0BC,
    0x8001_B10C,
    0x8001_B118,
    0x8001_B198,
    0x8001_B1A4,
    0x8001_C97C,
    0x8001_C9A4,
    0x8001_CA20,
    0x8001_CA48,
    0x8001_CC64,
    0x8001_CCCC,
    0x8001_CD34,
    0x8001_CDF0,
    0x8001_CE2C,
    0x8001_D25C,
    0x8001_D294,
    0x8001_D2CC,
    0x8001_D304,
    0x8001_D404,
    0x8001_D410,
    0x8001_DAF8,
    0x8002_4558,
];
const P1_DESCRIPTOR_SYNCHRONOUS_FIELD_WRITES: [u32; 9] = [
    0x8001_ADAC,
    0x8001_AFE0,
    0x8001_B0E4,
    0x8001_B0F8,
    0x8001_B170,
    0x8001_B184,
    0x8001_CDBC,
    0x8001_D280,
    0x8001_D2F0,
];
const P1_DESCRIPTOR_SINGLETON_SNAPSHOT_ROOT_COMBINES: [u32; 4] =
    [0x8001_AF24, 0x8001_B110, 0x8001_B19C, 0x8001_D884];
const P1_DESCRIPTOR_TYPE_SNAPSHOT_ROOT_COMBINES: [u32; 2] = [0x8001_D810, 0x8001_D928];
const P1_DESCRIPTOR_PAYLOAD_COPY_LOOPS: [u32; 2] = [0x8001_AF30, 0x8001_D88C];
const P1_DESCRIPTOR_SNAPSHOT_FIELD_WRITES: [u32; 4] =
    [0x8001_B114, 0x8001_B120, 0x8001_B1A0, 0x8001_B1AC];
const P1_DESCRIPTOR_TYPE_SNAPSHOT_LINK_STORES: [u32; 2] = [0x8001_D8C4, 0x8001_D8C8];
const P1_DESCRIPTOR_POINTER_ESCAPE_STORES: [u32; 3] = [0x8001_AF74, 0x8001_D27C, 0x8001_D2EC];
const P1_DESCRIPTOR_CACHE_LINK_STORES: [u32; 2] = [0x8001_D280, 0x8001_D2F0];
const P1_DESCRIPTOR_FIXED_CACHE_CALLBACKS: [u32; 5] = [
    0x8001_CF2C,
    0x8001_D048,
    0x8001_D0EC,
    0x8001_D114,
    0x8001_D150,
];
const P1_DESCRIPTOR_FIXED_CALLBACK_WRITES: [u32; 6] = [
    0x8001_D024,
    0x8001_D084,
    0x8001_D0A4,
    0x8001_D108,
    0x8001_D138,
    0x8001_D244,
];
const P1_DESCRIPTOR_FIXED_CALLBACK_CELL_READS: [u32; 3] = [0x8001_D180, 0x8001_D194, 0x8001_D1A8];
const P1_DESCRIPTOR_SINGLETON_CALLBACK_WRITES: [u32; 5] = [
    0x8001_AF84,
    0x8001_AF90,
    0x8001_B0A0,
    0x8001_B140,
    0x8001_B1C8,
];
const P1_DESCRIPTOR_CACHE_CALLBACKS: [u32; 6] = [
    0x8001_B280,
    0x8001_CF2C,
    0x8001_D048,
    0x8001_D0EC,
    0x8001_D114,
    0x8001_D150,
];
const P1_DESCRIPTOR_CALLBACK_COPY_LOOPS: [u32; 2] = [0x8001_D840, 0x8001_D95C];
const P1_DESCRIPTOR_COMPLETION_CALLBACK_FIELD_WRITES: [u32; 2] = [0x8001_B350, 0x8001_B39C];
const P1_DESCRIPTOR_COMPLETION_DESCRIPTOR_WRITES: [u32; 2] = [0x8001_B334, 0x8001_B38C];
const P1_DESCRIPTOR_TYPE_CACHE_ROOT_COMBINES: [u32; 3] = [0x8001_D834, 0x8001_D904, 0x8001_DA5C];
const P1_ASYNC_DISPATCH_TABLE_RUNTIME: u32 = 0x8001_03B0;
const P1_ASYNC_DISPATCH_RUNTIME: u32 = 0x8001_B600;
const P1_ASYNC_DISPATCH_END_RUNTIME: u32 = 0x8001_BD84;
const P1_ASYNC_DISPATCH_TARGETS: [u32; 35] = [
    0x8001_B648,
    0x8001_B65C,
    0x8001_B6B4,
    0x8001_B708,
    0x8001_B6E8,
    0x8001_B708,
    0x8001_B718,
    0x8001_B770,
    0x8001_B744,
    0x8001_B770,
    0x8001_B780,
    0x8001_B7C0,
    0x8001_B7E4,
    0x8001_B844,
    0x8001_B818,
    0x8001_B844,
    0x8001_B854,
    0x8001_B8B0,
    0x8001_B8D4,
    0x8001_B970,
    0x8001_B910,
    0x8001_B94C,
    0x8001_B970,
    0x8001_B980,
    0x8001_BB6C,
    0x8001_B9B4,
    0x8001_BB6C,
    0x8001_B9E0,
    0x8001_BA54,
    0x8001_BA7C,
    0x8001_BB6C,
    0x8001_BABC,
    0x8001_BAF8,
    0x8001_BB6C,
    0x8001_BB7C,
];
const P1_STATIC_INITIALIZER_TABLE_PREFIX_SHA256: &str =
    "a7fcf3a988617db87e38d75dccd3d416f892e8bf9f11073bbd43672d1e7ada8b";
const OBJECT_WORDS: i16 = 0x003B;
const OBJECT_POOL_COUNT: i16 = 0x01F3;
const OBJECT_POOL_ZERO_WORDS: i16 = 0x733C;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct E6ObjectRendererRouteSpec {
    scope: &'static str,
    allocator_call_runtime: u32,
    allocator_runtime: u32,
    allocator_type: i16,
    callback_high_runtime: u32,
    callback_low_runtime: u32,
    callback_runtime: u32,
    callback_release_call_runtime: u32,
    active_store_runtime: u32,
    active_value_reg: Reg,
    resource_value_runtime: u32,
    resource_store_runtime: u32,
    cell_store_runtime: u32,
    pool_init_runtime: u32,
    pool_runtime: u32,
    zero_words_runtime: u32,
    free_head_gp_offset: i16,
    type_flags_runtime: u32,
    object_list_runtime: u32,
    release_runtime: u32,
    render_dispatcher_runtime: u32,
    short_renderer_call_runtime: u32,
}

const E6_OBJECT_RENDERER_ROUTE_SPECS: [E6ObjectRendererRouteSpec; 2] = [
    E6ObjectRendererRouteSpec {
        scope: "PUYO1",
        allocator_call_runtime: 0x8001_8E9C,
        allocator_runtime: 0x8001_53EC,
        allocator_type: 9,
        callback_high_runtime: 0x8001_8E94,
        callback_low_runtime: 0x8001_8E98,
        callback_runtime: 0x8001_8D6C,
        callback_release_call_runtime: 0x8001_8D80,
        active_store_runtime: 0x8001_8EA8,
        active_value_reg: Reg::V1,
        resource_value_runtime: 0x8001_8EC8,
        resource_store_runtime: 0x8001_8ECC,
        cell_store_runtime: 0x8001_8ED4,
        pool_init_runtime: 0x8001_52B0,
        pool_runtime: 0x8009_6020,
        zero_words_runtime: 0x8006_4054,
        free_head_gp_offset: 0x03B0,
        type_flags_runtime: 0x800B_2D10,
        object_list_runtime: 0x800B_2D50,
        release_runtime: 0x8001_55A8,
        render_dispatcher_runtime: 0x8002_4750,
        short_renderer_call_runtime: 0x8002_4A50,
    },
    E6ObjectRendererRouteSpec {
        scope: "PUYO2",
        allocator_call_runtime: 0x8001_6E5C,
        allocator_runtime: 0x8001_4CF0,
        allocator_type: 4,
        callback_high_runtime: 0x8001_6E54,
        callback_low_runtime: 0x8001_6E58,
        callback_runtime: 0x8001_79C8,
        callback_release_call_runtime: 0x8001_79F8,
        active_store_runtime: 0x8001_6E68,
        active_value_reg: Reg::A0,
        resource_value_runtime: 0x8001_6E7C,
        resource_store_runtime: 0x8001_6E84,
        cell_store_runtime: 0x8001_6E88,
        pool_init_runtime: 0x8001_4BB4,
        pool_runtime: 0x8009_82F8,
        zero_words_runtime: 0x8006_02DC,
        free_head_gp_offset: 0x03E4,
        type_flags_runtime: 0x800B_4FE8,
        object_list_runtime: 0x800B_5028,
        release_runtime: 0x8001_4EAC,
        render_dispatcher_runtime: 0x8002_3798,
        short_renderer_call_runtime: 0x8002_3A8C,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqE6GlyphSpec {
    pub scope: &'static str,
    pub exe: &'static str,
    pub seq_archive: &'static str,
    pub grp_archive: &'static str,
    pub seq_descriptor_base_runtime: u32,
    pub seq_descriptor_records: usize,
    pub expected_e6_selection_records_per_resource: usize,
    pub expected_e6_selection_sources: &'static [&'static str],
    pub expected_e6_selection_flags: &'static [u8],
    pub glyph_resource: u16,
    pub glyph_asset: &'static str,
    pub glyph_vdd_index: usize,
    pub glyph_stored_bytes: u32,
    pub glyph_plain_bytes: usize,
    pub glyph_plain_sha256: &'static str,
    pub descriptor_base_runtime: u32,
    pub descriptor_root_runtime: u32,
    pub descriptor_root_lui_runtime: u32,
    pub descriptor_base_lui_runtime: u32,
    pub descriptor_base_addiu_runtime: u32,
    pub descriptor_root_store_runtime: u32,
    pub renderer_runtime: u32,
    pub renderer_resource_load_runtime: u32,
    pub renderer_cell_load_runtime: u32,
    pub renderer_descriptor_root_load_runtime: u32,
    pub renderer_cell_table_load_runtime: u32,
    pub renderer_cell_entry_runtime: u32,
    pub opcode_28_slot_runtime: u32,
    pub opcode_28_wrapper_runtime: u32,
    pub subdispatcher_runtime: u32,
    pub subdispatch_table_runtime: u32,
    pub cell_setup_handler_runtime: u32,
    pub cell_table_pointer_load_runtime: u32,
    pub descriptor_cell_table_store_runtime: u32,
    pub cell_count_load_runtime: u32,
    pub descriptor_cell_count_store_runtime: u32,
    pub expected_all_glyph_setups: usize,
    pub expected_e6_glyph_setups: usize,
    pub expected_table_offset: Option<usize>,
    pub rejected_resource: Option<u16>,
    pub rejected_asset: Option<&'static str>,
    pub rejected_vdd_index: Option<usize>,
    pub rejected_stored_bytes: Option<u32>,
    pub rejected_plain_sha256: Option<&'static str>,
    pub rejected_table_offset: Option<usize>,
    pub candidate_sequence: Option<&'static str>,
    pub candidate_setup_offset: Option<usize>,
    pub candidate_table_offset: Option<usize>,
}

pub const BOX_SUBGAME_SEQ_E6_GLYPH_SPECS: [SubgameSeqE6GlyphSpec; 2] = [
    SubgameSeqE6GlyphSpec {
        scope: "PUYO1",
        exe: "PUYO1.EXE",
        seq_archive: "/PUYO1/PP1_SEQ.VDD;1",
        grp_archive: "/PUYO1/PP1_GRP.VDD;1",
        seq_descriptor_base_runtime: 0x8006_9EEC,
        seq_descriptor_records: 137,
        expected_e6_selection_records_per_resource: 0,
        expected_e6_selection_sources: P1_E6_SELECTION_SOURCES,
        expected_e6_selection_flags: P1_E6_SELECTION_FLAGS,
        glyph_resource: 0x006D,
        glyph_asset: "CHR.BTX",
        glyph_vdd_index: 71,
        glyph_stored_bytes: 41_000,
        glyph_plain_bytes: 116_928,
        glyph_plain_sha256: "152dc02696d139f4c06f09920156c5b83d54c11d15c15eb9f4441031edabe3ee",
        descriptor_base_runtime: 0x8006_7E48,
        descriptor_root_runtime: 0x800B_4330,
        descriptor_root_lui_runtime: 0x8002_A518,
        descriptor_base_lui_runtime: 0x8002_A51C,
        descriptor_base_addiu_runtime: 0x8002_A520,
        descriptor_root_store_runtime: 0x8002_A528,
        renderer_runtime: 0x8002_4AB4,
        renderer_resource_load_runtime: 0x8002_4AE8,
        renderer_cell_load_runtime: 0x8002_4B00,
        renderer_descriptor_root_load_runtime: 0x8002_4AFC,
        renderer_cell_table_load_runtime: 0x8002_4B10,
        renderer_cell_entry_runtime: 0x8002_4B18,
        opcode_28_slot_runtime: 0x8008_9790,
        opcode_28_wrapper_runtime: 0x8002_4314,
        subdispatcher_runtime: 0x8001_C6F8,
        subdispatch_table_runtime: 0x8008_954C,
        cell_setup_handler_runtime: 0x8001_AEA4,
        cell_table_pointer_load_runtime: 0x8001_B168,
        descriptor_cell_table_store_runtime: 0x8001_B170,
        cell_count_load_runtime: 0x8001_B17C,
        descriptor_cell_count_store_runtime: 0x8001_B184,
        expected_all_glyph_setups: 1,
        expected_e6_glyph_setups: 0,
        expected_table_offset: None,
        rejected_resource: Some(0x007A),
        rejected_asset: Some("P2RANK1.BTX"),
        rejected_vdd_index: Some(84),
        rejected_stored_bytes: Some(19_588),
        rejected_plain_sha256: Some(
            "dd21fab7753df7ccc381034b8ce0078962531b0cc30e19bc39ae7bcaaf52889f",
        ),
        rejected_table_offset: Some(0x0008),
        candidate_sequence: Some("CAST_SEQ.SEQ"),
        candidate_setup_offset: Some(0x1E54),
        candidate_table_offset: Some(0x12F4),
    },
    SubgameSeqE6GlyphSpec {
        scope: "PUYO2",
        exe: "PUYO2.EXE",
        seq_archive: "/PUYO2/PP2_SEQ.VDD;1",
        grp_archive: "/PUYO2/PP2_GRP.VDD;1",
        seq_descriptor_base_runtime: 0x8006_6824,
        seq_descriptor_records: 202,
        expected_e6_selection_records_per_resource: 2,
        expected_e6_selection_sources: P2_E6_SELECTION_SOURCES,
        expected_e6_selection_flags: P2_E6_SELECTION_FLAGS,
        glyph_resource: 0x0088,
        glyph_asset: "DEMOFONT.BTX",
        glyph_vdd_index: 133,
        glyph_stored_bytes: 9_832,
        glyph_plain_bytes: 65_536,
        glyph_plain_sha256: "ee51306e0d53875b05b3d7004b1d9900379db02862e1d54e888e27658c4970ac",
        descriptor_base_runtime: 0x8006_3E9C,
        descriptor_root_runtime: 0x800B_6710,
        descriptor_root_lui_runtime: 0x8002_27D8,
        descriptor_base_lui_runtime: 0x8002_27E0,
        descriptor_base_addiu_runtime: 0x8002_27E4,
        descriptor_root_store_runtime: 0x8002_27F0,
        renderer_runtime: 0x8002_3AF0,
        renderer_resource_load_runtime: 0x8002_3B24,
        renderer_cell_load_runtime: 0x8002_3B3C,
        renderer_descriptor_root_load_runtime: 0x8002_3B38,
        renderer_cell_table_load_runtime: 0x8002_3B4C,
        renderer_cell_entry_runtime: 0x8002_3B54,
        opcode_28_slot_runtime: 0x8008_B7E0,
        opcode_28_wrapper_runtime: 0x8002_3364,
        subdispatcher_runtime: 0x8001_B184,
        subdispatch_table_runtime: 0x8008_B59C,
        cell_setup_handler_runtime: 0x8001_9930,
        cell_table_pointer_load_runtime: 0x8001_9BF4,
        descriptor_cell_table_store_runtime: 0x8001_9BFC,
        cell_count_load_runtime: 0x8001_9C08,
        descriptor_cell_count_store_runtime: 0x8001_9C10,
        expected_all_glyph_setups: 50,
        expected_e6_glyph_setups: 33,
        expected_table_offset: Some(0x0010),
        rejected_resource: None,
        rejected_asset: None,
        rejected_vdd_index: None,
        rejected_stored_bytes: None,
        rejected_plain_sha256: None,
        rejected_table_offset: None,
        candidate_sequence: None,
        candidate_setup_offset: None,
        candidate_table_offset: None,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6StorageLink {
    pub resource: u16,
    pub vdd_index: usize,
    pub sequence: String,
    pub stored_bytes: usize,
    pub plain_bytes: usize,
    pub sequence_sha256: [u8; 32],
    pub load_base: u32,
    pub initialized_bytes: usize,
    pub bss_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6GlyphRow {
    pub scope: &'static str,
    pub exe: &'static str,
    pub archive: &'static str,
    pub glyph_resource: u16,
    pub glyph_asset: &'static str,
    pub descriptor_runtime: u32,
    pub glyph_plain_bytes: usize,
    pub glyph_plain_sha256: String,
    pub physical_glyph_cells: usize,
    pub e6_sequences: usize,
    pub e6_sequence_resource_ids: Vec<u16>,
    pub e6_sequence_selection_records: usize,
    pub e6_sequence_selected_resources: usize,
    pub e6_sequence_selection_sources: Vec<String>,
    pub e6_sequence_selection_flags: Vec<u8>,
    pub e6_sequence_selection_sites: Vec<String>,
    pub e6_sequence_vdd_entries: usize,
    pub e6_sequence_descriptor_rows: usize,
    pub e6_sequence_cnx_payloads: usize,
    pub e6_sequence_overlay_rows: usize,
    pub e6_sequence_stored_bytes: usize,
    pub e6_sequence_plain_bytes: usize,
    pub e6_sequence_load_bases: Vec<u32>,
    pub e6_sequence_storage_links: Vec<SubgameSeqE6StorageLink>,
    pub e6_sequence_storage_verdict: &'static str,
    pub glyph_tokens: usize,
    pub used_cell_indices: usize,
    pub min_cell_index: usize,
    pub max_cell_index: usize,
    pub all_glyph_setup_records: usize,
    pub e6_glyph_setup_records: usize,
    pub e6_table_copies: usize,
    pub verified_e6_cells: usize,
    pub verified_e6_nonempty_cells: usize,
    pub e6_table_offset: Option<usize>,
    pub e6_table_sha256: Option<String>,
    pub non_e6_glyph_setup_records: usize,
    pub rejected_resource: Option<u16>,
    pub rejected_asset: Option<&'static str>,
    pub rejected_setup_records: usize,
    pub rejected_table_offset: Option<usize>,
    pub rejected_table_sha256: Option<String>,
    pub candidate_sequence: Option<&'static str>,
    pub candidate_setup_offset: Option<usize>,
    pub candidate_table_offset: Option<usize>,
    pub candidate_valid_prefix: usize,
    pub used_indices_above_candidate: usize,
    pub candidate_used_valid_cells: usize,
    pub candidate_used_invalid_cells: usize,
    pub candidate_used_nonempty_cells: usize,
    pub candidate_used_empty_cells: usize,
    pub candidate_invalid_used_indices: Vec<usize>,
    pub candidate_empty_used_indices: Vec<usize>,
    pub paired_cell_count_writers: usize,
    pub literal_halfword_20_stores: Vec<u32>,
    pub descriptor_literal_halfword_20_stores: Vec<u32>,
    pub gpu_primitive_halfword_20_stores: Vec<u32>,
    pub cache_record_halfword_20_stores: Vec<u32>,
    pub unclassified_literal_halfword_20_stores: usize,
    pub literal_halfword_20_verdict: &'static str,
    pub literal_resource_006d_loads: Vec<u32>,
    pub direct_chr_object_resource_stores: Vec<u32>,
    pub fixed_cell_chr_object_routes: usize,
    pub variable_cell_chr_object_routes: usize,
    pub non_object_literal_006d_loads: Vec<u32>,
    pub literal_resource_006d_verdict: &'static str,
    pub allocator_direct_calls: usize,
    pub allocator_immediate_type_calls: usize,
    pub allocator_dynamic_type_calls: usize,
    pub immediate_type9_calls: Vec<u32>,
    pub direct_chr_object_allocator_calls: Vec<u32>,
    pub inactive_type9_calls: Vec<u32>,
    pub immediate_type9_verdict: &'static str,
    pub descriptor_root_literal_combines: Vec<u32>,
    pub descriptor_root_resource_x36_selectors: Vec<u32>,
    pub descriptor_root_alternate_branch_combines: Vec<u32>,
    pub descriptor_root_registration_stores: Vec<u32>,
    pub unclassified_descriptor_root_combines: usize,
    pub chr_descriptor_direct_address_loads: usize,
    pub chr_descriptor_raw_pointer_words: usize,
    pub descriptor_table_records: usize,
    pub descriptor_table_sha256: Option<String>,
    pub descriptor_table_address_loads: Vec<u32>,
    pub descriptor_table_preadjusted_address_loads: usize,
    pub descriptor_table_raw_pointer_locations: Vec<u32>,
    pub descriptor_table_preadjusted_raw_pointers: usize,
    pub descriptor_root_census_verdict: &'static str,
    pub descriptor_selector_direct_callee_calls: Vec<u32>,
    pub descriptor_selector_local_consumers: Vec<u32>,
    pub descriptor_selector_unique_callees: Vec<u32>,
    pub descriptor_forwarding_calls: Vec<u32>,
    pub descriptor_forwarding_unique_callees: Vec<u32>,
    pub descriptor_synchronous_field_reads: Vec<u32>,
    pub descriptor_synchronous_field_writes: Vec<u32>,
    pub descriptor_pointer_escape_stores: Vec<u32>,
    pub descriptor_cache_link_stores: Vec<u32>,
    pub descriptor_fixed_cache_callbacks: Vec<u32>,
    pub descriptor_fixed_callback_writes: Vec<u32>,
    pub descriptor_fixed_callback_cell_reads: Vec<u32>,
    pub descriptor_fixed_callback_cell_writes: usize,
    pub descriptor_async_dispatch_targets: usize,
    pub descriptor_async_dispatch_incoming_uses: usize,
    pub descriptor_singleton_callback_writes: Vec<u32>,
    pub descriptor_cache_callbacks: Vec<u32>,
    pub descriptor_callback_copy_loops: Vec<u32>,
    pub descriptor_completion_callback_field_writes: Vec<u32>,
    pub descriptor_completion_descriptor_writes: Vec<u32>,
    pub descriptor_completion_callback_cell_reads: usize,
    pub descriptor_completion_callback_cell_writes: usize,
    pub descriptor_type_cache_root_combines: Vec<u32>,
    pub descriptor_singleton_snapshot_root_combines: Vec<u32>,
    pub descriptor_type_snapshot_root_combines: Vec<u32>,
    pub descriptor_payload_copy_loops: Vec<u32>,
    pub descriptor_snapshot_field_writes: Vec<u32>,
    pub descriptor_type_snapshot_link_stores: Vec<u32>,
    pub descriptor_snapshot_raw_pointer_words: usize,
    pub descriptor_alias_census_verdict: &'static str,
    pub setup_source_cell_writer_runtime: Option<u32>,
    pub cache_mirror_cell_writer_runtime: Option<u32>,
    pub static_initializer_runtime: Option<u32>,
    pub static_initializer_cell_writer_runtime: Option<u32>,
    pub static_initializer_direct_call_sites: Vec<u32>,
    pub static_initializer_entry_reached_call_sites: Vec<u32>,
    pub static_initializer_data_refs: usize,
    pub static_initializer_raw_pointer_words: usize,
    pub static_initializer_resource: Option<u16>,
    pub static_initializer_descriptor_runtime: Option<u32>,
    pub static_initializer_registered_assets: usize,
    pub static_initializer_cell_table_runtime: Option<u32>,
    pub static_initializer_cell_count: Option<u16>,
    pub static_initializer_cell_width: Option<u16>,
    pub static_initializer_cell_height: Option<u16>,
    pub alternate_static_route_verdict: &'static str,
    pub glyph_object_allocator_runtime: u32,
    pub glyph_object_allocator_type: i16,
    pub glyph_object_callback_runtime: u32,
    pub object_pool_init_runtime: u32,
    pub object_release_runtime: u32,
    pub object_type_flags_runtime: u32,
    pub object_list_runtime: u32,
    pub render_dispatcher_runtime: u32,
    pub render_dispatcher_short_call_runtime: u32,
    pub render_dispatcher_entry_reached: bool,
    pub renderer_route_verdict: &'static str,
    pub static_verdict: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6GlyphAnalysis {
    pub rows: Vec<SubgameSeqE6GlyphRow>,
    pub common_used_cell_index_values: Vec<usize>,
    pub common_used_cell_indices: usize,
    pub common_min_cell_index: usize,
    pub common_max_cell_index: usize,
    pub p2_static_ready_rows: usize,
    pub p1_no_write_rows: usize,
    pub renderer_route_rows: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqE6GlyphError(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DescriptorCellCountWriter {
    cell_store_runtime: u32,
    count_store_runtime: u32,
    base: Reg,
    cell_source: Reg,
    count_source: Reg,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct P1StaticRoute {
    paired_writers: usize,
    literal_halfword_20_stores: Vec<u32>,
    descriptor_literal_halfword_20_stores: Vec<u32>,
    gpu_primitive_halfword_20_stores: Vec<u32>,
    cache_record_halfword_20_stores: Vec<u32>,
    literal_resource_006d_loads: Vec<u32>,
    direct_chr_object_resource_stores: Vec<u32>,
    fixed_cell_chr_object_routes: usize,
    variable_cell_chr_object_routes: usize,
    non_object_literal_006d_loads: Vec<u32>,
    allocator_direct_calls: usize,
    allocator_immediate_type_calls: usize,
    allocator_dynamic_type_calls: usize,
    immediate_type9_calls: Vec<u32>,
    direct_chr_object_allocator_calls: Vec<u32>,
    inactive_type9_calls: Vec<u32>,
    descriptor_root_literal_combines: Vec<u32>,
    descriptor_root_resource_x36_selectors: Vec<u32>,
    descriptor_root_alternate_branch_combines: Vec<u32>,
    descriptor_root_registration_stores: Vec<u32>,
    chr_descriptor_direct_address_loads: usize,
    chr_descriptor_raw_pointer_words: usize,
    descriptor_table_records: usize,
    descriptor_table_sha256: String,
    descriptor_table_address_loads: Vec<u32>,
    descriptor_table_preadjusted_address_loads: usize,
    descriptor_table_raw_pointer_locations: Vec<u32>,
    descriptor_table_preadjusted_raw_pointers: usize,
    descriptor_selector_direct_callee_calls: Vec<u32>,
    descriptor_selector_local_consumers: Vec<u32>,
    descriptor_selector_unique_callees: Vec<u32>,
    descriptor_forwarding_calls: Vec<u32>,
    descriptor_forwarding_unique_callees: Vec<u32>,
    descriptor_synchronous_field_reads: Vec<u32>,
    descriptor_synchronous_field_writes: Vec<u32>,
    descriptor_pointer_escape_stores: Vec<u32>,
    descriptor_cache_link_stores: Vec<u32>,
    descriptor_fixed_cache_callbacks: Vec<u32>,
    descriptor_fixed_callback_writes: Vec<u32>,
    descriptor_fixed_callback_cell_reads: Vec<u32>,
    descriptor_async_dispatch_targets: usize,
    descriptor_async_dispatch_incoming_uses: usize,
    descriptor_singleton_callback_writes: Vec<u32>,
    descriptor_cache_callbacks: Vec<u32>,
    descriptor_callback_copy_loops: Vec<u32>,
    descriptor_completion_callback_field_writes: Vec<u32>,
    descriptor_completion_descriptor_writes: Vec<u32>,
    descriptor_type_cache_root_combines: Vec<u32>,
    descriptor_singleton_snapshot_root_combines: Vec<u32>,
    descriptor_type_snapshot_root_combines: Vec<u32>,
    descriptor_payload_copy_loops: Vec<u32>,
    descriptor_snapshot_field_writes: Vec<u32>,
    descriptor_type_snapshot_link_stores: Vec<u32>,
    descriptor_snapshot_raw_pointer_words: usize,
    setup_source_cell_writer_runtime: u32,
    cache_mirror_cell_writer_runtime: u32,
    initializer_cell_writer_runtime: u32,
    direct_call_sites: Vec<u32>,
    entry_reached_call_sites: Vec<u32>,
    data_refs: usize,
    raw_pointer_words: usize,
    descriptor_runtime: u32,
    registered_assets: usize,
    cell_count: u16,
    cell_width: u16,
    cell_height: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct P1AllocatorCensus {
    direct_calls: usize,
    immediate_type_calls: usize,
    dynamic_type_calls: usize,
    immediate_type9_calls: Vec<u32>,
    direct_chr_object_calls: Vec<u32>,
    inactive_type9_calls: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct P1DescriptorRootCensus {
    literal_combines: Vec<u32>,
    resource_x36_selectors: Vec<u32>,
    alternate_branch_combines: Vec<u32>,
    registration_stores: Vec<u32>,
    chr_descriptor_direct_address_loads: usize,
    chr_descriptor_raw_pointer_words: usize,
    descriptor_table_records: usize,
    descriptor_table_sha256: String,
    descriptor_table_address_loads: Vec<u32>,
    descriptor_table_preadjusted_address_loads: usize,
    descriptor_table_raw_pointer_locations: Vec<u32>,
    descriptor_table_preadjusted_raw_pointers: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct P1DescriptorAliasCensus {
    direct_callee_calls: Vec<u32>,
    local_consumers: Vec<u32>,
    unique_callees: Vec<u32>,
    forwarding_calls: Vec<u32>,
    forwarding_unique_callees: Vec<u32>,
    synchronous_field_reads: Vec<u32>,
    synchronous_field_writes: Vec<u32>,
    pointer_escape_stores: Vec<u32>,
    cache_link_stores: Vec<u32>,
    fixed_cache_callbacks: Vec<u32>,
    fixed_callback_writes: Vec<u32>,
    fixed_callback_cell_reads: Vec<u32>,
    async_dispatch_targets: usize,
    async_dispatch_incoming_uses: usize,
    singleton_callback_writes: Vec<u32>,
    cache_callbacks: Vec<u32>,
    callback_copy_loops: Vec<u32>,
    completion_callback_field_writes: Vec<u32>,
    completion_descriptor_writes: Vec<u32>,
    type_cache_root_combines: Vec<u32>,
    singleton_snapshot_root_combines: Vec<u32>,
    type_snapshot_root_combines: Vec<u32>,
    payload_copy_loops: Vec<u32>,
    snapshot_field_writes: Vec<u32>,
    type_snapshot_link_stores: Vec<u32>,
    snapshot_raw_pointer_words: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct P1DescriptorCallbackLineageCensus {
    singleton_callback_writes: Vec<u32>,
    cache_callbacks: Vec<u32>,
    callback_copy_loops: Vec<u32>,
    completion_callback_field_writes: Vec<u32>,
    completion_descriptor_writes: Vec<u32>,
    type_cache_root_combines: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct P1DescriptorPayloadCloneCensus {
    singleton_snapshot_root_combines: Vec<u32>,
    type_snapshot_root_combines: Vec<u32>,
    payload_copy_loops: Vec<u32>,
    snapshot_field_writes: Vec<u32>,
    type_snapshot_link_stores: Vec<u32>,
    snapshot_raw_pointer_words: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct P1CandidateCoverage {
    valid_cells: usize,
    invalid_indices: Vec<usize>,
    nonempty_cells: usize,
    empty_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct E6ObjectRendererRoute {
    spec: E6ObjectRendererRouteSpec,
    dispatcher_entry_reached: bool,
}

#[derive(Debug, Clone, Copy)]
struct GlyphAssetContract {
    resource: u16,
    name: &'static str,
    vdd_index: usize,
    stored_bytes: u32,
    plain_bytes: usize,
    plain_sha256: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct SetupRecord {
    offset: usize,
    resource: u16,
    count: u16,
    reserved: u32,
    pointer: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct E6SequenceSelectionRecord {
    sequence: String,
    offset: usize,
    flags: u8,
    resource: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct E6SequenceSelectionCensus {
    resources: Vec<u16>,
    records: Vec<E6SequenceSelectionRecord>,
    selected_resources: Vec<u16>,
    sources: Vec<String>,
    flags: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct E6SequenceStorageCensus {
    vdd_entries: usize,
    descriptor_rows: usize,
    cnx_payloads: usize,
    overlay_rows: usize,
    stored_bytes: usize,
    plain_bytes: usize,
    load_bases: Vec<u32>,
    links: Vec<SubgameSeqE6StorageLink>,
}

impl fmt::Display for SubgameSeqE6GlyphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqE6GlyphError {}

fn error(message: impl Into<String>) -> SubgameSeqE6GlyphError {
    SubgameSeqE6GlyphError(message.into())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_e6_glyph_tests.rs"]
mod subgame_seq_e6_glyph_tests;
