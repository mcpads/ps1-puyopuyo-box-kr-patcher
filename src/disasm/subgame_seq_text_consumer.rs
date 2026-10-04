//! 비-QUEST SEQ의 u16 텍스트 소비 경계.
//!
//! PCOMI·PUYO1·PUYO2는 서로 다른 EXE지만 같은 큰 구조를 가진다. 문자열
//! 포인터에서 little-endian u16을 읽어 2바이트 전진하고, 검증된
//! `0xFF00`–`0xFF2F`는 48-entry 간접 dispatch로, 나머지는 EXE별 glyph
//! 분기를 거쳐 BIOS glyph cache 또는 고정 BTX cell renderer로 보낸다. 이
//! 모듈은 공통 구조만 EXE별 주소와 typed R3000A 명령으로 검산한다. PCOMI의
//! direct/suffix code 의미는 전용 모듈이 소유하며, SEQ record root나
//! control별 인자 폭·의미는 여기서 주장하지 않는다.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::{linear, trace};

const CONTROL_TABLE_ENTRIES: usize = 48;
const CONTROL_TOKEN_THRESHOLD: u16 = 0xFEFF;
const TEXT_OBJECT_STREAM_OFFSET: i16 = 0x008C;
const GLYPH_CELL_OBJECT_OFFSET: i16 = 0x006A;
const CONSTRUCTOR_STACK_ARGUMENT_OFFSET: i16 = 56;

/// 소비 함수가 최초 stream pointer를 얻는 위치.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubgameTextSourceKind {
    /// 지속형 object의 `+0x8C` field.
    ObjectField,
    /// 함수의 두 번째 인자 `$a1`.
    Argument,
}

impl SubgameTextSourceKind {
    const fn label(self) -> &'static str {
        match self {
            Self::ObjectField => "object+0x8C",
            Self::Argument => "argument-a1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ConsumerSiteSpec {
    role: &'static str,
    entry_runtime: u32,
    source_kind: SubgameTextSourceKind,
    source_load_runtime: Option<u32>,
    source_store_runtime: u32,
    shadow_gp_offset: i16,
    token_load_runtime: u32,
    pointer_advance_runtime: u32,
    pointer_store_runtime: u32,
    threshold_load_runtime: u32,
    token_normalize_runtime: Option<u32>,
    threshold_compare_runtime: u32,
    table_high_runtime: u32,
    table_low_runtime: u32,
    index_mask_runtime: u32,
    index_shift_runtime: u32,
    table_add_runtime: u32,
    handler_load_runtime: u32,
    indirect_call_runtime: u32,
    glyph_cache_call_runtime: u32,
    renderer_extended_call_runtime: u32,
    renderer_short_call_runtime: u32,
}

/// 한 EXE에 고정한 두 텍스트 소비 함수와 공용 자원.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubgameTextConsumerSpec {
    pub exe: &'static str,
    pub archive: &'static str,
    pub control_table_runtime: u32,
    pub glyph_cache_runtime: u32,
    pub bios_wrapper_runtime: u32,
    pub bios_wrapper_call_runtime: u32,
    pub renderer_short_runtime: u32,
    pub renderer_extended_runtime: u32,
    pub constructor_runtime: u32,
    pub constructor_stream_load_runtime: u32,
    pub constructor_stream_store_runtime: u32,
    pub control_table_sha256: [u8; 32],
    pub control_unique_handlers: usize,
    sites: [ConsumerSiteSpec; 2],
}

macro_rules! site {
    (
        $role:expr,
        $entry_runtime:expr,
        $source_kind:expr,
        $source_load_runtime:expr,
        $source_store_runtime:expr,
        $shadow_gp_offset:expr,
        $token_load_runtime:expr,
        $pointer_advance_runtime:expr,
        $pointer_store_runtime:expr,
        $threshold_load_runtime:expr,
        $token_normalize_runtime:expr,
        $threshold_compare_runtime:expr,
        $table_high_runtime:expr,
        $table_low_runtime:expr,
        $index_mask_runtime:expr,
        $index_shift_runtime:expr,
        $table_add_runtime:expr,
        $handler_load_runtime:expr,
        $indirect_call_runtime:expr,
        $glyph_cache_call_runtime:expr,
        $renderer_extended_call_runtime:expr,
        $renderer_short_call_runtime:expr $(,)?
    ) => {
        ConsumerSiteSpec {
            role: $role,
            entry_runtime: $entry_runtime,
            source_kind: $source_kind,
            source_load_runtime: $source_load_runtime,
            source_store_runtime: $source_store_runtime,
            shadow_gp_offset: $shadow_gp_offset,
            token_load_runtime: $token_load_runtime,
            pointer_advance_runtime: $pointer_advance_runtime,
            pointer_store_runtime: $pointer_store_runtime,
            threshold_load_runtime: $threshold_load_runtime,
            token_normalize_runtime: $token_normalize_runtime,
            threshold_compare_runtime: $threshold_compare_runtime,
            table_high_runtime: $table_high_runtime,
            table_low_runtime: $table_low_runtime,
            index_mask_runtime: $index_mask_runtime,
            index_shift_runtime: $index_shift_runtime,
            table_add_runtime: $table_add_runtime,
            handler_load_runtime: $handler_load_runtime,
            indirect_call_runtime: $indirect_call_runtime,
            glyph_cache_call_runtime: $glyph_cache_call_runtime,
            renderer_extended_call_runtime: $renderer_extended_call_runtime,
            renderer_short_call_runtime: $renderer_short_call_runtime,
        }
    };
}

/// 지원 원본에서 분리한 세 EXE의 실제 u16 text consumer 사양.
pub const BOX_SUBGAME_TEXT_CONSUMERS: [SubgameTextConsumerSpec; 3] = [
    SubgameTextConsumerSpec {
        exe: "PCOMI.EXE",
        archive: "/PCOMI/PC1_SEQ.VDD;1",
        control_table_runtime: 0x8005_08E4,
        glyph_cache_runtime: 0x8001_BA4C,
        bios_wrapper_runtime: 0x8001_B7C4,
        bios_wrapper_call_runtime: 0x8001_BC58,
        renderer_short_runtime: 0x8001_8FE4,
        renderer_extended_runtime: 0x8001_92F0,
        constructor_runtime: 0x8001_D408,
        constructor_stream_load_runtime: 0x8001_D43C,
        constructor_stream_store_runtime: 0x8001_D46C,
        control_table_sha256: [
            0x6e, 0x28, 0xec, 0xa5, 0x7a, 0xba, 0x76, 0x4b, 0x3d, 0x6a, 0xf1, 0x5f, 0x1a, 0x06,
            0x9b, 0x55, 0xd7, 0x00, 0xef, 0xcf, 0x26, 0x4c, 0xa9, 0x19, 0x10, 0x2d, 0xea, 0x61,
            0x74, 0xdc, 0xbb, 0x34,
        ],
        control_unique_handlers: 30,
        sites: [
            site!(
                "object-loop",
                0x8001_BCBC,
                SubgameTextSourceKind::ObjectField,
                Some(0x8001_BD4C),
                0x8001_BD5C,
                0x0124,
                0x8001_BDE8,
                0x8001_BDEC,
                0x8001_BDF4,
                0x8001_BDB8,
                None,
                0x8001_BE10,
                0x8001_BD8C,
                0x8001_BD98,
                0x8001_BE18,
                0x8001_BE1C,
                0x8001_BE20,
                0x8001_BE24,
                0x8001_BE2C,
                0x8001_BEBC,
                0x8001_BF04,
                0x8001_BF14,
            ),
            site!(
                "argument-immediate",
                0x8001_C2E0,
                SubgameTextSourceKind::Argument,
                None,
                0x8001_C370,
                0x0124,
                0x8001_C3FC,
                0x8001_C400,
                0x8001_C408,
                0x8001_C424,
                None,
                0x8001_C428,
                0x8001_C354,
                0x8001_C358,
                0x8001_C430,
                0x8001_C434,
                0x8001_C438,
                0x8001_C43C,
                0x8001_C444,
                0x8001_C4AC,
                0x8001_C51C,
                0x8001_C52C,
            ),
        ],
    },
    SubgameTextConsumerSpec {
        exe: "PUYO1.EXE",
        archive: "/PUYO1/PP1_SEQ.VDD;1",
        control_table_runtime: 0x8008_9D7C,
        glyph_cache_runtime: 0x8002_8048,
        bios_wrapper_runtime: 0x8002_7D94,
        bios_wrapper_call_runtime: 0x8002_824C,
        renderer_short_runtime: 0x8002_4AB4,
        renderer_extended_runtime: 0x8002_4DC0,
        constructor_runtime: 0x8002_9A14,
        constructor_stream_load_runtime: 0x8002_9A44,
        constructor_stream_store_runtime: 0x8002_9AA8,
        control_table_sha256: [
            0x54, 0xcc, 0x32, 0x8f, 0x71, 0x37, 0xc1, 0xf8, 0x7d, 0xf0, 0x67, 0x0a, 0x31, 0xeb,
            0x52, 0xab, 0x96, 0xf5, 0xa8, 0xbe, 0x7f, 0xd2, 0x41, 0xab, 0x2e, 0xe4, 0x9e, 0x4d,
            0x4b, 0x2c, 0x27, 0xdc,
        ],
        control_unique_handlers: 30,
        sites: [
            site!(
                "object-loop",
                0x8002_82B0,
                SubgameTextSourceKind::ObjectField,
                Some(0x8002_8308),
                0x8002_833C,
                0x04B8,
                0x8002_83CC,
                0x8002_83D0,
                0x8002_83D8,
                0x8002_82D8,
                Some(0x8002_83F4),
                0x8002_83F8,
                0x8002_8400,
                0x8002_8404,
                0x8002_8408,
                0x8002_840C,
                0x8002_8410,
                0x8002_8414,
                0x8002_841C,
                0x8002_8490,
                0x8002_84D8,
                0x8002_84E8,
            ),
            site!(
                "argument-immediate",
                0x8002_88B0,
                SubgameTextSourceKind::Argument,
                None,
                0x8002_892C,
                0x04B8,
                0x8002_89C8,
                0x8002_89CC,
                0x8002_89D4,
                0x8002_89F4,
                Some(0x8002_89F0),
                0x8002_89F8,
                0x8002_8918,
                0x8002_8948,
                0x8002_8A00,
                0x8002_8A04,
                0x8002_8A08,
                0x8002_8A0C,
                0x8002_8A14,
                0x8002_8A7C,
                0x8002_8AEC,
                0x8002_8AFC,
            ),
        ],
    },
    SubgameTextConsumerSpec {
        exe: "PUYO2.EXE",
        archive: "/PUYO2/PP2_SEQ.VDD;1",
        control_table_runtime: 0x8008_BDCC,
        glyph_cache_runtime: 0x8002_6550,
        bios_wrapper_runtime: 0x8002_629C,
        bios_wrapper_call_runtime: 0x8002_6754,
        renderer_short_runtime: 0x8002_3AF0,
        renderer_extended_runtime: 0x8002_3DFC,
        constructor_runtime: 0x8002_7F1C,
        constructor_stream_load_runtime: 0x8002_7F4C,
        constructor_stream_store_runtime: 0x8002_7FB0,
        control_table_sha256: [
            0x7b, 0x84, 0x42, 0x52, 0xa5, 0xee, 0x75, 0x4b, 0x1e, 0xf2, 0xb3, 0xf9, 0xde, 0x12,
            0xd4, 0x03, 0x88, 0x04, 0xee, 0xb3, 0x47, 0xbd, 0x81, 0xc6, 0x94, 0x52, 0x80, 0x35,
            0xb5, 0x24, 0xac, 0x06,
        ],
        control_unique_handlers: 30,
        sites: [
            site!(
                "object-loop",
                0x8002_67B8,
                SubgameTextSourceKind::ObjectField,
                Some(0x8002_6810),
                0x8002_6844,
                0x04B4,
                0x8002_68D4,
                0x8002_68D8,
                0x8002_68E0,
                0x8002_67E0,
                Some(0x8002_68FC),
                0x8002_6900,
                0x8002_6908,
                0x8002_690C,
                0x8002_6910,
                0x8002_6914,
                0x8002_6918,
                0x8002_691C,
                0x8002_6924,
                0x8002_6998,
                0x8002_69E0,
                0x8002_69F0,
            ),
            site!(
                "argument-immediate",
                0x8002_6DB8,
                SubgameTextSourceKind::Argument,
                None,
                0x8002_6E34,
                0x04B4,
                0x8002_6ED0,
                0x8002_6ED4,
                0x8002_6EDC,
                0x8002_6EFC,
                Some(0x8002_6EF8),
                0x8002_6F00,
                0x8002_6E20,
                0x8002_6E50,
                0x8002_6F08,
                0x8002_6F0C,
                0x8002_6F10,
                0x8002_6F14,
                0x8002_6F1C,
                0x8002_6F84,
                0x8002_6FF4,
                0x8002_7004,
            ),
        ],
    },
];

/// 검산된 소비 함수 한 곳.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameTextConsumer {
    pub exe: &'static str,
    pub archive: &'static str,
    pub role: &'static str,
    pub entry_runtime: u32,
    pub source_kind: SubgameTextSourceKind,
    pub source_runtime: u32,
    pub token_load_runtime: u32,
    pub pointer_advance_runtime: u32,
    pub pointer_store_runtime: u32,
    pub shadow_gp_offset: i16,
    pub threshold_compare_runtime: u32,
    pub control_table_runtime: u32,
    pub indirect_call_runtime: u32,
    pub glyph_cache_call_runtime: u32,
    pub renderer_short_call_runtime: u32,
    pub renderer_extended_call_runtime: u32,
    pub entry_reached: bool,
}

/// EXE별 48-entry control table 검산 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameTextControlTable {
    pub exe: &'static str,
    pub runtime: u32,
    pub entries: usize,
    pub defined_entries: usize,
    pub zero_entries: usize,
    pub unique_handlers: usize,
    pub handlers: Vec<u32>,
    pub sha256: [u8; 32],
}

/// 세 EXE의 text consumer와 table 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameTextConsumerAnalysis {
    pub consumers: Vec<SubgameTextConsumer>,
    pub control_tables: Vec<SubgameTextControlTable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgameTextConsumerError(String);

impl fmt::Display for SubgameTextConsumerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for SubgameTextConsumerError {}

fn error(message: impl Into<String>) -> SubgameTextConsumerError {
    SubgameTextConsumerError(message.into())
}

fn instruction_at(
    instructions: &[Instruction],
    address: u32,
) -> Result<&Instruction, SubgameTextConsumerError> {
    instructions
        .binary_search_by_key(&address, |instruction| instruction.addr)
        .ok()
        .map(|index| &instructions[index])
        .ok_or_else(|| error(format!("text consumer 명령 주소 없음: 0x{address:08X}")))
}

fn require(
    instructions: &[Instruction],
    exe: &str,
    address: u32,
    expected: K,
) -> Result<(), SubgameTextConsumerError> {
    let actual = instruction_at(instructions, address)?;
    if actual.kind != expected {
        return Err(error(format!(
            "{exe} text consumer 명령 불일치: address=0x{address:08X}, \
             expected={expected:?}, actual={:?}",
            actual.kind
        )));
    }
    Ok(())
}

fn loaded_address(high: u16, low: i16) -> u32 {
    ((high as u32) << 16).wrapping_add_signed(low as i32)
}

fn verify_site(
    spec: &SubgameTextConsumerSpec,
    site: ConsumerSiteSpec,
    instructions: &[Instruction],
    reached: &BTreeSet<u32>,
) -> Result<SubgameTextConsumer, SubgameTextConsumerError> {
    let source_reg = match site.source_kind {
        SubgameTextSourceKind::ObjectField => {
            let load_runtime = site.source_load_runtime.ok_or_else(|| {
                error(format!(
                    "{} {} object source load 주소 없음",
                    spec.exe, site.role
                ))
            })?;
            let K::Lw {
                rt,
                base: Reg::S0,
                offset: TEXT_OBJECT_STREAM_OFFSET,
            } = instruction_at(instructions, load_runtime)?.kind
            else {
                return Err(error(format!(
                    "{} {} object +0x8C source load 불일치: 0x{load_runtime:08X}",
                    spec.exe, site.role
                )));
            };
            rt
        }
        SubgameTextSourceKind::Argument => Reg::A1,
    };
    require(
        instructions,
        spec.exe,
        site.source_store_runtime,
        K::Sw {
            rt: source_reg,
            base: Reg::GP,
            offset: site.shadow_gp_offset,
        },
    )?;

    let K::Lhu {
        rt: token_reg,
        base: pointer_reg,
        offset: 0,
    } = instruction_at(instructions, site.token_load_runtime)?.kind
    else {
        return Err(error(format!(
            "{} {} u16 token load 불일치: 0x{:08X}",
            spec.exe, site.role, site.token_load_runtime
        )));
    };
    require(
        instructions,
        spec.exe,
        site.pointer_advance_runtime,
        K::Addiu {
            rt: pointer_reg,
            rs: pointer_reg,
            imm: 2,
        },
    )?;
    require(
        instructions,
        spec.exe,
        site.pointer_store_runtime,
        K::Sw {
            rt: pointer_reg,
            base: Reg::GP,
            offset: site.shadow_gp_offset,
        },
    )?;

    let K::Ori {
        rt: threshold_reg,
        rs: Reg::ZERO,
        imm: CONTROL_TOKEN_THRESHOLD,
    } = instruction_at(instructions, site.threshold_load_runtime)?.kind
    else {
        return Err(error(format!(
            "{} {} control threshold load 불일치: 0x{:08X}",
            spec.exe, site.role, site.threshold_load_runtime
        )));
    };
    let compare_token_reg = if let Some(normalize_runtime) = site.token_normalize_runtime {
        let K::Andi {
            rt,
            rs,
            imm: 0xFFFF,
        } = instruction_at(instructions, normalize_runtime)?.kind
        else {
            return Err(error(format!(
                "{} {} token u16 normalize 불일치: 0x{normalize_runtime:08X}",
                spec.exe, site.role
            )));
        };
        if rs != token_reg {
            return Err(error(format!(
                "{} {} token normalize 원천 register 불일치: expected={token_reg}, actual={rs}",
                spec.exe, site.role
            )));
        }
        rt
    } else {
        token_reg
    };
    require(
        instructions,
        spec.exe,
        site.threshold_compare_runtime,
        K::Sltu {
            rd: Reg::V0,
            rs: threshold_reg,
            rt: compare_token_reg,
        },
    )?;

    let K::Lui {
        rt: table_high_reg,
        imm: table_high,
    } = instruction_at(instructions, site.table_high_runtime)?.kind
    else {
        return Err(error(format!(
            "{} {} control table high load 불일치: 0x{:08X}",
            spec.exe, site.role, site.table_high_runtime
        )));
    };
    let K::Addiu {
        rt: table_reg,
        rs: table_low_source,
        imm: table_low,
    } = instruction_at(instructions, site.table_low_runtime)?.kind
    else {
        return Err(error(format!(
            "{} {} control table low load 불일치: 0x{:08X}",
            spec.exe, site.role, site.table_low_runtime
        )));
    };
    if table_low_source != table_high_reg
        || loaded_address(table_high, table_low) != spec.control_table_runtime
    {
        return Err(error(format!(
            "{} {} control table 주소 불일치: expected=0x{:08X}",
            spec.exe, site.role, spec.control_table_runtime
        )));
    }

    let K::Andi {
        rt: index_reg,
        rs: index_source,
        imm: 0x00FF,
    } = instruction_at(instructions, site.index_mask_runtime)?.kind
    else {
        return Err(error(format!(
            "{} {} control index mask 불일치: 0x{:08X}",
            spec.exe, site.role, site.index_mask_runtime
        )));
    };
    if index_source != token_reg {
        return Err(error(format!(
            "{} {} control index 원천 register 불일치: expected={token_reg}, actual={index_source}",
            spec.exe, site.role
        )));
    }
    require(
        instructions,
        spec.exe,
        site.index_shift_runtime,
        K::Sll {
            rd: index_reg,
            rt: index_reg,
            sa: 2,
        },
    )?;
    let K::Addu {
        rd: indexed_table_reg,
        rs: add_left,
        rt: add_right,
    } = instruction_at(instructions, site.table_add_runtime)?.kind
    else {
        return Err(error(format!(
            "{} {} control table index add 불일치: 0x{:08X}",
            spec.exe, site.role, site.table_add_runtime
        )));
    };
    if !((add_left == index_reg && add_right == table_reg)
        || (add_left == table_reg && add_right == index_reg))
    {
        return Err(error(format!(
            "{} {} control table index register 연결 불일치",
            spec.exe, site.role
        )));
    }
    let K::Lw {
        rt: handler_reg,
        base: handler_base,
        offset: 0,
    } = instruction_at(instructions, site.handler_load_runtime)?.kind
    else {
        return Err(error(format!(
            "{} {} control handler load 불일치: 0x{:08X}",
            spec.exe, site.role, site.handler_load_runtime
        )));
    };
    if handler_base != indexed_table_reg {
        return Err(error(format!(
            "{} {} control handler table register 불일치",
            spec.exe, site.role
        )));
    }
    require(
        instructions,
        spec.exe,
        site.indirect_call_runtime,
        K::Jalr {
            rd: Reg::RA,
            rs: handler_reg,
        },
    )?;
    require(
        instructions,
        spec.exe,
        site.glyph_cache_call_runtime,
        K::Jal {
            target: spec.glyph_cache_runtime,
        },
    )?;
    require(
        instructions,
        spec.exe,
        site.renderer_short_call_runtime,
        K::Jal {
            target: spec.renderer_short_runtime,
        },
    )?;
    require(
        instructions,
        spec.exe,
        site.renderer_extended_call_runtime,
        K::Jal {
            target: spec.renderer_extended_runtime,
        },
    )?;
    let cell_store = instructions.iter().any(|instruction| {
        instruction.addr >= site.glyph_cache_call_runtime
            && instruction.addr
                <= site
                    .renderer_short_call_runtime
                    .max(site.renderer_extended_call_runtime)
            && matches!(
                instruction.kind,
                K::Sh {
                    base: Reg::S0,
                    offset: GLYPH_CELL_OBJECT_OFFSET,
                    ..
                }
            )
    });
    if !cell_store {
        return Err(error(format!(
            "{} {} glyph cache 결과의 object +0x6A cell store가 없음",
            spec.exe, site.role
        )));
    }

    let required = [
        site.entry_runtime,
        site.source_store_runtime,
        site.token_load_runtime,
        site.pointer_advance_runtime,
        site.pointer_store_runtime,
        site.threshold_compare_runtime,
        site.indirect_call_runtime,
        site.glyph_cache_call_runtime,
        site.renderer_short_call_runtime,
        site.renderer_extended_call_runtime,
    ];
    Ok(SubgameTextConsumer {
        exe: spec.exe,
        archive: spec.archive,
        role: site.role,
        entry_runtime: site.entry_runtime,
        source_kind: site.source_kind,
        source_runtime: site
            .source_load_runtime
            .unwrap_or(site.source_store_runtime),
        token_load_runtime: site.token_load_runtime,
        pointer_advance_runtime: site.pointer_advance_runtime,
        pointer_store_runtime: site.pointer_store_runtime,
        shadow_gp_offset: site.shadow_gp_offset,
        threshold_compare_runtime: site.threshold_compare_runtime,
        control_table_runtime: spec.control_table_runtime,
        indirect_call_runtime: site.indirect_call_runtime,
        glyph_cache_call_runtime: site.glyph_cache_call_runtime,
        renderer_short_call_runtime: site.renderer_short_call_runtime,
        renderer_extended_call_runtime: site.renderer_extended_call_runtime,
        entry_reached: required.iter().all(|address| reached.contains(address)),
    })
}

fn verify_control_table(
    spec: &SubgameTextConsumerSpec,
    map: &ExeMap,
    text: &[u8],
) -> Result<SubgameTextControlTable, SubgameTextConsumerError> {
    let offset = map.text_offset(spec.control_table_runtime).ok_or_else(|| {
        error(format!(
            "{} control table이 EXE text 밖임: 0x{:08X}",
            spec.exe, spec.control_table_runtime
        ))
    })?;
    let bytes_len = CONTROL_TABLE_ENTRIES * 4;
    let bytes = text.get(offset..offset + bytes_len).ok_or_else(|| {
        error(format!(
            "{} control table 48 word 범위 이탈: 0x{:08X}",
            spec.exe, spec.control_table_runtime
        ))
    })?;
    let mut unique_handlers = BTreeSet::new();
    let mut handlers = Vec::with_capacity(CONTROL_TABLE_ENTRIES);
    let mut defined_entries = 0usize;
    for (code, word) in bytes.chunks_exact(4).enumerate() {
        let handler = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
        if handler == 0 {
            handlers.push(handler);
            continue;
        }
        defined_entries += 1;
        if !map.contains(handler) {
            return Err(error(format!(
                "{} control FF{code:02X} handler가 자기 EXE text 밖임: 0x{handler:08X}",
                spec.exe
            )));
        }
        unique_handlers.insert(handler);
        handlers.push(handler);
    }
    let sha256: [u8; 32] = Sha256::digest(bytes).into();
    if sha256 != spec.control_table_sha256 || unique_handlers.len() != spec.control_unique_handlers
    {
        return Err(error(format!(
            "{} control table 검토 기준선 불일치: unique expected={}, actual={}, \
             SHA-256 expected={}, actual={}",
            spec.exe,
            spec.control_unique_handlers,
            unique_handlers.len(),
            hex_digest(&spec.control_table_sha256),
            hex_digest(&sha256),
        )));
    }
    Ok(SubgameTextControlTable {
        exe: spec.exe,
        runtime: spec.control_table_runtime,
        entries: CONTROL_TABLE_ENTRIES,
        defined_entries,
        zero_entries: CONTROL_TABLE_ENTRIES - defined_entries,
        unique_handlers: unique_handlers.len(),
        handlers,
        sha256,
    })
}

fn verify_one(
    spec: &SubgameTextConsumerSpec,
    map: &'static ExeMap,
    text: &[u8],
) -> Result<(Vec<SubgameTextConsumer>, SubgameTextControlTable), SubgameTextConsumerError> {
    if map.name != spec.exe {
        return Err(error(format!(
            "text consumer EXE 사상 불일치: expected={}, actual={}",
            spec.exe, map.name
        )));
    }
    if text.len() != map.text_len as usize {
        return Err(error(format!(
            "{} text 길이 불일치: expected={}, actual={}",
            spec.exe,
            map.text_len,
            text.len()
        )));
    }
    let instructions = linear::sweep(map, text)
        .ok_or_else(|| error(format!("{} text가 4바이트 정렬이 아님", spec.exe)))?;
    require(
        &instructions,
        spec.exe,
        spec.constructor_stream_load_runtime,
        K::Lw {
            rt: Reg::S4,
            base: Reg::SP,
            offset: CONSTRUCTOR_STACK_ARGUMENT_OFFSET,
        },
    )?;
    let constructor_store = instruction_at(&instructions, spec.constructor_stream_store_runtime)?;
    if !matches!(
        constructor_store.kind,
        K::Sw {
            rt: Reg::S4,
            offset: TEXT_OBJECT_STREAM_OFFSET,
            ..
        }
    ) {
        return Err(error(format!(
            "{} 생성자 제5인자 → object +0x8C store 불일치: 0x{:08X}",
            spec.exe, spec.constructor_stream_store_runtime
        )));
    }
    require(
        &instructions,
        spec.exe,
        spec.bios_wrapper_call_runtime,
        K::Jal {
            target: spec.bios_wrapper_runtime,
        },
    )?;

    let reached = trace(map, text, &[])
        .instructions
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let consumers = spec
        .sites
        .iter()
        .copied()
        .map(|site| verify_site(spec, site, &instructions, &reached))
        .collect::<Result<Vec<_>, _>>()?;
    let table = verify_control_table(spec, map, text)?;
    Ok((consumers, table))
}

/// 지원 원본 세 EXE의 두 소비자·control table·font route를 검산한다.
pub fn verify_box_subgame_text_consumers<'a>(
    executables: impl IntoIterator<Item = (&'static ExeMap, &'a [u8])>,
) -> Result<SubgameTextConsumerAnalysis, SubgameTextConsumerError> {
    let mut inputs = BTreeMap::new();
    for (map, text) in executables {
        if inputs.insert(map.name, (map, text)).is_some() {
            return Err(error(format!("text consumer EXE 입력 중복: {}", map.name)));
        }
    }
    let mut consumers = Vec::new();
    let mut control_tables = Vec::new();
    for spec in &BOX_SUBGAME_TEXT_CONSUMERS {
        let &(map, text) = inputs
            .get(spec.exe)
            .ok_or_else(|| error(format!("text consumer EXE 입력 누락: {}", spec.exe)))?;
        let (mut verified, table) = verify_one(spec, map, text)?;
        consumers.append(&mut verified);
        control_tables.push(table);
    }
    if consumers.len() != BOX_SUBGAME_TEXT_CONSUMERS.len() * 2
        || control_tables.len() != BOX_SUBGAME_TEXT_CONSUMERS.len()
    {
        return Err(error("지원 원본 text consumer 분모 불일치"));
    }
    Ok(SubgameTextConsumerAnalysis {
        consumers,
        control_tables,
    })
}

/// 소비 함수 6행과 48-entry table 3행을 결정적 long-form TSV로 만든다.
pub fn render_subgame_text_consumers_tsv(analysis: &SubgameTextConsumerAnalysis) -> String {
    let mut out = String::from(
        "kind\texe\tarchive\trole\tentry_runtime\tsource_kind\tsource_runtime\t\
         token_load_runtime\tpointer_advance_runtime\tpointer_store_runtime\tshadow_gp_offset\t\
         threshold_compare_runtime\tcontrol_table_runtime\tindirect_call_runtime\t\
         glyph_cache_call_runtime\trenderer_short_call_runtime\t\
         renderer_extended_call_runtime\tentry_reached\tentries\tdefined_entries\tzero_entries\t\
         unique_handlers\tsha256\n",
    );
    for consumer in &analysis.consumers {
        writeln!(
            out,
            "consumer\t{}\t{}\t{}\t0x{:08X}\t{}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t\
             0x{:08X}\t0x{:04X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t0x{:08X}\t\
             0x{:08X}\t0x{:08X}\t{}\t\t\t\t\t",
            consumer.exe,
            consumer.archive,
            consumer.role,
            consumer.entry_runtime,
            consumer.source_kind.label(),
            consumer.source_runtime,
            consumer.token_load_runtime,
            consumer.pointer_advance_runtime,
            consumer.pointer_store_runtime,
            consumer.shadow_gp_offset as u16,
            consumer.threshold_compare_runtime,
            consumer.control_table_runtime,
            consumer.indirect_call_runtime,
            consumer.glyph_cache_call_runtime,
            consumer.renderer_short_call_runtime,
            consumer.renderer_extended_call_runtime,
            consumer.entry_reached,
        )
        .expect("String 쓰기는 실패하지 않는다");
    }
    for table in &analysis.control_tables {
        let digest = hex_digest(&table.sha256);
        writeln!(
            out,
            "control-table\t{}\t\t\t0x{:08X}\t\t\t\t\t\t\t\t0x{:08X}\t\t\t\t\t\t{}\t{}\t{}\t{}\t{}",
            table.exe,
            table.runtime,
            table.runtime,
            table.entries,
            table.defined_entries,
            table.zero_entries,
            table.unique_handlers,
            digest,
        )
        .expect("String 쓰기는 실패하지 않는다");
        for (code, handler) in table.handlers.iter().enumerate() {
            writeln!(
                out,
                "control-entry\t{}\t\tFF{code:02X}\t0x{handler:08X}\t\t\t\t\t\t\t\t0x{:08X}\t\t\t\t\t\t\t\t\t\t",
                table.exe, table.runtime,
            )
            .expect("String 쓰기는 실패하지 않는다");
        }
    }
    out
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "subgame_seq_text_consumer_tests.rs"]
mod subgame_seq_text_consumer_tests;
