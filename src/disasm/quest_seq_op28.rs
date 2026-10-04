//! 공용 SEQ OP28 하위 dispatch와 stream 소비 검증.
//!
//! OP28 handler, 32-entry table, 14개 고유 handler의 직접 전진 15개
//! 경로와 sub00/01의 pointer branch 경로를 원본 `QUEST.EXE`에서 함께
//! 검증한다. dispatch 등록과 정적 소비 사실이며 실제 subopcode 도달을
//! 뜻하지 않는다.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::{
    quest_seq_op28_fallback_entry_count, quest_seq_op28_flow, quest_seq_op28_unique_handler_count,
    quest_seq_opcode_handler, QuestSeqOp28Flow, QUEST_SEQ_OP28_DISPATCH_HANDLERS,
    QUEST_SEQ_OP28_DISPATCH_RUNTIME, QUEST_SEQ_OP28_DISPATCH_TABLE_RUNTIME,
    QUEST_SEQ_OP28_HANDLER_RUNTIME, QUEST_SEQ_OP28_SUBOPCODE_COUNT,
};

use super::linear;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqOp28DirectAdvanceSite {
    pub subopcode: u8,
    pub record_bytes: i16,
    pub load_runtime: u32,
    pub increment_runtime: u32,
    pub store_runtime: u32,
    pub object_reg: Reg,
    pub source_reg: Reg,
    pub value_reg: Reg,
}

macro_rules! site {
    ($subopcode:expr, $bytes:expr, $load:expr, $increment:expr, $store:expr, $object:expr, $value:expr) => {
        QuestSeqOp28DirectAdvanceSite {
            subopcode: $subopcode,
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
    ($subopcode:expr, $bytes:expr, $load:expr, $increment:expr, $store:expr, $object:expr, $source:expr, $value:expr) => {
        QuestSeqOp28DirectAdvanceSite {
            subopcode: $subopcode,
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

pub const QUEST_SEQ_OP28_DIRECT_ADVANCE_SITES: [QuestSeqOp28DirectAdvanceSite; 15] = [
    site!(
        0x05,
        4,
        0x8001_C3C8,
        0x8001_C3D0,
        0x8001_C3D8,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x00,
        8,
        0x8001_C3F8,
        0x8001_C400,
        0x8001_C408,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x01,
        12,
        0x8001_C494,
        0x8001_C49C,
        0x8001_C4A4,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x02,
        4,
        0x8001_C4F8,
        0x8001_C500,
        0x8001_C504,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x03,
        8,
        0x8001_C55C,
        0x8001_C564,
        0x8001_C568,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x04,
        8,
        0x8001_C5C0,
        0x8001_C5C8,
        0x8001_C5CC,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x0B,
        8,
        0x8001_C608,
        0x8001_C610,
        0x8001_C614,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x0C,
        8,
        0x8001_C654,
        0x8001_C65C,
        0x8001_C660,
        Reg::S0,
        Reg::V0
    ),
    site!(
        0x06,
        4,
        0x8001_C674,
        0x8001_C67C,
        0x8001_C684,
        Reg::A0,
        Reg::V0
    ),
    site!(
        0x10,
        16,
        0x8001_88C4,
        0x8001_88CC,
        0x8001_88D0,
        Reg::S3,
        Reg::V0
    ),
    site!(
        0x12,
        16,
        0x8001_8CA0,
        0x8001_8CA8,
        0x8001_8CAC,
        Reg::S3,
        Reg::V0
    ),
    site!(
        0x18,
        4,
        0x8001_C760,
        0x8001_C768,
        0x8001_C770,
        Reg::A3,
        Reg::V0
    ),
    site_from!(
        0x19,
        4,
        0x8001_C78C,
        0x8001_C7D4,
        0x8001_C8B4,
        Reg::S2,
        Reg::A1,
        Reg::V0
    ),
    site!(
        0x19,
        4,
        0x8001_C8A8,
        0x8001_C8B0,
        0x8001_C8B4,
        Reg::S2,
        Reg::V0
    ),
    site!(
        0x1A,
        4,
        0x8001_C8FC,
        0x8001_C904,
        0x8001_C90C,
        Reg::A0,
        Reg::V0
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestSeqOp28Summary {
    pub dispatch_entries: usize,
    pub unique_handlers: usize,
    pub fallback_entries: usize,
    pub direct_advance_sites: usize,
    pub direct_advance_handlers: usize,
    pub branch_subopcodes: usize,
    pub widths: [i16; 4],
    pub dispatch_runtime: u32,
    pub dispatch_table_runtime: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestSeqOp28Error {
    WrongExecutable(&'static str),
    TextLengthMismatch {
        expected: usize,
        actual: usize,
    },
    UnalignedText,
    HandlerMismatch {
        actual: Option<u32>,
    },
    MissingInstruction(u32),
    InstructionMismatch {
        subopcode: Option<u8>,
        address: u32,
        expected: &'static str,
        actual: K,
    },
    DispatchWordMismatch {
        subopcode: u8,
        expected: u32,
        actual: u32,
    },
    SiteOutsideHandler {
        subopcode: u8,
        handler: u32,
        next_handler: Option<u32>,
        address: u32,
    },
    FlowCoverage {
        subopcode: u8,
        handler: u32,
        bytes: i16,
    },
    HandlerCoverage {
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for QuestSeqOp28Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(f, "SEQ OP28 검증 대상이 QUEST.EXE가 아님: {exe}")
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                f,
                "QUEST.EXE text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText => write!(f, "QUEST.EXE text 길이가 4바이트 정렬이 아님"),
            Self::HandlerMismatch { actual } => write!(
                f,
                "SEQ OP28 handler 불일치: expected=0x{QUEST_SEQ_OP28_HANDLER_RUNTIME:08X}, \
                 actual={actual:?}"
            ),
            Self::MissingInstruction(address) => {
                write!(f, "SEQ OP28 명령이 text 밖임: 0x{address:08X}")
            }
            Self::InstructionMismatch {
                subopcode,
                address,
                expected,
                actual,
            } => write!(
                f,
                "SEQ OP28 sub{subopcode:02X?} 명령 불일치: address=0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
            Self::DispatchWordMismatch {
                subopcode,
                expected,
                actual,
            } => write!(
                f,
                "SEQ OP28 sub{subopcode:02X} dispatch word 불일치: \
                 expected=0x{expected:08X}, actual=0x{actual:08X}"
            ),
            Self::SiteOutsideHandler {
                subopcode,
                handler,
                next_handler,
                address,
            } => write!(
                f,
                "SEQ OP28 sub{subopcode:02X} 전진 site가 handler-local 범위 밖임: \
                 handler=0x{handler:08X}, next={next_handler:?}, address=0x{address:08X}"
            ),
            Self::FlowCoverage {
                subopcode,
                handler,
                bytes,
            } => write!(
                f,
                "SEQ OP28 sub{subopcode:02X} flow를 직접 전진 site가 덮지 않음: \
                 handler=0x{handler:08X}, bytes={bytes}"
            ),
            Self::HandlerCoverage { expected, actual } => write!(
                f,
                "SEQ OP28 고유 handler 전진 분모 불일치: expected={expected}, actual={actual}"
            ),
        }
    }
}

impl Error for QuestSeqOp28Error {}

pub fn verify_box_quest_seq_op28(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestSeqOp28Summary, QuestSeqOp28Error> {
    if map.name != "QUEST.EXE" {
        return Err(QuestSeqOp28Error::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestSeqOp28Error::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }
    let instructions = linear::sweep(map, text).ok_or(QuestSeqOp28Error::UnalignedText)?;
    if quest_seq_opcode_handler(0x28) != Some(QUEST_SEQ_OP28_HANDLER_RUNTIME) {
        return Err(QuestSeqOp28Error::HandlerMismatch {
            actual: quest_seq_opcode_handler(0x28),
        });
    }

    verify_dispatcher(&instructions)?;
    verify_dispatch_words(map, text)?;
    verify_branch_paths(&instructions)?;

    let unique_handlers = QUEST_SEQ_OP28_DISPATCH_HANDLERS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut covered_handlers = BTreeSet::new();
    for site in QUEST_SEQ_OP28_DIRECT_ADVANCE_SITES {
        let handler = QUEST_SEQ_OP28_DISPATCH_HANDLERS[usize::from(site.subopcode)];
        let next_handler = unique_handlers
            .iter()
            .copied()
            .find(|&candidate| candidate > handler);
        if site.load_runtime < handler
            || next_handler.is_some_and(|next| site.store_runtime >= next)
            || !(site.load_runtime < site.increment_runtime
                && site.increment_runtime < site.store_runtime)
        {
            return Err(QuestSeqOp28Error::SiteOutsideHandler {
                subopcode: site.subopcode,
                handler,
                next_handler,
                address: site.load_runtime,
            });
        }
        covered_handlers.insert(handler);
        require(
            &instructions,
            Some(site.subopcode),
            site.load_runtime,
            "lw source, 0x1C(object)",
            K::Lw {
                rt: site.source_reg,
                base: site.object_reg,
                offset: 0x1C,
            },
        )?;
        require(
            &instructions,
            Some(site.subopcode),
            site.increment_runtime,
            "addiu value, source, record_bytes",
            K::Addiu {
                rt: site.value_reg,
                rs: site.source_reg,
                imm: site.record_bytes,
            },
        )?;
        require(
            &instructions,
            Some(site.subopcode),
            site.store_runtime,
            "sw value, 0x1C(object)",
            K::Sw {
                rt: site.value_reg,
                base: site.object_reg,
                offset: 0x1C,
            },
        )?;
    }
    if covered_handlers != unique_handlers {
        return Err(QuestSeqOp28Error::HandlerCoverage {
            expected: unique_handlers.len(),
            actual: covered_handlers.len(),
        });
    }

    for subopcode in 0u8..u8::try_from(QUEST_SEQ_OP28_SUBOPCODE_COUNT).expect("32") {
        let handler = QUEST_SEQ_OP28_DISPATCH_HANDLERS[usize::from(subopcode)];
        let bytes = match quest_seq_op28_flow(subopcode).expect("32-entry flow") {
            QuestSeqOp28Flow::FixedAdvance { bytes }
            | QuestSeqOp28Flow::AdvanceOrBranch {
                advance_bytes: bytes,
                ..
            } => bytes,
        };
        if !QUEST_SEQ_OP28_DIRECT_ADVANCE_SITES.iter().any(|site| {
            QUEST_SEQ_OP28_DISPATCH_HANDLERS[usize::from(site.subopcode)] == handler
                && site.record_bytes == bytes
        }) {
            return Err(QuestSeqOp28Error::FlowCoverage {
                subopcode,
                handler,
                bytes,
            });
        }
    }

    Ok(QuestSeqOp28Summary {
        dispatch_entries: QUEST_SEQ_OP28_SUBOPCODE_COUNT,
        unique_handlers: quest_seq_op28_unique_handler_count(),
        fallback_entries: quest_seq_op28_fallback_entry_count(),
        direct_advance_sites: QUEST_SEQ_OP28_DIRECT_ADVANCE_SITES.len(),
        direct_advance_handlers: covered_handlers.len(),
        branch_subopcodes: 2,
        widths: [4, 8, 12, 16],
        dispatch_runtime: QUEST_SEQ_OP28_DISPATCH_RUNTIME,
        dispatch_table_runtime: QUEST_SEQ_OP28_DISPATCH_TABLE_RUNTIME,
    })
}

fn verify_dispatcher(instructions: &[Instruction]) -> Result<(), QuestSeqOp28Error> {
    for (address, expected, kind) in [
        (
            0x8002_30FC,
            "jal OP28 dispatcher",
            K::Jal {
                target: QUEST_SEQ_OP28_DISPATCH_RUNTIME,
            },
        ),
        (
            0x8001_B0B0,
            "lw v0, 0x1C(a0)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0x1C,
            },
        ),
        (
            0x8001_B0B4,
            "lui v1, 0x800B",
            K::Lui {
                rt: Reg::V1,
                imm: 0x800B,
            },
        ),
        (
            0x8001_B0B8,
            "lbu v0, 1(v0)",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            0x8001_B0BC,
            "addiu v1, v1, 0x1A74",
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x1A74,
            },
        ),
        (
            0x8001_B0C0,
            "sll v0, v0, 2",
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8001_B0C4,
            "addu v0, v0, v1",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8001_B0C8,
            "lw v0, 0(v0)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8001_B0D0,
            "jalr v0",
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
    ] {
        require(instructions, None, address, expected, kind)?;
    }
    Ok(())
}

fn verify_dispatch_words(map: &ExeMap, text: &[u8]) -> Result<(), QuestSeqOp28Error> {
    for (index, expected) in QUEST_SEQ_OP28_DISPATCH_HANDLERS.iter().copied().enumerate() {
        let address = QUEST_SEQ_OP28_DISPATCH_TABLE_RUNTIME + u32::try_from(index).expect("32") * 4;
        let offset = map
            .text_offset(address)
            .ok_or(QuestSeqOp28Error::MissingInstruction(address))?;
        let actual = u32::from_le_bytes(
            text.get(offset..offset + 4)
                .ok_or(QuestSeqOp28Error::MissingInstruction(address))?
                .try_into()
                .expect("4바이트 범위를 확인함"),
        );
        if actual != expected {
            return Err(QuestSeqOp28Error::DispatchWordMismatch {
                subopcode: u8::try_from(index).expect("32"),
                expected,
                actual,
            });
        }
    }
    Ok(())
}

fn verify_branch_paths(instructions: &[Instruction]) -> Result<(), QuestSeqOp28Error> {
    for (
        subopcode,
        current,
        flag,
        branch,
        pointer,
        store,
        yield_lui,
        yield_store,
        pointer_offset,
    ) in [
        (
            0x00,
            0x8001_C40C,
            0x8001_C414,
            0x8001_C41C,
            0x8001_C424,
            0x8001_C42C,
            0x8001_C430,
            0x8001_C434,
            4,
        ),
        (
            0x01,
            0x8001_C4A8,
            0x8001_C4B0,
            0x8001_C4B8,
            0x8001_C4C0,
            0x8001_C4C8,
            0x8001_C4CC,
            0x8001_C4D0,
            8,
        ),
    ] {
        require(
            instructions,
            Some(subopcode),
            current,
            "lw v1, 0x1C(s0)",
            K::Lw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0x1C,
            },
        )?;
        require(
            instructions,
            Some(subopcode),
            flag,
            "lbu v0, 3(v1)",
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 3,
            },
        )?;
        require(
            instructions,
            Some(subopcode),
            branch,
            "beq v0, zero, yield",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 4,
            },
        )?;
        require(
            instructions,
            Some(subopcode),
            pointer,
            "lw v0, pointer_offset(v1)",
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: pointer_offset,
            },
        )?;
        require(
            instructions,
            Some(subopcode),
            store,
            "sw v0, 0x1C(s0)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: 0x1C,
            },
        )?;
        require(
            instructions,
            Some(subopcode),
            yield_lui,
            "lui at, 0x800C",
            K::Lui {
                rt: Reg::AT,
                imm: 0x800C,
            },
        )?;
        require(
            instructions,
            Some(subopcode),
            yield_store,
            "sw v0, -7008(at)",
            K::Sw {
                rt: Reg::V0,
                base: Reg::AT,
                offset: -7008,
            },
        )?;
    }
    Ok(())
}

fn require(
    instructions: &[Instruction],
    subopcode: Option<u8>,
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestSeqOp28Error> {
    let actual = instructions
        .iter()
        .find(|instruction| instruction.addr == address)
        .ok_or(QuestSeqOp28Error::MissingInstruction(address))?;
    if actual.kind != kind {
        return Err(QuestSeqOp28Error::InstructionMismatch {
            subopcode,
            address,
            expected,
            actual: actual.kind,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_seq_op28_tests.rs"]
mod tests;
