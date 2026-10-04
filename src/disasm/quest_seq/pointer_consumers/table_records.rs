//! QUEST SEQ pointer consumer의 opcode 역할군 증거.

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::super::{
    require, require_advance_8, QuestSeqPointerConsumerError, QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
};

pub(super) fn verify(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<(), QuestSeqPointerConsumerError> {
    // OP88: record +1을 entry 수로 읽어 32로 clamp하고, +4를 table
    // base로 읽어 index×4를 더한 주소를 즉시 역참조한다.
    require(
        instructions,
        map,
        0x8007_83C0,
        "lw a2, 0x1C(a0)",
        K::Lw {
            rt: Reg::A2,
            base: Reg::A0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_83C8,
        "lbu v1, 1(a2)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A2,
            offset: 1,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_83D0,
        "sltiu v0, v1, 33",
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: 33,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_83DC,
        "addiu v1, zero, 32",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 32,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_83E8,
        "lw t0, 4(a2)",
        K::Lw {
            rt: Reg::T0,
            base: Reg::A2,
            offset: 4,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_83EC,
        "andi a2, v1, 0xFF",
        K::Andi {
            rt: Reg::A2,
            rs: Reg::V1,
            imm: 0xFF,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8400,
        "sll v0, v0, 2",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8408,
        "lw v1, 0(v1)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::V1,
            offset: 0,
        },
    )?;
    require_advance_8(instructions, map, 0x8007_843C, 0x8007_8444, Reg::A0)?;

    // OP8F: record +1을 entry 수로 읽어 16으로 clamp하고, +4 table의
    // 각 4바이트 entry를 두 halfword로 역참조한다.
    require(
        instructions,
        map,
        0x8007_8544,
        "lw a0, 0x1C(t0)",
        K::Lw {
            rt: Reg::A0,
            base: Reg::T0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_854C,
        "lbu v1, 1(a0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 1,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8554,
        "sltiu v0, v1, 17",
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: 17,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8560,
        "addiu v1, zero, 16",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 16,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8564,
        "lw a3, 4(a0)",
        K::Lw {
            rt: Reg::A3,
            base: Reg::A0,
            offset: 4,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8568,
        "andi a2, v1, 0xFF",
        K::Andi {
            rt: Reg::A2,
            rs: Reg::V1,
            imm: 0xFF,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8580,
        "sll v0, v0, 2",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8588,
        "lhu v1, 0(a0)",
        K::Lhu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8594,
        "lhu v1, 2(a0)",
        K::Lhu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 2,
        },
    )?;

    Ok(())
}
