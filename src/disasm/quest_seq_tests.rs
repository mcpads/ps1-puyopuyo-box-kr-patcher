use super::*;
use crate::coords::exe_map;
use crate::isa::encode;

fn fixture() -> (&'static ExeMap, Vec<u8>) {
    let map = exe_map("QUEST.EXE").expect("QUEST.EXE 사상");
    let mut text = vec![0u8; map.text_len as usize];
    let instructions = [
        (
            0x8002_2584,
            K::Lui {
                rt: Reg::V0,
                imm: 0x800B,
            },
        ),
        (
            0x8002_258C,
            K::Addiu {
                rt: Reg::S1,
                rs: Reg::V0,
                imm: 0x1C18,
            },
        ),
        (
            0x8002_25B8,
            K::Lw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8002_25C0,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8002_25C8,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8002_25CC,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::S1,
            },
        ),
        (
            0x8002_25D0,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8002_25D8,
            K::Jalr {
                rd: Reg::RA,
                rs: Reg::V0,
            },
        ),
        (
            0x8007_80AC,
            K::Lui {
                rt: Reg::A0,
                imm: 0x800B,
            },
        ),
        (
            0x8007_80B0,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x36F0,
            },
        ),
        (
            0x8007_80B4,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: 0x80,
            },
        ),
        (
            0x8007_80BC,
            K::Jal {
                target: QUEST_SEQ_BRANCH_DISPATCH_REGISTER_RUNTIME,
            },
        ),
        (
            0x8007_80C0,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: 0x60,
            },
        ),
        (
            0x8007_002C,
            K::Lui {
                rt: Reg::A0,
                imm: 0x800B,
            },
        ),
        (
            0x8007_0030,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x3670,
            },
        ),
        (
            0x8007_0034,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: 0xE0,
            },
        ),
        (
            0x8007_003C,
            K::Jal {
                target: QUEST_SEQ_BRANCH_DISPATCH_REGISTER_RUNTIME,
            },
        ),
        (
            0x8007_0040,
            K::Addiu {
                rt: Reg::A2,
                rs: Reg::ZERO,
                imm: 0x20,
            },
        ),
        (
            0x8007_811C,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            0x8007_8124,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 14,
            },
        ),
        (
            0x8007_816C,
            K::Lw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8174,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V1,
                offset: 4,
            },
        ),
        (
            0x8007_8184,
            K::Sw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8190,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 8,
            },
        ),
        (
            0x8007_8194,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_33B8,
            K::Lw {
                rt: Reg::V0,
                base: Reg(17),
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_33C0,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            0x8007_359C,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: QUEST_SEQ_BRANCH_RECORD_POINTER_OFFSET,
            },
        ),
        (
            0x8007_35A8,
            K::Sw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: 0x40,
            },
        ),
        (
            0x8007_35B4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_SEQ_BRANCH_RECORD_STRIDE,
            },
        ),
        (
            0x8007_35B8,
            K::Sw {
                rt: Reg::V0,
                base: Reg(17),
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8001_4E9C,
            K::Lw {
                rt: Reg::V0,
                base: Reg(17),
                offset: 0x40,
            },
        ),
        (
            0x8001_4EA0,
            K::Lw {
                rt: Reg::S0,
                base: Reg(17),
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8001_4EA4,
            K::Jal {
                target: 0x8002_2570,
            },
        ),
        (
            0x8001_4EA8,
            K::Sw {
                rt: Reg::V0,
                base: Reg(17),
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8001_4EAC,
            K::Lw {
                rt: Reg::V0,
                base: Reg(17),
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8001_4EB0,
            K::Sw {
                rt: Reg::S0,
                base: Reg(17),
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8001_4EB4,
            K::Sw {
                rt: Reg::V0,
                base: Reg(17),
                offset: 0x40,
            },
        ),
        (
            0x8007_3C68,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8019,
            },
        ),
        (
            0x8007_3C74,
            K::Ori {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x9A70,
            },
        ),
        (
            0x8007_3C88,
            K::Sw {
                rt: Reg::A0,
                base: Reg(17),
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_3CDC,
            K::Lui {
                rt: Reg::A1,
                imm: 0x8019,
            },
        ),
        (
            0x8007_3CF4,
            K::Ori {
                rt: Reg::A1,
                rs: Reg::A1,
                imm: 0x9A70,
            },
        ),
        (
            0x8007_3D04,
            K::Sw {
                rt: Reg::A1,
                base: Reg(17),
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8280,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8010,
            },
        ),
        (
            0x8007_8284,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::V0,
                imm: -0x4260,
            },
        ),
        (
            0x8007_8290,
            K::Lw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8298,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 1,
            },
        ),
        (
            0x8007_82A0,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 4,
            },
        ),
        (
            0x8007_82B0,
            K::Sh {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_SEQ_OP85_SELECTOR_STATE_OFFSET,
            },
        ),
        (
            0x8007_82B4,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: -0x4270,
            },
        ),
        (
            0x8007_82BC,
            K::Sh {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_SEQ_OP85_SELECTOR_STATE_OFFSET,
            },
        ),
        (
            0x8007_82C0,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_82C4,
            K::Lui {
                rt: Reg::V1,
                imm: 0x8010,
            },
        ),
        (
            0x8007_82C8,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 2,
            },
        ),
        (
            0x8007_82CC,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: -0x4260,
            },
        ),
        (
            0x8007_82D0,
            K::Sh {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x10,
            },
        ),
        (
            0x8007_82DC,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8007_82E4,
            K::Sh {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x12,
            },
        ),
        (
            0x8007_82F0,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 6,
            },
        ),
        (
            0x8007_82F8,
            K::Sh {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x14,
            },
        ),
        (
            0x8007_8304,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 8,
            },
        ),
        (
            0x8007_830C,
            K::Sh {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x16,
            },
        ),
        (
            0x8007_8318,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 10,
            },
        ),
        (
            0x8007_8320,
            K::Sh {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x18,
            },
        ),
        (
            0x8007_832C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 12,
            },
        ),
        (
            0x8007_8334,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x1C,
            },
        ),
        (
            0x8007_8340,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 16,
            },
        ),
        (
            0x8007_8348,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x20,
            },
        ),
        (
            0x8007_8354,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 20,
            },
        ),
        (
            0x8007_835C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_3698,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8010,
            },
        ),
        (
            0x8007_369C,
            K::Addiu {
                rt: Reg(17),
                rs: Reg::V0,
                imm: -0x4260,
            },
        ),
        (
            0x8007_3734,
            K::Lw {
                rt: Reg::V1,
                base: Reg(17),
                offset: 0x1C,
            },
        ),
        (
            0x8007_3744,
            K::Jal {
                target: 0x8002_52D8,
            },
        ),
        (
            0x8007_3748,
            K::Sw {
                rt: Reg::V1,
                base: Reg::SP,
                offset: 0x18,
            },
        ),
        (
            0x8007_37F0,
            K::Lw {
                rt: Reg::V1,
                base: Reg(17),
                offset: 0x20,
            },
        ),
        (
            0x8007_3800,
            K::Jal {
                target: 0x8002_52D8,
            },
        ),
        (
            0x8007_3804,
            K::Sw {
                rt: Reg::V1,
                base: Reg::SP,
                offset: 0x18,
            },
        ),
        (
            0x8002_536C,
            K::Lw {
                rt: Reg(23),
                base: Reg::SP,
                offset: 0xB0,
            },
        ),
        (
            0x8002_5490,
            K::Sw {
                rt: Reg(23),
                base: Reg(19),
                offset: 0,
            },
        ),
        (
            0x8002_5494,
            K::Sw {
                rt: Reg(23),
                base: Reg::T0,
                offset: -5,
            },
        ),
        (
            0x8002_5658,
            K::Lw {
                rt: Reg(14),
                base: Reg(19),
                offset: 0,
            },
        ),
        (
            0x8002_5664,
            K::Lw {
                rt: Reg::A3,
                base: Reg::T0,
                offset: -5,
            },
        ),
        (
            0x8002_56B4,
            K::Lhu {
                rt: Reg(13),
                base: Reg(14),
                offset: 0,
            },
        ),
        (
            0x8002_56C8,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::A3,
                offset: 0,
            },
        ),
        (
            0x8007_83C0,
            K::Lw {
                rt: Reg::A2,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_83C8,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A2,
                offset: 1,
            },
        ),
        (
            0x8007_83D0,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 33,
            },
        ),
        (
            0x8007_83DC,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 32,
            },
        ),
        (
            0x8007_83E8,
            K::Lw {
                rt: Reg::T0,
                base: Reg::A2,
                offset: 4,
            },
        ),
        (
            0x8007_83EC,
            K::Andi {
                rt: Reg::A2,
                rs: Reg::V1,
                imm: 0xFF,
            },
        ),
        (
            0x8007_8400,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8007_8408,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            0x8007_843C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 8,
            },
        ),
        (
            0x8007_8444,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8544,
            K::Lw {
                rt: Reg::A0,
                base: Reg::T0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_854C,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 1,
            },
        ),
        (
            0x8007_8554,
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 17,
            },
        ),
        (
            0x8007_8560,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::ZERO,
                imm: 16,
            },
        ),
        (
            0x8007_8564,
            K::Lw {
                rt: Reg::A3,
                base: Reg::A0,
                offset: 4,
            },
        ),
        (
            0x8007_8568,
            K::Andi {
                rt: Reg::A2,
                rs: Reg::V1,
                imm: 0xFF,
            },
        ),
        (
            0x8007_8580,
            K::Sll {
                rd: Reg::V0,
                rt: Reg::V0,
                sa: 2,
            },
        ),
        (
            0x8007_8588,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 0,
            },
        ),
        (
            0x8007_8594,
            K::Lhu {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 2,
            },
        ),
        (
            0x8007_3974,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_397C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8007_3984,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_3A2C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_3A34,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 12,
            },
        ),
        (
            0x8007_3A3C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x1C,
            },
        ),
        (
            0x8007_3A40,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_3A48,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 16,
            },
        ),
        (
            0x8007_3A50,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0x20,
            },
        ),
        (
            0x8007_3A6C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 20,
            },
        ),
        (
            0x8007_3A78,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8680,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8007,
            },
        ),
        (
            0x8007_8684,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x778C,
            },
        ),
        (
            0x8007_86A0,
            K::Jal {
                target: 0x8001_4B14,
            },
        ),
        (
            0x8007_86A4,
            K::Addiu {
                rt: Reg::A1,
                rs: Reg::ZERO,
                imm: 2,
            },
        ),
        (
            0x8007_8704,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V1,
                offset: 4,
            },
        ),
        (
            0x8007_870C,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0xD4,
            },
        ),
        (
            0x8007_8718,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V1,
                offset: 12,
            },
        ),
        (
            0x8007_8720,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0xD8,
            },
        ),
        (
            0x8007_872C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 16,
            },
        ),
        (
            0x8007_8730,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_7794,
            K::Addu {
                rd: Reg(17),
                rs: Reg::A0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8007_77DC,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8007_77E4,
            K::Jal {
                target: 0x8001_B1D4,
            },
        ),
        (
            0x8007_77EC,
            K::Lui {
                rt: Reg::V1,
                imm: 0x8008,
            },
        ),
        (
            0x8007_77F0,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::V1,
                imm: -0x6670,
            },
        ),
        (
            0x8007_77F4,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 12,
            },
        ),
        (
            0x8001_B1F4,
            K::Jal {
                target: 0x8001_C120,
            },
        ),
        (
            0x8001_C140,
            K::Sw {
                rt: Reg::A0,
                base: Reg::V1,
                offset: 4,
            },
        ),
        (
            0x8007_7804,
            K::Lw {
                rt: Reg::V0,
                base: Reg(17),
                offset: 0xD4,
            },
        ),
        (
            0x8007_780C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_99A0,
            K::Lw {
                rt: Reg::A1,
                base: Reg::S0,
                offset: 4,
            },
        ),
        (
            0x8007_99C0,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_99C8,
            K::Lhu {
                rt: Reg::A0,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8007_99D4,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_99DC,
            K::Lhu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 6,
            },
        ),
        (
            0x8007_7884,
            K::Lw {
                rt: Reg::V0,
                base: Reg(17),
                offset: 0xD8,
            },
        ),
        (
            0x8007_788C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::SP,
                offset: 0x18,
            },
        ),
        (
            0x8007_7894,
            K::Jal {
                target: 0x8002_52D8,
            },
        ),
        (
            0x8007_3A90,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 8,
            },
        ),
        (
            0x8007_3A98,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_87CC,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_87D4,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8007_87D8,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8010,
            },
        ),
        (
            0x8007_87DC,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: -0x40BC,
            },
        ),
        (
            0x8007_87E8,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 8,
            },
        ),
        (
            0x8007_87F0,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_3EF0,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8010,
            },
        ),
        (
            0x8007_3EF4,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -0x4230,
            },
        ),
        (
            0x8007_3EF8,
            K::Lw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x174,
            },
        ),
        (
            0x8007_3F04,
            K::Sw {
                rt: Reg::V1,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_88E0,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_88E8,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8007_88F0,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8910,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 8,
            },
        ),
        (
            0x8007_8918,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8CDC,
            K::Lw {
                rt: Reg::A1,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8CF8,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 4,
            },
        ),
        (
            0x8007_8D00,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8D40,
            K::Lw {
                rt: Reg::V1,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8D4C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 4,
            },
        ),
        (
            0x8007_8D54,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8A68,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8A70,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 1,
            },
        ),
        (
            0x8007_8A74,
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0x00FF,
            },
        ),
        (
            0x8007_8A78,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::V1,
                offset: 4,
            },
        ),
        (
            0x8007_8A7C,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::A0,
                imm: QUEST_SEQ_BRANCH_RECORD_STRIDE,
            },
        ),
        (
            0x8007_8A80,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_BRANCH_RECORD_POINTER_OFFSET,
            },
        ),
        (
            0x8007_8A84,
            K::J {
                target: 0x8007_8A90,
            },
        ),
        (
            0x8007_8A88,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8A8C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8AC8,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8AD0,
            K::Lbu {
                rt: Reg::V1,
                base: Reg::A0,
                offset: 1,
            },
        ),
        (
            0x8007_8AD8,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::V1,
                offset: 13,
            },
        ),
        (
            0x8007_8ADC,
            K::Addiu {
                rt: Reg::V1,
                rs: Reg::A0,
                imm: QUEST_SEQ_BRANCH_RECORD_STRIDE,
            },
        ),
        (
            0x8007_8AF0,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x28,
            },
        ),
        (
            0x8007_8B04,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: QUEST_SEQ_BRANCH_RECORD_POINTER_OFFSET,
            },
        ),
        (
            0x8007_8B0C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8B10,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::A0,
                imm: QUEST_SEQ_BRANCH_RECORD_STRIDE,
            },
        ),
        (
            0x8007_8B14,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
    ];
    for (address, kind) in instructions {
        let offset = map.text_offset(address).expect("fixture 명령 범위");
        text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
    }
    let added_pointer_instructions = [
        (
            0x8007_89F8,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8007_8A00,
            K::Lw {
                rt: Reg::A1,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8007_8A08,
            K::Jal {
                target: 0x8006_6C9C,
            },
        ),
        (
            0x8007_8A20,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 8,
            },
        ),
        (
            0x8007_8A24,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8006_6CAC,
            K::Addu {
                rd: Reg(18),
                rs: Reg::A1,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8006_6CC0,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8005,
            },
        ),
        (
            0x8006_6CC4,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x42B4,
            },
        ),
        (
            0x8006_6CC8,
            K::Jal {
                target: 0x8001_4B14,
            },
        ),
        (
            0x8006_6CE4,
            K::Sw {
                rt: Reg(18),
                base: Reg::V0,
                offset: 0x20,
            },
        ),
        (
            0x8005_43BC,
            K::Addu {
                rd: Reg::A0,
                rs: Reg::S0,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8005_43C8,
            K::Jal {
                target: 0x8003_C8FC,
            },
        ),
        (
            0x8003_C920,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8004,
            },
        ),
        (
            0x8003_C924,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: -0x5070,
            },
        ),
        (
            0x8003_C92C,
            K::Jal {
                target: 0x8001_4B14,
            },
        ),
        (
            0x8003_C950,
            K::Sw {
                rt: Reg(18),
                base: Reg::V0,
                offset: 0x10,
            },
        ),
        (
            0x8003_C954,
            K::Lw {
                rt: Reg::V1,
                base: Reg(18),
                offset: 0x20,
            },
        ),
        (
            0x8003_C95C,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: 0x20,
            },
        ),
        (
            0x8003_B348,
            K::Lw {
                rt: Reg::V0,
                base: Reg(20),
                offset: 0x20,
            },
        ),
        (
            0x8003_B350,
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::A0,
            },
        ),
        (
            0x8003_B354,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 0,
            },
        ),
        (
            0x8007_9660,
            K::Lbu {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 1,
            },
        ),
        (
            0x8007_9668,
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 9,
            },
        ),
        (
            0x8007_967C,
            K::Lw {
                rt: Reg::V0,
                base: Reg::V0,
                offset: 4,
            },
        ),
        (
            0x8007_9684,
            K::Sw {
                rt: Reg::V0,
                base: Reg::AT,
                offset: -0x1974,
            },
        ),
        (
            0x8007_96A0,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 8,
            },
        ),
        (
            0x8007_96A8,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x800A_A764,
            K::Lui {
                rt: Reg::GP,
                imm: 0x800C,
            },
        ),
        (
            0x800A_A768,
            K::Addiu {
                rt: Reg::GP,
                rs: Reg::GP,
                imm: -0x2078,
            },
        ),
        (
            0x8007_F130,
            K::Lw {
                rt: Reg::V0,
                base: Reg::GP,
                offset: 0x708,
            },
        ),
        (
            0x8007_F184,
            K::Lui {
                rt: Reg::A0,
                imm: 0x8002,
            },
        ),
        (
            0x8007_F188,
            K::Addiu {
                rt: Reg::A0,
                rs: Reg::A0,
                imm: 0x2570,
            },
        ),
        (
            0x8007_F18C,
            K::Jal {
                target: 0x8001_4A84,
            },
        ),
        (
            0x8007_F194,
            K::Lw {
                rt: Reg::A0,
                base: Reg::GP,
                offset: 0x708,
            },
        ),
        (
            0x8007_F198,
            K::Lw {
                rt: Reg::V1,
                base: Reg::GP,
                offset: 0x704,
            },
        ),
        (
            0x8007_F19C,
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::GP,
                offset: 0x708,
            },
        ),
        (
            0x8007_F1A0,
            K::Sw {
                rt: Reg::V1,
                base: Reg::V0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8006_FF48,
            K::Lw {
                rt: Reg::A2,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8006_FF70,
            K::Lw {
                rt: Reg::A1,
                base: Reg::A2,
                offset: 8,
            },
        ),
        (
            0x8006_FF78,
            K::Jal {
                target: 0x8001_B12C,
            },
        ),
        (
            0x8006_FF88,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 12,
            },
        ),
        (
            0x8006_FF8C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::S0,
                offset: QUEST_SEQ_STREAM_POINTER_STATE_OFFSET,
            },
        ),
        (
            0x8001_B13C,
            K::Addu {
                rd: Reg(19),
                rs: Reg::A1,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8001_B150,
            K::Jal {
                target: 0x8001_B2F0,
            },
        ),
        (
            0x8001_B17C,
            K::Sw {
                rt: Reg(19),
                base: Reg::S0,
                offset: 0x14,
            },
        ),
        (
            0x8001_C0C8,
            K::Lui {
                rt: Reg::V0,
                imm: 0x8002,
            },
        ),
        (
            0x8001_C0CC,
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: -0x4280,
            },
        ),
        (
            0x8001_C0D8,
            K::Sw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 8,
            },
        ),
        (
            0x8001_BE20,
            K::Lw {
                rt: Reg::A1,
                base: Reg::S0,
                offset: 0x14,
            },
        ),
        (
            0x8001_BE24,
            K::Lw {
                rt: Reg::A0,
                base: Reg::S0,
                offset: 0x18,
            },
        ),
        (
            0x8001_BE54,
            K::Jal {
                target: 0x8007_E618,
            },
        ),
        (
            0x8001_BE58,
            K::Sra {
                rd: Reg::A2,
                rt: Reg::A2,
                sa: 2,
            },
        ),
        (
            0x8007_E620,
            K::Lw {
                rt: Reg::V0,
                base: Reg::A1,
                offset: 0,
            },
        ),
        (
            0x8007_E62C,
            K::Sw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: 0,
            },
        ),
    ];
    for (address, kind) in added_pointer_instructions {
        let offset = map
            .text_offset(address)
            .expect("추가 pointer fixture 명령 범위");
        text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
    }
    {
        let mut put = |address, kind| {
            let offset = map
                .text_offset(address)
                .expect("OP85 resource fixture 명령 범위");
            text[offset..offset + 4].copy_from_slice(&encode(kind).to_le_bytes());
        };
        for (address, kind) in [
            (
                0x8002_3120,
                K::Lui {
                    rt: Reg::V1,
                    imm: 0x800E,
                },
            ),
            (
                0x8002_312C,
                K::Addiu {
                    rt: Reg::V1,
                    rs: Reg::V1,
                    imm: -0x4CD8,
                },
            ),
            (
                0x8002_3130,
                K::Lbu {
                    rt: Reg::V0,
                    base: Reg::A0,
                    offset: 1,
                },
            ),
            (
                0x8002_3134,
                K::Lw {
                    rt: Reg::A0,
                    base: Reg::A0,
                    offset: 4,
                },
            ),
            (
                0x8002_3138,
                K::Sll {
                    rd: Reg::V0,
                    rt: Reg::V0,
                    sa: 2,
                },
            ),
            (
                0x8002_313C,
                K::Addu {
                    rd: Reg::V0,
                    rs: Reg::V0,
                    rt: Reg::V1,
                },
            ),
            (
                0x8002_3140,
                K::Sw {
                    rt: Reg::A0,
                    base: Reg::V0,
                    offset: 0,
                },
            ),
            (
                0x8002_3188,
                K::Addiu {
                    rt: Reg::V0,
                    rs: Reg::V0,
                    imm: 8,
                },
            ),
        ] {
            put(address, kind);
        }
        for (
            id_load,
            state_offset,
            table_load,
            shift_three,
            add_id,
            shift_four,
            add_descriptor,
            call,
            store,
        ) in [
            (
                0x8007_10FC,
                0x04,
                0x8007_1100,
                0x8007_1104,
                0x8007_1108,
                0x8007_110C,
                0x8007_1110,
                0x8007_1114,
                0x8007_1118,
            ),
            (
                0x8007_11CC,
                0x06,
                0x8007_11D0,
                0x8007_11D4,
                0x8007_11D8,
                0x8007_11DC,
                0x8007_11E0,
                0x8007_11E4,
                0x8007_11E8,
            ),
            (
                0x8007_129C,
                0x08,
                0x8007_12A0,
                0x8007_12A4,
                0x8007_12A8,
                0x8007_12AC,
                0x8007_12B0,
                0x8007_12B4,
                0x8007_12B8,
            ),
        ] {
            put(
                id_load,
                K::Lhu {
                    rt: Reg::V1,
                    base: Reg::S0,
                    offset: state_offset,
                },
            );
            put(
                table_load,
                K::Lw {
                    rt: Reg::A0,
                    base: Reg::A0,
                    offset: -0x4CD0,
                },
            );
            put(
                shift_three,
                K::Sll {
                    rd: Reg::V0,
                    rt: Reg::V1,
                    sa: 3,
                },
            );
            put(
                add_id,
                K::Addu {
                    rd: Reg::V0,
                    rs: Reg::V0,
                    rt: Reg::V1,
                },
            );
            put(
                shift_four,
                K::Sll {
                    rd: Reg::V0,
                    rt: Reg::V0,
                    sa: 2,
                },
            );
            put(
                add_descriptor,
                K::Addu {
                    rd: Reg::A0,
                    rs: Reg::A0,
                    rt: Reg::V0,
                },
            );
            put(
                call,
                K::Jal {
                    target: QUEST_OP85_MAP_LOADER_RUNTIME,
                },
            );
            put(
                store,
                K::Sw {
                    rt: Reg::A0,
                    base: Reg::S0,
                    offset: 0x28,
                },
            );
        }
        for (lui, ori, id_load, state_offset, high, low) in [
            (0x8007_1374, 0x8007_1378, 0x8007_1384, 0x0A, 0x801D, 0x9270),
            (0x8007_13FC, 0x8007_1400, 0x8007_1410, 0x0C, 0x801F, 0x1270),
        ] {
            put(
                lui,
                K::Lui {
                    rt: Reg::A1,
                    imm: high,
                },
            );
            put(
                ori,
                K::Ori {
                    rt: Reg::A1,
                    rs: Reg::A1,
                    imm: low,
                },
            );
            put(
                id_load,
                K::Lhu {
                    rt: Reg::V1,
                    base: Reg::S0,
                    offset: state_offset,
                },
            );
        }
        for (address, kind) in [
            (
                0x8007_1414,
                K::Lw {
                    rt: Reg::A0,
                    base: Reg::A0,
                    offset: -0x4CD8,
                },
            ),
            (
                0x8007_1418,
                K::Sll {
                    rd: Reg::V0,
                    rt: Reg::V1,
                    sa: 3,
                },
            ),
            (
                0x8007_141C,
                K::Addu {
                    rd: Reg::V0,
                    rs: Reg::V0,
                    rt: Reg::V1,
                },
            ),
            (
                0x8007_1420,
                K::Sll {
                    rd: Reg::V0,
                    rt: Reg::V0,
                    sa: 2,
                },
            ),
            (
                0x8007_1424,
                K::Addu {
                    rd: Reg::A0,
                    rs: Reg::A0,
                    rt: Reg::V0,
                },
            ),
            (
                0x8007_1428,
                K::Jal {
                    target: QUEST_OP85_GRP_LOADER_RUNTIME,
                },
            ),
            (
                0x8007_142C,
                K::Sw {
                    rt: Reg::A0,
                    base: Reg::S0,
                    offset: 0x28,
                },
            ),
        ] {
            put(address, kind);
        }
        for (
            destination_lui,
            destination_ori,
            source_lui,
            source_ori,
            call,
            words,
            destination,
            source,
            copy_words,
        ) in [
            (
                0x8007_36F4,
                0x8007_36F8,
                0x8007_36FC,
                0x8007_3700,
                0x8007_3704,
                0x8007_3708,
                QUEST_OP85_GRP_STAGING_RUNTIME[0],
                QUEST_OP85_GRP_COPY_SOURCE_RUNTIME[0],
                QUEST_OP85_GRP_COPY_WORDS[0],
            ),
            (
                0x8007_37B0,
                0x8007_37B4,
                0x8007_37B8,
                0x8007_37BC,
                0x8007_37C0,
                0x8007_37C4,
                QUEST_OP85_GRP_STAGING_RUNTIME[1],
                QUEST_OP85_GRP_COPY_SOURCE_RUNTIME[1],
                QUEST_OP85_GRP_COPY_WORDS[1],
            ),
        ] {
            put(
                destination_lui,
                K::Lui {
                    rt: Reg::A0,
                    imm: u16::try_from(destination >> 16).expect("destination high"),
                },
            );
            put(
                destination_ori,
                K::Ori {
                    rt: Reg::A0,
                    rs: Reg::A0,
                    imm: u16::try_from(destination & 0xFFFF).expect("destination low"),
                },
            );
            put(
                source_lui,
                K::Lui {
                    rt: Reg::A1,
                    imm: u16::try_from(source >> 16).expect("source high"),
                },
            );
            put(
                source_ori,
                K::Ori {
                    rt: Reg::A1,
                    rs: Reg::A1,
                    imm: u16::try_from(source & 0xFFFF).expect("source low"),
                },
            );
            put(
                call,
                K::Jal {
                    target: 0x8007_E618,
                },
            );
            put(
                words,
                K::Addiu {
                    rt: Reg::A2,
                    rs: Reg::ZERO,
                    imm: i16::try_from(copy_words).expect("copy words"),
                },
            );
        }
    }
    for (index, &handler_runtime) in QUEST_SEQ_BASE_DISPATCH_HANDLERS.iter().enumerate() {
        let table_address = QUEST_SEQ_BASE_DISPATCH_TABLE_RUNTIME
            + u32::try_from(index).expect("base dispatch index") * 4;
        let offset = map.text_offset(table_address).expect("fixture table 범위");
        text[offset..offset + 4].copy_from_slice(&handler_runtime.to_le_bytes());
    }
    for (index, &handler_runtime) in QUEST_SEQ_DISPATCH_HANDLERS.iter().enumerate() {
        let opcode =
            QUEST_SEQ_BRANCH_DISPATCH_OPCODE_BASE + u8::try_from(index).expect("dispatch index");
        let table_address = dispatch_word_runtime(opcode).expect("지원 opcode");
        let offset = map.text_offset(table_address).expect("fixture table 범위");
        text[offset..offset + 4].copy_from_slice(&handler_runtime.to_le_bytes());
    }
    for (address, value) in [
        (0x8001_304C, 0x8007_3594u32),
        (0x800B_371C, 0x8007_3DE8u32),
        (0x8008_9320, 0x0000_0029),
        (0x8008_9324, QUEST_GRP_DESCRIPTOR_ROOT_RUNTIME),
        (0x8008_9328, 0x0000_0229),
        (0x8008_932C, QUEST_MAP_DESCRIPTOR_ROOT_RUNTIME),
        (0x8008_9330, 0x0000_0129),
        (0x8008_9334, QUEST_SEQ_DESCRIPTOR_ROOT_RUNTIME),
        (0x8008_C8FC, 0x8008_DAF8),
        (0x8008_DFC8, 0x8008_F308),
        (0x8008_F9D4, 0x8009_09FC),
        (0x8008_DAF8, 0x5F47_5052),
        (0x8008_DAFC, 0x2E50_5247),
        (0x8008_DB00, 0x0044_4456),
        (0x8008_F308, 0x5F47_5052),
        (0x8008_F30C, 0x2E50_414D),
        (0x8008_F310, 0x0044_4456),
        (0x8009_09FC, 0x5F47_5052),
        (0x8009_0A00, 0x2E51_4553),
        (0x8009_0A04, 0x0044_4456),
    ] {
        let offset = map.text_offset(address).expect("fixture 보조 table 범위");
        text[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    (map, text)
}

#[test]
fn 분기_pointer를_읽어_stream_pointer로_저장한다() {
    let (map, text) = fixture();
    assert_eq!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Ok(QuestSeqPointerConsumerSummary {
            interpreter_runtime: 0x8002_2570,
            base_dispatch_table_runtime: 0x800B_1C18,
            base_dispatch_entries: 128,
            unique_base_dispatch_handlers: 47,
            base_fallback_dispatch_entries: 82,
            dispatch_registration_runtime: 0x8007_80A8,
            dispatch_table_runtime: 0x800B_36F0,
            dispatch_opcode_base: 0x80,
            dispatch_opcode_count: 0x60,
            extended_dispatch_registration_runtime: 0x8007_0028,
            extended_dispatch_table_runtime: 0x800B_3670,
            extended_dispatch_opcode_base: 0xE0,
            extended_dispatch_opcode_count: 0x20,
            dispatch_entries: 128,
            unique_dispatch_handlers: 80,
            fallback_dispatch_entries: 50,
            consumer_runtime: 0x8007_8A80,
            stack_consumer_runtime: 0x8007_8B04,
            record_pointer_offset: 8,
            record_stride: 12,
            stream_pointer_state_offset: 0x1C,
            pointer_record_specs: 19,
            pointer_consumer_sites: 21,
            prefix_entry_runtime: 0x8019_9A70,
            op85_consumer_runtime: 0x8007_8280,
            op85_state_runtime: 0x800F_BDA0,
            op85_record_stride: 20,
            op85_selector_state_offset: 0x0E,
            op85_halfword_state_offsets: [0x10, 0x12, 0x14, 0x16, 0x18],
            op85_render_pointer_state_offsets: [0x1C, 0x20],
            grp_descriptor_slot_runtime: 0x800D_B328,
            seq_descriptor_slot_runtime: 0x800D_B32C,
            map_descriptor_slot_runtime: 0x800D_B330,
            grp_descriptor_root_runtime: 0x8008_C8F8,
            seq_descriptor_root_runtime: 0x8008_F9D0,
            map_descriptor_root_runtime: 0x8008_DFC4,
            op85_map_loader_runtime: 0x8001_B12C,
            op85_grp_loader_runtime: 0x8001_B1D4,
            op85_grp_staging_runtime: [0x801D_9270, 0x801F_1270],
            op85_grp_copy_source_runtime: [0x8016_2A00, 0x8017_2A00],
            op85_grp_copy_words: [0x6000, 0x2000],
        })
    );
}

#[test]
fn town00_prefix_entry와_op85_halfword_drift를_거부한다() {
    let (map, mut text) = fixture();
    let entry = map.text_offset(0x8007_3C74).expect("fixture 명령 범위");
    text[entry..entry + 4].copy_from_slice(
        &encode(K::Ori {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: 0x9A74,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_3C74,
            ..
        })
    ));

    let (map, mut text) = fixture();
    let halfword = map.text_offset(0x8007_82F0).expect("fixture 명령 범위");
    text[halfword..halfword + 4].copy_from_slice(
        &encode(K::Lhu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 8,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_82F0,
            ..
        })
    ));
}

#[test]
fn quest_resource_slot과_op85_map_grp소비_drift를_거부한다() {
    let (map, mut text) = fixture();
    let map_record = map.text_offset(0x8008_9328).expect("fixture data 범위");
    text[map_record..map_record + 4].copy_from_slice(&0x0000_0129u32.to_le_bytes());
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::DataWordMismatch {
            address: 0x8008_9328,
            ..
        })
    ));

    let (map, mut text) = fixture();
    let map_slot = map.text_offset(0x8007_1100).expect("fixture 명령 범위");
    text[map_slot..map_slot + 4].copy_from_slice(
        &encode(K::Lw {
            rt: Reg::A0,
            base: Reg::A0,
            offset: -0x4CD8,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_1100,
            ..
        })
    ));

    let (map, mut text) = fixture();
    let copy_size = map.text_offset(0x8007_3708).expect("fixture 명령 범위");
    text[copy_size..copy_size + 4].copy_from_slice(
        &encode(K::Addiu {
            rt: Reg::A2,
            rs: Reg::ZERO,
            imm: 0x2000,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_3708,
            ..
        })
    ));
}

#[test]
fn pointer_load와_store_drift를_거부한다() {
    let (map, mut text) = fixture();
    let offset = map.text_offset(0x8007_8A80).expect("fixture 명령 범위");
    text[offset..offset + 4].copy_from_slice(
        &encode(K::Lw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 4,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_8A80,
            ..
        })
    ));
}

#[test]
fn op80조건과_op88역참조_drift를_거부한다() {
    let (map, mut text) = fixture();
    let condition = map.text_offset(0x8007_8124).expect("fixture 명령 범위");
    text[condition..condition + 4].copy_from_slice(
        &encode(K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 14,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_8124,
            ..
        })
    ));

    let (map, mut text) = fixture();
    let dereference = map.text_offset(0x8007_8408).expect("fixture 명령 범위");
    text[dereference..dereference + 4].copy_from_slice(
        &encode(K::Lw {
            rt: Reg::V1,
            base: Reg::V1,
            offset: 4,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_8408,
            ..
        })
    ));
}

#[test]
fn op8f_count상한과_halfword_pair_drift를_거부한다() {
    let (map, mut text) = fixture();
    let clamp = map.text_offset(0x8007_8554).expect("fixture 명령 범위");
    text[clamp..clamp + 4].copy_from_slice(
        &encode(K::Sltiu {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: 16,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_8554,
            ..
        })
    ));

    let (map, mut text) = fixture();
    let second_halfword = map.text_offset(0x8007_8594).expect("fixture 명령 범위");
    text[second_halfword..second_halfword + 4].copy_from_slice(
        &encode(K::Lhu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_8594,
            ..
        })
    ));
}

#[test]
fn op83_scheduler와_op95복원_drift를_거부한다() {
    let (map, mut text) = fixture();
    let scheduler = map.text_offset(0x8001_4EA4).expect("fixture 명령 범위");
    text[scheduler..scheduler + 4].copy_from_slice(
        &encode(K::Jal {
            target: 0x8002_2574,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8001_4EA4,
            ..
        })
    ));

    let (map, mut text) = fixture();
    let restore = map.text_offset(0x8007_3EF8).expect("fixture 명령 범위");
    text[restore..restore + 4].copy_from_slice(
        &encode(K::Lw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x170,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_3EF8,
            ..
        })
    ));
}

#[test]
fn op85두_pointer와_renderer역참조_drift를_거부한다() {
    let (map, mut text) = fixture();
    let second_pointer = map.text_offset(0x8007_8340).expect("fixture 명령 범위");
    text[second_pointer..second_pointer + 4].copy_from_slice(
        &encode(K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 12,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_8340,
            ..
        })
    ));

    let (map, mut text) = fixture();
    let dereference = map.text_offset(0x8002_56C8).expect("fixture 명령 범위");
    text[dereference..dereference + 4].copy_from_slice(
        &encode(K::Lhu {
            rt: Reg::V1,
            base: Reg::A3,
            offset: 2,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8002_56C8,
            ..
        })
    ));
}

#[test]
fn op89_inner두_pointer_drift를_거부한다() {
    let (map, mut text) = fixture();
    let inner_pointer = map.text_offset(0x8007_3A48).expect("fixture 명령 범위");
    text[inner_pointer..inner_pointer + 4].copy_from_slice(
        &encode(K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 12,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_3A48,
            ..
        })
    ));
}

#[test]
fn op93_object와renderer_pointer_drift를_거부한다() {
    let (map, mut text) = fixture();
    let object_dereference = map.text_offset(0x8007_99C8).expect("fixture 명령 범위");
    text[object_dereference..object_dereference + 4].copy_from_slice(
        &encode(K::Lhu {
            rt: Reg::A0,
            base: Reg::V0,
            offset: 2,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_99C8,
            ..
        })
    ));

    let (map, mut text) = fixture();
    let renderer_argument = map.text_offset(0x8007_788C).expect("fixture 명령 범위");
    text[renderer_argument..renderer_argument + 4].copy_from_slice(
        &encode(K::Sw {
            rt: Reg::V0,
            base: Reg::SP,
            offset: 0x14,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_788C,
            ..
        })
    ));
}

#[test]
fn op9f_byte역참조와_opc1조건_drift를_거부한다() {
    let (map, mut text) = fixture();
    let dereference = map.text_offset(0x8003_B354).expect("fixture 명령 범위");
    text[dereference..dereference + 4].copy_from_slice(
        &encode(K::Lbu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 1,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8003_B354,
            ..
        })
    ));

    let (map, mut text) = fixture();
    let condition = map.text_offset(0x8007_9668).expect("fixture 명령 범위");
    text[condition..condition + 4].copy_from_slice(
        &encode(K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 9,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_9668,
            ..
        })
    ));
}

#[test]
fn ope3_copy역참조와_extended_dispatch_drift를_거부한다() {
    let (map, mut text) = fixture();
    let dereference = map.text_offset(0x8007_E620).expect("fixture 명령 범위");
    text[dereference..dereference + 4].copy_from_slice(
        &encode(K::Lw {
            rt: Reg::V0,
            base: Reg::A1,
            offset: 4,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_E620,
            ..
        })
    ));

    let (map, mut text) = fixture();
    let table_address = dispatch_word_runtime(0xE3).expect("OPE3 dispatch word");
    let table_offset = map.text_offset(table_address).expect("fixture table 범위");
    text[table_offset..table_offset + 4].copy_from_slice(&0x8006_FF38u32.to_le_bytes());
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::DataWordMismatch {
            address,
            ..
        }) if address == table_address
    ));

    let (map, mut text) = fixture();
    let registration = map.text_offset(0x8007_0034).expect("fixture 명령 범위");
    text[registration..registration + 4].copy_from_slice(
        &encode(K::Addiu {
            rt: Reg::A1,
            rs: Reg::ZERO,
            imm: 0xDF,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_0034,
            ..
        })
    ));
}

#[test]
fn 포인터_record가_아닌_opcode의_dispatch_drift도_거부한다() {
    let (map, mut text) = fixture();
    let table_address = dispatch_word_runtime(0x82).expect("OP82 dispatch word");
    let table_offset = map.text_offset(table_address).expect("fixture table 범위");
    text[table_offset..table_offset + 4].copy_from_slice(&0x8007_2848u32.to_le_bytes());
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::DataWordMismatch {
            address,
            expected: 0x8007_2844,
            actual: 0x8007_2848,
        }) if address == table_address
    ));
}

#[test]
fn a2_pointer_load와_dispatch_table_drift를_거부한다() {
    let (map, mut text) = fixture();
    let pointer_offset = map.text_offset(0x8007_8B04).expect("fixture 명령 범위");
    text[pointer_offset..pointer_offset + 4].copy_from_slice(
        &encode(K::Lw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 4,
        })
        .to_le_bytes(),
    );
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::InstructionMismatch {
            address: 0x8007_8B04,
            ..
        })
    ));

    let (map, mut text) = fixture();
    let table_address = QUEST_SEQ_BRANCH_DISPATCH_TABLE_RUNTIME + u32::from(0xA1u8 - 0x80u8) * 4;
    let table_offset = map.text_offset(table_address).expect("fixture table 범위");
    text[table_offset..table_offset + 4].copy_from_slice(&0x8007_8AA0u32.to_le_bytes());
    assert!(matches!(
        verify_box_quest_seq_pointer_consumer(map, &text),
        Err(QuestSeqPointerConsumerError::DataWordMismatch {
            address,
            ..
        }) if address == table_address
    ));
}

#[test]
fn 다른_exe의_사상을_이식하지_않는다() {
    let (map, text) = fixture();
    let other = exe_map("PUYO1.EXE").expect("PUYO1.EXE 사상");
    assert_eq!(
        verify_box_quest_seq_pointer_consumer(other, &text),
        Err(QuestSeqPointerConsumerError::WrongExecutable("PUYO1.EXE"))
    );
    assert_eq!(map.name, "QUEST.EXE");
}
