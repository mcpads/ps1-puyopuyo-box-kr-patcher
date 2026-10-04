//! QUEST SEQ pointer consumer의 opcode 역할군 증거.

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::super::{
    require, require_advance_8, QuestSeqPointerConsumerError,
    QUEST_SEQ_BRANCH_RECORD_POINTER_OFFSET, QUEST_SEQ_BRANCH_RECORD_STRIDE,
    QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
};

pub(super) fn verify(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<(), QuestSeqPointerConsumerError> {
    // OP80: byte +1이 0일 때 record +4를 새 객체의 stream으로 넘기고
    // 현재 record는 8바이트 전진한다.
    require(
        instructions,
        map,
        0x8007_811C,
        "lbu v0, 1(v0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 1,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8124,
        "beq v0, zero, 0x80078160",
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 14,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_816C,
        "lw v1, 0x1C(s0)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::S0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8174,
        "lw v1, 4(v1)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::V1,
            offset: 4,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8184,
        "sw v1, 0x1C(a0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::A0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require_advance_8(instructions, map, 0x8007_8190, 0x8007_8194, Reg::S0)?;

    // OP83 subtype 5: record +8을 객체의 scheduler stream slot +0x40에
    // 저장한다. 공용 scheduler는 이를 임시 +0x1C로 옮겨 interpreter를
    // 호출한 뒤 갱신된 stream을 +0x40으로 되돌린다.
    let s1 = Reg(17);
    require(
        instructions,
        map,
        0x8007_33B8,
        "lw v0, 0x1C(s1)",
        K::Lw {
            rt: Reg::V0,
            base: s1,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_33C0,
        "lbu v1, 1(v0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 1,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_359C,
        "lw v1, 8(v0)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: QUEST_SEQ_BRANCH_RECORD_POINTER_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_35A8,
        "sw v1, 0x40(s0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::S0,
            offset: 0x40,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_35B4,
        "addiu v0, v0, 12",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: QUEST_SEQ_BRANCH_RECORD_STRIDE,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_35B8,
        "sw v0, 0x1C(s1)",
        K::Sw {
            rt: Reg::V0,
            base: s1,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_4E9C,
        "lw v0, 0x40(s1)",
        K::Lw {
            rt: Reg::V0,
            base: s1,
            offset: 0x40,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_4EA0,
        "lw s0, 0x1C(s1)",
        K::Lw {
            rt: Reg::S0,
            base: s1,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_4EA4,
        "jal 0x80022570",
        K::Jal {
            target: 0x8002_2570,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_4EA8,
        "sw v0, 0x1C(s1)",
        K::Sw {
            rt: Reg::V0,
            base: s1,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_4EAC,
        "lw v0, 0x1C(s1)",
        K::Lw {
            rt: Reg::V0,
            base: s1,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_4EB0,
        "sw s0, 0x1C(s1)",
        K::Sw {
            rt: Reg::S0,
            base: s1,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_4EB4,
        "sw v0, 0x40(s1)",
        K::Sw {
            rt: Reg::V0,
            base: s1,
            offset: 0x40,
        },
    )?;

    Ok(())
}
