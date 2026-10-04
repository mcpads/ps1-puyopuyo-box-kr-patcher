//! QUEST 공용 SEQ base handler의 직접 stream 전진 증거.
//!
//! 공용 fallback과 활성 handler 37종에서 현재 stream field `+0x1C`를
//! 읽고 고정값을 더해 같은 field에 저장하는 71개 경로를 검증한다.
//! branch/return/OP28 위임 9종은 별도 전이 모듈이 소유한다. 이는 각
//! 정적 경로의 직접 증가 사실이며 handler 전체를 단일 record 폭으로
//! 축약하지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::{
    quest_seq_opcode_handler, QUEST_SEQ_BASE_DISPATCH_HANDLERS,
    QUEST_SEQ_BASE_FALLBACK_HANDLER_RUNTIME,
};

use super::linear;

pub const QUEST_SEQ_BASE_WITHOUT_DIRECT_ADVANCE: [u8; 9] =
    [0x03, 0x04, 0x05, 0x09, 0x0A, 0x0D, 0x0E, 0x28, 0x39];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqBaseDirectAdvanceSite {
    pub opcode: u8,
    pub record_bytes: i16,
    pub load_runtime: u32,
    pub move_runtime: Option<u32>,
    pub increment_runtime: u32,
    pub store_runtime: u32,
    pub object_reg: Reg,
    pub load_reg: Reg,
    pub increment_source_reg: Reg,
    pub value_reg: Reg,
}

macro_rules! site {
    ($opcode:expr, $bytes:expr, $load:expr, $increment:expr, $store:expr, $object:expr, $value:expr) => {
        QuestSeqBaseDirectAdvanceSite {
            opcode: $opcode,
            record_bytes: $bytes,
            load_runtime: $load,
            move_runtime: None,
            increment_runtime: $increment,
            store_runtime: $store,
            object_reg: $object,
            load_reg: $value,
            increment_source_reg: $value,
            value_reg: $value,
        }
    };
}

macro_rules! site_alias {
    ($opcode:expr, $bytes:expr, $load:expr, $move:expr, $increment:expr, $store:expr, $object:expr, $load_reg:expr, $value:expr) => {
        QuestSeqBaseDirectAdvanceSite {
            opcode: $opcode,
            record_bytes: $bytes,
            load_runtime: $load,
            move_runtime: Some($move),
            increment_runtime: $increment,
            store_runtime: $store,
            object_reg: $object,
            load_reg: $load_reg,
            increment_source_reg: $value,
            value_reg: $value,
        }
    };
}

macro_rules! site_from {
    ($opcode:expr, $bytes:expr, $load:expr, $increment:expr, $store:expr, $object:expr, $load_reg:expr, $value:expr) => {
        QuestSeqBaseDirectAdvanceSite {
            opcode: $opcode,
            record_bytes: $bytes,
            load_runtime: $load,
            move_runtime: None,
            increment_runtime: $increment,
            store_runtime: $store,
            object_reg: $object,
            load_reg: $load_reg,
            increment_source_reg: $load_reg,
            value_reg: $value,
        }
    };
}

pub const QUEST_SEQ_BASE_DIRECT_ADVANCE_SITES: [QuestSeqBaseDirectAdvanceSite; 71] = [
    site!(
        0x2B,
        4,
        0x8002_263C,
        0x8002_2644,
        0x8002_264C,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x00,
        4,
        0x8002_2664,
        0x8002_266C,
        0x8002_2670,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x01,
        4,
        0x8002_268C,
        0x8002_2698,
        0x8002_26A0,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x02,
        4,
        0x8002_26D0,
        0x8002_26D8,
        0x8002_26E0,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x06,
        8,
        0x8002_27AC,
        0x8002_27B4,
        0x8002_27BC,
        Reg::A0,
        Reg::V0
    ),
    site_from!(
        0x07,
        4,
        0x8002_27D0,
        0x8002_27D8,
        0x8002_27DC,
        Reg::A0,
        Reg::V1,
        Reg::V0
    ),
    site!(
        0x07,
        8,
        0x8002_2874,
        0x8002_287C,
        0x8002_288C,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x07,
        4,
        0x8002_2880,
        0x8002_2888,
        0x8002_288C,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x07,
        4,
        0x8002_28A8,
        0x8002_28B0,
        0x8002_28B8,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x08,
        8,
        0x8001_FFF8,
        0x8002_0004,
        0x8002_0008,
        Reg::A0,
        Reg::V0
    ),
    site_from!(
        0x08,
        4,
        0x8002_0040,
        0x8002_0064,
        0x8002_0124,
        Reg::A0,
        Reg::A1,
        Reg::V0
    ),
    site_from!(
        0x08,
        4,
        0x8002_0070,
        0x8002_0094,
        0x8002_0124,
        Reg::A0,
        Reg::A1,
        Reg::V0
    ),
    site_from!(
        0x08,
        8,
        0x8002_00A0,
        0x8002_00C4,
        0x8002_0158,
        Reg::A0,
        Reg::A1,
        Reg::V0
    ),
    site_from!(
        0x08,
        4,
        0x8002_00F4,
        0x8002_0118,
        0x8002_0124,
        Reg::A0,
        Reg::A1,
        Reg::V0
    ),
    site_from!(
        0x08,
        8,
        0x8002_0128,
        0x8002_014C,
        0x8002_0158,
        Reg::A0,
        Reg::A1,
        Reg::V0
    ),
    site_from!(
        0x08,
        4,
        0x8002_0128,
        0x8002_015C,
        0x8002_0160,
        Reg::A0,
        Reg::A1,
        Reg::V0
    ),
    site_from!(
        0x0B,
        4,
        0x8002_0188,
        0x8002_0190,
        0x8002_0194,
        Reg::A0,
        Reg::V1,
        Reg::V0
    ),
    site!(
        0x0B,
        8,
        0x8002_0230,
        0x8002_0238,
        0x8002_0248,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x0B,
        4,
        0x8002_023C,
        0x8002_0244,
        0x8002_0248,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x0B,
        4,
        0x8002_0264,
        0x8002_026C,
        0x8002_0274,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x0C,
        8,
        0x8002_0288,
        0x8002_0294,
        0x8002_0298,
        Reg::A0,
        Reg::V0
    ),
    site_from!(
        0x0C,
        4,
        0x8002_02D0,
        0x8002_02F8,
        0x8002_03C8,
        Reg::A0,
        Reg::A1,
        Reg::V0
    ),
    site_from!(
        0x0C,
        4,
        0x8002_0304,
        0x8002_032C,
        0x8002_03C8,
        Reg::A0,
        Reg::A1,
        Reg::V0
    ),
    site_from!(
        0x0C,
        8,
        0x8002_0338,
        0x8002_0360,
        0x8002_0400,
        Reg::A0,
        Reg::A1,
        Reg::V0
    ),
    site_from!(
        0x0C,
        4,
        0x8002_0394,
        0x8002_03BC,
        0x8002_03C8,
        Reg::A0,
        Reg::A1,
        Reg::V0
    ),
    site_from!(
        0x0C,
        8,
        0x8002_03CC,
        0x8002_03F4,
        0x8002_0400,
        Reg::A0,
        Reg::A1,
        Reg::V0
    ),
    site_from!(
        0x0C,
        4,
        0x8002_03CC,
        0x8002_0404,
        0x8002_0408,
        Reg::A0,
        Reg::A1,
        Reg::V0
    ),
    site!(
        0x0F,
        4,
        0x8002_0704,
        0x8002_070C,
        0x8002_0710,
        Reg::S2,
        Reg::V0
    ),
    site!(
        0x10,
        4,
        0x8002_0A10,
        0x8002_0A18,
        0x8002_0A28,
        Reg::S2,
        Reg::V0
    ),
    site!(
        0x10,
        8,
        0x8002_0A1C,
        0x8002_0A24,
        0x8002_0A28,
        Reg::S2,
        Reg::V0
    ),
    site!(
        0x11,
        4,
        0x8002_0D08,
        0x8002_0D10,
        0x8002_0D20,
        Reg::S2,
        Reg::V0
    ),
    site!(
        0x11,
        8,
        0x8002_0D14,
        0x8002_0D1C,
        0x8002_0D20,
        Reg::S2,
        Reg::V0
    ),
    site!(
        0x12,
        4,
        0x8002_0E64,
        0x8002_0E6C,
        0x8002_0E70,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x12,
        4,
        0x8002_0F30,
        0x8002_0F38,
        0x8002_0F40,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x13,
        8,
        0x8002_100C,
        0x8002_101C,
        0x8002_1088,
        Reg::T0,
        Reg::V0
    ),
    site!(
        0x13,
        4,
        0x8002_1020,
        0x8002_103C,
        0x8002_1044,
        Reg::T0,
        Reg::A0
    ),
    site!(
        0x13,
        4,
        0x8002_1048,
        0x8002_1060,
        0x8002_1068,
        Reg::T0,
        Reg::V1
    ),
    site!(
        0x13,
        4,
        0x8002_106C,
        0x8002_1084,
        0x8002_1088,
        Reg::T0,
        Reg::V0
    ),
    site!(
        0x13,
        4,
        0x8002_114C,
        0x8002_1154,
        0x8002_115C,
        Reg::T0,
        Reg::V0
    ),
    site!(
        0x14,
        8,
        0x8002_1228,
        0x8002_1238,
        0x8002_12A4,
        Reg::T0,
        Reg::V0
    ),
    site!(
        0x14,
        4,
        0x8002_123C,
        0x8002_1258,
        0x8002_1260,
        Reg::T0,
        Reg::A0
    ),
    site!(
        0x14,
        4,
        0x8002_1264,
        0x8002_127C,
        0x8002_1284,
        Reg::T0,
        Reg::V1
    ),
    site!(
        0x14,
        4,
        0x8002_1288,
        0x8002_12A0,
        0x8002_12A4,
        Reg::T0,
        Reg::V0
    ),
    site!(
        0x14,
        4,
        0x8002_1368,
        0x8002_1370,
        0x8002_1378,
        Reg::T0,
        Reg::V0
    ),
    site!(
        0x15,
        8,
        0x8002_1644,
        0x8002_164C,
        0x8002_1650,
        Reg::S2,
        Reg::V0
    ),
    site!(
        0x16,
        8,
        0x8002_1938,
        0x8002_1940,
        0x8002_1944,
        Reg::S2,
        Reg::V0
    ),
    site!(
        0x17,
        8,
        0x8002_1C14,
        0x8002_1C1C,
        0x8002_1C20,
        Reg::S2,
        Reg::V0
    ),
    site!(
        0x18,
        4,
        0x8002_1F5C,
        0x8002_1F64,
        0x8002_1F6C,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x19,
        4,
        0x8002_29E0,
        0x8002_29E8,
        0x8002_29F0,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x1A,
        4,
        0x8002_2B0C,
        0x8002_2B14,
        0x8002_2B18,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x1B,
        4,
        0x8002_2C58,
        0x8002_2C60,
        0x8002_2C64,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x1C,
        8,
        0x8002_2CB4,
        0x8002_2CBC,
        0x8002_2CC0,
        Reg::S0,
        Reg::V1
    ),
    site!(
        0x1D,
        4,
        0x8002_2CE4,
        0x8002_2CEC,
        0x8002_2CF4,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x1E,
        4,
        0x8002_2D1C,
        0x8002_2D24,
        0x8002_2D28,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x1F,
        4,
        0x8002_2DA4,
        0x8002_2DAC,
        0x8002_2DB4,
        Reg::A2,
        Reg::V0
    ),
    site!(
        0x20,
        8,
        0x8002_2EE8,
        0x8002_2EF0,
        0x8002_2EF4,
        Reg::S1,
        Reg::V0
    ),
    site!(
        0x21,
        4,
        0x8002_2F0C,
        0x8002_2F18,
        0x8002_2F20,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x22,
        8,
        0x8002_2F24,
        0x8002_2F30,
        0x8002_2F38,
        Reg::A0,
        Reg::V0
    ),
    site_alias!(
        0x23,
        4,
        0x8002_2F48,
        0x8002_2F58,
        0x8002_2F60,
        0x8002_2F64,
        Reg::A0,
        Reg::V1,
        Reg::V0
    ),
    site_alias!(
        0x24,
        4,
        0x8002_2F78,
        0x8002_2F80,
        0x8002_2F88,
        0x8002_2F8C,
        Reg::A0,
        Reg::V1,
        Reg::V0
    ),
    site!(
        0x25,
        8,
        0x8002_2FBC,
        0x8002_2FC8,
        0x8002_2FCC,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x26,
        4,
        0x8002_3024,
        0x8002_302C,
        0x8002_3034,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x27,
        12,
        0x8002_30E0,
        0x8002_30E8,
        0x8002_30F0,
        Reg::A2,
        Reg::V0
    ),
    site!(
        0x29,
        8,
        0x8002_3180,
        0x8002_3188,
        0x8002_318C,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x2A,
        4,
        0x8002_31D4,
        0x8002_31DC,
        0x8002_31E0,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x33,
        12,
        0x8002_3280,
        0x8002_3288,
        0x8002_32C8,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x33,
        8,
        0x8002_32BC,
        0x8002_32C4,
        0x8002_32C8,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x3A,
        8,
        0x8002_23F8,
        0x8002_2400,
        0x8002_2480,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x3A,
        4,
        0x8002_240C,
        0x8002_2414,
        0x8002_2424,
        Reg::S0,
        Reg::A0
    ),
    site!(
        0x3A,
        4,
        0x8002_2428,
        0x8002_2434,
        0x8002_2480,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x3A,
        4,
        0x8002_2474,
        0x8002_247C,
        0x8002_2480,
        Reg::S0,
        Reg::V0
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqBaseAdvanceSummary {
    pub sites: usize,
    pub active_opcodes: usize,
    pub handlers: usize,
    pub fallback_entries: usize,
    pub widths: [i16; 3],
    pub without_direct_advance: [u8; 9],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestSeqBaseAdvanceError {
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
    SiteOutsideHandler {
        opcode: u8,
        handler: u32,
        next_handler: Option<u32>,
        address: u32,
    },
    CoverageMismatch {
        opcode: u8,
        direct: bool,
        transition: bool,
    },
    HandlerCoverage {
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for QuestSeqBaseAdvanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(f, "SEQ base 직접 전진 검증 대상이 QUEST.EXE가 아님: {exe}")
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                f,
                "QUEST.EXE text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText => write!(f, "QUEST.EXE text 길이가 4바이트 정렬이 아님"),
            Self::MissingInstruction(address) => {
                write!(f, "SEQ base 직접 전진 명령이 text 밖임: 0x{address:08X}")
            }
            Self::InstructionMismatch {
                opcode,
                address,
                expected,
                actual,
            } => write!(
                f,
                "SEQ base OP{opcode:02X} 직접 전진 명령 불일치: \
                 address=0x{address:08X}, expected={expected}, actual={actual:?}"
            ),
            Self::SiteOutsideHandler {
                opcode,
                handler,
                next_handler,
                address,
            } => write!(
                f,
                "SEQ base OP{opcode:02X} 직접 전진 site가 handler-local 범위 밖임: \
                 handler=0x{handler:08X}, next={next_handler:?}, address=0x{address:08X}"
            ),
            Self::CoverageMismatch {
                opcode,
                direct,
                transition,
            } => write!(
                f,
                "SEQ base OP{opcode:02X} 직접/전이 분류 불일치: \
                 direct={direct}, transition={transition}"
            ),
            Self::HandlerCoverage { expected, actual } => write!(
                f,
                "SEQ base 직접 전진 handler 분모 불일치: expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for QuestSeqBaseAdvanceError {}

pub fn verify_box_quest_seq_base_direct_advances(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqBaseAdvanceSummary, QuestSeqBaseAdvanceError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestSeqBaseAdvanceError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestSeqBaseAdvanceError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }
    let instructions = linear::sweep(map, text).ok_or(QuestSeqBaseAdvanceError::UnalignedText)?;
    let unique_handlers = QUEST_SEQ_BASE_DISPATCH_HANDLERS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut direct = [false; 0x80];
    let mut covered_handlers = BTreeSet::new();

    for site in QUEST_SEQ_BASE_DIRECT_ADVANCE_SITES {
        let handler = quest_seq_opcode_handler(site.opcode).expect("base opcode");
        let next_handler = unique_handlers
            .iter()
            .copied()
            .find(|&candidate| candidate > handler);
        if site.load_runtime < handler
            || next_handler.is_some_and(|next| site.store_runtime >= next)
            || !(site.load_runtime < site.increment_runtime
                && site.increment_runtime < site.store_runtime)
        {
            return Err(QuestSeqBaseAdvanceError::SiteOutsideHandler {
                opcode: site.opcode,
                handler,
                next_handler,
                address: site.load_runtime,
            });
        }
        covered_handlers.insert(handler);
        if handler != QUEST_SEQ_BASE_FALLBACK_HANDLER_RUNTIME {
            direct[usize::from(site.opcode)] = true;
        }
        require(
            &instructions,
            site.opcode,
            site.load_runtime,
            "lw load_reg, 0x1C(object)",
            K::Lw {
                rt: site.load_reg,
                base: site.object_reg,
                offset: 0x1C,
            },
        )?;
        if let Some(address) = site.move_runtime {
            require(
                &instructions,
                site.opcode,
                address,
                "move value, load_reg",
                K::Addu {
                    rd: site.value_reg,
                    rs: site.load_reg,
                    rt: Reg::ZERO,
                },
            )?;
        }
        require(
            &instructions,
            site.opcode,
            site.increment_runtime,
            "addiu value, value, record_bytes",
            K::Addiu {
                rt: site.value_reg,
                rs: site.increment_source_reg,
                imm: site.record_bytes,
            },
        )?;
        require(
            &instructions,
            site.opcode,
            site.store_runtime,
            "sw value, 0x1C(object)",
            K::Sw {
                rt: site.value_reg,
                base: site.object_reg,
                offset: 0x1C,
            },
        )?;
    }

    let mut transitions = [false; 0x80];
    for opcode in QUEST_SEQ_BASE_WITHOUT_DIRECT_ADVANCE {
        transitions[usize::from(opcode)] = true;
    }
    let mut active_opcodes = 0;
    for (opcode, &handler) in QUEST_SEQ_BASE_DISPATCH_HANDLERS.iter().enumerate() {
        if handler == QUEST_SEQ_BASE_FALLBACK_HANDLER_RUNTIME {
            continue;
        }
        if direct[opcode] == transitions[opcode] {
            return Err(QuestSeqBaseAdvanceError::CoverageMismatch {
                opcode: u8::try_from(opcode).expect("base opcode"),
                direct: direct[opcode],
                transition: transitions[opcode],
            });
        }
        if direct[opcode] {
            active_opcodes += 1;
        }
    }
    if covered_handlers.len() != 38 {
        return Err(QuestSeqBaseAdvanceError::HandlerCoverage {
            expected: 38,
            actual: covered_handlers.len(),
        });
    }

    Ok(QuestSeqBaseAdvanceSummary {
        sites: QUEST_SEQ_BASE_DIRECT_ADVANCE_SITES.len(),
        active_opcodes,
        handlers: covered_handlers.len(),
        fallback_entries: QUEST_SEQ_BASE_DISPATCH_HANDLERS
            .iter()
            .filter(|&&handler| handler == QUEST_SEQ_BASE_FALLBACK_HANDLER_RUNTIME)
            .count(),
        widths: [4, 8, 12],
        without_direct_advance: QUEST_SEQ_BASE_WITHOUT_DIRECT_ADVANCE,
    })
}

fn require(
    instructions: &[Instruction],
    opcode: u8,
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestSeqBaseAdvanceError> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or(QuestSeqBaseAdvanceError::MissingInstruction(address))?;
    if actual.kind != kind {
        return Err(QuestSeqBaseAdvanceError::InstructionMismatch {
            opcode,
            address,
            expected,
            actual: actual.kind,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_seq_base_advance_tests.rs"]
mod tests;
