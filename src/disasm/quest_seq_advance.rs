//! `QUEST.EXE` SEQ handler 내부의 직접 stream 전진 증거.
//!
//! 공용 fallback을 제외한 78개 opcode 중 74개에서 현재 stream field
//! `+0x1C`를 읽고, 고정값을 더하고, 같은 객체 field에 저장하는 76개
//! 명령 site를 검증한다. 이는 해당 경로의 직접 증가 사실이며, branch로
//! 다른 주소를 선택하는 handler 전체를 단일 고정 record 폭으로
//! 축약하지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::{
    quest_seq_opcode_handler, QUEST_SEQ_DISPATCH_HANDLERS,
    QUEST_SEQ_EXTENDED_FALLBACK_HANDLER_RUNTIME, QUEST_SEQ_MAIN_FALLBACK_HANDLER_RUNTIME,
};

use super::linear;

pub const QUEST_SEQ_STREAM_POINTER_FIELD_OFFSET: i16 = 0x1C;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqDirectAdvanceSite {
    pub opcode: u8,
    pub record_bytes: i16,
    pub load_runtime: u32,
    pub increment_runtime: u32,
    pub store_runtime: u32,
    pub object_reg: Reg,
    pub source_reg: Reg,
    pub value_reg: Reg,
}

macro_rules! site {
    ($opcode:expr, $bytes:expr, $load:expr, $increment:expr, $store:expr, $object:expr, $value:expr) => {
        QuestSeqDirectAdvanceSite {
            opcode: $opcode,
            record_bytes: $bytes,
            load_runtime: $load,
            increment_runtime: $increment,
            store_runtime: $store,
            object_reg: $object,
            source_reg: $value,
            value_reg: $value,
        }
    };
}

macro_rules! site_from {
    ($opcode:expr, $bytes:expr, $load:expr, $increment:expr, $store:expr, $object:expr, $source:expr, $value:expr) => {
        QuestSeqDirectAdvanceSite {
            opcode: $opcode,
            record_bytes: $bytes,
            load_runtime: $load,
            increment_runtime: $increment,
            store_runtime: $store,
            object_reg: $object,
            source_reg: $source,
            value_reg: $value,
        }
    };
}

pub const QUEST_SEQ_DIRECT_ADVANCE_SITES: [QuestSeqDirectAdvanceSite; 76] = [
    site!(
        0xE1,
        4,
        0x8006_F964,
        0x8006_F96C,
        0x8006_F970,
        Reg::S1,
        Reg::V0
    ),
    site!(
        0xE0,
        4,
        0x8006_FEA8,
        0x8006_FEB8,
        0x8006_FEBC,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0xE2,
        4,
        0x8006_FF10,
        0x8006_FF18,
        0x8006_FF1C,
        Reg::S1,
        Reg::V0
    ),
    site!(
        0xE3,
        12,
        0x8006_FF80,
        0x8006_FF88,
        0x8006_FF8C,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0xE4,
        4,
        0x8007_0080,
        0x8007_0088,
        0x8007_008C,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x82,
        12,
        0x8007_3368,
        0x8007_3370,
        0x8007_3374,
        Reg::S3,
        Reg::V0
    ),
    site!(
        0x83,
        12,
        0x8007_35AC,
        0x8007_35B4,
        0x8007_35B8,
        Reg::S1,
        Reg::V0
    ),
    site!(
        0x87,
        12,
        0x8007_3938,
        0x8007_3940,
        0x8007_3944,
        Reg::S2,
        Reg::V0
    ),
    site!(
        0x89,
        20,
        0x8007_3A64,
        0x8007_3A6C,
        0x8007_3A78,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x89,
        8,
        0x8007_3A88,
        0x8007_3A90,
        0x8007_3A98,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x8B,
        4,
        0x8007_3E9C,
        0x8007_3EA4,
        0x8007_41B0,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x8E,
        12,
        0x8007_42C4,
        0x8007_42CC,
        0x8007_42D0,
        Reg::S1,
        Reg::V0
    ),
    site!(
        0x97,
        12,
        0x8007_442C,
        0x8007_4434,
        0x8007_4438,
        Reg::S4,
        Reg::V0
    ),
    site!(
        0x98,
        12,
        0x8007_457C,
        0x8007_4584,
        0x8007_4588,
        Reg::S4,
        Reg::V0
    ),
    site!(
        0x99,
        12,
        0x8007_46CC,
        0x8007_46D4,
        0x8007_46D8,
        Reg::S4,
        Reg::V0
    ),
    site!(
        0x9B,
        12,
        0x8007_481C,
        0x8007_4824,
        0x8007_4828,
        Reg::S7,
        Reg::V0
    ),
    site!(
        0x9C,
        12,
        0x8007_496C,
        0x8007_4974,
        0x8007_4978,
        Reg::S7,
        Reg::V0
    ),
    site!(
        0xA0,
        8,
        0x8007_4ABC,
        0x8007_4AC4,
        0x8007_4ACC,
        Reg::A3,
        Reg::V0
    ),
    site!(
        0xA5,
        8,
        0x8007_4CCC,
        0x8007_4CD4,
        0x8007_4CD8,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0xAA,
        8,
        0x8007_4EA8,
        0x8007_4EB0,
        0x8007_4EB8,
        Reg::S1,
        Reg::V0
    ),
    site!(
        0xAA,
        8,
        0x8007_5090,
        0x8007_5098,
        0x8007_509C,
        Reg::S1,
        Reg::V0
    ),
    site!(
        0xAC,
        4,
        0x8007_55B0,
        0x8007_55B8,
        0x8007_55BC,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0xAE,
        4,
        0x8007_59F8,
        0x8007_5A0C,
        0x8007_5A18,
        Reg::S2,
        Reg::A0
    ),
    site!(
        0xB1,
        8,
        0x8007_5C10,
        0x8007_5C18,
        0x8007_5C20,
        Reg::A2,
        Reg::V0
    ),
    site!(
        0xB2,
        4,
        0x8007_5D18,
        0x8007_5D20,
        0x8007_5D28,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0xB7,
        8,
        0x8007_5EC8,
        0x8007_5ED0,
        0x8007_5ED4,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0xBA,
        8,
        0x8007_6040,
        0x8007_6048,
        0x8007_604C,
        Reg::S5,
        Reg::V0
    ),
    site!(
        0xBF,
        4,
        0x8007_6684,
        0x8007_668C,
        0x8007_6690,
        Reg::S1,
        Reg::V0
    ),
    site!(
        0xC3,
        4,
        0x8007_69BC,
        0x8007_69C4,
        0x8007_69C8,
        Reg::S2,
        Reg::V0
    ),
    site!(
        0xC4,
        4,
        0x8007_6AAC,
        0x8007_6AB4,
        0x8007_6ABC,
        Reg::T2,
        Reg::V0
    ),
    site!(
        0xC5,
        4,
        0x8007_6B2C,
        0x8007_6B34,
        0x8007_6B3C,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0xC8,
        4,
        0x8007_6DDC,
        0x8007_6DE4,
        0x8007_6DE8,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x80,
        8,
        0x8007_8188,
        0x8007_8190,
        0x8007_8194,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x81,
        4,
        0x8007_81B0,
        0x8007_81C4,
        0x8007_81D4,
        Reg::A0,
        Reg::V1
    ),
    site!(
        0x90,
        4,
        0x8007_826C,
        0x8007_8274,
        0x8007_827C,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x85,
        20,
        0x8007_834C,
        0x8007_8354,
        0x8007_835C,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x86,
        4,
        0x8007_839C,
        0x8007_83A4,
        0x8007_83AC,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x88,
        8,
        0x8007_8434,
        0x8007_843C,
        0x8007_8444,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x8C,
        4,
        0x8007_8490,
        0x8007_8498,
        0x8007_849C,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x8D,
        4,
        0x8007_8520,
        0x8007_8528,
        0x8007_852C,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x8F,
        8,
        0x8007_85B0,
        0x8007_85B8,
        0x8007_85C0,
        Reg::T0,
        Reg::V0
    ),
    site!(
        0x91,
        4,
        0x8007_8600,
        0x8007_8608,
        0x8007_8610,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x92,
        4,
        0x8007_863C,
        0x8007_8644,
        0x8007_864C,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x93,
        16,
        0x8007_8724,
        0x8007_872C,
        0x8007_8730,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x94,
        4,
        0x8007_87AC,
        0x8007_87B4,
        0x8007_87B8,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x95,
        8,
        0x8007_87E0,
        0x8007_87E8,
        0x8007_87F0,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x96,
        4,
        0x8007_8818,
        0x8007_8820,
        0x8007_8824,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x9A,
        4,
        0x8007_888C,
        0x8007_8894,
        0x8007_8898,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x9D,
        8,
        0x8007_8908,
        0x8007_8910,
        0x8007_8918,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x9E,
        8,
        0x8007_89B0,
        0x8007_89B8,
        0x8007_89BC,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x9F,
        8,
        0x8007_8A18,
        0x8007_8A20,
        0x8007_8A24,
        Reg::A0,
        Reg::V0
    ),
    site_from!(
        0xA1,
        12,
        0x8007_8A68,
        0x8007_8A7C,
        0x8007_8A8C,
        Reg::S0,
        Reg::A0,
        Reg::V0
    ),
    site_from!(
        0xA2,
        12,
        0x8007_8AC8,
        0x8007_8B10,
        0x8007_8B14,
        Reg::S0,
        Reg::A0,
        Reg::V0
    ),
    site_from!(
        0xA8,
        8,
        0x8007_8CDC,
        0x8007_8CF4,
        0x8007_8D08,
        Reg::A0,
        Reg::A1,
        Reg::V0
    ),
    site_from!(
        0xA9,
        8,
        0x8007_8D0C,
        0x8007_8D24,
        0x8007_8D5C,
        Reg::A0,
        Reg::A1,
        Reg::V0
    ),
    site!(
        0xA3,
        4,
        0x8007_8B50,
        0x8007_8B58,
        0x8007_8B5C,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0xA4,
        4,
        0x8007_8C44,
        0x8007_8C4C,
        0x8007_8C50,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0xA6,
        4,
        0x8007_8C80,
        0x8007_8C88,
        0x8007_8C90,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0xA7,
        4,
        0x8007_8CC8,
        0x8007_8CD0,
        0x8007_8CD8,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0xAB,
        4,
        0x8007_8DB4,
        0x8007_8DBC,
        0x8007_8DC4,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0xAF,
        4,
        0x8007_8F0C,
        0x8007_8F14,
        0x8007_8F18,
        Reg::A2,
        Reg::V0
    ),
    site!(
        0xB0,
        4,
        0x8007_8F44,
        0x8007_8F50,
        0x8007_8F58,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0xB3,
        16,
        0x8007_9008,
        0x8007_9010,
        0x8007_9014,
        Reg::S2,
        Reg::V0
    ),
    site!(
        0xB4,
        4,
        0x8007_90D0,
        0x8007_90D8,
        0x8007_90DC,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0xB5,
        8,
        0x8007_91AC,
        0x8007_91B4,
        0x8007_91B8,
        Reg::S2,
        Reg::V0
    ),
    site!(
        0xB6,
        12,
        0x8007_9254,
        0x8007_925C,
        0x8007_9260,
        Reg::S2,
        Reg::V0
    ),
    site!(
        0xB8,
        4,
        0x8007_9294,
        0x8007_929C,
        0x8007_92A4,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0xB9,
        4,
        0x8007_92DC,
        0x8007_92E4,
        0x8007_92E8,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0xBB,
        12,
        0x8007_93BC,
        0x8007_93C4,
        0x8007_93CC,
        Reg::A1,
        Reg::V0
    ),
    site!(
        0xBC,
        4,
        0x8007_9434,
        0x8007_943C,
        0x8007_9440,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0xBD,
        4,
        0x8007_9478,
        0x8007_9480,
        0x8007_9484,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0xBE,
        4,
        0x8007_95C8,
        0x8007_95D0,
        0x8007_95D4,
        Reg::S1,
        Reg::V0
    ),
    site!(
        0xC1,
        8,
        0x8007_9698,
        0x8007_96A0,
        0x8007_96A8,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0xC2,
        4,
        0x8007_9758,
        0x8007_9760,
        0x8007_9764,
        Reg::S1,
        Reg::V0
    ),
    site!(
        0xC6,
        4,
        0x8007_98A4,
        0x8007_98AC,
        0x8007_98B0,
        Reg::S0,
        Reg::V1
    ),
    site!(
        0xC7,
        4,
        0x8007_9960,
        0x8007_9968,
        0x8007_996C,
        Reg::S1,
        Reg::V0
    ),
];

/// 직접 고정 증가식이 없는 non-fallback opcode.
pub const QUEST_SEQ_WITHOUT_DIRECT_ADVANCE: [u8; 4] = [0x84, 0x8A, 0xAD, 0xC0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqAdvanceSummary {
    pub sites: usize,
    pub opcodes: usize,
    pub widths: [i16; 5],
    pub without_direct_advance: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestSeqAdvanceError {
    WrongExecutable(&'static str),
    TextLengthMismatch {
        expected: usize,
        actual: usize,
    },
    UnalignedText,
    MissingInstruction(u32),
    InstructionMismatch {
        opcode: u8,
        address: u32,
        expected: &'static str,
        actual: K,
    },
    FallbackOpcode(u8),
    SiteOutsideHandler {
        opcode: u8,
        handler: u32,
        next_handler: Option<u32>,
        address: u32,
    },
}

impl fmt::Display for QuestSeqAdvanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(f, "SEQ 직접 전진 검증 대상이 QUEST.EXE가 아님: {exe}")
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                f,
                "QUEST.EXE text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText => write!(f, "QUEST.EXE text 길이가 4바이트 정렬이 아님"),
            Self::MissingInstruction(address) => {
                write!(f, "SEQ 직접 전진 명령이 text 밖임: 0x{address:08X}")
            }
            Self::InstructionMismatch {
                opcode,
                address,
                expected,
                actual,
            } => write!(
                f,
                "SEQ OP{opcode:02X} 직접 전진 명령 불일치: address=0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
            Self::FallbackOpcode(opcode) => {
                write!(
                    f,
                    "SEQ OP{opcode:02X} 직접 전진 사양이 fallback handler를 가리킴"
                )
            }
            Self::SiteOutsideHandler {
                opcode,
                handler,
                next_handler,
                address,
            } => write!(
                f,
                "SEQ OP{opcode:02X} 직접 전진 site가 handler-local 범위 밖임: \
                 handler=0x{handler:08X}, next={next_handler:?}, address=0x{address:08X}"
            ),
        }
    }
}

impl Error for QuestSeqAdvanceError {}

pub fn verify_box_quest_seq_direct_advances(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqAdvanceSummary, QuestSeqAdvanceError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestSeqAdvanceError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestSeqAdvanceError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }
    let instructions = linear::sweep(map, text).ok_or(QuestSeqAdvanceError::UnalignedText)?;
    let mut seen = [false; 256];
    let mut opcode_count = 0;

    for site in QUEST_SEQ_DIRECT_ADVANCE_SITES {
        let handler = quest_seq_opcode_handler(site.opcode)
            .ok_or(QuestSeqAdvanceError::FallbackOpcode(site.opcode))?;
        if handler == QUEST_SEQ_MAIN_FALLBACK_HANDLER_RUNTIME
            || handler == QUEST_SEQ_EXTENDED_FALLBACK_HANDLER_RUNTIME
        {
            return Err(QuestSeqAdvanceError::FallbackOpcode(site.opcode));
        }
        let next_handler = QUEST_SEQ_DISPATCH_HANDLERS
            .iter()
            .copied()
            .filter(|&candidate| candidate > handler)
            .min();
        if site.load_runtime < handler
            || next_handler.is_some_and(|next| site.store_runtime >= next)
            || !(site.load_runtime < site.increment_runtime
                && site.increment_runtime < site.store_runtime)
        {
            return Err(QuestSeqAdvanceError::SiteOutsideHandler {
                opcode: site.opcode,
                handler,
                next_handler,
                address: site.load_runtime,
            });
        }
        if !seen[usize::from(site.opcode)] {
            seen[usize::from(site.opcode)] = true;
            opcode_count += 1;
        }
        require(
            &instructions,
            site,
            site.load_runtime,
            "lw value, 0x1C(object)",
            K::Lw {
                rt: site.source_reg,
                base: site.object_reg,
                offset: QUEST_SEQ_STREAM_POINTER_FIELD_OFFSET,
            },
        )?;
        require(
            &instructions,
            site,
            site.increment_runtime,
            "addiu value, value, record_bytes",
            K::Addiu {
                rt: site.value_reg,
                rs: site.source_reg,
                imm: site.record_bytes,
            },
        )?;
        require(
            &instructions,
            site,
            site.store_runtime,
            "sw value, 0x1C(object)",
            K::Sw {
                rt: site.value_reg,
                base: site.object_reg,
                offset: QUEST_SEQ_STREAM_POINTER_FIELD_OFFSET,
            },
        )?;
    }

    Ok(QuestSeqAdvanceSummary {
        sites: QUEST_SEQ_DIRECT_ADVANCE_SITES.len(),
        opcodes: opcode_count,
        widths: [4, 8, 12, 16, 20],
        without_direct_advance: QUEST_SEQ_WITHOUT_DIRECT_ADVANCE,
    })
}

fn require(
    instructions: &[Instruction],
    site: QuestSeqDirectAdvanceSite,
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestSeqAdvanceError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or(QuestSeqAdvanceError::MissingInstruction(address))?;
    if actual.kind != kind {
        return Err(QuestSeqAdvanceError::InstructionMismatch {
            opcode: site.opcode,
            address,
            expected,
            actual: actual.kind,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_seq_advance_tests.rs"]
mod tests;
