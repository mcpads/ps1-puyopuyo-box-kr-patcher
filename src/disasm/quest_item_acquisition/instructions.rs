//! OPAA handler와 아이템 획득 state machine의 typed MIPS 원천 계약.

use crate::isa::{InstructionKind as K, Reg};

use super::{
    QUEST_ITEM_ACQUISITION_OBJECT_STATE_OFFSET, QUEST_ITEM_ACQUISITION_RECORD_BYTES,
    QUEST_ITEM_ACQUISITION_RECORD_ITEM_ID_OFFSET, QUEST_ITEM_ACQUISITION_RECORD_PARAMETER_OFFSET,
    QUEST_ITEM_ACQUISITION_RECORD_SUBMODE_OFFSET, QUEST_ITEM_ACQUISITION_STATE_MACHINE_RUNTIME,
};

pub(super) fn acquisition_instructions(draw_runtime: u32) -> Vec<(u32, K)> {
    vec![
        (
            0x8007_4D00,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            0x8007_4D08,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: QUEST_ITEM_ACQUISITION_RECORD_SUBMODE_OFFSET as i16,
            },
        ),
        (
            0x8007_4D10,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 6,
            },
        ),
        (
            0x8007_4D18,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8001,
            },
        ),
        (
            0x8007_4D1C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x3088,
            },
        ),
        (
            0x8007_4D20,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
        ),
        (
            0x8007_4D24,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8007_4D28,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (0x8007_4D30, K::Jr { rs: Reg::V0 }),
        (
            0x8007_4D38,
            K::Lbu {
                rt: Reg::S0,
                base: Reg::S1,
                offset: QUEST_ITEM_ACQUISITION_OBJECT_STATE_OFFSET,
            },
        ),
        (
            0x8007_4D58,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            0x8007_4D5C,
            K::Lui {
                rt: Reg::V1,
                imm: 0x800E,
            },
        ),
        (
            0x8007_4D60,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: QUEST_ITEM_ACQUISITION_RECORD_ITEM_ID_OFFSET as i16,
            },
        ),
        (
            0x8007_4D64,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: -0x4940,
            },
        ),
        (
            0x8007_4D68,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x120,
            },
        ),
        (
            0x8007_4D6C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            0x8007_4D74,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: QUEST_ITEM_ACQUISITION_RECORD_PARAMETER_OFFSET as i16,
            },
        ),
        (
            0x8007_4D7C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x124,
            },
        ),
        (
            0x8007_4D80,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            0x8007_4D88,
            K::Lbu {
                rt: Reg::A1,
                base: Reg::V0,
                offset: QUEST_ITEM_ACQUISITION_RECORD_SUBMODE_OFFSET as i16,
            },
        ),
        (
            0x8007_4D90,
            K::Xori {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 5,
            },
        ),
        (
            0x8007_4D94,
            K::Jal {
                target: QUEST_ITEM_ACQUISITION_STATE_MACHINE_RUNTIME,
            },
        ),
        (
            0x8007_4D98,
            K::Sltiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 1,
            },
        ),
        (
            0x8006_1278,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::S1,
                offset: 0xA2,
            },
        ),
        (
            0x8006_13BC,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8006,
            },
        ),
        (
            0x8006_13C0,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x0CE8,
            },
        ),
        (
            0x8006_13C4,
            K::Jal {
                target: 0x8001_4B14,
            },
        ),
        (
            0x8006_13CC,
            K::Lui {
                rt: Reg::A0,
                imm: 0x801A,
            },
        ),
        (
            0x8006_13D0,
            K::Lui {
                rt: Reg::V1,
                imm: 0x800E,
            },
        ),
        (
            0x8006_13D4,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V1,
                offset: -0x4820,
            },
        ),
        (
            0x8006_13D8,
            K::Ori {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x1A70,
            },
        ),
        (
            0x8006_13DC,
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
        ),
        (
            0x8006_13E0,
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::A0,
            },
        ),
        (
            0x8006_13E4,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            0x8006_13F0,
            K::Sw {
                rt: Reg::V1,
                base: Reg::A1,
                offset: 0x20,
            },
        ),
        (
            0x8006_143C,
            K::Lui {
                rt: Reg::V1,
                imm: 0x801A,
            },
        ),
        (
            0x8006_1440,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800E,
            },
        ),
        (
            0x8006_1444,
            K::Addiu {
                rt: Reg::S2,
                rs: Reg::V0,
                imm: -0x4940,
            },
        ),
        (
            0x8006_1448,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S2,
                offset: 0x120,
            },
        ),
        (
            0x8006_144C,
            K::Ori {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x1A70,
            },
        ),
        (
            0x8006_1450,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8006_1454,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8006_1458,
            K::Lw {
                rt: Reg::S0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8006_14CC,
            K::Lw {
                rt: Reg::A1,
                base: Reg::S0,
                offset: 0x0C,
            },
        ),
        (
            0x8006_14D0,
            K::Jal {
                target: draw_runtime,
            },
        ),
        (
            0x8006_18C0,
            K::Lui {
                rt: Reg::V1,
                imm: 0x801A,
            },
        ),
        (
            0x8006_18C4,
            K::Ori {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: 0x1A70,
            },
        ),
        (
            0x8006_18CC,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800E,
            },
        ),
        (
            0x8006_18D4,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: -0x4820,
            },
        ),
        (
            0x8006_18DC,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8006_18E0,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8006_18E4,
            K::Lw {
                rt: Reg::S0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8006_18F0,
            K::Lw {
                rt: Reg::A1,
                base: Reg::S0,
                offset: 0x0C,
            },
        ),
        (
            0x8006_18F4,
            K::Jal {
                target: draw_runtime,
            },
        ),
        (
            0x8006_14B4,
            K::Lui {
                rt: Reg::A1,
                imm: 0x8001,
            },
        ),
        (
            0x8006_14BC,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x2210,
            },
        ),
        (
            0x8006_14DC,
            K::Lui {
                rt: Reg::A1,
                imm: 0x8001,
            },
        ),
        (
            0x8006_14E0,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x2224,
            },
        ),
        (
            0x8006_14E4,
            K::Jal {
                target: draw_runtime,
            },
        ),
        (
            0x8006_17B4,
            K::Lui {
                rt: Reg::A1,
                imm: 0x8001,
            },
        ),
        (
            0x8006_17B8,
            K::Jal {
                target: draw_runtime,
            },
        ),
        (
            0x8006_17BC,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x2234,
            },
        ),
        (
            0x8006_17F8,
            K::Lui {
                rt: Reg::A1,
                imm: 0x8001,
            },
        ),
        (
            0x8006_17FC,
            K::Jal {
                target: draw_runtime,
            },
        ),
        (
            0x8006_1800,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x225C,
            },
        ),
        (
            0x8006_18D0,
            K::Lui {
                rt: Reg::A1,
                imm: 0x8001,
            },
        ),
        (
            0x8006_18E8,
            K::Jal {
                target: draw_runtime,
            },
        ),
        (
            0x8006_18EC,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x2270,
            },
        ),
        (
            0x8006_1900,
            K::Lui {
                rt: Reg::A1,
                imm: 0x800C,
            },
        ),
        (
            0x8006_1904,
            K::Jal {
                target: draw_runtime,
            },
        ),
        (
            0x8006_1908,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: -0x1E10,
            },
        ),
        (
            0x8006_1918,
            K::Lui {
                rt: Reg::A1,
                imm: 0x8001,
            },
        ),
        (
            0x8006_191C,
            K::Jal {
                target: draw_runtime,
            },
        ),
        (
            0x8006_1920,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x227C,
            },
        ),
        (
            0x8007_4EA8,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            0x8007_4EB0,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_ITEM_ACQUISITION_RECORD_BYTES as i16,
            },
        ),
        (
            0x8007_4EB8,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            0x8007_5090,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
        (
            0x8007_5098,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_ITEM_ACQUISITION_RECORD_BYTES as i16,
            },
        ),
        (
            0x8007_509C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S1,
                offset: 0x1C,
            },
        ),
    ]
}
