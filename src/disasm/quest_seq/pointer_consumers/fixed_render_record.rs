//! QUEST SEQ pointer consumer의 opcode 역할군 증거.

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};

use super::super::{require, QuestSeqPointerConsumerError, QUEST_SEQ_STREAM_POINTER_STATE_OFFSET};

pub(super) fn verify(
    instructions: &[Instruction],
    map: &ExeMap,
) -> Result<(), QuestSeqPointerConsumerError> {
    let s1 = Reg(17);

    // OP85: 20바이트 record의 +12/+16을 두 전역 슬롯에 저장한다.
    // 후속 draw 경로는 각 값을 renderer의 여섯 번째 인자로 넘기며,
    // renderer는 두 색 데이터 주소를 모두 lhu로 역참조한다.
    let s3 = Reg(19);
    let s7 = Reg(23);
    let t5 = Reg(13);
    let t6 = Reg(14);
    require(
        instructions,
        map,
        0x8007_832C,
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
        0x8007_8334,
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
        0x8007_8340,
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
        0x8007_8348,
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
        0x8007_8354,
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
        0x8007_835C,
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
        0x8007_3698,
        "lui v0, 0x8010",
        K::Lui {
            rt: Reg::V0,
            imm: 0x8010,
        },
    )?;
    require(
        instructions,
        map,
        0x8007_369C,
        "addiu s1, v0, -0x4260",
        K::Addiu {
            rt: s1,
            rs: Reg::V0,
            imm: -0x4260,
        },
    )?;
    for (load_address, offset, call_address, pass_address) in [
        (0x8007_3734, 0x1C, 0x8007_3744, 0x8007_3748),
        (0x8007_37F0, 0x20, 0x8007_3800, 0x8007_3804),
    ] {
        require(
            instructions,
            map,
            load_address,
            "lw v1, render-data(s1)",
            K::Lw {
                rt: Reg::V1,
                base: s1,
                offset,
            },
        )?;
        require(
            instructions,
            map,
            call_address,
            "jal 0x800252D8",
            K::Jal {
                target: 0x8002_52D8,
            },
        )?;
        require(
            instructions,
            map,
            pass_address,
            "sw v1, 0x18(sp)",
            K::Sw {
                rt: Reg::V1,
                base: Reg::SP,
                offset: 0x18,
            },
        )?;
    }
    require(
        instructions,
        map,
        0x8002_536C,
        "lw s7, 0xB0(sp)",
        K::Lw {
            rt: s7,
            base: Reg::SP,
            offset: 0xB0,
        },
    )?;
    require(
        instructions,
        map,
        0x8002_5490,
        "sw s7, 0(s3)",
        K::Sw {
            rt: s7,
            base: s3,
            offset: 0,
        },
    )?;
    require(
        instructions,
        map,
        0x8002_5494,
        "sw s7, -5(t0)",
        K::Sw {
            rt: s7,
            base: Reg::T0,
            offset: -5,
        },
    )?;
    require(
        instructions,
        map,
        0x8002_5658,
        "lw t6, 0(s3)",
        K::Lw {
            rt: t6,
            base: s3,
            offset: 0,
        },
    )?;
    require(
        instructions,
        map,
        0x8002_5664,
        "lw a3, -5(t0)",
        K::Lw {
            rt: Reg::A3,
            base: Reg::T0,
            offset: -5,
        },
    )?;
    require(
        instructions,
        map,
        0x8002_56B4,
        "lhu t5, 0(t6)",
        K::Lhu {
            rt: t5,
            base: t6,
            offset: 0,
        },
    )?;
    require(
        instructions,
        map,
        0x8002_56C8,
        "lhu v1, 0(a3)",
        K::Lhu {
            rt: Reg::V1,
            base: Reg::A3,
            offset: 0,
        },
    )?;

    Ok(())
}
