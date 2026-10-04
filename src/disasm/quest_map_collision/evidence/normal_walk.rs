//! QUEST player 정상 보행 dispatch와 위치 commit의 typed source 증거.

use crate::isa::{InstructionKind as K, Reg};

use crate::disasm::quest_player_input::{
    QUEST_PLAYER_HELD_INPUT_SIGNED_OFFSET, QUEST_PLAYER_INPUT_LUI_IMMEDIATE,
};

use super::super::{
    QUEST_COLLISION_ACTOR_FIXED_X_OFFSET, QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET,
    QUEST_COLLISION_FIXED_POINT_SHIFT, QUEST_MAP_MOVEMENT_RUNTIME,
    QUEST_PLAYER_COLLISION_CANCEL_RUNTIME, QUEST_PLAYER_MAP_MOVEMENT_CALL_RUNTIME,
    QUEST_PLAYER_NORMAL_MOVEMENT_INPUT_MASKS, QUEST_PLAYER_NORMAL_MOVEMENT_STATE,
    QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS, QUEST_PLAYER_OBJECT_MOVEMENT_CALL_RUNTIME,
    QUEST_PLAYER_POSITION_COMMIT_RUNTIME,
};

const QUEST_PLAYER_STATE_DISPATCH_TABLE_RUNTIME: u32 = 0x8001_2530;
const QUEST_PLAYER_MOVEMENT_DISPATCH_TABLE_RUNTIME: u32 = 0x8001_2688;
const QUEST_PLAYER_NORMAL_STATE_HANDLER_RUNTIME: u32 = 0x8005_7328;
const QUEST_PLAYER_NORMAL_MOVEMENT_HANDLER_RUNTIME: u32 = 0x8005_7E80;

pub(super) fn instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        (
            0x8005_60DC,
            "load player state before behavior dispatch",
            K::Lh {
                rt: Reg::V1,
                base: Reg::S4,
                offset: 0xC6,
            },
        ),
        (
            0x8005_60E4,
            "bound player state dispatch",
            K::Sltiu {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 19,
            },
        ),
        (
            0x8005_60EC,
            "load player state dispatch table high address",
            K::Lui {
                rt: Reg::V0,
                imm: 0x8001,
            },
        ),
        (
            0x8005_60F0,
            "load player state dispatch table",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_STATE_DISPATCH_TABLE_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8005_60F4,
            "scale player state to dispatch entry",
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
        ),
        (
            0x8005_60F8,
            "address player state dispatch entry",
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8005_60FC,
            "read player state handler",
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            0x8005_6104,
            "dispatch player state handler",
            K::Jr { rs: Reg::V0 },
        ),
        (
            0x8005_7338,
            "load normal movement input high address",
            K::Lui {
                rt: Reg::A0,
                imm: QUEST_PLAYER_INPUT_LUI_IMMEDIATE,
            },
        ),
        (
            0x8005_7540,
            "load normal movement input word",
            K::Lhu {
                rt: Reg::V1,
                base: Reg::A0,
                offset: QUEST_PLAYER_HELD_INPUT_SIGNED_OFFSET,
            },
        ),
        (
            0x8005_7548,
            "mask left movement input",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: QUEST_PLAYER_NORMAL_MOVEMENT_INPUT_MASKS[1],
            },
        ),
        (
            0x8005_7554,
            "set negative horizontal movement",
            K::Addiu {
                rt: Reg::S6,
                rs: Reg::S6,
                imm: -1,
            },
        ),
        (
            0x8005_7550,
            "mask right movement input",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: QUEST_PLAYER_NORMAL_MOVEMENT_INPUT_MASKS[3],
            },
        ),
        (
            0x8005_7560,
            "set positive horizontal movement",
            K::Addiu {
                rt: Reg::S6,
                rs: Reg::S6,
                imm: 1,
            },
        ),
        (
            0x8005_7564,
            "give horizontal movement priority",
            K::Bne {
                rs: Reg::S6,
                rt: Reg::ZERO,
                offset: 8,
            },
        ),
        (
            0x8005_7568,
            "mask up movement input",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: QUEST_PLAYER_NORMAL_MOVEMENT_INPUT_MASKS[2],
            },
        ),
        (
            0x8005_7574,
            "set negative vertical movement",
            K::Addiu {
                rt: Reg::S7,
                rs: Reg::S7,
                imm: -1,
            },
        ),
        (
            0x8005_7570,
            "mask down movement input",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: QUEST_PLAYER_NORMAL_MOVEMENT_INPUT_MASKS[0],
            },
        ),
        (
            0x8005_7584,
            "set positive vertical movement",
            K::Addiu {
                rt: Reg::S7,
                rs: Reg::S7,
                imm: 1,
            },
        ),
        (
            0x8005_7648,
            "scale horizontal movement to fixed delta",
            K::Sll {
                rd: Reg::V1,
                rt: Reg::S6,
                sa: QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS.trailing_zeros() as u8
                    + QUEST_COLLISION_FIXED_POINT_SHIFT,
            },
        ),
        (
            0x8005_764C,
            "add horizontal movement to pending delta",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8005_7650,
            "store pending horizontal movement delta",
            K::Sw {
                rt: Reg::V0,
                base: Reg::SP,
                offset: 0xD8,
            },
        ),
        (
            0x8005_7658,
            "scale vertical movement to fixed delta",
            K::Sll {
                rd: Reg::V1,
                rt: Reg::S7,
                sa: QUEST_PLAYER_NORMAL_MOVEMENT_STEP_PIXELS.trailing_zeros() as u8
                    + QUEST_COLLISION_FIXED_POINT_SHIFT,
            },
        ),
        (
            0x8005_765C,
            "add vertical movement to pending delta",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8005_7664,
            "store pending vertical movement delta",
            K::Sw {
                rt: Reg::V0,
                base: Reg::SP,
                offset: 0xDC,
            },
        ),
        (
            0x8005_7DE0,
            "load player state before movement dispatch",
            K::Lh {
                rt: Reg::V1,
                base: Reg::S4,
                offset: 0xC6,
            },
        ),
        (
            0x8005_7DF4,
            "load player movement dispatch table",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: QUEST_PLAYER_MOVEMENT_DISPATCH_TABLE_RUNTIME as u16 as i16,
            },
        ),
        (
            0x8005_7DF8,
            "scale player state to movement dispatch entry",
            K::Sll {
                rd: Reg::V1,
                rt: Reg::V1,
                sa: 2,
            },
        ),
        (
            0x8005_7DFC,
            "address player movement dispatch entry",
            K::Addu {
                rd: Reg::V1,
                rs: Reg::V1,
                rt: Reg::V0,
            },
        ),
        (
            0x8005_7E00,
            "read player movement handler",
            K::Lw {
                rt: Reg::V0,
                base: Reg::V1,
                offset: 0,
            },
        ),
        (
            0x8005_7E08,
            "dispatch player movement handler",
            K::Jr { rs: Reg::V0 },
        ),
        (
            0x8005_7E8C,
            "bind pending horizontal delta argument",
            K::Addiu {
                rt: Reg::S2,
                rs: Reg::SP,
                imm: 0xD8,
            },
        ),
        (
            0x8005_7E94,
            "bind pending vertical delta argument",
            K::Addiu {
                rt: Reg::S1,
                rs: Reg::SP,
                imm: 0xDC,
            },
        ),
        (
            QUEST_PLAYER_MAP_MOVEMENT_CALL_RUNTIME,
            "call map collision before position commit",
            K::Jal {
                target: QUEST_MAP_MOVEMENT_RUNTIME,
            },
        ),
        (
            0x8005_7EA8,
            "normalize map collision result",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::V0,
                imm: 0xFF,
            },
        ),
        (
            0x8005_7EAC,
            "convert map collision result to movement cancellation flag",
            K::Sltu {
                rd: Reg::V0,
                rs: Reg::ZERO,
                rt: Reg::V0,
            },
        ),
        (
            0x8005_7EB0,
            "retain movement cancellation flag",
            K::Addu {
                rd: Reg::S5,
                rs: Reg::V0,
                rt: Reg::ZERO,
            },
        ),
        (
            QUEST_PLAYER_OBJECT_MOVEMENT_CALL_RUNTIME,
            "call object collision before position commit",
            K::Jal {
                target: 0x8006_F0EC,
            },
        ),
        (
            QUEST_PLAYER_COLLISION_CANCEL_RUNTIME,
            "branch when collision did not cancel movement",
            K::Beq {
                rs: Reg::S5,
                rt: Reg::ZERO,
                offset: 3,
            },
        ),
        (
            0x8005_8008,
            "cancel horizontal movement delta",
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::SP,
                offset: 0xD8,
            },
        ),
        (
            0x8005_800C,
            "cancel vertical movement delta",
            K::Sw {
                rt: Reg::ZERO,
                base: Reg::SP,
                offset: 0xDC,
            },
        ),
        (
            QUEST_PLAYER_POSITION_COMMIT_RUNTIME,
            "load horizontal position for final commit",
            K::Lw {
                rt: Reg::V0,
                base: Reg::S4,
                offset: QUEST_COLLISION_ACTOR_FIXED_X_OFFSET,
            },
        ),
        (
            0x8005_81D0,
            "add surviving horizontal movement delta",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8005_81D4,
            "commit horizontal position",
            K::Sw {
                rt: Reg::V0,
                base: Reg::S4,
                offset: QUEST_COLLISION_ACTOR_FIXED_X_OFFSET,
            },
        ),
        (
            0x8005_81EC,
            "add surviving vertical movement delta",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::V0,
                rt: Reg::V1,
            },
        ),
        (
            0x8005_81F0,
            "commit vertical position",
            K::Sw {
                rt: Reg::V0,
                base: Reg::S4,
                offset: QUEST_COLLISION_ACTOR_FIXED_Y_OFFSET,
            },
        ),
    ]
}

pub(super) fn data_words() -> Vec<(u32, &'static str, u32)> {
    vec![
        (
            QUEST_PLAYER_STATE_DISPATCH_TABLE_RUNTIME
                + u32::try_from(QUEST_PLAYER_NORMAL_MOVEMENT_STATE).expect("state 0") * 4,
            "normal player state handler",
            QUEST_PLAYER_NORMAL_STATE_HANDLER_RUNTIME,
        ),
        (
            QUEST_PLAYER_MOVEMENT_DISPATCH_TABLE_RUNTIME
                + u32::try_from(QUEST_PLAYER_NORMAL_MOVEMENT_STATE).expect("state 0") * 4,
            "normal player movement handler",
            QUEST_PLAYER_NORMAL_MOVEMENT_HANDLER_RUNTIME,
        ),
    ]
}
