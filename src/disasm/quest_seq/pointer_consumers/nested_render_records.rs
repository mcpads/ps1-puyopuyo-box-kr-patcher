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
    let s1 = Reg(17);

    // OP89: runtime 전역 상태가 켜진 경로에서 outer record +4를 현재
    // stream pointer로 바꾼다. inner 20바이트 record의 +12/+16은
    // OP85와 같은 전역 슬롯을 거쳐 같은 renderer에서 역참조된다.
    // 꺼진 경로는 outer record 8바이트를 전진한다.
    require(
        instructions,
        map,
        0x8007_3974,
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
        0x8007_397C,
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
        0x8007_3984,
        "sw v0, 0x1C(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_3A2C,
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
        0x8007_3A34,
        "lw v0, 12(v0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 12,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_3A3C,
        "sw v0, 0x1C(v1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x1C,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_3A40,
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
        0x8007_3A48,
        "lw v0, 16(v0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 16,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_3A50,
        "sw v0, 0x20(v1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x20,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_3A6C,
        "addiu v0, v0, 20",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 20,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_3A78,
        "sw v0, 0x1C(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require_advance_8(instructions, map, 0x8007_3A90, 0x8007_3A98, Reg::A0)?;

    // OP93: +4는 callback child의 parent link를 거쳐 객체 데이터로
    // 역참조되고, +12는 renderer의 여섯 번째 인자로 전달된다.
    require(
        instructions,
        map,
        0x8007_8680,
        "lui a0, 0x8007",
        K::Lui {
            rt: Reg::A0,
            imm: 0x8007,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8684,
        "addiu a0, a0, 0x778C",
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: 0x778C,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_86A0,
        "jal 0x80014B14",
        K::Jal {
            target: 0x8001_4B14,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_86A4,
        "addiu a1, zero, 2",
        K::Addiu {
            rt: Reg::A1,
            rs: Reg::ZERO,
            imm: 2,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8704,
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
        0x8007_870C,
        "sw v1, 0xD4(v0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0xD4,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8718,
        "lw v1, 12(v1)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::V1,
            offset: 12,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8720,
        "sw v1, 0xD8(v0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0xD8,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_872C,
        "addiu v0, v0, 16",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 16,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_8730,
        "sw v0, 0x1C(s0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_7794,
        "move s1, a0",
        K::Addu {
            rd: s1,
            rs: Reg::A0,
            rt: Reg::ZERO,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_77DC,
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
        0x8007_77E4,
        "jal 0x8001B1D4",
        K::Jal {
            target: 0x8001_B1D4,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_77EC,
        "lui v1, 0x8008",
        K::Lui {
            rt: Reg::V1,
            imm: 0x8008,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_77F0,
        "addiu v1, v1, -0x6670",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::V1,
            imm: -0x6670,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_77F4,
        "sw v1, 12(v0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 12,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_B1F4,
        "jal 0x8001C120",
        K::Jal {
            target: 0x8001_C120,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_C140,
        "sw a0, 4(v1)",
        K::Sw {
            rt: Reg::A0,
            base: Reg::V1,
            offset: 4,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_7804,
        "lw v0, 0xD4(s1)",
        K::Lw {
            rt: Reg::V0,
            base: s1,
            offset: 0xD4,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_780C,
        "sw v0, 0x1C(s0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_99A0,
        "lw a1, 4(s0)",
        K::Lw {
            rt: Reg::A1,
            base: Reg::S0,
            offset: 4,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_99C0,
        "lw v0, 0x1C(a1)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::A1,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_99C8,
        "lhu a0, 4(v0)",
        K::Lhu {
            rt: Reg::A0,
            base: Reg::V0,
            offset: 4,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_99D4,
        "lw v0, 0x1C(a1)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::A1,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_99DC,
        "lhu v0, 6(v0)",
        K::Lhu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 6,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_7884,
        "lw v0, 0xD8(s1)",
        K::Lw {
            rt: Reg::V0,
            base: s1,
            offset: 0xD8,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_788C,
        "sw v0, 0x18(sp)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::SP,
            offset: 0x18,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_7894,
        "jal 0x800252D8",
        K::Jal {
            target: 0x8002_52D8,
        },
    )?;

    Ok(())
}
