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
    // OP9F: record +4를 callback 객체 둘을 거쳐 child의 +0x20에
    // 보존하고, child callback이 index를 더한 뒤 byte로 역참조한다.
    let s2 = Reg(18);
    let s4 = Reg(20);
    require(
        instructions,
        map,
        0x8007_89F8,
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
        0x8007_8A00,
        "lw a1, 4(v0)",
        K::Lw {
            rt: Reg::A1,
            base: Reg::V0,
            offset: 4,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8A08,
        "jal 0x80066C9C",
        K::Jal {
            target: 0x8006_6C9C,
        },
    )?;
    require_advance_8(instructions, map, 0x8007_8A20, 0x8007_8A24, Reg::A0)?;
    require(
        instructions,
        map,
        0x8006_6CAC,
        "move s2, a1",
        K::Addu {
            rd: s2,
            rs: Reg::A1,
            rt: Reg::ZERO,
        },
    )?;
    require(
        instructions,
        map,
        0x8006_6CC0,
        "lui a0, 0x8005",
        K::Lui {
            rt: Reg::A0,
            imm: 0x8005,
        },
    )?;
    require(
        instructions,
        map,
        0x8006_6CC4,
        "addiu a0, a0, 0x42B4",
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: 0x42B4,
        },
    )?;
    require(
        instructions,
        map,
        0x8006_6CC8,
        "jal 0x80014B14",
        K::Jal {
            target: 0x8001_4B14,
        },
    )?;
    require(
        instructions,
        map,
        0x8006_6CE4,
        "sw s2, 0x20(v0)",
        K::Sw {
            rt: s2,
            base: Reg::V0,
            offset: 0x20,
        },
    )?;
    require(
        instructions,
        map,
        0x8005_43BC,
        "move a0, s0",
        K::Addu {
            rd: Reg::A0,
            rs: Reg::S0,
            rt: Reg::ZERO,
        },
    )?;
    require(
        instructions,
        map,
        0x8005_43C8,
        "jal 0x8003C8FC",
        K::Jal {
            target: 0x8003_C8FC,
        },
    )?;
    require(
        instructions,
        map,
        0x8003_C920,
        "lui a0, 0x8004",
        K::Lui {
            rt: Reg::A0,
            imm: 0x8004,
        },
    )?;
    require(
        instructions,
        map,
        0x8003_C924,
        "addiu a0, a0, -0x5070",
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: -0x5070,
        },
    )?;
    require(
        instructions,
        map,
        0x8003_C92C,
        "jal 0x80014B14",
        K::Jal {
            target: 0x8001_4B14,
        },
    )?;
    require(
        instructions,
        map,
        0x8003_C950,
        "sw s2, 0x10(v0)",
        K::Sw {
            rt: s2,
            base: Reg::V0,
            offset: 0x10,
        },
    )?;
    require(
        instructions,
        map,
        0x8003_C954,
        "lw v1, 0x20(s2)",
        K::Lw {
            rt: Reg::V1,
            base: s2,
            offset: 0x20,
        },
    )?;
    require(
        instructions,
        map,
        0x8003_C95C,
        "sw v1, 0x20(v0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x20,
        },
    )?;
    require(
        instructions,
        map,
        0x8003_B348,
        "lw v0, 0x20(s4)",
        K::Lw {
            rt: Reg::V0,
            base: s4,
            offset: 0x20,
        },
    )?;
    require(
        instructions,
        map,
        0x8003_B350,
        "addu v0, v0, a0",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::A0,
        },
    )?;
    require(
        instructions,
        map,
        0x8003_B354,
        "lbu v0, 0(v0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 0,
        },
    )?;

    Ok(())
}
