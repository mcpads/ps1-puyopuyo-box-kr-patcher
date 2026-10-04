//! QUEST `COM_FONT.BTX` chain label의 직접 primitive writer.
//!
//! `quest-chain-label` object는 공용 short renderer를 거치지 않는다. object
//! callback이 cell record를 직접 읽고 8바이트 primitive command를 기록한다.
//! 이 모듈은 지원 원본의 writer와 callback 명령열을 typed R3000A 명령으로
//! 검증해 runtime breakpoint와 RAM field의 의미를 고정한다.

use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::coords::ExeMap;
use crate::isa::{decode, InstructionKind as K, Reg};

pub const QUEST_CHAIN_LABEL_WRITER_RUNTIME: u32 = 0x8004_CD28;
pub const QUEST_CHAIN_LABEL_WRITER_POST_RUNTIME: u32 = 0x8004_CD2C;
pub const QUEST_CHAIN_LABEL_RESOURCE_STORE_RUNTIME: u32 = 0x8004_CD20;
pub const QUEST_CHAIN_LABEL_DRAW_RUNTIME: u32 = 0x8004_7450;
pub const QUEST_CHAIN_LABEL_DRAW_HANDOFF_RUNTIME: u32 = 0x8004_7454;
pub const QUEST_CHAIN_LABEL_DRAW_COMMIT_RUNTIME: u32 = 0x8004_755C;
pub const QUEST_CHAIN_LABEL_DRAW_SOURCE_BYTES: usize = 0x118;
pub const QUEST_CHAIN_LABEL_DESCRIPTOR_RUNTIME: u32 = 0x8008_CE98;
pub const QUEST_CHAIN_LABEL_DESCRIPTOR_ROOT_POINTER_RUNTIME: u32 = 0x800D_B328;
pub const QUEST_CHAIN_LABEL_DESCRIPTOR_TABLE_POINTER_OFFSET: u32 = 0x05BC;
pub const QUEST_CHAIN_LABEL_COMMAND_BUFFER_RUNTIME: u32 = 0x800E_FAC0;
pub const QUEST_CHAIN_LABEL_COMMAND_COUNT_GP_OFFSET: i16 = 0x021C;
pub const QUEST_CHAIN_LABEL_CELL_FIELD_OFFSET: i16 = 0x0066;
pub const QUEST_CHAIN_LABEL_RESOURCE_FIELD_OFFSET: i16 = 0x0068;
pub const QUEST_CHAIN_LABEL_CALLBACK_FIELD_OFFSET: i16 = 0x0074;
pub const QUEST_CHAIN_LABEL_CELL: u16 = 27;
pub const QUEST_CHAIN_LABEL_RESOURCE: u16 = 40;
pub const QUEST_CHAIN_LABEL_CELL_COUNT: usize = 251;

pub const QUEST_CHAIN_LABEL_DRAW_SOURCE_SHA256: [u8; 32] = [
    0x77, 0x70, 0x84, 0x90, 0x4D, 0xE1, 0x28, 0x9F, 0x42, 0x06, 0x5C, 0x85, 0x1F, 0xBF, 0xFD, 0xD0,
    0x11, 0x90, 0x66, 0x92, 0xE0, 0xC2, 0xE4, 0xE2, 0xF5, 0x5D, 0x7C, 0x75, 0xED, 0x33, 0xE2, 0x5C,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontDirectDrawSite {
    pub exe: &'static str,
    pub archive: &'static str,
    pub surface_id: &'static str,
    pub source_text: &'static str,
    pub writer_runtime: u32,
    pub writer_post_runtime: u32,
    pub resource_store_runtime: u32,
    pub function_runtime: u32,
    pub handoff_runtime: u32,
    pub commit_runtime: u32,
    pub source_bytes: usize,
    pub source_sha256: [u8; 32],
    pub descriptor_runtime: u32,
    pub descriptor_root_pointer_runtime: u32,
    pub descriptor_table_pointer_offset: u32,
    pub command_buffer_runtime: u32,
    pub command_count_gp_offset: i16,
    pub cell_field_offset: i16,
    pub resource_field_offset: i16,
    pub callback_field_offset: i16,
    pub cell: u16,
    pub resource: u16,
    pub cell_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommonFontDirectDrawSiteError(String);

impl fmt::Display for CommonFontDirectDrawSiteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for CommonFontDirectDrawSiteError {}

fn error(message: impl Into<String>) -> CommonFontDirectDrawSiteError {
    CommonFontDirectDrawSiteError(message.into())
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn instruction_kind(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
) -> Result<K, CommonFontDirectDrawSiteError> {
    let offset = map.text_offset(runtime).ok_or_else(|| {
        error(format!(
            "QUEST chain draw 주소가 text 밖임: 0x{runtime:08X}"
        ))
    })?;
    let word = text
        .get(offset..offset + 4)
        .and_then(|raw| raw.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| error(format!("QUEST chain draw 명령이 잘림: 0x{runtime:08X}")))?;
    Ok(decode(runtime, word).kind)
}

fn require_instruction(
    map: &ExeMap,
    text: &[u8],
    runtime: u32,
    expected: K,
    role: &str,
) -> Result<(), CommonFontDirectDrawSiteError> {
    let actual = instruction_kind(map, text, runtime)?;
    if actual != expected {
        return Err(error(format!(
            "QUEST chain draw {role} 명령 불일치: 0x{runtime:08X}, expected={expected:?}, actual={actual:?}"
        )));
    }
    Ok(())
}

/// 지원 원본 QUEST writer와 직접 primitive callback의 명령 의미를 검증한다.
pub fn verify_quest_common_font_direct_draw(
    map: &'static ExeMap,
    text: &[u8],
) -> Result<CommonFontDirectDrawSite, CommonFontDirectDrawSiteError> {
    if map.name != "QUEST.EXE" {
        return Err(error(format!(
            "QUEST chain draw 대상이 QUEST.EXE가 아님: {}",
            map.name
        )));
    }
    if text.len() != map.text_len as usize {
        return Err(error(format!(
            "QUEST chain draw text 길이 불일치: expected={}, actual={}",
            map.text_len,
            text.len()
        )));
    }
    let source_offset = map
        .text_offset(QUEST_CHAIN_LABEL_DRAW_RUNTIME)
        .ok_or_else(|| error("QUEST chain draw callback이 text 밖임"))?;
    let source = text
        .get(source_offset..source_offset + QUEST_CHAIN_LABEL_DRAW_SOURCE_BYTES)
        .ok_or_else(|| error("QUEST chain draw callback source가 잘림"))?;
    let source_sha256: [u8; 32] = Sha256::digest(source).into();
    if source_sha256 != QUEST_CHAIN_LABEL_DRAW_SOURCE_SHA256 {
        return Err(error(format!(
            "QUEST chain draw callback source SHA-256 불일치: expected={}, actual={}",
            hex_digest(&QUEST_CHAIN_LABEL_DRAW_SOURCE_SHA256),
            hex_digest(&source_sha256)
        )));
    }

    let instructions = [
        (
            QUEST_CHAIN_LABEL_RESOURCE_STORE_RUNTIME,
            K::Sh {
                rt: Reg::S2,
                base: Reg::A2,
                offset: QUEST_CHAIN_LABEL_RESOURCE_FIELD_OFFSET,
            },
            "resource writer",
        ),
        (
            QUEST_CHAIN_LABEL_WRITER_RUNTIME,
            K::Sh {
                rt: Reg::V0,
                base: Reg::A2,
                offset: QUEST_CHAIN_LABEL_CELL_FIELD_OFFSET,
            },
            "cell writer",
        ),
        (
            QUEST_CHAIN_LABEL_DRAW_RUNTIME,
            K::Addu {
                rd: Reg::T0,
                rs: Reg::A0,
                rt: Reg::ZERO,
            },
            "object handoff",
        ),
        (
            0x8004_7460,
            K::Lhu {
                rt: Reg::A0,
                base: Reg::T0,
                offset: QUEST_CHAIN_LABEL_CELL_FIELD_OFFSET,
            },
            "cell read",
        ),
        (
            0x8004_7468,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800E,
            },
            "descriptor root pointer high",
        ),
        (
            0x8004_7470,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: -19672,
            },
            "descriptor root pointer read",
        ),
        (
            0x8004_7478,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: QUEST_CHAIN_LABEL_DESCRIPTOR_TABLE_POINTER_OFFSET as i16,
            },
            "cell table pointer read",
        ),
        (
            0x8004_746C,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::A0,
                sa: 1,
            },
            "cell stride x2",
        ),
        (
            0x8004_7474,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::A0,
            },
            "cell stride x3",
        ),
        (
            0x8004_747C,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
            "cell stride x12",
        ),
        (
            0x8004_7488,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::V0,
                imm: 12,
            },
            "cell table header",
        ),
        (
            0x8004_7510,
            K::Lw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: QUEST_CHAIN_LABEL_COMMAND_COUNT_GP_OFFSET,
            },
            "command count read",
        ),
        (
            0x8004_7514,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: -1344,
            },
            "command buffer low",
        ),
        (
            0x8004_7518,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 3,
            },
            "command slot stride",
        ),
        (
            0x8004_751C,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
            "command slot address",
        ),
        (
            0x8004_7520,
            K::Sh {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 0,
            },
            "command x",
        ),
        (
            0x8004_7524,
            K::Sh {
                rt: Reg::A3,
                base: Reg::V0,
                offset: 2,
            },
            "command y",
        ),
        (
            0x8004_7528,
            K::Lhu {
                rt: Reg::A1,
                base: Reg::T0,
                offset: QUEST_CHAIN_LABEL_CELL_FIELD_OFFSET,
            },
            "command cell read",
        ),
        (
            0x8004_752C,
            K::Sb {
                rt: Reg::ZERO,
                base: Reg::V0,
                offset: 6,
            },
            "command reserved byte",
        ),
        (
            0x8004_753C,
            K::Sh {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 4,
            },
            "command cell",
        ),
        (
            0x8004_7540,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::T0,
                offset: 0x006E,
            },
            "object draw flags read",
        ),
        (
            0x8004_7548,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 7,
            },
            "object draw flags mask",
        ),
        (
            0x8004_754C,
            K::Sb {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 7,
            },
            "command draw flags",
        ),
        (
            0x8004_7550,
            K::Lw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: QUEST_CHAIN_LABEL_COMMAND_COUNT_GP_OFFSET,
            },
            "command count reread",
        ),
        (
            0x8004_7558,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 1,
            },
            "command count increment",
        ),
        (
            QUEST_CHAIN_LABEL_DRAW_COMMIT_RUNTIME,
            K::Sw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: QUEST_CHAIN_LABEL_COMMAND_COUNT_GP_OFFSET,
            },
            "command count commit",
        ),
    ];
    for (runtime, expected, role) in instructions {
        require_instruction(map, text, runtime, expected, role)?;
    }
    require_instruction(
        map,
        text,
        0x8004_750C,
        K::Lui {
            rt: Reg::A0,
            imm: 0x800F,
        },
        "command buffer high",
    )?;
    require_instruction(
        map,
        text,
        0x8004_7560,
        K::Jr { rs: Reg::RA },
        "callback return",
    )?;

    Ok(CommonFontDirectDrawSite {
        exe: "QUEST.EXE",
        archive: "/QUEST/RPG_GRP.VDD;1",
        surface_id: "quest-chain-label",
        source_text: "れんさ",
        writer_runtime: QUEST_CHAIN_LABEL_WRITER_RUNTIME,
        writer_post_runtime: QUEST_CHAIN_LABEL_WRITER_POST_RUNTIME,
        resource_store_runtime: QUEST_CHAIN_LABEL_RESOURCE_STORE_RUNTIME,
        function_runtime: QUEST_CHAIN_LABEL_DRAW_RUNTIME,
        handoff_runtime: QUEST_CHAIN_LABEL_DRAW_HANDOFF_RUNTIME,
        commit_runtime: QUEST_CHAIN_LABEL_DRAW_COMMIT_RUNTIME,
        source_bytes: QUEST_CHAIN_LABEL_DRAW_SOURCE_BYTES,
        source_sha256,
        descriptor_runtime: QUEST_CHAIN_LABEL_DESCRIPTOR_RUNTIME,
        descriptor_root_pointer_runtime: QUEST_CHAIN_LABEL_DESCRIPTOR_ROOT_POINTER_RUNTIME,
        descriptor_table_pointer_offset: QUEST_CHAIN_LABEL_DESCRIPTOR_TABLE_POINTER_OFFSET,
        command_buffer_runtime: QUEST_CHAIN_LABEL_COMMAND_BUFFER_RUNTIME,
        command_count_gp_offset: QUEST_CHAIN_LABEL_COMMAND_COUNT_GP_OFFSET,
        cell_field_offset: QUEST_CHAIN_LABEL_CELL_FIELD_OFFSET,
        resource_field_offset: QUEST_CHAIN_LABEL_RESOURCE_FIELD_OFFSET,
        callback_field_offset: QUEST_CHAIN_LABEL_CALLBACK_FIELD_OFFSET,
        cell: QUEST_CHAIN_LABEL_CELL,
        resource: QUEST_CHAIN_LABEL_RESOURCE,
        cell_count: QUEST_CHAIN_LABEL_CELL_COUNT,
    })
}

#[cfg(test)]
#[path = "common_font_direct_draw_tests.rs"]
mod common_font_direct_draw_tests;
