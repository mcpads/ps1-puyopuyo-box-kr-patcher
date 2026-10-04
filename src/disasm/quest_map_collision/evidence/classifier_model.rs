//! QUEST collision classifier 모델의 typed instruction 증거.

use crate::isa::{InstructionKind as K, Reg};

use super::super::{
    QUEST_COLLISION_ACTOR_LAYER_OFFSET, QUEST_COLLISION_CLASSIFIER_RUNTIME,
    QUEST_COLLISION_SPECIAL_CLASS, QUEST_COLLISION_TILE_FLAG_MASKS,
    QUEST_COLLISION_TILE_LOW_BYTE_MASK,
};

pub(super) fn instructions() -> Vec<(u32, &'static str, K)> {
    vec![
        (
            QUEST_COLLISION_CLASSIFIER_RUNTIME,
            "preserve raw collision tile",
            K::Addu {
                rd: Reg::A2,
                rs: Reg::A1,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8006_B938,
            "compare collision ID below 2",
            K::Slt {
                rd: Reg::V0,
                rs: Reg::A1,
                rt: Reg::V0,
            },
        ),
        (
            0x8006_B93C,
            "branch collision ID below 2 to plane selection",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 13,
            },
        ),
        (
            0x8006_B948,
            "branch collision ID at least 6 to plane selection",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 9,
            },
        ),
        (
            0x8006_B94C,
            "preserve low ID for 3 through 5",
            K::Andi {
                rt: Reg::V1,
                rs: Reg::A2,
                imm: QUEST_COLLISION_TILE_LOW_BYTE_MASK,
            },
        ),
        (
            0x8006_B950,
            "dispatch collision IDs 3 through 5",
            K::J {
                target: 0x8006_B9A8,
            },
        ),
        (
            0x8006_B960,
            "dispatch collision ID 2 for nonzero layer",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 16,
            },
        ),
        (
            0x8006_B964,
            "preserve collision ID 2",
            K::Andi {
                rt: Reg::V1,
                rs: Reg::A2,
                imm: QUEST_COLLISION_TILE_LOW_BYTE_MASK,
            },
        ),
        (
            0x8006_B970,
            "mask selected-plane override flag",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::A2,
                imm: QUEST_COLLISION_TILE_FLAG_MASKS[1],
            },
        ),
        (
            0x8006_B974,
            "dispatch selected-plane override",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 11,
            },
        ),
        (
            0x8006_B978,
            "preserve plane-selected low ID",
            K::Andi {
                rt: Reg::V1,
                rs: Reg::A2,
                imm: QUEST_COLLISION_TILE_LOW_BYTE_MASK,
            },
        ),
        (
            0x8006_B984,
            "branch nonzero actor layer",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 5,
            },
        ),
        (
            0x8006_B98C,
            "dispatch lower actor plane",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 6,
            },
        ),
        (
            0x8006_B994,
            "block lower actor against upper plane",
            K::J {
                target: 0x8006_B968,
            },
        ),
        (
            0x8006_B99C,
            "block upper actor against lower plane",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: -14,
            },
        ),
        (
            0x8006_B9A0,
            "preserve upper-plane low ID",
            K::Andi {
                rt: Reg::V1,
                rs: Reg::A2,
                imm: QUEST_COLLISION_TILE_LOW_BYTE_MASK,
            },
        ),
        (
            0x8006_B9A8,
            "branch primary-only collision ID 102",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 103,
            },
        ),
        (
            0x8006_B9AC,
            "split collision IDs below 103",
            K::Slti {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 103,
            },
        ),
        (
            0x8006_B9B0,
            "branch collision IDs at least 103",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 16,
            },
        ),
        (
            0x8006_B9B8,
            "branch directional collision ID 6",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 69,
            },
        ),
        (
            0x8006_B9BC,
            "split collision IDs below 7",
            K::Slti {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 7,
            },
        ),
        (
            0x8006_B9C0,
            "branch collision IDs 7 through 102",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 5,
            },
        ),
        (
            0x8006_B9C8,
            "branch collision ID 1 to blocking return",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: -25,
            },
        ),
        (
            0x8006_B9CC,
            "return no collision for other low IDs",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8006_B9D0,
            "finish no-collision low ID",
            K::J {
                target: 0x8006_BB94,
            },
        ),
        (
            0x8006_B9DC,
            "branch first clear-condition collision ID",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 21,
            },
        ),
        (
            0x8006_B9E4,
            "branch second clear-condition collision ID",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 47,
            },
        ),
        (
            0x8006_B9EC,
            "finish unmatched ID below 103",
            K::J {
                target: 0x8006_BB94,
            },
        ),
        (
            0x8006_B9F8,
            "branch second set-condition collision ID",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 49,
            },
        ),
        (
            0x8006_B9FC,
            "split collision IDs below 111",
            K::Slti {
                rt: Reg::V0,
                rs: Reg::V1,
                imm: 111,
            },
        ),
        (
            0x8006_BA00,
            "branch collision IDs at least 111",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 5,
            },
        ),
        (
            0x8006_BA08,
            "branch first set-condition collision ID",
            K::Beq {
                rs: Reg::V1,
                rt: Reg::V0,
                offset: 27,
            },
        ),
        (
            0x8006_BA10,
            "finish unmatched ID below 111",
            K::J {
                target: 0x8006_BB94,
            },
        ),
        (
            0x8006_BA1C,
            "reject collision IDs above primary range",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 92,
            },
        ),
        (
            0x8006_BA24,
            "reject collision IDs below primary range",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 91,
            },
        ),
        (
            0x8006_BA2C,
            "dispatch primary-only collision range",
            K::J {
                target: 0x8006_BB48,
            },
        ),
        (
            0x8006_BA40,
            "disable first clear-condition ID when condition is set",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 84,
            },
        ),
        (
            0x8006_BA50,
            "branch first condition lower layer",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 5,
            },
        ),
        (
            0x8006_BA58,
            "reject first condition upper actor on lower plane",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 78,
            },
        ),
        (
            0x8006_BA60,
            "block first condition upper actor on upper plane",
            K::J {
                target: 0x8006_B968,
            },
        ),
        (
            0x8006_BA68,
            "reject first condition lower actor on upper plane",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 74,
            },
        ),
        (
            0x8006_BA70,
            "block first condition lower actor on lower plane",
            K::J {
                target: 0x8006_B968,
            },
        ),
        (
            0x8006_BA84,
            "disable first set-condition ID when condition is clear",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 67,
            },
        ),
        (
            0x8006_BA94,
            "branch set-condition upper layer",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: -16,
            },
        ),
        (
            0x8006_BA9C,
            "dispatch set-condition lower layer",
            K::J {
                target: 0x8006_BA68,
            },
        ),
        (
            0x8006_BAB0,
            "disable second clear-condition ID when condition is set",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 56,
            },
        ),
        (
            0x8006_BAB8,
            "dispatch second clear-condition plane check",
            K::J {
                target: 0x8006_BA8C,
            },
        ),
        (
            0x8006_BAC8,
            "dispatch second set-condition predicate",
            K::J {
                target: 0x8006_BA80,
            },
        ),
        (
            0x8006_BAD0,
            "load directional actor layer",
            K::Lw {
                rt: Reg::V0,
                base: Reg::A0,
                offset: QUEST_COLLISION_ACTOR_LAYER_OFFSET,
            },
        ),
        (
            0x8006_BAD8,
            "branch directional lower actor layer",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 14,
            },
        ),
        (
            0x8006_BADC,
            "mask directional plane flag",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::A2,
                imm: QUEST_COLLISION_TILE_FLAG_MASKS[0],
            },
        ),
        (
            0x8006_BAE0,
            "reject directional upper actor on lower plane",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 44,
            },
        ),
        (
            0x8006_BAF0,
            "block directional tile outside direction zero",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: -99,
            },
        ),
        (
            0x8006_BB04,
            "return special contact for directional input",
            K::Bne {
                rs: Reg::V1,
                rt: Reg::ZERO,
                offset: 35,
            },
        ),
        (
            0x8006_BB08,
            "load directional special-contact class",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: i16::from(QUEST_COLLISION_SPECIAL_CLASS),
            },
        ),
        (
            0x8006_BB0C,
            "block directional tile without input",
            K::J {
                target: 0x8006_B968,
            },
        ),
        (
            0x8006_BB14,
            "reject directional lower actor on upper plane",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 31,
            },
        ),
        (
            0x8006_BB24,
            "block lower-plane directional tile outside direction zero",
            K::Bne {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: -112,
            },
        ),
        (
            0x8006_BB38,
            "return lower-plane special contact for directional input",
            K::Bne {
                rs: Reg::V1,
                rt: Reg::ZERO,
                offset: 22,
            },
        ),
        (
            0x8006_BB3C,
            "load lower-plane special-contact class",
            K::Addiu {
                rt: Reg::V0,
                rs: Reg::ZERO,
                imm: i16::from(QUEST_COLLISION_SPECIAL_CLASS),
            },
        ),
        (
            0x8006_BB40,
            "block lower-plane directional tile without input",
            K::J {
                target: 0x8006_B968,
            },
        ),
        (
            0x8006_BB64,
            "branch primary actor lower layer",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 8,
            },
        ),
        (
            0x8006_BB68,
            "mask primary actor plane flag",
            K::Andi {
                rt: Reg::V0,
                rs: Reg::A2,
                imm: QUEST_COLLISION_TILE_FLAG_MASKS[0],
            },
        ),
        (
            0x8006_BB6C,
            "reject primary upper actor on lower plane",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: 8,
            },
        ),
        (
            0x8006_BB88,
            "accept primary lower actor on lower plane",
            K::Beq {
                rs: Reg::V0,
                rt: Reg::ZERO,
                offset: -6,
            },
        ),
        (
            0x8006_BB90,
            "return no collision for rejected primary tile",
            K::Addu {
                rd: Reg::V0,
                rs: Reg::ZERO,
                rt: Reg::ZERO,
            },
        ),
        (
            0x8006_BB94,
            "return classified collision class",
            K::Jr { rs: Reg::RA },
        ),
    ]
}
