//! `QUEST.EXE`의 `FFxx` 제어코드 의미 소비 검증.
//!
//! dispatch와 stream 폭은 [`crate::script::quest`]가 소유한다. 이 모듈은
//! 그중 handler의 직접 상태 변경과 하위 소비 함수가 함께 닫히는 의미만
//! 원본 typed MIPS로 검증한다. 장면 문맥만으로 이름을 붙이지 않는다.

use std::error::Error;
use std::fmt;

use crate::coords::ExeMap;
use crate::isa::{Instruction, InstructionKind as K, Reg};
use crate::script::{quest_control_meaning, QuestControlMeaning, QUEST_CONFIRMED_CONTROL_MEANINGS};

use super::linear;

pub const QUEST_ADVANCE_LINE_RUNTIME: u32 = 0x8002_C664;
pub const QUEST_FIXED_TEXT_POINTER_RUNTIME: u32 = 0x800D_B7A0;
pub const QUEST_CLOSE_TEXT_WINDOW_RUNTIME: u32 = 0x8002_C6BC;
pub const QUEST_OPEN_TEXT_WINDOW_RUNTIME: u32 = 0x8002_C22C;
pub const QUEST_RELEASE_OBJECT_RUNTIME: u32 = 0x8001_4CD0;
pub const QUEST_ALLOCATE_OBJECT_RUNTIME: u32 = 0x8001_4B14;
pub const QUEST_SEQ_DISPATCH_YIELD_GP_OFFSET: i16 = 0x518;

/// 기본 10종 뒤에 확정한 의미의 direct·downstream typed MIPS evidence.
///
/// unit fixture도 같은 표에서 생성해 verifier와 fixture의 수동 중복을
/// 피한다. 주소 하나라도 drift하면 source gate 전체가 실패한다.
const QUEST_ADDITIONAL_CONTROL_MEANING_EVIDENCE: &[(u32, &str, K)] = &[
    // 공통 interpreter dispatcher는 +0x518 flag를 0으로 시작하고,
    // handler가 1로 만들면 같은 pass의 반복을 끝낸다.
    (
        0x8002_2594,
        "sw zero, 0x518(gp)",
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::GP,
            offset: QUEST_SEQ_DISPATCH_YIELD_GP_OFFSET,
        },
    ),
    (
        0x8002_25E0,
        "lw v0, 0x518(gp)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::GP,
            offset: QUEST_SEQ_DISPATCH_YIELD_GP_OFFSET,
        },
    ),
    (
        0x8002_25E8,
        "beq v0, zero, -21",
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: -21,
        },
    ),
    // FF00: 객체 해제 뒤 dispatcher flag를 1로 만든다.
    (
        0x8003_DE04,
        "jal 0x80014CD0",
        K::Jal {
            target: QUEST_RELEASE_OBJECT_RUNTIME,
        },
    ),
    (
        0x8003_DE10,
        "addiu v0, zero, 1",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 1,
        },
    ),
    (
        0x8003_DE18,
        "sw v0, -0x1B60(at)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::AT,
            offset: -0x1B60,
        },
    ),
    // FF01: stream +2 뒤 dispatcher flag를 1로 만든다.
    (
        0x8003_DE28,
        "addiu v1, zero, 1",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 1,
        },
    ),
    (
        0x8003_DE30,
        "sw v1, -0x1B60(at)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::AT,
            offset: -0x1B60,
        },
    ),
    (
        0x8003_DE34,
        "addiu v0, v0, 2",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 2,
        },
    ),
    (
        0x8003_DE3C,
        "sw v0, 0x1C(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    ),
    // FF02: active window이면 countdown을 취소하고, 아니면 state +0x3A의
    // u16을 초기화·감소시키며 남아 있는 동안 dispatcher를 yield한다.
    (
        0x8003_DE68,
        "lbu v0, 0x1F(v0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 0x1F,
        },
    ),
    (
        0x8003_DE7C,
        "sh zero, 0x3A(a1)",
        K::Sh {
            rt: Reg::ZERO,
            base: Reg::A1,
            offset: 0x3A,
        },
    ),
    (
        0x8003_DE80,
        "addiu v0, v0, 4",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 4,
        },
    ),
    (
        0x8003_DE8C,
        "lh v0, 0x3A(a1)",
        K::Lh {
            rt: Reg::V0,
            base: Reg::A1,
            offset: 0x3A,
        },
    ),
    (
        0x8003_DE98,
        "addiu v0, v1, -1",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: -1,
        },
    ),
    (
        0x8003_DE9C,
        "sh v0, 0x3A(a1)",
        K::Sh {
            rt: Reg::V0,
            base: Reg::A1,
            offset: 0x3A,
        },
    ),
    (
        0x8003_DEB0,
        "sw v0, -0x1B60(at)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::AT,
            offset: -0x1B60,
        },
    ),
    (
        0x8003_DED0,
        "lhu v1, 2(v0)",
        K::Lhu {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 2,
        },
    ),
    (
        0x8003_DEDC,
        "sw v0, -0x1B60(at)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::AT,
            offset: -0x1B60,
        },
    ),
    (
        0x8003_DEE4,
        "sh v1, 0x3A(a1)",
        K::Sh {
            rt: Reg::V1,
            base: Reg::A1,
            offset: 0x3A,
        },
    ),
    // FF08: signed word 두 개를 4-bit fixed point scale로 쓴다.
    (
        0x8003_DF94,
        "lh v1, 2(a2)",
        K::Lh {
            rt: Reg::V1,
            base: Reg::A2,
            offset: 2,
        },
    ),
    (
        0x8003_DF9C,
        "sll v1, v1, 4",
        K::Sll {
            rd: Reg::V1,
            rt: Reg::V1,
            sa: 4,
        },
    ),
    (
        0x8003_DFA0,
        "sw v1, 0x58(v0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x58,
        },
    ),
    (
        0x8003_DFCC,
        "lh v1, 2(a2)",
        K::Lh {
            rt: Reg::V1,
            base: Reg::A2,
            offset: 2,
        },
    ),
    (
        0x8003_DFD4,
        "sll v1, v1, 4",
        K::Sll {
            rd: Reg::V1,
            rt: Reg::V1,
            sa: 4,
        },
    ),
    (
        0x8003_DFD8,
        "sw v1, 0x5C(v0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x5C,
        },
    ),
    // FF0A: +0x16은 glyph 뒤 cursor x, +0x17은 line 전진 y에 더해진다.
    (
        0x8003_E0A0,
        "sb v1, 0x16(v0)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x16,
        },
    ),
    (
        0x8003_E0D0,
        "sb v1, 0x17(v0)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x17,
        },
    ),
    (
        0x8002_BE24,
        "lbu v1, 0x16(v0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x16,
        },
    ),
    (
        0x8002_BE2C,
        "addu v1, a1, v1",
        K::Addu {
            rd: Reg::V1,
            rs: Reg::A1,
            rt: Reg::V1,
        },
    ),
    (
        0x8002_C694,
        "lbu v1, 0x17(v0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x17,
        },
    ),
    // FF0B: +0x0F와 +0x10 counter가 glyph dispatch 간격을 만든다.
    (
        0x8003_E11C,
        "sb v1, 0x0F(v0)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x0F,
        },
    ),
    (
        0x8003_B964,
        "lbu v0, 0x0F(v1)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x0F,
        },
    ),
    (
        0x8003_B984,
        "lbu v0, 0x10(v1)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x10,
        },
    ),
    (
        0x8003_B990,
        "sb v0, 0x10(v1)",
        K::Sb {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x10,
        },
    ),
    (
        0x8003_B9B4,
        "lbu v1, 0x0F(a0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0x0F,
        },
    ),
    (
        0x8003_B9B8,
        "lbu v0, 0x10(a0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x10,
        },
    ),
    (
        0x8003_B9C0,
        "sltu v0, v0, v1",
        K::Sltu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::V1,
        },
    ),
    // FF0C: +0x0E는 glyph object +0x6A texture-page word로 변환된다.
    (
        0x8003_E168,
        "sb v1, 0x0E(v0)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x0E,
        },
    ),
    (
        0x8002_BC84,
        "lbu v0, 0x0E(v1)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x0E,
        },
    ),
    (
        0x8002_BC8C,
        "sll v0, v0, 4",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 4,
        },
    ),
    (
        0x8002_BC90,
        "addiu v0, v0, 0x500",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0x500,
        },
    ),
    (
        0x8002_BC94,
        "sh v0, 0x6A(a2)",
        K::Sh {
            rt: Reg::V0,
            base: Reg::A2,
            offset: 0x6A,
        },
    ),
    (
        0x8001_7B34,
        "lhu v1, 0x6A(s1)",
        K::Lhu {
            rt: Reg::V1,
            base: Reg(17),
            offset: 0x6A,
        },
    ),
    // FF0D: +0x1E가 36-byte font resource descriptor 격자를 고른다.
    (
        0x8003_E1B4,
        "sb v1, 0x1E(v0)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x1E,
        },
    ),
    (
        0x8002_BBD4,
        "lbu v1, 0x1E(a1)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A1,
            offset: 0x1E,
        },
    ),
    (
        0x8002_BBE0,
        "addu v0, v0, v1",
        K::Addu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::V1,
        },
    ),
    (
        0x8002_BBF8,
        "lw v1, 0x1C(v1)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::V1,
            offset: 0x1C,
        },
    ),
    // FF0E: +0x15-1이 object +0x6F가 되고 +0x6E와 합쳐 OT index가 된다.
    (
        0x8003_E200,
        "sb v1, 0x15(v0)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x15,
        },
    ),
    (
        0x8002_BF54,
        "lbu v0, 0x15(s3)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg(19),
            offset: 0x15,
        },
    ),
    (
        0x8002_BF5C,
        "addiu v0, v0, -1",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: -1,
        },
    ),
    (
        0x8002_BF60,
        "sb v0, 0x6F(a1)",
        K::Sb {
            rt: Reg::V0,
            base: Reg::A1,
            offset: 0x6F,
        },
    ),
    (
        0x8001_7B7C,
        "lbu v1, 0x6F(s1)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg(17),
            offset: 0x6F,
        },
    ),
    (
        0x8001_7B88,
        "addu s0, v1, v0",
        K::Addu {
            rd: Reg::S0,
            rs: Reg::V1,
            rt: Reg::V0,
        },
    ),
    // FF10: 0은 12×16 default이고, 두 field는 glyph scale·line height에 쓰인다.
    (
        0x8003_BC50,
        "sb v1, 0x12(v0)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x12,
        },
    ),
    (
        0x8003_BC80,
        "sb v1, 0x13(v0)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x13,
        },
    ),
    (
        0x8003_BCB8,
        "addiu v0, zero, 12",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 12,
        },
    ),
    (
        0x8003_BCBC,
        "sb v0, 0x12(v1)",
        K::Sb {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x12,
        },
    ),
    (
        0x8003_BCEC,
        "addiu v0, zero, 16",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::ZERO,
            imm: 16,
        },
    ),
    (
        0x8003_BCF0,
        "sb v0, 0x13(v1)",
        K::Sb {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x13,
        },
    ),
    (
        0x8002_BC20,
        "lbu a1, 0x12(a1)",
        K::Lbu {
            rt: Reg::A1,
            base: Reg::A1,
            offset: 0x12,
        },
    ),
    (
        0x8002_BC74,
        "lbu v0, 0x13(v1)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x13,
        },
    ),
    // FF11: 첫 u8이 0이면 OR, 아니면 NOT mask와 AND한다.
    (
        0x8003_E278,
        "lbu v0, 2(v0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 2,
        },
    ),
    (
        0x8003_E2B4,
        "or v1, v1, a0",
        K::Or {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::A0,
        },
    ),
    (
        0x8003_E2BC,
        "sb v1, 0x19(v0)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x19,
        },
    ),
    (
        0x8003_E2E8,
        "nor v1, zero, v1",
        K::Nor {
            rd: Reg::V1,
            rs: Reg::ZERO,
            rt: Reg::V1,
        },
    ),
    (
        0x8003_E2EC,
        "and a0, a0, v1",
        K::And {
            rd: Reg::A0,
            rs: Reg::A0,
            rt: Reg::V1,
        },
    ),
    (
        0x8003_E2F0,
        "sb a0, 0x19(v0)",
        K::Sb {
            rt: Reg::A0,
            base: Reg::V0,
            offset: 0x19,
        },
    ),
    // FF13: source mode로 얻은 값을 10으로 나누고 decimal glyph renderer를 부른다.
    (
        0x8003_E438,
        "jal 0x8002C050",
        K::Jal {
            target: 0x8002_C050,
        },
    ),
    (
        0x8002_C0E8,
        "divu s2, s0",
        K::Divu {
            rs: Reg(18),
            rt: Reg::S0,
        },
    ),
    (
        0x8002_C1F8,
        "jal 0x8002BA40",
        K::Jal {
            target: 0x8002_BA40,
        },
    ),
    // FF18: +0x18을 glyph consumer가 0..7 callback/effect dispatch에 사용한다.
    (
        0x8003_E5B4,
        "sb v1, 0x18(v0)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x18,
        },
    ),
    (
        0x8002_BA9C,
        "lbu v1, 0x18(a0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0x18,
        },
    ),
    (
        0x8002_BAA4,
        "sltiu v0, v1, 8",
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: 8,
        },
    ),
    (0x8002_BAC4, "jr v0", K::Jr { rs: Reg::V0 }),
    // FF16: item count가 있는 동안 menu 입력 helper를 반복하고 선택 index
    // 또는 cancel 값을 interpreter +0x34에 저장한다.
    (
        0x8003_BD94,
        "lbu v0, 0x21(v1)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x21,
        },
    ),
    (
        0x8003_BDA8,
        "sw s0, -0x1B60(at)",
        K::Sw {
            rt: Reg::S0,
            base: Reg::AT,
            offset: -0x1B60,
        },
    ),
    (
        0x8003_BDB0,
        "jal 0x8002C2E0",
        K::Jal {
            target: 0x8002_C2E0,
        },
    ),
    (
        0x8003_BE18,
        "addiu v0, v0, -1",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: -1,
        },
    ),
    (
        0x8003_BE74,
        "sw v0, 0x34(s1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg(17),
            offset: 0x34,
        },
    ),
    (
        0x8003_BEB4,
        "sb zero, 0x21(v0)",
        K::Sb {
            rt: Reg::ZERO,
            base: Reg::V0,
            offset: 0x21,
        },
    ),
    (
        0x8002_C358,
        "lbu v0, 0x20(s0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::S0,
            offset: 0x20,
        },
    ),
    (
        0x8002_C364,
        "sb v0, 0x20(s0)",
        K::Sb {
            rt: Reg::V0,
            base: Reg::S0,
            offset: 0x20,
        },
    ),
    (
        0x8002_C3D8,
        "lbu v1, 0x21(a0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0x21,
        },
    ),
    (
        0x8002_C3E0,
        "sb v0, 0x20(a0)",
        K::Sb {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x20,
        },
    ),
    (
        0x8002_C404,
        "andi v0, v1, 0x34",
        K::Andi {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: 0x34,
        },
    ),
    (
        0x8002_C40C,
        "andi v0, v1, 0x41",
        K::Andi {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: 0x41,
        },
    ),
    // FF17: mode별 renderer 위치·scale transition을 시작하거나 transition
    // object의 +0xC6 완료를 기다리고 stream을 전진시킨다.
    (
        0x8003_BF18,
        "lbu v1, 4(a1)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A1,
            offset: 4,
        },
    ),
    (
        0x8003_C014,
        "jal 0x8002C9BC",
        K::Jal {
            target: 0x8002_C9BC,
        },
    ),
    (
        0x8003_C17C,
        "jal 0x8002CB38",
        K::Jal {
            target: 0x8002_CB38,
        },
    ),
    (
        0x8003_C1B0,
        "lh v0, 0xC6(v0)",
        K::Lh {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 0xC6,
        },
    ),
    (
        0x8003_C208,
        "sb zero, 4(v1)",
        K::Sb {
            rt: Reg::ZERO,
            base: Reg::V1,
            offset: 4,
        },
    ),
    (
        0x8002_C9FC,
        "sw a3, 0x58(v1)",
        K::Sw {
            rt: Reg::A3,
            base: Reg::V1,
            offset: 0x58,
        },
    ),
    (
        0x8002_CA08,
        "sw v0, 0x5C(v1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x5C,
        },
    ),
    (
        0x8002_CB34,
        "sb a1, 4(t0)",
        K::Sb {
            rt: Reg::A1,
            base: Reg::T0,
            offset: 4,
        },
    ),
    // FF19: GP +0xF6가 0이 아니면 controller button에 의한 window
    // fast-forward state(+0x10=0, +0x1F=1) 갱신을 건너뛴다.
    (
        0x8003_E5DC,
        "lbu v1, 2(v1)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::V1,
            offset: 2,
        },
    ),
    (
        0x8003_E5E8,
        "sb v1, 0xF6(gp)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::GP,
            offset: 0xF6,
        },
    ),
    (
        0x8003_B878,
        "lbu v0, 0xF6(gp)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::GP,
            offset: 0xF6,
        },
    ),
    (
        0x8003_B880,
        "bne v0, zero, 26",
        K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 26,
        },
    ),
    (
        0x8003_B890,
        "andi v0, v0, 0xC1",
        K::Andi {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 0xC1,
        },
    ),
    (
        0x8003_B8BC,
        "sb zero, 0x10(v0)",
        K::Sb {
            rt: Reg::ZERO,
            base: Reg::V0,
            offset: 0x10,
        },
    ),
    (
        0x8003_B8E8,
        "sb v1, 0x1F(v0)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x1F,
        },
    ),
    // FF1A: parent +0xA2를 올리고 현재 interpreter를 해제한 뒤 yield.
    (
        0x8003_E600,
        "lw v1, 0x10(a0)",
        K::Lw {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0x10,
        },
    ),
    (
        0x8003_E610,
        "lhu v0, 0xA2(v1)",
        K::Lhu {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0xA2,
        },
    ),
    (
        0x8003_E61C,
        "sh v0, 0xA2(v1)",
        K::Sh {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0xA2,
        },
    ),
    (
        0x8003_E620,
        "jal 0x80014CD0",
        K::Jal {
            target: QUEST_RELEASE_OBJECT_RUNTIME,
        },
    ),
    (
        0x8003_E634,
        "sw v0, -0x1B60(at)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::AT,
            offset: -0x1B60,
        },
    ),
    // FF1B: source mode·asset index·두 좌표로 0x54..0x5C renderer
    // animation 객체를 만들고 callback이 이동·렌더·해제를 수행한다.
    (
        0x8003_C248,
        "lui a0, 0x8004",
        K::Lui {
            rt: Reg::A0,
            imm: 0x8004,
        },
    ),
    (
        0x8003_C24C,
        "addiu a0, a0, -9040",
        K::Addiu {
            rt: Reg::A0,
            rs: Reg::A0,
            imm: -9040,
        },
    ),
    (
        0x8003_C260,
        "jal 0x80014B14",
        K::Jal {
            target: QUEST_ALLOCATE_OBJECT_RUNTIME,
        },
    ),
    (
        0x8003_C28C,
        "lbu v1, 0(a0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0,
        },
    ),
    (
        0x8003_C298,
        "lbu a2, 1(a0)",
        K::Lbu {
            rt: Reg::A2,
            base: Reg::A0,
            offset: 1,
        },
    ),
    (
        0x8003_C3F4,
        "sltiu v0, v1, 9",
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: 9,
        },
    ),
    (
        0x8003_C460,
        "sh v0, 0x66(a1)",
        K::Sh {
            rt: Reg::V0,
            base: Reg::A1,
            offset: 0x66,
        },
    ),
    (
        0x8003_DD24,
        "lw a1, 0x88(a2)",
        K::Lw {
            rt: Reg::A1,
            base: Reg::A2,
            offset: 0x88,
        },
    ),
    (
        0x8003_DD44,
        "sw v1, 0x48(a2)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::A2,
            offset: 0x48,
        },
    ),
    (
        0x8003_DDB4,
        "jal 0x80014CD0",
        K::Jal {
            target: QUEST_RELEASE_OBJECT_RUNTIME,
        },
    ),
    (
        0x8003_DDC4,
        "jal 0x8006EE2C",
        K::Jal {
            target: 0x8006_EE2C,
        },
    ),
    // FF1C: 현재 menu item count +0x9F slot에 u8 값을 기록하고
    // 연결 renderer의 +0x5C field를 16 올린다.
    (
        0x8003_E678,
        "lbu v0, 0x21(v0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 0x21,
        },
    ),
    (
        0x8003_E67C,
        "lbu v1, 2(a3)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A3,
            offset: 2,
        },
    ),
    (
        0x8003_E680,
        "addiu v0, v0, 159",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 159,
        },
    ),
    (
        0x8003_E684,
        "sll v0, v0, 2",
        K::Sll {
            rd: Reg::V0,
            rt: Reg::V0,
            sa: 2,
        },
    ),
    (
        0x8003_E68C,
        "sw v1, 0(v0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0,
        },
    ),
    (
        0x8003_E6B8,
        "lw v0, 0x5C(v1)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x5C,
        },
    ),
    (
        0x8003_E6C0,
        "addiu v0, v0, 16",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 16,
        },
    ),
    (
        0x8003_E6C4,
        "sw v0, 0x5C(v1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x5C,
        },
    ),
    // FF1D: FF16과 같은 menu 입력을 기다리되 confirm은 +0x9F mapping
    // table 값을, cancel은 두 번째 u8을 interpreter +0x34에 쓴다.
    (
        0x8003_C528,
        "lbu v0, 0x21(v1)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x21,
        },
    ),
    (
        0x8003_C53C,
        "sw s0, -0x1B60(at)",
        K::Sw {
            rt: Reg::S0,
            base: Reg::AT,
            offset: -0x1B60,
        },
    ),
    (
        0x8003_C544,
        "jal 0x8002C2E0",
        K::Jal {
            target: 0x8002_C2E0,
        },
    ),
    (
        0x8003_C5A4,
        "lbu v0, 0x20(v0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 0x20,
        },
    ),
    (
        0x8003_C5AC,
        "addiu v0, v0, 159",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 159,
        },
    ),
    (
        0x8003_C5B8,
        "lw v0, 0(v0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 0,
        },
    ),
    (
        0x8003_C5C0,
        "sw v0, 0x34(s1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg(17),
            offset: 0x34,
        },
    ),
    (
        0x8003_C5FC,
        "lbu v0, 3(v0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::V0,
            offset: 3,
        },
    ),
    (
        0x8003_C604,
        "sw v0, 0x34(s1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg(17),
            offset: 0x34,
        },
    ),
    (
        0x8003_C644,
        "sb zero, 0x21(v0)",
        K::Sb {
            rt: Reg::ZERO,
            base: Reg::V0,
            offset: 0x21,
        },
    ),
    // FF1E: mode 0은 고정 u16 string을 glyph renderer로 순회하고,
    // mode 1..5는 고정 global 값을 decimal renderer에 넘긴다.
    (
        0x8003_C6A0,
        "lbu v1, 2(v0)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 2,
        },
    ),
    (
        0x8003_C6A8,
        "sltiu v0, v1, 6",
        K::Sltiu {
            rt: Reg::V0,
            rs: Reg::V1,
            imm: 6,
        },
    ),
    (
        0x8003_C6D8,
        "lw s2, -18528(v0)",
        K::Lw {
            rt: Reg(18),
            base: Reg::V0,
            offset: -18528,
        },
    ),
    (
        0x8003_C6E0,
        "jal 0x80091D98",
        K::Jal {
            target: 0x8009_1D98,
        },
    ),
    (
        0x8003_C6F8,
        "lhu a1, 0(v0)",
        K::Lhu {
            rt: Reg::A1,
            base: Reg::V0,
            offset: 0,
        },
    ),
    (
        0x8003_C6FC,
        "jal 0x8002BA40",
        K::Jal {
            target: 0x8002_BA40,
        },
    ),
    (
        0x8003_C720,
        "lw a1, -18536(v0)",
        K::Lw {
            rt: Reg::A1,
            base: Reg::V0,
            offset: -18536,
        },
    ),
    (
        0x8003_C774,
        "jal 0x8002C050",
        K::Jal {
            target: 0x8002_C050,
        },
    ),
    (
        0x8003_C784,
        "addiu v0, v0, 2",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 2,
        },
    ),
];

/// 원본 소비 코드에서 확인한 제어코드 의미 분모.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestControlMeaningSummary {
    pub confirmed_meanings: usize,
    pub advance_line_runtime: u32,
    pub close_text_window_runtime: u32,
    pub open_text_window_runtime: u32,
    pub release_object_runtime: u32,
    pub allocate_object_runtime: u32,
}

/// QUEST 제어코드 의미 검증 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestControlMeaningError {
    WrongExecutable(&'static str),
    TextLengthMismatch {
        expected: usize,
        actual: usize,
    },
    UnalignedText,
    MeaningTableMismatch {
        code: u8,
        expected: QuestControlMeaning,
        actual: Option<QuestControlMeaning>,
    },
    MissingInstruction(u32),
    InstructionMismatch {
        address: u32,
        expected: &'static str,
        actual: K,
    },
}

impl fmt::Display for QuestControlMeaningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongExecutable(exe) => {
                write!(f, "QUEST control 의미 검증 대상이 QUEST.EXE가 아님: {exe}")
            }
            Self::TextLengthMismatch { expected, actual } => write!(
                f,
                "QUEST.EXE text 길이 불일치: expected={expected}, actual={actual}"
            ),
            Self::UnalignedText => write!(f, "QUEST.EXE text 길이가 4바이트 정렬이 아님"),
            Self::MeaningTableMismatch {
                code,
                expected,
                actual,
            } => write!(
                f,
                "QUEST control 의미 표 불일치: code=FF{code:02X}, \
                 expected={expected:?}, actual={actual:?}"
            ),
            Self::MissingInstruction(address) => write!(
                f,
                "QUEST control 의미 소비 명령이 text 밖임: 0x{address:08X}"
            ),
            Self::InstructionMismatch {
                address,
                expected,
                actual,
            } => write!(
                f,
                "QUEST control 의미 소비 명령 불일치: address=0x{address:08X}, \
                 expected={expected}, actual={actual:?}"
            ),
        }
    }
}

impl Error for QuestControlMeaningError {}

/// 지원 원본 `QUEST.EXE`에서 확정 의미 32종의 직접·하위 소비를 검증한다.
pub fn verify_box_quest_control_meanings(
    map: &ExeMap,
    text: &[u8],
) -> Result<QuestControlMeaningSummary, QuestControlMeaningError> {
    if map.name != "QUEST.EXE" {
        return Err(QuestControlMeaningError::WrongExecutable(map.name));
    }
    if text.len() != map.text_len as usize {
        return Err(QuestControlMeaningError::TextLengthMismatch {
            expected: map.text_len as usize,
            actual: text.len(),
        });
    }
    let instructions = linear::sweep(map, text).ok_or(QuestControlMeaningError::UnalignedText)?;

    for (code, expected) in QUEST_CONFIRMED_CONTROL_MEANINGS {
        let actual = quest_control_meaning(code);
        if actual != Some(expected) {
            return Err(QuestControlMeaningError::MeaningTableMismatch {
                code,
                expected,
                actual,
            });
        }
    }

    // FF03: interpreter active word를 0으로 만들고 (stream + 9) & !3.
    require(
        &instructions,
        map,
        0x8003_DEEC,
        "addiu v1, zero, -4",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: -4,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_DEF0,
        "sw zero, 0(a0)",
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::A0,
            offset: 0,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_DEF4,
        "addiu v0, v0, 9",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 9,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_DEF8,
        "and v0, v0, v1",
        K::And {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::V1,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_DF00,
        "sw v0, 0x1C(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    )?;

    // FF04: helper가 x를 원점으로 되돌리고 y를 행 높이+간격만큼 전진한다.
    require(
        &instructions,
        map,
        0x8003_BB08,
        "jal 0x8002C664",
        K::Jal {
            target: QUEST_ADVANCE_LINE_RUNTIME,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C698,
        "lhu a1, 6(v0)",
        K::Lhu {
            rt: Reg::A1,
            base: Reg::V0,
            offset: 6,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C69C,
        "addu a0, a0, v1",
        K::Addu {
            rd: Reg::A0,
            rs: Reg::A0,
            rt: Reg::V1,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C6A4,
        "sh a1, 0x0A(v0)",
        K::Sh {
            rt: Reg::A1,
            base: Reg::V0,
            offset: 0x0A,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C6A8,
        "lbu a1, 0x13(v0)",
        K::Lbu {
            rt: Reg::A1,
            base: Reg::V0,
            offset: 0x13,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C6AC,
        "addu v1, v1, a0",
        K::Addu {
            rd: Reg::V1,
            rs: Reg::V1,
            rt: Reg::A0,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C6B0,
        "sh v1, 0x0C(v0)",
        K::Sh {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x0C,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C6B8,
        "sb a1, 0x14(v0)",
        K::Sb {
            rt: Reg::A1,
            base: Reg::V0,
            offset: 0x14,
        },
    )?;

    // FF05: 같은 window index의 객체를 해제하고 active·renderer를 지운다.
    require(
        &instructions,
        map,
        0x8003_DF18,
        "jal 0x8002C6BC",
        K::Jal {
            target: QUEST_CLOSE_TEXT_WINDOW_RUNTIME,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C6F4,
        "andi s1, s3, 0xFF",
        K::Andi {
            rt: Reg(17),
            rs: Reg(19),
            imm: 0xFF,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C6FC,
        "lbu v0, 0xDE(s0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::S0,
            offset: 0xDE,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C704,
        "bne s1, v0, 3",
        K::Bne {
            rs: Reg(17),
            rt: Reg::V0,
            offset: 3,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C70C,
        "jal 0x80014CD0",
        K::Jal {
            target: QUEST_RELEASE_OBJECT_RUNTIME,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C748,
        "lhu v1, 6(v0)",
        K::Lhu {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 6,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C74C,
        "lhu a0, 8(v0)",
        K::Lhu {
            rt: Reg::A0,
            base: Reg::V0,
            offset: 8,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C754,
        "sb zero, 0x1F(v0)",
        K::Sb {
            rt: Reg::ZERO,
            base: Reg::V0,
            offset: 0x1F,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C758,
        "sw zero, 0x64(v0)",
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::V0,
            offset: 0x64,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C75C,
        "sw zero, 0x68(v0)",
        K::Sw {
            rt: Reg::ZERO,
            base: Reg::V0,
            offset: 0x68,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C760,
        "sh v1, 0x0A(v0)",
        K::Sh {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x0A,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C764,
        "sh a0, 0x0C(v0)",
        K::Sh {
            rt: Reg::A0,
            base: Reg::V0,
            offset: 0x0C,
        },
    )?;

    // FF06: opcode 뒤 u8을 interpreter의 active window index로 쓴다.
    require(
        &instructions,
        map,
        0x8003_DF50,
        "lbu v1, 2(v1)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::V1,
            offset: 2,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_DF60,
        "sb v1, 0xDE(a0)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0xDE,
        },
    )?;

    // FF07: 두 signed word를 12-bit fixed point로 바꿔 현재·기준 위치에 쓴다.
    require(
        &instructions,
        map,
        0x8003_BB60,
        "lh v1, 2(a2)",
        K::Lh {
            rt: Reg::V1,
            base: Reg::A2,
            offset: 2,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_BB68,
        "sll v1, v1, 12",
        K::Sll {
            rd: Reg::V1,
            rt: Reg::V1,
            sa: 12,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_BB6C,
        "sw v1, 0x4C(v0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x4C,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_BB98,
        "lh v1, 2(a2)",
        K::Lh {
            rt: Reg::V1,
            base: Reg::A2,
            offset: 2,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_BBA0,
        "sll v1, v1, 12",
        K::Sll {
            rd: Reg::V1,
            rt: Reg::V1,
            sa: 12,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_BBA4,
        "sw v1, 0x50(v0)",
        K::Sw {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x50,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_BBD8,
        "lw v0, 0x4C(v1)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x4C,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_BBE0,
        "sw v0, 0x90(v1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x90,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_BC0C,
        "lw v0, 0x50(v1)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x50,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_BC14,
        "sw v0, 0x94(v1)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::V1,
            offset: 0x94,
        },
    )?;

    // FF09: 두 word를 window 원점 +6/+8에 쓴다.
    require(
        &instructions,
        map,
        0x8003_E01C,
        "lhu v1, 2(a2)",
        K::Lhu {
            rt: Reg::V1,
            base: Reg::A2,
            offset: 2,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_E024,
        "sh v1, 6(v0)",
        K::Sh {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 6,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_E04C,
        "lhu v1, 2(a2)",
        K::Lhu {
            rt: Reg::V1,
            base: Reg::A2,
            offset: 2,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_E054,
        "sh v1, 8(v0)",
        K::Sh {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 8,
        },
    )?;

    // FF12: 현재 +0x14보다 큰 인자만 새 행 높이로 저장한다.
    require(
        &instructions,
        map,
        0x8003_E33C,
        "lbu v1, 2(a1)",
        K::Lbu {
            rt: Reg::V1,
            base: Reg::A1,
            offset: 2,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_E340,
        "lbu v0, 0x14(a0)",
        K::Lbu {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x14,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_E348,
        "sltu v0, v0, v1",
        K::Sltu {
            rd: Reg::V0,
            rs: Reg::V0,
            rt: Reg::V1,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_E34C,
        "beq v0, zero, 2",
        K::Beq {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 2,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_E354,
        "sb v1, 0x14(a0)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::A0,
            offset: 0x14,
        },
    )?;

    // FF14: 두 word를 현재 cursor +0x0A/+0x0C에 쓴다.
    require(
        &instructions,
        map,
        0x8003_E47C,
        "lhu v1, 2(a2)",
        K::Lhu {
            rt: Reg::V1,
            base: Reg::A2,
            offset: 2,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_E484,
        "sh v1, 0x0A(v0)",
        K::Sh {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x0A,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_E4AC,
        "lhu v1, 2(a2)",
        K::Lhu {
            rt: Reg::V1,
            base: Reg::A2,
            offset: 2,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_E4B4,
        "sh v1, 0x0C(v0)",
        K::Sh {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x0C,
        },
    )?;

    // FF15: window를 active로 만들고 helper가 없던 renderer를 생성·연결한다.
    require(
        &instructions,
        map,
        0x8003_E500,
        "addiu v1, zero, 1",
        K::Addiu {
            rt: Reg::V1,
            rs: Reg::ZERO,
            imm: 1,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_E504,
        "sb v1, 0x1F(v0)",
        K::Sb {
            rt: Reg::V1,
            base: Reg::V0,
            offset: 0x1F,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_E550,
        "sb zero, 0x21(v0)",
        K::Sb {
            rt: Reg::ZERO,
            base: Reg::V0,
            offset: 0x21,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_E558,
        "jal 0x8002C22C",
        K::Jal {
            target: QUEST_OPEN_TEXT_WINDOW_RUNTIME,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C264,
        "lw v0, 0x68(s0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: 0x68,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C26C,
        "bne v0, zero, 23",
        K::Bne {
            rs: Reg::V0,
            rt: Reg::ZERO,
            offset: 23,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C278,
        "jal 0x80014B14",
        K::Jal {
            target: QUEST_ALLOCATE_OBJECT_RUNTIME,
        },
    )?;
    require(
        &instructions,
        map,
        0x8002_C2C8,
        "sw v0, 0x68(s0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::S0,
            offset: 0x68,
        },
    )?;

    // FF1F: stream load, +2, store밖에 하지 않는다.
    require(
        &instructions,
        map,
        0x8003_DDDC,
        "lw v0, 0x1C(a0)",
        K::Lw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_DDE0,
        "sll zero, zero, 0",
        K::Sll {
            rd: Reg::ZERO,
            rt: Reg::ZERO,
            sa: 0,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_DDE4,
        "addiu v0, v0, 2",
        K::Addiu {
            rt: Reg::V0,
            rs: Reg::V0,
            imm: 2,
        },
    )?;
    require(
        &instructions,
        map,
        0x8003_DDE8,
        "jr ra",
        K::Jr { rs: Reg::RA },
    )?;
    require(
        &instructions,
        map,
        0x8003_DDEC,
        "sw v0, 0x1C(a0)",
        K::Sw {
            rt: Reg::V0,
            base: Reg::A0,
            offset: 0x1C,
        },
    )?;

    for &(address, expected, kind) in QUEST_ADDITIONAL_CONTROL_MEANING_EVIDENCE {
        require(&instructions, map, address, expected, kind)?;
    }

    Ok(QuestControlMeaningSummary {
        confirmed_meanings: QUEST_CONFIRMED_CONTROL_MEANINGS.len(),
        advance_line_runtime: QUEST_ADVANCE_LINE_RUNTIME,
        close_text_window_runtime: QUEST_CLOSE_TEXT_WINDOW_RUNTIME,
        open_text_window_runtime: QUEST_OPEN_TEXT_WINDOW_RUNTIME,
        release_object_runtime: QUEST_RELEASE_OBJECT_RUNTIME,
        allocate_object_runtime: QUEST_ALLOCATE_OBJECT_RUNTIME,
    })
}

fn require(
    instructions: &[Instruction],
    map: &ExeMap,
    address: u32,
    expected: &'static str,
    kind: K,
) -> Result<(), QuestControlMeaningError> {
    let offset = map
        .text_offset(address)
        .ok_or(QuestControlMeaningError::MissingInstruction(address))?;
    let actual = instructions
        .get(offset / 4)
        .filter(|instruction| instruction.addr == address)
        .ok_or(QuestControlMeaningError::MissingInstruction(address))?;
    if actual.kind != kind {
        return Err(QuestControlMeaningError::InstructionMismatch {
            address,
            expected,
            actual: actual.kind,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "quest_control_tests.rs"]
mod tests;
