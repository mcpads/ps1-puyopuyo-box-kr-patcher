//! 비-QUEST text 생성자 제5인자의 EXE 내부 소비 bridge.
//!
//! PCOMI·PUYO1·PUYO2의 text 생성자는 제5인자를 object `+0x8C`에 저장하고,
//! EXE 내부 합성 record `[21 02 00 00, pointer]`의 `+4`에도 기록한다. 같은
//! object의 `+0x1C`는 이 합성 record를 가리키며 generic dispatcher의
//! opcode `0x21`, mode `2` 경로와 연결된다. 이 모듈은 이 계약과 기존
//! `object-loop` consumer의 `+0x8C` read를 typed R3000A로 검산한다.
//!
//! 또한 지원 EXE와 해제된 비-QUEST SEQ 전체에서 생성자 주소의 direct
//! flow·주소 조립·raw pointer와 canonical OP21/mode2 record를 찾고 EXE entry
//! trace를 교차한다. entry trace의 non-return `jr`는 BIOS vector veneer와
//! 범위 검사된 initialized absolute jump table로 분류한다. direct/SEQ
//! reference 0건과 entry 미도달은 runtime pointer caller나 실행 불가를
//! 뜻하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::{normalize_kseg, ExeMap};
use crate::isa::{decode_slice, FlowKind, Reg};
use crate::script::{parse_subgame_seq_overlay_footer, BOX_SUBGAME_SEQ_LAYOUT_FILES};

use super::subgame_seq_text_consumer::{SubgameTextConsumerAnalysis, SubgameTextSourceKind};
use super::{linear, trace, xref};

mod constructor;
mod indirect_sources;
mod report;
mod trace_analysis;

pub use report::render_subgame_seq_text_bridges_tsv;

use constructor::{count_canonical_mode2_records, verify_constructor, verify_dispatch};
#[cfg(test)]
use indirect_sources::find_stack_arg5_jalr_candidates;
use indirect_sources::verify_indirect_candidate;
use trace_analysis::{
    classify_entry_jr_sources, count_raw_pointer, expand_entry_trace_through_jr_tables,
};

#[cfg(test)]
use crate::isa::{Instruction, InstructionKind as K};
#[cfg(test)]
use indirect_sources::StackArg5JalrCandidate;
#[cfg(test)]
use trace_analysis::{
    analyze_forwarded_jalr_sources, analyze_outside_text_sources, classify_expanded_jalr_sources,
    inspect_initialized_jalr_table,
};
#[cfg(test)]
use trace_analysis::{collect_jr_table_targets, is_bios_vector_jr, jr_table_source};

pub const SUBGAME_SEQ_TEXT_BRIDGE_SCHEMA_VERSION: &str = "subgame-seq-text-bridge-v12";
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_ROWS: usize = 3;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_SEQUENCE_FILES: usize = BOX_SUBGAME_SEQ_LAYOUT_FILES;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXE_RECORDS: usize = 3;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_SEQUENCE_RECORDS: usize = 0;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_ENTRY_REACHED: usize = 0;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_ENTRY_JR_SITES: usize = 83;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_ENTRY_JR_BIOS_SITES: usize = 46;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_ENTRY_JR_TABLE_SITES: usize = 37;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_ENTRY_JR_UNCLASSIFIED: usize = 0;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_ENTRY_JR_CONSTRUCTOR_TARGETS: usize = 0;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_SITES: usize = 145;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_BIOS_SITES: usize = 7;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_INITIALIZED_SITES: usize = 116;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_OUTSIDE_TEXT_SITES: usize = 6;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_OUTSIDE_TEXT_DIRECT_READS: usize = 6;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_OUTSIDE_TEXT_DIRECT_WRITES: usize = 0;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_OUTSIDE_TEXT_ADDRESS_VALUES: usize = 0;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_COMPUTED_SITES: usize = 13;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_FORWARDED_SITES: usize = 3;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_FORWARDED_DIRECT_CALLERS: usize = 6;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_UNCLASSIFIED_SITES: usize = 0;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_FRESH_ARG5_SITES: usize = 0;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_CONSTRUCTOR_TARGETS: usize = 0;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXE_STACK_ARG5_JALR_SITES: usize = 4;
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXE_CONSTRUCTOR_TARGETS: usize = 0;
pub const SUBGAME_SEQ_TEXT_BRIDGE_OPCODE: u8 = 0x21;
pub const SUBGAME_SEQ_TEXT_BRIDGE_MODE: u8 = 0x02;
const TEXT_OBJECT_CURRENT_RECORD_OFFSET: i16 = 0x1C;
const TEXT_OBJECT_STREAM_OFFSET: i16 = 0x8C;
const CONSTRUCTOR_STACK_ARGUMENT_OFFSET: i16 = 56;
const CALLER_STACK_ARGUMENT_OFFSET: i16 = 16;
pub const SUBGAME_SEQ_TEXT_BRIDGE_SYNTHETIC_RECORD_BYTES: [u8; 16] = [
    SUBGAME_SEQ_TEXT_BRIDGE_OPCODE,
    SUBGAME_SEQ_TEXT_BRIDGE_MODE,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0x40,
    0,
    0,
    0,
];

/// 지원 원본에서 생성한 3행 TSV의 SHA-256.
pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_REPORT_SHA256: [u8; 32] = [
    0x16, 0x2F, 0xE5, 0xFF, 0xB4, 0x71, 0xBF, 0x39, 0x2E, 0x20, 0xF1, 0xA6, 0xCF, 0x06, 0x78, 0x61,
    0x6C, 0xE8, 0x0D, 0x8A, 0x79, 0xC5, 0x4B, 0xCB, 0x88, 0x14, 0x9A, 0x96, 0xDE, 0x2A, 0x04, 0xE2,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqTextJrTableSource {
    pub call_runtime: u32,
    pub table_runtime: u32,
    pub table_entries: usize,
    pub constructor_targets: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextExpandedTrace {
    pub iterations: usize,
    pub table_target_seeds: usize,
    pub table_targets_outside_exe: usize,
    pub reached_instructions: usize,
    pub indirect_sites: usize,
    pub jalr_sites: usize,
    pub jr_bios_sites: usize,
    pub jr_table_sites: usize,
    pub jr_unclassified_sites: Vec<u32>,
    pub jalr_bios_vector_sites: Vec<u32>,
    pub jalr_initialized_direct_sources: Vec<SubgameSeqTextJalrInitializedSource>,
    pub jalr_initialized_rooted_sources: Vec<SubgameSeqTextJalrInitializedSource>,
    pub jalr_initialized_indexed_sources: Vec<SubgameSeqTextJalrInitializedTableSource>,
    pub jalr_outside_text_sources: Vec<SubgameSeqTextJalrRuntimeSource>,
    pub jalr_computed_memory_sources: Vec<SubgameSeqTextJalrRuntimeFieldSource>,
    pub jalr_forwarded_register_sites: Vec<u32>,
    pub jalr_forwarded_register_sources: Vec<SubgameSeqTextJalrForwardedSource>,
    pub jalr_unclassified_sites: Vec<u32>,
    pub jalr_fresh_stack_arg5_sites: Vec<u32>,
    pub constructor_reached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextJalrInitializedSource {
    pub source_runtime: u32,
    pub initialized_target: u32,
    pub call_sites: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextJalrRuntimeSource {
    pub source_runtime: u32,
    pub call_sites: Vec<u32>,
    pub direct_read_sites: Vec<u32>,
    pub direct_write_sites: Vec<u32>,
    pub address_value_sites: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqTextJalrOutsideTextSpec {
    pub source_runtime: u32,
    pub read_runtime: u32,
    pub call_runtime: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqTextJalrRuntimeFieldSource {
    pub call_runtime: u32,
    pub load_runtime: u32,
    pub target_reg: Reg,
    pub base_reg: Reg,
    pub field_offset: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextJalrInitializedTableSource {
    pub call_runtime: u32,
    pub table_runtime: u32,
    pub table_entries: usize,
    pub unique_targets: usize,
    pub zero_targets: usize,
    pub outside_exe_targets: usize,
    pub constructor_targets: usize,
    pub sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextJalrForwardedSource {
    pub call_runtime: u32,
    pub target_reg: Reg,
    pub capture_runtime: u32,
    pub function_entry_runtime: u32,
    pub source_argument_reg: Reg,
    pub direct_call_sites: Vec<u32>,
    pub direct_call_targets: Vec<u32>,
    pub entry_address_load_refs: usize,
    pub entry_raw_pointer_refs: usize,
    pub constructor_targets: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqTextExpandedTraceExpectation {
    pub iterations: usize,
    pub table_target_seeds: usize,
    pub reached_instructions: usize,
    pub indirect_sites: usize,
    pub jalr_sites: usize,
    pub jr_bios_sites: usize,
    pub jr_table_sites: usize,
    pub jalr_bios_vector_sites: usize,
    pub jalr_initialized_direct_sites: usize,
    pub jalr_initialized_rooted_sites: usize,
    pub jalr_initialized_indexed_sites: usize,
    pub jalr_outside_text_sites: usize,
    pub jalr_computed_memory_sites: usize,
    pub jalr_forwarded_register_sites: usize,
    pub jalr_forwarded_direct_callers: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqTextIndirectCandidateSpec {
    pub call_runtime: u32,
    pub argument_store_runtime: u32,
    pub table_runtime: u32,
    pub table_entries: usize,
    pub table_unique_targets: usize,
    pub table_zero_targets: usize,
    pub table_outside_exe_targets: usize,
    pub table_sha256: [u8; 32],
    pub table_kind: &'static str,
}

const PCOMI_STACK_ARG5_JALR: [SubgameSeqTextIndirectCandidateSpec; 2] = [
    SubgameSeqTextIndirectCandidateSpec {
        call_runtime: 0x8001_C444,
        argument_store_runtime: 0x8001_C3B4,
        table_runtime: 0x8005_08E4,
        table_entries: 48,
        table_unique_targets: 30,
        table_zero_targets: 0,
        table_outside_exe_targets: 0,
        table_sha256: [
            0x6e, 0x28, 0xec, 0xa5, 0x7a, 0xba, 0x76, 0x4b, 0x3d, 0x6a, 0xf1, 0x5f, 0x1a, 0x06,
            0x9b, 0x55, 0xd7, 0x00, 0xef, 0xcf, 0x26, 0x4c, 0xa9, 0x19, 0x10, 0x2d, 0xea, 0x61,
            0x74, 0xdc, 0xbb, 0x34,
        ],
        table_kind: "text-control-ff00-ff2f",
    },
    SubgameSeqTextIndirectCandidateSpec {
        call_runtime: 0x8003_0CF0,
        argument_store_runtime: 0x8003_0CF4,
        table_runtime: 0x8005_23D0,
        table_entries: 1,
        table_unique_targets: 1,
        table_zero_targets: 0,
        table_outside_exe_targets: 0,
        table_sha256: [
            0x8e, 0x7b, 0xe4, 0xd9, 0xfb, 0x34, 0xac, 0x44, 0xd3, 0x14, 0x7c, 0xe7, 0xac, 0x0f,
            0x51, 0x7e, 0x60, 0xc1, 0xfe, 0x70, 0xd1, 0x2e, 0xfc, 0x38, 0x0d, 0xd2, 0x89, 0x4d,
            0x4d, 0x14, 0xc0, 0x48,
        ],
        table_kind: "format-callback-slot",
    },
];

const PUYO1_STACK_ARG5_JALR: [SubgameSeqTextIndirectCandidateSpec; 1] =
    [SubgameSeqTextIndirectCandidateSpec {
        call_runtime: 0x8006_DEC4,
        argument_store_runtime: 0x8006_DEC8,
        table_runtime: 0x8008_E730,
        table_entries: 1,
        table_unique_targets: 1,
        table_zero_targets: 0,
        table_outside_exe_targets: 0,
        table_sha256: [
            0x21, 0x45, 0xca, 0xab, 0x30, 0x4b, 0x6b, 0xff, 0x3b, 0x7a, 0x08, 0x65, 0xbe, 0x30,
            0x99, 0x35, 0x8f, 0xd2, 0x69, 0xba, 0xdb, 0xd8, 0x2a, 0xac, 0xc6, 0xef, 0xa1, 0x78,
            0xc8, 0x35, 0x2e, 0xe1,
        ],
        table_kind: "format-callback-slot",
    }];

const PUYO2_STACK_ARG5_JALR: [SubgameSeqTextIndirectCandidateSpec; 1] =
    [SubgameSeqTextIndirectCandidateSpec {
        call_runtime: 0x8006_CDC0,
        argument_store_runtime: 0x8006_CDC4,
        table_runtime: 0x8009_09FC,
        table_entries: 1,
        table_unique_targets: 1,
        table_zero_targets: 0,
        table_outside_exe_targets: 0,
        table_sha256: [
            0xde, 0xf8, 0x32, 0xe2, 0xee, 0xc5, 0x17, 0x2e, 0xc7, 0x78, 0x2a, 0xa9, 0x54, 0x51,
            0xdb, 0x8d, 0x58, 0x56, 0x89, 0x88, 0x01, 0xad, 0x5f, 0x29, 0xf5, 0xeb, 0x06, 0x54,
            0xb1, 0x63, 0x20, 0x0f,
        ],
        table_kind: "format-callback-slot",
    }];

const PCOMI_COMPUTED_MEMORY_SOURCES: [SubgameSeqTextJalrRuntimeFieldSource; 5] = [
    SubgameSeqTextJalrRuntimeFieldSource {
        call_runtime: 0x8001_1DC4,
        load_runtime: 0x8001_1DB4,
        target_reg: Reg::V0,
        base_reg: Reg::S0,
        field_offset: 0x18,
    },
    SubgameSeqTextJalrRuntimeFieldSource {
        call_runtime: 0x8001_3280,
        load_runtime: 0x8001_3278,
        target_reg: Reg::V0,
        base_reg: Reg::S0,
        field_offset: 0x08,
    },
    SubgameSeqTextJalrRuntimeFieldSource {
        call_runtime: 0x8001_389C,
        load_runtime: 0x8001_3894,
        target_reg: Reg::V0,
        base_reg: Reg::S0,
        field_offset: 0x08,
    },
    SubgameSeqTextJalrRuntimeFieldSource {
        call_runtime: 0x8001_8F04,
        load_runtime: 0x8001_8EF4,
        target_reg: Reg::V0,
        base_reg: Reg::S0,
        field_offset: 0x78,
    },
    SubgameSeqTextJalrRuntimeFieldSource {
        call_runtime: 0x8004_7AD0,
        load_runtime: 0x8004_7AC0,
        target_reg: Reg::V0,
        base_reg: Reg::S0,
        field_offset: 0,
    },
];

const PUYO1_COMPUTED_MEMORY_SOURCES: [SubgameSeqTextJalrRuntimeFieldSource; 4] = [
    SubgameSeqTextJalrRuntimeFieldSource {
        call_runtime: 0x8001_5500,
        load_runtime: 0x8001_54F0,
        target_reg: Reg::V0,
        base_reg: Reg::S0,
        field_offset: 0x18,
    },
    SubgameSeqTextJalrRuntimeFieldSource {
        call_runtime: 0x8001_B68C,
        load_runtime: 0x8001_B684,
        target_reg: Reg::V0,
        base_reg: Reg::S0,
        field_offset: 0x08,
    },
    SubgameSeqTextJalrRuntimeFieldSource {
        call_runtime: 0x8001_BD30,
        load_runtime: 0x8001_BD28,
        target_reg: Reg::V0,
        base_reg: Reg::S2,
        field_offset: 0x08,
    },
    SubgameSeqTextJalrRuntimeFieldSource {
        call_runtime: 0x8002_49C8,
        load_runtime: 0x8002_49B8,
        target_reg: Reg::V0,
        base_reg: Reg::S0,
        field_offset: 0x78,
    },
];

const PUYO2_COMPUTED_MEMORY_SOURCES: [SubgameSeqTextJalrRuntimeFieldSource; 4] = [
    SubgameSeqTextJalrRuntimeFieldSource {
        call_runtime: 0x8001_4E04,
        load_runtime: 0x8001_4DF4,
        target_reg: Reg::V0,
        base_reg: Reg::S0,
        field_offset: 0x18,
    },
    SubgameSeqTextJalrRuntimeFieldSource {
        call_runtime: 0x8001_A118,
        load_runtime: 0x8001_A110,
        target_reg: Reg::V0,
        base_reg: Reg::S0,
        field_offset: 0x08,
    },
    SubgameSeqTextJalrRuntimeFieldSource {
        call_runtime: 0x8001_A7BC,
        load_runtime: 0x8001_A7B4,
        target_reg: Reg::V0,
        base_reg: Reg::S2,
        field_offset: 0x08,
    },
    SubgameSeqTextJalrRuntimeFieldSource {
        call_runtime: 0x8002_3A04,
        load_runtime: 0x8002_39F4,
        target_reg: Reg::V0,
        base_reg: Reg::S0,
        field_offset: 0x78,
    },
];

const PCOMI_OUTSIDE_TEXT_SOURCES: [SubgameSeqTextJalrOutsideTextSpec; 2] = [
    SubgameSeqTextJalrOutsideTextSpec {
        source_runtime: 0x8009_5680,
        read_runtime: 0x8004_0A10,
        call_runtime: 0x8004_0A18,
    },
    SubgameSeqTextJalrOutsideTextSpec {
        source_runtime: 0x8009_5808,
        read_runtime: 0x8004_09E4,
        call_runtime: 0x8004_09EC,
    },
];

const PUYO1_OUTSIDE_TEXT_SOURCES: [SubgameSeqTextJalrOutsideTextSpec; 2] = [
    SubgameSeqTextJalrOutsideTextSpec {
        source_runtime: 0x800D_C4A8,
        read_runtime: 0x8007_E254,
        call_runtime: 0x8007_E25C,
    },
    SubgameSeqTextJalrOutsideTextSpec {
        source_runtime: 0x800D_C630,
        read_runtime: 0x8007_E228,
        call_runtime: 0x8007_E230,
    },
];

const PUYO2_OUTSIDE_TEXT_SOURCES: [SubgameSeqTextJalrOutsideTextSpec; 2] = [
    SubgameSeqTextJalrOutsideTextSpec {
        source_runtime: 0x800D_D188,
        read_runtime: 0x8007_C670,
        call_runtime: 0x8007_C678,
    },
    SubgameSeqTextJalrOutsideTextSpec {
        source_runtime: 0x800D_D310,
        read_runtime: 0x8007_C644,
        call_runtime: 0x8007_C64C,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameSeqTextBridgeSpec {
    pub exe: &'static str,
    pub archive: &'static str,
    pub constructor_runtime: u32,
    pub fifth_argument_load_runtime: u32,
    pub synthetic_record_runtime: u32,
    pub synthetic_base_high_runtime: u32,
    pub synthetic_base_high: u16,
    pub synthetic_pointer_store_runtime: u32,
    pub synthetic_pointer_offset: i16,
    pub synthetic_pointer_adjust_runtime: u32,
    pub synthetic_record_adjust_runtime: u32,
    pub callback_high_runtime: u32,
    pub callback_high_reg: Reg,
    pub callback_high: u16,
    pub callback_low_runtime: u32,
    pub callback_low: i16,
    pub allocator_call_runtime: u32,
    pub allocator_runtime: u32,
    pub allocator_result_move_runtime: Option<u32>,
    pub object_reg: Reg,
    pub current_record_store_runtime: u32,
    pub object_stream_store_runtime: u32,
    pub dispatcher_runtime: u32,
    pub dispatcher_table_runtime: u32,
    pub dispatcher_record_reg: Reg,
    pub dispatcher_opcode_reg: Reg,
    pub dispatcher_handler_reg: Reg,
    pub op21_handler_runtime: u32,
    pub op21_mode_reg: Reg,
    pub op21_mode_table_runtime: u32,
    pub op21_mode_handler_reg: Reg,
    pub op21_mode2_handler_runtime: u32,
    pub object_consumer_runtime: u32,
    pub object_consumer_source_runtime: u32,
    pub object_control_table_runtime: u32,
    pub object_control_call_runtime: u32,
    pub expected_entry_indirect_sites: usize,
    pub expected_entry_jr_sites: usize,
    pub expected_entry_jr_bios_sites: usize,
    pub expected_entry_jr_table_sites: usize,
    pub expected_expanded_trace: SubgameSeqTextExpandedTraceExpectation,
    pub outside_text_sources: &'static [SubgameSeqTextJalrOutsideTextSpec],
    pub computed_memory_sources: &'static [SubgameSeqTextJalrRuntimeFieldSource],
    pub stack_arg5_jalr: &'static [SubgameSeqTextIndirectCandidateSpec],
    pub drac_relation: &'static str,
}

pub const BOX_SUBGAME_SEQ_TEXT_BRIDGE_SPECS: [SubgameSeqTextBridgeSpec; 3] = [
    SubgameSeqTextBridgeSpec {
        exe: "PCOMI.EXE",
        archive: "/PCOMI/PC1_SEQ.VDD;1",
        constructor_runtime: 0x8001_D408,
        fifth_argument_load_runtime: 0x8001_D43C,
        synthetic_record_runtime: 0x8005_06F8,
        synthetic_base_high_runtime: 0x8001_D410,
        synthetic_base_high: 0x8005,
        synthetic_pointer_store_runtime: 0x8001_D448,
        synthetic_pointer_offset: 0x06FC,
        synthetic_pointer_adjust_runtime: 0x8001_D450,
        synthetic_record_adjust_runtime: 0x8001_D490,
        callback_high_runtime: 0x8001_D430,
        callback_high_reg: Reg::A0,
        callback_high: 0x8002,
        callback_low_runtime: 0x8001_D434,
        callback_low: -32664,
        allocator_call_runtime: 0x8001_D44C,
        allocator_runtime: 0x8001_1CA4,
        allocator_result_move_runtime: Some(0x8001_D454),
        object_reg: Reg::V1,
        current_record_store_runtime: 0x8001_D494,
        object_stream_store_runtime: 0x8001_D46C,
        dispatcher_runtime: 0x8001_8068,
        dispatcher_table_runtime: 0x8005_0258,
        dispatcher_record_reg: Reg::V1,
        dispatcher_opcode_reg: Reg::V0,
        dispatcher_handler_reg: Reg::V1,
        op21_handler_runtime: 0x8001_7894,
        op21_mode_reg: Reg::A0,
        op21_mode_table_runtime: 0x8001_02F4,
        op21_mode_handler_reg: Reg::A0,
        op21_mode2_handler_runtime: 0x8001_78DC,
        object_consumer_runtime: 0x8001_BCBC,
        object_consumer_source_runtime: 0x8001_BD4C,
        object_control_table_runtime: 0x8005_08E4,
        object_control_call_runtime: 0x8001_BE2C,
        expected_entry_indirect_sites: 79,
        expected_entry_jr_sites: 32,
        expected_entry_jr_bios_sites: 20,
        expected_entry_jr_table_sites: 12,
        expected_expanded_trace: SubgameSeqTextExpandedTraceExpectation {
            iterations: 1,
            table_target_seeds: 125,
            reached_instructions: 23_220,
            indirect_sites: 85,
            jalr_sites: 52,
            jr_bios_sites: 21,
            jr_table_sites: 12,
            jalr_bios_vector_sites: 5,
            jalr_initialized_direct_sites: 21,
            jalr_initialized_rooted_sites: 16,
            jalr_initialized_indexed_sites: 2,
            jalr_outside_text_sites: 2,
            jalr_computed_memory_sites: 5,
            jalr_forwarded_register_sites: 1,
            jalr_forwarded_direct_callers: 2,
        },
        outside_text_sources: &PCOMI_OUTSIDE_TEXT_SOURCES,
        computed_memory_sources: &PCOMI_COMPUTED_MEMORY_SOURCES,
        stack_arg5_jalr: &PCOMI_STACK_ARG5_JALR,
        drac_relation: "not-drac",
    },
    SubgameSeqTextBridgeSpec {
        exe: "PUYO1.EXE",
        archive: "/PUYO1/PP1_SEQ.VDD;1",
        constructor_runtime: 0x8002_9A14,
        fifth_argument_load_runtime: 0x8002_9A44,
        synthetic_record_runtime: 0x8008_9B90,
        synthetic_base_high_runtime: 0x8002_9A34,
        synthetic_base_high: 0x8009,
        synthetic_pointer_store_runtime: 0x8002_9A50,
        synthetic_pointer_offset: -25708,
        synthetic_pointer_adjust_runtime: 0x8002_9A58,
        synthetic_record_adjust_runtime: 0x8002_9A5C,
        callback_high_runtime: 0x8002_9A38,
        callback_high_reg: Reg::V0,
        callback_high: 0x8002,
        callback_low_runtime: 0x8002_9A48,
        callback_low: 0x37D4,
        allocator_call_runtime: 0x8002_9A54,
        allocator_runtime: 0x8001_53EC,
        allocator_result_move_runtime: None,
        object_reg: Reg::V0,
        current_record_store_runtime: 0x8002_9A94,
        object_stream_store_runtime: 0x8002_9AA8,
        dispatcher_runtime: 0x8002_37D4,
        dispatcher_table_runtime: 0x8008_96F0,
        dispatcher_record_reg: Reg::V0,
        dispatcher_opcode_reg: Reg::V0,
        dispatcher_handler_reg: Reg::V0,
        op21_handler_runtime: 0x8002_3038,
        op21_mode_reg: Reg::V1,
        op21_mode_table_runtime: 0x8001_06E8,
        op21_mode_handler_reg: Reg::V0,
        op21_mode2_handler_runtime: 0x8002_3080,
        object_consumer_runtime: 0x8002_82B0,
        object_consumer_source_runtime: 0x8002_8308,
        object_control_table_runtime: 0x8008_9D7C,
        object_control_call_runtime: 0x8002_841C,
        expected_entry_indirect_sites: 67,
        expected_entry_jr_sites: 26,
        expected_entry_jr_bios_sites: 13,
        expected_entry_jr_table_sites: 13,
        expected_expanded_trace: SubgameSeqTextExpandedTraceExpectation {
            iterations: 2,
            table_target_seeds: 143,
            reached_instructions: 24_274,
            indirect_sites: 75,
            jalr_sites: 46,
            jr_bios_sites: 15,
            jr_table_sites: 14,
            jalr_bios_vector_sites: 1,
            jalr_initialized_direct_sites: 21,
            jalr_initialized_rooted_sites: 15,
            jalr_initialized_indexed_sites: 2,
            jalr_outside_text_sites: 2,
            jalr_computed_memory_sites: 4,
            jalr_forwarded_register_sites: 1,
            jalr_forwarded_direct_callers: 2,
        },
        outside_text_sources: &PUYO1_OUTSIDE_TEXT_SOURCES,
        computed_memory_sources: &PUYO1_COMPUTED_MEMORY_SOURCES,
        stack_arg5_jalr: &PUYO1_STACK_ARG5_JALR,
        drac_relation: "distinct-from-drac-e0-no-static-link",
    },
    SubgameSeqTextBridgeSpec {
        exe: "PUYO2.EXE",
        archive: "/PUYO2/PP2_SEQ.VDD;1",
        constructor_runtime: 0x8002_7F1C,
        fifth_argument_load_runtime: 0x8002_7F4C,
        synthetic_record_runtime: 0x8008_BBE0,
        synthetic_base_high_runtime: 0x8002_7F3C,
        synthetic_base_high: 0x8009,
        synthetic_pointer_store_runtime: 0x8002_7F58,
        synthetic_pointer_offset: -17436,
        synthetic_pointer_adjust_runtime: 0x8002_7F60,
        synthetic_record_adjust_runtime: 0x8002_7F64,
        callback_high_runtime: 0x8002_7F40,
        callback_high_reg: Reg::V0,
        callback_high: 0x8002,
        callback_low_runtime: 0x8002_7F50,
        callback_low: 0x2824,
        allocator_call_runtime: 0x8002_7F5C,
        allocator_runtime: 0x8001_4CF0,
        allocator_result_move_runtime: None,
        object_reg: Reg::V0,
        current_record_store_runtime: 0x8002_7F9C,
        object_stream_store_runtime: 0x8002_7FB0,
        dispatcher_runtime: 0x8002_2824,
        dispatcher_table_runtime: 0x8008_B740,
        dispatcher_record_reg: Reg::V0,
        dispatcher_opcode_reg: Reg::V0,
        dispatcher_handler_reg: Reg::V0,
        op21_handler_runtime: 0x8002_1FEC,
        op21_mode_reg: Reg::V1,
        op21_mode_table_runtime: 0x8001_070C,
        op21_mode_handler_reg: Reg::V0,
        op21_mode2_handler_runtime: 0x8002_2034,
        object_consumer_runtime: 0x8002_67B8,
        object_consumer_source_runtime: 0x8002_6810,
        object_control_table_runtime: 0x8008_BDCC,
        object_control_call_runtime: 0x8002_6924,
        expected_entry_indirect_sites: 67,
        expected_entry_jr_sites: 25,
        expected_entry_jr_bios_sites: 13,
        expected_entry_jr_table_sites: 12,
        expected_expanded_trace: SubgameSeqTextExpandedTraceExpectation {
            iterations: 1,
            table_target_seeds: 124,
            reached_instructions: 23_527,
            indirect_sites: 74,
            jalr_sites: 47,
            jr_bios_sites: 15,
            jr_table_sites: 12,
            jalr_bios_vector_sites: 1,
            jalr_initialized_direct_sites: 22,
            jalr_initialized_rooted_sites: 15,
            jalr_initialized_indexed_sites: 2,
            jalr_outside_text_sites: 2,
            jalr_computed_memory_sites: 4,
            jalr_forwarded_register_sites: 1,
            jalr_forwarded_direct_callers: 2,
        },
        outside_text_sources: &PUYO2_OUTSIDE_TEXT_SOURCES,
        computed_memory_sources: &PUYO2_COMPUTED_MEMORY_SOURCES,
        stack_arg5_jalr: &PUYO2_STACK_ARG5_JALR,
        drac_relation: "distinct-from-drac-e0-no-static-link",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextBridgeRow {
    pub exe: &'static str,
    pub archive: &'static str,
    pub constructor_runtime: u32,
    pub synthetic_record_runtime: u32,
    pub dispatcher_runtime: u32,
    pub dispatcher_table_runtime: u32,
    pub op21_handler_runtime: u32,
    pub op21_mode2_handler_runtime: u32,
    pub object_consumer_runtime: u32,
    pub object_consumer_source_runtime: u32,
    pub exe_direct_flow_refs: usize,
    pub exe_address_load_refs: usize,
    pub exe_raw_pointer_refs: usize,
    pub sequence_files_scanned: usize,
    pub sequence_direct_jal_refs: usize,
    pub sequence_direct_jump_refs: usize,
    pub sequence_address_load_refs: usize,
    pub sequence_raw_pointer_refs: usize,
    pub constructor_entry_reached: bool,
    pub entry_indirect_sites: usize,
    pub entry_jr_bios_sites: Vec<u32>,
    pub entry_jr_table_sources: Vec<SubgameSeqTextJrTableSource>,
    pub expanded_trace: SubgameSeqTextExpandedTrace,
    pub exe_stack_arg5_jalr_sites: usize,
    pub stack_arg5_jalr: Vec<SubgameSeqTextIndirectCandidateSpec>,
    pub indirect_table_constructor_targets: Vec<usize>,
    pub exe_canonical_mode2_records: usize,
    pub sequence_canonical_mode2_records: usize,
    pub drac_relation: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextBridgeAnalysis {
    pub rows: Vec<SubgameSeqTextBridgeRow>,
    pub sequence_files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameSeqTextBridgeError(String);

impl fmt::Display for SubgameSeqTextBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameSeqTextBridgeError {}

fn error(message: impl Into<String>) -> SubgameSeqTextBridgeError {
    SubgameSeqTextBridgeError(message.into())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn audit_subgame_seq_text_bridges<'a, 'b>(
    specs: &[SubgameSeqTextBridgeSpec],
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sequences: impl IntoIterator<Item = (&'b str, &'b str, &'b [u8])>,
    consumers: &SubgameTextConsumerAnalysis,
) -> Result<SubgameSeqTextBridgeAnalysis, SubgameSeqTextBridgeError> {
    if specs.is_empty() {
        return Err(error("text bridge spec이 없음"));
    }
    let mut spec_names = BTreeSet::new();
    for spec in specs {
        if !spec_names.insert(spec.exe) {
            return Err(error(format!("{} text bridge spec 중복", spec.exe)));
        }
    }
    let mut executable_map = BTreeMap::new();
    for (map, text) in executables {
        if executable_map.insert(map.name, (map, text)).is_some() {
            return Err(error(format!("{} text bridge EXE 중복", map.name)));
        }
    }
    let mut sequence_keys = BTreeSet::new();
    let mut sequence_rows = Vec::new();
    for (archive, sequence, raw) in sequences {
        if !sequence_keys.insert((archive, sequence)) {
            return Err(error(format!("{archive}:{sequence} text bridge SEQ 중복")));
        }
        sequence_rows.push((archive, sequence, raw));
    }
    if sequence_rows.len() != BOX_SUBGAME_SEQ_TEXT_BRIDGE_SEQUENCE_FILES {
        return Err(error(format!(
            "text bridge SEQ 분모 불일치: expected={}, actual={}",
            BOX_SUBGAME_SEQ_TEXT_BRIDGE_SEQUENCE_FILES,
            sequence_rows.len()
        )));
    }
    let sequence_canonical_mode2_records = sequence_rows
        .iter()
        .map(|(_, _, raw)| count_canonical_mode2_records(raw))
        .sum::<usize>();
    if sequence_canonical_mode2_records != BOX_SUBGAME_SEQ_TEXT_BRIDGE_SEQUENCE_RECORDS {
        return Err(error(format!(
            "text bridge SEQ canonical OP21/mode2 record 기준선 불일치: expected={}, actual={sequence_canonical_mode2_records}",
            BOX_SUBGAME_SEQ_TEXT_BRIDGE_SEQUENCE_RECORDS,
        )));
    }
    let mut rows = Vec::with_capacity(specs.len());
    for &spec in specs {
        let &(map, text) = executable_map
            .get(spec.exe)
            .ok_or_else(|| error(format!("{} text bridge EXE 누락", spec.exe)))?;
        if text.len() != map.text_len as usize {
            return Err(error(format!("{} text bridge EXE 길이 불일치", spec.exe)));
        }
        let instructions = linear::sweep(map, text)
            .ok_or_else(|| error(format!("{} text bridge EXE 비정렬", spec.exe)))?;
        verify_constructor(map, text, &instructions, spec)?;
        verify_dispatch(map, text, &instructions, spec)?;
        let (exe_stack_arg5_jalr_sites, indirect_table_constructor_targets) =
            verify_indirect_candidate(map, text, &instructions, spec, consumers)?;
        let entry_trace = trace(map, text, &[]);
        let constructor_entry_reached = entry_trace
            .instructions
            .contains_key(&spec.constructor_runtime);
        let entry_indirect_sites = entry_trace.indirect_sites.len();
        if constructor_entry_reached || entry_indirect_sites != spec.expected_entry_indirect_sites {
            return Err(error(format!(
                "{} constructor entry trace 기준선 불일치: reached={constructor_entry_reached}, indirect expected={}, actual={entry_indirect_sites}",
                spec.exe, spec.expected_entry_indirect_sites,
            )));
        }
        let (entry_jr_bios_sites, entry_jr_table_sources) =
            classify_entry_jr_sources(map, text, &instructions, &entry_trace, spec)?;
        let expanded_trace = expand_entry_trace_through_jr_tables(
            map,
            text,
            &instructions,
            &entry_jr_table_sources,
            spec,
        )?;
        let expected_expanded = spec.expected_expanded_trace;
        let initialized_direct_sites = expanded_trace
            .jalr_initialized_direct_sources
            .iter()
            .map(|source| source.call_sites.len())
            .sum::<usize>();
        let initialized_rooted_sites = expanded_trace
            .jalr_initialized_rooted_sources
            .iter()
            .map(|source| source.call_sites.len())
            .sum::<usize>();
        let initialized_indexed_sites = expanded_trace.jalr_initialized_indexed_sources.len();
        let outside_text_sites = expanded_trace
            .jalr_outside_text_sources
            .iter()
            .map(|source| source.call_sites.len())
            .sum::<usize>();
        let outside_text_direct_reads = expanded_trace
            .jalr_outside_text_sources
            .iter()
            .map(|source| source.direct_read_sites.len())
            .sum::<usize>();
        let outside_text_direct_writes = expanded_trace
            .jalr_outside_text_sources
            .iter()
            .map(|source| source.direct_write_sites.len())
            .sum::<usize>();
        let outside_text_address_values = expanded_trace
            .jalr_outside_text_sources
            .iter()
            .map(|source| source.address_value_sites.len())
            .sum::<usize>();
        let outside_text_sources_match = expanded_trace.jalr_outside_text_sources.len()
            == spec.outside_text_sources.len()
            && expanded_trace
                .jalr_outside_text_sources
                .iter()
                .zip(spec.outside_text_sources)
                .all(|(source, expected)| {
                    source.source_runtime == expected.source_runtime
                        && source.call_sites == [expected.call_runtime]
                        && source.direct_read_sites == [expected.read_runtime]
                        && source.direct_write_sites.is_empty()
                        && source.address_value_sites.is_empty()
                });
        let initialized_constructor_targets = expanded_trace
            .jalr_initialized_direct_sources
            .iter()
            .chain(&expanded_trace.jalr_initialized_rooted_sources)
            .filter(|source| {
                normalize_kseg(source.initialized_target)
                    == normalize_kseg(spec.constructor_runtime)
            })
            .count()
            + expanded_trace
                .jalr_initialized_indexed_sources
                .iter()
                .map(|source| source.constructor_targets)
                .sum::<usize>();
        let classified_jalr_sites = expanded_trace.jalr_bios_vector_sites.len()
            + initialized_direct_sites
            + initialized_rooted_sites
            + initialized_indexed_sites
            + outside_text_sites
            + expanded_trace.jalr_computed_memory_sources.len()
            + expanded_trace.jalr_forwarded_register_sites.len()
            + expanded_trace.jalr_unclassified_sites.len();
        let forwarded_direct_callers = expanded_trace
            .jalr_forwarded_register_sources
            .iter()
            .map(|source| source.direct_call_sites.len())
            .sum::<usize>();
        let forwarded_constructor_targets = expanded_trace
            .jalr_forwarded_register_sources
            .iter()
            .map(|source| source.constructor_targets)
            .sum::<usize>();
        if expanded_trace.iterations != expected_expanded.iterations
            || expanded_trace.table_target_seeds != expected_expanded.table_target_seeds
            || expanded_trace.table_targets_outside_exe != 0
            || expanded_trace.reached_instructions != expected_expanded.reached_instructions
            || expanded_trace.indirect_sites != expected_expanded.indirect_sites
            || expanded_trace.jalr_sites != expected_expanded.jalr_sites
            || expanded_trace.jr_bios_sites != expected_expanded.jr_bios_sites
            || expanded_trace.jr_table_sites != expected_expanded.jr_table_sites
            || !expanded_trace.jr_unclassified_sites.is_empty()
            || expanded_trace.jalr_bios_vector_sites.len()
                != expected_expanded.jalr_bios_vector_sites
            || initialized_direct_sites != expected_expanded.jalr_initialized_direct_sites
            || initialized_rooted_sites != expected_expanded.jalr_initialized_rooted_sites
            || initialized_indexed_sites != expected_expanded.jalr_initialized_indexed_sites
            || outside_text_sites != expected_expanded.jalr_outside_text_sites
            || outside_text_direct_reads != expected_expanded.jalr_outside_text_sites
            || outside_text_direct_writes != 0
            || outside_text_address_values != 0
            || !outside_text_sources_match
            || expanded_trace.jalr_computed_memory_sources.len()
                != expected_expanded.jalr_computed_memory_sites
            || expanded_trace.jalr_computed_memory_sources != spec.computed_memory_sources
            || expanded_trace.jalr_forwarded_register_sites.len()
                != expected_expanded.jalr_forwarded_register_sites
            || expanded_trace.jalr_forwarded_register_sources.len()
                != expected_expanded.jalr_forwarded_register_sites
            || forwarded_direct_callers != expected_expanded.jalr_forwarded_direct_callers
            || forwarded_constructor_targets != 0
            || expanded_trace
                .jalr_forwarded_register_sources
                .iter()
                .any(|source| {
                    source.source_argument_reg != Reg::A2
                        || source.entry_address_load_refs != 0
                        || source.entry_raw_pointer_refs != 0
                })
            || !expanded_trace.jalr_unclassified_sites.is_empty()
            || !expanded_trace.jalr_fresh_stack_arg5_sites.is_empty()
            || initialized_constructor_targets != 0
            || classified_jalr_sites != expanded_trace.jalr_sites
            || expanded_trace.constructor_reached
        {
            return Err(error(format!(
                "{} expanded jr-table trace 기준선 불일치: expected={expected_expanded:?}, actual={expanded_trace:?}",
                spec.exe
            )));
        }
        let exe_canonical_mode2_records = count_canonical_mode2_records(text);
        if exe_canonical_mode2_records != 1 {
            return Err(error(format!(
                "{} EXE canonical OP21/mode2 record 기준선 불일치: expected=1, actual={exe_canonical_mode2_records}",
                spec.exe,
            )));
        }

        let consumer = consumers
            .consumers
            .iter()
            .find(|consumer| {
                consumer.exe == spec.exe
                    && consumer.role == "object-loop"
                    && consumer.source_kind == SubgameTextSourceKind::ObjectField
            })
            .ok_or_else(|| error(format!("{} object-loop consumer 누락", spec.exe)))?;
        if consumer.archive != spec.archive
            || consumer.entry_runtime != spec.object_consumer_runtime
            || consumer.source_runtime != spec.object_consumer_source_runtime
            || consumer.control_table_runtime != spec.object_control_table_runtime
            || consumer.indirect_call_runtime != spec.object_control_call_runtime
        {
            return Err(error(format!(
                "{} constructor/object-loop consumer 교차 불일치",
                spec.exe
            )));
        }

        let target = normalize_kseg(spec.constructor_runtime);
        let alias = target | 0x2000_0000;
        let exe_direct_flow_refs = instructions
            .iter()
            .filter(|instruction| match instruction.flow_kind() {
                FlowKind::Call { target: value }
                | FlowKind::Jump { target: value }
                | FlowKind::Branch { taken: value } => normalize_kseg(value) == target,
                _ => false,
            })
            .count();
        let exe_address_load_refs = xref::find_address_loads(&instructions, xref::DEFAULT_WINDOW)
            .iter()
            .filter(|load| normalize_kseg(load.value) == target)
            .count();
        let exe_raw_pointer_refs = count_raw_pointer(text, target) + count_raw_pointer(text, alias);

        let jal_word = 0x0C00_0000 | ((target >> 2) & 0x03FF_FFFF);
        let jump_word = 0x0800_0000 | ((target >> 2) & 0x03FF_FFFF);
        let sequence_direct_jal_refs = sequence_rows
            .iter()
            .flat_map(|(_, _, raw)| raw.chunks_exact(4))
            .filter(|bytes| {
                u32::from_le_bytes((*bytes).try_into().expect("4바이트 word")) == jal_word
            })
            .count();
        let sequence_direct_jump_refs = sequence_rows
            .iter()
            .flat_map(|(_, _, raw)| raw.chunks_exact(4))
            .filter(|bytes| {
                u32::from_le_bytes((*bytes).try_into().expect("4바이트 word")) == jump_word
            })
            .count();
        let mut sequence_address_load_refs = 0usize;
        for &(archive, sequence, raw) in &sequence_rows {
            let Some(footer) = parse_subgame_seq_overlay_footer(raw)
                .map_err(|cause| error(format!("{archive}:{sequence} overlay footer: {cause}")))?
            else {
                continue;
            };
            let initialized = raw.get(..footer.footer_offset).ok_or_else(|| {
                error(format!(
                    "{archive}:{sequence} overlay initialized 범위 이탈"
                ))
            })?;
            let overlay_instructions =
                decode_slice(footer.load_base, initialized).ok_or_else(|| {
                    error(format!(
                        "{archive}:{sequence} overlay initialized 영역 비정렬"
                    ))
                })?;
            sequence_address_load_refs +=
                xref::find_address_loads(&overlay_instructions, xref::DEFAULT_WINDOW)
                    .iter()
                    .filter(|load| normalize_kseg(load.value) == target)
                    .count();
        }
        let sequence_raw_pointer_refs = sequence_rows
            .iter()
            .map(|(_, _, raw)| count_raw_pointer(raw, target) + count_raw_pointer(raw, alias))
            .sum();
        if exe_direct_flow_refs != 0
            || exe_address_load_refs != 0
            || exe_raw_pointer_refs != 0
            || sequence_direct_jal_refs != 0
            || sequence_direct_jump_refs != 0
            || sequence_address_load_refs != 0
            || sequence_raw_pointer_refs != 0
        {
            return Err(error(format!(
                "{} constructor direct reference 기준선 불일치: exe flow/address/raw={exe_direct_flow_refs}/{exe_address_load_refs}/{exe_raw_pointer_refs}, SEQ jal/j/address/raw={sequence_direct_jal_refs}/{sequence_direct_jump_refs}/{sequence_address_load_refs}/{sequence_raw_pointer_refs}",
                spec.exe
            )));
        }

        rows.push(SubgameSeqTextBridgeRow {
            exe: spec.exe,
            archive: spec.archive,
            constructor_runtime: spec.constructor_runtime,
            synthetic_record_runtime: spec.synthetic_record_runtime,
            dispatcher_runtime: spec.dispatcher_runtime,
            dispatcher_table_runtime: spec.dispatcher_table_runtime,
            op21_handler_runtime: spec.op21_handler_runtime,
            op21_mode2_handler_runtime: spec.op21_mode2_handler_runtime,
            object_consumer_runtime: spec.object_consumer_runtime,
            object_consumer_source_runtime: spec.object_consumer_source_runtime,
            exe_direct_flow_refs,
            exe_address_load_refs,
            exe_raw_pointer_refs,
            sequence_files_scanned: sequence_rows.len(),
            sequence_direct_jal_refs,
            sequence_direct_jump_refs,
            sequence_address_load_refs,
            sequence_raw_pointer_refs,
            constructor_entry_reached,
            entry_indirect_sites,
            entry_jr_bios_sites,
            entry_jr_table_sources,
            expanded_trace,
            exe_stack_arg5_jalr_sites,
            stack_arg5_jalr: spec.stack_arg5_jalr.to_vec(),
            indirect_table_constructor_targets,
            exe_canonical_mode2_records,
            sequence_canonical_mode2_records,
            drac_relation: spec.drac_relation,
        });
    }
    rows.sort_by_key(|row| row.exe);
    Ok(SubgameSeqTextBridgeAnalysis {
        rows,
        sequence_files: sequence_rows.len(),
    })
}

/// 지원 원본의 생성자 제5인자→합성 OP21→object stream bridge를 검증한다.
pub fn verify_box_subgame_seq_text_bridges<'a, 'b>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
    sequences: impl IntoIterator<Item = (&'b str, &'b str, &'b [u8])>,
    consumers: &SubgameTextConsumerAnalysis,
) -> Result<SubgameSeqTextBridgeAnalysis, SubgameSeqTextBridgeError> {
    let analysis = audit_subgame_seq_text_bridges(
        &BOX_SUBGAME_SEQ_TEXT_BRIDGE_SPECS,
        executables,
        sequences,
        consumers,
    )?;
    if analysis.rows.len() != BOX_SUBGAME_SEQ_TEXT_BRIDGE_ROWS
        || analysis.sequence_files != BOX_SUBGAME_SEQ_TEXT_BRIDGE_SEQUENCE_FILES
    {
        return Err(error("text bridge 지원 원본 분모 불일치"));
    }
    let exe_records = analysis
        .rows
        .iter()
        .map(|row| row.exe_canonical_mode2_records)
        .sum::<usize>();
    let reached = analysis
        .rows
        .iter()
        .filter(|row| row.constructor_entry_reached)
        .count();
    let exe_stack_arg5_jalr_sites = analysis
        .rows
        .iter()
        .map(|row| row.exe_stack_arg5_jalr_sites)
        .sum::<usize>();
    let entry_jr_bios_sites = analysis
        .rows
        .iter()
        .map(|row| row.entry_jr_bios_sites.len())
        .sum::<usize>();
    let entry_jr_table_sites = analysis
        .rows
        .iter()
        .map(|row| row.entry_jr_table_sources.len())
        .sum::<usize>();
    let entry_jr_constructor_targets = analysis
        .rows
        .iter()
        .flat_map(|row| row.entry_jr_table_sources.iter())
        .map(|source| source.constructor_targets)
        .sum::<usize>();
    let entry_jr_sites = entry_jr_bios_sites + entry_jr_table_sites;
    let exe_constructor_targets = analysis
        .rows
        .iter()
        .flat_map(|row| row.indirect_table_constructor_targets.iter().copied())
        .sum::<usize>();
    let expanded_jalr_sites = analysis
        .rows
        .iter()
        .map(|row| row.expanded_trace.jalr_sites)
        .sum::<usize>();
    let expanded_jalr_bios_sites = analysis
        .rows
        .iter()
        .map(|row| row.expanded_trace.jalr_bios_vector_sites.len())
        .sum::<usize>();
    let expanded_jalr_initialized_sites = analysis
        .rows
        .iter()
        .flat_map(|row| {
            row.expanded_trace
                .jalr_initialized_direct_sources
                .iter()
                .chain(&row.expanded_trace.jalr_initialized_rooted_sources)
        })
        .map(|source| source.call_sites.len())
        .sum::<usize>()
        + analysis
            .rows
            .iter()
            .map(|row| row.expanded_trace.jalr_initialized_indexed_sources.len())
            .sum::<usize>();
    let expanded_jalr_outside_text_sites = analysis
        .rows
        .iter()
        .flat_map(|row| &row.expanded_trace.jalr_outside_text_sources)
        .map(|source| source.call_sites.len())
        .sum::<usize>();
    let expanded_jalr_outside_text_direct_reads = analysis
        .rows
        .iter()
        .flat_map(|row| &row.expanded_trace.jalr_outside_text_sources)
        .map(|source| source.direct_read_sites.len())
        .sum::<usize>();
    let expanded_jalr_outside_text_direct_writes = analysis
        .rows
        .iter()
        .flat_map(|row| &row.expanded_trace.jalr_outside_text_sources)
        .map(|source| source.direct_write_sites.len())
        .sum::<usize>();
    let expanded_jalr_outside_text_address_values = analysis
        .rows
        .iter()
        .flat_map(|row| &row.expanded_trace.jalr_outside_text_sources)
        .map(|source| source.address_value_sites.len())
        .sum::<usize>();
    let expanded_jalr_computed_sites = analysis
        .rows
        .iter()
        .map(|row| row.expanded_trace.jalr_computed_memory_sources.len())
        .sum::<usize>();
    let expanded_jalr_forwarded_sites = analysis
        .rows
        .iter()
        .map(|row| row.expanded_trace.jalr_forwarded_register_sites.len())
        .sum::<usize>();
    let expanded_jalr_forwarded_direct_callers = analysis
        .rows
        .iter()
        .flat_map(|row| &row.expanded_trace.jalr_forwarded_register_sources)
        .map(|source| source.direct_call_sites.len())
        .sum::<usize>();
    let expanded_jalr_unclassified_sites = analysis
        .rows
        .iter()
        .map(|row| row.expanded_trace.jalr_unclassified_sites.len())
        .sum::<usize>();
    let expanded_jalr_fresh_arg5_sites = analysis
        .rows
        .iter()
        .map(|row| row.expanded_trace.jalr_fresh_stack_arg5_sites.len())
        .sum::<usize>();
    let expanded_jalr_constructor_targets = analysis
        .rows
        .iter()
        .flat_map(|row| {
            row.expanded_trace
                .jalr_initialized_direct_sources
                .iter()
                .chain(&row.expanded_trace.jalr_initialized_rooted_sources)
                .map(move |source| (row.constructor_runtime, source.initialized_target))
        })
        .filter(|&(constructor, target)| normalize_kseg(constructor) == normalize_kseg(target))
        .count()
        + analysis
            .rows
            .iter()
            .flat_map(|row| &row.expanded_trace.jalr_initialized_indexed_sources)
            .map(|source| source.constructor_targets)
            .sum::<usize>()
        + analysis
            .rows
            .iter()
            .flat_map(|row| &row.expanded_trace.jalr_forwarded_register_sources)
            .map(|source| source.constructor_targets)
            .sum::<usize>();
    if exe_records != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXE_RECORDS
        || reached != BOX_SUBGAME_SEQ_TEXT_BRIDGE_ENTRY_REACHED
        || entry_jr_sites != BOX_SUBGAME_SEQ_TEXT_BRIDGE_ENTRY_JR_SITES
        || entry_jr_bios_sites != BOX_SUBGAME_SEQ_TEXT_BRIDGE_ENTRY_JR_BIOS_SITES
        || entry_jr_table_sites != BOX_SUBGAME_SEQ_TEXT_BRIDGE_ENTRY_JR_TABLE_SITES
        || entry_jr_constructor_targets != BOX_SUBGAME_SEQ_TEXT_BRIDGE_ENTRY_JR_CONSTRUCTOR_TARGETS
        || exe_stack_arg5_jalr_sites != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXE_STACK_ARG5_JALR_SITES
        || exe_constructor_targets != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXE_CONSTRUCTOR_TARGETS
        || expanded_jalr_sites != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_SITES
        || expanded_jalr_bios_sites != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_BIOS_SITES
        || expanded_jalr_initialized_sites
            != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_INITIALIZED_SITES
        || expanded_jalr_outside_text_sites
            != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_OUTSIDE_TEXT_SITES
        || expanded_jalr_outside_text_direct_reads
            != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_OUTSIDE_TEXT_DIRECT_READS
        || expanded_jalr_outside_text_direct_writes
            != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_OUTSIDE_TEXT_DIRECT_WRITES
        || expanded_jalr_outside_text_address_values
            != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_OUTSIDE_TEXT_ADDRESS_VALUES
        || expanded_jalr_computed_sites != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_COMPUTED_SITES
        || expanded_jalr_forwarded_sites
            != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_FORWARDED_SITES
        || expanded_jalr_forwarded_direct_callers
            != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_FORWARDED_DIRECT_CALLERS
        || expanded_jalr_unclassified_sites
            != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_UNCLASSIFIED_SITES
        || expanded_jalr_fresh_arg5_sites
            != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_FRESH_ARG5_SITES
        || expanded_jalr_constructor_targets
            != BOX_SUBGAME_SEQ_TEXT_BRIDGE_EXPANDED_JALR_CONSTRUCTOR_TARGETS
        || analysis.rows.iter().any(|row| {
            row.sequence_canonical_mode2_records != BOX_SUBGAME_SEQ_TEXT_BRIDGE_SEQUENCE_RECORDS
        })
    {
        return Err(error(
            "text bridge canonical record/entry trace 합계 불일치",
        ));
    }
    let report = render_subgame_seq_text_bridges_tsv(&analysis);
    let digest: [u8; 32] = Sha256::digest(report.as_bytes()).into();
    if digest != BOX_SUBGAME_SEQ_TEXT_BRIDGE_REPORT_SHA256 {
        return Err(error(format!(
            "text bridge report SHA-256 불일치: expected={}, actual={}",
            hex(&BOX_SUBGAME_SEQ_TEXT_BRIDGE_REPORT_SHA256),
            hex(&digest)
        )));
    }
    Ok(analysis)
}
#[cfg(test)]
#[path = "subgame_seq_text_bridge_tests.rs"]
mod subgame_seq_text_bridge_tests;
