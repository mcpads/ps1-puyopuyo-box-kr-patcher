//! QUEST SEQ pointer consumer의 opcode 역할군 증거.

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::super::{
    require, require_advance_8, verify_mask_branch, QuestSeqPointerConsumerError,
    QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
};

pub(super) fn verify(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<(), QuestSeqPointerConsumerError> {
    let s3 = Reg(19);

    // OPC1: byte +1이 0이 아닌 경우에만 record +4를 전역에 저장한다.
    // 후속 경로는 같은 전역에서 값을 읽어 새 interpreter의 stream으로
    // 설정하고 소비 flag를 지운다.
    require(
        instructions,
        map,
        0x8007_9660,
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
        0x8007_9668,
        "beq v0, zero, 0x80079690",
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 9,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_967C,
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
        0x8007_9684,
        "sw v0, -0x1974(at)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::AT,
            offset: -0x1974,
        },
    )?;
    require_advance_8(instructions, map, 0x8007_96A0, 0x8007_96A8, Reg::A0)?;
    require(
        instructions,
        map,
        0x800A_A764,
        "lui gp, 0x800C",
        K::Lui {
            rt: Reg::GP,
            imm: 0x800C,
        },
    )?;
    require(
        instructions,
        map,
        0x800A_A768,
        "addiu gp, gp, -0x2078",
        K::Addiu {
            rt: Reg::GP,
            rs: Reg::GP,
            imm: -0x2078,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_F130,
        "lw v0, 0x708(gp)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 0x708,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_F184,
        "lui a0, 0x8002",
        K::Lui {
            rt: Reg::A0,
            imm: 0x8002,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_F188,
        "addiu a0, a0, 0x2570",
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: 0x2570,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_F18C,
        "jal 0x80014A84",
        K::Jal {
            target: 0x8001_4A84,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_F194,
        "lw a0, 0x708(gp)",
        K::Lw {
            rt: Reg::A0,
            base: Reg::GP,
            offset: 0x708,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_F198,
        "lw v1, 0x704(gp)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::GP,
            offset: 0x704,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_F19C,
        "sw zero, 0x708(gp)",
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::GP,
            offset: 0x708,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_F1A0,
        "sw v1, 0x1C(v0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;

    // OPE3: record +8을 비동기 loader 요청에 보존하고 callback이 이를
    // 복사 함수의 source로 넘긴다. copy loop는 source를 u32로 역참조한다.
    require(
        instructions,
        map,
        0x8006_FF48,
        "lw a2, 0x1C(s0)",
        K::Lw {
            rt: Reg::A2,
            base: Reg::S0,
            offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
        },
    )?;
    require(
        instructions,
        map,
        0x8006_FF70,
        "lw a1, 8(a2)",
        K::Lw {
            rt: Reg::A1,
            base: Reg::A2,
            offset: 8,
        },
    )?;
    require(
        instructions,
        map,
        0x8006_FF78,
        "jal 0x8001B12C",
        K::Jal {
            target: 0x8001_B12C,
        },
    )?;
    require(
        instructions,
        map,
        0x8006_FF88,
        "addiu v0, v0, 12",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 12,
        },
    )?;
    require(
        instructions,
        map,
        0x8006_FF8C,
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
        0x8001_B13C,
        "move s3, a1",
        K::Addu {
            rd: s3,
            rs: Reg::A1,
            rt: Reg::ZERO,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_B150,
        "jal 0x8001B2F0",
        K::Jal {
            target: 0x8001_B2F0,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_B17C,
        "sw s3, 0x14(s0)",
        K::Sw {
            rt: s3,
            base: Reg::S0,
            offset: 0x14,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_C0C8,
        "lui v0, 0x8002",
        K::Lui {
            rt: Reg::V0,
            imm: 0x8002,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_C0CC,
        "addiu v0, v0, -0x4280",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: -0x4280,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_C0D8,
        "sw v0, 8(v1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 8,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_BE20,
        "lw a1, 0x14(s0)",
        K::Lw {
            rt: Reg::A1,
            base: Reg::S0,
            offset: 0x14,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_BE24,
        "lw a0, 0x18(s0)",
        K::Lw {
            rt: Reg::A0,
            base: Reg::S0,
            offset: 0x18,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_BE54,
        "jal 0x8007E618",
        K::Jal {
            target: 0x8007_E618,
        },
    )?;
    require(
        instructions,
        map,
        0x8001_BE58,
        "sra a2, a2, 2",
        K::Sra {
            rd: Reg::A2,
            rt: Reg::A2,
            sa: 2,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_E620,
        "lw v0, 0(a1)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::A1,
            offset: 0,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_E62C,
        "sw v0, 0(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0,
        },
    )?;

    verify_mask_branch(
        instructions,
        map,
        0x8007_8CDC,
        0x8007_8CF8,
        0x8007_8D00,
        Reg::A1,
    )?;
    verify_mask_branch(
        instructions,
        map,
        0x8007_8D40,
        0x8007_8D4C,
        0x8007_8D54,
        Reg::V1,
    )?;
    Ok(())
}
