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
    // OP95: record +4를 0x800FBF44에 보관한다. OP8B의 한 경로가 같은
    // 주소를 읽어 현재 stream pointer로 복원한다.
    require(
        instructions,
        map,
        0x8007_87CC,
        "lw v0, 0x1C(a0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_87D4,
        "lw v1, 4(v0)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 4,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_87D8,
        "lui v0, 0x8010",
        K::Lui {
            rt: Reg::V0,
            imm: 0x8010,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_87DC,
        "sw v1, -0x40BC(v0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: -0x40BC,
        },
    )?;
    require_advance_8(instructions, map, 0x8007_87E8, 0x8007_87F0, Reg::A0)?;
    require(
        instructions,
        map,
        0x8007_3EF0,
        "lui v0, 0x8010",
        K::Lui {
            rt: Reg::V0,
            imm: 0x8010,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_3EF4,
        "addiu v0, v0, -0x4230",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: -0x4230,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_3EF8,
        "lw v1, 0x174(v0)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x174,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_3F04,
        "sw v1, 0x1C(s0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::S0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;

    // OP9D: record +4를 선택한 객체의 stream pointer로 저장한다.
    require(
        instructions,
        map,
        0x8007_88E0,
        "lw v0, 0x1C(a0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_88E8,
        "lw v0, 4(v0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 4,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_88F0,
        "sw v0, 0x1C(v1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require_advance_8(instructions, map, 0x8007_8910, 0x8007_8918, Reg::A0)?;

    Ok(())
}
